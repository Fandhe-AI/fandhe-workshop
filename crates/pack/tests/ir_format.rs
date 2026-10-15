//! `ir_format` 1 の出力のバイト列の固定（04-behavior の PACK-24、TASK-41）。
//!
//! `tests/data/golden/<名前>.ir.json` は、compile できる 4 パックの `compile_dir` の出力である。
//! 版 1 の契約の固定なので、将来の任意の欄の追加が既定値を誤って出力すると、この試験が落ちる。
//! 新しい項目を使わない教材パックは、バイト単位で同じ IR になる。
//!
//! golden の書き換えは契約の変更であり、`ir_format` の版上げの判断が要る（spec を先に直す）。試験は
//! 作業ツリーを書き換えない。再生成の手順は、`cargo build --locked -p fandhe-workshop-pack` の後に、
//! `target/debug/fandhe-pack compile <パックのディレクトリ>` の標準出力を golden のファイルに保存する。
//! 一時ディレクトリは `CARGO_TARGET_TMPDIR` の下に作る。

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{minimal, testdata, Tmp, WORKSHOP};
use fandhe_workshop_pack::compile::compile_dir;

fn golden(name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/golden")
        .join(format!("{name}.ir.json"));
    fs::read(&p).unwrap_or_else(|_| panic!("cannot read golden {name}"))
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
    ]
}

/// 最初に違うバイトの位置（長さが違うだけなら短い方の長さ）。
fn first_diff(a: &[u8], b: &[u8]) -> usize {
    (0..a.len().min(b.len()))
        .find(|&i| a[i] != b[i])
        .unwrap_or(a.len().min(b.len()))
}

#[test]
fn ir1_golden_bytes_match() {
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
