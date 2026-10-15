//! 教材テストの誤入力の期待 `expect` を、contract の判定の型で読む（TASK-45 2/2、PACK-11）。
//!
//! 呼び出し元は、結合試験（`tests/error_input_expect.rs`）と、後続の `fandhe-pack test`（TASK-404）である。
//! `compile`・`validate` には結線しない（`profile::check_play_inputs` と同じ扱い）。
//!
//! 判定結果・分類の型は contract の `judgment`（TASK-256）が正で、本モジュールは定義しない。[`ResultExpect`] は、
//! 期待の書き方 3 形（受付・拒否・問い返し）を contract の型の組で表す読み取りの結果にすぎない。
//! 語の解釈は [`crate::vocab::result_kind`]・[`crate::vocab::rejection_category`] を通す（`rejection.rs` と同じ入口）。
//!
//! 扱わないこと
//! - 誤入力をエンジンへ流して実際の判定と期待を比べる実行系（TASK-404。core の判定は `judge::judge_input`（TASK-267）で、`apply` への結線は TASK-207 待ち）。
//!   [`ResultExpect::matches`] は、その比較で使う照合の規則だけを持つ
//! - `pre[].expect`・`playthroughs` の期待（`progress`・`achieved` など語彙外の書き方を含む。TASK-404）
//! - `next_action` が空の拒否・問い返しの診断（`E-EMPTY-NEXT-ACTION`。TASK-46）
//!
//! 語彙の違反は `E-PARSE` にする（`rejection.rs` と同じ。`docs/diagnostics.md` 版 1 は
//! `E-PLAY-CATEGORY-MISMATCH` を固定しない）。診断の文面は固定文で、`expect` の値は写さない。

use core::fmt;

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::{Category, Judgment, JudgmentKind, RejectCategory};
use serde_json::Value;

use crate::diag::{DiagCode, Diagnostic};
use crate::source::{safe_ident, LoadedPack};
use crate::vocab::{rejection_category, result_kind};

/// 誤入力の期待。contract の型の組で、受付・拒否・問い返しの 3 形だけを表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultExpect {
    /// `accepted`（`free_text: false`）または `accepted:free_text_unjudged`（`true`）。
    Accepted {
        /// 自然文の受付の注記（`free_text_unjudged`）を期待するか。
        free_text: bool,
    },
    /// `rejected:<7 分類>`。`free_text_unjudged` の拒否は型の上で作れない。
    Rejected(RejectCategory),
    /// `clarified`。
    Clarified,
}

impl ResultExpect {
    /// 期待が指す判定結果の種類。
    pub fn kind(&self) -> JudgmentKind {
        match self {
            Self::Accepted { .. } => JudgmentKind::Accepted,
            Self::Rejected(_) => JudgmentKind::Rejected,
            Self::Clarified => JudgmentKind::Clarified,
        }
    }

    /// 期待に付く分類。問い返しは分類を持たない。
    pub fn category(&self) -> Option<Category> {
        match self {
            Self::Accepted { free_text: true } => Some(Category::FreeTextUnjudged),
            Self::Accepted { free_text: false } | Self::Clarified => None,
            Self::Rejected(c) => Some(Category::from(*c)),
        }
    }

    /// 実際の判定が期待と一致するか。`Confirmed`・`Progressed`・`Ignored` はどの期待にも一致しない。
    pub fn matches(&self, j: &Judgment) -> bool {
        match (self, j) {
            (Self::Accepted { free_text }, Judgment::Accepted(note)) => {
                *free_text == note.is_some()
            }
            (Self::Rejected(c), Judgment::Rejected(r)) => *c == r.category,
            (Self::Clarified, Judgment::Clarified(_)) => true,
            _ => false,
        }
    }
}

/// 期待の文字列を読めない理由。`Display` は入力値を含まない固定文にする。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectError {
    /// 頭が語彙の `result` の 3 語でない（空文字列・`confirmed` などを含む）。
    UnknownResult,
    /// `rejected` に分類が付いていない。
    MissingCategory,
    /// 分類が語彙の 8 語でない。
    UnknownCategory,
    /// `rejected:free_text_unjudged`（受付に付く分類で、拒否ではない）。
    NotARejection,
    /// `accepted` に `free_text_unjudged` 以外の分類、または `clarified` に分類が付いている。
    UnexpectedCategory,
}

impl fmt::Display for ExpectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnknownResult => "expect: unknown result",
            Self::MissingCategory => "expect: rejected needs a category",
            Self::UnknownCategory => "expect: unknown category",
            Self::NotARejection => "expect: free_text_unjudged is not a rejection",
            Self::UnexpectedCategory => "expect: unexpected category",
        })
    }
}

impl std::error::Error for ExpectError {}

/// `expect` の文字列を読む。最初の `:` だけで頭と尾に分け、前後の空白・大文字小文字の揺れは受けない。
///
/// `rejected` を分類なしで受けないのは、誤入力の目的が分類の確認であり、分類を省くと別の拒否でも通って
/// しまうためである（公開済みの教材テストもすべて分類を書いている）。
pub fn parse_expect(s: &str) -> Result<ResultExpect, ExpectError> {
    let (head, tail) = match s.split_once(':') {
        Some((h, t)) => (h, Some(t)),
        None => (s, None),
    };
    let kind = result_kind(head).ok_or(ExpectError::UnknownResult)?;
    match (kind, tail) {
        (JudgmentKind::Accepted, None) => Ok(ResultExpect::Accepted { free_text: false }),
        (JudgmentKind::Accepted, Some(t)) => match rejection_category(t) {
            Some(Category::FreeTextUnjudged) => Ok(ResultExpect::Accepted { free_text: true }),
            Some(_) => Err(ExpectError::UnexpectedCategory),
            None => Err(ExpectError::UnknownCategory),
        },
        (JudgmentKind::Rejected, None) => Err(ExpectError::MissingCategory),
        (JudgmentKind::Rejected, Some(t)) => {
            let category = rejection_category(t).ok_or(ExpectError::UnknownCategory)?;
            RejectCategory::try_from(category)
                .map(ResultExpect::Rejected)
                .map_err(|_| ExpectError::NotARejection)
        }
        (JudgmentKind::Clarified, None) => Ok(ResultExpect::Clarified),
        (JudgmentKind::Clarified, Some(_)) => Err(ExpectError::UnexpectedCategory),
        // result_kind は語彙の 3 語だけを返すので、残りの 3 種には到達しない。到達しても拒否に倒す。
        (JudgmentKind::Confirmed | JudgmentKind::Progressed | JudgmentKind::Ignored, _) => {
            Err(ExpectError::UnknownResult)
        }
    }
}

fn pack_diag(file: &str, item: &str, message: String) -> Diagnostic {
    let file = if file.trim().is_empty() { "." } else { file };
    // file と item を空でない値に直したので、Diagnostic::new は失敗しない。
    Diagnostic::new(file, item, DiagCode::Parse, message).expect("file and item are non-empty")
}

/// 一覧の要素の item。id は `safe_ident` を通るときだけ写し、そうでなければ 1 始まりの位置にする。
fn item_of(index: usize, v: &Value) -> String {
    match v.get("id").and_then(Value::as_str) {
        Some(id) if safe_ident(id) => format!("error_input:{id}"),
        _ => format!("error_input:#{}", index + 1),
    }
}

/// 誤入力の期待を ID と組で読む。誤りがあれば、一覧の順の診断（`E-PARSE`）を返す。
///
/// ID が `safe_ident` を通らない要素の ID は、位置を使った `#<1 始まりの位置>` の形で返す。
pub fn read_error_input_expects(
    pack: &LoadedPack,
) -> Result<Vec<(String, ResultExpect)>, Vec<Diagnostic>> {
    let (Some(file), Some(tests)) = (pack.tests_file.as_deref(), pack.tests.as_ref()) else {
        return Ok(Vec::new());
    };
    let mut found = Vec::new();
    let mut diags = Vec::new();
    for (n, v) in tests.error_inputs.iter().enumerate() {
        let item = item_of(n, v);
        let expect = v.get("input").and_then(|i| i.get("expect"));
        let message = match expect {
            None => Err("error_input: expect is missing".to_string()),
            Some(Value::String(s)) => parse_expect(s).map_err(|e| format!("error_input: {e}")),
            Some(_) => Err("error_input: expect must be a string".to_string()),
        };
        match message {
            Ok(e) => {
                let id = item
                    .strip_prefix("error_input:")
                    .unwrap_or(item.as_str())
                    .to_string();
                found.push((id, e));
            }
            Err(m) => diags.push(pack_diag(file, &item, m)),
        }
    }
    if diags.is_empty() {
        Ok(found)
    } else {
        Err(diags)
    }
}

/// 教材テストの `error_inputs[].input.expect` を検査し、読めないものを `E-PARSE` にする。
///
/// 教材テストが無いときは空を返す。`pre[].expect` と `playthroughs` は読まない（TASK-404）。
pub fn check_error_input_expects(pack: &LoadedPack) -> Vec<Diagnostic> {
    read_error_input_expects(pack).err().unwrap_or_default()
}

/// 開始点から見た拒否の文面を静的に解決する。Step の `on_reject[分類]`、無ければ `defaults[分類]` の ID で
/// `rejections` を引く。どこかで見つからなければ `None`。
///
/// 開始 ID は章の `s-1` もモジュールの `<実体>#<ID>` も `ir.starts` で引ける。実行時の差し込み（EP-7）は
/// 行わない（TASK-267・TASK-46）。
pub fn resolve_rejection<'a>(
    ir: &'a Ir,
    start_id: &str,
    category: RejectCategory,
) -> Option<(&'a str, &'a Value)> {
    let start = ir.starts.iter().find(|s| s.id == start_id)?;
    let node = ir.nodes.iter().find(|n| n.uid == start.node)?;
    let key = category.as_str();
    let text_id = node
        .step
        .on_reject
        .get(key)
        .or_else(|| ir.defaults.get(key))?;
    let (id, value) = ir.rejections.get_key_value(text_id.as_str())?;
    Some((id.as_str(), value))
}
