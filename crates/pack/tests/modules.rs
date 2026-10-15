//! 学習モジュールの `use` の展開（PACK-19、TASK-50 1/2）の結合試験。
//!
//! 受入基準は 2 つ。(a) 2 つの実体（`round1`・`round2`）が別の Step の実体になり、モジュールの定義の
//! 1 か所の修正が両方に反映される。(b) 3 周目の実体を、章の定義ファイルを 1 つ足すだけで書ける。
//! `testdata/` の写しは書き換えず、一時ディレクトリへ写してから改変する。
//! なお、spec の例の `impl-run` は、この testdata では `issue-to-merge` に当たる。

mod common;

use std::fs;
use std::path::Path;

use common::{copy_dir, testdata, Tmp, WORKSHOP};

use fandhe_workshop_contract::ir::{Ir, Node};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const PACK: &str = "module-reuse-rounds/test-data/pack";
const MODULE_YAML: &str = "modules/issue-to-merge/module.yaml";
/// 手順 4（`step4-edit.md`）が直す行。行頭の空白が 12 個の行だけが `st-6-1` の規則の行である。
const EDIT_FROM: &str = "\n            - put: { path: \"issues/child-${child}\", fixture: fx-kakuu-issue-child, attrs: { state: open } }";
const EDIT_TO: &str = "\n            - put: { path: \"issues/child-${child}\", fixture: fx-kakuu-issue-child, attrs: { state: open, label: tracked } }";

fn ir_of(dir: &Path) -> Ir {
    let bytes = compile_dir(dir).expect("compile");
    load_ir(&bytes).expect("load_ir")
}

fn node<'a>(ir: &'a Ir, uid: &str) -> &'a Node {
    ir.nodes
        .iter()
        .find(|n| n.uid == uid)
        .unwrap_or_else(|| panic!("node {uid} not found"))
}

fn copy_pack() -> Tmp {
    let t = Tmp::new("mod");
    copy_dir(&testdata(PACK), &t.0);
    t
}

fn diags(t: &Tmp) -> Vec<Diagnostic> {
    compile_dir(&t.0).expect_err("expected diagnostics")
}

fn codes(d: &[Diagnostic]) -> Vec<DiagCode> {
    d.iter().map(Diagnostic::code).collect()
}

#[test]
fn two_instances_become_distinct_nodes() {
    let dir = testdata(PACK);
    let ir = ir_of(&dir);
    assert_eq!(ir.nodes.len(), 36);
    assert_eq!(ir.instances.len(), 5);
    assert_eq!(ir.starts.len(), 19);

    let ids: Vec<&str> = ir.instances.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "main",
            "idea-cycle@round1",
            "issue-to-merge@round1",
            "idea-cycle@round2",
            "issue-to-merge@round2"
        ]
    );
    let r1 = node(&ir, "issue-to-merge@round1/st-7-2");
    let r2 = node(&ir, "issue-to-merge@round2/st-7-2");
    assert_eq!(r1.scope, "issue-to-merge@round1");
    assert_eq!(r2.scope, "issue-to-merge@round2");
    assert_eq!(r1.unit, "issue-to-merge");
    assert_eq!(r2.unit, "issue-to-merge");

    // 引数の置換の結果が実体ごとに違う。
    let rules = |n: &Node| serde_json::to_string(&n.step.rules).expect("json");
    assert!(rules(node(&ir, "issue-to-merge@round1/st-6-1")).contains("issues/child-1\""));
    assert!(rules(node(&ir, "issue-to-merge@round2/st-6-1")).contains("issues/child-9\""));

    // `use` の node の enter と、実体の終端の node の exit_to が呼び出し元を結ぶ。途中の node は戻り先を持たない。
    let call = node(&ir, "main/r1-impl");
    assert_eq!(call.enter.as_deref(), Some("issue-to-merge@round1/st-3-2"));
    assert_eq!(r1.exit_to, None);
    assert_eq!(r2.exit_to, None);
    let end1 = node(&ir, "issue-to-merge@round1/st-7-4");
    let end2 = node(&ir, "issue-to-merge@round2/st-7-4");
    assert_eq!(end1.exit_to.as_deref(), Some("main/r1-impl"));
    assert_eq!(end2.exit_to.as_deref(), Some("main/r2-impl"));
    let inst = ir
        .instances
        .iter()
        .find(|i| i.id == "issue-to-merge@round2")
        .expect("instance");
    assert_eq!(inst.caller.as_deref(), Some("main/r2-impl"));
    assert_eq!(inst.module.as_deref(), Some("issue-to-merge"));
    assert_eq!(inst.args["child"], "9");

    // モジュールの開始は実体ごとに展開する。置換するのは effects だけで、node は実体の中を指す。
    let s = ir
        .starts
        .iter()
        .find(|s| s.id == "issue-to-merge@round2#s-6-1")
        .expect("module start");
    assert_eq!(s.node, "issue-to-merge@round2/st-6-1");

    // 2 回の compile でバイト列が同じ。
    assert_eq!(compile_dir(&dir).expect("a"), compile_dir(&dir).expect("b"));
}

#[test]
fn one_edit_in_the_module_reaches_both_instances() {
    let before = ir_of(&testdata(PACK));
    let t = copy_pack();
    let p = t.0.join(MODULE_YAML);
    let text = fs::read_to_string(&p).expect("read");
    assert_eq!(text.matches(EDIT_FROM).count(), 1, "edit target is unique");
    fs::write(&p, text.replacen(EDIT_FROM, EDIT_TO, 1)).expect("write");
    let after = ir_of(&t.0);

    let changed: Vec<&str> = after
        .nodes
        .iter()
        .zip(&before.nodes)
        .filter(|(a, b)| a != b)
        .map(|(a, _)| a.uid.as_str())
        .collect();
    assert_eq!(
        changed,
        [
            "issue-to-merge@round1/st-6-1",
            "issue-to-merge@round2/st-6-1"
        ]
    );
    assert_eq!(after.nodes.len(), before.nodes.len());
    assert_eq!(after.instances, before.instances);
    assert_eq!(after.starts, before.starts);
}

fn all_files(root: &Path, dir: &Path, out: &mut Vec<String>) {
    for e in fs::read_dir(dir).expect("read_dir").flatten() {
        let p = e.path();
        if p.is_dir() {
            all_files(root, &p, out);
        } else {
            let rel = p.strip_prefix(root).expect("prefix");
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

#[test]
fn a_third_round_is_added_by_adding_one_file() {
    let before = ir_of(&testdata(PACK));
    let t = copy_pack();
    let add = testdata("module-reuse-rounds/test-data/step5/ch-round3.chapter.yaml");
    t.put(
        "chapters/ch-round3/chapter.yaml",
        &fs::read(&add).expect("read step5"),
    );

    // 既存のファイルは 1 バイトも変えていない（追加だけ）。
    let src = testdata(PACK);
    let mut files = Vec::new();
    all_files(&src, &src, &mut files);
    for f in &files {
        assert_eq!(
            fs::read(src.join(f)).expect("src"),
            fs::read(t.0.join(f)).expect("copy"),
            "{f}"
        );
    }

    let after = ir_of(&t.0);
    assert_eq!(after.nodes.len(), 36 + 2 + 8 + 8);
    assert_eq!(after.instances.len(), 7);
    for n in &before.nodes {
        assert_eq!(node(&after, &n.uid), n, "{}", n.uid);
    }
    assert!(after
        .instances
        .iter()
        .any(|i| i.id == "issue-to-merge@round3"));
    // `requires`・`unmet` は値として通す（評価は TASK-51）。
    let s = after
        .starts
        .iter()
        .find(|s| s.id == "s-r3")
        .expect("s-r3 start");
    assert!(s.requires.is_some() && s.unmet.is_some());
    assert_eq!(s.node, "main/r3-idea");
}

const MOD_OK: &str =
    "id: m\ntitle: M\nparams: {x: d}\nsteps:\n  - {id: a, title: \"T ${x}\", final: true}\n";

fn chapter_using(use_: &str) -> String {
    format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: u}}]\nsteps:\n  - {{id: u, title: U, final: true, use: {use_}}}\n"
    )
}

fn mini(chapter: &str, module: &str) -> Tmp {
    let t = Tmp::new("modneg");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t.put("modules/m/module.yaml", module.as_bytes());
    t
}

#[test]
fn neg_non_string_arg_is_a_parse_error() {
    let t = mini(
        &chapter_using("{module: m, instance: i, args: {x: 5}}"),
        MOD_OK,
    );
    assert_eq!(codes(&diags(&t)), [DiagCode::Parse]);
}

#[test]
fn neg_unknown_use_field_is_a_parse_error() {
    let t = mini(&chapter_using("{module: m, instance: i, extra: 1}"), MOD_OK);
    assert_eq!(codes(&diags(&t)), [DiagCode::Parse]);
}

#[test]
fn neg_undefined_argument_names_the_name_but_not_the_value() {
    let module = MOD_OK.replace("T ${x}", "T ${nope}");
    let t = mini(
        &chapter_using("{module: m, instance: i, args: {x: SECRET-ARG-VALUE}}"),
        &module,
    );
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::Structure]);
    let s = d[0].to_string();
    assert!(s.contains("nope"), "{s}");
    assert!(!s.contains("SECRET-ARG-VALUE"), "{s}");
    assert_eq!(d[0].file(), "modules/m/module.yaml");
}

#[test]
fn neg_unterminated_placeholder_is_a_structure_error() {
    let module = MOD_OK.replace("T ${x}", "T ${x");
    let t = mini(&chapter_using("{module: m, instance: i}"), &module);
    assert_eq!(codes(&diags(&t)), [DiagCode::Structure]);
}

#[test]
fn neg_undefined_module_is_ref_module() {
    let t = mini(&chapter_using("{module: zz, instance: i}"), MOD_OK);
    assert_eq!(codes(&diags(&t)), [DiagCode::RefModule]);
}

#[test]
fn neg_same_instance_twice_is_a_duplicate() {
    let chapter = concat!(
        "id: c1\ntitle: C\nstarts: [{id: s1, node: u}]\nsteps:\n",
        "  - {id: u, title: U, next: [{to: v}], use: {module: m, instance: i}}\n",
        "  - {id: v, title: V, final: true, use: {module: m, instance: i}}\n",
    );
    let t = mini(chapter, MOD_OK);
    assert_eq!(codes(&diags(&t)), [DiagCode::DupId]);
}

#[test]
fn neg_module_and_instance_must_be_identifiers() {
    for bad in [
        "{module: m, instance: \"a@b\"}",
        "{module: m, instance: \"a/b\"}",
        "{module: \"m/x\", instance: i}",
        "{module: m, instance: \"a b\"}",
    ] {
        let t = mini(&chapter_using(bad), MOD_OK);
        assert_eq!(codes(&diags(&t)), [DiagCode::Structure], "{bad}");
    }
}

#[test]
fn neg_self_call_stops_at_the_depth_limit() {
    let module = concat!(
        "id: m\ntitle: M\nsteps:\n",
        "  - {id: a, title: A, final: true, use: {module: m, instance: i}}\n",
    );
    let t = mini(&chapter_using("{module: m, instance: i}"), module);
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::ModuleCycle]);
}

/// `m1` から `m<n>` まで 1 段ずつ呼ぶ連鎖のパック。`m<n>` は呼ばない。
fn chain(n: usize) -> Tmp {
    let t = Tmp::new("modchain");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        chapter_using("{module: m1, instance: i}").as_bytes(),
    );
    for k in 1..=n {
        let call = if k < n {
            format!(", use: {{module: m{}, instance: i}}", k + 1)
        } else {
            String::new()
        };
        t.put(
            &format!("modules/m{k}/module.yaml"),
            format!("id: m{k}\ntitle: M\nsteps:\n  - {{id: a, title: A, final: true{call}}}\n")
                .as_bytes(),
        );
    }
    t
}

#[test]
fn nesting_of_eight_levels_expands_and_nine_is_rejected() {
    let ok = chain(8);
    let ir = ir_of(&ok.0);
    assert_eq!(ir.instances.len(), 9);
    assert!(ir.nodes.iter().any(|n| n.uid.ends_with("m8@i/a")));
    let deepest = ir.instances.last().expect("instance");
    assert_eq!(deepest.id.matches('@').count(), 8);

    let d = diags(&chain(9));
    assert_eq!(codes(&d), [DiagCode::ModuleCycle]);
}

#[test]
fn an_inserted_value_is_not_substituted_again_and_chapters_are_literal() {
    let chapter = concat!(
        "id: c1\ntitle: \"${keep}\"\nstarts: [{id: s1, node: u}]\nsteps:\n",
        "  - {id: u, title: \"${keep}\", final: true, use: {module: m, instance: i, args: {x: \"${y}\"}}}\n",
    );
    let t = mini(chapter, MOD_OK);
    let ir = ir_of(&t.0);
    assert_eq!(node(&ir, "m@i/a").step.title, "T ${y}");
    assert_eq!(node(&ir, "main/u").step.title, "${keep}");
}

#[test]
fn neg_module_start_pointing_outside_the_instance_is_ref_step() {
    let module = format!("{MOD_OK}starts:\n  - {{id: ms, node: nowhere}}\n");
    let t = mini(&chapter_using("{module: m, instance: i}"), &module);
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::RefStep]);
    assert_eq!(d[0].item(), "start:ms");
}

#[test]
fn neg_transition_inside_a_module_is_checked_against_its_own_instance() {
    // 章の Step `a` と同名の Step がモジュールに無くても、モジュールの遷移先は章では解決しない。
    let module = "id: m\ntitle: M\nsteps:\n  - {id: z, title: Z, final: true, next: [{to: u}]}\n";
    let t = mini(&chapter_using("{module: m, instance: i}"), module);
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::RefStep]);
    assert_eq!(d[0].file(), "modules/m/module.yaml");
}

#[test]
fn neg_argument_not_declared_in_params_is_a_structure_error() {
    let t = mini(
        &chapter_using("{module: m, instance: i, args: {typo: SECRET-ARG-VALUE}}"),
        MOD_OK,
    );
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::Structure]);
    let s = d[0].to_string();
    assert!(s.contains("typo"), "{s}");
    assert!(!s.contains("SECRET-ARG-VALUE"), "{s}");
}

#[test]
fn neg_oversized_start_expansion_stops_all_instances_at_once() {
    // 小さな入力が実体ごとに巨大な置換になる。最初の超過で全体を打ち切り、実体の数だけ繰り返さない。
    let refs = vec!["${x}"; 300].join(",");
    let module = format!(
        "id: m\ntitle: M\nparams: {{x: d}}\nsteps:\n  - {{id: a, title: A, final: true}}\nstarts:\n  - {{id: ms, node: a, effects: [{{put: \"{refs}\"}}]}}\n"
    );
    let big = "a".repeat(100_000);
    let mut chapter = String::from("id: c1\ntitle: C\nstarts: [{id: s1, node: u0}]\nsteps:\n");
    for k in 0..6 {
        chapter.push_str(&format!(
            "  - {{id: u{k}, title: U, final: true, use: {{module: m, instance: i{k}, args: {{x: {big}}}}}}}\n"
        ));
    }
    let t = mini(&chapter, &module);
    let d = diags(&t);
    let limits = codes(&d).iter().filter(|c| **c == DiagCode::Limit).count();
    assert_eq!(limits, 1, "{d:?}");
}

#[test]
fn neg_input_emptied_by_an_argument_is_checked_after_expansion() {
    let module = "id: m\ntitle: M\nparams: {x: d}\nsteps:\n  - {id: a, title: T, final: true, input: {kind: command, name: \"${x}\"}, confirm: {kind: approve}}\n";
    let ok = mini(
        &chapter_using("{module: m, instance: i, args: {x: ls}}"),
        module,
    );
    assert!(compile_dir(&ok.0).is_ok());
    let t = mini(
        &chapter_using("{module: m, instance: i, args: {x: \"\"}}"),
        module,
    );
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::Structure]);
    assert_eq!(d[0].file(), "modules/m/module.yaml");
    assert!(d[0].to_string().contains("step:a"), "{}", d[0]);
}

#[test]
fn an_arg_shape_placeholder_in_a_module_input_compiles_after_substitution() {
    // 置換前の `${shape}` は ArgShape として読めない。モジュールの input は展開後だけで検査する。
    let module = "id: m\ntitle: M\nparams: {shape: d}\nsteps:\n  - {id: a, title: T, final: true, input: {kind: command, name: x, args: [{name: n, shape: \"${shape}\"}]}, confirm: {kind: approve}}\n";
    let ok = mini(
        &chapter_using("{module: m, instance: i, args: {shape: digits}}"),
        module,
    );
    let d = compile_dir(&ok.0).err().unwrap_or_default();
    assert!(d.is_empty(), "{d:?}");
    // 展開後に不正な値になる場合は、展開後の検査で E-PARSE になる。
    let bad = mini(
        &chapter_using("{module: m, instance: i, args: {shape: nonsense}}"),
        module,
    );
    assert_eq!(codes(&diags(&bad)), [DiagCode::Parse]);
}

#[test]
fn neg_post_expansion_diagnostics_do_not_carry_the_argument_value() {
    // 展開後の検査（未定義の fixture・未知の input の kind）の診断に、引数の値を写さない。
    let fixture = "id: m\ntitle: M\nparams: {x: d}\nsteps:\n  - {id: a, title: T, final: true, on_enter: [{put: {path: p, fixture: \"${x}\"}}]}\n";
    let kind = "id: m\ntitle: M\nparams: {x: d}\nsteps:\n  - {id: a, title: T, final: true, input: {kind: \"${x}\"}, confirm: {kind: approve}}\n";
    for (module, code, needle) in [
        (fixture, DiagCode::RefFixture, "fixture is undefined"),
        (kind, DiagCode::Parse, "unknown kind"),
    ] {
        let t = mini(
            &chapter_using("{module: m, instance: i, args: {x: shortname}}"),
            module,
        );
        let d = diags(&t);
        assert_eq!(codes(&d), [code]);
        let s = d[0].to_string();
        assert!(s.contains(needle), "{s}");
        assert!(!s.contains("shortname"), "{s}");
    }
}

#[test]
fn exit_to_is_set_only_on_the_final_step_of_an_instance() {
    // a → b（b が final）のモジュールでは、b だけが呼び出し元へ戻る。
    let module = "id: m\ntitle: M\nsteps:\n  - {id: a, title: A, next: [{to: b}]}\n  - {id: b, title: B, final: true}\n";
    let t = mini(&chapter_using("{module: m, instance: i}"), module);
    let ir = ir_of(&t.0);
    assert_eq!(node(&ir, "m@i/a").exit_to, None);
    assert_eq!(node(&ir, "m@i/b").exit_to.as_deref(), Some("main/u"));
}

#[test]
fn neg_substituted_use_diagnostics_do_not_carry_the_argument_value() {
    // モジュールの中の `use` も引数で置換される。置換後のモジュール名・実体名を診断に写さない。
    let undefined = "id: m\ntitle: M\nparams: {x: d}\nsteps:\n  - {id: a, title: A, final: true, use: {module: \"${x}\", instance: i}}\n";
    let empty = "id: m\ntitle: M\nparams: {x: d}\nsteps:\n  - {id: a, title: A, final: true, use: {module: \"e${x}\", instance: i}}\n";
    let dup = "id: m\ntitle: M\nparams: {x: d}\nsteps:\n  - {id: a, title: A, use: {module: e, instance: \"${x}\"}, next: [{to: b}]}\n  - {id: b, title: B, final: true, use: {module: e, instance: \"${x}\"}}\n";
    let target = "id: e\ntitle: E\nsteps:\n  - {id: z, title: Z, final: true}\n";
    let empty_target = "id: eshortname\ntitle: E\nsteps: []\n";
    for (module, extra, code, needle) in [
        (undefined, None, DiagCode::RefModule, "module is undefined"),
        (
            empty,
            Some(("modules/eshortname/module.yaml", empty_target)),
            DiagCode::Structure,
            "module has no step",
        ),
        (
            dup,
            Some(("modules/e/module.yaml", target)),
            DiagCode::DupId,
            "duplicate module instance",
        ),
    ] {
        let t = mini(
            &chapter_using("{module: m, instance: i, args: {x: shortname}}"),
            module,
        );
        if let Some((path, body)) = extra {
            t.put(path, body.as_bytes());
        }
        let d = diags(&t);
        assert_eq!(codes(&d), [code], "{d:?}");
        let s = d[0].to_string();
        assert!(s.contains(needle), "{s}");
        assert!(!s.contains("shortname"), "{s}");
    }
}

#[test]
fn neg_confirm_reason_emptied_by_an_argument_is_no_confirm() {
    // 置換前は理由ありに見える `reason: "${why}"` も、空文字で置換されると理由つきの省略にならない（PACK-13）。
    let module = "id: m\ntitle: M\nparams: {why: d}\nsteps:\n  - {id: a, title: T, final: true, input: {kind: approval}, confirm: {kind: none, reason: \"${why}\"}}\n";
    let t = mini(
        &chapter_using("{module: m, instance: i, args: {why: \"\"}}"),
        module,
    );
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::NoConfirm], "{d:?}");
    assert_eq!(d[0].file(), "modules/m/module.yaml");
    assert!(d[0].to_string().contains("step:a"), "{}", d[0]);
    // 空白だけの理由も同じく拒否する。理由が入れば通る。
    let blank = mini(
        &chapter_using("{module: m, instance: i, args: {why: \"  \"}}"),
        module,
    );
    assert_eq!(codes(&diags(&blank)), [DiagCode::NoConfirm]);
    let ok = mini(
        &chapter_using("{module: m, instance: i, args: {why: tutorial}}"),
        module,
    );
    let d = compile_dir(&ok.0).err().unwrap_or_default();
    assert!(d.is_empty(), "{d:?}");
}

#[test]
fn neg_answer_options_broken_by_an_argument_are_structure_errors() {
    // 置換後に選択肢の id が重なる・label が空になる宣言は、展開後の検査で E-STRUCTURE になる。
    let module = "id: m\ntitle: M\nparams: {o: d, l: d}\nsteps:\n  - {id: a, title: T, final: true, input: {kind: approval}, confirm: {kind: answer, on_wrong: w, options: [{id: x, correct: true}, {id: \"${o}\", correct: false, label: \"${l}\"}]}}\n";
    for args in ["{o: x, l: L}", "{o: y, l: \" \"}"] {
        let t = mini(
            &chapter_using(&format!("{{module: m, instance: i, args: {args}}}")),
            module,
        );
        assert_eq!(codes(&diags(&t)), [DiagCode::Structure], "{args}");
    }
}

/// `pack.profile` を持つ `workshop.yaml` に差し替える。
fn with_profile(t: &Tmp, profile: &str) {
    let w = format!(
        "pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: A, profile: {profile}}}\n"
    );
    t.put("workshop.yaml", w.as_bytes());
}

#[test]
fn neg_input_kind_substituted_to_text_is_checked_against_the_profile() {
    // `kind: "${k}"` が `text` に置換されると、接頭辞が空のパックでは E-PROFILE-TEXT になる。
    let module = "id: m\ntitle: M\nparams: {k: command}\nsteps:\n  - {id: a, title: T, final: true, input: {kind: \"${k}\", min: 1, max: 9}, confirm: {kind: approve}}\n";
    let t = mini(
        &chapter_using("{module: m, instance: i, args: {k: text}}"),
        module,
    );
    with_profile(&t, "{command_prefix: \"\"}");
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::ProfileText], "{d:?}");
    assert_eq!(d[0].file(), "modules/m/module.yaml");
    // 入力欄が無いパックでは、同じ置換が E-PROFILE-NO-INPUT になる。
    with_profile(&t, "{input_field: hidden}");
    assert_eq!(codes(&diags(&t)), [DiagCode::ProfileNoInput]);
}

#[test]
fn a_confirm_kind_placeholder_compiles_after_substitution() {
    // 置換前の `${k}` は確認の kind として読めない。モジュールの confirm は展開後だけで検査する。
    let module = "id: m\ntitle: M\nparams: {k: d}\nsteps:\n  - {id: a, title: T, final: true, input: {kind: approval}, confirm: {kind: \"${k}\"}}\n";
    let ok = mini(
        &chapter_using("{module: m, instance: i, args: {k: approve}}"),
        module,
    );
    let d = compile_dir(&ok.0).err().unwrap_or_default();
    assert!(d.is_empty(), "{d:?}");
    // 展開後に未知の kind になる場合は、展開後の検査で E-PARSE が 1 件だけ出る。
    let bad = mini(
        &chapter_using("{module: m, instance: i, args: {k: nonsense}}"),
        module,
    );
    assert_eq!(codes(&diags(&bad)), [DiagCode::Parse]);
}

#[test]
fn neg_post_expansion_confirm_diagnostics_do_not_carry_the_argument_value() {
    // 展開後の確認の検査の診断に、引数の値を写さない（未知の kind・置換後の理由の欠け）。
    let kind = "id: m\ntitle: M\nparams: {x: d}\nsteps:\n  - {id: a, title: T, final: true, input: {kind: approval}, confirm: {kind: \"${x}\"}}\n";
    let unknown_field = "id: m\ntitle: M\nparams: {x: d}\nsteps:\n  - {id: a, title: T, final: true, input: {kind: approval}, confirm: {kind: approve, note: \"${x}\"}}\n";
    for (module, code, needle) in [
        (kind, DiagCode::Parse, "unknown kind"),
        (unknown_field, DiagCode::Parse, "unknown field note"),
    ] {
        let t = mini(
            &chapter_using("{module: m, instance: i, args: {x: shortname}}"),
            module,
        );
        let d = diags(&t);
        assert_eq!(codes(&d), [code], "{d:?}");
        let s = d[0].to_string();
        assert!(s.contains(needle), "{s}");
        assert!(!s.contains("shortname"), "{s}");
    }
}

#[test]
fn neg_a_module_declaration_without_placeholders_is_reported_once() {
    // 置換記号を含まない宣言は置換で変わらないため、置換前の検査だけが報告する（同じ違反を 2 件にしない）。
    let module = "id: m\ntitle: M\nsteps:\n  - {id: a, title: T, final: true, input: {kind: foo}, confirm: {kind: bar}}\n";
    let t = mini(&chapter_using("{module: m, instance: i}"), module);
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::Parse, DiagCode::Parse], "{d:?}");
}
