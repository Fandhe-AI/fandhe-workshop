//! 公開の資料（`docs/examples/` の教材パック 2 つと `docs/cli-guide.md`）が、実行ファイルの出力と食い違わない
//! ことの結合試験（TASK-412、AUTHOR-12）。
//!
//! 作成者はエンジンのソースを読まずに、これらの資料と `fandhe-pack` だけで教材を書く。例が `validate` を通らない、
//! `test` で落ちる、手引きに写した出力が実際と違う、のいずれも資料として壊れているため、CI（`rust-test`）で守る。
//! 試験は fail-closed にする（試験 0 件・転記 0 件・許可リストの外の印は失敗）。文書から取り出した文字列は
//! シェルに渡さず、引数として 1 つずつ渡す。

#[path = "common/sha256.rs"]
mod sha256;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PACKS: [&str; 2] = ["minimal-pack", "choice-only-pack"];

fn docs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs")
}

fn examples_dir() -> PathBuf {
    docs_dir().join("examples")
}

/// 実行ファイルを引数 1 つずつで起動する。
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .args(args)
        .output()
        .expect("run fandhe-pack")
}

fn pack_arg(name: &str) -> String {
    examples_dir().join(name).to_string_lossy().into_owned()
}

fn stdout(o: &Output) -> String {
    String::from_utf8(o.stdout.clone()).expect("utf-8 stdout")
}

#[test]
fn author12_examples_validate_with_zero_diagnostics() {
    for name in PACKS {
        let o = run(&["validate", &pack_arg(name)]);
        assert_eq!(o.status.code(), Some(0), "{name}: exit code");
        assert!(o.stderr.is_empty(), "{name}: stderr must be empty");
        let out = stdout(&o);
        assert_eq!(out.lines().count(), 1, "{name}: exactly one line: {out}");
        assert!(out.starts_with("OK: 診断 0 件（"), "{name}: {out}");
    }
}

/// `模範解答: N/N 完走` のような `<ラベル>: A/B ...` の行から A と B を取り出す。
fn ratio(out: &str, label: &str) -> (u32, u32) {
    let line = out
        .lines()
        .find(|l| l.starts_with(label))
        .unwrap_or_else(|| panic!("missing line {label}: {out}"));
    let rest = line[label.len()..].trim_start();
    let frac = rest.split_whitespace().next().expect("fraction");
    let (a, b) = frac.split_once('/').expect("a/b");
    (
        a.parse().expect("numerator"),
        b.parse().expect("denominator"),
    )
}

#[test]
fn author12_examples_pass_test() {
    for name in PACKS {
        let o = run(&["test", &pack_arg(name)]);
        let out = stdout(&o);
        assert_eq!(o.status.code(), Some(0), "{name}: exit code: {out}");
        assert!(o.stderr.is_empty(), "{name}: stderr must be empty");
        assert!(
            !out.lines().any(|l| l.starts_with("FAIL ")),
            "{name}: FAIL line: {out}"
        );
        assert!(
            !out.lines().any(|l| l.contains("E-")),
            "{name}: diagnostic line: {out}"
        );
        let (done, total) = ratio(&out, "模範解答:");
        assert!(
            total >= 1,
            "{name}: no playthrough ran (0 tests are not a pass)"
        );
        assert_eq!(done, total, "{name}: playthroughs");
        let (ok, all) = ratio(&out, "誤入力:");
        assert_eq!(ok, all, "{name}: error inputs");
    }
}

#[test]
fn author12_examples_compile() {
    for name in PACKS {
        let o = run(&["compile", &pack_arg(name)]);
        assert_eq!(o.status.code(), Some(0), "{name}: exit code");
        assert!(o.stderr.is_empty(), "{name}: stderr must be empty");
        let out = stdout(&o);
        assert!(out.trim_start().starts_with('{'), "{name}: IR is JSON");
    }
}

#[test]
fn author12_examples_are_the_two_named_packs() {
    let mut dirs = BTreeSet::new();
    for e in std::fs::read_dir(examples_dir()).expect("read examples") {
        let e = e.expect("entry");
        if e.file_type().expect("type").is_dir() {
            dirs.insert(e.file_name().to_string_lossy().into_owned());
        }
    }
    let want: BTreeSet<String> = PACKS.iter().map(|s| s.to_string()).collect();
    assert_eq!(dirs, want);

    // 選択肢と画面操作だけで進む例は、コマンドも自然文も入力方式に持たない。
    let ir = stdout(&run(&["compile", &pack_arg("choice-only-pack")]));
    for banned in ["\"command\"", "\"text\""] {
        assert!(
            !ir.contains(&format!("\"kind\": {banned}")),
            "choice-only-pack must not use input.kind {banned}"
        );
    }
    let chapter = std::fs::read_to_string(
        examples_dir().join("choice-only-pack/chapters/c-pick/chapter.yaml"),
    )
    .expect("chapter.yaml");
    assert!(!chapter.contains("kind: command") && !chapter.contains("kind: text"));
}

fn collect_files(dir: &Path, base: &Path, out: &mut BTreeSet<String>) {
    for e in std::fs::read_dir(dir).expect("read dir") {
        let e = e.expect("entry");
        let p = e.path();
        let ty = e.file_type().expect("type");
        if ty.is_dir() {
            collect_files(&p, base, out);
        } else if ty.is_file() {
            let rel = p
                .strip_prefix(base)
                .expect("prefix")
                .to_string_lossy()
                .into_owned();
            if rel != "MANIFEST.sha256" {
                out.insert(rel);
            }
        }
    }
}

#[test]
fn author12_examples_manifest_matches() {
    let base = examples_dir();
    let text = std::fs::read_to_string(base.join("MANIFEST.sha256")).expect("manifest");
    let mut listed = BTreeSet::new();
    for line in text.lines() {
        let (hash, path) = line.split_once("  ").expect("<hash>  <path>");
        assert_eq!(hash.len(), 64, "hash length: {line}");
        assert!(
            hash.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "lowercase hex: {line}"
        );
        assert!(
            !path.is_empty() && !path.starts_with('/') && !path.split('/').any(|s| s == ".."),
            "relative path without ..: {line}"
        );
        let bytes = std::fs::read(base.join(path)).expect("listed file exists");
        assert_eq!(sha256::hex(&sha256::sha256(&bytes)), hash, "{path}");
        assert!(listed.insert(path.to_string()), "duplicate: {path}");
    }
    let mut actual = BTreeSet::new();
    collect_files(&base, &base, &mut actual);
    assert_eq!(listed, actual, "MANIFEST.sha256 must list every file");
}

/// 手引きの `<!-- cli-guide-run: <command> <pack-name> -->` の直後の text ブロックを集める。
fn transcripts(md: &str) -> Vec<(String, String, String)> {
    let lines: Vec<&str> = md.lines().collect();
    let mut found = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let Some(rest) = l.strip_prefix("<!-- cli-guide-run:") else {
            continue;
        };
        let rest = rest.strip_suffix("-->").expect("marker closes").trim();
        let (cmd, name) = rest.split_once(' ').expect("<command> <pack-name>");
        assert!(
            ["validate", "test", "vocab", "metrics"].contains(&cmd),
            "command not allowed: {cmd}"
        );
        assert!(PACKS.contains(&name), "pack not allowed: {name}");
        assert_eq!(
            lines.get(i + 1).copied(),
            Some("```text"),
            "block after marker"
        );
        let mut body = String::new();
        let mut closed = false;
        for b in &lines[i + 2..] {
            if *b == "```" {
                closed = true;
                break;
            }
            body.push_str(b);
            body.push('\n');
        }
        assert!(closed, "block closes");
        found.push((cmd.to_string(), name.to_string(), body));
    }
    found
}

#[test]
fn author12_cli_guide_transcripts_match() {
    let md = std::fs::read_to_string(docs_dir().join("cli-guide.md")).expect("cli-guide.md");
    let all = transcripts(&md);
    assert!(!all.is_empty(), "no transcripts (fail-closed)");
    for (cmd, name, body) in all {
        let o = run(&[&cmd, &pack_arg(&name)]);
        assert_eq!(stdout(&o), body, "{cmd} {name}");
    }
}
