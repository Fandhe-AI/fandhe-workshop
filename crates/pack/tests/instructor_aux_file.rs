//! 講師資料の補助ファイル `instructor/instructor.yaml` の受け口の固定（04-behavior の PACK-49、#748 の決定、TASK-74）。
//!
//! 講師資料（INSTR。次以降）が要する値は教材パックの補助ファイルに持つ。初期で持つ範囲は受け口だけで、
//! 補助ファイルを置いても `validate`・`compile` が診断 0 件で通り、無い教材パックの IR が受け口の前とバイト単位で
//! 同じことを、ここで固定する。製品のコード（core・web・pack の読み込み層）と語彙は変えていない。
//!
//! 範囲外: 中身の読み込み・不足の診断（`E-INSTR-*`。INSTR-18）と講師資料の生成。
//! 限界: 「中身を読まない」ことの固定（`pack49_aux_file_contents_are_not_read`）は、INSTR-18 で中身の検査が入ると
//! 置き換わる境界である。`metrics` は `toml`・`yaml` の通常ファイルを数えるので補助ファイルも CSV に出る
//! （観察として固定。扱いの変更は INSTR 側のタスクで決める）。`E-SECRET-SHAPE` は読み込んだ値だけを調べるため
//! 補助ファイルは対象外で、公開前の確認（`make check-internal-info-secrets`）が全ファイルを照合する。
//! 試験では `unwrap` を使ってよい。失敗の文言には教材名とバイト位置だけを出し、本文を出さない。

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{copy_dir, testdata, Tmp, WORKSHOP};
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::DiagCode;

const AUX_PATH: &str = "instructor/instructor.yaml";
const SENTINEL: &str = "PACK49-SENTINEL";

/// 見本の中身。章ごとの所要時間・模範の例・追加の声かけの形。番兵の文字列を 1 つ含める。
fn aux_sample() -> String {
    format!(
        "chapters:\n  - id: c1\n    minutes: 20\n    note: {SENTINEL}\nmodel_answers:\n  - step: a\n    text: sample\nprompts:\n  - when: stuck\n    say: look at the left pane\n"
    )
}

fn data(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(rel)
}

fn docs_example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples")
        .join(name)
}

/// `ext9_ir_identity.rs` の golden の組と、凍結物のある他の教材。
fn packs() -> Vec<(&'static str, PathBuf)> {
    let mut v = vec![
        (
            "min-3step-pack",
            testdata("engine-event-consistency/test-data/min-3step-pack"),
        ),
        ("ch1-pack", testdata("input-field-ime/test-data/ch1-pack")),
        ("ime-pack", testdata("input-field-ime/test-data/ime-pack")),
        ("min-3step-pack-b", data("min-3step-pack-b")),
        ("fixture-pack", data("fixture-pack")),
    ];
    for n in [
        "confirm-kinds-pack",
        "confirm-label-pack",
        "fill-pack",
        "input-kinds-pack",
        "judgment-pack",
        "slot-pack",
        "chosen-value-pack",
    ] {
        v.push((n, data(n)));
    }
    v
}

fn with_aux(src: &Path, tag: &str, body: &str) -> Tmp {
    let t = Tmp::new(tag);
    copy_dir(src, &t.0);
    t.put(AUX_PATH, body.as_bytes());
    t
}

fn first_diff(a: &[u8], b: &[u8]) -> usize {
    (0..a.len().min(b.len()))
        .find(|&i| a[i] != b[i])
        .unwrap_or(a.len().min(b.len()))
}

fn assert_identical(name: &str, got: &[u8], want: &[u8]) {
    assert!(
        got == want,
        "{name}: IR differs at byte {} (got {} bytes, want {} bytes)",
        first_diff(got, want),
        got.len(),
        want.len()
    );
}

fn minimal_toml_pack() -> Tmp {
    let t = Tmp::new("pack49-toml");
    t.put(
        "workshop.toml",
        b"[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n",
    );
    t.put(
        "chapters/c1/chapter.toml",
        b"id = \"c1\"\ntitle = \"C\"\n[[starts]]\nid = \"s1\"\nnode = \"a\"\n[[steps]]\nid = \"a\"\ntitle = \"A\"\nfinal = true\n",
    );
    t
}

fn minimal_yaml_pack() -> Tmp {
    let t = Tmp::new("pack49-min");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n  - {id: a, title: A, final: true}\n",
    );
    t
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
}

#[test]
fn pack49_aux_file_passes_validate_and_compile() {
    for (name, dir) in packs() {
        let t = with_aux(&dir, "pack49-ok", &aux_sample());
        // 元の教材が診断を持つ場合（ch1-pack など）もあるため、補助ファイルを足して増えないことを確かめる。
        let base: Vec<String> = validate_dir(&dir).iter().map(|d| d.to_string()).collect();
        let got: Vec<String> = validate_dir(&t.0).iter().map(|d| d.to_string()).collect();
        assert_eq!(got, base, "{name}: validate diagnostics changed");
        assert!(compile_dir(&t.0).is_ok(), "{name}: compile failed");
    }
}

#[test]
fn pack49_ir_is_byte_identical_with_and_without_aux_file() {
    for (name, dir) in packs() {
        let t = with_aux(&dir, "pack49-ir", &aux_sample());
        let without = compile_dir(&dir).unwrap();
        let with = compile_dir(&t.0).unwrap();
        assert_identical(name, &with, &without);
        let a: Ir = serde_json::from_slice(&without).unwrap();
        let b: Ir = serde_json::from_slice(&with).unwrap();
        assert_eq!(
            content_hash(&a).unwrap(),
            content_hash(&b).unwrap(),
            "{name}: content_hash"
        );
    }
}

#[test]
fn pack49_pack_without_trees_is_unchanged() {
    let plain = minimal_yaml_pack();
    let with = minimal_yaml_pack();
    with.put(AUX_PATH, aux_sample().as_bytes());
    assert!(validate_dir(&with.0).is_empty());
    assert_identical(
        "min-yaml",
        &compile_dir(&with.0).unwrap(),
        &compile_dir(&plain.0).unwrap(),
    );
}

#[test]
fn pack49_format_a_pack_accepts_yaml_aux_file() {
    let plain = minimal_toml_pack();
    let with = minimal_toml_pack();
    with.put(AUX_PATH, aux_sample().as_bytes());
    assert!(validate_dir(&with.0).is_empty());
    assert_identical(
        "min-toml",
        &compile_dir(&with.0).unwrap(),
        &compile_dir(&plain.0).unwrap(),
    );
}

/// INSTR-18 で中身の検査が入ると置き換わる境界。いまは中身を解釈しないので、不正な YAML でも失敗の経路が増えない。
#[test]
fn pack49_aux_file_contents_are_not_read() {
    let plain = minimal_yaml_pack();
    let want = compile_dir(&plain.0).unwrap();
    for (label, body) in [
        ("unclosed", "a: [1, 2\nb: {x"),
        ("duplicate-key", "a: 1\na: 2\n"),
        ("anchor", "a: &x 1\nb: *x\nc: *undefined\n"),
        ("empty", ""),
        ("binary", "\u{0}\u{1}\u{2}"),
    ] {
        let t = minimal_yaml_pack();
        t.put(AUX_PATH, body.as_bytes());
        assert!(validate_dir(&t.0).is_empty(), "{label}: diagnostics");
        assert_identical(label, &compile_dir(&t.0).unwrap(), &want);
    }
}

#[test]
fn pack49_aux_values_do_not_reach_ir_or_output() {
    let t = minimal_yaml_pack();
    t.put(AUX_PATH, aux_sample().as_bytes());
    let ir = compile_dir(&t.0).unwrap();
    assert!(!String::from_utf8_lossy(&ir).contains(SENTINEL));
    for cmd in ["validate", "compile"] {
        let o = bin().arg(cmd).arg(&t.0).output().unwrap();
        assert!(
            !String::from_utf8_lossy(&o.stdout).contains(SENTINEL),
            "{cmd}"
        );
        assert!(
            !String::from_utf8_lossy(&o.stderr).contains(SENTINEL),
            "{cmd}"
        );
    }
}

#[test]
fn pack49_cli_validate_compile_compare_test_exit_ok() {
    let src = docs_example("minimal-pack");
    let t = with_aux(&src, "pack49-cli", &aux_sample());

    let v = bin().arg("validate").arg(&t.0).output().unwrap();
    assert_eq!(v.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&v.stdout).contains("診断 0 件"));

    let c = bin().arg("compile").arg(&t.0).output().unwrap();
    assert_eq!(c.status.code(), Some(0));
    assert_identical("compile", &c.stdout, &compile_dir(&t.0).unwrap());

    let cmp = bin().arg("compare").arg(&src).arg(&t.0).output().unwrap();
    assert_eq!(cmp.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&cmp.stdout).contains("IR が一致しました"));

    let r = bin().arg("test").arg(&t.0).output().unwrap();
    assert_eq!(r.status.code(), Some(0));
}

/// 受け口が上限の検査を素通りさせないこと。`instructor/` の下のシンボリックリンクは拒否される。
#[cfg(unix)]
#[test]
fn pack49_limits_still_apply_under_instructor_dir() {
    let t = minimal_yaml_pack();
    let outside = Tmp::new("pack49-outside");
    outside.put("target.yaml", b"a: 1\n");
    std::fs::create_dir_all(t.0.join("instructor")).unwrap();
    std::os::unix::fs::symlink(outside.0.join("target.yaml"), t.0.join(AUX_PATH)).unwrap();
    let d = validate_dir(&t.0);
    assert!(
        !d.is_empty() && d.iter().all(|x| x.code() == DiagCode::LimitSymlink),
        "expected only E-LIMIT-SYMLINK"
    );
}

/// 観察: `metrics` の CSV に補助ファイルの行が出る（`docs/cli.md` の「パック根の下の `toml`・`yaml`」のとおり）。
/// 扱いを変える場合は INSTR 側のタスクで、この期待と `docs/known-gaps.md` を合わせて直す。
#[test]
fn pack49_metrics_lists_aux_file_observation() {
    let t = minimal_yaml_pack();
    t.put(AUX_PATH, aux_sample().as_bytes());
    let o = bin().arg("metrics").arg(&t.0).output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&o.stdout).contains(AUX_PATH));
}
