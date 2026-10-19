//! CLI の外部仕様の文書（`docs/cli.md`）とコード側の契約（`cli_contract`）の一致の試験（TASK-399 1/2）。
//!
//! 版の欄・コマンドの表・使い方のブロック・終了コードの表を、定数と型から照合する。文書は読むだけで、
//! 作業ツリーを書き換えない。実行ファイルの挙動は `cli_contract.rs` が見る。

use std::fs;
use std::path::Path;

use fandhe_workshop_pack::cli_contract::{
    usage, Subcommand, CLI_CONTRACT_VERSION, EXIT_DIAGNOSTICS, EXIT_OK, EXIT_USAGE,
};

fn doc() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/cli.md"))
        .expect("契約の文書を読めること")
}

/// `## <heading>` の節の本文（次の `## ` の前まで）。
fn section(doc: &str, heading: &str) -> String {
    let mut lines = doc.lines();
    lines
        .by_ref()
        .find(|l| *l == heading)
        .unwrap_or_else(|| panic!("見出しが無い: {heading}"));
    lines
        .take_while(|l| !l.starts_with("## "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 節の中の表の本文の行を、セルの配列にして返す（見出し行・区切り行を除く）。
fn table_rows(section: &str) -> Vec<Vec<String>> {
    section
        .lines()
        .filter(|l| l.starts_with('|'))
        .skip(2)
        .map(|l| {
            l.trim_matches('|')
                .split('|')
                .map(|c| c.trim().to_string())
                .collect()
        })
        .collect()
}

#[test]
fn cli_doc_version_matches_constant() {
    let doc = doc();
    let v = doc
        .lines()
        .find_map(|l| l.strip_prefix("CLI の外部仕様の版: "))
        .expect("版の欄があること");
    assert_eq!(v.trim().parse::<u32>().unwrap(), CLI_CONTRACT_VERSION);
    assert!(doc.lines().any(|l| l == "状態: 固定（契約）"));
}

#[test]
fn cli_doc_command_table_matches_contract() {
    let rows = table_rows(&section(&doc(), "## 3. コマンド一覧と引数の形"));
    assert_eq!(rows.len(), Subcommand::ALL.len());
    for (row, c) in rows.iter().zip(Subcommand::ALL) {
        assert_eq!(row[0], format!("`{}`", c.name()));
        assert_eq!(row[1], format!("`{}`", c.args()));
    }
}

#[test]
fn cli_doc_usage_block_matches_usage() {
    let body = section(&doc(), "## 4. 使い方");
    let mut in_block = false;
    let mut block = Vec::new();
    for l in body.lines() {
        if l == "```text" {
            in_block = true;
        } else if l == "```" {
            break;
        } else if in_block {
            block.push(l);
        }
    }
    assert_eq!(format!("{}\n", block.join("\n")), usage());
}

#[test]
fn cli_doc_exit_code_table_matches_constants() {
    let rows = table_rows(&section(&doc(), "## 5. 終了コード"));
    let codes: Vec<u8> = rows.iter().map(|r| r[0].parse().unwrap()).collect();
    assert_eq!(codes, [EXIT_OK, EXIT_DIAGNOSTICS, EXIT_USAGE]);
}
