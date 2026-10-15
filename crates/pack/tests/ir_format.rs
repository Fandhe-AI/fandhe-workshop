//! `ir_format` 2 の出力のバイト列の固定（04-behavior の PACK-24、TASK-41・TASK-60）。
//!
//! `tests/data/golden/<名前>.ir.json` は、compile できる 5 パックの `compile_dir` の出力（版 2。本文と
//! 練習データの木を持つ）である。将来の任意の欄の追加が既定値を誤って出力すると、この試験が落ちる。
//! `tests/data/ir_format_1/<名前>.ir.json` は、TASK-60 の前に出力していた版 1 の golden を凍結したもので、
//! 木の欄を除いて版を 1 に戻した出力が、これとバイト一致する（版 1 の部分が変わっていないことの確認）。
//! 本文も練習データも持たない教材パックは、`ir_format` 1 のまま、バイト単位で変わらない。
//!
//! 版 2 の golden を作り直したのは、D-51 による版上げ（契約の変更）である。以後の書き換えも、
//! `ir_format` の版上げの判断が要る（spec を先に直す）。試験は
//! 作業ツリーを書き換えない。再生成の手順は、`cargo build --locked -p fandhe-workshop-pack` の後に、
//! `target/debug/fandhe-pack compile <パックのディレクトリ>` の標準出力を golden のファイルに保存する。
//! 一時ディレクトリは `CARGO_TARGET_TMPDIR` の下に作る。

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{minimal, testdata, Tmp, WORKSHOP};
use fandhe_workshop_contract::ir::{to_canonical_json, Ir};
use fandhe_workshop_pack::compile::compile_dir;

fn golden(name: &str) -> Vec<u8> {
    golden_in("golden", name)
}

fn golden_in(dir: &str, name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(dir)
        .join(format!("{name}.ir.json"));
    fs::read(&p).unwrap_or_else(|_| panic!("cannot read golden {dir}/{name}"))
}

fn packs() -> Vec<(&'static str, PathBuf)> {
    vec![
        (
            "min-3step-pack",
            testdata("engine-event-consistency/test-data/min-3step-pack"),
        ),
        ("ch1-pack", testdata("input-field-ime/test-data/ch1-pack")),
        ("ime-pack", testdata("input-field-ime/test-data/ime-pack")),
        (
            "min-3step-pack-b",
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/min-3step-pack-b"),
        ),
        (
            "fixture-pack",
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/fixture-pack"),
        ),
    ]
}

/// 最初に違うバイトの位置（長さが違うだけなら短い方の長さ）。
fn first_diff(a: &[u8], b: &[u8]) -> usize {
    (0..a.len().min(b.len()))
        .find(|&i| a[i] != b[i])
        .unwrap_or(a.len().min(b.len()))
}

#[test]
fn ir2_golden_bytes_match() {
    for (name, dir) in packs() {
        let got = compile_dir(&dir).unwrap_or_else(|_| panic!("{name} must compile"));
        let want = golden(name);
        assert!(
            got == want,
            "{name}: IR differs from golden at byte {} (got {} bytes, golden {} bytes)",
            first_diff(&got, &want),
            got.len(),
            want.len()
        );
    }
}

/// 木の欄を除き、版を 1 に戻すと、凍結した版 1 の golden とバイト一致する（`fixture-pack` は版 1 に無い）。
#[test]
fn ir2_without_trees_matches_frozen_v1_golden() {
    for (name, dir) in packs().into_iter().filter(|(n, _)| *n != "fixture-pack") {
        let bytes = compile_dir(&dir).unwrap_or_else(|_| panic!("{name} must compile"));
        let mut ir: Ir = serde_json::from_slice(&bytes).expect("IR parses");
        assert_eq!(ir.format_version, 2, "{name}");
        ir.format_version = 1;
        ir.pack.ir_format = 1;
        ir.fixture_docs.clear();
        for n in &mut ir.nodes {
            n.body_doc = None;
            n.body_levels_doc.clear();
        }
        let got = to_canonical_json(&ir).expect("serialize");
        let want = golden_in("ir_format_1", name);
        assert!(
            got == want,
            "{name}: v1 part differs at byte {}",
            first_diff(&got, &want)
        );
    }
}

/// 本文も練習データも持たないパックは、`ir_format` 1 のまま、版 1 と同じバイト列になる。
#[test]
fn ir1_pack_without_trees_stays_version_one() {
    let t = Tmp::new("ir-v1");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    let chapter = concat!(
        "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n",
        "  - {id: a, title: A, next: [{to: b}]}\n",
        "  - {id: b, title: B, final: true}\n",
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    let bytes = compile_dir(&t.0).expect("compiles");
    let ir: Ir = serde_json::from_slice(&bytes).expect("IR parses");
    assert_eq!((ir.format_version, ir.pack.ir_format), (1, 1));
    assert!(to_canonical_json(&ir).expect("serialize") == bytes);
    let text = String::from_utf8(bytes).expect("utf8");
    assert!(!text.contains("body_doc") && !text.contains("fixture_docs"));
}

/// `workshop.*` が `ir_format: 2` と宣言しても、木を使わなければ最小の版（1）で出す（D-52）。
#[test]
fn ir_declared_two_without_trees_is_emitted_as_one() {
    let t = Tmp::new("ir-v2-declared");
    t.put(
        "workshop.yaml",
        WORKSHOP.replace("ir_format: 1", "ir_format: 2").as_bytes(),
    );
    let chapter = concat!(
        "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n",
        "  - {id: a, title: A, next: [{to: b}]}\n",
        "  - {id: b, title: B, final: true}\n",
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    let ir: Ir = serde_json::from_slice(&compile_dir(&t.0).expect("compiles")).expect("parses");
    assert_eq!((ir.format_version, ir.pack.ir_format), (1, 1));
}

#[test]
fn ir2_golden_has_canonical_shape() {
    for (name, _) in packs() {
        let g = golden(name);
        assert!(g.ends_with(b"}\n") && !g.ends_with(b"\n\n"), "{name}");
        assert!(!g.windows(4).any(|w| w == b"null"), "{name}: null in IR");
    }
}

/// 既定値を明示した書き方は、省略した書き方と同じバイト列になる。
#[test]
fn ir3_explicit_defaults_match_omitted() {
    let base = minimal();
    let omitted = compile_dir(&base.0).expect("omitted");

    let t = Tmp::new("ir-defaults");
    t.put(
        "workshop.yaml",
        format!(
            "{WORKSHOP}{}",
            concat!(
                "refs: []\ncommands: {}\nviews: {}\nfixtures: {}\nhints: {}\n",
                "rejections: {}\nclarifications: {}\ndefaults: {}\n"
            )
        )
        .as_bytes(),
    );
    let chapter = concat!(
        "id: c1\ntitle: C\nstarts: [{id: s1, node: a, effects: []}]\nsteps:\n",
        "  - {id: a, title: A, body: a.md, final: false, lo: [], views: [], fixtures: [], hints: [],\n",
        "     body_levels: {}, on_reject: {}, on_enter: [], processing: [], rules: [], on_achieved: [],\n",
        "     next: [{to: b, when: null}]}\n",
        "  - {id: b, title: B, final: true, next: []}\n",
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    let body = fs::read(base.0.join("chapters/c1/a.md")).expect("body");
    t.put("chapters/c1/a.md", &body);
    let explicit = compile_dir(&t.0).expect("explicit");
    assert!(
        explicit == omitted,
        "explicit defaults differ at byte {}",
        first_diff(&explicit, &omitted)
    );
}
