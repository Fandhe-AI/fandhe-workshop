//! 本文・ヒントへの引数の値の差し込みが無いこと（忠実さの限界）の観察の固定（TASK-302、GUIDE-15）。
//!
//! 決定の状況（2026-10-23 時点）: D-50（H-3 の差し込みを足さない）と D-55（実体ごとの案内レベルを足さない）が決めた
//! 範囲に、本文の木（IR 版 2）とヒントの文面への引数の値の差し込みは含まれない。GUIDE-15 と PACK-45 は決定が無く、
//! 差し込み語は語彙に無い。本試験は、足さない現状を一時ディレクトリの小さな教材と実パック（PoC-4）で固定する。
//! 差し込み語を足す決定が出たときは、本試験の期待を決定に合わせて書き直す。
//!
//! 固定する観察:
//! - 本文の `${名前}` は置換されず、木に字面で残る。実体ごとに同じ木になる
//! - 本文の未定義の `${名前}` は検査されない。同じ字面を `do` に書くと `E-STRUCTURE` になる
//! - 本文の `{{step_do}}` は充填されない。充填は拒否・問い返しの文面だけに効く
//! - ヒントの文面は実体で共有される（`pack45_` の観察を core の選択まで通す）
//! - 対照: `do` は実体ごとに置換され、拒否の文面の `{{step_do}}` が実体ごとの値で埋まる
//! - 本文への差し込みらしい未知の欄は `E-PARSE` で止まる
//! - PoC-4 の実パックは、題材の名前を本文から言い換えて避けている
//!
//! 確かめないこと: 画面での本文・ヒントの表示（TASK-530・SHELL-7）、差し込み語の採否（spec 側の決定待ち）。
//! 試験では `unwrap` を使ってよい。

mod common;

use common::{assert_no_secret, testdata, Tmp, SECRET, WORKSHOP};
use fandhe_workshop_contract::doc::Doc;
use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::{Ir, Node};
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::body_level::select_body;
use fandhe_workshop_core::hint_select::hints_at;
use fandhe_workshop_core::input::Input;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::RunState;
use fandhe_workshop_core::text_fill::{fill_next_action, slot_values, step_do_at};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::DiagCode;

const LEVELS: [GuidanceLevel; 3] = [GuidanceLevel::A, GuidanceLevel::B, GuidanceLevel::C];

/// 本文・ヒントの文面に差し込みらしい字面を置き、`do` には同じ名前を正しく書く（対照）モジュール `m`。
const MODULE: &str = "id: m\ntitle: M\nparams: {name: x}\nsteps:\n  - {id: a, title: T, do: \"${name} を作る\", body: a.md, body_levels: {B: a.b.md}, hints: [h-a], final: true}\n";

fn pack(a_md: &str, b_md: &str, module_yaml: &str, step_extra: &str) -> Tmp {
    let t = Tmp::new("guide15");
    let ws = format!("{WORKSHOP}hints:\n  h-a: {{text: 'hint ${{name}}', levels: [A]}}\n");
    t.put("workshop.yaml", ws.as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: u1}}]\nsteps:\n  - {{id: u1, title: U1, next: [{{to: u2}}], use: {{module: m, instance: i1, args: {{name: alpha}}}}{step_extra}}}\n  - {{id: u2, title: U2, final: true, use: {{module: m, instance: i2, args: {{name: beta}}}}}}\n"
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t.put("modules/m/module.yaml", module_yaml.as_bytes());
    t.put("modules/m/a.md", a_md.as_bytes());
    t.put("modules/m/a.b.md", b_md.as_bytes());
    t
}

fn ir_of(t: &Tmp) -> Ir {
    load_ir(&compile_dir(&t.0).expect("compile")).expect("load_ir")
}

/// 実体 `scope` が展開したモジュール `m` の Step `a`（Node の `scope` は `m@<実体 ID>`）。
fn node_of<'a>(ir: &'a Ir, scope: &str) -> &'a Node {
    ir.nodes
        .iter()
        .find(|n| n.scope == format!("m@{scope}") && n.step.id == "a")
        .unwrap_or_else(|| panic!("node of {scope}"))
}

/// 木の JSON 表現。差し込みの字面が残るかの確認に使う。
fn flat(doc: &Doc) -> String {
    serde_json::to_string(doc).unwrap()
}

#[test]
fn guide15_module_body_placeholder_stays_literal_in_the_tree() {
    let t = pack("# ${name}\n", "# b ${name}\n", MODULE, "");
    let ir = ir_of(&t);
    for scope in ["i1", "i2"] {
        let n = node_of(&ir, scope);
        let a = flat(n.body_doc.as_ref().expect("body_doc"));
        let b = flat(&n.body_levels_doc["B"]);
        for s in [&a, &b] {
            assert!(s.contains("${name}"), "{scope}: {s}");
            assert!(!s.contains("alpha") && !s.contains("beta"), "{scope}: {s}");
        }
    }
}

#[test]
fn guide15_instances_share_the_same_body_tree() {
    let t = pack("# ${name}\n", "# b ${name}\n", MODULE, "");
    let ir = ir_of(&t);
    let (n1, n2) = (node_of(&ir, "i1"), node_of(&ir, "i2"));
    for lvl in LEVELS {
        let (s1, s2) = (select_body(n1, lvl).unwrap(), select_body(n2, lvl).unwrap());
        assert_eq!(s1.level, s2.level, "{lvl}");
        assert_eq!(s1.doc, s2.doc, "{lvl}");
    }
}

#[test]
fn guide15_undefined_name_and_slot_in_body_are_not_checked_or_filled() {
    let t = pack("# ${undefined} {{step_do}}\n", "# b\n", MODULE, "");
    // 本文の未定義の名前は検査されず、compile も通る（`ir_of`）。
    let d = validate_dir(&t.0);
    assert!(d.iter().all(|x| x.code() != DiagCode::Structure), "{d:?}");
    let ir = ir_of(&t);
    let a = flat(node_of(&ir, "i1").body_doc.as_ref().unwrap());
    assert!(a.contains("${undefined}"), "{a}");
    // 拒否・問い返しの枠は、本文では充填されない。
    assert!(a.contains("{{step_do}}"), "{a}");
    // 対照: 同じ字面を `do` に書くと E-STRUCTURE になる。
    let bad = "id: m\ntitle: M\nparams: {name: x}\nsteps:\n  - {id: a, title: T, do: \"${undefined}\", body: a.md, final: true}\n";
    let t2 = pack("# a\n", "# b\n", bad, "");
    let d2 = compile_dir(&t2.0).expect_err("undefined argument in do");
    assert!(d2.iter().any(|x| x.code() == DiagCode::Structure), "{d2:?}");
}

#[test]
fn guide15_hint_text_is_shared_between_instances() {
    let t = pack("# a\n", "# b\n", MODULE, "");
    let ir = ir_of(&t);
    let (n1, n2) = (node_of(&ir, "i1"), node_of(&ir, "i2"));
    let h1 = hints_at(&ir, &StepUid(n1.uid.clone()), GuidanceLevel::A);
    let h2 = hints_at(&ir, &StepUid(n2.uid.clone()), GuidanceLevel::A);
    assert_eq!(h1, ["h-a"]);
    assert_eq!(h1, h2);
    // 文面は IR に 1 つで、`${name}` が残る（`per_instance_guidance_gap.rs` の `pack45_` と同じ観察を core の選択まで通す）。
    let text = ir.hints["h-a"].to_string();
    assert!(text.contains("${name}"), "{text}");
    assert!(!text.contains("alpha") && !text.contains("beta"), "{text}");
}

#[test]
fn guide15_do_and_rejection_text_receive_argument_values() {
    let t = pack("# a\n", "# b\n", MODULE, "");
    let ir = ir_of(&t);
    let mut got = Vec::new();
    for scope in ["i1", "i2"] {
        let n = node_of(&ir, scope);
        let d = step_do_at(&n.step, GuidanceLevel::A)
            .expect("do")
            .to_string();
        let run = RunState::default();
        let v = slot_values(&ir, &n.step, &run, &Input::Command("/x".into()), None).unwrap();
        let na = fill_next_action("今の Step では、{{step_do}}。", &v).unwrap();
        assert!(format!("{na:?}").contains(&d), "{na:?}");
        got.push(d);
    }
    assert_eq!(got, ["alpha を作る", "beta を作る"]);
}

#[test]
fn guide15_no_insertion_word_for_body() {
    let t = pack(
        "# a\n",
        "# b\n",
        MODULE,
        &format!(", body_args: {{name: {SECRET}}}"),
    );
    // 差し込みらしい欄は語彙に無く、compile も validate も E-PARSE で止まる。
    let d = compile_dir(&t.0).expect_err("unknown key");
    assert!(d.iter().any(|x| x.code() == DiagCode::Parse), "{d:?}");
    assert_no_secret(&d);
    let v = validate_dir(&t.0);
    assert!(v.iter().any(|x| x.code() == DiagCode::Parse), "{v:?}");
    assert_no_secret(&v);
}

#[test]
fn guide15_poc4_rounds_paraphrase_the_subject() {
    let dir = testdata("module-reuse-rounds/test-data/pack");
    let ir = load_ir(&compile_dir(&dir).expect("compile")).expect("load_ir");
    let mut pairs = 0;
    let mut do_differs = 0;
    for n1 in ir.nodes.iter().filter(|n| n.scope.ends_with("@round1")) {
        let Some(n2) = ir
            .nodes
            .iter()
            .find(|n| n.scope.ends_with("@round2") && n.unit == n1.unit && n.step.id == n1.step.id)
        else {
            continue;
        };
        pairs += 1;
        for lvl in LEVELS {
            let (s1, s2) = (select_body(n1, lvl), select_body(n2, lvl));
            assert_eq!(s1, s2, "{} {lvl}", n1.uid);
            if let Some(s) = s1 {
                let text = flat(s.doc);
                assert!(
                    !text.contains("bbs-with-login") && !text.contains("bbs-search"),
                    "{text}"
                );
            }
        }
        if step_do_at(&n1.step, GuidanceLevel::A) != step_do_at(&n2.step, GuidanceLevel::A) {
            do_differs += 1;
        }
    }
    assert!(pairs >= 10, "pairs = {pairs}");
    assert!(do_differs >= 1, "do_differs = {do_differs}");
}
