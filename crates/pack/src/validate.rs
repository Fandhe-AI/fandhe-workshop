//! `fandhe-pack validate` の静的検査を置くモジュール（TASK-54 1/2: 存在しない対象への参照）。
//!
//! 読み込んだパック（`LoadedPack`）だけを見る純関数で、ファイルシステムには触れない（ディスクに触れるのは
//! `source::load`）。`validate_dir` は読み込みに失敗したとき、その診断をそのまま返し、半端に読んだ
//! パックの検査は流さない（fail-closed）。`E-REF-BODY`・練習データの宣言のパス（`E-REF-FIXTURE`）は
//! 読み込みの層が検出するため、ここでは新しい論理を持たない。
//!
//! CLI の `validate` への接続・最終行・終了コードは TASK-401・TASK-403、モジュールの展開は TASK-50 の
//! 持ち分である。`E-DUP-ID`・`E-FORMAT-VERSION`・`E-STRUCTURE` は TASK-54 2/2 が [`validate`] へ足す。
//!
//! 語彙の中身（`input`・`rules`・`confirm`・`on_enter` など）は `serde_json::Value` のまま通っているため、
//! 参照を取り出す経路をここに集める。教材データは信頼しない入力として扱い、文字列でない参照は黙って
//! 飛ばさず診断にする。診断に写す ID は `safe_ident` を通ったものだけで、本文・練習データの中身は写さない。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fandhe_workshop_contract::ir::Step;
use serde_json::Value;

use crate::diag::{DiagCode, Diagnostic};
use crate::source::{self, safe_ident, LoadedPack, LoadedScope, ScopeKind};

fn diag(file: &str, item: &str, code: DiagCode, message: impl Into<String>) -> Diagnostic {
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

/// 読み込んだパックの静的検査。当面は参照の検査だけで、TASK-54 2/2 が検査を足す。
pub fn validate(pack: &LoadedPack) -> Vec<Diagnostic> {
    check_refs(pack)
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
    let shown = if safe_ident(id) { id } else { "(id not shown)" };
    out.push(diag(
        site.file,
        &site.item,
        kind.code(),
        format!("undefined {}: {shown}", kind.noun()),
    ));
}

fn get<'v>(v: &'v Value, key: &str) -> Option<&'v Value> {
    v.as_object().and_then(|m| m.get(key))
}

fn kind_is(v: &Value, expected: &str) -> bool {
    get(v, "kind").and_then(Value::as_str) == Some(expected)
}

fn elements(v: Option<&Value>) -> &[Value] {
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

fn check_step(out: &mut Vec<Diagnostic>, scope: &LoadedScope, t: &Targets<'_>, step: &Step) {
    let in_module = scope.kind == ScopeKind::Module;
    let site = Site {
        file: &scope.file,
        item: format!("step:{}", step.id),
        in_module,
    };
    let local_steps: BTreeSet<&str> = scope.src.steps.iter().map(|s| s.id.as_str()).collect();
    let step_set = if in_module {
        &local_steps
    } else {
        &t.main_steps
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

fn check_starts(out: &mut Vec<Diagnostic>, scope: &LoadedScope, t: &Targets<'_>) {
    let in_module = scope.kind == ScopeKind::Module;
    let local_steps: BTreeSet<&str> = scope.src.steps.iter().map(|s| s.id.as_str()).collect();
    let step_set = if in_module {
        &local_steps
    } else {
        &t.main_steps
    };
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
        check_starts(&mut out, scope, &t);
        for step in &scope.src.steps {
            check_step(&mut out, scope, &t, step);
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
