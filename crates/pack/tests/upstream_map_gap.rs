//! Step ごとの上流の行（対応表の欄）が語彙に無いことと、代用の手段の観察の固定（TASK-72、PACK-46・D-70）。
//!
//! 決定（D-70、2026-10-22）は「対応表の欄を語彙に持たない」である。対応表は読み込まれない教材 repo の別の文書に置き、
//! 教材テストで Step ID と突き合わせる。参照元の版を上げたときに影響する Step の洗い出し（C-18）もその側で行う。
//! ヒントの 3 段目の日本語の括弧書きは、コマンドの文字列の外（ヒントの別の文）に置く教材側の約束で、検査は無い。
//! 本試験は、代用が依拠する現状を一時ディレクトリの小さな教材で固定する。
//!
//! - Step に上流らしい欄を書くと `compile` も `validate` も `E-PARSE` で止める（fail-closed）。値は診断に写らない
//! - 参照版はパック全体の `refs` に書ける。型は検査されず IR にそのまま写り、Step とは結びつかない
//! - Step ID は `fandhe-pack vocab` の表の `scope,step` から漏れなく取れる。別の文書の行との突き合わせは、
//!   エンジンの出力だけで組める（試験内の突き合わせは教材テストの形の例示であり、エンジンの機能ではない）
//! - 括弧書きをコマンドの外に置く約束は検査されない（違反の形も診断 0 件で通る）
//!
//! 確かめないこと: 教材 repo の別の文書と教材テストそのもの、画面の表示。試験では `unwrap` を使ってよい。

mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::{assert_no_secret, Tmp, SECRET, WORKSHOP};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::vocab_report::report_dir;
use fandhe_workshop_pack::DiagCode;

fn pack_with_step_field(field: &str) -> Tmp {
    let t = Tmp::new("p46");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: a}}]\nsteps:\n  - {{id: a, title: A, {field}, final: true}}\n"
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t
}

const UPSTREAM_FIELDS: [&str; 3] = [
    "upstream: {repo: upstream-a, file: f.txt, line: 3, commit: 0123456789abcdef0123456789abcdef01234567}",
    "upstream_ref: upstream-a",
    "source: {repo: upstream-a}",
];

#[test]
fn pack46_step_upstream_field_is_rejected_by_compile() {
    for f in UPSTREAM_FIELDS {
        let t = pack_with_step_field(f);
        let d = compile_dir(&t.0).expect_err("upstream fields are not vocabulary words");
        assert_eq!(d.len(), 1, "{f}: {d:?}");
        assert_eq!(d[0].code(), DiagCode::Parse, "{f}");
    }
}

#[test]
fn pack46_step_upstream_field_is_rejected_by_validate() {
    for f in UPSTREAM_FIELDS {
        let t = pack_with_step_field(f);
        let d = validate_dir(&t.0);
        let parse: Vec<_> = d.iter().filter(|x| x.code() == DiagCode::Parse).collect();
        assert_eq!(parse.len(), 1, "{f}: {d:?}");
    }
}

#[test]
fn pack46_upstream_field_value_is_not_echoed() {
    let t = pack_with_step_field(&format!("upstream: {{repo: {SECRET}}}"));
    assert_no_secret(&compile_dir(&t.0).expect_err("rejected"));
    assert_no_secret(&validate_dir(&t.0));
}

#[test]
fn pack46_pack_refs_pass_and_are_copied_to_ir() {
    let refs = "refs:\n  - {name: upstream-a, commit: 0123456789abcdef0123456789abcdef01234567, link: 'https://example.invalid/a'}\n  - {name: upstream-b, commit: fedcba9876543210fedcba9876543210fedcba98, link: 'https://example.invalid/b'}\n";
    let t = Tmp::new("p46");
    t.put("workshop.yaml", format!("{WORKSHOP}{refs}").as_bytes());
    t.put("chapters/c1/chapter.yaml", common::CHAPTER.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    assert!(validate_dir(&t.0).is_empty(), "{:?}", validate_dir(&t.0));
    let ir = load_ir(&compile_dir(&t.0).expect("compile")).expect("load_ir");
    assert_eq!(ir.refs.len(), 2);
    assert_eq!(ir.refs[0]["name"], "upstream-a");
    assert_eq!(ir.refs[1]["link"], "https://example.invalid/b");

    // 型は検査されず、Step とも結びつかない（存在しない Step ID らしい値も診断にならない）。
    let with_step = format!(
        "{WORKSHOP}refs:\n  - {{name: upstream-a, step: no-such-step, commit: 0123456789abcdef0123456789abcdef01234567}}\n"
    );
    t.put("workshop.yaml", with_step.as_bytes());
    assert!(validate_dir(&t.0).is_empty(), "{:?}", validate_dir(&t.0));
    let ir = load_ir(&compile_dir(&t.0).expect("compile")).expect("load_ir");
    assert_eq!(ir.refs[0]["step"], "no-such-step");
}

/// 別の文書の 1 行を模した記録。教材 repo の教材テストが持つ表の形の例示である。
struct MapRow {
    step: &'static str,
    reference: &'static str,
}

#[test]
fn pack46_vocab_table_yields_step_ids_for_external_map() {
    let t = Tmp::new("p46");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n  - {id: a, title: A, next: [{to: b}]}\n  - {id: b, title: B, use: {module: m, instance: i1}, final: true}\n",
    );
    t.put(
        "modules/m/module.yaml",
        b"id: m\ntitle: M\nsteps:\n  - {id: m1, title: M1, next: [{to: m2}]}\n  - {id: m2, title: M2, final: true}\n",
    );
    let table = report_dir(&t.0).expect("vocab");
    let mut lines = table.lines();
    assert_eq!(lines.next(), Some("scope,step,kind,extension_points"));
    let mut ids: BTreeSet<(String, String)> = BTreeSet::new();
    for l in lines {
        let cols: Vec<&str> = l.split(',').collect();
        if cols.len() == 4 && !cols[0].is_empty() && !l.starts_with('#') {
            ids.insert((cols[0].to_string(), cols[1].to_string()));
        }
    }
    // 定義した Step がすべて出る。展開後の複製（instance 付き）は出ない。
    for want in [("c1", "a"), ("c1", "b"), ("m", "m1"), ("m", "m2")] {
        assert!(
            ids.contains(&(want.0.to_string(), want.1.to_string())),
            "{want:?} in {ids:?}"
        );
    }
    assert!(ids.iter().all(|(_, s)| !s.contains("i1")), "{ids:?}");
    let step_ids: BTreeSet<&str> = ids.iter().map(|(_, s)| s.as_str()).collect();

    // 別の文書の行を模した表と、Step ID の突き合わせ。
    let map = [
        MapRow {
            step: "a",
            reference: "upstream-a",
        },
        MapRow {
            step: "m1",
            reference: "upstream-b",
        },
        MapRow {
            step: "m2",
            reference: "upstream-a",
        },
        MapRow {
            step: "gone",
            reference: "upstream-a",
        },
    ];
    let unknown: Vec<&str> = map
        .iter()
        .filter(|r| !step_ids.contains(r.step))
        .map(|r| r.step)
        .collect();
    assert_eq!(unknown, ["gone"], "表に無い Step ID を持つ行は検出できる");

    // 参照名 upstream-a の commit を上げたときに影響する Step は、決定的に列挙できる。
    let mut by_ref: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for r in map.iter().filter(|r| step_ids.contains(r.step)) {
        by_ref.entry(r.reference).or_default().insert(r.step);
    }
    assert_eq!(
        by_ref["upstream-a"].iter().copied().collect::<Vec<_>>(),
        ["a", "m2"]
    );
    assert_eq!(
        by_ref["upstream-b"].iter().copied().collect::<Vec<_>>(),
        ["m1"]
    );
}

#[test]
fn pack46_hint_bracket_outside_command_is_plain_text() {
    let good = "次のコマンドを入力します。cmd --flag value。（組み立ての例）";
    let bad = "cmd --flag value（組み立ての例）";
    for text in [good, bad] {
        let t = Tmp::new("p46");
        t.put(
            "workshop.yaml",
            format!("{WORKSHOP}hints:\n  h-a: {{text: '{text}', levels: [A, B, C]}}\n").as_bytes(),
        );
        t.put(
            "chapters/c1/chapter.yaml",
            b"id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n  - {id: a, title: A, hints: [h-a], final: true}\n",
        );
        // 約束の違反の形（bad）も診断 0 件で通る。機械的な検査は無い。
        assert!(
            validate_dir(&t.0).is_empty(),
            "{text}: {:?}",
            validate_dir(&t.0)
        );
        let ir = load_ir(&compile_dir(&t.0).expect("compile")).expect("load_ir");
        assert!(ir.hints["h-a"].to_string().contains(text), "{text}");
    }
}
