//! `fandhe-pack validate` の静的検査を置くモジュール（TASK-54: 参照・ID の重複・形式の版・構造）。
//!
//! 読み込んだパック（`LoadedPack`）だけを見る純関数で、ファイルシステムには触れない（ディスクに触れるのは
//! `source::load`）。`validate_dir` は読み込みに失敗したとき、その診断をそのまま返し、半端に読んだ
//! パックの検査は流さない（fail-closed）。`E-REF-BODY`・練習データの宣言のパス（`E-REF-FIXTURE`）は
//! 読み込みの層が検出するため、ここでは新しい論理を持たない。
//!
//! CLI の `validate` への接続・最終行・終了コードは TASK-401・TASK-403、モジュールの展開は TASK-50 の
//! 持ち分である。[`validate`] は、形式の版（`E-FORMAT-VERSION`）→ ID の重複（`E-DUP-ID`）→ 参照
//! （`E-REF-*`）→ 構造（`E-STRUCTURE`）→ 文面の差し込みの枠と次の行動（`E-SLOT-*`・`E-EMPTY-NEXT-ACTION`。`slot_check`）→ 到達性・行き止まり → モジュール展開（巡回・置換後の確認）→ 外観（`E-PARSE`。`appearance`、TASK-53）→ 左エリア（`E-PARSE`・`E-LEFT-*`。`left_pane`、TASK-53 2/2）の順に検査する。形式の版が不一致なら、残りを解釈せずその 1 件だけを
//! 返す（`compile` と同じ fail-closed）。
//!
//! 構造の検査は、`expand::check_use_steps`（`use` と `input`・`confirm` の同時指定）・`input::check_inputs`・
//! `confirm::check_confirms`（正答の無い `answer` は `E-STRUCTURE`）を再利用し、結果を絞らずに返す。絞ると
//! `validate` は通るが `compile` は失敗する状態になるため、`E-PARSE`・`E-NO-CONFIRM` も報告される。
//! 「章が無い」「開始点が 0 件」の判定は `compile` と同じ文面で重複して持つ（一本化は別の変更で行う）。
//! 到達性（`E-UNREACHABLE`）・行き止まり（`E-DEAD-END`）は `reach` が受け持つ（TASK-55 1/2）。
//! モジュール呼び出しの巡回（`E-MODULE-CYCLE`）と、引数の置換後に判明する確認の欠落（`E-NO-CONFIRM`）は、
//! 宛先も `confirm` の中身も置換後でないと決まらないため、`compile` と同じ `expand::Expander` を走らせて
//! その 2 コードと、展開の打ち切り（`E-LIMIT`）だけを採る（TASK-55 2/2）。展開由来の他の診断（`E-REF-MODULE`・`E-DUP-ID`・`E-PARSE` など）は、
//! 既存の検査と文面の違う二重報告になるか本タスクの範囲外のため採らない。呼ばれないモジュールは展開されず、
//! その中の巡回は `compile` と同じく報告しない。`E-PROFILE-*` は TASK-46、網羅は TASK-56 の持ち分である。
//!
//! 語彙の中身（`input`・`rules`・`confirm`・`on_enter` など）は `serde_json::Value` のまま通っているため、
//! 参照を取り出す経路をここに集める。教材データは信頼しない入力として扱い、文字列でない参照は黙って
//! 飛ばさず診断にする。診断に写す ID は `safe_ident` を通ったものだけで、本文・練習データの中身は写さない。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fandhe_workshop_contract::ir::{is_supported_ir_format, Step};
use serde_json::Value;

use fandhe_workshop_contract::limits::MAX_IR_BYTES;

use crate::compile::Budget;
use crate::diag::{DiagCode, Diagnostic};
use crate::expand::Expander;
use crate::source::{self, safe_ident, LoadedPack, LoadedScope, ScopeKind};

pub(crate) fn diag(
    file: &str,
    item: &str,
    code: DiagCode,
    message: impl Into<String>,
) -> Diagnostic {
    let file = if file.trim().is_empty() { "." } else { file };
    let item = if item.trim().is_empty() { "pack" } else { item };
    // file と item を空でない値に直したので、Diagnostic::new は失敗しない。
    Diagnostic::new(file, item, code, message).expect("file and item are non-empty")
}

/// ディレクトリを読み込んで検査する。読み込みに失敗したときはその診断を返し、参照の検査は流さない。
/// 診断が 0 件なら空の Vec。
pub fn validate_dir(dir: &Path) -> Vec<Diagnostic> {
    match source::load(dir) {
        Ok(pack) => validate(&pack),
        Err(diags) => diags,
    }
}

/// 読み込んだパックの静的検査。形式の版が不一致なら、その 1 件だけを返して残りを検査しない。
/// 診断が 0 件なら空の Vec。
pub fn validate(pack: &LoadedPack) -> Vec<Diagnostic> {
    if let Some(d) = check_format_version(pack) {
        return vec![d];
    }
    let mut out = check_dup_ids(pack);
    out.extend(check_refs(pack));
    out.extend(check_structure(pack));
    out.extend(crate::slot_check::check_texts(pack));
    out.extend(check_reachability(pack));
    out.extend(check_module_expansion(pack));
    out.extend(crate::appearance::check_appearance(
        &pack.workshop_file,
        &pack.workshop,
    ));
    let (left_pane, left_parse) =
        crate::left_pane::check_left_pane(&pack.workshop_file, &pack.workshop);
    if left_parse.is_empty() {
        out.extend(crate::left_pane::check_left_targets(pack, left_pane));
    }
    out.extend(left_parse);
    // 展開の前と後の検査が同じ Step を重ねて報告しうる `E-SLOT-NEEDS-DO` は、同じ内容のものを 1 件にする。
    let mut seen = BTreeSet::new();
    out.retain(|d| d.code() != DiagCode::SlotNeedsDo || seen.insert(format!("{d:?}")));
    out
}

/// モジュールの展開を `compile` と同じ順で走らせ、`E-MODULE-CYCLE`・`E-NO-CONFIRM` と、展開の打ち切りを示す
/// `E-LIMIT` だけを返す。`E-LIMIT` を捨てると、打ち切られた後ろの巡回・確認の欠落を検査しないまま
/// 診断なしで成功に見えるため、検査が未完了であることを示すものとして必ず残す（fail-closed）。
/// `E-NO-CONFIRM` は、`check_confirms` が飛ばす `${` を含む `confirm` の置換後の検査でだけ出るため、
/// `check_structure` の報告と重ならない。同一内容の診断は 1 件にまとめる（`compile` と同じ）。
pub fn check_module_expansion(pack: &LoadedPack) -> Vec<Diagnostic> {
    // `E-PROFILE-*` は TASK-46 の持ち分のため、設定だけ取り出して診断は捨てる。
    let (profile, _) = crate::profile::check_profile(&pack.workshop_file, &pack.workshop);
    let mut ex = Expander::new(pack, &profile, Budget::new(MAX_IR_BYTES));
    for scope in pack.scopes.iter().filter(|s| s.kind == ScopeKind::Chapter) {
        ex.expand_chapter(scope);
    }
    // 展開の後の Step と開始点で `step_do` を使う文面に `do` があるか（`E-SLOT-NEEDS-DO`）。
    let mut starts: Vec<fandhe_workshop_contract::ir::Start> = Vec::new();
    let mut start_ids: BTreeSet<String> = BTreeSet::new();
    for scope in pack.scopes.iter().filter(|s| s.kind == ScopeKind::Chapter) {
        for st in &scope.src.starts {
            if start_ids.insert(st.id.clone()) {
                let mut s = st.clone();
                s.node = format!("main/{}", st.node);
                starts.push(s);
            }
        }
    }
    let module_diags = std::mem::take(&mut ex.diags);
    starts.extend(ex.module_starts(&mut start_ids));
    ex.diags.splice(0..0, module_diags);
    let needs_do = crate::slot_check::check_expanded(pack, &ex.nodes, &starts);
    ex.diags.extend(needs_do);
    let mut seen = BTreeSet::new();
    ex.diags
        .into_iter()
        .filter(|d| {
            matches!(
                d.code(),
                DiagCode::ModuleCycle
                    | DiagCode::NoConfirm
                    | DiagCode::Limit
                    | DiagCode::SlotNeedsDo
                    | DiagCode::EmptyNextAction
            )
        })
        .filter(|d| seen.insert(format!("{d:?}")))
        .collect()
}

/// 到達性（`E-UNREACHABLE`）と行き止まり（`E-DEAD-END`）の検査。意味論は `reach` のモジュール文書を参照。
pub fn check_reachability(pack: &LoadedPack) -> Vec<Diagnostic> {
    crate::reach::check_reach(pack)
}

/// 診断の文面に写す ID。`safe_ident` を通らない ID は写さない（教材データは信頼しない入力）。
pub(crate) fn shown(id: &str) -> &str {
    if safe_ident(id) {
        id
    } else {
        "(id not shown)"
    }
}

/// IR の形式の版（`pack.ir_format`）が対応する版かを検査する（`E-FORMAT-VERSION`）。
/// `validate_dir` の経路では読み込みの層が先に止めるため、メモリ上の `LoadedPack` を直接渡す呼び出し向けの判定。
pub fn check_format_version(pack: &LoadedPack) -> Option<Diagnostic> {
    let v = pack.workshop.pack.ir_format;
    if is_supported_ir_format(v) {
        return None;
    }
    Some(diag(
        &pack.workshop_file,
        "pack:ir_format",
        DiagCode::FormatVersion,
        format!("ir_format {v} is not supported (expected 1 or 2)"),
    ))
}

/// ID の重複の検査（`E-DUP-ID`）。名前空間は `compile` に合わせる。章の Step・開始点は全章で 1 つ、
/// モジュールの Step・開始点はモジュールごと、モジュールの ID は全モジュールで 1 つ。1 件目を通し、
/// 2 件目以降を、その定義のファイルで報告する。呼ばれないモジュールも検査する。
pub fn check_dup_ids(pack: &LoadedPack) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut chapter_steps: BTreeSet<&str> = BTreeSet::new();
    let mut chapter_starts: BTreeSet<&str> = BTreeSet::new();
    let mut module_ids: BTreeSet<&str> = BTreeSet::new();
    for scope in &pack.scopes {
        let is_module = scope.kind == ScopeKind::Module;
        if is_module && !module_ids.insert(scope.src.id.as_str()) {
            out.push(diag(
                &scope.file,
                "module",
                DiagCode::DupId,
                format!("duplicate module id: {}", shown(&scope.src.id)),
            ));
        }
        let mut local_steps: BTreeSet<&str> = BTreeSet::new();
        let mut local_starts: BTreeSet<&str> = BTreeSet::new();
        let (steps, starts) = if is_module {
            (&mut local_steps, &mut local_starts)
        } else {
            (&mut chapter_steps, &mut chapter_starts)
        };
        for start in &scope.src.starts {
            if !starts.insert(start.id.as_str()) {
                out.push(diag(
                    &scope.file,
                    &format!("start:{}", start.id),
                    DiagCode::DupId,
                    format!("duplicate start id: {}", shown(&start.id)),
                ));
            }
        }
        for step in &scope.src.steps {
            if !steps.insert(step.id.as_str()) {
                out.push(diag(
                    &scope.file,
                    &format!("step:{}", step.id),
                    DiagCode::DupId,
                    format!("duplicate step id: {}", shown(&step.id)),
                ));
            }
        }
    }
    out
}

/// 構造の検査（`E-STRUCTURE`）。章・開始点の有無は `compile` と同じ判定で、`use` と `input` の同時指定・
/// 入力方式の宣言・確認の宣言（正答の無い `answer` を含む）は既存の検査を呼んで、結果を絞らずに返す。
pub fn check_structure(pack: &LoadedPack) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut chapters = 0usize;
    let mut starts = 0usize;
    for scope in pack.scopes.iter().filter(|s| s.kind == ScopeKind::Chapter) {
        chapters += 1;
        starts += scope.src.starts.len();
    }
    if chapters == 0 {
        out.push(diag(
            "chapters",
            "pack",
            DiagCode::Structure,
            "no chapter is defined",
        ));
    } else if starts == 0 {
        out.push(diag(
            "chapters",
            "pack",
            DiagCode::Structure,
            "no start is defined",
        ));
    }
    out.extend(crate::expand::check_use_steps(&pack.scopes));
    out.extend(crate::input::check_inputs(&pack.scopes));
    out.extend(crate::confirm::check_confirms(&pack.scopes));
    out
}

/// 照合先の ID の集合。
struct Targets<'a> {
    views: BTreeSet<&'a str>,
    fixtures: BTreeSet<&'a str>,
    hints: BTreeSet<&'a str>,
    rejections: BTreeSet<&'a str>,
    clarifications: BTreeSet<&'a str>,
    commands: BTreeSet<&'a str>,
    modules: BTreeSet<&'a str>,
    /// 全章の Step の ID（`compile` の `main` 名前空間に合わせ、`use` の Step も含む）。
    main_steps: BTreeSet<&'a str>,
}

/// 診断を積む場所（ファイルと項目）。
struct Site<'a> {
    file: &'a str,
    item: String,
    /// モジュールの中か。引数（`${...}`）を含む ID は展開（TASK-50）まで照合できないため飛ばす。
    in_module: bool,
}

/// 参照の種類。コードと文面の名詞を決める。
#[derive(Clone, Copy)]
enum Kind {
    Step,
    View,
    Fixture,
    Hint,
    Rejection,
    /// 問い返しの欠けの専用コードは無いため、`E-REF-REJECTION` で報告する（固定した一覧に従う）。
    Clarification,
    Command,
    Module,
}

impl Kind {
    fn code(self) -> DiagCode {
        match self {
            Kind::Step => DiagCode::RefStep,
            Kind::View => DiagCode::RefView,
            Kind::Fixture => DiagCode::RefFixture,
            Kind::Hint => DiagCode::RefHint,
            Kind::Rejection | Kind::Clarification => DiagCode::RefRejection,
            Kind::Command => DiagCode::RefCommand,
            Kind::Module => DiagCode::RefModule,
        }
    }

    fn noun(self) -> &'static str {
        match self {
            Kind::Step => "step",
            Kind::View => "view",
            Kind::Fixture => "fixture",
            Kind::Hint => "hint",
            Kind::Rejection => "rejection",
            Kind::Clarification => "clarification",
            Kind::Command => "command",
            Kind::Module => "module",
        }
    }
}

/// 1 件の参照を照合する。`null` と欄の省略は参照なし。文字列でない値は fail-closed で診断にする。
fn check_value(
    out: &mut Vec<Diagnostic>,
    site: &Site<'_>,
    kind: Kind,
    value: Option<&Value>,
    set: &BTreeSet<&str>,
) {
    match value {
        None | Some(Value::Null) => {}
        Some(Value::String(id)) => check_id(out, site, kind, id, set),
        Some(_) => out.push(diag(
            site.file,
            &site.item,
            kind.code(),
            format!("{} reference must be a string ID", kind.noun()),
        )),
    }
}

fn check_id(
    out: &mut Vec<Diagnostic>,
    site: &Site<'_>,
    kind: Kind,
    id: &str,
    set: &BTreeSet<&str>,
) {
    if set.contains(id) || (site.in_module && id.contains("${")) {
        return;
    }
    out.push(diag(
        site.file,
        &site.item,
        kind.code(),
        format!("undefined {}: {}", kind.noun(), shown(id)),
    ));
}

pub(crate) fn get<'v>(v: &'v Value, key: &str) -> Option<&'v Value> {
    v.as_object().and_then(|m| m.get(key))
}

pub(crate) fn kind_is(v: &Value, expected: &str) -> bool {
    get(v, "kind").and_then(Value::as_str) == Some(expected)
}

pub(crate) fn elements(v: Option<&Value>) -> &[Value] {
    match v {
        Some(Value::Array(a)) => a,
        _ => &[],
    }
}

fn keys_of<V>(m: &BTreeMap<String, V>) -> BTreeSet<&str> {
    m.keys().map(String::as_str).collect()
}

/// 効果の列から `put.fixture`・`replace.fixture` を照合する。
fn check_effects(out: &mut Vec<Diagnostic>, site: &Site<'_>, t: &Targets<'_>, effects: &[Value]) {
    for e in effects {
        for op in ["put", "replace"] {
            if let Some(body) = get(e, op) {
                check_value(out, site, Kind::Fixture, get(body, "fixture"), &t.fixtures);
            }
        }
    }
}

/// `ui_action`・`point_at` の対象の `view` を照合する。
fn check_targets(out: &mut Vec<Diagnostic>, site: &Site<'_>, t: &Targets<'_>, v: &Value) {
    for target in elements(get(v, "targets")) {
        check_value(out, site, Kind::View, get(target, "view"), &t.views);
    }
}

/// `step_set` は、スコープの Step の参照先の集合（章は全章、モジュールは自分の中）。スコープごとに 1 回だけ組む。
fn check_step(
    out: &mut Vec<Diagnostic>,
    scope: &LoadedScope,
    t: &Targets<'_>,
    step_set: &BTreeSet<&str>,
    step: &Step,
) {
    let in_module = scope.kind == ScopeKind::Module;
    let site = Site {
        file: &scope.file,
        item: format!("step:{}", step.id),
        in_module,
    };

    for next in &step.next {
        check_id(out, &site, Kind::Step, &next.to, step_set);
    }
    for v in &step.views {
        check_id(out, &site, Kind::View, v, &t.views);
    }
    for f in &step.fixtures {
        check_id(out, &site, Kind::Fixture, f, &t.fixtures);
    }
    for h in &step.hints {
        check_id(out, &site, Kind::Hint, h, &t.hints);
    }
    for r in step.on_reject.values() {
        check_id(out, &site, Kind::Rejection, r, &t.rejections);
    }

    for list in [&step.on_enter, &step.on_achieved] {
        check_effects(out, &site, t, list);
    }
    for p in &step.processing {
        check_effects(out, &site, t, elements(get(p, "effects")));
    }

    if let Some(input) = &step.input {
        if kind_is(input, "ui_action") {
            check_targets(out, &site, t, input);
        } else if kind_is(input, "command") {
            check_value(out, &site, Kind::Command, get(input, "name"), &t.commands);
        }
    }

    for rule in &step.rules {
        let on = get(rule, "on");
        if let Some(ui) = on.and_then(|on| get(on, "ui")) {
            check_value(out, &site, Kind::View, get(ui, "view"), &t.views);
        }
        check_value(
            out,
            &site,
            Kind::Command,
            on.and_then(|on| get(on, "command")),
            &t.commands,
        );
        let Some(outcome) = get(rule, "outcome") else {
            continue;
        };
        check_effects(out, &site, t, elements(get(outcome, "effects")));
        if kind_is(outcome, "reject") {
            check_value(
                out,
                &site,
                Kind::Rejection,
                get(outcome, "rejection"),
                &t.rejections,
            );
        } else if kind_is(outcome, "clarify") {
            check_value(
                out,
                &site,
                Kind::Clarification,
                get(outcome, "clarification"),
                &t.clarifications,
            );
        }
    }

    if let Some(confirm) = &step.confirm {
        if kind_is(confirm, "point_at") {
            check_targets(out, &site, t, confirm);
        }
        for key in ["on_wrong", "on_miss"] {
            check_value(
                out,
                &site,
                Kind::Rejection,
                get(confirm, key),
                &t.rejections,
            );
        }
        for option in elements(get(confirm, "options")) {
            check_value(
                out,
                &site,
                Kind::Rejection,
                get(option, "on_wrong"),
                &t.rejections,
            );
        }
    }

    if let Some(use_) = &step.use_module {
        check_value(out, &site, Kind::Module, get(use_, "module"), &t.modules);
    }
}

/// `step_set` は `check_step` と同じ、スコープの Step の参照先の集合。
fn check_starts(
    out: &mut Vec<Diagnostic>,
    scope: &LoadedScope,
    t: &Targets<'_>,
    step_set: &BTreeSet<&str>,
) {
    let in_module = scope.kind == ScopeKind::Module;
    for start in &scope.src.starts {
        let site = Site {
            file: &scope.file,
            item: format!("start:{}", start.id),
            in_module,
        };
        check_id(out, &site, Kind::Step, &start.node, step_set);
        check_effects(out, &site, t, &start.effects);
        check_value(
            out,
            &site,
            Kind::Rejection,
            start.unmet.as_ref(),
            &t.rejections,
        );
    }
}

/// 存在しない対象への参照の検査（`E-REF-STEP`・`E-REF-VIEW`・`E-REF-FIXTURE`・`E-REF-HINT`・
/// `E-REF-REJECTION`・`E-REF-MODULE`・`E-REF-COMMAND`）。走査の順は決定的（スコープの順、ファイルの中の順）。
pub fn check_refs(pack: &LoadedPack) -> Vec<Diagnostic> {
    let w = &pack.workshop;
    let t = Targets {
        views: keys_of(&w.views),
        fixtures: keys_of(&w.fixtures),
        hints: keys_of(&w.hints),
        rejections: keys_of(&w.rejections),
        clarifications: keys_of(&w.clarifications),
        commands: keys_of(&w.commands),
        modules: pack
            .scopes
            .iter()
            .filter(|s| s.kind == ScopeKind::Module)
            .map(|s| s.src.id.as_str())
            .collect(),
        main_steps: pack
            .scopes
            .iter()
            .filter(|s| s.kind == ScopeKind::Chapter)
            .flat_map(|s| s.src.steps.iter().map(|st| st.id.as_str()))
            .collect(),
    };

    let mut out = Vec::new();
    for scope in &pack.scopes {
        // 章は全章の集合 `main_steps` を共有し、モジュールだけ自分の Step の集合をここで 1 回組む。
        let local_steps: BTreeSet<&str>;
        let step_set = if scope.kind == ScopeKind::Module {
            local_steps = scope.src.steps.iter().map(|s| s.id.as_str()).collect();
            &local_steps
        } else {
            &t.main_steps
        };
        check_starts(&mut out, scope, &t, step_set);
        for step in &scope.src.steps {
            check_step(&mut out, scope, &t, step_set, step);
        }
    }
    for (class, id) in &w.defaults {
        let item = if safe_ident(class) {
            format!("defaults:{class}")
        } else {
            "defaults".to_string()
        };
        let site = Site {
            file: &pack.workshop_file,
            item,
            in_module: false,
        };
        check_id(&mut out, &site, Kind::Rejection, id, &t.rejections);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn set<'a>(ids: &[&'a str]) -> BTreeSet<&'a str> {
        ids.iter().copied().collect()
    }

    fn site() -> Site<'static> {
        Site {
            file: "chapters/c1/chapter.yaml",
            item: "step:a".to_string(),
            in_module: false,
        }
    }

    fn run(kind: Kind, v: Option<Value>, ids: &[&str]) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        check_value(&mut out, &site(), kind, v.as_ref(), &set(ids));
        out
    }

    #[test]
    fn missing_and_null_are_no_reference() {
        assert!(run(Kind::View, None, &[]).is_empty());
        assert!(run(Kind::View, Some(Value::Null), &[]).is_empty());
    }

    #[test]
    fn non_string_reference_is_reported_not_skipped() {
        for v in [json!(1), json!({"a": 1}), json!(["x"])] {
            let d = run(Kind::View, Some(v), &["x"]);
            assert_eq!(d.len(), 1);
            assert_eq!(d[0].code(), DiagCode::RefView);
            assert!(d[0].message().contains("string ID"));
        }
    }

    #[test]
    fn unsafe_id_is_not_echoed() {
        let d = run(Kind::Hint, Some(json!("bad\nid with space")), &[]);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].message(), "undefined hint: (id not shown)");
        let d = run(Kind::Hint, Some(json!("h-x")), &[]);
        assert_eq!(d[0].message(), "undefined hint: h-x");
    }

    #[test]
    fn unsafe_duplicate_id_is_not_shown() {
        assert_eq!(shown("a b\nc"), "(id not shown)");
        assert_eq!(shown("a-b"), "a-b");
    }

    #[test]
    fn unsupported_format_version_returns_only_that_diagnostic() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/min-3step-pack-b");
        let mut pack = source::load(&dir).expect("load");
        assert!(validate(&pack).is_empty());
        pack.workshop.pack.ir_format = 3;
        pack.workshop.hints.clear();
        let d = validate(&pack);
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].code(), DiagCode::FormatVersion);
        assert_eq!(d[0].item(), "pack:ir_format");
    }

    #[test]
    fn known_id_passes() {
        assert!(run(Kind::Hint, Some(json!("h")), &["h"]).is_empty());
    }

    #[test]
    fn effects_check_put_and_replace_fixture() {
        let t = Targets {
            views: set(&[]),
            fixtures: set(&["fx"]),
            hints: set(&[]),
            rejections: set(&[]),
            clarifications: set(&[]),
            commands: set(&[]),
            modules: set(&[]),
            main_steps: set(&[]),
        };
        let effects = vec![
            json!({"put": {"path": "a", "fixture": "fx"}}),
            json!({"put": {"path": "a"}}),
            json!({"replace": {"path": "a", "fixture": "nope"}}),
        ];
        let mut out = Vec::new();
        check_effects(&mut out, &site(), &t, &effects);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].code(), DiagCode::RefFixture);
    }

    #[test]
    fn module_params_are_skipped_only_in_modules() {
        let mut s = site();
        let mut out = Vec::new();
        check_id(&mut out, &s, Kind::Fixture, "fx-${a}", &set(&[]));
        assert_eq!(out.len(), 1);
        s.in_module = true;
        let mut out = Vec::new();
        check_id(&mut out, &s, Kind::Fixture, "fx-${a}", &set(&[]));
        assert!(out.is_empty());
    }
}
