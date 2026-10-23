//! `fandhe-pack validate` の出力・終了コード・読み込み失敗時の打ち切りの結合試験（TASK-403、AUTHOR-2・AUTHOR-3）。
//!
//! 正は `docs/cli.md` の 6・8・9 節。診断の行は `validate::validate` の結果そのままで、最終行は
//! `cli_contract::final_line` と完全一致することを見る。作業ツリーは書き換えず、一時ディレクトリだけを使う。

use std::path::Path;
use std::process::{Command, Output};

use fandhe_workshop_pack::cli_contract::{
    final_line, is_item, FinalLine, PackSummary, EXIT_DIAGNOSTICS, EXIT_OK,
};
use fandhe_workshop_pack::{source, validate};

mod common;
use common::{minimal, Tmp, CHAPTER, WORKSHOP};

const UNREADABLE_MARK: &str = "読み込めません";

fn run(dir: &Path) -> (Option<i32>, Vec<String>, String) {
    let o: Output = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .arg("validate")
        .arg(dir)
        .output()
        .unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    let err = String::from_utf8(o.stderr).unwrap();
    (
        o.status.code(),
        out.lines().map(str::to_owned).collect(),
        err,
    )
}

/// 最後の行を最終行とし、残りの全行が 4 欄の診断で、件数が最終行の N と一致することを見る。
fn split_lines(lines: &[String]) -> (&[String], &str) {
    let (last, diag) = lines.split_last().expect("最終行がある");
    for l in diag {
        let f: Vec<&str> = l.splitn(4, ": ").collect();
        assert_eq!(f.len(), 4, "4 欄でない: {l}");
        assert!(is_item(f[1]) && f[2].starts_with("E-"), "{l}");
    }
    assert!(
        last.starts_with(&format!("NG: 診断 {} 件", diag.len())) || last.starts_with("OK: "),
        "件数が合わない: {last}"
    );
    (diag, last)
}

fn defective() -> Tmp {
    with_chapter(&CHAPTER.replace("{to: b}", "{to: zz}"))
}

fn with_chapter(chapter: &str) -> Tmp {
    let t = Tmp::new("vc-pack");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    t
}

#[test]
fn author2_ok_pack_prints_ok_line_and_exits_0() {
    let t = minimal();
    let pack = source::load(&t.0).unwrap();
    let want = final_line(&FinalLine::Checked {
        diagnostics: 0,
        summary: PackSummary::of(&pack),
    });
    let (code, lines, err) = run(&t.0);
    assert_eq!(code, Some(i32::from(EXIT_OK)));
    assert_eq!(lines, vec![want.clone()]);
    assert!(want.starts_with("OK: 診断 0 件（"));
    assert!(err.is_empty());
}

#[test]
fn author2_defective_pack_prints_diagnostics_then_ng_line_and_exits_1() {
    let t = defective();
    let pack = source::load(&t.0).unwrap();
    let (code, lines, err) = run(&t.0);
    assert_eq!(code, Some(i32::from(EXIT_DIAGNOSTICS)));
    assert!(err.is_empty());
    let (diag, last) = split_lines(&lines);
    assert!(!diag.is_empty());
    assert!(diag.iter().any(|l| l.contains(": step:a: E-REF-STEP: ")));
    assert_eq!(
        last,
        final_line(&FinalLine::Checked {
            diagnostics: diag.len(),
            summary: PackSummary::of(&pack),
        })
    );
    assert!(last.starts_with("NG: ") && !last.contains(UNREADABLE_MARK));
    assert!(!lines.iter().any(|l| l.starts_with("OK:")));
}

#[test]
fn author2_diagnostic_lines_match_library() {
    let t = defective();
    let want: Vec<String> = validate::validate(&source::load(&t.0).unwrap())
        .iter()
        .map(ToString::to_string)
        .collect();
    let (_, lines, _) = run(&t.0);
    assert_eq!(&lines[..lines.len() - 1], &want[..]);
}

fn assert_unreadable(t: &Tmp, prefix: &str) {
    let (code, lines, err) = run(&t.0);
    assert_eq!(code, Some(i32::from(EXIT_DIAGNOSTICS)));
    assert!(err.is_empty());
    let (diag, last) = split_lines(&lines);
    assert!(!diag.is_empty());
    assert_eq!(
        last,
        format!("NG: 診断 {} 件（{UNREADABLE_MARK}）", diag.len())
    );
    assert!(
        diag.iter().all(|l| l.contains(prefix)),
        "{prefix}: {lines:?}"
    );
    assert!(!lines.iter().any(|l| l.starts_with("OK:")));
}

#[test]
fn author3_unreadable_causes_end_with_unreadable_line() {
    // 構文の誤り（閉じない flow mapping）。
    let t = Tmp::new("vc-syntax");
    t.put("workshop.yaml", b"pack: {id: p, title: t\n");
    assert_unreadable(&t, ": E-PARSE: ");
    // 未知の欄。
    assert_unreadable(
        &with_chapter(&CHAPTER.replace("title: A,", "title: A, script: x,")),
        ": E-PARSE: ",
    );
    // 型の違い。
    assert_unreadable(
        &with_chapter(&CHAPTER.replace("title: A,", "title: [1],")),
        ": E-PARSE: ",
    );
    // 必須欄の欠け（Step の id）。
    assert_unreadable(
        &with_chapter(
            &CHAPTER.replace("{id: b, title: B, final: true}", "{title: B, final: true}"),
        ),
        ": E-PARSE: ",
    );
    // 上限超過（深さ）。
    let t = Tmp::new("vc-depth");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    let mut deep = String::from("deep");
    for _ in 1..33 {
        deep.push_str("/d");
    }
    t.put(&format!("{deep}/x.txt"), b"x");
    assert_unreadable(&t, ": E-LIMIT");
    // 存在しないパス。
    let t = Tmp::new("vc-missing");
    let (code, lines, _) = run(&t.0.join("gone"));
    assert_eq!(code, Some(i32::from(EXIT_DIAGNOSTICS)));
    assert_eq!(lines.last().unwrap(), "NG: 診断 1 件（読み込めません）");
}

#[cfg(unix)]
#[test]
fn author3_symlink_is_unreadable() {
    let t = Tmp::new("vc-symlink");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    std::os::unix::fs::symlink("nowhere", t.0.join("link")).unwrap();
    assert_unreadable(&t, ": E-LIMIT");
}

/// 負の対照: 読み込みに失敗するパックでは、同時に持つ参照の欠陥の検査は流れない。
/// 構文の誤りだけを直した写しでは参照の診断が出て、要約つきの最終行になる。
#[test]
fn author3_load_failure_suppresses_checks() {
    let broken = CHAPTER
        .replace("{to: b}", "{to: zz}")
        .replace("title: A,", "title: [1],");
    let t = with_chapter(&broken);
    let (code, lines, _) = run(&t.0);
    assert_eq!(code, Some(i32::from(EXIT_DIAGNOSTICS)));
    let joined = lines.join("\n");
    assert!(joined.contains("E-PARSE"), "{joined}");
    for banned in ["E-REF-", "E-UNREACHABLE", "E-DEAD-END", "E-STRUCTURE"] {
        assert!(!joined.contains(banned), "{banned}: {joined}");
    }
    assert!(lines
        .last()
        .unwrap()
        .ends_with(&format!("（{UNREADABLE_MARK}）")));

    let fixed = defective();
    let (code, lines, _) = run(&fixed.0);
    assert_eq!(code, Some(i32::from(EXIT_DIAGNOSTICS)));
    assert!(lines.join("\n").contains("E-REF-STEP"));
    assert!(!lines.last().unwrap().contains(UNREADABLE_MARK));
}

/// 境界の固定: 形式の版の不一致は読み込みの層が出すので、検査を流さず `読み込めません` で終わる。
#[test]
fn author3_format_version_mismatch_is_unreadable() {
    let t = Tmp::new("vc-formatver");
    t.put(
        "workshop.yaml",
        WORKSHOP.replace("ir_format: 1", "ir_format: 99").as_bytes(),
    );
    t.put("chapters/c1/chapter.yaml", CHAPTER.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    let (code, lines, _) = run(&t.0);
    assert_eq!(code, Some(i32::from(EXIT_DIAGNOSTICS)));
    let joined = lines.join("\n");
    assert_eq!(joined.matches("E-FORMAT-VERSION").count(), 1, "{joined}");
    let last = lines.last().unwrap();
    assert_eq!(last, "NG: 診断 1 件（読み込めません）");
}

/// A09: 存在しないパスを指定しても、その文字列を標準出力に写さない。
#[test]
fn author2_missing_dir_does_not_leak_path() {
    let name = format!("fandhe-secret-dir-{}", std::process::id());
    let p = Path::new("/nonexistent").join(&name);
    let (_, lines, err) = run(&p);
    assert!(!lines.join("\n").contains(&name));
    assert!(err.is_empty());
}
