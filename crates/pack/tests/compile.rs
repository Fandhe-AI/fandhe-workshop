//! 読み込み層（`source::load`）と `compile` の結合試験。
//!
//! 04-behavior の PACK-1（実行時は IR だけで足りる）・PACK-24（決定的な出力）・PACK-26 の前提を、
//! `testdata/` の実パックと、一時ディレクトリに作る最小パックで確かめる。負の試験は、パックの外に
//! 置いた目印のファイルが読まれないこと、診断に本文の中身が出ないことも確かめる。
//! 一時ディレクトリは `CARGO_TARGET_TMPDIR` の下に作り、終わったら消す。作業ツリーは書き換えない。
//! `compare` とバイト一致の本格的な試験は #98。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use fandhe_workshop_core::load_ir;
use fandhe_workshop_pack::compile::{compile, compile_dir, to_json};
use fandhe_workshop_pack::source::load;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const SECRET: &str = "SECRET-BODY-MARKER-4711";

fn testdata(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(rel)
}

fn ch1() -> PathBuf {
    testdata("input-field-ime/test-data/ch1-pack")
}

/// 一意な一時ディレクトリ。Drop で消す。
struct Tmp(PathBuf);

impl Tmp {
    fn new(tag: &str) -> Tmp {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "compile-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).expect("create tmp dir");
        Tmp(p)
    }

    fn put(&self, rel: &str, content: &[u8]) {
        let p = self.0.join(rel);
        fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
        fs::write(p, content).expect("write");
    }
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("mkdir");
    for e in fs::read_dir(from).expect("read_dir").flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            copy_dir(&src, &dst);
        } else {
            fs::copy(&src, &dst).expect("copy");
        }
    }
}

const WORKSHOP: &str = "pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}\n";
const CHAPTER: &str = concat!(
    "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n",
    "  - {id: a, title: A, body: a.md, next: [{to: b}]}\n",
    "  - {id: b, title: B, final: true}\n",
);

/// 正しい最小パック（形式 B）。
fn minimal() -> Tmp {
    let t = Tmp::new("min");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put("chapters/c1/chapter.yaml", CHAPTER.as_bytes());
    t.put("chapters/c1/a.md", format!("# a\n{SECRET}\n").as_bytes());
    t
}

fn diags(t: &Tmp) -> Vec<Diagnostic> {
    match compile_dir(&t.0) {
        Ok(_) => panic!("expected diagnostics"),
        Err(d) => d,
    }
}

fn codes(d: &[Diagnostic]) -> Vec<DiagCode> {
    d.iter().map(Diagnostic::code).collect()
}

fn assert_no_secret(d: &[Diagnostic]) {
    for x in d {
        let s = x.to_string();
        assert!(!s.contains(SECRET), "diagnostic leaks body: {s}");
    }
}

#[test]
fn pack1_ir_alone_is_enough() {
    let tmp = Tmp::new("ch1");
    copy_dir(&ch1(), &tmp.0);
    let bytes = compile_dir(&tmp.0).expect("compile ch1");
    drop(tmp); // パックを消した後でも、IR だけで読める。
    let ir = load_ir(&bytes).expect("load_ir");
    let golden =
        load_ir(&fs::read(testdata("input-field-ime/test-data/ch1-pack.json")).expect("golden"))
            .expect("golden loads");
    let uids = |i: &fandhe_workshop_contract::ir::Ir| -> Vec<String> {
        i.nodes.iter().map(|n| n.uid.clone()).collect()
    };
    assert_eq!(uids(&ir), uids(&golden));
    let starts = |i: &fandhe_workshop_contract::ir::Ir| -> Vec<(String, String)> {
        i.starts
            .iter()
            .map(|s| (s.id.clone(), s.node.clone()))
            .collect()
    };
    assert_eq!(starts(&ir), starts(&golden));
    let nexts = |i: &fandhe_workshop_contract::ir::Ir| -> Vec<Vec<String>> {
        i.nodes
            .iter()
            .map(|n| n.step.next.iter().map(|t| t.to.clone()).collect())
            .collect()
    };
    assert_eq!(nexts(&ir), nexts(&golden));
}

#[test]
fn pack1_format_b_ime_pack_compiles() {
    let bytes = compile_dir(&testdata("input-field-ime/test-data/ime-pack")).expect("compile");
    assert!(load_ir(&bytes).is_ok());
}

#[test]
fn pack1_format_a_min_3step_uids() {
    let bytes = compile_dir(&testdata(
        "engine-event-consistency/test-data/min-3step-pack",
    ))
    .expect("compile");
    let ir = load_ir(&bytes).expect("load_ir");
    let uids: Vec<&str> = ir.nodes.iter().map(|n| n.uid.as_str()).collect();
    assert_eq!(uids, ["main/st-1", "main/st-2", "main/st-3"]);
    assert_eq!(ir.starts[0].node, "main/st-1");
}

#[test]
fn pack24_output_is_deterministic_and_path_free() {
    let a = compile_dir(&ch1()).expect("a");
    let b = compile_dir(&ch1()).expect("b");
    let tmp = Tmp::new("det");
    copy_dir(&ch1(), &tmp.0);
    let c = compile_dir(&tmp.0).expect("c");
    let d = compile_dir(&tmp.0).expect("d");
    assert!(a == b && b == c && c == d);
    let text = String::from_utf8(c).expect("utf8");
    assert!(!text.contains(tmp.0.to_str().expect("utf8 path")));
    assert!(text.ends_with("}\n") && !text.ends_with("\n\n"));
}

#[test]
fn pack24_map_keys_are_sorted_in_output() {
    let bytes = compile_dir(&ch1()).expect("compile");
    let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    let text = String::from_utf8(bytes).expect("utf8");
    // 連想配列のキーが、出力の中で昇順に並ぶ（BTreeMap の順）。
    for table in ["rejections", "commands", "hints"] {
        let mut keys: Vec<&String> = v[table].as_object().expect("object").keys().collect();
        keys.sort();
        let start = text
            .find(&format!("\n  \"{table}\""))
            .expect("table present");
        let mut last = start;
        for k in keys {
            let pos = start + text[start..].find(&format!("\n    \"{k}\"")).expect("key");
            assert!(pos >= last, "{table}.{k} out of order");
            last = pos;
        }
    }
}

#[test]
fn pack1_module_pack_loads_but_does_not_expand() {
    let dir = testdata("module-reuse-rounds/test-data/pack");
    let pack = load(&dir).expect("load");
    let modules = pack
        .scopes
        .iter()
        .filter(|s| s.kind == fandhe_workshop_pack::source::ScopeKind::Module)
        .count();
    assert_eq!(modules, 2);
    let d = compile(&pack).expect_err("use is not expanded");
    // `use` の Step は node にならないため、それへの遷移は E-REF-STEP としても報告される。
    assert!(d
        .iter()
        .all(|x| matches!(x.code(), DiagCode::Structure | DiagCode::RefStep)));
    assert!(d.iter().any(|x| x.code() == DiagCode::Structure));
}

#[test]
fn minimal_pack_compiles() {
    let t = minimal();
    let pack = load(&t.0).expect("load");
    assert!(pack.bodies.contains_key("chapters/c1/a.md"));
    let ir = compile(&pack).expect("compile");
    let json = to_json(&ir).expect("json");
    assert!(load_ir(&json).is_ok());
}

#[test]
fn neg_workshop_format() {
    let t = minimal();
    t.put("workshop.toml", b"[pack]\n");
    assert_eq!(codes(&diags(&t)), [DiagCode::Parse]);
    let t = Tmp::new("none");
    assert_eq!(codes(&diags(&t)), [DiagCode::Parse]);
    let d = compile_dir(&t.0.join("missing")).expect_err("no dir");
    assert_eq!(codes(&d), [DiagCode::Parse]);
}

#[test]
fn neg_unknown_step_field() {
    let t = minimal();
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nsteps:\n  - {id: a, title: A, script: x}\n",
    );
    assert_eq!(codes(&diags(&t)), [DiagCode::Parse]);
}

#[test]
fn neg_body_refs() {
    for body in ["missing.md", "../outside.md", "/etc/hostname", "..\\x.md"] {
        let t = minimal();
        t.put("outside.md", SECRET.as_bytes());
        t.put("chapters/outside.md", SECRET.as_bytes());
        let ch = CHAPTER.replace(
            "body: a.md",
            &format!("body: '{}'", body.replace('\\', "\\\\")),
        );
        t.put("chapters/c1/chapter.yaml", ch.as_bytes());
        let d = diags(&t);
        assert_eq!(codes(&d), [DiagCode::RefBody], "{body}");
        assert_no_secret(&d);
    }
}

#[test]
fn neg_body_not_utf8_and_too_large() {
    let t = minimal();
    t.put("chapters/c1/a.md", &[0xff, 0xfe, 0x00]);
    assert_eq!(codes(&diags(&t)), [DiagCode::Parse]);

    let t = minimal();
    t.put("chapters/c1/a.md", &vec![b'a'; 512 * 1024 + 1]);
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::LimitMdSize]);

    let t = minimal();
    t.put("chapters/c1/a.md", &vec![b'a'; 512 * 1024]);
    assert!(compile_dir(&t.0).is_ok());
}

#[cfg(unix)]
#[test]
fn neg_symlink_is_rejected_without_following() {
    let t = minimal();
    let outside = Tmp::new("outside");
    outside.put("target.md", SECRET.as_bytes());
    std::os::unix::fs::symlink(outside.0.join("target.md"), t.0.join("chapters/c1/link.md"))
        .expect("symlink");
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::LimitSymlink]);
    assert_no_secret(&d);
}

#[test]
fn neg_fixture_refs() {
    for path in ["fixtures/missing.txt", "../outside.txt", "other/x.txt"] {
        let t = minimal();
        t.put("outside.txt", SECRET.as_bytes());
        t.put("other/x.txt", SECRET.as_bytes());
        let ws = format!("{WORKSHOP}fixtures:\n  f1: {{path: '{path}', kind: document}}\n");
        t.put("workshop.yaml", ws.as_bytes());
        let d = diags(&t);
        assert_eq!(codes(&d), [DiagCode::RefFixture], "{path}");
        assert_no_secret(&d);
    }
    let t = minimal();
    t.put("fixtures/ok.txt", b"x");
    let ws = format!("{WORKSHOP}fixtures:\n  f1: {{path: fixtures/ok.txt, kind: document}}\n");
    t.put("workshop.yaml", ws.as_bytes());
    assert!(compile_dir(&t.0).is_ok());
}

#[test]
fn neg_ir_format() {
    let t = minimal();
    t.put(
        "workshop.yaml",
        WORKSHOP.replace("ir_format: 1", "ir_format: 2").as_bytes(),
    );
    assert_eq!(codes(&diags(&t)), [DiagCode::FormatVersion]);
}

#[test]
fn neg_unresolved_refs_and_duplicates() {
    let t = minimal();
    t.put(
        "chapters/c1/chapter.yaml",
        CHAPTER.replace("to: b", "to: zz").as_bytes(),
    );
    assert_eq!(codes(&diags(&t)), [DiagCode::RefStep]);

    let t = minimal();
    t.put(
        "chapters/c1/chapter.yaml",
        CHAPTER.replace("node: a", "node: zz").as_bytes(),
    );
    assert_eq!(codes(&diags(&t)), [DiagCode::RefStep]);

    let t = minimal();
    t.put(
        "chapters/c1/chapter.yaml",
        CHAPTER.replace("{id: b,", "{id: a,").as_bytes(),
    );
    assert!(codes(&diags(&t)).contains(&DiagCode::DupId));
}

#[test]
fn neg_chapter_without_definition() {
    let t = minimal();
    t.put("chapters/empty/readme.md", b"x");
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::Structure]);
    assert_eq!(d[0].file(), "chapters/empty");
}

#[test]
fn neg_diagnostics_do_not_contain_absolute_paths() {
    let t = minimal();
    t.put("chapters/c1/a.md", &[0xff]);
    for d in diags(&t) {
        assert!(!d.to_string().contains(t.0.to_str().expect("utf8 path")));
    }
}

#[test]
fn neg_transition_target_shows_original_id() {
    let t = minimal();
    t.put(
        "chapters/c1/chapter.yaml",
        CHAPTER.replace("to: b", "to: zz").as_bytes(),
    );
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::RefStep]);
    assert!(d[0].message().ends_with(": zz"), "{}", d[0].message());
}

#[test]
fn neg_duplicate_step_blames_first_definition_file() {
    let t = minimal();
    // c1 の a は未定義の遷移先を持つ。c2 は同じ ID の a を重複して定義する。
    t.put(
        "chapters/c1/chapter.yaml",
        CHAPTER.replace("to: b", "to: zz").as_bytes(),
    );
    t.put(
        "chapters/c2/chapter.yaml",
        b"id: c2\ntitle: C2\nsteps:\n  - {id: a, title: A, final: true}\n",
    );
    let d = diags(&t);
    let r = d
        .iter()
        .find(|x| x.code() == DiagCode::RefStep)
        .expect("ref-step");
    assert_eq!(r.file(), "chapters/c1/chapter.yaml");
}

#[test]
fn neg_use_step_is_not_a_resolved_target_and_is_checked() {
    let t = minimal();
    t.put(
        "chapters/c1/chapter.yaml",
        concat!(
            "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n",
            "  - {id: a, title: A, body: a.md, next: [{to: u}]}\n",
            "  - {id: u, title: U, next: [{to: zz}], use: {module: m, instance: i, args: {}}}\n",
        )
        .as_bytes(),
    );
    let d = diags(&t);
    let refs: Vec<&str> = d
        .iter()
        .filter(|x| x.code() == DiagCode::RefStep)
        .map(Diagnostic::message)
        .collect();
    assert!(refs.iter().any(|m| m.ends_with(": u")), "{refs:?}");
    assert!(refs.iter().any(|m| m.ends_with(": zz")), "{refs:?}");
}
