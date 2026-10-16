//! 拒否・問い返し・注記の文面への、実行時の値の差し込み（JUDGE-8、TASK-274、語彙 v1.1 の EP-7）。
//!
//! 文面の枠 `{{名前|代替の文}}` の書式・閉じた 6 語・差し込みの純関数は contract の `slot` が持つ。本モジュールは、
//! その枠に入れる実行時の値（入力の先頭の語・近いコマンド・Step の題名と `do`・処理中の残りと全体の回数）を
//! 教材の IR・実行状態・入力から組み立て、次の行動と文面を充填する。値の組み立てはここ 1 か所に寄せ、
//! 判定（[`crate::judge`]）と教材テスト（pack）・画面（web）が同じ値を読む。
//!
//! 呼び出し元は、`Judged` の材料を作る [`crate::judge`]（次の行動の充填）と、文面（`message`）も要る
//! pack の教材テスト・web（[`filled_text`]）である。[`crate::rejection_text`] が返す値は「差し込み前」で、
//! 実行時に利用者へ出す値はこちらを通した後の値になる。静的な検査（pack の `slot_check`）は最悪の場合の値で
//! 数えるため、本モジュールの値の規則（特に `step_do` は題名で代えない）を揃えている。
//!
//! # 値の規則
//!
//! | 枠 | 値 |
//! |---|---|
//! | `input_head` | コマンドとして字句化できた入力だけ、教材の接頭辞とコマンド名。それ以外は値なし（代替の文） |
//! | `nearest_command` | 分類が `unknown_command` のときだけ。`commands` の名前から、編集距離（隣り合う 2 文字の入れ替えを 1 と数える）が [`NEAREST_MAX_DISTANCE`] 以下で最小のもの。`unsupported` と読めない値は除く。同じ距離なら名前の辞書順の先頭。接頭辞を付ける。[`NEAREST_MAX_NAME_CHARS`] 文字を超える入力・候補は計算せず候補なしにする |
//! | `step_title` | 現在の Step の題名 |
//! | `step_do` | 現在の Step の `do`。無ければ値なし（題名で代えない） |
//! | `remaining` | 処理中の残りの回数。処理中でなければ値なし |
//! | `total` | 処理中の全体の回数。処理中でなければ値なし |
//!
//! # 扱わないこと
//!
//! - 枠の書式・静的な診断・最悪の場合の値の数え上げ（contract の `slot`、pack の `slot_check`、TASK-46）
//! - 画面への表示とエスケープ（web、TASK-447・TASK-445）。結果は平文で、HTML として解釈しない
//! - 状態の変更・`Judged` の発行
//!
//! 外部への呼び出し・正規表現の評価は持たず、panic しない。編集距離の計算は `unknown_command` のときだけで、
//! コマンド表の件数に比例する。入力が長くても動的計画法の大きさが膨らまないよう、文字数の差が上限を超える候補は
//! 先に除く（距離は文字数の差より小さくならないため、結果は変わらない）。教材の文面は未信頼の入力として扱い、
//! 未知の枠・構文の誤り・差し込みの後の空は [`TextFillError`] にする（fail-closed）。`Display` は固定の英文で、
//! 入力値・教材の値・ID を埋め込まない。

use std::fmt;

use fandhe_workshop_contract::ir::{Ir, Step};
use fandhe_workshop_contract::judgment::{Judgment, LexedCommand, NextAction, RejectCategory};
use fandhe_workshop_contract::profile::read_profile;
use fandhe_workshop_contract::slot::{substitute, SlotValues};
use serde_json::Value;

use crate::command_class::CommandKind;
use crate::command_lex::lex_command;
use crate::input::Input;
use crate::session::RunState;

/// 近いコマンドとみなす編集距離の上限。これを超える候補は選ばない（代替の文が出る）。
pub const NEAREST_MAX_DISTANCE: usize = 2;

/// 近いコマンドの探索で距離を計算する名前の最大文字数。超える入力・候補は計算せず候補なしとして扱う。
/// 教材の名前は未信頼で長さを縛られていないので、計算量の上限をここで固定する。
pub const NEAREST_MAX_NAME_CHARS: usize = 64;

/// 充填できない理由。`Display` は入力値・教材の値・ID を埋め込まない固定の英文。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextFillError {
    /// 枠の構文の誤り、または閉じた集合にない名前。
    InvalidSlot,
    /// 差し込みの後に次の行動が空（または空白だけ）になった。
    EmptyNextAction,
    /// 値の材料（`pack.profile`・文面の欄・現在の Step）が読めない。
    InvalidDeclaration,
}

impl fmt::Display for TextFillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidSlot => "text slot cannot be filled",
            Self::EmptyNextAction => "next action is empty after filling",
            Self::InvalidDeclaration => "text fill input cannot be read",
        })
    }
}

impl std::error::Error for TextFillError {}

/// 充填した文面。画面と教材テストが読む。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilledText {
    /// 文面の ID（`Judged` の `text` と同じ）。
    pub text: String,
    /// 差し込みの後の文面。
    pub message: String,
    /// 差し込みの後の次の行動（`Judged` の欄と同じ値）。
    pub next_action: NextAction,
}

/// 隣り合う 2 文字の入れ替えを 1 と数える編集距離（最適文字列整列距離）。
pub fn osa_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev2: Vec<usize> = vec![0; b.len() + 1];
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut d = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d = d.min(prev2[j - 2] + 1);
            }
            cur[j] = d;
        }
        prev2 = prev;
        prev = cur;
    }
    prev[b.len()]
}

/// 距離が `max` 以下のときだけその値を返す、帯状の編集距離（OSA）。計算量は O(長さ × (2 × max + 1))。
///
/// 帯（`|i - j| <= max`）の外は距離が `max` を超えるので計算しない。行の最小値が `max` を超えたら打ち切る。
pub fn osa_distance_within(a: &str, b: &str, max: usize) -> Option<usize> {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (n, m) = (a.len(), b.len());
    if n.abs_diff(m) > max {
        return None;
    }
    const INF: usize = usize::MAX / 2;
    let mut prev2 = vec![INF; m + 1];
    let mut prev = vec![INF; m + 1];
    for (j, cell) in prev.iter_mut().enumerate().take(max.min(m) + 1) {
        *cell = j;
    }
    for i in 1..=n {
        let mut cur = vec![INF; m + 1];
        let lo = i.saturating_sub(max);
        let hi = i.saturating_add(max).min(m);
        if lo == 0 {
            cur[0] = i;
        }
        let mut row_min = if lo == 0 { i } else { INF };
        for j in lo.max(1)..=hi {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut d = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d = d.min(prev2[j - 2] + 1);
            }
            cur[j] = d;
            row_min = row_min.min(d);
        }
        if row_min > max {
            return None;
        }
        prev2 = prev;
        prev = cur;
    }
    (prev[m] <= max).then_some(prev[m])
}

/// 表にない名前 `name` に近いコマンドを選ぶ。接頭辞を付けて返す。候補が無ければ `None`。
///
/// `unsupported` の名前と、種類を読めない値の名前は候補にしない（エラーにもしない）。同じ距離のときは、
/// 厳密に小さいときだけ更新するので、`BTreeMap` の順（名前の辞書順）の先頭が残る。
pub fn nearest_command(ir: &Ir, prefix: &str, name: &str) -> Option<String> {
    let len = name.chars().count();
    if len > NEAREST_MAX_NAME_CHARS {
        return None;
    }
    let mut best: Option<(usize, &str)> = None;
    for (cand, kind) in &ir.commands {
        if matches!(
            CommandKind::from_value(kind),
            None | Some(CommandKind::Unsupported)
        ) {
            continue;
        }
        let cand_len = cand.chars().count();
        if cand_len > NEAREST_MAX_NAME_CHARS || cand_len.abs_diff(len) > NEAREST_MAX_DISTANCE {
            continue;
        }
        let Some(d) = osa_distance_within(name, cand, NEAREST_MAX_DISTANCE) else {
            continue;
        };
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, cand));
        }
    }
    best.map(|(_, c)| format!("{prefix}{c}"))
}

/// 差し込む値の組を作る。`category` は拒否の分類（`unknown_command` のときだけ近い候補を計算する）。
pub fn slot_values(
    ir: &Ir,
    step: &Step,
    run: &RunState,
    input: &Input,
    category: Option<RejectCategory>,
) -> Result<SlotValues, TextFillError> {
    let prefix = read_profile(ir.pack.profile.as_ref())
        .map_err(|_| TextFillError::InvalidDeclaration)?
        .command_prefix;
    let lexed: Option<LexedCommand> = match input {
        Input::Command(raw) => lex_command(&prefix, raw).ok(),
        _ => None,
    };
    let input_head = lexed
        .as_ref()
        .filter(|l| !l.name.is_empty())
        .map(|l| format!("{prefix}{}", l.name));
    let nearest = match (category, &lexed) {
        (Some(RejectCategory::UnknownCommand), Some(l)) => nearest_command(ir, &prefix, &l.name),
        _ => None,
    };
    let (remaining, total) = match &run.busy {
        Some(b) => (
            Some(u64::from(b.total.saturating_sub(b.done))),
            Some(u64::from(b.total)),
        ),
        None => (None, None),
    };
    Ok(SlotValues {
        input_head,
        nearest_command: nearest,
        step_title: step.title.clone(),
        step_do: step.do_.clone(),
        remaining,
        total,
    })
}

/// 文面 1 件を充填する。未知の枠・構文の誤りは [`TextFillError::InvalidSlot`]。
pub fn fill_message(text: &str, values: &SlotValues) -> Result<String, TextFillError> {
    substitute(text, values).map_err(|_| TextFillError::InvalidSlot)
}

/// 次の行動を充填する。差し込みの後に空なら [`TextFillError::EmptyNextAction`]（空の次の行動は作らない）。
pub fn fill_next_action(text: &str, values: &SlotValues) -> Result<NextAction, TextFillError> {
    let filled = fill_message(text, values)?;
    NextAction::new(filled).map_err(|_| TextFillError::EmptyNextAction)
}

/// `Judged` の結果（拒否・問い返し・自然文の受付の注記）の文面を、実行時の値で充填する。
///
/// `run` は判定の直前の状態である（拒否・問い返しは状態を変えないので直後と同じ）。それ以外の結果は
/// `Ok(None)`。次の行動は `judgment` が持つ差し込み済みの値をそのまま返し、二重に充填しない。
pub fn filled_text(
    ir: &Ir,
    run: &RunState,
    input: &Input,
    judgment: &Judgment,
) -> Result<Option<FilledText>, TextFillError> {
    let (table, id, next_action, category) = match judgment {
        Judgment::Rejected(r) => (&ir.rejections, &r.text, &r.next_action, Some(r.category)),
        Judgment::Clarified(c) => (&ir.clarifications, &c.text, &c.next_action, None),
        Judgment::Accepted(Some(n)) => (&ir.rejections, &n.text, &n.next_action, None),
        _ => return Ok(None),
    };
    let uid = run
        .current
        .as_ref()
        .ok_or(TextFillError::InvalidDeclaration)?;
    let node = ir
        .nodes
        .iter()
        .find(|n| n.uid == uid.0)
        .ok_or(TextFillError::InvalidDeclaration)?;
    let raw = table
        .get(id)
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .ok_or(TextFillError::InvalidDeclaration)?;
    let values = slot_values(ir, &node.step, run, input, category)?;
    Ok(Some(FilledText {
        text: id.clone(),
        message: fill_message(raw, &values)?,
        next_action: next_action.clone(),
    }))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn osa_counts_transposition_as_one() {
        assert_eq!(osa_distance("hlep", "help"), 1);
        assert_eq!(osa_distance("hep", "help"), 1);
        assert_eq!(osa_distance("helpp", "help"), 1);
        assert_eq!(osa_distance("hxlp", "help"), 1);
        assert_eq!(osa_distance("", "ab"), 2);
        assert_eq!(osa_distance("abc", "xyz"), 3);
        assert_eq!(osa_distance("日本語", "日本"), 1);
    }

    #[test]
    fn banded_matches_full_within_limit() {
        let words = [
            "",
            "a",
            "ab",
            "ba",
            "help",
            "hlep",
            "hep",
            "helpp",
            "xyz",
            "abcd",
            "badc",
            "日本語",
            "日本",
        ];
        for a in words {
            for b in words {
                let full = osa_distance(a, b);
                let got = osa_distance_within(a, b, 2);
                assert_eq!(got, (full <= 2).then_some(full), "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn max_usize_max_does_not_overflow() {
        assert_eq!(osa_distance_within("a", "a", usize::MAX), Some(0));
        assert_eq!(osa_distance_within("abc", "acb", usize::MAX), Some(1));
        assert_eq!(osa_distance_within("", "ab", usize::MAX), Some(2));
    }

    #[test]
    fn long_inputs_are_bounded() {
        let long = "a".repeat(20_000);
        let other = "b".repeat(20_000);
        assert_eq!(osa_distance_within(&long, &other, 2), None);
        let ir = ir_with(&[(other.as_str(), "builtin")]);
        assert_eq!(nearest_command(&ir, "/", &long), None);
        assert_eq!(nearest_command(&ir, "/", &other), None);
    }

    fn ir_with(commands: &[(&str, &str)]) -> Ir {
        let mut ir: Ir = serde_json::from_value(serde_json::json!({
            "format_version": 1,
            "pack": {"id": "p", "title": "t", "version": "0.0.1", "ir_format": 1, "guidance": "A"},
            "refs": [], "commands": {}, "views": {}, "fixtures": {}, "hints": {},
            "rejections": {}, "clarifications": {}, "defaults": {},
            "instances": [], "nodes": [], "starts": [], "fixture_docs": {}
        }))
        .expect("ir");
        let map: BTreeMap<String, Value> = commands
            .iter()
            .map(|(k, v)| ((*k).to_string(), Value::String((*v).to_string())))
            .collect();
        ir.commands = map;
        ir
    }

    #[test]
    fn nearest_picks_within_limit_and_prefixes() {
        let ir = ir_with(&[("help", "builtin"), ("status", "builtin")]);
        assert_eq!(nearest_command(&ir, "/", "hlep").as_deref(), Some("/help"));
        assert_eq!(nearest_command(&ir, "/", "hel").as_deref(), Some("/help"));
        assert_eq!(nearest_command(&ir, "/", "zzzzzz"), None);
    }

    #[test]
    fn nearest_limit_is_two() {
        let ir = ir_with(&[("help", "builtin")]);
        assert_eq!(nearest_command(&ir, "/", "he").as_deref(), Some("/help"));
        assert_eq!(nearest_command(&ir, "/", "h"), None);
    }

    #[test]
    fn nearest_skips_unsupported_and_unreadable() {
        let ir = ir_with(&[
            ("hela", "unsupported"),
            ("helq", "weird"),
            ("helps", "flow"),
        ]);
        assert_eq!(nearest_command(&ir, "/", "help").as_deref(), Some("/helps"));
        let ir = ir_with(&[("hela", "unsupported")]);
        assert_eq!(nearest_command(&ir, "/", "help"), None);
    }

    #[test]
    fn nearest_tie_goes_to_dictionary_order() {
        let ir = ir_with(&[("bat", "builtin"), ("cat", "builtin")]);
        assert_eq!(nearest_command(&ir, "/", "dat").as_deref(), Some("/bat"));
    }

    #[test]
    fn nearest_ignores_candidates_far_in_length() {
        let ir = ir_with(&[("a", "builtin")]);
        assert_eq!(nearest_command(&ir, "/", "abcdefghij"), None);
    }

    #[test]
    fn fill_next_action_fails_closed() {
        let v = SlotValues {
            input_head: None,
            nearest_command: None,
            step_title: "題".into(),
            step_do: None,
            remaining: None,
            total: None,
        };
        assert_eq!(
            fill_next_action("{{nope}}", &v),
            Err(TextFillError::InvalidSlot)
        );
        assert_eq!(
            fill_next_action("{{input_head}}", &v),
            Err(TextFillError::EmptyNextAction)
        );
        assert_eq!(
            fill_next_action("{{input_head|代替}}", &v)
                .map(|n| n.as_str().to_string())
                .as_deref(),
            Ok("代替")
        );
    }

    #[test]
    fn errors_are_fixed_text() {
        for e in [
            TextFillError::InvalidSlot,
            TextFillError::EmptyNextAction,
            TextFillError::InvalidDeclaration,
        ] {
            assert!(e.to_string().is_ascii());
        }
    }
}
