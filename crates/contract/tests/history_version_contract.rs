//! 履歴の版の欄と判定結果の型の適合試験（TASK-173 1/2）。
//!
//! 欄の順序・全欄必須・未知の欄の拒否・書式の検査を、golden（`tests/data/history_versions_1/`）との
//! バイト一致と個別の入力で固定する。失敗の理由コード名は検査しない（TASK-310 の範囲）。
//! 試験は作業ツリーを書き換えない。試験では `unwrap` を使ってよい。

use std::collections::BTreeSet;
use std::path::Path;

use fandhe_workshop_contract::history_version::*;
use serde_json::{json, Value};

fn golden(name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/history_versions_1")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("cannot read golden {name}"))
}

fn render(v: &HistoryVersions) -> Vec<u8> {
    let mut b = serde_json::to_vec_pretty(v).unwrap();
    b.push(b'\n');
    b
}

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn sample() -> HistoryVersions {
    HistoryVersions {
        engine: "1.2.3".into(),
        ir_format: 1,
        vocab: "1.3".into(),
        content: "2.0.1".into(),
        content_hash: HASH.into(),
        refs: Default::default(),
    }
}

fn full_json() -> Value {
    serde_json::from_slice(&golden("full.json")).unwrap()
}

#[test]
fn contract_version_is_1() {
    assert_eq!(HISTORY_VERSION_CONTRACT_VERSION, 1);
}

#[test]
fn golden_roundtrip_is_byte_identical() {
    for name in ["minimal.json", "full.json"] {
        let bytes = golden(name);
        let v: HistoryVersions = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(render(&v), bytes, "{name}");
        v.parse().unwrap();
    }
}

#[test]
fn built_value_matches_golden_and_field_order() {
    let mut v = sample();
    v.engine = "0.1.0".into();
    v.vocab = "1.0".into();
    v.content = "0.1.0".into();
    v.content_hash = "0".repeat(64);
    assert_eq!(render(&v), golden("minimal.json"));
    let text = String::from_utf8(golden("full.json")).unwrap();
    let pos: Vec<usize> = [
        "engine",
        "ir_format",
        "vocab",
        "content",
        "content_hash",
        "refs",
    ]
    .iter()
    .map(|k| text.find(&format!("\"{k}\"")).unwrap())
    .collect();
    assert!(pos.windows(2).all(|w| w[0] < w[1]));
    assert!(String::from_utf8(golden("minimal.json"))
        .unwrap()
        .contains("\"refs\": {}"));
}

#[test]
fn missing_field_is_rejected() {
    for key in [
        "engine",
        "ir_format",
        "vocab",
        "content",
        "content_hash",
        "refs",
    ] {
        let mut j = full_json();
        j.as_object_mut().unwrap().remove(key);
        assert!(
            serde_json::from_value::<HistoryVersions>(j).is_err(),
            "{key}"
        );
    }
}

#[test]
fn wrong_type_and_unknown_field_are_rejected() {
    let mut j = full_json();
    j["ir_format"] = json!("1");
    assert!(serde_json::from_value::<HistoryVersions>(j).is_err());
    let mut j = full_json();
    j["refs"] = json!([]);
    assert!(serde_json::from_value::<HistoryVersions>(j).is_err());
    let mut j = full_json();
    j["extra"] = json!(1);
    assert!(serde_json::from_value::<HistoryVersions>(j).is_err());
}

#[test]
fn format_is_checked_by_parse_not_by_deserialize() {
    let mut j = full_json();
    j["engine"] = json!("x");
    let v: HistoryVersions = serde_json::from_value(j).unwrap();
    assert_eq!(v.parse().unwrap_err().field, VersionField::Engine);
}

#[test]
fn parse_accepts_canonical_forms() {
    let mut v = sample();
    v.engine = "0.0.0".into();
    v.content = "4294967295.0.0".into();
    let p = v.parse().unwrap();
    assert_eq!(
        p.engine,
        Version3 {
            major: 0,
            minor: 0,
            patch: 0
        }
    );
    assert_eq!(p.content.major, u32::MAX);
    assert_eq!(p.vocab, VocabVersion { major: 1, minor: 3 });
    assert_eq!(p.content_hash.as_str(), HASH);
    assert_eq!(p.engine.to_string(), "0.0.0");
    assert_eq!(p.vocab.to_string(), "1.3");
}

#[test]
fn parse_rejects_bad_versions_with_the_right_field() {
    for bad in [
        "1",
        "1.2",
        "1.2.3.4",
        "a.b.c",
        "+1.0.0",
        "1..0",
        " 1.0.0",
        "01.0.0",
        "4294967296.0.0",
        "-1.0.0",
        "",
    ] {
        let mut v = sample();
        v.engine = bad.into();
        assert_eq!(
            v.parse().unwrap_err().field,
            VersionField::Engine,
            "{bad:?}"
        );
        let mut v = sample();
        v.content = bad.into();
        assert_eq!(
            v.parse().unwrap_err().field,
            VersionField::Content,
            "{bad:?}"
        );
    }
    for bad in ["1", "1.2.3", "1.", ".1", "+1.0", "01.0"] {
        let mut v = sample();
        v.vocab = bad.into();
        assert_eq!(v.parse().unwrap_err().field, VersionField::Vocab, "{bad:?}");
    }
    let upper = HASH.to_uppercase();
    for bad in [&HASH[..63], &format!("{HASH}0"), &upper, &"g".repeat(64)] {
        let mut v = sample();
        v.content_hash = (*bad).to_string();
        assert_eq!(
            v.parse().unwrap_err().field,
            VersionField::ContentHash,
            "{bad:?}"
        );
    }
}

#[test]
fn parse_error_reports_first_field_and_hides_value() {
    let mut v = sample();
    v.vocab = "secret-value".into();
    v.content = "bad".into();
    let e = v.parse().unwrap_err();
    assert_eq!(e.field, VersionField::Vocab);
    let msg = e.to_string();
    assert_eq!(msg, "invalid version string in versions.vocab");
    assert!(!msg.contains("secret-value"));
}

#[test]
fn ordering() {
    let v = |s| Version3::parse(s).unwrap();
    assert!(v("0.1.0") < v("0.2.0"));
    assert!(v("1.0.0") > v("0.9.9"));
    assert!(v("1.2.3").same_major(&v("1.9.0")));
    assert!(!v("1.2.3").same_major(&v("2.2.3")));
    let w = |s| VocabVersion::parse(s).unwrap();
    assert!(w("1.0") < w("1.2") && w("1.2") < w("1.3"));
    assert_eq!(VocabVersion::DEFAULT, w("1.0"));
}

#[test]
fn diff_is_empty_for_identical() {
    assert!(sample().diff(&sample()).is_empty());
}

#[test]
fn diff_reports_each_axis() {
    let base = sample();
    let mut o = base.clone();
    o.content = "2.1.0".into();
    assert_eq!(
        base.diff(&o),
        vec![AxisDiff {
            axis: VersionAxis::Content,
            ref_name: None,
            history: Some("2.0.1".into()),
            opening: Some("2.1.0".into()),
        }]
    );
    let mut o = base.clone();
    o.vocab = "1.4".into();
    assert_eq!(base.diff(&o)[0].axis, VersionAxis::Vocab);
    let mut o = base.clone();
    o.engine = "1.3.0".into();
    assert_eq!(base.diff(&o)[0].axis, VersionAxis::Engine);
    let mut o = base.clone();
    o.ir_format = 2;
    let d = base.diff(&o);
    assert_eq!(d[0].axis, VersionAxis::IrFormat);
    assert_eq!(d[0].history.as_deref(), Some("1"));
    assert_eq!(d[0].opening.as_deref(), Some("2"));
}

#[test]
fn diff_refs_changed_added_removed_in_order() {
    let mut h = sample();
    h.refs.insert("b".into(), "c1".into());
    h.refs.insert("c".into(), "c3".into());
    h.refs.insert("d".into(), "same".into());
    let mut o = sample();
    o.refs.insert("a".into(), "c0".into());
    o.refs.insert("b".into(), "c2".into());
    o.refs.insert("d".into(), "same".into());
    let d = h.diff(&o);
    let got: Vec<_> = d
        .iter()
        .map(|x| {
            (
                x.axis,
                x.ref_name.as_deref(),
                x.history.as_deref(),
                x.opening.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            (VersionAxis::Refs, Some("a"), None, Some("c0")),
            (VersionAxis::Refs, Some("b"), Some("c1"), Some("c2")),
            (VersionAxis::Refs, Some("c"), Some("c3"), None),
        ]
    );
}

#[test]
fn diff_order_follows_axis_order() {
    let h = sample();
    let mut o = sample();
    o.refs.insert("r".into(), "x".into());
    o.content_hash = "f".repeat(64);
    o.engine = "9.9.9".into();
    let axes: Vec<_> = h.diff(&o).iter().map(|d| d.axis).collect();
    assert_eq!(
        axes,
        vec![
            VersionAxis::Engine,
            VersionAxis::ContentHash,
            VersionAxis::Refs
        ]
    );
    assert_eq!(VersionAxis::ALL.len(), 6);
}

#[test]
fn conversion_notes_cannot_be_empty() {
    assert!(ConversionNotes::new(vec![]).is_none());
    let mut o = sample();
    o.engine = "1.2.4".into();
    let d = sample().diff(&o);
    let n = ConversionNotes::new(d.clone()).unwrap();
    assert_eq!(n.diffs(), d.as_slice());
}

#[test]
fn disposition_names_are_distinct() {
    assert_eq!(Disposition::ALL.len(), 4);
    let names: BTreeSet<_> = Disposition::ALL.iter().map(Disposition::as_str).collect();
    assert_eq!(names.len(), 4);
    assert!(names.iter().all(|n| !n.is_empty()));
}

#[derive(Debug, PartialEq)]
enum Dummy {
    Newer,
    TooBig,
}

impl ReasonCode for Dummy {
    fn as_str(&self) -> &'static str {
        match self {
            Dummy::Newer => "dummy_newer",
            Dummy::TooBig => "dummy_too_big",
        }
    }
    fn kind(&self) -> Option<ReasonKind> {
        match self {
            Dummy::Newer => Some(ReasonKind::HistoryNewer(VersionAxis::IrFormat)),
            Dummy::TooBig => None,
        }
    }
}

#[test]
fn verdict_accessors() {
    let mut o = sample();
    o.vocab = "1.4".into();
    let notes = ConversionNotes::new(sample().diff(&o)).unwrap();
    let open: Verdict<Dummy> = Verdict::Open;
    assert_eq!(open.disposition(), Disposition::Open);
    assert!(open.code().is_none());
    let conv: Verdict<Dummy> = Verdict::Converted { notes };
    assert_eq!(conv.disposition(), Disposition::Converted);
    assert!(conv.code().is_none());
    let view = Verdict::ViewOnly { code: Dummy::Newer };
    assert_eq!(view.disposition(), Disposition::ViewOnly);
    assert_eq!(view.code(), Some(&Dummy::Newer));
    assert_eq!(view.code().unwrap().as_str(), "dummy_newer");
    let refused = Verdict::Refused {
        code: Dummy::TooBig,
    };
    assert_eq!(refused.disposition(), Disposition::Refused);
    assert_eq!(refused.code().unwrap().kind(), None);
    assert_eq!(
        Dummy::Newer.kind(),
        Some(ReasonKind::HistoryNewer(VersionAxis::IrFormat))
    );
}
