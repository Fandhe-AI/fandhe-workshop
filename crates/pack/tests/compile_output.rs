//! `compile` の出力とバイト単位の決定性の結合試験（CLI の面。04-behavior の AUTHOR-8、TASK-407）。
//!
//! 実行ファイルを起動し、標準出力の IR と `-o` のファイルの IR が同じバイト列であること、同じ内容の教材パックは
//! 何度でも・別のディレクトリへ写しても同じバイト列になること、末尾の形（`}` の後に LF が 1 つ）、診断があるときに
//! ファイルを作らないこと、書き出せないときの終了コード 2 を確かめる。一時ディレクトリは `CARGO_TARGET_TMPDIR` の
//! 下に作り、作業ツリーには書かない。`-o` には必ず絶対パスを渡す。正は `docs/cli.md` 版 3 の 10 節である。

mod common;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::{copy_dir, minimal, reversed_copy, testdata, Tmp};
use fandhe_workshop_pack::compile::compile_dir;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
}

fn packs() -> Vec<PathBuf> {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    vec![
        testdata("engine-event-consistency/test-data/min-3step-pack"),
        testdata("input-field-ime/test-data/ch1-pack"),
        testdata("input-field-ime/test-data/ime-pack"),
        data.join("min-3step-pack-b"),
        data.join("document-body-pack"),
    ]
}

fn compile_stdout(dir: &Path) -> Vec<u8> {
    let o = bin().arg("compile").arg(dir).output().unwrap();
    assert_eq!(o.status.code(), Some(0), "{dir:?}");
    assert!(o.stderr.is_empty(), "{dir:?}");
    o.stdout
}

fn compile_to(dir: &Path, file: &Path) -> Output {
    bin()
        .arg("compile")
        .arg(dir)
        .arg("-o")
        .arg(file)
        .output()
        .unwrap()
}

fn names_in(dir: &Path) -> Vec<OsString> {
    let mut v: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    v.sort();
    v
}

/// 標準出力に出した IR と `-o` のファイルの IR が同じバイト列で、`-o` のとき標準出力・標準エラーは空（AUTHOR-8）。
#[test]
fn author8_stdout_and_output_file_are_byte_identical() {
    let t = Tmp::new("a8-same");
    for (i, pack) in packs().iter().enumerate() {
        let file = t.0.join(format!("ir{i}.json"));
        let o = compile_to(pack, &file);
        assert_eq!(o.status.code(), Some(0), "{pack:?}");
        assert!(o.stdout.is_empty(), "{pack:?}");
        assert!(o.stderr.is_empty(), "{pack:?}");
        let written = fs::read(&file).unwrap();
        assert!(!written.is_empty());
        assert_eq!(written, compile_stdout(pack), "{pack:?}");
        assert_eq!(written, compile_dir(pack).expect("compile_dir"), "{pack:?}");
    }
}

/// 同じパックを何度 compile しても、標準出力でも `-o` でも同じバイト列になる（AUTHOR-8）。
#[test]
fn author8_repeated_runs_are_byte_identical() {
    let t = Tmp::new("a8-repeat");
    for pack in packs() {
        let first = compile_stdout(&pack);
        assert_eq!(first, compile_stdout(&pack));
        assert_eq!(first, compile_stdout(&pack));
        for n in 0..2 {
            let file = t.0.join(format!("again{n}.json"));
            assert_eq!(compile_to(&pack, &file).status.code(), Some(0));
            assert_eq!(first, fs::read(&file).unwrap(), "{pack:?}");
        }
    }
}

/// 別のディレクトリへ写したパック・列挙順を変えた写しも、絶対パスでも相対パスでも同じバイト列になり、
/// IR に一時ディレクトリのパスが入らない（AUTHOR-8）。
#[test]
fn author8_copies_in_other_directories_are_byte_identical() {
    for (i, pack) in packs().iter().enumerate() {
        let want = compile_stdout(pack);
        let text = String::from_utf8(want.clone()).unwrap();
        assert!(!text.contains(env!("CARGO_TARGET_TMPDIR")));
        let t = Tmp::new(&format!("a8-copy{i}"));
        let shallow = t.0.join("a b/日本語 dir");
        let deep = t.0.join("x/y/z/w/pack");
        copy_dir(pack, &shallow);
        copy_dir(pack, &deep);
        let rev = reversed_copy(pack, &format!("a8-rev{i}"));
        for copy in [&shallow, &deep, &rev.0] {
            assert_eq!(want, compile_stdout(copy), "absolute {copy:?}");
            let o = bin()
                .current_dir(copy.parent().unwrap())
                .arg("compile")
                .arg(copy.file_name().unwrap())
                .output()
                .unwrap();
            assert_eq!(o.status.code(), Some(0), "relative {copy:?}");
            assert_eq!(want, o.stdout, "relative {copy:?}");
        }
    }
}

/// 出力の頭と末尾の形が固定で、読み戻せる（AUTHOR-8）。
#[test]
fn author8_output_tail_is_fixed() {
    for pack in packs() {
        let b = compile_stdout(&pack);
        assert!(b.starts_with(b"{"), "no BOM: {pack:?}");
        assert!(b.ends_with(b"}\n"), "{pack:?}");
        assert!(!b.ends_with(b"\n\n"), "{pack:?}");
        assert!(!b.contains(&b'\r'), "{pack:?}");
        assert!(std::str::from_utf8(&b).is_ok());
        serde_json::from_slice::<serde_json::Value>(&b).unwrap();
        fandhe_workshop_core::load_ir(&b).expect("load_ir accepts the output");
    }
}

/// 既存のファイルは丸ごと置き換わり、IR より長い古い内容の末尾が残らない（AUTHOR-8）。
#[test]
fn author8_output_replaces_existing_file_entirely() {
    let t = Tmp::new("a8-replace");
    let file = t.0.join("ir.json");
    fs::write(&file, "OLD-CONTENT-MARKER\n".repeat(100_000)).unwrap();
    let pack = common::ch1();
    assert_eq!(compile_to(&pack, &file).status.code(), Some(0));
    assert_eq!(fs::read(&file).unwrap(), compile_stdout(&pack));
}

/// 診断があるときはファイルを作らず、既存のファイルにも触れない（AUTHOR-8。fail-closed）。
#[test]
fn author8_diagnostics_do_not_write_output() {
    let t = Tmp::new("a8-diag");
    let missing = t.0.join("no-such-pack");
    let broken = minimal();
    broken.put("chapters/c1/chapter.yaml", b"id: [unclosed\n");
    for pack in [missing, broken.0.clone()] {
        let fresh = t.0.join("fresh.json");
        let o = compile_to(&pack, &fresh);
        assert_eq!(o.status.code(), Some(1), "{pack:?}");
        assert!(
            String::from_utf8_lossy(&o.stdout).contains("E-"),
            "{pack:?}"
        );
        assert!(o.stderr.is_empty(), "{pack:?}");
        assert!(!fresh.exists(), "{pack:?}");

        let existing = t.0.join("existing.json");
        fs::write(&existing, b"KEEP").unwrap();
        assert_eq!(compile_to(&pack, &existing).status.code(), Some(1));
        assert_eq!(fs::read(&existing).unwrap(), b"KEEP");
    }
}

/// 書き出せないとき（親が無い・ディレクトリを指す）は終了コード 2 と `error:` の 1 行で、一時ファイルを残さない（AUTHOR-8）。
#[test]
fn author8_unwritable_output_exits_2() {
    let t = Tmp::new("a8-unwritable");
    let pack = common::ch1();
    let no_parent = t.0.join("no-such-dir/ir.json");
    let dir_target = t.0.join("a-directory");
    fs::create_dir(&dir_target).unwrap();
    fs::write(dir_target.join("keep"), b"x").unwrap();
    for target in [&no_parent, &dir_target] {
        let o = compile_to(&pack, target);
        assert_eq!(o.status.code(), Some(2), "{target:?}");
        assert!(o.stdout.is_empty(), "{target:?}");
        let err = String::from_utf8(o.stderr).unwrap();
        assert!(err.starts_with("error:"), "{err}");
        assert_eq!(err.lines().count(), 1, "{err}");
    }
    assert!(!t.0.join("no-such-dir").exists());
    assert_eq!(names_in(&dir_target), vec![OsString::from("keep")]);
    assert_eq!(names_in(&t.0), vec![OsString::from("a-directory")]);
}

/// 成功したあと、出力先のディレクトリには出力のファイルだけが残る（一時ファイルが残らない。AUTHOR-8）。
#[test]
fn author8_no_temporary_file_is_left_after_success() {
    let t = Tmp::new("a8-clean");
    let file = t.0.join("ir.json");
    assert_eq!(compile_to(&common::ch1(), &file).status.code(), Some(0));
    assert_eq!(names_in(&t.0), vec![OsString::from("ir.json")]);
}
