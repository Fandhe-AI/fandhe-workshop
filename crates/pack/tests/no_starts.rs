//! 開始点（`starts`）が 0 件の教材パックの拒否の試験（PACK-25、D-43、TASK-57 2/2）。
//!
//! 章が 1 つ以上あって、章の `starts` の合計が 0 件のとき、`validate` の検査の段階で `E-STRUCTURE` として拒否する。
//! 開始点が無いと教材は始められず、到達性の検査も根が無いので判定できない（`E-UNREACHABLE` を連鎖させない）。
//! モジュールの `starts` は章の開始点に数えない。`fandhe-pack validate` の結線は TASK-401 が受け持つため、ここでは
//! ライブラリの経路（`validate_dir`・`compile_dir`）を使う。

mod common;

use common::{Tmp, CHAPTER, WORKSHOP};

use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const NO_START_CHAPTER: &str = concat!(
    "id: c1\ntitle: C\nsteps:\n",
    "  - {id: a, title: A, body: a.md, next: [{to: b}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-a, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}\n",
    "  - {id: b, title: B, final: true}\n",
);

fn pack(workshop: &str, chapter: &str) -> Tmp {
    let t = Tmp::new("nostarts");
    t.put("workshop.yaml", workshop.as_bytes());
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    t
}

fn assert_no_start(d: &[Diagnostic], what: &str) {
    assert!(!d.is_empty(), "{what}");
    assert!(
        d.iter().any(|x| x.code() == DiagCode::Structure
            && x.file() == "chapters"
            && x.message().contains("no start is defined")),
        "{what}: {d:?}"
    );
    assert!(
        d.iter().all(|x| x.code() != DiagCode::Unreachable),
        "{what}: unreachable cascaded: {d:?}"
    );
}

#[test]
fn pack25_premise_pack_with_a_start_is_valid() {
    let t = pack(WORKSHOP, CHAPTER);
    assert!(validate_dir(&t.0).is_empty());
}

#[test]
fn pack25_expect_missing_and_empty_starts_are_rejected() {
    let t = pack(WORKSHOP, NO_START_CHAPTER);
    assert_no_start(&validate_dir(&t.0), "no starts field");
    let empty = NO_START_CHAPTER.replace("title: C\n", "title: C\nstarts: []\n");
    let t = pack(WORKSHOP, &empty);
    assert_no_start(&validate_dir(&t.0), "starts: []");
}

#[test]
fn pack25_expect_module_starts_do_not_count() {
    let t = pack(WORKSHOP, NO_START_CHAPTER);
    t.put(
        "modules/m1/module.yaml",
        b"id: m1\ntitle: M\nstarts: [{id: ms, node: ma}]\nsteps:\n  - {id: ma, title: MA, final: true}\n",
    );
    assert_no_start(&validate_dir(&t.0), "module only");
}

#[test]
fn pack25_expect_one_chapter_with_a_start_is_enough() {
    let t = pack(WORKSHOP, NO_START_CHAPTER);
    t.put(
        "chapters/c2/chapter.yaml",
        CHAPTER
            .replace("id: c1", "id: c2")
            .replace("s1", "s2")
            .as_bytes(),
    );
    t.put("chapters/c2/a.md", b"# a\n");
    let d = validate_dir(&t.0);
    assert!(
        d.iter()
            .all(|x| !x.message().contains("no start is defined")),
        "{d:?}"
    );
    // 両方の章に開始点が無ければ拒否する。
    let t = pack(WORKSHOP, NO_START_CHAPTER);
    t.put(
        "chapters/c2/chapter.yaml",
        NO_START_CHAPTER.replace("id: c1", "id: c2").as_bytes(),
    );
    t.put("chapters/c2/a.md", b"# a\n");
    assert_no_start(&validate_dir(&t.0), "two chapters");
}

#[test]
fn pack25_expect_toml_format_is_rejected_too() {
    let t = Tmp::new("nostarts-toml");
    t.put(
        "workshop.toml",
        b"[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n",
    );
    t.put(
        "chapters/c1/chapter.toml",
        b"id = \"c1\"\ntitle = \"C\"\n\n[[steps]]\nid = \"a\"\ntitle = \"A\"\nfinal = true\n",
    );
    assert_no_start(&validate_dir(&t.0), "toml");
}

#[test]
fn pack25_expect_compile_rejects_without_ir() {
    let t = pack(WORKSHOP, NO_START_CHAPTER);
    let d = compile_dir(&t.0).expect_err("rejected");
    assert_no_start(&d, "compile");
}
