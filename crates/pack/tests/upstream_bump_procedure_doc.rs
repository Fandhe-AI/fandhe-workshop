//! 参照元の版を上げる手順書の形の検査（TASK-182 2/2、VER-15）。
//!
//! 正は `docs/upstream-bump-procedure.md` で、この試験は形だけを見る。版の欄・節の順・受け入れの固定の文・
//! 道具の終了コードの記述・引用したパスの実在を、文書が古くならないように確かめる。意味の正しさは人が読んで確かめる。

use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn doc() -> String {
    fs::read_to_string(root().join("docs/upstream-bump-procedure.md")).expect("手順書を読めること")
}

#[test]
fn procedure_has_version_block() {
    let doc = doc();
    let version = doc
        .lines()
        .find_map(|l| l.strip_prefix("手順書の版: "))
        .expect("版の欄があること");
    let n: u32 = version.trim().parse().expect("版が整数であること");
    assert!(n >= 1);
    assert!(doc.lines().any(|l| l == "状態: 固定（契約）"));
}

#[test]
fn procedure_has_sections_in_order() {
    let doc = doc();
    let headings: Vec<&str> = doc.lines().filter(|l| l.starts_with("## ")).collect();
    let expected = [
        "## 4. 影響する Step の洗い出し",
        "## 5. 教材の更新と教材テスト",
        "## 6. 公開前の人の確認と記録",
        "## 7. AI の生成物の扱い",
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
fn procedure_states_acceptance_rules() {
    let doc = doc();
    for s in [
        "教材テストを通す",
        "公開前に人が対応表を確かめて記録を残す",
        "生成物を無検証で公開しない",
        "D-78",
        "11 場面",
        "終了コード 2 を合格として扱わない",
    ] {
        assert!(doc.contains(s), "固定の文が無い: {s}");
    }
}

#[test]
fn procedure_paths_exist() {
    let doc = doc();
    let root = root();
    let mut checked = 0;
    let mut has_tool = false;
    for part in doc.split('`').skip(1).step_by(2) {
        let is_path = ["crates/", "docs/", "tools/"]
            .iter()
            .any(|p| part.starts_with(p));
        if !is_path || part.starts_with("docs/spec/") || part.contains(char::is_whitespace) {
            continue;
        }
        assert!(root.join(part).exists(), "実在しないパス: {part}");
        has_tool |= part == "tools/upstream-impact.mjs";
        checked += 1;
    }
    assert!(checked >= 1, "パスの引用が無い");
    assert!(has_tool, "洗い出しの道具のパスが無い");
}

#[test]
fn procedure_tool_exit_codes_match() {
    let doc = doc();
    // 道具側の正は `tools/upstream-impact.mjs` の先頭コメント。ここでは文書の表の行の字面だけを見る。
    for (code, word) in [
        ("0", "列挙できた"),
        ("1", "教材に無い Step"),
        ("2", "入力の形が不正"),
    ] {
        let prefix = format!("| {code} |");
        assert!(
            doc.lines()
                .any(|l| l.starts_with(&prefix) && l.contains(word)),
            "終了コード {code} の行が無い: {word}"
        );
    }
}
