//! CLI の外部仕様の文書（`docs/cli.md`）とコード側の契約（`cli_contract`）の一致の試験（TASK-399 1/2・2/2）。
//!
//! 版の欄・コマンドの表・使い方のブロック・終了コードの表・項目の種別の表・最終行の例・出し先の節を、定数と型から照合する。文書は読むだけで、
//! 作業ツリーを書き換えない。実行ファイルの挙動は `cli_contract.rs` が見る。

use std::fs;
use std::path::Path;

use fandhe_workshop_pack::cli_contract::{
    compare_differ_line, compare_same_line, compare_unreadable_line, final_line, metrics_row,
    test_error_pass_line, test_fail_line, test_pass_line, test_r21_line, test_r22_line,
    test_summary_lines, usage, vocab_pack_line, vocab_row, vocab_summary_lines, FinalLine,
    PackSummary, Subcommand, VocabRowKind, AUTHOR2_ITEM_KINDS, CLI_CONTRACT_VERSION,
    EXIT_DIAGNOSTICS, EXIT_OK, EXIT_USAGE, ITEM_KINDS_V2, METRICS_HEADER, METRICS_TOTAL_LABEL,
    VOCAB_HEADER,
};
use fandhe_workshop_pack::source::Format;

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

#[test]
fn cli_doc_item_kinds_table_matches() {
    let rows = table_rows(&section(&doc(), "## 7. 項目の書式"));
    let kinds: Vec<String> = rows
        .iter()
        .map(|r| r[0].trim_matches('`').to_string())
        .collect();
    assert_eq!(kinds, ITEM_KINDS_V2);
    for k in AUTHOR2_ITEM_KINDS {
        assert!(kinds.iter().any(|x| x == k), "{k}");
    }
}

#[test]
fn cli_doc_final_line_examples_match() {
    let body = section(&doc(), "## 8. 最終行");
    let mut in_block = false;
    let mut block = Vec::new();
    for l in body.lines() {
        if l == "```text" {
            in_block = true;
        } else if l == "```" {
            break;
        } else if in_block {
            block.push(l.to_string());
        }
    }
    let summary = PackSummary {
        format: Format::Yaml,
        chapters: 1,
        modules: 0,
        step_defs: 2,
    };
    let expected = [
        final_line(&FinalLine::Checked {
            diagnostics: 0,
            summary,
        }),
        final_line(&FinalLine::Checked {
            diagnostics: 3,
            summary,
        }),
        final_line(&FinalLine::Unreadable { diagnostics: 8 }),
        compare_same_line(120),
        compare_differ_line(3, 57),
        compare_unreadable_line(0, 4),
    ];
    assert_eq!(block, expected);
}

#[test]
fn cli_doc_test_pass_line_example_matches() {
    let body = section(&doc(), "## 12.2. `test` の模範解答の行");
    let mut in_block = false;
    let mut block = Vec::new();
    for l in body.lines() {
        if l == "```text" {
            in_block = true;
        } else if l == "```" {
            break;
        } else if in_block {
            block.push(l.to_string());
        }
    }
    assert_eq!(block, [test_pass_line("pt-main", 2)]);
}

fn text_block(heading: &str) -> Vec<String> {
    let body = section(&doc(), heading);
    let mut in_block = false;
    let mut block = Vec::new();
    for l in body.lines() {
        if l == "```text" {
            in_block = true;
        } else if l == "```" {
            break;
        } else if in_block {
            block.push(l.to_string());
        }
    }
    block
}

#[test]
fn cli_doc_test_error_lines_example_matches() {
    assert_eq!(
        text_block("## 12.3. `test` の誤入力の行と FAIL の行"),
        [
            test_error_pass_line("e01", "rejected:bad_arg_shape"),
            test_fail_line(
                "e03",
                "rejected:bad_arg_shape",
                "rejected:missing_arg",
                None
            ),
            test_fail_line("pt-main", "achieved", "progress", Some("区間 1 の入力 2")),
        ]
    );
}

#[test]
fn cli_doc_test_summary_example_matches() {
    assert_eq!(
        text_block("## 12.4. `test` の集計"),
        test_summary_lines(1, 1, 3, 3, 5)
    );
}

#[test]
fn cli_doc_test_r22_r21_example_matches() {
    assert_eq!(
        text_block("## 12.5. `test` の R-22・R-21 の集計"),
        vec![test_r22_line(1, 1, 0), test_r21_line(3, 0)]
    );
}

#[test]
fn cli_doc_destination_section_present() {
    let body = section(&doc(), "## 9. 診断の出し先");
    assert!(body.contains("標準出力"));
    assert!(body.contains("標準エラー出力"));
}

#[test]
fn cli_doc_metrics_block_matches_contract() {
    let body = section(&doc(), "## 11. `metrics` の出力");
    let mut in_block = false;
    let mut block = Vec::new();
    for l in body.lines() {
        if l == "```text" {
            in_block = true;
        } else if l == "```" {
            break;
        } else if in_block {
            block.push(l.to_string());
        }
    }
    assert_eq!(block.len(), 4);
    assert_eq!(block[0], METRICS_HEADER);
    assert_eq!(
        block[1],
        metrics_row("chapters/c1/chapter.yaml", 12, 10, 230, 4)
    );
    assert_eq!(block[2], metrics_row("workshop.yaml", 6, 5, 120, 3));
    assert_eq!(block[3], format!("{METRICS_TOTAL_LABEL},18,15,350,4"));
    assert_eq!(
        metrics_row("a,b\n", 1, 1, 1, 1),
        "a\\u{002C}b\\u{000A},1,1,1,1"
    );
    assert_eq!(
        metrics_row("\"a.yaml", 1, 1, 1, 1),
        "\\u{0022}a.yaml,1,1,1,1"
    );
}

/// 12 節の例のブロックが、`vocab` の出力の形を作る関数の結果と完全に一致する（TASK-408）。
#[test]
fn cli_doc_vocab_example_matches_contract() {
    let body = section(&doc(), "## 12. vocab の出力");
    let mut in_block = false;
    let mut block = Vec::new();
    for l in body.lines() {
        if l == "```text" {
            in_block = true;
        } else if l == "```" {
            break;
        } else if in_block {
            block.push(l.to_string());
        }
    }
    let mut expected = vec![
        VOCAB_HEADER.to_string(),
        vocab_row("c1", "s1", VocabRowKind::Step, &[]),
        vocab_row("c1", "s2", VocabRowKind::Step, &["EP-1", "EP-7"]),
        vocab_row("c1", "s3", VocabRowKind::Use, &[]),
        vocab_row("m1", "s4", VocabRowKind::Step, &["EP-3"]),
    ];
    expected.extend(vocab_summary_lines(
        1,
        2,
        &[("EP-1", 1), ("EP-3", 1), ("EP-7", 1)],
    ));
    expected.push(vocab_pack_line("EP-11", "pack.appearance"));
    assert_eq!(block, expected);
}
