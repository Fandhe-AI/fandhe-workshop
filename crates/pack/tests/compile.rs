//! 読み込み層（`source::load`）と `compile` の結合試験。
//!
//! 04-behavior の PACK-1（実行時は IR だけで足りる）・PACK-24（決定的な出力）・PACK-26 の前提を、
//! `testdata/` の実パックと、一時ディレクトリに作る最小パックで確かめる。負の試験は、パックの外に
//! 置いた目印のファイルが読まれないこと、診断に本文の中身が出ないことも確かめる。
//! 一時ディレクトリは `CARGO_TARGET_TMPDIR` の下に作り、終わったら消す。作業ツリーは書き換えない。
//! `compare` とバイト一致の試験は `tests/compare.rs`。

mod common;

use std::fs;

use common::{assert_no_secret, ch1, copy_dir, minimal, testdata, Tmp, CHAPTER, SECRET, WORKSHOP};

use fandhe_workshop_contract::limits::MAX_MARKDOWN_BYTES;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_pack::compile::{compile, compile_dir, to_json};
use fandhe_workshop_pack::source::load;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

fn diags(t: &Tmp) -> Vec<Diagnostic> {
    match compile_dir(&t.0) {
        Ok(_) => panic!("expected diagnostics"),
        Err(d) => d,
    }
}

fn codes(d: &[Diagnostic]) -> Vec<DiagCode> {
    d.iter().map(Diagnostic::code).collect()
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
fn pack19_module_pack_expands() {
    let dir = testdata("module-reuse-rounds/test-data/pack");
    let pack = load(&dir).expect("load");
    let modules = pack
        .scopes
        .iter()
        .filter(|s| s.kind == fandhe_workshop_pack::source::ScopeKind::Module)
        .count();
    assert_eq!(modules, 2);
    let ir = compile(&pack).expect("module pack expands");
    assert_eq!(ir.nodes.len(), 36);
    assert_eq!(ir.instances.len(), 5);
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
fn neg_workshop_path_is_not_a_directory() {
    let d = compile_dir(&Tmp::new("none").0.join("missing")).expect_err("no dir");
    assert_eq!(codes(&d), [DiagCode::Parse]);
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
    // md の上限ちょうどは md の検査を通るが、IR に入ると 1 文字列が文字列長の上限（100,000 文字）を超える。
    t.put("chapters/c1/a.md", &vec![b'a'; 512 * 1024]);
    assert_eq!(codes(&diags(&t)), [DiagCode::Limit]);

    // 文字列長の上限ちょうどの本文は通る。
    let t = minimal();
    t.put("chapters/c1/a.md", &vec![b'a'; 100_000]);
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
        WORKSHOP.replace("ir_format: 1", "ir_format: 3").as_bytes(),
    );
    assert_eq!(codes(&diags(&t)), [DiagCode::FormatVersion]);
}

/// PACK-24 の読み: `workshop.*` の `ir_format` は 1 と 2 を受け付ける。出力の版は使った項目で決まる。
#[test]
fn ir_format_two_is_accepted_and_output_version_follows_the_content() {
    let t = minimal();
    t.put(
        "workshop.yaml",
        WORKSHOP.replace("ir_format: 1", "ir_format: 2").as_bytes(),
    );
    let ir = load_ir(&compile_dir(&t.0).expect("ir_format 2 compiles")).expect("loads");
    // `minimal` の本文は木になるので、宣言が 1 でも 2 でも出力は 2。
    assert_eq!((ir.format_version, ir.pack.ir_format), (2, 2));
    let t = minimal();
    let ir = load_ir(&compile_dir(&t.0).expect("ir_format 1 compiles")).expect("loads");
    assert_eq!((ir.format_version, ir.pack.ir_format), (2, 2));
}

/// 練習データの内容を読むときの診断に、内容を入れない。UTF-8 でない・上限を超える練習データは断る。
#[test]
fn neg_fixture_content_is_diagnosed_without_echo() {
    let ws = format!("{WORKSHOP}fixtures:\n  f1: {{path: fixtures/x.md, kind: document}}\n");
    let t = minimal();
    t.put("workshop.yaml", ws.as_bytes());
    let mut bad = SECRET.as_bytes().to_vec();
    bad.push(0xff);
    t.put("fixtures/x.md", &bad);
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::Parse]);
    assert_no_secret(&d);

    let t = minimal();
    let ws = format!("{WORKSHOP}fixtures:\n  f1: {{path: fixtures/x.txt, kind: log}}\n");
    t.put("workshop.yaml", ws.as_bytes());
    t.put("fixtures/x.txt", &bad);
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::Parse]);
    assert_no_secret(&d);

    let t = minimal();
    let ws = format!("{WORKSHOP}fixtures:\n  f1: {{path: fixtures/x.md, kind: document}}\n");
    t.put("workshop.yaml", ws.as_bytes());
    let big = vec![b'a'; usize::try_from(MAX_MARKDOWN_BYTES).expect("fits") + 1];
    t.put("fixtures/x.md", &big);
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::LimitMdSize]);
    assert_no_secret(&d);
}

/// PACK-24: `pack.ir_format` が整数の 1 でなければ、型付きの読み込みより前に `E-FORMAT-VERSION` 1 件で止まる。
#[test]
fn neg_ir_format_variants_are_format_version() {
    let variants = [
        "ir_format: 0",
        "ir_format: -1",
        "ir_format: '1'",
        "ir_format: 1.0",
        "ir_format: 4294967296",
        "ir_format: null",
        "ir_format: [1]",
    ];
    for v in variants {
        let t = minimal();
        t.put(
            "workshop.yaml",
            WORKSHOP.replace("ir_format: 1", v).as_bytes(),
        );
        assert_eq!(codes(&diags(&t)), [DiagCode::FormatVersion], "{v}");
    }

    // 欠落。
    let t = minimal();
    t.put(
        "workshop.yaml",
        b"pack: {id: p, title: t, version: '0', guidance: A}\n",
    );
    assert_eq!(codes(&diags(&t)), [DiagCode::FormatVersion]);

    // 未対応の版と、既知の語彙にない欄の併存。残りを解釈せず、版の診断だけを返す。
    let t = minimal();
    t.put(
        "workshop.yaml",
        format!(
            "{}future_table: {{x: 1}}\n",
            WORKSHOP.replace("ir_format: 1", "ir_format: 3")
        )
        .as_bytes(),
    );
    t.put(
        "chapters/c1/chapter.yaml",
        CHAPTER
            .replace("title: A,", "title: A, future: 1,")
            .as_bytes(),
    );
    assert_eq!(codes(&diags(&t)), [DiagCode::FormatVersion]);

    // 構文の誤りと、pack の表が無いものは従来どおり E-PARSE。
    let t = minimal();
    t.put("workshop.yaml", b"pack: {id: p\n");
    assert_eq!(codes(&diags(&t)), [DiagCode::Parse]);
    let t = minimal();
    t.put("workshop.yaml", b"refs: []\n");
    assert_eq!(codes(&diags(&t)), [DiagCode::Parse]);
}

/// 形式 A（TOML）でも版を先に判定する。診断の文面に入力値を写さない。
#[test]
fn neg_ir_format_toml_and_no_echo() {
    let t = Tmp::new("fmt-toml");
    t.put(
        "workshop.toml",
        format!(
            "[pack]\nid = \"p\"\ntitle = \"{SECRET}\"\nversion = \"0\"\nir_format = \"{SECRET}\"\nguidance = \"A\"\n"
        )
        .as_bytes(),
    );
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::FormatVersion]);
    assert_no_secret(&d);
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
fn neg_chapter_without_steps_and_starts() {
    // Step も開始点も無い章だけでは、nodes・starts が空の、開始できない IR になる（PACK-25）。
    let t = minimal();
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: []\nsteps: []\n",
    );
    let d = diags(&t);
    assert_eq!(codes(&d), [DiagCode::Structure]);
    assert_eq!(d[0].file(), "chapters");

    // 開始点だけを書くと、宛先の Step が無いことの診断だけになり、開始点 0 件とは重ねない。
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps: []\n",
    );
    assert_eq!(codes(&diags(&t)), [DiagCode::RefStep]);
}

#[test]
fn empty_chapter_beside_a_runnable_chapter_compiles() {
    // Step の無い章があっても、別の章に開始点があれば開始できる。章ごとに Step を求める規則は spec に無い。
    let t = minimal();
    t.put(
        "chapters/c2/chapter.yaml",
        b"id: c2\ntitle: C2\nsteps: []\n",
    );
    assert!(compile_dir(&t.0).is_ok());
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
fn neg_use_of_undefined_module_is_reported_and_other_targets_are_checked() {
    let t = minimal();
    t.put(
        "chapters/c1/chapter.yaml",
        concat!(
            "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n",
            "  - {id: a, title: A, body: a.md, next: [{to: u}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-a, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}\n",
            "  - {id: u, title: U, next: [{to: zz}], use: {module: m, instance: i, args: {}}}\n",
        )
        .as_bytes(),
    );
    let d = diags(&t);
    // `use` の Step は node になるので、`u` への遷移は解決する。未定義なのはモジュール `m` と遷移先 `zz`。
    assert!(d.iter().any(|x| x.code() == DiagCode::RefModule), "{d:?}");
    let refs: Vec<&str> = d
        .iter()
        .filter(|x| x.code() == DiagCode::RefStep)
        .map(Diagnostic::message)
        .collect();
    assert_eq!(refs.len(), 1, "{refs:?}");
    assert!(refs[0].ends_with(": zz"), "{refs:?}");
}

#[test]
fn tests_path_is_optional_only_when_absent() {
    // 存在しないときだけ、教材テストの省略として成功する。
    let t = minimal();
    assert!(compile_dir(&t.0).is_ok());
    // tests/playthrough.yaml がディレクトリなら、読み込みの失敗として診断する。
    let t = minimal();
    fs::create_dir_all(t.0.join("tests/playthrough.yaml")).expect("mkdir");
    let d = diags(&t);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].file(), "tests/playthrough.yaml");
    // tests 自体が通常のファイルなら、NotFound ではないので診断する。
    let t = minimal();
    t.put("tests", b"x");
    assert_eq!(diags(&t).len(), 1);
}

#[test]
fn escaped_output_over_ir_limit_is_a_diagnostic() {
    // 入力は 20 MiB 以下でも、二重引用符は JSON で 2 バイトになり、出力が MAX_IR_BYTES を超える。
    let t = Tmp::new("big-out");
    let title = "\"".repeat(11 * 1024 * 1024);
    t.put(
        "workshop.yaml",
        format!("pack: {{id: p, title: '{title}', version: '0', ir_format: 1, guidance: A}}\n")
            .as_bytes(),
    );
    t.put("chapters/c1/chapter.yaml", CHAPTER.as_bytes());
    t.put("chapters/c1/a.md", format!("# a\n{SECRET}\n").as_bytes());
    let d = diags(&t);
    assert_eq!(codes(&d), vec![DiagCode::Limit]);
    assert_no_secret(&d);
}
