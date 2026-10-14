//! エンジン側の IR の読み込み（`load_ir`）が、IR のバイト列だけを入力にすることの試験（PACK-1）。
//!
//! 1 つ目は、試作が作った golden の IR（旧い形で `null` を含む）を `load_ir` が受けること。
//! 2 つ目は、core のソースがファイルシステムの機能を使っていないことの機械的な確認で、
//! 実行時のエンジンが構造定義や Markdown を読む道を作らないための守りである。
//! データは `testdata/` から読み、一時ファイルは作らない。

use std::fs;
use std::path::{Path, PathBuf};

use fandhe_workshop_core::load_ir;

fn testdata(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(rel)
}

#[test]
fn pack1_golden_irs_load() {
    for rel in [
        "engine-event-consistency/test-data/min-3step.json",
        "input-field-ime/test-data/ch1-pack.json",
        "input-field-ime/test-data/ime-pack.json",
    ] {
        let bytes = fs::read(testdata(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let ir = load_ir(&bytes).unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert!(!ir.nodes.is_empty(), "{rel}");
        assert!(!ir.starts.is_empty(), "{rel}");
    }
}

/// `.rs` を再帰的に集める。
fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).expect("read src dir").flatten() {
        let p = e.path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn pack1_core_source_has_no_filesystem_access() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rs_files(&src, &mut files);
    assert!(!files.is_empty());
    let banned = [
        "std::fs",
        "std::path",
        "std::env",
        "std::process",
        "File::",
        "fs::",
    ];
    for f in files {
        let text = fs::read_to_string(&f).expect("read source");
        for (i, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for b in banned {
                assert!(
                    !line.contains(b),
                    "{}:{}: core must not use `{b}`",
                    f.display(),
                    i + 1
                );
            }
        }
    }
}
