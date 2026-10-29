//! 教材パックから受け口を有効にできないことの否定の試験（TASK-191 1/2、EXT-10・D-53）。
//!
//! 決定（D-53）は「受け口は実行環境だけで有効にし、教材パックからは有効にできない」である。教材パックに
//! 有効化らしい欄を書いても、未知の欄として `E-PARSE` で止まり（fail-closed）、IR にも欄が無い。
//! 型を検査しない `refs` に書いた値は IR に写るが、関門（`receptor_gate::activation`）は IR を入力に取らないため
//! 無効のままである。値は診断に写らない。試験では `unwrap` を使ってよい。

mod common;

use common::{assert_no_secret, Tmp, CHAPTER, SECRET, WORKSHOP};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::receptor_gate::{
    activation, Activation, DisabledReason, LearnerConsent, ReceptorId, ReceptorSettings,
};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::DiagCode;

const FIELDS: [&str; 5] = [
    "receptors",
    "extensions",
    "enable_receptors",
    "advice_provider",
    "consent",
];

/// 有効化らしい欄を 4 か所（最上位・pack・Step・開始点）のどれかに置いた教材を作る。
fn pack_with(place: &str, field: &str) -> Tmp {
    let v = format!("{{enabled: true, token: {SECRET}}}");
    let t = Tmp::new("e10");
    let (ws, ch) = match place {
        "top" => (format!("{WORKSHOP}{field}: {v}\n"), CHAPTER.to_string()),
        "pack" => (
            format!(
                "pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: A, {field}: {v}}}\n"
            ),
            CHAPTER.to_string(),
        ),
        "step" => (
            WORKSHOP.to_string(),
            CHAPTER.replacen("{id: a, ", &format!("{{id: a, {field}: {v}, "), 1),
        ),
        _ => (
            WORKSHOP.to_string(),
            CHAPTER.replacen("{id: s1, ", &format!("{{id: s1, {field}: {v}, "), 1),
        ),
    };
    t.put("workshop.yaml", ws.as_bytes());
    t.put("chapters/c1/chapter.yaml", ch.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    t
}

#[test]
fn ext10_receptor_fields_are_rejected_by_compile_and_validate() {
    for place in ["top", "pack", "step", "start"] {
        for f in FIELDS {
            let t = pack_with(place, f);
            let d = compile_dir(&t.0).expect_err("receptor fields are not vocabulary words");
            assert_eq!(d.len(), 1, "{place}/{f}: {d:?}");
            assert_eq!(d[0].code(), DiagCode::Parse, "{place}/{f}");
            assert_no_secret(&d);
            let v = validate_dir(&t.0);
            let parse: Vec<_> = v.iter().filter(|x| x.code() == DiagCode::Parse).collect();
            assert_eq!(parse.len(), 1, "{place}/{f}: {v:?}");
            assert_no_secret(&v);
        }
    }
}

#[test]
fn ext10_ir_with_receptor_field_is_refused_by_load_ir() {
    let t = common::minimal();
    let ir: serde_json::Value = serde_json::from_slice(&compile_dir(&t.0).unwrap()).unwrap();
    for f in FIELDS {
        let mut top = ir.clone();
        top[f] = serde_json::json!({"enabled": true});
        assert!(
            load_ir(&serde_json::to_vec(&top).unwrap()).is_err(),
            "top/{f}"
        );
        let mut pack = ir.clone();
        pack["pack"][f] = serde_json::json!({"enabled": true});
        assert!(
            load_ir(&serde_json::to_vec(&pack).unwrap()).is_err(),
            "pack/{f}"
        );
    }
}

#[test]
fn ext10_refs_values_do_not_enable_anything() {
    let refs = "refs:\n  - {name: x, receptors: {enabled: true}, consent: true}\n";
    let t = Tmp::new("e10");
    t.put("workshop.yaml", format!("{WORKSHOP}{refs}").as_bytes());
    t.put("chapters/c1/chapter.yaml", CHAPTER.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    // 観察: refs は型を検査されず通る。関門は IR を入力に取らないので、無効のまま。
    let ir = load_ir(&compile_dir(&t.0).expect("compile")).expect("load_ir");
    assert_eq!(ir.refs.len(), 1);
    let got = activation(
        &ReceptorSettings::default(),
        &LearnerConsent::default(),
        ReceptorId::new("advice"),
    );
    assert_eq!(got, Activation::Disabled(DisabledReason::NotConfigured));
}
