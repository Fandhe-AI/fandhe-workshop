//! 条件の語彙（TASK-42 1/2、PACK-8）の読み込み時の検査の結合試験。
//!
//! 受理（`next[].when`・`rules[].when`・`starts[].requires`・効果の `when`）、`E-PARSE`・`E-STRUCTURE`、
//! 式やスクリプトの欄の拒否、診断に値を写さないことを確かめる。試験名の `pack8_` で 04 の ID を追える。

mod common;

use common::{assert_no_secret, Tmp, SECRET, WORKSHOP};

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

/// Step `a` の `next`・`rules`・追加欄と、開始点の `requires` を差し込んだパックを作る。
fn pack(next_when: &str, rule_when: &str, step_extra: &str, requires: &str) -> Tmp {
    // `requires: null` は省略と同じ扱いで、`unmet` だけを残すと片方だけの `E-STRUCTURE` になる（TASK-51）。
    let unmet = if requires == "null" {
        ""
    } else {
        ", unmet: rj-x"
    };
    let t = Tmp::new("condition");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        format!(
            "id: c1\ntitle: C\nstarts: [{{id: s1, node: a, requires: {requires}{unmet}}}]\nsteps:\n  - {{id: a, title: A, next: [{{to: b, when: {next_when}}}], rules: [{{id: r1, when: {rule_when}}}]{step_extra}}}\n  - {{id: b, title: B, final: true}}\n"
        )
        .as_bytes(),
    );
    t
}

fn diags(t: &Tmp) -> Vec<Diagnostic> {
    match compile_dir(&t.0) {
        Ok(_) => panic!("expected diagnostics"),
        Err(d) => d,
    }
}

fn first(t: &Tmp, code: DiagCode) -> Diagnostic {
    let mut d = diags(t);
    let hit = d
        .iter()
        .position(|x| x.code() == code)
        .unwrap_or_else(|| panic!("no {code:?} in {d:?}"));
    assert_no_secret(&d);
    d.remove(hit)
}

/// 11 語の例。JSON は YAML の流れ形としてそのまま読めるため、同じ文字列を期待値の解釈にも使う。
const ALL_WORDS: &[&str] = &[
    r#"{"step": {"step": "b", "state": "achieved"}}"#,
    r#"{"flag": "f"}"#,
    r#"{"world_has": "panel"}"#,
    r#"{"world_attr": {"path": "panel", "attr": "k", "eq": "v"}}"#,
    r#"{"count": {"prefix": "x/", "where": {"k": "v"}, "op": ">=", "n": 2}}"#,
    r#"{"chose": {"step": "a", "option": "o"}}"#,
    r#"{"all": [{"flag": "f"}, {"not": {"flag": "g"}}]}"#,
    r#"{"any": [{"flag": "f"}, {"world_has": "p"}]}"#,
    r#"{"not": {"flag": "f"}}"#,
    r#"{"param": {"name": "n", "eq": "v"}}"#,
    r#"{"clarifying": "cl-1"}"#,
];

#[test]
fn pack8_all_words_compile_everywhere_and_stay_values_in_the_ir() {
    for w in ALL_WORDS {
        let extra = format!(", on_enter: [{{put: {{path: p}}, when: {w}}}], processing: [{{name: n, effects: [{{set_flag: f, when: {w}}}]}}]");
        let t = pack(w, w, &extra, w);
        let bytes = compile_dir(&t.0).unwrap_or_else(|d| panic!("{w}: {d:?}"));
        let ir: Ir = serde_json::from_slice(&bytes).expect("ir");
        let a = ir.nodes.iter().find(|n| n.uid == "main/a").expect("a");
        let want: serde_json::Value = serde_json::from_str(w).expect("json");
        assert_eq!(a.step.next[0].when.as_ref(), Some(&want), "{w}");
        assert_eq!(a.step.rules[0]["when"], want, "{w}");
        assert_eq!(ir.starts[0].requires.as_ref(), Some(&want), "{w}");
    }
}

#[test]
fn pack8_null_when_means_unconditional() {
    let t = pack("null", "null", "", "null");
    compile_dir(&t.0).expect("null when/requires compile");
}

#[test]
fn pack8_unknown_word_and_code_fields_are_e_parse_without_values() {
    for key in ["script", "code", "eval", "exec", "expr", "js"] {
        for place in 0..4 {
            let bad = format!("{{{key}: {SECRET}}}");
            let nested = format!("{{all: [{{not: {{{key}: {SECRET}}}}}]}}");
            for w in [bad, nested] {
                let ok = "{flag: f}";
                let (n, r, e, q) = match place {
                    0 => (w.as_str(), ok, String::new(), ok),
                    1 => (ok, w.as_str(), String::new(), ok),
                    2 => (
                        ok,
                        ok,
                        format!(", on_enter: [{{put: {{path: p}}, when: {w}}}]"),
                        ok,
                    ),
                    _ => (ok, ok, String::new(), w.as_str()),
                };
                let t = pack(n, r, &e, q);
                let d = first(&t, DiagCode::Parse);
                assert!(!d.message().contains(SECRET), "{d}");
            }
        }
    }
}

#[test]
fn pack8_word_count_and_empty_names_are_e_structure() {
    for bad in [
        "{}",
        "{flag: f, world_has: p}",
        "{flag: ''}",
        "{all: [{world_has: ''}]}",
    ] {
        let t = pack(bad, "{flag: f}", "", "{flag: f}");
        let d = first(&t, DiagCode::Structure);
        assert!(d.message().starts_with("next[0].when"), "{d}");
    }
}

#[test]
fn pack8_type_mismatches_are_e_parse_with_location() {
    let t = pack(
        "{flag: f}",
        "{count: {prefix: '', op: '!=', n: 1}}",
        "",
        "{flag: f}",
    );
    let d = first(&t, DiagCode::Parse);
    assert!(d.message().starts_with("rules[0].when"), "{d}");
    let t = pack("{flag: f}", "{flag: f}", "", "{nope: 1}");
    let d = first(&t, DiagCode::Parse);
    assert!(d.message().starts_with("requires"), "{d}");
    assert_eq!(d.item(), "start:s1");
}

#[test]
fn pack8_testdata_packs_still_compile() {
    for rel in [
        "input-field-ime/test-data/ch1-pack",
        "module-reuse-rounds/test-data/pack",
    ] {
        let dir = common::testdata(rel);
        compile_dir(&dir).unwrap_or_else(|d| panic!("{rel}: {d:?}"));
    }
}
