//! モジュールの先頭の Step を呼び出しごとに省く語が無いこと（忠実さの限界）の観察の固定（TASK-71 1/3、PACK-51・D-43）。
//!
//! 決定（D-43、2026-10-12）の項 7 は「先頭の Step を省ける語は足さず、作成者の規約として受け入れる」である
//! （PACK-51・MOD-17 (a)）。本試験は、その現状を testdata と一時ディレクトリの小さな教材で固定する。
//!
//! - `use` の node の `enter` は、引数がどう Step を迂回させても、モジュールの先頭の Step を指す
//! - 引数で迂回した Step も IR から消えず、遷移で通らないだけである（静的な側）
//! - `use` や Step に省く趣旨の欄を書くと `compile` も `validate` も `E-PARSE` で止める（fail-closed）。値は診断に写らない
//!
//! 確かめないこと: 迂回した Step が実行時に未着手のまま残り、途中開始の「省略」と区別されること
//! （MOD-17 (b) の実行時の側は `module_skip_steps.rs`、`docs/testing.md` の 5.422）、引数の型と範囲の検査（TASK-71 2/3）。
//! 試験では `unwrap` を使ってよい。

mod common;

use common::{assert_no_secret, testdata, Tmp, SECRET, WORKSHOP};
use fandhe_workshop_contract::ir::{Ir, Node};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::DiagCode;

const PACK: &str = "module-reuse-rounds/test-data/pack";

const MODULE: &str = "id: m\ntitle: M\nparams: {skip: \"false\"}\nsteps:\n  - id: a\n    title: A\n    input: {kind: choice, options: [{id: ok, label: OK}]}\n    rules: [{id: r-a, on: {option: ok}, outcome: {kind: accept}}]\n    confirm: {kind: none, reason: r}\n    next:\n      - {to: c, when: {param: {name: skip, eq: \"true\"}}}\n      - {to: b}\n  - {id: b, title: B, next: [{to: c}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-b, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}\n  - {id: c, title: C, final: true}\n";

fn pack_with(module: &str, steps: &str) -> Tmp {
    let t = Tmp::new("p51");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    let chapter = format!("id: c1\ntitle: C\nstarts: [{{id: s1, node: u1}}]\nsteps:\n{steps}");
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t.put("modules/m/module.yaml", module.as_bytes());
    t
}

fn pack(use_: &str) -> Tmp {
    pack_with(
        MODULE,
        &format!("  - {{id: u1, title: U1, use: {use_}, final: true}}\n"),
    )
}

fn node<'a>(ir: &'a Ir, uid: &str) -> &'a Node {
    ir.nodes
        .iter()
        .find(|n| n.uid == uid)
        .unwrap_or_else(|| panic!("node {uid} not found"))
}

fn ir_of(dir: &std::path::Path) -> Ir {
    load_ir(&compile_dir(dir).expect("compile")).expect("load_ir")
}

#[test]
fn pack51_use_enters_the_first_step_even_when_args_route_around_steps() {
    let ir = ir_of(&testdata(PACK));
    // 1 周目（設定の 2 Step を通る）も 2 周目（`setup: "false"` で通らない）も、入り口は先頭の `st-3-2` である。
    for (call, round) in [("main/r1-impl", "round1"), ("main/r2-impl", "round2")] {
        assert_eq!(
            node(&ir, call).enter.as_deref(),
            Some(format!("issue-to-merge@{round}/st-3-2").as_str()),
            "{call}"
        );
    }
    // 2 周目で通らない Step も IR に残る。省くのは遷移であって、Step の定義ではない。
    for id in ["st-4-1", "st-5-1"] {
        node(&ir, &format!("issue-to-merge@round2/{id}"));
    }
    // 2 周目の先頭の Step は、引数の条件で `st-4-1` と `st-6-1` に分かれる。
    let head = node(&ir, "issue-to-merge@round2/st-3-2");
    let to: Vec<&str> = head.step.next.iter().map(|t| t.to.as_str()).collect();
    assert_eq!(
        to,
        [
            "issue-to-merge@round2/st-4-1",
            "issue-to-merge@round2/st-6-1"
        ]
    );
    assert!(head.step.next[0].when.is_some());
    assert!(head.step.next[1].when.is_none());
}

#[test]
fn pack51_enter_is_the_first_step_for_a_small_module() {
    for (inst, skip) in [("i1", "true"), ("i2", "false")] {
        let t = pack(&format!(
            "{{module: m, instance: {inst}, args: {{skip: \"{skip}\"}}}}"
        ));
        let ir = ir_of(&t.0);
        assert_eq!(
            node(&ir, "main/u1").enter.as_deref(),
            Some(format!("m@{inst}/a").as_str()),
            "skip={skip}"
        );
        // 迂回される `b` も IR に残る。
        node(&ir, &format!("m@{inst}/b"));
    }
}

const SKIP_LIKE: [&str; 3] = ["start: b", "skip_first: \"true\"", "from: b"];

#[test]
fn pack51_skip_like_use_field_is_rejected_by_compile() {
    for field in SKIP_LIKE {
        let t = pack(&format!("{{module: m, instance: i, {field}}}"));
        let d = compile_dir(&t.0).expect_err("use has no field that skips the first step");
        assert_eq!(d.len(), 1, "{field}: {d:?}");
        assert_eq!(d[0].code(), DiagCode::Parse, "{field}");
    }
}

#[test]
fn pack51_skip_like_use_field_is_rejected_by_validate() {
    for field in SKIP_LIKE {
        let t = pack(&format!("{{module: m, instance: i, {field}}}"));
        let d = validate_dir(&t.0);
        let parse = d.iter().filter(|x| x.code() == DiagCode::Parse).count();
        assert_eq!(parse, 1, "{field}: {d:?}");
    }
}

#[test]
fn pack51_step_level_skip_field_is_rejected() {
    let module = MODULE.replace(
        "  - id: a\n    title: A\n",
        "  - id: a\n    title: A\n    skip: true\n",
    );
    let t = pack_with(
        &module,
        "  - {id: u1, title: U1, use: {module: m, instance: i}, final: true}\n",
    );
    let d = compile_dir(&t.0).expect_err("Step has no skip field");
    assert!(d.iter().all(|x| x.code() == DiagCode::Parse), "{d:?}");
    assert_eq!(d.len(), 1, "{d:?}");
}

#[test]
fn pack51_skip_like_values_are_not_echoed() {
    let t = pack(&format!("{{module: m, instance: i, start: {SECRET}}}"));
    assert_no_secret(&compile_dir(&t.0).expect_err("rejected"));
    assert_no_secret(&validate_dir(&t.0));
}
