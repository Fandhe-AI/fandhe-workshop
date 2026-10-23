//! CLI の外部仕様の契約の適合試験（TASK-399 1/2・2/2、AUTHOR-1〜AUTHOR-3）。
//!
//! 正は `docs/cli.md`（版 9）で、本ファイルは実行ファイルが契約の終了コードと出し先で終わることと、
//! 定数・型・使い方の文言を確かめる。文書との一致は `cli_contract_doc.rs` が見る（変異注入は
//! 文書を一時ディレクトリへ写さないため、文書を読む試験を分けている）。
//! 診断 1 行・項目の形・最終行・出し先は版 2（TASK-399 2/2）で足した。`compile -o` の動作は `compile_output.rs`（TASK-407）が見る。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use fandhe_workshop_pack::cli_contract::{
    compare_differ_line, compare_same_line, compare_unreadable_line, final_line, is_item, usage,
    FinalLine, PackSummary, Subcommand, AUTHOR2_ITEM_KINDS, BIN_NAME, EXIT_DIAGNOSTICS, EXIT_OK,
    EXIT_USAGE, ITEM_KINDS_V2,
};
use fandhe_workshop_pack::source::{self, Format, ScopeKind};
use fandhe_workshop_pack::{DiagCode, Diagnostic, DiagnosticError};

mod common;
use common::{copy_dir, testdata, Tmp, CHAPTER, WORKSHOP};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
}

fn code(o: &Output) -> Option<i32> {
    o.status.code()
}

fn ch1_pack() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/input-field-ime/test-data/ch1-pack")
}

fn min_3step_a() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step-pack")
}

fn min_3step_b() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/min-3step-pack-b")
}

#[test]
fn exit_codes_are_fixed_values() {
    assert_eq!(EXIT_OK, 0);
    assert_eq!(EXIT_DIAGNOSTICS, 1);
    assert_eq!(EXIT_USAGE, 2);
    assert_eq!(BIN_NAME, "fandhe-pack");
}

#[test]
fn commands_are_the_six_in_contract_order() {
    let names: Vec<&str> = Subcommand::ALL.iter().map(|c| c.name()).collect();
    assert_eq!(
        names,
        ["validate", "test", "compile", "vocab", "metrics", "compare"]
    );
}

#[test]
fn defects_is_not_an_author_command() {
    assert_eq!(Subcommand::from_name("defects"), None);
    assert!(!usage().contains("defects"));
    let o = bin().args(["defects", "x"]).output().unwrap();
    assert_eq!(code(&o), Some(2));
    assert!(o.stdout.is_empty());
    assert!(String::from_utf8_lossy(&o.stderr).contains(&usage()));
}

#[test]
fn author1_no_args_prints_usage_and_exits_2() {
    let o = bin().output().unwrap();
    assert_eq!(code(&o), Some(2));
    assert!(o.stdout.is_empty());
    assert_eq!(String::from_utf8_lossy(&o.stderr), usage());
}

#[test]
fn unknown_command_exits_2_with_usage() {
    let o = bin().arg("bogus").output().unwrap();
    assert_eq!(code(&o), Some(2));
    assert!(o.stdout.is_empty());
    assert!(String::from_utf8_lossy(&o.stderr).contains(&usage()));
}

#[test]
fn argument_shape_errors_exit_2() {
    let a = ch1_pack();
    let cases: Vec<Vec<&std::ffi::OsStr>> = vec![
        vec!["compile".as_ref()],
        vec!["compare".as_ref()],
        vec!["compare".as_ref(), a.as_os_str()],
        vec![
            "compare".as_ref(),
            a.as_os_str(),
            a.as_os_str(),
            a.as_os_str(),
        ],
    ];
    for args in cases {
        let o = bin().args(&args).output().unwrap();
        assert_eq!(code(&o), Some(2), "{args:?}");
        assert!(o.stdout.is_empty(), "{args:?}");
    }
}

#[test]
fn argument_shape_errors_are_reported_before_loading() {
    let a = ch1_pack();
    let f = std::ffi::OsStr::new("f");
    let o_ = std::ffi::OsStr::new("-o");
    let x = std::ffi::OsStr::new("-x");
    let extra = std::ffi::OsStr::new("extra");
    let mut cases: Vec<Vec<&std::ffi::OsStr>> = Vec::new();
    for c in ["validate", "test", "vocab", "metrics"] {
        let c = std::ffi::OsStr::new(c);
        cases.push(vec![c]);
        cases.push(vec![c, a.as_os_str(), extra]);
        cases.push(vec![c, x]);
        cases.push(vec![c, o_, f, a.as_os_str()]);
    }
    let c = std::ffi::OsStr::new("compile");
    cases.push(vec![c, a.as_os_str(), extra]);
    cases.push(vec![c, x]);
    cases.push(vec![c, o_, f, a.as_os_str()]);
    cases.push(vec![c, a.as_os_str(), o_]);
    cases.push(vec![c, a.as_os_str(), o_, x]);
    cases.push(vec![c, a.as_os_str(), o_, f, o_, extra]);
    cases.push(vec![c, a.as_os_str(), o_, f, extra]);
    cases.push(vec![std::ffi::OsStr::new("compare"), x, a.as_os_str()]);
    for args in cases {
        let o = bin().args(&args).output().unwrap();
        assert_eq!(code(&o), Some(2), "{args:?}");
        assert!(o.stdout.is_empty(), "{args:?}");
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(err.starts_with("error:"), "{args:?}: {err}");
        assert!(err.contains(&usage()), "{args:?}");
        assert!(!err.contains("not implemented"), "{args:?}: {err}");
    }
}

#[test]
fn exit_0_and_1_follow_diagnostics() {
    let o = bin().arg("compile").arg(ch1_pack()).output().unwrap();
    assert_eq!(code(&o), Some(0));
    let o = bin()
        .args(["compile", "/nonexistent/path"])
        .output()
        .unwrap();
    assert_eq!(code(&o), Some(1));
    let o = bin()
        .arg("compare")
        .arg(min_3step_a())
        .arg(min_3step_b())
        .output()
        .unwrap();
    assert_eq!(code(&o), Some(0));
    let o = bin()
        .arg("compare")
        .arg(ch1_pack())
        .arg(min_3step_a())
        .output()
        .unwrap();
    assert_eq!(code(&o), Some(1));
}

#[test]
fn usage_declares_compile_output_option() {
    // `-o` の動作は `compile_output.rs`（TASK-407）が見る。ここでは使い方に宣言されていることだけを見る。
    assert!(usage().contains("  compile <pack-dir> [--chapters <id>[,<id>...]] [-o <file>]\n"));
    assert!(usage().starts_with("usage: fandhe-pack <command> [<args>]\n\ncommands:\n"));
}

/// 参照の欠陥（存在しない Step への遷移）を 1 つ持つ最小パック。
fn defective_pack() -> Tmp {
    let t = Tmp::new("author2-defect");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        CHAPTER.replace("{to: b}", "{to: zz}").as_bytes(),
    );
    t.put("chapters/c1/a.md", b"# a\n");
    t
}

/// 標準出力の全行が `<ファイル>: <項目>: <コード>: <メッセージ>` の 4 欄で、項目が契約の形であることを見る。
/// 見た行数を返す。
fn assert_diagnostic_lines(stdout: &str) -> usize {
    let mut n = 0;
    for line in stdout.lines() {
        let f: Vec<&str> = line.splitn(4, ": ").collect();
        assert_eq!(f.len(), 4, "4 欄でない: {line}");
        assert!(!f[0].trim().is_empty(), "ファイルが空: {line}");
        assert!(!f[1].trim().is_empty(), "項目が空: {line}");
        assert!(is_item(f[1]), "項目の形でない: {line}");
        assert!(f[2].starts_with("E-"), "コードでない: {line}");
        n += 1;
    }
    n
}

#[test]
fn diagnostic_requires_file_and_item() {
    for (file, item, want) in [
        ("", "step:a", DiagnosticError::EmptyFile),
        ("  ", "step:a", DiagnosticError::EmptyFile),
        ("a.yaml", "", DiagnosticError::EmptyItem),
        ("a.yaml", " \t", DiagnosticError::EmptyItem),
    ] {
        assert_eq!(
            Diagnostic::new(file, item, DiagCode::Parse, "m"),
            Err(want),
            "{file:?} {item:?}"
        );
    }
    assert!(Diagnostic::new("a.yaml", "step:a", DiagCode::Parse, "m").is_ok());
}

#[test]
fn author2_diagnostic_lines_have_four_fields() {
    let missing = bin()
        .args(["compile", "/nonexistent/path"])
        .output()
        .unwrap();
    assert_eq!(code(&missing), Some(1));
    assert!(missing.stderr.is_empty());
    let out = String::from_utf8(missing.stdout).unwrap();
    assert!(assert_diagnostic_lines(&out) >= 1);

    let pack = defective_pack();
    let o = bin().arg("compile").arg(&pack.0).output().unwrap();
    assert_eq!(code(&o), Some(1));
    assert!(o.stderr.is_empty());
    let out = String::from_utf8(o.stdout).unwrap();
    assert!(assert_diagnostic_lines(&out) >= 1);
    assert!(out.contains(": step:a: E-REF-STEP: "), "{out}");
}

#[test]
fn diagnostic_separator_in_fields_is_escaped() {
    let d = Diagnostic::new("a: b.yaml", "step:x: y", DiagCode::RefStep, "m: n").unwrap();
    let s = d.to_string();
    let f: Vec<&str> = s.splitn(4, ": ").collect();
    assert_eq!(f.len(), 4, "{s}");
    assert_eq!(f[0], "a:\\u{0020}b.yaml");
    assert_eq!(f[1], "step:x:\\u{0020}y");
    assert_eq!(f[3], "m: n");
}

#[test]
fn diagnostics_go_to_stdout() {
    let pack = defective_pack();
    let o = bin().arg("compile").arg(&pack.0).output().unwrap();
    assert_eq!(code(&o), Some(1));
    assert!(!o.stdout.is_empty());
    assert!(o.stderr.is_empty());
}

#[test]
fn author2_item_kinds_include_spec_five() {
    assert_eq!(
        AUTHOR2_ITEM_KINDS,
        ["step", "rejection", "start", "playthrough", "error_input"]
    );
    for k in AUTHOR2_ITEM_KINDS {
        assert!(is_item(k) && is_item(&format!("{k}:x")), "{k}");
        assert!(ITEM_KINDS_V2.contains(&k), "{k}");
    }
}

#[test]
fn is_item_rejects_malformed() {
    for bad in ["", ":x", "Step:a", "1a", "step:", "_a", "st ep:a"] {
        assert!(!is_item(bad), "{bad:?}");
    }
    for good in [
        "pack",
        "step:a",
        "command:#2",
        "error_input:#1",
        "pack:profile.command_prefix",
    ] {
        assert!(is_item(good), "{good:?}");
    }
}

#[test]
fn author2_final_line_forms() {
    let summary = PackSummary {
        format: Format::Yaml,
        chapters: 1,
        modules: 0,
        step_defs: 2,
    };
    assert_eq!(
        final_line(&FinalLine::Checked {
            diagnostics: 0,
            summary
        }),
        "OK: 診断 0 件（形式 B(YAML)、章 1、モジュール 0、Step の定義 2）"
    );
    assert_eq!(
        final_line(&FinalLine::Checked {
            diagnostics: 3,
            summary
        }),
        "NG: 診断 3 件（形式 B(YAML)、章 1、モジュール 0、Step の定義 2）"
    );
    let toml = PackSummary {
        format: Format::Toml,
        ..summary
    };
    assert!(toml.to_string().starts_with("形式 A(TOML)、"));
}

#[test]
fn author3_unreadable_final_line() {
    assert_eq!(
        final_line(&FinalLine::Unreadable { diagnostics: 8 }),
        "NG: 診断 8 件（読み込めません）"
    );
    assert!(!final_line(&FinalLine::Unreadable { diagnostics: 0 }).starts_with("OK:"));
}

#[test]
fn pack_summary_counts() {
    let dir = testdata("pack-defects/base/format-b");
    let tmp = Tmp::new("author2-summary");
    copy_dir(&dir, &tmp.0);
    let pack = source::load(&tmp.0).expect("土台の欠陥パックは読める");
    let s = PackSummary::of(&pack);
    let count_dirs = |sub: &str| {
        std::fs::read_dir(dir.join(sub))
            .unwrap()
            .flatten()
            .filter(|e| e.path().is_dir())
            .count()
    };
    assert_eq!(s.format, Format::Yaml);
    assert_eq!(s.chapters, count_dirs("chapters"));
    assert_eq!(s.modules, count_dirs("modules"));
    let steps: usize = pack
        .scopes
        .iter()
        .filter(|sc| matches!(sc.kind, ScopeKind::Chapter | ScopeKind::Module))
        .map(|sc| sc.src.steps.len())
        .sum();
    assert_eq!(s.step_defs, steps);
    assert!(s.step_defs > 0);
}

#[test]
fn compare_lines_follow_contract() {
    let same = bin()
        .arg("compare")
        .arg(min_3step_a())
        .arg(min_3step_b())
        .output()
        .unwrap();
    assert_eq!(code(&same), Some(0));
    let out = String::from_utf8(same.stdout).unwrap();
    assert!(out.starts_with("OK: IR が一致しました（"), "{out}");
    assert!(out.ends_with("バイト）\n"), "{out}");

    let differ = bin()
        .arg("compare")
        .arg(ch1_pack())
        .arg(min_3step_a())
        .output()
        .unwrap();
    assert_eq!(code(&differ), Some(1));
    let out = String::from_utf8(differ.stdout).unwrap();
    assert!(out.starts_with("NG: IR が一致しません（"), "{out}");
    assert!(
        out.contains("行目、") && out.ends_with("バイト目）\n"),
        "{out}"
    );

    let unreadable = bin()
        .arg("compare")
        .arg(ch1_pack())
        .arg("/nonexistent/path")
        .output()
        .unwrap();
    assert_eq!(code(&unreadable), Some(1));
    assert!(unreadable.stderr.is_empty());
    let out = String::from_utf8(unreadable.stdout).unwrap();
    let lines: Vec<&str> = out.lines().collect();
    let (last, diags) = lines.split_last().unwrap();
    assert_eq!(*last, compare_unreadable_line(0, diags.len()));
    assert!(!out.contains("一致"), "{out}");
    assert_eq!(assert_diagnostic_lines(&diags.join("\n")), diags.len());
    // 行の組み立て関数が、文書の例と同じ形を返す。
    assert_eq!(compare_same_line(7), "OK: IR が一致しました（7 バイト）");
    assert_eq!(
        compare_differ_line(3, 57),
        "NG: IR が一致しません（3 行目、58 バイト目）"
    );
}
