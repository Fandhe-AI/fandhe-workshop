//! 状態変化の語彙（要素・属性・フラグ）の「読み込み → IR → 適用」の結合試験（TASK-43 1/2、PACK-9）。
//!
//! 読み込み時の検査は pack、適用は core が持つ（置き場の理由は `docs/architecture.md` の第 6 節）。
//! 試験名の `pack9_` で 04 の ID を追える。

mod common;

use common::{assert_no_secret, Tmp, SECRET, WORKSHOP};

use fandhe_workshop_contract::effect::EffectItem;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Root, Stamp, StepUid};
use fandhe_workshop_core::effect::apply_effect;
use fandhe_workshop_core::event::Cause;
use fandhe_workshop_core::session::RunState;
use fandhe_workshop_core::world::Element;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const WS: &str = "fixtures:\n  f1: {path: fixtures/x.txt, kind: document}\n";

/// 章の 1 つ目の Step（`a`）の欄と開始点の `effects` を差し込んだパックを作る。
fn pack(step_extra: &str, start_effects: &str) -> Tmp {
    let t = Tmp::new("effects");
    t.put("workshop.yaml", format!("{WORKSHOP}{WS}").as_bytes());
    t.put("fixtures/x.txt", b"x");
    t.put(
        "chapters/c1/chapter.yaml",
        format!(
            "id: c1\ntitle: C\nstarts: [{{id: s1, node: a, effects: {start_effects}}}]\nsteps:\n  - {{id: a, title: A, next: [{{to: b}}]{step_extra}}}\n  - {{id: b, title: B, final: true}}\n"
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

fn one(t: &Tmp, code: DiagCode) -> Diagnostic {
    let mut d = diags(t);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code(), code, "{d:?}");
    assert_no_secret(&d);
    d.remove(0)
}

const SEVEN: &str = concat!(
    ", on_enter: [{put: {path: panel, fixture: f1, attrs: {k: v}}}, {set_attr: {path: panel, attr: k, value: w}},",
    " {set_flag: f}, {clear_flag: f}, {set_flag_global: g}],",
    " on_achieved: [{replace: {path: panel, attrs: {z: y}}}, {remove: panel}]"
);

fn ir_of(t: &Tmp) -> Ir {
    let bytes = compile_dir(&t.0).expect("compile");
    serde_json::from_slice(&bytes).expect("ir")
}

#[test]
fn pack9_seven_words_compile_and_stay_values_in_the_ir() {
    let t = pack(SEVEN, "[{put: {path: seed}}]");
    let ir = ir_of(&t);
    let a = ir.nodes.iter().find(|n| n.uid == "main/a").expect("node a");
    assert_eq!(a.step.on_enter.len(), 5);
    assert_eq!(a.step.on_enter[0]["put"]["path"], "panel");
    assert_eq!(a.step.on_achieved.len(), 2);
    assert_eq!(a.step.on_achieved[1]["remove"], "panel");
    assert_eq!(ir.starts[0].effects.len(), 1);
}

#[test]
fn pack9_load_to_ir_to_apply() {
    let t = pack(SEVEN, "[{put: {path: seed, attrs: {s: t}}}]");
    let ir = ir_of(&t);
    let at = Stamp {
        generation: Generation::INITIAL,
        step: StepUid("main/a".into()),
    };
    let root = Root("main".into());
    let mut run = RunState::default();
    let apply_all = |run: &mut RunState, list: &[serde_json::Value]| {
        for v in list {
            let item = EffectItem::from_value(v).expect("parse");
            apply_effect(run, &root, &item.effect, &at, Cause::Start);
        }
    };
    apply_all(&mut run, &ir.starts[0].effects);
    assert_eq!(run.world["seed"].attrs["s"], "t");
    let a = ir.nodes.iter().find(|n| n.uid == "main/a").expect("node a");
    apply_all(&mut run, &a.step.on_enter);
    assert_eq!(
        run.world["panel"],
        Element {
            fixture: Some("f1".into()),
            attrs: [("k".to_string(), "w".to_string())].into()
        }
    );
    assert!(run.slices[&root].flags.is_empty());
    assert!(run.global_flags.contains("g"));
    apply_all(&mut run, &a.step.on_achieved);
    assert!(!run.world.contains_key("panel"));
    assert!(run.world.contains_key("seed"));
}

#[test]
fn pack9_unknown_field_is_e_parse() {
    let t = pack(
        &format!(", on_enter: [{{put: {{path: a}}, script: {SECRET}}}]"),
        "[]",
    );
    let d = one(&t, DiagCode::Parse);
    assert!(d.message().contains("unknown field `script`"), "{d}");
    assert!(d.message().starts_with("on_enter[0]"), "{d}");
    assert_eq!(d.item(), "step:a");
}

#[test]
fn pack9_word_count_is_e_structure() {
    for extra in [
        ", on_enter: [{}]",
        ", on_enter: [{put: {path: a}, remove: a}]",
    ] {
        one(&pack(extra, "[]"), DiagCode::Structure);
    }
}

#[test]
fn pack9_undeclared_fixture_is_e_ref_fixture() {
    let t = pack(", on_enter: [{put: {path: a, fixture: nope}}]", "[]");
    one(&t, DiagCode::RefFixture);
    let t = pack("", "[{replace: {path: a, fixture: nope}}]");
    let d = one(&t, DiagCode::RefFixture);
    assert_eq!(d.item(), "start:s1");
}

#[test]
fn pack9_empty_names_are_e_structure() {
    one(
        &pack(", on_enter: [{put: {path: ''}}]", "[]"),
        DiagCode::Structure,
    );
    one(&pack("", "[{set_flag: ''}]"), DiagCode::Structure);
}

#[test]
fn pack9_unquoted_number_is_e_parse_without_echoing_values() {
    let t = pack(
        ", on_enter: [{set_attr: {path: a, attr: k, value: 1}}]",
        "[]",
    );
    let d = one(&t, DiagCode::Parse);
    assert!(!d.message().contains("value: 1"), "{d}");
}

#[test]
fn pack9_format_a_accepts_and_rejects() {
    let toml = |effects: &str| {
        let t = Tmp::new("effects-toml");
        t.put(
            "workshop.toml",
            b"[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n",
        );
        t.put(
            "chapters/c1/chapter.toml",
            format!(
                "id = \"c1\"\ntitle = \"C\"\n[[starts]]\nid = \"s1\"\nnode = \"a\"\neffects = {effects}\n[[steps]]\nid = \"a\"\ntitle = \"A\"\nfinal = true\n"
            )
            .as_bytes(),
        );
        t
    };
    assert!(compile_dir(&toml("[{set_flag = \"f\"}]").0).is_ok());
    let t = toml(&format!("[{{set_flag = \"f\", script = \"{SECRET}\"}}]"));
    one(&t, DiagCode::Parse);
}
