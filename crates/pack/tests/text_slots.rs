//! 拒否・問い返しの文面の差し込みの枠と次の行動の検査の受け入れ試験（PACK-12、PACK-16、TASK-46 2/2）。
//!
//! 架空の教材 `tests/data/slot-pack` を一時ディレクトリへ複製し、文字列を置き換えて欠陥を埋める（PoC の D15〜D19 の方式）。
//! 欠陥ごとに `validate` と `compile` が同じコード・ファイル・項目を 1 件だけ報告すること、診断の文面に本文・代替の文の
//! 値が出ないことを確かめる。公開の試験データと crate の試験データのすべてで、新しい診断が 0 件で、差し込みの後に次の行動が
//! 空になる組が 0 件であることも確かめる（静的・動的）。
//!
//! 確かめないこと: `fandhe-pack test` の実行と、core の判定の結果から文面を組み立てて差し込む実行系（TASK-404・TASK-267）。
//! 動的な数え上げは、IR を総なめにする純関数 `tally_empty_next_actions` と、値の組を作って差し込む
//! `rendered_next_action` の水準で確かめる。

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::{assert_no_secret, copy_dir, testdata, Tmp, SECRET};

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::Category;
use fandhe_workshop_contract::slot::{SlotName, SlotValues};
use fandhe_workshop_core::command_lex::lex_command;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::play_expect::{
    read_error_input_expects, resolve_rejection, ResultExpect,
};
use fandhe_workshop_pack::slot_check::{
    empty_next_action_diagnostics, rendered_next_action, tally_empty_next_actions, TextKind,
    TextRef,
};
use fandhe_workshop_pack::source;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::vocab::TERMS;
use fandhe_workshop_pack::{DiagCode, Diagnostic};
use serde_json::Value;

const WORKSHOP_FILE: &str = "workshop.yaml";
const CHAPTER_FILE: &str = "chapters/c1/chapter.yaml";

fn slot_pack_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/slot-pack")
}

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

/// 架空の教材を一時ディレクトリへ複製する。
fn copy() -> Tmp {
    let t = Tmp::new("slots");
    copy_dir(&slot_pack_dir(), &t.0);
    t
}

/// `rel` の中の `from`（ちょうど 1 回現れること）を `to` に置き換える。
fn replace(t: &Tmp, rel: &str, from: &str, to: &str) {
    replace_n(t, rel, from, to, 1);
}

fn replace_n(t: &Tmp, rel: &str, from: &str, to: &str, n: usize) {
    let path = t.0.join(rel);
    let text = std::fs::read_to_string(&path).expect("read");
    assert_eq!(text.matches(from).count(), n, "occurrences of {from:?}");
    t.put(rel, text.replace(from, to).as_bytes());
}

/// `validate` と `compile` の診断。どちらも失敗する欠陥を想定する。
fn both(t: &Tmp) -> (Vec<Diagnostic>, Vec<Diagnostic>) {
    (
        validate_dir(&t.0),
        compile_dir(&t.0).expect_err("compile must fail"),
    )
}

/// 診断が 1 件で、期待のコード・ファイル・項目であること。
fn only(d: &[Diagnostic], code: DiagCode, file: &str, item: &str) {
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code(), code, "{d:?}");
    assert_eq!(d[0].file(), file, "{d:?}");
    assert_eq!(d[0].item(), item, "{d:?}");
}

/// `validate` と `compile` が同じ 1 件を報告すること。
fn both_only(t: &Tmp, code: DiagCode, file: &str, item: &str) {
    let (v, c) = both(t);
    only(&v, code, file, item);
    only(&c, code, file, item);
    assert_no_secret(&v);
    assert_no_secret(&c);
}

#[test]
fn pack16_base_pack_has_no_diagnostics() {
    let t = copy();
    assert!(validate_dir(&t.0).is_empty());
    compile_dir(&t.0).expect("compile");
}

#[test]
fn pack16_unknown_slot() {
    let t = copy();
    replace(
        &t,
        WORKSHOP_FILE,
        "{{nearest_command|/help}}",
        "{{nearest|/help}}",
    );
    both_only(
        &t,
        DiagCode::SlotUnknown,
        WORKSHOP_FILE,
        "rejection:rj-unknown-command",
    );
}

#[test]
fn pack16_unknown_slot_name_is_not_echoed_unless_safe() {
    let t = copy();
    replace(
        &t,
        "workshop.yaml",
        "{{nearest_command|/help}}",
        &format!("{{{{{SECRET} x}}}}"),
    );
    both_only(
        &t,
        DiagCode::SlotUnknown,
        WORKSHOP_FILE,
        "rejection:rj-unknown-command",
    );
}

#[test]
fn pack16_context_rejection_and_clarification() {
    let t = copy();
    replace(
        &t,
        WORKSHOP_FILE,
        "今の Step では、{{step_do}}。",
        "今の Step では、{{step_do}}。残り {{remaining}}。",
    );
    both_only(
        &t,
        DiagCode::SlotContext,
        WORKSHOP_FILE,
        "rejection:rj-wrong-order",
    );

    let t = copy();
    replace(
        &t,
        WORKSHOP_FILE,
        "{{step_do}}。値を足して",
        "{{nearest_command|x}}。値を足して",
    );
    both_only(
        &t,
        DiagCode::SlotContext,
        WORKSHOP_FILE,
        "clarification:cl-ask",
    );
}

/// 分類を読めない拒否は `E-PARSE` だけにし、問い返しと取り違えて分類の制限（`E-SLOT-CONTEXT`）を重ねない。
#[test]
fn pack16_unreadable_category_does_not_add_slot_context() {
    let t = copy();
    replace(
        &t,
        WORKSHOP_FILE,
        "category: \"wrong_order\"",
        "category: \"typo\"",
    );
    replace(
        &t,
        WORKSHOP_FILE,
        "今の Step では、{{step_do}}。",
        "今の Step では、{{step_do}}。残り {{remaining}}。",
    );
    let (v, c) = both(&t);
    for d in [&v, &c] {
        assert!(d.iter().all(|x| x.code() != DiagCode::SlotContext), "{d:?}");
    }
    // 分類の語の検査は compile の側が `E-PARSE` にする。
    assert!(c.iter().any(|x| x.code() == DiagCode::Parse), "{c:?}");
}

#[test]
fn pack16_no_fallback() {
    for to in ["{{nearest_command}}", "{{nearest_command| }}"] {
        let t = copy();
        replace(&t, WORKSHOP_FILE, "{{nearest_command|/help}}", to);
        both_only(
            &t,
            DiagCode::SlotNoFallback,
            WORKSHOP_FILE,
            "rejection:rj-unknown-command",
        );
    }
    let t = copy();
    replace(
        &t,
        WORKSHOP_FILE,
        "{{input_head|入力した語}}",
        "{{input_head| }}",
    );
    both_only(
        &t,
        DiagCode::SlotNoFallback,
        WORKSHOP_FILE,
        "rejection:rj-unknown-command",
    );
}

#[test]
fn pack16_needs_do() {
    // `do` を持たない入力つきの Step で、`step_do` を使う文面（wrong_order の既定・問い返し）が出る。
    let t = copy();
    replace(
        &t,
        CHAPTER_FILE,
        "    do: \"/help を送ってコマンドの一覧を開く\"\n",
        "",
    );
    both_only(&t, DiagCode::SlotNeedsDo, CHAPTER_FILE, "step:a");

    // 空白だけの `do` も無いものとして扱う。
    let t = copy();
    replace(
        &t,
        CHAPTER_FILE,
        "do: \"/help を送ってコマンドの一覧を開く\"",
        "do: \"  \"",
    );
    both_only(&t, DiagCode::SlotNeedsDo, CHAPTER_FILE, "step:a");
}

#[test]
fn pack16_needs_do_has_no_false_positive() {
    // 入力を持たない最後の Step は、文面が出ないので `do` が無くてもよい。
    let t = copy();
    assert!(validate_dir(&t.0).is_empty());

    // `step_do` を使う文面がどこからも出ない教材では、`do` が無くても報告しない。
    let t = copy();
    replace_n(&t, WORKSHOP_FILE, "{{step_do}}", "{{step_title}}", 2);
    replace(
        &t,
        CHAPTER_FILE,
        "    do: \"/help を送ってコマンドの一覧を開く\"\n",
        "",
    );
    replace(
        &t,
        CHAPTER_FILE,
        "    do: \"ここまでの感想を自分の言葉で書く\"\n",
        "",
    );
    assert!(validate_dir(&t.0).is_empty());
    compile_dir(&t.0).expect("compile");
}

#[test]
fn pack16_syntax() {
    let t = copy();
    replace(
        &t,
        WORKSHOP_FILE,
        "「{{step_title}}」を処理",
        "「{{step_title」を処理",
    );
    // 同じ欄に他の枠の診断を重ねない。
    both_only(&t, DiagCode::SlotSyntax, WORKSHOP_FILE, "rejection:rj-busy");

    for bad in ["{{ }}", "{{|x}}", "{{a|{{b}}"] {
        let t = copy();
        replace(
            &t,
            WORKSHOP_FILE,
            "「{{step_title}}」を処理",
            &format!("{bad}を処理"),
        );
        both_only(&t, DiagCode::SlotSyntax, WORKSHOP_FILE, "rejection:rj-busy");
    }
}

#[test]
fn pack12_static_empty_next_action() {
    for to in ["\"\"", "\"   \""] {
        let t = copy();
        replace(
            &t,
            WORKSHOP_FILE,
            "next_action: \"教材の操作に戻ります。\"",
            &format!("next_action: {to}"),
        );
        both_only(
            &t,
            DiagCode::EmptyNextAction,
            WORKSHOP_FILE,
            "rejection:rj-not-supported",
        );
    }
    // 問い返しの次の行動の欄は how_to_answer。
    let t = copy();
    replace(
        &t,
        WORKSHOP_FILE,
        "how_to_answer: \"{{step_do}}。値を足して、もう一度送ります。\"",
        "how_to_answer: \"\"",
    );
    both_only(
        &t,
        DiagCode::EmptyNextAction,
        WORKSHOP_FILE,
        "clarification:cl-ask",
    );
}

#[test]
fn pack12_blank_fallback_is_no_fallback_not_empty() {
    // 空になりうる枠だけの次の行動で、代替の文が空白だけ。枠の誤りとして 1 件だけ報告し、空の診断を重ねない。
    let t = copy();
    replace(
        &t,
        WORKSHOP_FILE,
        "next_action: \"「{{nearest_command|/help}}」に直して、もう一度送ります。\"",
        "next_action: \"{{input_head| }}\"",
    );
    both_only(
        &t,
        DiagCode::SlotNoFallback,
        WORKSHOP_FILE,
        "rejection:rj-unknown-command",
    );
}

#[test]
fn pack12_missing_or_non_string_field_is_parse() {
    let t = copy();
    replace(
        &t,
        WORKSHOP_FILE,
        "    next_action: \"自分の文を、自分で読み返します。\"\n",
        "",
    );
    both_only(
        &t,
        DiagCode::Parse,
        WORKSHOP_FILE,
        "rejection:rj-free-text-note",
    );

    let t = copy();
    replace(
        &t,
        WORKSHOP_FILE,
        "next_action: \"自分の文を、自分で読み返します。\"",
        "next_action: 5",
    );
    both_only(
        &t,
        DiagCode::Parse,
        WORKSHOP_FILE,
        "rejection:rj-free-text-note",
    );

    let t = copy();
    replace(
        &t,
        WORKSHOP_FILE,
        "    how_to_answer: \"{{step_do}}。値を足して、もう一度送ります。\"\n",
        "    next_action: \"{{step_do}}。値を足して、もう一度送ります。\"\n",
    );
    both_only(&t, DiagCode::Parse, WORKSHOP_FILE, "clarification:cl-ask");
}

#[test]
fn pack16_secret_text_is_not_echoed() {
    let t = copy();
    replace(
        &t,
        WORKSHOP_FILE,
        "message: \"引数の形が合っていません。\"",
        &format!("message: \"{SECRET} {{{{{SECRET}\""),
    );
    both_only(
        &t,
        DiagCode::SlotSyntax,
        WORKSHOP_FILE,
        "rejection:rj-bad-arg-shape",
    );
}

/// 公開の試験データと crate の試験データのすべて。第 2 要素は、差し込みの対象の文面を持つ（`checked > 0` を要する）か。
fn all_packs() -> Vec<(String, PathBuf, bool)> {
    let mut v: Vec<(String, PathBuf, bool)> = vec![
        (
            "module-reuse-rounds".into(),
            testdata("module-reuse-rounds/test-data/pack"),
            true,
        ),
        (
            "ch1-pack".into(),
            testdata("input-field-ime/test-data/ch1-pack"),
            true,
        ),
        (
            "ime-pack".into(),
            testdata("input-field-ime/test-data/ime-pack"),
            true,
        ),
        (
            "min-3step-pack".into(),
            testdata("engine-event-consistency/test-data/min-3step-pack"),
            true,
        ),
    ];
    for name in [
        "other-pack",
        "v1.0.0",
        "v1.0.0-vocab-1.0",
        "v1.1.0",
        "v1.2.0",
        "v2.0.0",
    ] {
        v.push((
            format!("save-restore-versions/{name}"),
            testdata(&format!("save-restore-versions/test-data/packs/{name}")),
            true,
        ));
    }
    for name in [
        "slot-pack",
        "judgment-pack",
        "min-3step-pack-b",
        "confirm-kinds-pack",
        "confirm-label-pack",
        "fixture-pack",
        "input-kinds-pack",
    ] {
        v.push((name.to_string(), data(name), false));
    }
    v
}

fn ir_of(dir: &Path) -> Ir {
    let bytes = compile_dir(dir).unwrap_or_else(|d| panic!("compile {dir:?}: {d:?}"));
    serde_json::from_slice(&bytes).expect("ir")
}

fn is_slot_diag(d: &Diagnostic) -> bool {
    matches!(
        d.code(),
        DiagCode::SlotSyntax
            | DiagCode::SlotUnknown
            | DiagCode::SlotContext
            | DiagCode::SlotNeedsDo
            | DiagCode::SlotNoFallback
            | DiagCode::EmptyNextAction
    )
}

#[test]
fn pack12_dynamic_zero_on_public_packs() {
    for (name, dir, must_check) in all_packs() {
        let diags: Vec<_> = validate_dir(&dir)
            .into_iter()
            .filter(is_slot_diag)
            .collect();
        assert!(diags.is_empty(), "{name}: {diags:?}");
        let ir = ir_of(&dir);
        let tally = tally_empty_next_actions(&ir);
        assert_eq!(tally.empty, 0, "{name}: {:?}", tally.findings);
        assert!(tally.findings.is_empty());
        assert!(empty_next_action_diagnostics(&ir, "workshop.yaml").is_empty());
        if must_check {
            assert!(tally.checked > 0, "{name}: nothing was checked");
        }
    }
}

const MODULE_WORKSHOP: &str = "\
pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}
commands: {help: builtin}
rejections:
    rj-wo: {category: wrong_order, message: m, next_action: '{{step_do}}'}
defaults: {wrong_order: rj-wo}
";

const MODULE_CHAPTER: &str = "\
id: c1
title: C
starts: [{id: s1, node: a}]
steps:
    - {id: a, title: A, body: a.md, next: [{to: b}], use: {module: m1, instance: i1, args: {what: ARG}}}
    - {id: b, title: B, final: true}
";

const MODULE_MODULE: &str = "\
id: m1
title: M
params: {what: x}
steps:
    -   id: m-a
        title: MA
        body: a.md
        do: '${what}'
        input: {kind: command, name: help}
        confirm: {kind: none, reason: r}
        next: [{to: m-b}]
    - {id: m-b, title: MB, final: true}
";

fn module_pack(arg: &str) -> Tmp {
    let t = Tmp::new("slots-mod");
    t.put("workshop.yaml", MODULE_WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        MODULE_CHAPTER.replace("ARG", arg).as_bytes(),
    );
    t.put("chapters/c1/a.md", b"# a\n");
    t.put("modules/m1/module.yaml", MODULE_MODULE.as_bytes());
    t.put("modules/m1/a.md", b"# a\n");
    t
}

#[test]
fn pack12_dynamic_detects_after_expansion() {
    // 引数が空でなければ、静的も動的も 0 件。
    let ok = module_pack("'go'");
    assert!(validate_dir(&ok.0).iter().all(|d| !is_slot_diag(d)));
    let tally = tally_empty_next_actions(&ir_of(&ok.0));
    assert_eq!((tally.checked, tally.empty), (1, 0));

    // 引数が空だと、置換の後の `do` が空になる。置換前の `do` は空でないので展開の前の検査は通るが、
    // 展開の後の検査が `E-SLOT-NEEDS-DO` にする（モジュールを定義したファイルの Step）。
    let t = module_pack("''");
    both_only(
        &t,
        DiagCode::SlotNeedsDo,
        "modules/m1/module.yaml",
        "step:m-a",
    );

    // 動的な数え上げは、`do` が無くなった展開後の node を空として数える。
    let mut ir = ir_of(&ok.0);
    for node in ir.nodes.iter_mut().filter(|n| n.uid == "m1@i1/m-a") {
        node.step.do_ = None;
    }
    let tally = tally_empty_next_actions(&ir);
    assert_eq!((tally.checked, tally.empty), (1, 1), "{tally:?}");
    assert_eq!(tally.findings[0].0, "m1@i1/m-a");
    let d = empty_next_action_diagnostics(&ir, WORKSHOP_FILE);
    only(
        &d,
        DiagCode::EmptyNextAction,
        WORKSHOP_FILE,
        "rejection:rj-wo",
    );
}

#[test]
fn pack12_blank_step_title_makes_title_only_next_action_empty() {
    // 次の行動が `{{step_title}}` だけの文面は、題名が空・空白の Step では空になる（静的検査の仮の題名では空にならない）。
    let build = |arg: &str| {
        let t = module_pack(arg);
        t.put(
            "workshop.yaml",
            MODULE_WORKSHOP
                .replace("'{{step_do}}'", "'{{step_title}}'")
                .as_bytes(),
        );
        t.put(
            "modules/m1/module.yaml",
            MODULE_MODULE
                .replace("title: MA", "title: '${what}'")
                .replace("do: '${what}'", "do: x")
                .as_bytes(),
        );
        t
    };
    let ok = build("'go'");
    assert!(validate_dir(&ok.0).iter().all(|d| !is_slot_diag(d)));
    for arg in ["''", "'   '"] {
        both_only(
            &build(arg),
            DiagCode::EmptyNextAction,
            "modules/m1/module.yaml",
            "step:m-a",
        );
    }
}

#[test]
fn pack16_needs_do_after_argument_substitution() {
    // モジュールの Step が `on_reject` を引数で受ける。置換の前は文面の ID が読めず、`do` も無いので展開の前の検査は
    // 飛ばすが、引数に `step_do` を使う文面を渡すと置換の後の Step が `do` を欠く。
    let t = Tmp::new("slots-arg");
    t.put("workshop.yaml", MODULE_WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        MODULE_CHAPTER.replace("what: ARG", "rj: rj-wo").as_bytes(),
    );
    t.put("chapters/c1/a.md", b"# a\n");
    t.put(
        "modules/m1/module.yaml",
        MODULE_MODULE
            .replace("params: {what: x}", "params: {rj: x}")
            .replace(
                "        do: '${what}'\n",
                "        on_reject: {wrong_order: '${rj}'}\n",
            )
            .as_bytes(),
    );
    t.put("modules/m1/a.md", b"# a\n");
    both_only(
        &t,
        DiagCode::SlotNeedsDo,
        "modules/m1/module.yaml",
        "step:m-a",
    );
}

#[test]
fn pack16_needs_do_for_start_unmet() {
    // 開始点の `unmet` が `step_do` を使う文面を指し、開始先の Step に `do` が無い（入力が無いので既定の文面は出ない）。
    let t = Tmp::new("slots-start");
    t.put("workshop.yaml", MODULE_WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: [{id: s1, node: a, requires: {flag: f}, unmet: rj-wo}]\nsteps:\n    - {id: a, title: A, body: a.md, next: [{to: b}]}\n    - {id: b, title: B, final: true}\n",
    );
    t.put("chapters/c1/a.md", b"# a\n");
    both_only(&t, DiagCode::SlotNeedsDo, CHAPTER_FILE, "step:a");
}

#[test]
fn pack12_dynamic_counts_unrenderable_text_as_empty() {
    // 描画できない組（次の行動の欄が無い・文字列でない・文面が IR に無い）は、飛ばさず空として数える（fail-closed）。
    let ok = module_pack("'go'");
    for edit in ["missing", "non-string", "absent"] {
        let mut ir = ir_of(&ok.0);
        match edit {
            "missing" => {
                ir.rejections
                    .get_mut("rj-wo")
                    .and_then(Value::as_object_mut)
                    .expect("rejection")
                    .remove("next_action");
            }
            "non-string" => {
                ir.rejections
                    .get_mut("rj-wo")
                    .and_then(Value::as_object_mut)
                    .expect("rejection")
                    .insert("next_action".to_string(), Value::from(1));
            }
            _ => {
                ir.rejections.remove("rj-wo");
            }
        }
        let tally = tally_empty_next_actions(&ir);
        assert_eq!((tally.checked, tally.empty), (1, 1), "{edit}: {tally:?}");
        let d = empty_next_action_diagnostics(&ir, WORKSHOP_FILE);
        assert_eq!(d.len(), 1, "{edit}: {d:?}");
        assert_eq!(d[0].code(), DiagCode::EmptyNextAction, "{edit}");
    }
}

/// 誤入力の `start`・`cmd`・`text` を引く。
fn error_input(pack: &Path, id: &str) -> (String, Option<String>) {
    let lp = source::load(pack).expect("load");
    let tests = lp.tests.expect("tests");
    let v: &Value = tests
        .error_inputs
        .iter()
        .find(|v| v.get("id").and_then(Value::as_str) == Some(id))
        .expect("error input");
    let start = v["start"].as_str().expect("start").to_string();
    let raw = v["input"]
        .get("cmd")
        .or_else(|| v["input"].get("text"))
        .and_then(Value::as_str)
        .map(str::to_string);
    (start, raw)
}

fn values_for(ir: &Ir, start: &str, head: Option<String>, busy: bool) -> SlotValues {
    let s = ir.starts.iter().find(|s| s.id == start).expect("start");
    let n = ir.nodes.iter().find(|n| n.uid == s.node).expect("node");
    SlotValues {
        input_head: head,
        nearest_command: None,
        step_title: n.step.title.clone(),
        step_do: n.step.do_.clone(),
        remaining: busy.then_some(1),
        total: busy.then_some(1),
    }
}

#[test]
fn pack12_error_inputs_rendered() {
    let mut rendered = BTreeMap::new();
    for name in ["slot-pack", "judgment-pack"] {
        let dir = data(name);
        let ir = ir_of(&dir);
        let lp = source::load(&dir).expect("load");
        for (id, expect) in read_error_input_expects(&lp).expect("expects") {
            let ResultExpect::Rejected(category) = expect else {
                continue;
            };
            let (start, raw) = error_input(&dir, &id);
            let (text_id, _) = resolve_rejection(&ir, &start, category).expect("resolve");
            let text = TextRef {
                kind: TextKind::Rejection,
                id: text_id.to_string(),
            };
            let busy = category.as_str() == Category::Busy.as_str();
            let lexed = raw
                .as_deref()
                .and_then(|r| lex_command("/", r).ok())
                .map(|l| l.name);
            for head in [None, lexed] {
                let values = values_for(&ir, &start, head, busy);
                let out = rendered_next_action(&ir, &text, &values)
                    .unwrap_or_else(|| panic!("{name}/{id}: cannot render"));
                assert!(!out.trim().is_empty(), "{name}/{id}: empty next action");
                rendered.insert(format!("{name}/{id}"), out);
            }
        }
    }
    // 入力の先頭の語が無くても、代替の文で次の行動が残る。
    assert!(rendered["slot-pack/e-unknown"].contains("/help"));
    assert!(rendered["slot-pack/e-busy"].contains("あと 1 回"));
}

#[test]
fn pack12_inserted_input_head_is_not_expanded() {
    let ir = ir_of(&slot_pack_dir());
    let text = TextRef {
        kind: TextKind::Clarification,
        id: "cl-ask".to_string(),
    };
    let mut values = values_for(&ir, "s1", Some("{{step_title}}".into()), false);
    values.step_do = Some("{{input_head}}".into());
    let out = rendered_next_action(&ir, &text, &values).expect("render");
    assert!(out.starts_with("{{input_head}}"), "{out}");
}

#[test]
fn pack16_slot_names_match_the_vocabulary() {
    let contract: std::collections::BTreeSet<&str> =
        SlotName::ALL.iter().map(|n| n.as_str()).collect();
    let vocab: std::collections::BTreeSet<&str> = TERMS
        .iter()
        .filter(|t| t.kind == "slot")
        .map(|t| t.term)
        .collect();
    assert_eq!(contract, vocab);
}
