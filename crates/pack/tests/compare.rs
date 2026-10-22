//! `compare` とバイト一致の結合試験（04-behavior の PACK-6）。
//!
//! 同じ内容の教材パックは、書き方が違っても（形式 A と形式 B、同じ形式の中の別表記）、また同じパックを
//! 何回コンパイルしても、IR がバイト単位で一致することを、`compare_dirs` で確かめる。違う内容のパックは
//! `Differ` になり、報告する位置が実際に最初に違うバイトを指すことも確かめる。
//! 形式 B の組は `tests/data/min-3step-pack-b/`（`testdata/` の形式 A 版と同じ内容）を使う。
//! 形式 A を製品が読むかどうか（PACK-7）が検討中のため、PACK-7 で形式 B だけを読むと決まったら
//! `pack6_format_a_and_format_b_match` を直す。一時ディレクトリは `CARGO_TARGET_TMPDIR` の下に作る。

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{minimal, reversed_copy, testdata, Tmp, CHAPTER, SECRET, WORKSHOP};
use fandhe_workshop_pack::cli::{run, EXIT_DIAGNOSTICS};
use fandhe_workshop_pack::compare::{compare_dirs, Outcome};
use fandhe_workshop_pack::compile::compile_dir;

fn min3_a() -> PathBuf {
    testdata("engine-event-consistency/test-data/min-3step-pack")
}

fn min3_b() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/min-3step-pack-b")
}

/// `compile_dir` が通る testdata の 3 パックと、形式 B 版。
fn compilable_packs() -> Vec<PathBuf> {
    vec![
        min3_a(),
        testdata("input-field-ime/test-data/ch1-pack"),
        testdata("input-field-ime/test-data/ime-pack"),
        min3_b(),
    ]
}

fn assert_same(a: &Path, b: &Path) {
    match compare_dirs(a, b) {
        Ok(Outcome::Same { len }) => assert!(len > 0),
        other => panic!("expected Same, got {other:?}"),
    }
    // 判定の根拠が同じバイト列であることを、`compare_dirs` を介さず直接も確かめる。
    assert_eq!(compile_dir(a).expect("a"), compile_dir(b).expect("b"));
}

/// 最初に違うバイトの位置と行を、`compare` とは別に数える。
fn first_diff(a: &[u8], b: &[u8]) -> (usize, usize) {
    let offset = (0..a.len().min(b.len()))
        .find(|&i| a[i] != b[i])
        .unwrap_or(a.len().min(b.len()));
    let line = 1 + a[..offset].iter().filter(|&&c| c == b'\n').count();
    (offset, line)
}

fn assert_differ(a: &Path, b: &Path) {
    let (x, y) = (compile_dir(a).expect("a"), compile_dir(b).expect("b"));
    assert_ne!(x, y);
    let (offset, line) = first_diff(&x, &y);
    assert_eq!(
        compare_dirs(a, b).expect("compare"),
        Outcome::Differ { offset, line }
    );
}

fn tmp_copy(from: &Path, tag: &str) -> Tmp {
    let t = Tmp::new(tag);
    common::copy_dir(from, &t.0);
    t
}

fn rewrite(t: &Tmp, rel: &str, from: &str, to: &str) {
    let p = t.0.join(rel);
    let text = fs::read_to_string(&p).expect("read");
    assert!(text.contains(from), "{rel} lacks {from:?}");
    fs::write(p, text.replacen(from, to, 1)).expect("write");
}

#[test]
fn pack6_format_a_and_format_b_match() {
    assert_same(&min3_a(), &min3_b());
}

#[test]
fn pack6_format_b_rewritten_notation_matches() {
    let base = minimal();
    let alt = minimal();
    // キーの順・block と flow・コメントと空行・引用符・末尾の改行の有無を変える。
    alt.put(
        "workshop.yaml",
        b"# comment\n\npack:\n  guidance: \"A\"\n  ir_format: 1\n  version: \"0\"\n  title: t\n  id: 'p'",
    );
    alt.put(
        "chapters/c1/chapter.yaml",
        concat!(
            "steps:\n",
            "  - title: A\n    body: 'a.md'\n    next:\n      - to: b\n    id: a\n    input: {kind: choice, options: [{id: ok, label: OK}]}\n    rules: [{id: r-a, on: {option: ok}, outcome: {kind: accept}}]\n    confirm: {kind: none, reason: r}\n\n",
            "  # the last step\n",
            "  - {final: true, title: \"B\", id: b}\n",
            "starts:\n  - node: a\n    id: s1\n",
            "title: C\n",
            "id: c1\n",
        )
        .as_bytes(),
    );
    assert_same(&base.0, &alt.0);
}

#[test]
fn pack6_format_a_rewritten_notation_matches() {
    let alt = tmp_copy(&min3_a(), "toml");
    rewrite(
        &alt,
        "workshop.toml",
        "[views.v-files]\nkind = \"file_tree\"",
        "[views]\nv-files.kind = 'file_tree'",
    );
    rewrite(
        &alt,
        "workshop.toml",
        "level = 1\ntext = \"ノートを新しく作る場面です。\"",
        "text = \"ノートを新しく作る場面です。\"\nlevel = 1",
    );
    rewrite(
        &alt,
        "chapters/ch-min/chapter.toml",
        "id = \"ch-min\"\ntitle = \"ノートを作って公開する（最小）\"",
        "title = \"ノートを作って公開する（最小）\"\n\nid = 'ch-min'",
    );
    assert_same(&min3_a(), &alt.0);
}

#[test]
fn pack6_key_order_inside_free_form_values_does_not_matter() {
    // `input`・`rules[].outcome` のように型を付けずに通す欄の中のキーの順。
    let alt = tmp_copy(&min3_b(), "free");
    let ch = "chapters/ch-min/chapter.yaml";
    rewrite(
        &alt,
        ch,
        "{name: \"name\", positional: true, required: true, shape: \"kebab_case\"}",
        "{shape: \"kebab_case\", required: true, positional: true, name: \"name\"}",
    );
    rewrite(
        &alt,
        ch,
        "attrs: {state: \"created\", kind: \"none\"}",
        "attrs: {kind: \"none\", state: \"created\"}",
    );
    rewrite(
        &alt,
        ch,
        "set_attr: {path: \"notes/draft\", attr: \"state\", value: \"writing\"}",
        "set_attr: {value: \"writing\", attr: \"state\", path: \"notes/draft\"}",
    );
    assert_same(&min3_b(), &alt.0);
}

#[test]
fn pack6_repeated_compiles_match() {
    for (i, pack) in compilable_packs().iter().enumerate() {
        assert_same(pack, pack);
        let copy = tmp_copy(pack, &format!("copy{i}"));
        assert_same(pack, &copy.0);
        let rev = reversed_copy(pack, &format!("rev{i}"));
        assert_same(pack, &rev.0);
        assert_same(&copy.0, &rev.0);
    }
}

#[test]
fn pack6_different_content_differs_at_the_first_differing_byte() {
    let hint = tmp_copy(&min3_b(), "hint");
    rewrite(
        &hint,
        "workshop.yaml",
        "使うコマンドは make-note です。",
        "使うコマンドは publish-note です。",
    );
    assert_differ(&min3_b(), &hint.0);

    let base = minimal();
    let swapped = minimal();
    swapped.put(
        "chapters/c1/chapter.yaml",
        concat!(
            "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n",
            "  - {id: b, title: B, final: true}\n",
            "  - {id: a, title: A, body: a.md, next: [{to: b}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-a, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}\n",
        )
        .as_bytes(),
    );
    assert_differ(&base.0, &swapped.0);

    assert_differ(
        &testdata("input-field-ime/test-data/ch1-pack"),
        &testdata("input-field-ime/test-data/ime-pack"),
    );
}

#[test]
fn pack6_reports_do_not_leak_content_or_paths() {
    let a = minimal();
    let b = minimal();
    b.put(
        "chapters/c1/chapter.yaml",
        CHAPTER
            .replace("title: A", &format!("title: {SECRET}"))
            .as_bytes(),
    );
    let mut out = Vec::new();
    let mut err = Vec::new();
    let args = [a.0.clone().into_os_string(), b.0.clone().into_os_string()];
    let argv = std::iter::once("compare".into()).chain(args);
    let code = run(argv, &mut out, &mut err).expect("run");
    assert_eq!(code, EXIT_DIAGNOSTICS);
    let text = String::from_utf8(out).expect("utf8");
    assert!(text.starts_with("NG: IR が一致しません（"), "{text}");
    assert!(!text.contains(SECRET), "{text}");

    // 診断の出力にも、本文・絶対パスは出ない。
    b.put("chapters/c1/a.md", &[0xff]);
    b.put("workshop.yaml", format!("{WORKSHOP}bogus: 1\n").as_bytes());
    let mut out = Vec::new();
    let args = [a.0.clone().into_os_string(), b.0.clone().into_os_string()];
    let argv = std::iter::once("compare".into()).chain(args);
    let code = run(argv, &mut out, &mut Vec::new()).expect("run");
    assert_eq!(code, EXIT_DIAGNOSTICS);
    let text = String::from_utf8(out).expect("utf8");
    assert!(!text.is_empty());
    assert!(
        !text.contains("OK:") && !text.contains("一致しま"),
        "{text}"
    );
    assert!(!text.contains(SECRET), "{text}");
    assert!(!text.contains(a.0.to_str().expect("utf8 path")), "{text}");
    assert!(!text.contains(b.0.to_str().expect("utf8 path")), "{text}");
}
