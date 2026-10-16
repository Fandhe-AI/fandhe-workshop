//! 履歴の読み込みの判定の順序（TASK-310 3/3）の契約適合試験。
//!
//! 順序の golden・段への対応・上限の契約（TASK-199）との整合・契約の版を固定する。
//! 試験では `unwrap` を使ってよい。

use fandhe_workshop_contract::history_version::ReasonCode;
use fandhe_workshop_contract::limits::{LimitReason, LoadStage, LOAD_STAGE_ORDER};
use fandhe_workshop_contract::load_reason::{HistoryLoadStage, LoadReason, HISTORY_LOAD_ORDER};
use fandhe_workshop_contract::storage::STORAGE_CONTRACT_VERSION;
use std::collections::HashSet;

const ORDER_GOLDEN: [&str; 19] = [
    "too_large",
    "too_deep",
    "truncated",
    "not_json",
    "missing_version",
    "malformed",
    "too_many_ops",
    "history_format_newer",
    "other_pack",
    "bad_version",
    "format_newer",
    "vocab_newer",
    "engine_newer",
    "format_older",
    "engine_major",
    "content_major",
    "start_missing",
    "replay_mismatch",
    "replay_diverged",
];

use HistoryLoadStage as S;

const STAGE_GOLDEN: [(&str, HistoryLoadStage); 19] = [
    ("too_large", S::Size),
    ("too_deep", S::Depth),
    ("truncated", S::Json),
    ("not_json", S::Json),
    ("missing_version", S::VersionField),
    ("malformed", S::Type),
    ("too_many_ops", S::Type),
    ("history_format_newer", S::Type),
    ("other_pack", S::Type),
    ("bad_version", S::VersionJudgment),
    ("format_newer", S::VersionJudgment),
    ("vocab_newer", S::VersionJudgment),
    ("engine_newer", S::VersionJudgment),
    ("format_older", S::VersionJudgment),
    ("engine_major", S::VersionJudgment),
    ("content_major", S::VersionJudgment),
    ("start_missing", S::Replay),
    ("replay_mismatch", S::SnapshotMatch),
    ("replay_diverged", S::SnapshotMatch),
];

#[test]
fn order_matches_contract() {
    let actual: Vec<&str> = HISTORY_LOAD_ORDER.iter().map(|r| r.as_str()).collect();
    assert_eq!(actual, ORDER_GOLDEN);
}

#[test]
fn order_is_permutation_of_all() {
    let order: HashSet<LoadReason> = HISTORY_LOAD_ORDER.into_iter().collect();
    let all: HashSet<LoadReason> = LoadReason::ALL.into_iter().collect();
    assert_eq!(HISTORY_LOAD_ORDER.len(), 19);
    assert_eq!(order.len(), 19, "順序に重複がある");
    assert_eq!(order, all);
}

#[test]
fn stages_match_spec_order() {
    assert_eq!(
        HistoryLoadStage::ALL,
        [
            S::Size,
            S::Depth,
            S::Json,
            S::VersionField,
            S::Type,
            S::VersionJudgment,
            S::Replay,
            S::SnapshotMatch,
        ]
    );
    assert!(HistoryLoadStage::ALL.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn stage_is_non_decreasing_along_order() {
    assert!(HISTORY_LOAD_ORDER
        .windows(2)
        .all(|w| w[0].stage() <= w[1].stage()));
}

#[test]
fn every_stage_has_a_reason() {
    for stage in HistoryLoadStage::ALL {
        assert!(
            LoadReason::ALL.iter().any(|r| r.stage() == stage),
            "{stage:?} に理由が無い"
        );
    }
}

#[test]
fn stage_mapping_golden() {
    for (code, stage) in STAGE_GOLDEN {
        let reason = LoadReason::parse(code).unwrap();
        assert_eq!(reason.stage(), stage, "{code}");
    }
}

#[test]
fn coarse_is_monotonic_and_matches_safe20() {
    let coarse: Vec<LoadStage> = HistoryLoadStage::ALL.iter().map(|s| s.coarse()).collect();
    assert!(coarse.windows(2).all(|w| w[0] <= w[1]));
    assert_eq!(coarse[..3], LOAD_STAGE_ORDER[..3]);
    assert!(coarse[3..].iter().all(|s| *s == LoadStage::TypeAndContent));
}

#[test]
fn limit_reasons_agree_with_limits_contract() {
    for r in LimitReason::ALL {
        assert_eq!(LoadReason::from(r).stage().coarse(), r.stage(), "{r:?}");
    }
}

#[test]
fn version_reasons_are_in_version_judgment_stage() {
    for r in LoadReason::ALL {
        if r.kind().is_none() {
            continue;
        }
        let expected = match r {
            LoadReason::MissingVersion => S::VersionField,
            LoadReason::ReplayDiverged => S::SnapshotMatch,
            _ => S::VersionJudgment,
        };
        assert_eq!(r.stage(), expected, "{r}");
    }
}

#[test]
fn contract_version_is_1() {
    assert_eq!(STORAGE_CONTRACT_VERSION, 1);
}

#[test]
fn side_effect_free_stages_precede_replay() {
    let before = HISTORY_LOAD_ORDER
        .iter()
        .filter(|r| r.stage() < S::Replay)
        .count();
    assert_eq!(before, 16);
}
