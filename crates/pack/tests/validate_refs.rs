//! `validate` の参照の検査（TASK-54 1/2）の受け入れ試験。
//!
//! 参照 8 種（`E-REF-STEP`・`E-REF-VIEW`・`E-REF-FIXTURE`・`E-REF-HINT`・`E-REF-REJECTION`・`E-REF-MODULE`・
//! `E-REF-COMMAND`・`E-REF-BODY`）のそれぞれについて、参照の欠けを 1 つだけ入れた教材パックが、期待のコードで
//! 期待のファイル・項目に報告されることを確かめる。正しい教材パックでは診断が 0 件であることも確かめる。
//!
//! 範囲外: CLI の `validate` への接続（TASK-401・TASK-403）、欠陥パック 35 種（TASK-56）。

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{
    assert_no_secret, copy_dir, testdata, without_answer_label_required, without_expanded_do_leak,
    Tmp, SECRET,
};

use fandhe_workshop_pack::source::load;
use fandhe_workshop_pack::validate::{validate, validate_dir};
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const WORKSHOP_FILE: &str = "workshop.yaml";
const CHAPTER_FILE: &str = "chapters/c1/chapter.yaml";
const MODULE_FILE: &str = "modules/m1/module.yaml";

/// 参照の置き場ごとに別の ID を使い、書き換えが 1 か所だけに効くようにする。
const WORKSHOP: &str = "\
pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}
commands: {help: {description: d}}
views: {v-step: {kind: files}, v-input: {kind: files}, v-rule: {kind: files}, v-conf: {kind: files}}
fixtures:
    fx-step: {path: fixtures/a.md, kind: doc}
    fx-start: {path: fixtures/a.md, kind: doc}
    fx-rule: {path: fixtures/a.md, kind: doc}
hints: {h-a: {text: x}}
rejections: {rj-reject: {category: not_in_this_step, message: m, next_action: n}, rj-miss: {category: not_in_this_step, message: m, next_action: n}, rj-start: {category: not_in_this_step, message: m, next_action: n}, rj-default: {category: not_in_this_step, message: m, next_action: n}, rj-rule: {category: not_in_this_step, message: m, next_action: n}}
clarifications: {cl-a: {message: m, how_to_answer: n}}
defaults: {unknown: rj-default}
";

const CHAPTER: &str = "\
id: c1
title: C
starts: [{id: s1, node: a, requires: {flag: f}, unmet: rj-start, effects: [{put: {path: x, fixture: fx-start}}]}]
steps:
-   id: a
    title: A
    body: a.md
    views: [v-step]
    fixtures: [fx-step]
    hints: [h-a]
    on_reject: {unknown: rj-reject}
    input: {kind: command, name: help}
    rules:
        -   id: r1
            on: {ui: {view: v-rule, path: x}}
            outcome: {kind: clarify, clarification: cl-a}
        -   id: r2
            on: {command: help}
            outcome: {kind: reject, rejection: rj-rule, effects: [{put: {path: x, fixture: fx-rule}}]}
    confirm: {kind: point_at, targets: [{view: v-conf, path: x}], on_miss: rj-miss}
    next: [{to: u}]
-   id: u
    title: U
    use: {module: m1, instance: i1, args: {}}
    next: [{to: z}]
-   {id: z, title: Z, final: true}
";

const MODULE: &str = "\
id: m1
title: M
steps:
    - {id: m-a, title: MA, next: [{to: m-b}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-m-a, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}
    - {id: m-b, title: MB, final: true}
";

/// 参照を一通り持つ正しいパック（形式 B）。
fn base() -> Tmp {
    let t = Tmp::new("refs");
    t.put(WORKSHOP_FILE, WORKSHOP.as_bytes());
    t.put(CHAPTER_FILE, CHAPTER.as_bytes());
    t.put(MODULE_FILE, MODULE.as_bytes());
    t.put("chapters/c1/a.md", format!("# a\n{SECRET}\n").as_bytes());
    t.put(
        "fixtures/a.md",
        format!("# 【架空】f\n{SECRET}\n").as_bytes(),
    );
    t
}

/// `file` の `from`（ちょうど 1 か所）を `to` に書き換える。
fn mutate(t: &Tmp, file: &str, from: &str, to: &str) {
    let path = t.0.join(file);
    let text = fs::read_to_string(&path).expect("read");
    assert_eq!(text.matches(from).count(), 1, "`{from}` must occur once");
    fs::write(path, text.replacen(from, to, 1)).expect("write");
}

fn only(d: &[Diagnostic], code: DiagCode, file: &str, item: &str) {
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code(), code, "{d:?}");
    assert_eq!(d[0].file(), file, "{d:?}");
    assert_eq!(d[0].item(), item, "{d:?}");
    assert_no_secret(d);
}

#[test]
fn premise_base_pack_has_no_diagnostics() {
    let t = base();
    let d = validate_dir(&t.0);
    assert!(d.is_empty(), "{d:?}");
}

/// `(名前, 書き換える file, from, to, 期待のコード, 期待の file, 期待の item)`。
#[allow(
    clippy::type_complexity,
    reason = "試験の表を 1 行 1 件のタプルで持つため"
)]
fn cases() -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    DiagCode,
    &'static str,
    &'static str,
)> {
    use DiagCode::*;
    vec![
        (
            "next-to",
            CHAPTER_FILE,
            "to: u}",
            "to: gone}",
            RefStep,
            CHAPTER_FILE,
            "step:a",
        ),
        (
            "start-node",
            CHAPTER_FILE,
            "node: a,",
            "node: gone,",
            RefStep,
            CHAPTER_FILE,
            "start:s1",
        ),
        (
            "module-next",
            MODULE_FILE,
            "to: m-b}",
            "to: m-gone}",
            RefStep,
            MODULE_FILE,
            "step:m-a",
        ),
        (
            "step-views",
            CHAPTER_FILE,
            "views: [v-step]",
            "views: [gone]",
            RefView,
            CHAPTER_FILE,
            "step:a",
        ),
        (
            "rule-ui-view",
            CHAPTER_FILE,
            "view: v-rule,",
            "view: gone,",
            RefView,
            CHAPTER_FILE,
            "step:a",
        ),
        (
            "confirm-target-view",
            CHAPTER_FILE,
            "view: v-conf,",
            "view: gone,",
            RefView,
            CHAPTER_FILE,
            "step:a",
        ),
        (
            "step-fixtures",
            CHAPTER_FILE,
            "fixtures: [fx-step]",
            "fixtures: [gone]",
            RefFixture,
            CHAPTER_FILE,
            "step:a",
        ),
        (
            "rule-effect-fixture",
            CHAPTER_FILE,
            "fixture: fx-rule",
            "fixture: gone",
            RefFixture,
            CHAPTER_FILE,
            "step:a",
        ),
        (
            "start-effect-fixture",
            CHAPTER_FILE,
            "fixture: fx-start",
            "fixture: gone",
            RefFixture,
            CHAPTER_FILE,
            "start:s1",
        ),
        (
            "fixture-decl-path",
            WORKSHOP_FILE,
            "fx-step: {path: fixtures/a.md",
            "fx-step: {path: fixtures/gone.md",
            RefFixture,
            WORKSHOP_FILE,
            "fixture:fx-step",
        ),
        (
            "hints",
            CHAPTER_FILE,
            "hints: [h-a]",
            "hints: [gone]",
            RefHint,
            CHAPTER_FILE,
            "step:a",
        ),
        (
            "on-reject",
            CHAPTER_FILE,
            "unknown: rj-reject",
            "unknown: gone",
            RefRejection,
            CHAPTER_FILE,
            "step:a",
        ),
        (
            "confirm-on-miss",
            CHAPTER_FILE,
            "on_miss: rj-miss",
            "on_miss: gone",
            RefRejection,
            CHAPTER_FILE,
            "step:a",
        ),
        (
            "rule-rejection",
            CHAPTER_FILE,
            "rejection: rj-rule",
            "rejection: gone",
            RefRejection,
            CHAPTER_FILE,
            "step:a",
        ),
        (
            "rule-clarification",
            CHAPTER_FILE,
            "clarification: cl-a",
            "clarification: gone",
            RefRejection,
            CHAPTER_FILE,
            "step:a",
        ),
        (
            "start-unmet",
            CHAPTER_FILE,
            "unmet: rj-start",
            "unmet: gone",
            RefRejection,
            CHAPTER_FILE,
            "start:s1",
        ),
        (
            "defaults",
            WORKSHOP_FILE,
            "unknown: rj-default}",
            "unknown: gone}",
            RefRejection,
            WORKSHOP_FILE,
            "defaults:unknown",
        ),
        (
            "rule-on-command",
            CHAPTER_FILE,
            "command: help}",
            "command: gone}",
            RefCommand,
            CHAPTER_FILE,
            "step:a",
        ),
        (
            "input-name",
            CHAPTER_FILE,
            "name: help",
            "name: gone",
            RefCommand,
            CHAPTER_FILE,
            "step:a",
        ),
        (
            "use-module",
            CHAPTER_FILE,
            "module: m1",
            "module: gone",
            RefModule,
            CHAPTER_FILE,
            "step:u",
        ),
        (
            "body",
            CHAPTER_FILE,
            "body: a.md",
            "body: gone.md",
            RefBody,
            CHAPTER_FILE,
            "step:a",
        ),
    ]
}

#[test]
fn each_missing_reference_is_reported_once_at_the_right_place() {
    for (name, file, from, to, code, rfile, ritem) in cases() {
        let t = base();
        mutate(&t, file, from, to);
        let d = validate_dir(&t.0);
        assert_eq!(d.len(), 1, "{name}: {d:?}");
        only(&d, code, rfile, ritem);
    }
}

#[test]
fn all_eight_codes_are_covered() {
    let codes: std::collections::BTreeSet<String> =
        cases().iter().map(|c| c.4.to_string()).collect();
    for want in [
        "E-REF-STEP",
        "E-REF-VIEW",
        "E-REF-FIXTURE",
        "E-REF-HINT",
        "E-REF-REJECTION",
        "E-REF-MODULE",
        "E-REF-COMMAND",
        "E-REF-BODY",
    ] {
        assert!(codes.contains(want), "{want} not covered: {codes:?}");
    }
}

#[test]
fn non_string_reference_is_reported() {
    let t = base();
    // `input` の宣言を壊すと入力方式の検査（E-PARSE）も重なるため、`rules` の参照を壊す。
    mutate(&t, CHAPTER_FILE, "on: {command: help}", "on: {command: 7}");
    let d = validate_dir(&t.0);
    only(&d, DiagCode::RefCommand, CHAPTER_FILE, "step:a");
    assert!(d[0].message().contains("string ID"), "{d:?}");
}

#[test]
fn unsafe_id_is_not_echoed_in_the_message() {
    let t = base();
    mutate(&t, CHAPTER_FILE, "hints: [h-a]", "hints: [\"a b\\nc\"]");
    let d = validate_dir(&t.0);
    only(&d, DiagCode::RefHint, CHAPTER_FILE, "step:a");
    assert!(d[0].message().contains("id not shown"), "{d:?}");
}

#[test]
fn load_failure_is_returned_without_running_ref_checks() {
    let t = base();
    // 構文エラーと参照の欠けが同居しても、読み込みの診断だけを返す。
    mutate(&t, CHAPTER_FILE, "hints: [h-a]", "hints: [gone]");
    t.put("workshop.toml", b"[pack]\n");
    let d = validate_dir(&t.0);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code(), DiagCode::Parse);
}

#[test]
fn copied_correct_pack_has_no_diagnostics() {
    let t = Tmp::new("refs-toml");
    copy_dir(&testdata("input-field-ime/test-data/ch1-pack"), &t.0);
    // spec の写しは編集できず、`label` の必須化（PACK-37・D-67）の前の形のため、その診断だけを除く。
    let (rest, removed) = without_answer_label_required(validate_dir(&t.0));
    assert!(rest.is_empty(), "{rest:?}");
    assert!(removed > 0, "除外が空振りしていない");
}

/// 教材パックの根（`workshop.yaml`・`workshop.toml` を持つディレクトリ）を再帰的に集める。
fn pack_roots(dir: &Path, out: &mut Vec<PathBuf>) {
    if dir.join("workshop.yaml").is_file() || dir.join("workshop.toml").is_file() {
        out.push(dir.to_path_buf());
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut subs: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    subs.sort();
    for s in subs {
        pack_roots(&s, out);
    }
}

/// `pack-defects/` の下で、土台（`base/`）以外のディレクトリか。
fn is_defect_overlay(root: &Path) -> bool {
    let names: Vec<_> = root.components().map(|c| c.as_os_str()).collect();
    match names.iter().position(|n| *n == "pack-defects") {
        Some(i) => names.get(i + 1).is_some_and(|n| *n != "base"),
        None => false,
    }
}

#[test]
fn correct_testdata_packs_have_no_false_positives() {
    let mut roots = Vec::new();
    pack_roots(&testdata(""), &mut roots);
    pack_roots(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data"),
        &mut roots,
    );
    let mut checked = 0;
    let mut excluded = 0;
    let mut leak_excluded = 0;
    for root in roots {
        // 欠陥パックの差し替えファイルの置き場（`pack-defects/v1*/<形式>/D*`）は、パックの断片であり
        // 完全なパックではない（章が無い・欠陥を意図して含む）。完全な土台（`base/`）は対象に残す。
        if is_defect_overlay(&root) {
            continue;
        }
        // `v1.0.0-vocab-1.0` は、`1.0` を宣言して `requires`（EP-9）を使う PACK-31 の欠陥の例であり、
        // `E-VOCAB-MISMATCH` になるのが正しい（`vocab_declaration.rs` が確かめる）。
        if root.ends_with("packs/v1.0.0-vocab-1.0") {
            continue;
        }
        // 読み込みに失敗するパック（欠陥を含む試験データ等）は、参照の検査の対象外にする。
        let Ok(pack) = load(&root) else { continue };
        checked += 1;
        // spec の写しと `fill-pack` は `label` の必須化（PACK-37・D-67）の前の形で編集できないため、その診断だけを除く。
        let (d, removed) = without_answer_label_required(validate(&pack));
        excluded += removed;
        // 案内レベルの章が既定値の skill 名を出す写し（R-5。PACK-52）の診断だけを除く。
        let (d, removed) = without_expanded_do_leak(d);
        leak_excluded += removed;
        assert!(d.is_empty(), "{}: {d:?}", root.display());
    }
    assert!(excluded > 0, "除外が空振りしていない");
    assert!(
        leak_excluded > 0,
        "置換後の do の漏れの除外が空振りしていない"
    );
    assert!(checked >= 3, "too few packs checked: {checked}");
}
