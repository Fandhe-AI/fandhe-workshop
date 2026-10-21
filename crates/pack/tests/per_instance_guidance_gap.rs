//! 実体ごとの案内レベルの引数が語彙に無いこと（忠実さの限界）の観察の固定（TASK-70 1/2、PACK-45・D-55）。
//!
//! 決定（D-55、2026-10-12）は「実体ごとの案内レベル `use.guidance` を語彙に足さない」である（GUIDE-13。案内レベルは
//! 実行全体で 1 つで、開始点の `starts[].guidance` で渡す）。本文への引数の値の差し込み（GUIDE-15）とヒントの文面を
//! 実体ごとに変えることには決定が無い。本試験は、その現状を一時ディレクトリの小さな教材で固定する。
//!
//! - `use` に `guidance` を書くと `compile` も `validate` も `E-PARSE` で止める（fail-closed）。値は診断に写らない
//! - IR の実体は案内レベルの欄を持たない
//! - 回避として、ヒントの ID を引数で渡せば実体ごとに別のヒントを指せる。ただしヒントの定義の文面は置換されない
//!
//! 確かめないこと: 置換後のヒント ID の存在の検査（現状は無い。`docs/known-gaps.md`）、実行時の表示。
//! 試験では `unwrap` を使ってよい。

mod common;

use common::{assert_no_secret, Tmp, SECRET, WORKSHOP};
use fandhe_workshop_contract::guidance::{visible_hints, GuidanceLevel};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::DiagCode;

const MODULE: &str =
    "id: m\ntitle: M\nparams: {h: h-a}\nsteps:\n  - {id: a, title: T, hints: [\"${h}\"], final: true}\n";

fn pack(workshop: &str, use_: &str) -> Tmp {
    let t = Tmp::new("p45");
    t.put("workshop.yaml", workshop.as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: u1}}]\nsteps:\n  - {{id: u1, title: U1, use: {use_}, final: true}}\n"
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t.put("modules/m/module.yaml", MODULE.as_bytes());
    t
}

#[test]
fn pack45_use_guidance_is_rejected_by_compile() {
    let t = pack(WORKSHOP, "{module: m, instance: i, guidance: B}");
    let d = compile_dir(&t.0).expect_err("use.guidance is not a vocabulary word");
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code(), DiagCode::Parse);
}

#[test]
fn pack45_use_guidance_is_rejected_by_validate() {
    let t = pack(WORKSHOP, "{module: m, instance: i, guidance: B}");
    let d = validate_dir(&t.0);
    let parse: Vec<_> = d.iter().filter(|x| x.code() == DiagCode::Parse).collect();
    assert_eq!(parse.len(), 1, "{d:?}");
}

#[test]
fn pack45_use_guidance_value_is_not_echoed() {
    let t = pack(
        WORKSHOP,
        &format!("{{module: m, instance: i, guidance: {SECRET}}}"),
    );
    assert_no_secret(&compile_dir(&t.0).expect_err("rejected"));
    assert_no_secret(&validate_dir(&t.0));
}

#[test]
fn pack45_hint_ids_can_differ_per_instance_but_texts_are_not_substituted() {
    let ws = format!(
        "{WORKSHOP}hints:\n  h-a: {{text: 'first ${{x}}', levels: [A]}}\n  h-b: {{text: second, levels: [B]}}\n"
    );
    let t1 = pack(&ws, "{module: m, instance: i1, args: {h: h-a}}");
    let t2 = pack(&ws, "{module: m, instance: i2, args: {h: h-b}}");
    let find = |dir: &std::path::Path| {
        let ir = load_ir(&compile_dir(dir).expect("compile")).expect("load_ir");
        let node = ir
            .nodes
            .iter()
            .find(|n| n.step.id == "a")
            .expect("node a")
            .clone();
        (ir, node)
    };
    let (ir1, n1) = find(&t1.0);
    let (ir2, n2) = find(&t2.0);
    assert_eq!(n1.step.hints, ["h-a"]);
    assert_eq!(n2.step.hints, ["h-b"]);
    assert_eq!(visible_hints(&ir1, &n1.step, GuidanceLevel::A), ["h-a"]);
    assert!(visible_hints(&ir2, &n2.step, GuidanceLevel::A).is_empty());
    assert_eq!(visible_hints(&ir2, &n2.step, GuidanceLevel::B), ["h-b"]);
    // 定義の文面の `${}` は置換されない。
    assert!(ir1.hints["h-a"].to_string().contains("${x}"));
    // 実体は案内レベルの欄を持たない。
    let inst = serde_json::to_value(&ir1.instances).unwrap();
    for i in inst.as_array().unwrap() {
        for k in i.as_object().unwrap().keys() {
            assert!(
                ["id", "module", "args", "caller"].contains(&k.as_str()),
                "{k}"
            );
        }
    }
}
