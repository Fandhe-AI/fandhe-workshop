//! 拒否・問い返しの文面の差し込みの枠と「次の行動」の検査（TASK-46 2/2、PACK-12、PACK-16、R-21）。
//!
//! 静的な検査 [`check_texts`] は `validate`・`compile` から呼ばれ、`workshop.*` の `rejections`・`clarifications` の
//! 各文面を読む。枠の構文・名前・分類の制限・代替の文は contract の [`fandhe_workshop_contract::slot`] が持つ表と
//! 読み取りで判定し（実行時に差し込む core〔TASK-267〕と同じ規則を共有する）、ここでは診断への写しと、IR を
//! 総なめにする数え上げだけを持つ。IR の形は変えない（文面は枠を含んだまま残し、差し込みは実行時に行う）。
//!
//! 診断
//! - `E-SLOT-SYNTAX`・`E-SLOT-UNKNOWN`・`E-SLOT-CONTEXT`・`E-SLOT-NO-FALLBACK`: 文面の欄（`message`・次の行動の欄）の枠。
//!   `workshop.*` の `rejection:<id>`・`clarification:<id>` に付ける
//! - `E-SLOT-NEEDS-DO`: `step_do` を使う文面が出うる Step に `do` が無い。Step を定義した章・モジュールの `step:<id>` に付ける
//! - `E-EMPTY-NEXT-ACTION`: 次の行動の欄（拒否は `next_action`、問い返しは `how_to_answer`）が空、または最悪の場合の
//!   値を差し込むと空になる
//! - 欄の欠け・型の違いは `E-PARSE`（黙って飛ばさない）。分類を読めない拒否は `rejection::check_categories` が `E-PARSE`
//!   を出すので、ここでは分類の制限を判定しない
//!
//! 動的な数え上げ [`tally_empty_next_actions`] は、IR の各 node で出うる文面を、最悪の場合の値（空になりうる枠は
//! 値なし）で差し込み、次の行動が空になる件数を数える。`fandhe-pack test` の結線（TASK-404）が呼ぶ前提の
//! 公開 API で、実行時の値の組み立て（入力の先頭の語・近い候補の編集距離・処理中の件数）は TASK-267 の持ち分である。
//! 診断の文面は英語の固定文で、本文・代替の文の値は写さない。ID・枠の名前は `safe_ident` を通ったものだけを写す。

use std::collections::{BTreeMap, BTreeSet};

use fandhe_workshop_contract::ir::{Ir, Node, Start, Step};
use fandhe_workshop_contract::judgment::Category;
use fandhe_workshop_contract::slot::{
    parse_slots, substitute, SlotName, SlotValues, SubstituteError,
};
use serde_json::Value;

use crate::diag::{DiagCode, Diagnostic};
use crate::input::diag as step_diag;
use crate::source::{safe_ident, LoadedPack, LoadedScope};
use crate::validate::diag;
use crate::vocab::rejection_category;

/// 文面の種類。拒否は `rejections`、問い返しは `clarifications` の表にある。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TextKind {
    /// 拒否の文面（次の行動の欄は `next_action`）。
    Rejection,
    /// 問い返しの文面（次の行動の欄は `how_to_answer`）。
    Clarification,
}

impl TextKind {
    fn noun(self) -> &'static str {
        match self {
            Self::Rejection => "rejection",
            Self::Clarification => "clarification",
        }
    }

    /// 次の行動を書く欄の名前。
    fn next_field(self) -> &'static str {
        match self {
            Self::Rejection => "next_action",
            Self::Clarification => "how_to_answer",
        }
    }
}

/// 文面への参照。種類と ID の組。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextRef {
    /// 拒否か問い返しか。
    pub kind: TextKind,
    /// 表の中の ID。
    pub id: String,
}

fn item_of(kind: TextKind, id: &str) -> String {
    if safe_ident(id) {
        format!("{}:{id}", kind.noun())
    } else {
        kind.noun().to_string()
    }
}

/// 静的検査で差し込む仮の値。空になりうる枠は値なし、そのほかは空でない値にする（最悪の場合）。
fn worst_case(title: &str, step_do: Option<&str>, counted: bool) -> SlotValues {
    SlotValues {
        input_head: None,
        nearest_command: None,
        step_title: title.to_string(),
        step_do: step_do.map(str::to_string),
        remaining: counted.then_some(1),
        total: counted.then_some(1),
    }
}

/// 文面 1 件の、枠の検査と次の行動の空の検査。診断は `workshop.*` の `<種類>:<id>` に付ける。
fn check_one_text(out: &mut Vec<Diagnostic>, file: &str, kind: TextKind, id: &str, value: &Value) {
    let item = item_of(kind, id);
    let noun = kind.noun();
    let Some(map) = value.as_object() else {
        // 拒否の形の誤りは `rejection::check_categories` が `E-PARSE` にする。問い返しはここで報告する。
        if kind == TextKind::Clarification {
            out.push(diag(
                file,
                &item,
                DiagCode::Parse,
                "clarification: must be a mapping",
            ));
        }
        return;
    };
    // 分類を読めない拒否は check_categories が E-PARSE にするので、分類の制限は判定しない。
    let category = match kind {
        TextKind::Rejection => map
            .get("category")
            .and_then(Value::as_str)
            .and_then(rejection_category)
            .map(Some),
        TextKind::Clarification => Some(None),
    };
    for field in ["message", kind.next_field()] {
        let is_next = field != "message";
        let text = match map.get(field) {
            None => {
                out.push(diag(
                    file,
                    &item,
                    DiagCode::Parse,
                    format!("{noun}: {field} is missing"),
                ));
                continue;
            }
            Some(Value::String(s)) => s.as_str(),
            Some(_) => {
                out.push(diag(
                    file,
                    &item,
                    DiagCode::Parse,
                    format!("{noun}: {field} must be a string"),
                ));
                continue;
            }
        };
        let before = out.len();
        check_slots(out, file, &item, field, text, category);
        // 次の行動は、最悪の場合の値を差し込んだ後でも空でないこと（欄そのものが空・空白だけの場合を含む）。
        // 枠の誤りがある文面には流さない（二重に報告しない）。
        if is_next && out.len() == before {
            let rendered = substitute(text, &worst_case("t", Some("d"), true));
            if matches!(&rendered, Ok(s) if s.trim().is_empty()) {
                out.push(diag(
                    file,
                    &item,
                    DiagCode::EmptyNextAction,
                    format!("{noun}: {field} is empty"),
                ));
            }
        }
    }
}

/// 1 つの欄の枠を検査する。`category` は拒否の分類（`None` は読めない、`Some(None)` は問い返しで分類なし）。
fn check_slots(
    out: &mut Vec<Diagnostic>,
    file: &str,
    item: &str,
    field: &str,
    text: &str,
    category: Option<Option<Category>>,
) {
    let slots = match parse_slots(text) {
        Ok(s) => s,
        Err(e) => {
            out.push(diag(
                file,
                item,
                DiagCode::SlotSyntax,
                format!("{field}: {e}"),
            ));
            return;
        }
    };
    for slot in slots {
        let Some(name) = SlotName::from_str_opt(slot.name) else {
            let msg = if safe_ident(slot.name) {
                format!("{field}: unknown slot {}", slot.name)
            } else {
                format!("{field}: unknown slot")
            };
            out.push(diag(file, item, DiagCode::SlotUnknown, msg));
            continue;
        };
        if let (Some(only), Some(actual)) = (name.only_category(), category) {
            if actual != Some(only) {
                out.push(diag(
                    file,
                    item,
                    DiagCode::SlotContext,
                    format!(
                        "{field}: slot {} is not allowed in this kind of text",
                        name.as_str()
                    ),
                ));
            }
        }
        if name.may_be_empty() && slot.fallback.is_none_or(|f| f.trim().is_empty()) {
            out.push(diag(
                file,
                item,
                DiagCode::SlotNoFallback,
                format!("{field}: slot {} needs a fallback text", name.as_str()),
            ));
        }
    }
}

/// 文面が（構文が正しいうえで）`step_do` を使っているか。
fn uses_step_do(value: &Value, kind: TextKind) -> bool {
    ["message", kind.next_field()].iter().any(|f| {
        value
            .get(*f)
            .and_then(Value::as_str)
            .and_then(|t| parse_slots(t).ok())
            .is_some_and(|slots| slots.iter().any(|s| s.name == SlotName::StepDo.as_str()))
    })
}

/// `do` が実質的にあるか。空・空白だけは無いものとして扱う（モジュールの `${...}` を含む値は空白だけにならない）。
fn has_do(step: &Step) -> bool {
    step.do_.as_deref().is_some_and(|d| !d.trim().is_empty())
}

/// Step で出うる文面を、決定的な順で重複なく返す。静的検査と動的な数え上げの両方が使う。
///
/// - `on_reject` の値
/// - 入力を持つ Step では、`on_reject` に無い分類の `defaults` の値
/// - `rules[].outcome` の `reject.rejection`・`clarify.clarification`
/// - `confirm` の `on_wrong`・`on_miss`・`options[].on_wrong`
pub(crate) fn texts_at_step(step: &Step, defaults: &BTreeMap<String, String>) -> Vec<TextRef> {
    let mut found: Vec<TextRef> = Vec::new();
    let mut add = |kind: TextKind, id: Option<&str>| {
        if let Some(id) = id {
            let t = TextRef {
                kind,
                id: id.to_string(),
            };
            if !found.contains(&t) {
                found.push(t);
            }
        }
    };
    for id in step.on_reject.values() {
        add(TextKind::Rejection, Some(id));
    }
    if step.input.is_some() {
        for (category, id) in defaults {
            if !step.on_reject.contains_key(category) {
                add(TextKind::Rejection, Some(id));
            }
        }
    }
    for rule in &step.rules {
        let Some(outcome) = rule.get("outcome") else {
            continue;
        };
        match outcome.get("kind").and_then(Value::as_str) {
            Some("reject") => add(
                TextKind::Rejection,
                outcome.get("rejection").and_then(Value::as_str),
            ),
            Some("clarify") => add(
                TextKind::Clarification,
                outcome.get("clarification").and_then(Value::as_str),
            ),
            _ => {}
        }
    }
    if let Some(confirm) = &step.confirm {
        for key in ["on_wrong", "on_miss"] {
            add(
                TextKind::Rejection,
                confirm.get(key).and_then(Value::as_str),
            );
        }
        if let Some(options) = confirm.get("options").and_then(Value::as_array) {
            for option in options {
                add(
                    TextKind::Rejection,
                    option.get("on_wrong").and_then(Value::as_str),
                );
            }
        }
    }
    found
}

/// `step_do` を使う文面（構文が正しいもの）の集合。
fn step_do_users(pack: &LoadedPack) -> BTreeSet<TextRef> {
    let w = &pack.workshop;
    let mut users: BTreeSet<TextRef> = BTreeSet::new();
    for (kind, table) in [
        (TextKind::Rejection, &w.rejections),
        (TextKind::Clarification, &w.clarifications),
    ] {
        for (id, value) in table {
            if uses_step_do(value, kind) {
                users.insert(TextRef {
                    kind,
                    id: id.clone(),
                });
            }
        }
    }
    users
}

/// `E-SLOT-NEEDS-DO`: `step_do` を使う文面が出うるのに `do` の無い Step を、定義したファイルで報告する。
/// 展開の前の Step を見る。モジュールの引数を置換した後の Step は [`check_expanded`] が見る。
fn check_needs_do(pack: &LoadedPack) -> Vec<Diagnostic> {
    let w = &pack.workshop;
    let users = step_do_users(pack);
    let mut out = Vec::new();
    if users.is_empty() {
        return out;
    }
    for scope in &pack.scopes {
        for step in &scope.src.steps {
            if step.use_module.is_some() || has_do(step) {
                continue;
            }
            // 未定義の ID は `users` に無いので自然に飛ぶ（`E-REF-*` が別に報告する）。
            if texts_at_step(step, &w.defaults)
                .iter()
                .any(|t| users.contains(t))
            {
                out.push(needs_do_diag(scope, &step.id));
            }
        }
    }
    out
}

/// 展開の後の node と開始点から、出うる（node, 文面）の組を決定的な順で重複なく返す。
/// node ごとの [`texts_at_step`] に加え、開始点（章と展開後のモジュール）の `unmet` の文面を、開始先の node との組にする。
fn text_sites<'a>(
    nodes: &'a [Node],
    starts: &[Start],
    defaults: &BTreeMap<String, String>,
) -> Vec<(&'a Node, TextRef)> {
    let mut out: Vec<(&Node, TextRef)> = Vec::new();
    for node in nodes {
        for text in texts_at_step(&node.step, defaults) {
            out.push((node, text));
        }
    }
    for start in starts {
        let Some(id) = start.unmet.as_ref().and_then(Value::as_str) else {
            continue;
        };
        let Some(node) = nodes.iter().find(|n| n.uid == start.node) else {
            continue;
        };
        let text = TextRef {
            kind: TextKind::Rejection,
            id: id.to_string(),
        };
        if !out.iter().any(|(n, t)| n.uid == node.uid && *t == text) {
            out.push((node, text));
        }
    }
    out
}

/// 展開の後（モジュールの引数の置換の後）の node と開始点を見る `E-SLOT-NEEDS-DO`。
/// 引数で `step_do` を使う文面の ID を渡した場合や、`do` に空の引数が入った場合を捕まえる。
/// 診断は、Step を定義した章・モジュールの定義ファイルに付ける。
pub(crate) fn check_expanded(
    pack: &LoadedPack,
    nodes: &[Node],
    starts: &[Start],
) -> Vec<Diagnostic> {
    let users = step_do_users(pack);
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for (node, text) in text_sites(nodes, starts, &pack.workshop.defaults) {
        check_blank_title_next_action(pack, node, &text, &mut seen, &mut out);
        if !users.contains(&text) || has_do(&node.step) {
            continue;
        }
        let d = match pack.scopes.iter().find(|s| s.src.id == node.unit) {
            Some(scope) => needs_do_diag(scope, &node.step.id),
            None => diag(
                pack.workshop_file.as_str(),
                &format!("step:{}", node.step.id),
                DiagCode::SlotNeedsDo,
                "step_do is used by a text of this step but the step has no do",
            ),
        };
        if seen.insert(format!("{d:?}")) {
            out.push(d);
        }
    }
    out
}

/// 展開の後の Step の題名が空・空白だけのとき、`step_title` だけの次の行動が空になる組を `E-EMPTY-NEXT-ACTION` にする。
/// 文面の静的検査は仮の題名で差し込むため、題名が空の Step との組合せはここで見る。`do` は仮の値（`do` の欠落は
/// `E-SLOT-NEEDS-DO` が報告する）。仮の題名では空にならない組だけを扱い、`check_one_text` の診断と二重にしない。
fn check_blank_title_next_action(
    pack: &LoadedPack,
    node: &Node,
    text: &TextRef,
    seen: &mut BTreeSet<String>,
    out: &mut Vec<Diagnostic>,
) {
    if !node.step.title.trim().is_empty() {
        return;
    }
    let table = match text.kind {
        TextKind::Rejection => &pack.workshop.rejections,
        TextKind::Clarification => &pack.workshop.clarifications,
    };
    let Some(map) = table.get(&text.id).and_then(Value::as_object) else {
        return;
    };
    let Some(next) = map.get(text.kind.next_field()).and_then(Value::as_str) else {
        return;
    };
    let counted = text.kind == TextKind::Rejection
        && map.get("category").and_then(Value::as_str) == Some(Category::Busy.as_str());
    let is_empty = |title: &str| {
        matches!(
            substitute(next, &worst_case(title, Some("d"), counted)),
            Ok(s) if s.trim().is_empty()
        )
    };
    if is_empty("t") || !is_empty(&node.step.title) {
        return;
    }
    let d = match pack.scopes.iter().find(|s| s.src.id == node.unit) {
        Some(scope) => step_diag(
            scope,
            &node.step.id,
            DiagCode::EmptyNextAction,
            format!(
                "{}: {} is empty when step_title is blank",
                text.kind.noun(),
                text.kind.next_field()
            ),
        ),
        None => diag(
            pack.workshop_file.as_str(),
            &format!("step:{}", node.step.id),
            DiagCode::EmptyNextAction,
            "next action is empty when step_title is blank",
        ),
    };
    if seen.insert(format!("{d:?}")) {
        out.push(d);
    }
}

fn needs_do_diag(scope: &LoadedScope, step_id: &str) -> Diagnostic {
    step_diag(
        scope,
        step_id,
        DiagCode::SlotNeedsDo,
        "step_do is used by a text of this step but the step has no do".to_string(),
    )
}

/// 拒否・問い返しの文面の枠と次の行動を静的に検査する。順序は決定的（拒否の ID 順、問い返しの ID 順、Scope 順・Step 順）。
pub(crate) fn check_texts(pack: &LoadedPack) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let file = pack.workshop_file.as_str();
    for (id, value) in &pack.workshop.rejections {
        check_one_text(&mut out, file, TextKind::Rejection, id, value);
    }
    for (id, value) in &pack.workshop.clarifications {
        check_one_text(&mut out, file, TextKind::Clarification, id, value);
    }
    out.extend(check_needs_do(pack));
    out
}

/// IR から文面を引き、次の行動を差し込む。`None` は、文面が IR に無い・次の行動の欄が無い・差し込めない場合。
fn render(ir: &Ir, text: &TextRef, values: &SlotValues) -> Option<Result<String, SubstituteError>> {
    let table = match text.kind {
        TextKind::Rejection => &ir.rejections,
        TextKind::Clarification => &ir.clarifications,
    };
    let field = table.get(&text.id)?.get(text.kind.next_field())?.as_str()?;
    Some(substitute(field, values))
}

/// 文面の次の行動を、与えられた値で差し込んだ結果。文面が無い・次の行動の欄が無い・差し込めないときは `None`。
pub fn rendered_next_action(ir: &Ir, text: &TextRef, values: &SlotValues) -> Option<String> {
    render(ir, text, values)?.ok()
}

/// 差し込みの後の次の行動を数えた結果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NextActionTally {
    /// 数えた（node, 文面）の組の件数。
    pub checked: usize,
    /// 次の行動が空になった件数（`findings` の件数）。
    pub empty: usize,
    /// 空になった（node の `uid`, 文面）の組。順序は node の順、文面の出現順。
    pub findings: Vec<(String, TextRef)>,
}

/// IR の各 node で出うる文面を、最悪の場合の値で差し込み、次の行動が空になる組を数える。
///
/// 最悪の場合の値: `input_head`・`nearest_command` は値なし、`step_title`・`step_do` は node の値
/// （`do` が無ければ値なし）、`remaining`・`total` は拒否の分類が `busy` のときだけ値あり。モジュールの引数の
/// 展開の後で `do` が空になる場合も、展開後の node を見るのでここで捕まる。開始点（`starts[].unmet`）の文面は、
/// 開始先の node との組で数える。文面が IR に無い・次の行動の欄が無い・文字列でない・構文の誤りや未知の名前で
/// 差し込めない組も、飛ばさず空として数える（fail-closed）。
pub fn tally_empty_next_actions(ir: &Ir) -> NextActionTally {
    let mut tally = NextActionTally::default();
    for (node, text) in text_sites(&ir.nodes, &ir.starts, &ir.defaults) {
        let counted = text.kind == TextKind::Rejection
            && ir
                .rejections
                .get(&text.id)
                .and_then(|v| v.get("category"))
                .and_then(Value::as_str)
                == Some(Category::Busy.as_str());
        let values = worst_case(&node.step.title, node.step.do_.as_deref(), counted);
        // 描画できない組（文面が IR に無い・次の行動の欄が無い・文字列でない・差し込めない）も空として数える。
        let result = render(ir, &text, &values);
        tally.checked += 1;
        if !matches!(&result, Some(Ok(s)) if !s.trim().is_empty()) {
            tally.findings.push((node.uid.clone(), text));
        }
    }
    tally.empty = tally.findings.len();
    tally
}

/// [`tally_empty_next_actions`] の結果を `E-EMPTY-NEXT-ACTION` にする。`file` は `workshop.*` のパス。
/// 同じ文面は 1 件にまとめる。`fandhe-pack test` の結線（TASK-404）が使う。
pub fn empty_next_action_diagnostics(ir: &Ir, file: &str) -> Vec<Diagnostic> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (_, text) in tally_empty_next_actions(ir).findings {
        if seen.insert(text.clone()) {
            out.push(diag(
                file,
                &item_of(text.kind, &text.id),
                DiagCode::EmptyNextAction,
                format!(
                    "{}: {} is empty or cannot be rendered",
                    text.kind.noun(),
                    text.kind.next_field()
                ),
            ));
        }
    }
    out
}
