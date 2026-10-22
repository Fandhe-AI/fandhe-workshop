//! `compile` の章の範囲の指定と、他の章への `next` の拒否の受け入れ試験（TASK-67。PACK-43・SHELL-12・D-67）。
//!
//! 絞った IR が範囲外の開始点・Step・実体を含まないこと、全章の指定が指定なしとバイト単位で同じこと、
//! 他の章への `next` が `validate`・`compile` の両方で `E-REF-STEP` になること、章の選択の誤りが終了コード 2 で
//! IR もファイルも出さないことを確かめる。一時ディレクトリは `common::Tmp` が作る。正は `docs/cli.md` 版 7 の 10 節。

mod common;

use std::ffi::OsStr;
use std::fs;
use std::process::{Command, Output};

use common::Tmp;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_pack::compile::{compile_dir, compile_dir_with, CompileError, CompileOptions};
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::DiagCode;

const CHOICE: &str =
    "input: {kind: choice, options: [{id: ok, label: OK}]}, confirm: {kind: none, reason: r}";

fn choice_step(id: &str, next: &str) -> String {
    format!(
        "-   {{id: {id}, title: T, next: [{{to: {next}}}], {CHOICE}, rules: [{{id: r-{id}, on: {{option: ok}}, outcome: {{kind: accept}}}}]}}\n"
    )
}

/// 3 章（c1・c2・c3）と 2 モジュール（m1・m2。m2 は m1 を入れ子で呼ぶ）の教材パック。
fn pack(c3_body: bool) -> Tmp {
    let t = Tmp::new("chapter-range");
    t.put(
        "workshop.yaml",
        b"pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}\n",
    );
    t.put(
        "chapters/c1/chapter.yaml",
        format!(
            "id: c1\ntitle: C1\nstarts: [{{id: s1, node: a}}]\nsteps:\n{}-   {{id: b, title: B, final: true}}\n",
            choice_step("a", "b")
        )
        .as_bytes(),
    );
    t.put(
        "chapters/c2/chapter.yaml",
        b"id: c2\ntitle: C2\nstarts: [{id: s2, node: x}]\nsteps:\n-   {id: x, title: X, use: {module: m1, instance: i1, args: {}}, next: [{to: y}]}\n-   {id: y, title: Y, final: true}\n",
    );
    let body = if c3_body { ", body: q.md" } else { "" };
    t.put(
        "chapters/c3/chapter.yaml",
        format!(
            "id: c3\ntitle: C3\nstarts: [{{id: s3, node: p}}]\nsteps:\n-   {{id: p, title: P, use: {{module: m2, instance: i2, args: {{}}}}, next: [{{to: q}}]}}\n-   {{id: q, title: Q, final: true{body}}}\n"
        )
        .as_bytes(),
    );
    if c3_body {
        t.put("chapters/c3/q.md", b"# q\n");
    }
    t.put(
        "modules/m1/module.yaml",
        format!(
            "id: m1\ntitle: M1\nstarts: [{{id: ms, node: m-mid}}]\nsteps:\n{}-   {{id: m-b, title: MB, final: true}}\n{}",
            choice_step("m-a", "m-b"),
            choice_step("m-mid", "m-b")
        )
        .as_bytes(),
    );
    t.put(
        "modules/m2/module.yaml",
        b"id: m2\ntitle: M2\nstarts: [{id: ns, node: n-a}]\nsteps:\n-   {id: n-a, title: NA, use: {module: m1, instance: j1, args: {}}, next: [{to: n-b}]}\n-   {id: n-b, title: NB, final: true}\n",
    );
    t
}

fn ids(v: &[&str]) -> Option<Vec<String>> {
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn narrowed(t: &Tmp, v: &[&str]) -> Vec<u8> {
    compile_dir_with(&t.0, &CompileOptions { chapters: ids(v) }).expect("compile")
}

fn bin(args: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .args(args)
        .output()
        .unwrap()
}

fn edit(t: &Tmp, rel: &str, from: &str, to: &str) {
    let path = t.0.join(rel);
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains(from), "{from}");
    fs::write(&path, text.replacen(from, to, 1)).unwrap();
}

#[test]
fn pack43_base_pack_is_clean() {
    let t = pack(true);
    assert!(validate_dir(&t.0).is_empty(), "{:?}", validate_dir(&t.0));
    compile_dir(&t.0).expect("compile");
}

#[test]
fn pack43_cross_chapter_next_is_ref_step() {
    let t = pack(false);
    // c1 の `a` が c2 の Step `y` を指す。
    edit(&t, "chapters/c1/chapter.yaml", "{to: b}", "{to: y}");
    for d in [
        validate_dir(&t.0),
        compile_dir(&t.0).expect_err("no IR on a cross-chapter next"),
    ] {
        let hits: Vec<_> = d.iter().filter(|d| d.code() == DiagCode::RefStep).collect();
        assert_eq!(hits.len(), 1, "{d:?}");
        assert_eq!(hits[0].file(), "chapters/c1/chapter.yaml");
        assert_eq!(hits[0].item(), "step:a");
    }
    let o = bin(&["compile".as_ref(), t.0.as_os_str()]);
    assert_eq!(o.status.code(), Some(1));
}

#[test]
fn pack43_undefined_target_is_not_doubled() {
    let t = pack(false);
    edit(&t, "chapters/c1/chapter.yaml", "{to: b}", "{to: nowhere}");
    let d = compile_dir(&t.0).unwrap_err();
    let hits: Vec<_> = d.iter().filter(|d| d.code() == DiagCode::RefStep).collect();
    assert_eq!(hits.len(), 1, "{d:?}");
}

#[test]
fn pack43_narrowed_ir_has_no_out_of_range_start() {
    let t = pack(false);
    let bytes = narrowed(&t, &["c1", "c2"]);
    let ir = load_ir(&bytes).expect("narrowed IR loads in core");
    assert!(!ir.starts.is_empty());
    for s in &ir.starts {
        let n = ir
            .nodes
            .iter()
            .find(|n| n.uid == s.node)
            .expect("start node");
        assert!(
            n.unit == "c1" || n.unit == "c2" || n.unit == "m1",
            "{s:?} -> {}",
            n.unit
        );
    }
    assert!(ir.starts.iter().all(|s| s.id != "s3" && s.id != "ns"));
    // 範囲外の章の node・m2 とその入れ子の実体は 0 件。範囲内の m1 の実体は残る。
    assert!(ir.nodes.iter().all(|n| n.unit != "c3" && n.unit != "m2"));
    assert!(ir
        .instances
        .iter()
        .all(|i| i.module.as_deref() != Some("m2")));
    assert!(ir.instances.iter().all(|i| !i.id.contains("j1")));
    assert!(ir
        .instances
        .iter()
        .any(|i| i.module.as_deref() == Some("m1")));
    // 辺・enter・exit_to はすべて残した node に解決する。
    for n in &ir.nodes {
        for r in n
            .step
            .next
            .iter()
            .map(|t| t.to.as_str())
            .chain(n.enter.as_deref())
            .chain(n.exit_to.as_deref())
        {
            assert!(ir.nodes.iter().any(|m| m.uid == r), "dangling {r}");
        }
    }
}

#[test]
fn pack43_all_chapters_is_byte_identical() {
    let t = pack(true);
    let full = compile_dir(&t.0).unwrap();
    assert_eq!(narrowed(&t, &["c1", "c2", "c3"]), full);
    assert_eq!(narrowed(&t, &["c3", "c1", "c2"]), full);
}

#[test]
fn pack43_narrowing_is_deterministic() {
    let t = pack(false);
    assert_eq!(narrowed(&t, &["c2"]), narrowed(&t, &["c2"]));
    let out = t.0.join("out.json");
    let o = bin(&[
        "compile".as_ref(),
        t.0.as_os_str(),
        "-o".as_ref(),
        out.as_os_str(),
        "--chapters".as_ref(),
        "c2".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(fs::read(&out).unwrap(), narrowed(&t, &["c2"]));
    let o = bin(&[
        "compile".as_ref(),
        t.0.as_os_str(),
        "--chapters".as_ref(),
        "c2".as_ref(),
    ]);
    assert_eq!(o.stdout, narrowed(&t, &["c2"]));
}

#[test]
fn pack43_format_version_recomputed() {
    let t = pack(true);
    let full = load_ir(&compile_dir(&t.0).unwrap()).unwrap();
    assert_eq!(full.format_version, 2);
    let ir = load_ir(&narrowed(&t, &["c1"])).unwrap();
    assert_eq!((ir.format_version, ir.pack.ir_format), (1, 1));
}

#[test]
fn pack43_unknown_or_ambiguous_chapter_exits_2() {
    let t = pack(false);
    let out = t.0.join("out.json");
    fs::write(&out, b"keep").unwrap();
    let run = |t: &Tmp, id: &str| {
        bin(&[
            "compile".as_ref(),
            t.0.as_os_str(),
            "--chapters".as_ref(),
            id.as_ref(),
            "-o".as_ref(),
            out.as_os_str(),
        ])
    };
    let o = run(&t, "nope");
    assert_eq!(o.status.code(), Some(2));
    assert!(o.stdout.is_empty());
    let err = String::from_utf8(o.stderr).unwrap();
    assert_eq!(err.lines().count(), 1, "{err}");
    assert!(err.starts_with("error:"), "{err}");
    assert_eq!(fs::read(&out).unwrap(), b"keep");
    // 同じ ID の章が 2 つ（c4 は id が c1 のまま、Step の ID だけ変えた写し）。
    let c4 = fs::read_to_string(t.0.join("chapters/c1/chapter.yaml"))
        .unwrap()
        .replace("s1", "s4")
        .replace("id: a,", "id: a4,")
        .replace("node: a}", "node: a4}")
        .replace("to: b}", "to: b4}")
        .replace("id: b,", "id: b4,")
        .replace("r-a", "r-a4");
    t.put("chapters/c4/chapter.yaml", c4.as_bytes());
    let o = run(&t, "c1");
    assert_eq!(
        o.status.code(),
        Some(2),
        "{:?}",
        String::from_utf8_lossy(&o.stdout)
    );
    assert!(o.stdout.is_empty());
    assert!(String::from_utf8(o.stderr).unwrap().contains("ambiguous"));
    assert_eq!(fs::read(&out).unwrap(), b"keep");
    let e = compile_dir_with(
        &t.0,
        &CompileOptions {
            chapters: ids(&["zz"]),
        },
    );
    assert!(matches!(e, Err(CompileError::Chapter(_))));
}

#[test]
fn pack43_foreign_start_into_selected_chapter_is_dropped() {
    let t = pack(false);
    // 範囲外の章 c2 が、選んだ章 c1 の Step を指す開始点を定義する。
    edit(
        &t,
        "chapters/c2/chapter.yaml",
        "starts: [{id: s2, node: x}]",
        "starts: [{id: s2, node: x}, {id: s2a, node: a}]",
    );
    let ir = load_ir(&narrowed(&t, &["c1"])).expect("ir");
    assert!(ir.starts.iter().all(|s| s.id != "s2a" && s.id != "s2"));
    assert!(ir.starts.iter().any(|s| s.id == "s1"));
}

#[test]
fn pack43_foreign_start_alone_does_not_make_selection_succeed() {
    let t = pack(false);
    edit(
        &t,
        "chapters/c1/chapter.yaml",
        "starts: [{id: s1, node: a}]\n",
        "",
    );
    edit(
        &t,
        "chapters/c2/chapter.yaml",
        "starts: [{id: s2, node: x}]",
        "starts: [{id: s2, node: x}, {id: s2a, node: a}]",
    );
    let e = compile_dir_with(
        &t.0,
        &CompileOptions {
            chapters: ids(&["c1"]),
        },
    );
    let Err(CompileError::Diagnostics(d)) = e else {
        panic!("expected diagnostics");
    };
    assert_eq!(d[0].code(), DiagCode::Structure);
}

#[test]
fn pack43_chapters_without_start_is_structure() {
    let t = pack(false);
    edit(
        &t,
        "chapters/c1/chapter.yaml",
        "starts: [{id: s1, node: a}]\n",
        "",
    );
    let e = compile_dir_with(
        &t.0,
        &CompileOptions {
            chapters: ids(&["c1"]),
        },
    );
    let Err(CompileError::Diagnostics(d)) = e else {
        panic!("expected diagnostics");
    };
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code(), DiagCode::Structure);
    let o = bin(&[
        "compile".as_ref(),
        t.0.as_os_str(),
        "--chapters".as_ref(),
        "c1".as_ref(),
    ]);
    assert_eq!(o.status.code(), Some(1));
}

#[test]
fn pack43_option_shape_errors_exit_2() {
    let t = pack(false);
    let d = t.0.as_os_str();
    let cases: Vec<Vec<&str>> = vec![
        vec!["--chapters"],
        vec!["--chapters", ""],
        vec!["--chapters", "c1,,c2"],
        vec!["--chapters", "c1,"],
        vec!["--chapters", "c1,c1"],
        vec!["--chapters", "c1", "--chapters", "c2"],
        vec!["--chapters", "-c1"],
    ];
    for c in cases {
        let mut args: Vec<&OsStr> = vec!["compile".as_ref(), d];
        args.extend(c.iter().map(|s| OsStr::new(*s)));
        let o = bin(&args);
        assert_eq!(o.status.code(), Some(2), "{c:?}");
        assert!(o.stdout.is_empty(), "{c:?}");
        assert!(
            String::from_utf8_lossy(&o.stderr).starts_with("error:"),
            "{c:?}"
        );
    }
    // `-o` との順不同は受け付ける。
    let out = t.0.join("o.json");
    let o = bin(&[
        "compile".as_ref(),
        d,
        "--chapters".as_ref(),
        "c1".as_ref(),
        "-o".as_ref(),
        out.as_os_str(),
    ]);
    assert_eq!(o.status.code(), Some(0));
}

#[test]
fn pack43_pack_diagnostics_are_unchanged_by_selection() {
    let t = pack(false);
    // 範囲外の c3 に未定義の宛先を入れても、c1 だけを選んだ compile が同じ診断で止まる。
    edit(&t, "chapters/c3/chapter.yaml", "{to: q}", "{to: nowhere}");
    let full = compile_dir(&t.0).unwrap_err();
    let Err(CompileError::Diagnostics(sel)) = compile_dir_with(
        &t.0,
        &CompileOptions {
            chapters: ids(&["c1"]),
        },
    ) else {
        panic!("expected diagnostics");
    };
    assert_eq!(format!("{full:?}"), format!("{sel:?}"));
}
