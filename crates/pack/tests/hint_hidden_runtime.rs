//! 案内レベル別のヒントの出し分け（TASK-293、GUIDE-2）の、検査と実行時の一致の試験。
//!
//! pack の `E-HINT-HIDDEN`（展開の前の Step と定義元のヒントの表から引く）が報告する (定義ファイル, Step, 水準) の
//! 集合と、core の `hints_at`（展開の後の node と IR のヒントの表から引く）が空になる組の集合が等しいことを確かめる。
//! 両者は同じ契約の規則を違う入力から引くので、片方がもう片方を呼ぶ形にはしない。対象は定義のあるヒントを
//! 1 つ以上持つ Step で、モジュールの実体は 1 組にまとまる。試験では `unwrap` を使ってよい。

mod common;

use std::collections::BTreeSet;
use std::path::Path;

use common::{assert_no_secret, copy_dir, testdata, Tmp};
use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::hint_select::hints_at;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::source::load;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::DiagCode;

type Triple = (String, String, GuidanceLevel);

fn ir_of(dir: &Path) -> Ir {
    serde_json::from_slice(&compile_dir(dir).expect("compile")).expect("ir")
}

/// 検査側の集合。水準は message の末尾の固定の語から読む。
fn s_check(dir: &Path) -> BTreeSet<Triple> {
    let diags = validate_dir(dir);
    assert_no_secret(&diags);
    diags
        .iter()
        .filter(|d| d.code() == DiagCode::HintHidden)
        .map(|d| {
            let msg = d.to_string();
            let word = msg
                .rsplit("guidance level ")
                .next()
                .expect("level word")
                .trim();
            let level = GuidanceLevel::parse(word).unwrap_or_else(|| panic!("bad level in {msg}"));
            (d.file().to_string(), d.item().to_string(), level)
        })
        .collect()
}

/// 実行時側の集合。
fn s_run(dir: &Path, ir: &Ir) -> BTreeSet<Triple> {
    let pack = load(dir).expect("load");
    let mut out = BTreeSet::new();
    for node in &ir.nodes {
        if !node.step.hints.iter().any(|h| ir.hints.contains_key(h)) {
            continue;
        }
        let file = pack
            .scopes
            .iter()
            .find(|s| s.src.id == node.unit)
            .map(|s| s.file.clone())
            .expect("scope of node");
        for l in GuidanceLevel::ALL {
            if hints_at(ir, &StepUid(node.uid.clone()), l).is_empty() {
                out.insert((file.clone(), format!("step:{}", node.step.id), l));
            }
        }
    }
    out
}

fn agreement(dir: &Path) -> BTreeSet<Triple> {
    let ir = ir_of(dir);
    let (c, r) = (s_check(dir), s_run(dir, &ir));
    assert_eq!(
        c,
        r,
        "check-only: {:?}\nrun-only: {:?}",
        c.difference(&r).collect::<Vec<_>>(),
        r.difference(&c).collect::<Vec<_>>()
    );
    c
}

fn base() -> std::path::PathBuf {
    testdata("pack-defects/base/format-b-v1_1")
}

fn with_d22() -> Tmp {
    let t = Tmp::new("hint-runtime-d22");
    copy_dir(&base(), &t.0);
    copy_dir(&testdata("pack-defects/v1_1/b/D22"), &t.0);
    t
}

fn node_hints(ir: &Ir, step: &str) -> [usize; 3] {
    let n = ir.nodes.iter().find(|n| n.step.id == step).expect("node");
    let uid = StepUid(n.uid.clone());
    GuidanceLevel::ALL.map(|l| hints_at(ir, &uid, l).len())
}

#[test]
fn hint_lists_follow_the_level() {
    // base のヒントは levels なし（全水準）、D22 は [B, C] だけ（A は 0 件）。
    let ir = ir_of(&base());
    assert_eq!(node_hints(&ir, "st-13-2"), [1, 1, 1]);
    let t = with_d22();
    let ir = ir_of(&t.0);
    assert_eq!(node_hints(&ir, "st-13-2"), [0, 1, 1]);
}

#[test]
fn agrees_on_base() {
    assert!(agreement(&base()).is_empty());
}

#[test]
fn agrees_on_d22() {
    let t = with_d22();
    let got = agreement(&t.0);
    let want: BTreeSet<Triple> = [(
        "chapters/ch-13/chapter.yaml".to_string(),
        "step:st-13-2".to_string(),
        GuidanceLevel::A,
    )]
    .into();
    assert_eq!(got, want);
}

#[test]
fn agrees_on_module_instances() {
    let t = Tmp::new("hint-runtime-mod");
    copy_dir(&testdata("module-reuse-rounds/test-data/pack"), &t.0);
    agreement(&t.0);
    let ir = ir_of(&t.0);
    let mut count = std::collections::BTreeMap::new();
    for n in &ir.nodes {
        *count
            .entry((n.unit.clone(), n.step.id.clone()))
            .or_insert(0) += 1;
    }
    assert!(
        count.values().any(|&c| c >= 2),
        "module expanded >= 2 nodes"
    );
}

const WORKSHOP_HEAD: &str = "pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}\n";

fn levels_text(l: &Option<Vec<&str>>) -> String {
    match l {
        None => String::new(),
        Some(v) => format!(", levels: [{}]", v.join(", ")),
    }
}

/// 章 c1 の Step a が、渡した宣言のヒントを持つ小さな教材。
fn synthetic(decls: &[Option<Vec<&str>>]) -> Tmp {
    let t = Tmp::new("hint-runtime-syn");
    let mut w = String::from(WORKSHOP_HEAD);
    w.push_str("hints:\n");
    let mut ids = Vec::new();
    for (i, d) in decls.iter().enumerate() {
        w.push_str(&format!("  h{i}: {{text: t{}}}\n", levels_text(d)));
        ids.push(format!("h{i}"));
    }
    if decls.is_empty() {
        w.push_str("  {}\n");
    }
    t.put("workshop.yaml", w.as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: a}}]\nsteps:\n  - {{id: a, title: A, hints: [{}], next: [{{to: b}}], input: {{kind: choice, options: [{{id: ok, label: OK}}]}}, rules: [{{id: r-a, on: {{option: ok}}, outcome: {{kind: accept}}}}], confirm: {{kind: none, reason: r}}}}\n  - {{id: b, title: B, final: true}}\n",
        ids.join(", ")
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t
}

#[test]
fn agrees_on_synthetic_packs() {
    use std::collections::BTreeSet as S;
    let cases: Vec<Vec<Option<Vec<&str>>>> = vec![
        vec![Some(vec!["A"])],
        vec![Some(vec!["B", "C"])],
        vec![Some(vec!["C"])],
        vec![Some(vec![])],
        vec![None],
        vec![Some(vec!["A"]), Some(vec!["B", "C"])],
        vec![Some(vec!["B"]), Some(vec!["B"])],
        vec![Some(vec!["A"]), Some(vec!["C"])],
    ];
    let mut hidden_any = S::new();
    for c in &cases {
        let t = synthetic(c);
        for (_, _, l) in agreement(&t.0) {
            hidden_any.insert(l);
        }
    }
    // 水準 A・B・C のどれもが「隠れる」報告として一度は現れる（一致が空集合だけで成り立たない）。
    assert_eq!(hidden_any.len(), 3, "{hidden_any:?}");
}

#[test]
fn undefined_hint_only_is_out_of_scope_for_hidden() {
    // 未定義のヒントだけを持つ Step は E-REF-HINT の担当で、E-HINT-HIDDEN は 0 件。compile は失敗するので実行時側は対象外。
    let t = synthetic(&[]);
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n  - {id: a, title: A, hints: [nope], next: [{to: b}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-a, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}\n  - {id: b, title: B, final: true}\n",
    );
    assert!(s_check(&t.0).is_empty());
}
