//! 固定の履歴 4 本の版の記録の試験（TASK-180 2/2、VER-12・D-51）。
//!
//! D-51 は `content_hash` から本文の木を除く現状を維持すると決めた。そのため `testdata` の固定の履歴と
//! その期待値は書き換えない。ここでは、4 本の版の記録が全軸で同一で形式が正しいことだけを確かめる。
//! 「エンジンが同じ版として開く」判定は TASK-174 の担当であり、この試験は何も主張しない。
//! 履歴は未信頼入力として `deny_unknown_fields` の型と `parse` を通して読む。試験では `unwrap` を使ってよい。

use std::path::{Component, Path, PathBuf};

use fandhe_workshop_contract::history_version::{ContentHash, HistoryVersions};
use serde_json::Value;

/// D-51 で変えない値。本文の木を除いて求めるので、本文の扱いを理由に期待を更新しない。
const FIXED_HASH: &str = "530797430e77fe7d05aa9a680d14decc791bd15659dfb37df19bc9cf96e32b91";

const FIXED: [&str; 4] = ["h-early.json", "h-mid.json", "h-busy.json", "h-full.json"];

fn versions(name: &str) -> HistoryVersions {
    let p = Path::new(name);
    assert!(
        p.components().all(|c| matches!(c, Component::Normal(_))),
        "unsafe history path: {name}"
    );
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/save-restore-versions/test-data-addendum/histories")
        .join(p);
    let v: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let hv: HistoryVersions = serde_json::from_value(v["versions"].clone()).unwrap();
    hv.parse().unwrap();
    hv
}

#[test]
fn fixed_four_histories_share_one_version_record() {
    let first = versions(FIXED[0]);
    for name in &FIXED[1..] {
        assert_eq!(versions(name), first, "{name}");
    }
}

#[test]
fn fixed_histories_expectation_follows_d51() {
    for name in FIXED {
        let hv = versions(name);
        assert_eq!(
            ContentHash::parse(&hv.content_hash).unwrap().as_str(),
            FIXED_HASH
        );
    }
}

#[test]
fn other_histories_differ_on_some_axis() {
    let base = versions(FIXED[0]);

    let other_pack = versions("h-mid-other-pack.json");
    assert_ne!(other_pack.content_hash, base.content_hash);

    let vocab = versions("h-mid-vocab-1.0.json");
    assert_ne!(vocab.vocab, base.vocab);
    assert_ne!(vocab.content_hash, base.content_hash);

    let refs = versions("v-ref.json");
    assert_eq!(refs.content_hash, base.content_hash);
    assert_ne!(refs.refs, base.refs);

    let engine = versions("v-v-2-engine-0.2.0.json");
    assert_eq!(engine.content_hash, base.content_hash);
    assert_ne!(engine.engine, base.engine);
}
