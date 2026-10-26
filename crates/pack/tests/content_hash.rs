//! pack が出す IR が `content_hash` の仕様に従うことの結合試験（TASK-180 1/2、VER-12）。
//!
//! 本文の Markdown だけを直して compile し直しても、ハッシュは変わらない（本文の版は `pack.version` で示す）。
//! 期待値は凍結物（`tests/data/ir_before_ep13/fixture-pack-choice.ir.json`。0fa7146 のエンジンで作った）を Python で独立に計算して固定した。
//! 一時ディレクトリは `CARGO_TARGET_TMPDIR` の下に作り、作業ツリーは書き換えない。試験では `unwrap` を使ってよい。

mod common;

use std::path::{Path, PathBuf};

use common::{copy_dir, Tmp};
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_pack::compile::compile_dir;

/// 凍結物を本文の木を除いて compact の JSON にし、Python で求めた SHA-256。
const FIXTURE_PACK_HASH: &str = "06a13c377e7e8d6af319b59f0b2a4099c44478af9c5bcba0fb844d9c6cf8dcca";

fn fixture_pack() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/fixture-pack")
}

fn compiled(dir: &Path) -> Ir {
    serde_json::from_slice(&compile_dir(dir).expect("compiles")).unwrap()
}

#[test]
fn compiled_ir_hash_matches_golden_and_fixed_expectation() {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/ir_before_ep13/fixture-pack-choice.ir.json");
    let from_golden: Ir = serde_json::from_slice(&std::fs::read(golden).unwrap()).unwrap();
    let h = content_hash(&compiled(&fixture_pack())).unwrap();
    assert_eq!(h, content_hash(&from_golden).unwrap());
    assert_eq!(h.as_str(), FIXTURE_PACK_HASH);
}

#[test]
fn editing_only_a_markdown_body_keeps_the_hash() {
    let t = Tmp::new("content-hash");
    copy_dir(&fixture_pack(), &t.0);
    let before = compiled(&t.0);
    t.put(
        "chapters/c1/a.md",
        "# 直した見出し\n\n本文を書き直した。\n".as_bytes(),
    );
    let after = compiled(&t.0);
    assert_ne!(before, after, "the bodies must really differ");
    assert_eq!(
        content_hash(&before).unwrap(),
        content_hash(&after).unwrap()
    );
}
