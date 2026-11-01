//! `fandhe-pack preview` の関門と診断時の終了の適合試験（AUTHOR-22・AUTHOR-2、TASK-429 1/3）。
//!
//! 診断のある教材パックは何も配らず、`validate` と同じ形（診断の行と最終行）を標準出力に出して終了コード 1 で
//! 終わることを実行ファイルで確かめる。診断 0 件の経路は feature `embed-web` の有無で分かれる（配信の試験は `preview_serve.rs`・`preview_embedded.rs`）。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use fandhe_workshop_pack::cli_contract::is_item;
use fandhe_workshop_pack::preview::{gate, Gate};

mod common;
use common::{minimal, Tmp, CHAPTER, WORKSHOP};

fn run(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .args(args)
        .output()
        .unwrap()
}

/// 次の遷移先が存在しない（`validate` が参照の誤りを報告する）パック。
fn broken_ref() -> Tmp {
    let t = Tmp::new("preview-broken");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        CHAPTER.replace("to: b", "to: zz").as_bytes(),
    );
    t.put("chapters/c1/a.md", b"# a\n");
    t
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn author22_unreadable_pack_is_not_previewed() {
    let missing = Tmp::new("preview-missing");
    let nodir = missing.0.join("nope");
    let broken = Tmp::new("preview-syntax");
    broken.put("workshop.yaml", b"pack: [unclosed\n");
    for dir in [nodir.as_path(), broken.0.as_path()] {
        let o = run(&["preview".as_ref(), dir.as_os_str()]);
        assert_eq!(o.status.code(), Some(1), "{dir:?}");
        assert!(o.stderr.is_empty(), "{dir:?}");
        let out = stdout(&o);
        let last = out.lines().last().unwrap();
        assert!(last.starts_with("NG: 診断 "), "{last}");
        assert!(last.ends_with("（読み込めません）"), "{last}");
    }
}

#[test]
fn author22_validate_diagnostics_stop_preview() {
    let t = broken_ref();
    let o = run(&["preview".as_ref(), t.0.as_os_str()]);
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stderr.is_empty());
    let out = stdout(&o);
    assert!(!out.starts_with('{'), "IR must not be printed: {out}");
    assert!(
        out.lines().last().unwrap().starts_with("NG: 診断 "),
        "{out}"
    );
}

/// `validate` だけが報告する診断（練習データの【架空】の印の欠け。`E-FICTIONAL-MARK`）を持つパック。`compile` は通る。
fn validate_only() -> Tmp {
    let t = Tmp::new("preview-validate-only");
    t.put(
        "workshop.yaml",
        b"pack: { id: p, title: t, version: '0', ir_format: 1, guidance: A }\nfixtures:\n  fx-item: { path: fixtures/item.txt, kind: file }\n",
    );
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts:\n  - id: s1\n    node: a\n    effects:\n      - put: { path: shelf/a.dat, fixture: fx-item }\nsteps:\n  - {id: a, title: A, body: a.md, next: [{to: b}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-a, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}\n  - {id: b, title: B, final: true}\n",
    );
    t.put("chapters/c1/a.md", b"# a\n");
    t.put("fixtures/item.txt", b"x\n");
    t
}

#[test]
fn author22_validate_only_diagnostics_stop_preview() {
    let t = validate_only();
    let c = run(&["compile".as_ref(), t.0.as_os_str()]);
    assert_eq!(
        c.status.code(),
        Some(0),
        "compile must pass for this fixture"
    );
    let o = run(&["preview".as_ref(), t.0.as_os_str()]);
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stderr.is_empty());
    assert!(stdout(&o).lines().last().unwrap().starts_with("NG: 診断 "));
    assert!(matches!(gate(&t.0), Gate::Diagnosed { .. }));
}

#[test]
fn author22_output_matches_validate() {
    let t = broken_ref();
    let p = run(&["preview".as_ref(), t.0.as_os_str()]);
    let v = run(&["validate".as_ref(), t.0.as_os_str()]);
    assert_eq!(p.status.code(), v.status.code());
    assert_eq!(p.stdout, v.stdout);
    assert_eq!(p.stderr, v.stderr);
}

#[test]
fn author22_diagnostic_lines_have_four_fields() {
    let t = broken_ref();
    let o = run(&["preview".as_ref(), t.0.as_os_str()]);
    let out = stdout(&o);
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines.len() >= 2, "{out}");
    for l in &lines[..lines.len() - 1] {
        let f: Vec<&str> = l.splitn(4, ": ").collect();
        assert_eq!(f.len(), 4, "{l}");
        assert!(!f[0].is_empty() && is_item(f[1]), "{l}");
        assert!(f[2].starts_with("E-"), "{l}");
    }
}

fn snapshot(dir: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    common::files_of(dir, dir, &mut v);
    v.sort();
    v
}

#[test]
fn author22_diagnosed_pack_writes_nothing() {
    let t = broken_ref();
    let cwd = Tmp::new("preview-cwd");
    let (before_p, before_c) = (snapshot(&t.0), snapshot(&cwd.0));
    let o = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .arg("preview")
        .arg(&t.0)
        .current_dir(&cwd.0)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(before_p, snapshot(&t.0));
    assert_eq!(before_c, snapshot(&cwd.0));
    let out = stdout(&o);
    assert!(!out.contains("http://") && !out.contains("https://"));
}

/// 同梱の無いビルド（feature なし）では何も配らず、理由つきの誤りで 2 になる。
#[cfg(not(feature = "embed-web"))]
#[test]
fn author22_clean_pack_without_bundle_is_usage_error() {
    let t = minimal();
    let o = run(&["preview".as_ref(), t.0.as_os_str()]);
    assert_eq!(o.status.code(), Some(2));
    assert!(o.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&o.stderr),
        "error: fandhe-pack preview: the engine's web files are not bundled in this build\n"
    );
}

#[test]
fn author22_gate_ready_ir_matches_compile() {
    let t = minimal();
    let c = run(&["compile".as_ref(), t.0.as_os_str()]);
    assert_eq!(c.status.code(), Some(0));
    match gate(&t.0) {
        Gate::Ready { ir, .. } => assert_eq!(ir, c.stdout),
        other => panic!("expected Ready: {other:?}"),
    }
}

#[test]
fn author22_gate_stops_at_validate_layer() {
    let t = broken_ref();
    assert!(matches!(gate(&t.0), Gate::Diagnosed { .. }));
    assert!(matches!(gate(&minimal().0), Gate::Ready { .. }));
}
