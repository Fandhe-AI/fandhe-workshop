//! `fandhe-pack` の実行ファイルを起動する結合試験。

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
}

#[test]
fn i1_subcommands_start() {
    let o = bin().args(["test", "/nonexistent/path"]).output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stderr.is_empty());
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.ends_with("NG: 診断 1 件（読み込めません）\n"), "{out}");
}

/// 形が正しい呼び出しは、そのコマンドの handler に届く。`test` は本体に届き、未実装の案内を出さない（TASK-404 1/3）。
#[test]
fn i10_each_command_is_dispatched_to_its_handler() {
    let o = bin().args(["test", "/nonexistent/path"]).output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(!err.contains("not implemented"), "{err}");
}

/// `vocab` は自分の handler に届く（標準出力の 1 行目が `VOCAB_HEADER`）。他のコマンドの振り分けとは取り違えない。
#[test]
fn i11_vocab_is_dispatched_to_vocab() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/vocab-pack");
    let o = bin().arg("vocab").arg(dir).output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert!(o.stderr.is_empty());
    let out = String::from_utf8(o.stdout).unwrap();
    assert_eq!(
        out.lines().next(),
        Some(fandhe_workshop_pack::cli_contract::VOCAB_HEADER)
    );
}

#[test]
fn i2_no_args_prints_usage() {
    let o = bin().output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).starts_with("usage: fandhe-pack"));
}

#[test]
fn i3_unknown_subcommand() {
    let o = bin().arg("bogus").output().unwrap();
    assert_eq!(o.status.code(), Some(2));
}

fn ch1_pack() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/input-field-ime/test-data/ch1-pack")
}

#[test]
fn i4_compile_prints_ir() {
    let o = bin().arg("compile").arg(ch1_pack()).output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert!(o.stderr.is_empty());
    assert!(o.stdout.ends_with(b"}\n") && !o.stdout.ends_with(b"\n\n"));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    // 本文を持つパックは、最小の版が 2 になる（TASK-60）。
    assert_eq!(v["format_version"], 2);
}

#[test]
fn i5_compile_missing_dir_is_diagnostic() {
    let o = bin()
        .args(["compile", "/nonexistent/path"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stdout).contains("E-PARSE"));
}

#[test]
fn i6_compile_argument_errors() {
    let o = bin().arg("compile").output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    let o = bin()
        .arg("compile")
        .arg(ch1_pack())
        .arg("-o")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(o.stdout.is_empty());
    let o = bin()
        .args(["compile", "-o", "x"])
        .arg(ch1_pack())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(o.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&o.stderr).contains("not implemented"));
}

fn min_3step_a() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step-pack")
}

fn min_3step_b() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/min-3step-pack-b")
}

#[test]
fn i7_compare_same_content_is_ok() {
    let o = bin()
        .arg("compare")
        .arg(min_3step_a())
        .arg(min_3step_b())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert!(o.stderr.is_empty());
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.starts_with("OK: IR が一致しました（"), "{out}");
}

#[test]
fn i8_compare_different_packs_is_ng() {
    let o = bin()
        .arg("compare")
        .arg(ch1_pack())
        .arg(min_3step_a())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.starts_with("NG: IR が一致しません（"), "{out}");
}

#[test]
fn i9_compare_missing_dir_is_diagnostic_not_verdict() {
    let o = bin()
        .arg("compare")
        .arg(ch1_pack())
        .arg("/nonexistent/path")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("E-PARSE"), "{out}");
    assert!(!out.contains("OK:") && !out.contains("一致しま"), "{out}");
    assert!(
        out.lines().last().is_some_and(|l| l.starts_with("NG: 診断 ") && l.contains("比べられません。a 0 件、b ")),
        "{out}"
    );
}

#[test]
fn i10_compare_argument_errors() {
    let a = ch1_pack();
    assert_eq!(
        bin().arg("compare").output().unwrap().status.code(),
        Some(2)
    );
    let o = bin().arg("compare").arg(&a).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    let o = bin()
        .arg("compare")
        .arg(&a)
        .arg(&a)
        .arg(&a)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(o.stdout.is_empty());
}

/// PACK-2: 両方ある・どちらも無いは終了コード 1 で `E-PARSE` を出す。
#[test]
fn pack2_cli_exit_code_for_both_and_neither() {
    let base = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("pack2-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let both = base.join("both");
    let none = base.join("none");
    std::fs::create_dir_all(&both).unwrap();
    std::fs::create_dir_all(&none).unwrap();
    std::fs::write(both.join("workshop.yaml"), "pack: {}\n").unwrap();
    std::fs::write(both.join("workshop.toml"), "[pack]\n").unwrap();
    for dir in [&both, &none] {
        let o = bin().arg("compile").arg(dir).output().unwrap();
        assert_eq!(o.status.code(), Some(1), "{}", dir.display());
        assert!(String::from_utf8_lossy(&o.stdout).contains("E-PARSE"));
    }
    let _ = std::fs::remove_dir_all(&base);
}

/// PACK-5: 10 進でない整数を書いたパックは終了コード 1 で `E-PARSE` を出し、IR を出さない。
/// `validate` も同じ読み込みを通る（`validate_command.rs`）。ここでは `compile` で確かめる。
#[test]
fn pack5_cli_radix_literal_is_diagnostic_and_emits_no_ir() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("pack5-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("workshop.yaml"),
        "pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}\ncommands:\n  x: {n: 0x1f}\n",
    )
    .unwrap();
    let o = bin().arg("compile").arg(&dir).output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("E-PARSE"), "{out}");
    assert!(!out.contains("\"ir_format\""), "{out}");
    assert!(!out.contains("0x1f"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// PACK-5: アンカー・エイリアス・タグを書いたパックは終了コード 1 で `E-PARSE` を出し、IR を出さない。
#[test]
fn pack5_cli_anchor_and_tag_are_diagnostic_and_emit_no_ir() {
    for (tag, commands) in [
        ("anchor", "x: &anchor_name {n: 1}\n  y: *anchor_name"),
        ("tag", "x: !!int 31"),
    ] {
        let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("pack5-cli-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("workshop.yaml"),
            format!("pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: A}}\ncommands:\n  {commands}\n"),
        )
        .unwrap();
        let o = bin().arg("compile").arg(&dir).output().unwrap();
        assert_eq!(o.status.code(), Some(1), "{tag}");
        let out = String::from_utf8_lossy(&o.stdout);
        assert!(out.contains("E-PARSE"), "{tag}: {out}");
        assert!(!out.contains("\"ir_format\""), "{tag}: {out}");
        assert!(!out.contains("anchor_name"), "{tag}: {out}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// PACK-25: 開始点が 0 件のパックは終了コード 1 で `E-STRUCTURE` を出し、IR を出さない。
#[test]
fn pack25_cli_no_start_is_diagnostic_and_emits_no_ir() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("pack25-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("chapters/c1")).unwrap();
    std::fs::write(
        dir.join("workshop.yaml"),
        "pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("chapters/c1/chapter.yaml"),
        "id: c1\ntitle: C\nsteps:\n  - {id: a, title: A, final: true}\n",
    )
    .unwrap();
    let o = bin().arg("compile").arg(&dir).output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("E-STRUCTURE"), "{out}");
    assert!(!out.contains("\"ir_format\""), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// SAFE-22: 教材パック全体の上限（20MB）を 1 バイト超えると、終了コード 1 で `E-LIMIT-PACK-SIZE` を出し、
/// IR を出さない。巨大な実体を書かないよう疎ファイルで作る。
#[test]
fn i11_compile_pack_over_limit_is_diagnostic() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("i11-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("workshop.yaml"), "pack: {}\n").unwrap();
    std::fs::File::create(dir.join("pad.bin"))
        .unwrap()
        .set_len(20_971_520 + 1)
        .unwrap();
    let o = bin().arg("compile").arg(&dir).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(o.status.code(), Some(1));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("E-LIMIT-PACK-SIZE"), "{out}");
    assert!(!out.contains("\"ir_format\""), "{out}");
}

#[test]
fn i12_compile_dir_too_deep_is_diagnostic() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("i12-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut deep = dir.join("deep");
    for _ in 1..33 {
        deep = deep.join("d");
    }
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(dir.join("workshop.yaml"), "pack: {}\n").unwrap();
    let o = bin().arg("compile").arg(&dir).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(o.status.code(), Some(1));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("E-LIMIT-DEPTH"), "{out}");
    assert!(!out.contains("\"ir_format\""), "{out}");
}

#[cfg(unix)]
#[test]
fn i13_compile_symlink_is_diagnostic() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("i13-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("workshop.yaml"), "pack: {}\n").unwrap();
    std::os::unix::fs::symlink("nowhere", dir.join("link")).unwrap();
    let o = bin().arg("compile").arg(&dir).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(o.status.code(), Some(1));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("E-LIMIT-SYMLINK"), "{out}");
    assert!(!out.contains("\"ir_format\""), "{out}");
}

#[test]
fn i14_compile_markdown_over_limit_is_diagnostic() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("i14-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("workshop.yaml"), "pack: {}\n").unwrap();
    std::fs::write(dir.join("unused.md"), vec![b'a'; 524_289]).unwrap();
    let o = bin().arg("compile").arg(&dir).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(o.status.code(), Some(1));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("E-LIMIT-MD-SIZE"), "{out}");
    assert!(!out.contains("\"ir_format\""), "{out}");
}

#[test]
fn i15_compile_non_md_body_over_limit_is_e_limit() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("i15-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("chapters/c1")).unwrap();
    std::fs::write(
        dir.join("workshop.yaml"),
        "pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("chapters/c1/chapter.yaml"),
        concat!(
            "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n",
            "  - {id: a, title: A, body: b.txt, next: [{to: b}]}\n",
            "  - {id: b, title: B, final: true}\n",
        ),
    )
    .unwrap();
    std::fs::write(dir.join("chapters/c1/b.txt"), vec![b'a'; 524_289]).unwrap();
    let o = bin().arg("compile").arg(&dir).output().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(o.status.code(), Some(1));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains(" E-LIMIT: "), "{out}");
    assert!(!out.contains("\"ir_format\""), "{out}");
}

/// `validate` は実装済みで、未実装の案内ではなく診断の行と最終行を標準出力に書く（TASK-403）。
#[test]
fn i16_validate_is_dispatched_to_validate() {
    let o = bin()
        .args(["validate", "/nonexistent/path"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("E-PARSE"), "{out}");
    assert_eq!(out.lines().last(), Some("NG: 診断 1 件（読み込めません）"));
    assert!(o.stderr.is_empty());
    assert!(!out.contains("not implemented"));
}
