//! 拡張点の追加の手続きの文書の形の検査（TASK-189、EXT-8、D-53）。
//!
//! 正は `docs/extension-point-procedure.md` で、この試験は形だけを見る。版の欄・5 要素の節の順・
//! 「任意のコードは書けない」「実行時の登録の仕組みは作らない」の明記・引用したパスと実装の名前の実在を、
//! 文書が古くならないように確かめる。実行時の登録の仕組みは作らないため（D-53）、その試験は無い。

use std::fs;
use std::path::{Path, PathBuf};

use fandhe_workshop_core::history_open::VOCAB_MAX;
use fandhe_workshop_pack::diag::DiagCode;
use fandhe_workshop_pack::vocab::{EXTENSION_POINTS, LATEST};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn doc() -> String {
    fs::read_to_string(root().join("docs/extension-point-procedure.md"))
        .expect("手続きの文書を読めること")
}

#[test]
fn procedure_has_version_block() {
    let doc = doc();
    let version = doc
        .lines()
        .find_map(|l| l.strip_prefix("手続きの文書の版: "))
        .expect("版の欄があること");
    let n: u32 = version.trim().parse().expect("版が整数であること");
    assert!(n >= 1);
    assert!(doc.lines().any(|l| l == "状態: 固定（契約）"));
}

#[test]
fn procedure_has_five_elements() {
    let doc = doc();
    let headings: Vec<&str> = doc.lines().filter(|l| l.starts_with("## ")).collect();
    let expected = [
        "## 3. 汎用であることの説明",
        "## 4. 語彙の文書の固定",
        "## 5. 診断コードの追加",
        "## 6. 公開の一覧への追記",
        "## 7. 既存の教材パックが同じ IR になることの確認",
    ];
    let mut last = None;
    for h in expected {
        let pos = headings
            .iter()
            .position(|x| *x == h)
            .unwrap_or_else(|| panic!("見出しが無い: {h}"));
        if let Some(prev) = last {
            assert!(pos > prev, "見出しの順が違う: {h}");
        }
        last = Some(pos);
    }
}

#[test]
fn procedure_states_no_arbitrary_code() {
    let doc = doc();
    assert!(doc.contains("教材パックから任意のコードは書けない"));
    assert!(doc.contains("実行時に拡張点を登録する仕組みは作らない"));
}

#[test]
fn procedure_paths_exist() {
    let doc = doc();
    let root = root();
    let mut checked = 0;
    for part in doc.split('`').skip(1).step_by(2) {
        let is_path = ["crates/", "docs/", "tools/"]
            .iter()
            .any(|p| part.starts_with(p));
        if !is_path || part.starts_with("docs/spec/") || part.contains(char::is_whitespace) {
            continue;
        }
        assert!(root.join(part).exists(), "実在しないパス: {part}");
        checked += 1;
    }
    assert!(checked > 10, "パスの引用が少なすぎる: {checked}");
}

#[test]
fn procedure_names_match_code() {
    let doc = doc();
    // 参照が通ることで、文書の挙げる名前が実装に在ることをコンパイル時に縛る。
    let _ = (LATEST, VOCAB_MAX, EXTENSION_POINTS.len());
    let _ = std::mem::size_of::<DiagCode>();
    for name in ["EXTENSION_POINTS", "LATEST", "VOCAB_MAX", "DiagCode"] {
        assert!(doc.contains(name), "文書に名前が無い: {name}");
    }
}
