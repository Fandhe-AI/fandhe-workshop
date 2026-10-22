//! `do` を案内レベルごとに書き分ける欄 `do_levels` の受け入れ試験（語彙 v1.6。PACK-50・GUIDE-14、TASK-70 2/2、D-55）。
//!
//! 架空の教材 `tests/data/do-levels-pack` を `compile_dir` → `load_ir` で IR にし、core の `judge_input` と
//! `filled_text` に通す。順序違い（`wrong_order`）の拒否の次の行動は `{{step_do}}` で現在の水準の `do` を受けるため、
//! 案内レベル B で skill の名前が次の行動に出ないことが受け入れ条件である。
//! 静的な検査（鍵 `A`・未知の鍵・空の値・`do` の欠落・漏れ・語彙の版・モジュールの引数）は一時ディレクトリの
//! 小さな教材で確かめる。診断はちょうど 1 件で、書かれた値を写さない。
//!
//! 確かめないこと: 開始点の水準の結線（`RunState.guidance_level` を直接組む。TASK-294）、画面の表示、復元のときの
//! 文面の再充填（`docs/known-gaps.md` の PACK-50）。試験では `unwrap` を使ってよい。

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::{assert_no_secret, Tmp, SECRET};
use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::Judgment;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::effect::NoConditions;
use fandhe_workshop_core::input::Input;
use fandhe_workshop_core::judge::judge_input;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::{RunState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

fn pack_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/do-levels-pack")
}

fn ir_of(dir: &Path) -> Ir {
    let bytes = compile_dir(dir).unwrap_or_else(|d| panic!("compile {dir:?}: {d:?}"));
    load_ir(&bytes).expect("load_ir")
}

/// 水準 `level` で先頭の Step にいるときに、後の Step の flow コマンドを送った拒否の次の行動。
fn wrong_order_next_action(ir: &Ir, level: GuidanceLevel) -> String {
    let start = ir.starts.iter().find(|s| s.id == "s1").expect("start");
    let uid = StepUid(start.node.clone());
    let mut slice = Slice::default();
    slice.status.insert(uid.clone(), StepStatus::Entered);
    let mut slices = BTreeMap::new();
    slices.insert(uid.root(), slice);
    let run = RunState {
        current: Some(uid),
        slices,
        guidance_level: level,
        ..RunState::default()
    };
    let input = Input::Command("/beta-cmd".into());
    let outcome = judge_input(ir, &run, &input, &NoConditions).expect("judge_input");
    match outcome.judgment {
        Judgment::Rejected(r) => {
            assert_eq!(r.text, "rj-wrong-order", "wrong_order の文面");
            r.next_action.as_str().to_string()
        }
        other => panic!("expected a rejection, got {other:?}"),
    }
}

/// 試験用の教材を一時ディレクトリへ写し、章の定義を `chapter` に差し替える。
fn with_chapter(workshop_extra: &str, chapter: &str) -> Tmp {
    let t = Tmp::new("pack50");
    let ws = std::fs::read_to_string(pack_dir().join("workshop.yaml")).unwrap();
    t.put("workshop.yaml", format!("{ws}{workshop_extra}").as_bytes());
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    for f in ["a.md", "b.md", "c.md"] {
        t.put(&format!("chapters/c1/{f}"), b"body\n");
    }
    t
}

/// 終端の Step `a` だけを持つ章。`a_extra` は `a` の欄（`do` と `do_levels` など）に挿す行。入力を持たない途中の Step は
/// 作れない（PACK-19）ため、`do` の検査を確かめる `a` は終端にする。
fn chapter_with(a_extra: &str) -> String {
    format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: a}}]\nsteps:\n  - id: a\n    title: A\n    body: a.md\n{a_extra}    final: true\n"
    )
}

fn only(d: &[Diagnostic], code: DiagCode) -> Vec<&Diagnostic> {
    d.iter().filter(|x| x.code() == code).collect()
}

/// `validate` と `compile` の両方が、コード `code` の診断をちょうど 1 件、項目 `step:a` で返す。
fn assert_rejected_once(t: &Tmp, code: DiagCode) {
    let c = compile_dir(&t.0).expect_err("compile must fail");
    assert_eq!(c.len(), 1, "{c:?}");
    assert_eq!(c[0].code(), code);
    let v = validate_dir(&t.0);
    let hit = only(&v, code);
    assert_eq!(hit.len(), 1, "{v:?}");
    assert!(hit[0].to_string().contains("step:a") || code == DiagCode::Parse);
}

#[test]
fn pack50_level_b_wrong_order_next_action_hides_skill_name() {
    let ir = ir_of(&pack_dir());
    let na = wrong_order_next_action(&ir, GuidanceLevel::B);
    assert!(!na.contains("alpha-cmd"), "skill 名が出た: {na}");
    assert!(na.contains("アイデアを書くコマンドを送る"), "{na}");
}

#[test]
fn pack50_level_a_keeps_do() {
    let ir = ir_of(&pack_dir());
    let na = wrong_order_next_action(&ir, GuidanceLevel::A);
    assert!(na.contains("alpha-cmd"), "{na}");
    assert!(!na.contains("アイデアを書くコマンドを送る"), "{na}");
}

#[test]
fn pack50_level_c_falls_back_to_b() {
    let ir = ir_of(&pack_dir());
    let na = wrong_order_next_action(&ir, GuidanceLevel::C);
    assert!(na.contains("アイデアを書くコマンドを送る"), "{na}");
    // C の文を持つ写しでは C の文が出る。
    let chapter = std::fs::read_to_string(pack_dir().join("chapters/c1/chapter.yaml")).unwrap();
    let with_c = chapter.replace(
        "      B: \"アイデアを書くコマンドを送る\"\n",
        "      B: \"アイデアを書くコマンドを送る\"\n      C: \"最初のコマンドを送る\"\n",
    );
    assert_ne!(chapter, with_c);
    let t = with_chapter("", &with_c);
    let ir = ir_of(&t.0);
    assert!(wrong_order_next_action(&ir, GuidanceLevel::C).contains("最初のコマンドを送る"));
    assert!(wrong_order_next_action(&ir, GuidanceLevel::B).contains("アイデアを書くコマンドを送る"));
}

#[test]
fn pack50_ir_carries_do_levels_only_when_used() {
    let ir = ir_of(&pack_dir());
    let json = serde_json::to_string(&ir).unwrap();
    assert!(json.contains("\"do_levels\""));
    // 使わない教材の IR は、欄そのものを書き出さない。
    let t = with_chapter("", &chapter_with("    do: x\n"));
    let json = serde_json::to_string(&ir_of(&t.0)).unwrap();
    assert!(!json.contains("\"do_levels\""));
}

#[test]
fn pack50_leak_is_clean_with_do_levels() {
    let d = validate_dir(&pack_dir());
    assert!(d.is_empty(), "{d:?}");
    // do_levels だけを持つ Step（body_levels は無い）で、水準 A の本文への誤検出が出ない。
    let t = with_chapter(
        "",
        &chapter_with("    do: alpha-cmd を送る\n    do_levels: {B: 別の言い方}\n"),
    );
    let d = validate_dir(&t.0);
    assert!(only(&d, DiagCode::GuidanceLeak).is_empty(), "{d:?}");
}

#[test]
fn pack50_leak_detects_hidden_word_in_do_levels_b() {
    let t = with_chapter(
        "",
        &chapter_with("    do: alpha-cmd を送る\n    do_levels: {B: alpha-cmd を送る}\n"),
    );
    let d = validate_dir(&t.0);
    let leak = only(&d, DiagCode::GuidanceLeak);
    assert_eq!(leak.len(), 1, "{d:?}");
    // 本文の漏れとして数えない（本文には隠す語が無い）。
    assert!(leak[0].to_string().contains("do"), "{d:?}");
    assert_no_secret(&d);
}

#[test]
fn pack50_leak_check_reads_the_do_shown_at_that_level() {
    // do に隠す語があっても、B の文が清潔なら B の漏れではない。C は B に倒れるので同じく清潔。
    let t = with_chapter(
        "",
        &chapter_with("    do: alpha-cmd を送る\n    do_levels: {B: 清潔な文}\n"),
    );
    let d = validate_dir(&t.0);
    assert!(only(&d, DiagCode::GuidanceLeak).is_empty(), "{d:?}");
}

#[test]
fn pack50_key_a_is_structure() {
    let t = with_chapter("", &chapter_with("    do: x\n    do_levels: {A: y}\n"));
    assert_rejected_once(&t, DiagCode::Structure);
}

#[test]
fn pack50_unknown_key_is_parse() {
    let t = with_chapter("", &chapter_with("    do: x\n    do_levels: {b: y}\n"));
    let c = compile_dir(&t.0).expect_err("compile must fail");
    assert_eq!(only(&c, DiagCode::Parse).len(), 1, "{c:?}");
    let v = validate_dir(&t.0);
    assert_eq!(only(&v, DiagCode::Parse).len(), 1, "{v:?}");
}

#[test]
fn pack50_blank_value_is_structure() {
    for blank in ["''", "' '"] {
        let t = with_chapter(
            "",
            &chapter_with(&format!("    do: x\n    do_levels: {{B: {blank}}}\n")),
        );
        assert_rejected_once(&t, DiagCode::Structure);
    }
}

#[test]
fn pack50_requires_do() {
    let t = with_chapter("", &chapter_with("    do_levels: {B: y}\n"));
    assert_rejected_once(&t, DiagCode::Structure);
    let t = with_chapter("", &chapter_with("    do: ' '\n    do_levels: {B: y}\n"));
    assert_rejected_once(&t, DiagCode::Structure);
}

#[test]
fn pack50_non_string_value_is_parse() {
    // 配列・表は型の読み込みで止まる。`~`・`null` は既存の `do` と同じく文字列として読まれる（PACK-4）。
    for v in ["[a]", "{k: v}"] {
        let t = with_chapter(
            "",
            &chapter_with(&format!("    do: x\n    do_levels: {{B: {v}}}\n")),
        );
        let c = compile_dir(&t.0).expect_err("compile must fail");
        assert_eq!(only(&c, DiagCode::Parse).len(), 1, "{v}: {c:?}");
    }
}

#[test]
fn pack50_vocab_mismatch_below_1_6() {
    let chapter = std::fs::read_to_string(pack_dir().join("chapters/c1/chapter.yaml")).unwrap();
    let ws = std::fs::read_to_string(pack_dir().join("workshop.yaml")).unwrap();
    let put = |ws: &str| {
        let t = Tmp::new("pack50v");
        t.put("workshop.yaml", ws.as_bytes());
        t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
        for f in ["a.md", "b.md", "c.md"] {
            t.put(&format!("chapters/c1/{f}"), b"body\n");
        }
        t
    };
    let old = put(&ws.replace("vocab: \"1.6\"", "vocab: \"1.5\""));
    let d = validate_dir(&old.0);
    assert_eq!(only(&d, DiagCode::VocabMismatch).len(), 1, "{d:?}");
    // 省略した教材は検査しない。
    let none = put(&ws.replace("  vocab: \"1.6\"\n", ""));
    let d = validate_dir(&none.0);
    assert!(only(&d, DiagCode::VocabMismatch).is_empty(), "{d:?}");
}

const MODULE_WS: &str = "";

fn module_pack(arg: &str) -> Tmp {
    let t = with_chapter(
        MODULE_WS,
        "id: c1\ntitle: C\nstarts: [{id: s1, node: u1}]\nsteps:\n  - {id: u1, title: U, body: a.md, use: {module: m, instance: i, args: {x: ARG}}, final: true}\n"
            .replace("ARG", arg)
            .as_str(),
    );
    t.put(
        "modules/m/module.yaml",
        "id: m\ntitle: M\nparams: {x: base}\nsteps:\n  - {id: a, title: T, body: a.md, do: 'alpha-cmd ${x}', do_levels: {B: '${x}'}, input: {kind: command, name: beta-cmd}, rules: [{id: r, on: {command: beta-cmd}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}, final: true}\n"
            .as_bytes(),
    );
    t.put("modules/m/a.md", b"body\n");
    t
}

#[test]
fn pack50_module_arg_substitution() {
    let t = module_pack("abc");
    let ir = ir_of(&t.0);
    let node = ir.nodes.iter().find(|n| n.step.id == "a").expect("node a");
    assert_eq!(node.step.do_levels["B"], "abc");
    assert_eq!(node.step.do_.as_deref(), Some("alpha-cmd abc"));
}

#[test]
fn pack50_module_blank_arg_is_caught_after_expansion() {
    // 空の引数で水準の文が空になると、`step_do` を使う文面があるとき展開の後で止まる。
    let t = module_pack("''");
    let d = validate_dir(&t.0);
    assert!(
        d.iter()
            .any(|x| x.code() == DiagCode::SlotNeedsDo || x.code() == DiagCode::EmptyNextAction),
        "{d:?}"
    );
}

/// `step_do` を使う文面が無い教材で、引数の置換の後に `do_levels` の値・`do` が空になる組を `validate` と
/// `compile` の両方が `E-STRUCTURE` で拒否する（PACK-50）。
fn expanded_structure_pack(step_fields: &str, arg: &str) -> Tmp {
    let t = with_chapter(
        MODULE_WS,
        "id: c1\ntitle: C\nstarts: [{id: s1, node: u1}]\nsteps:\n  - {id: u1, title: U, body: a.md, use: {module: m, instance: i, args: {x: ARG}}, final: true}\n"
            .replace("ARG", arg)
            .as_str(),
    );
    t.put(
        "modules/m/module.yaml",
        format!("id: m\ntitle: M\nparams: {{x: base}}\nsteps:\n  - {{id: a, title: T, body: a.md, {step_fields}, final: true}}\n")
            .as_bytes(),
    );
    t.put("modules/m/a.md", b"body\n");
    t
}

fn assert_structure_both(t: &Tmp, message: &str) {
    let v = validate_dir(&t.0);
    let c = compile_dir(&t.0).expect_err("compile rejects");
    for d in [&v, &c] {
        assert!(
            d.iter()
                .any(|x| x.code() == DiagCode::Structure && x.message() == message),
            "{d:?}"
        );
        assert_no_secret(d);
    }
}

#[test]
fn pack50_blank_level_arg_without_step_do_is_rejected() {
    let t = expanded_structure_pack("do: act, do_levels: {B: '${x}'}", "''");
    assert_structure_both(&t, "do_levels values must not be empty");
}

#[test]
fn pack50_blank_do_arg_with_levels_is_rejected() {
    let t = expanded_structure_pack("do: '${x}', do_levels: {B: fine}", "''");
    assert_structure_both(&t, "do_levels requires do");
}

#[test]
fn pack50_nonblank_args_pass_expansion_check() {
    let t = expanded_structure_pack("do: '${x}', do_levels: {B: '${x}'}", "abc");
    assert!(compile_dir(&t.0).is_ok());
}

#[test]
fn pack50_values_are_not_echoed() {
    for extra in [
        format!("    do: x\n    do_levels: {{A: {SECRET}}}\n"),
        format!("    do: x\n    do_levels: {{{SECRET}: y}}\n"),
        format!("    do_levels: {{B: {SECRET}}}\n"),
    ] {
        let t = with_chapter("", &chapter_with(&extra));
        assert_no_secret(&compile_dir(&t.0).expect_err("rejected"));
        assert_no_secret(&validate_dir(&t.0));
    }
}
