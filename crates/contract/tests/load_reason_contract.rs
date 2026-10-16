//! 読み込みの拒否理由コード（TASK-310 2/3）の契約適合試験。
//!
//! 19 種の名前・重複の無さ・上限の契約への委譲・版の判定の分類との対応を固定する。
//! 試験では `unwrap` を使ってよい。

use fandhe_workshop_contract::history_version::{
    Disposition, ReasonCode, ReasonKind, Verdict, VersionAxis,
};
use fandhe_workshop_contract::limits::LimitReason;
use fandhe_workshop_contract::load_reason::LoadReason;
use std::collections::HashSet;

const GOLDEN: [&str; 19] = [
    "too_large",
    "too_deep",
    "too_many_ops",
    "truncated",
    "not_json",
    "missing_version",
    "malformed",
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

#[test]
fn all_has_19_distinct_variants() {
    assert_eq!(LoadReason::ALL.len(), 19);
    let set: HashSet<LoadReason> = LoadReason::ALL.into_iter().collect();
    assert_eq!(set.len(), 19);
}

#[test]
fn codes_are_unique() {
    let set: HashSet<&str> = LoadReason::ALL
        .into_iter()
        .map(LoadReason::as_str)
        .collect();
    assert_eq!(set.len(), LoadReason::ALL.len());
}

fn is_snake_case(s: &str) -> bool {
    let mut parts = s.split('_');
    let ok_part = |p: &str| {
        !p.is_empty()
            && p.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    };
    let Some(first) = parts.next() else {
        return false;
    };
    first.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && ok_part(first)
        && parts.all(ok_part)
}

#[test]
fn codes_are_non_empty_snake_case() {
    for r in LoadReason::ALL {
        assert!(!r.as_str().is_empty());
        assert!(is_snake_case(r.as_str()), "{}", r.as_str());
    }
}

#[test]
fn codes_match_contract_list() {
    let got: Vec<&str> = LoadReason::ALL
        .into_iter()
        .map(LoadReason::as_str)
        .collect();
    assert_eq!(got, GOLDEN);
}

#[test]
fn issue_named_codes_exist() {
    for name in [
        "too_large",
        "too_deep",
        "not_json",
        "missing_version",
        "malformed",
        "replay_mismatch",
        "truncated",
        "other_pack",
        "content_major",
        "vocab_newer",
        "format_newer",
        "engine_newer",
        "format_older",
        "engine_major",
        "replay_diverged",
    ] {
        assert!(LoadReason::parse(name).is_some(), "{name}");
    }
}

#[test]
fn limit_codes_delegate_to_limit_reason() {
    for r in LimitReason::ALL {
        let l = LoadReason::from(r);
        assert_eq!(l.as_str(), r.as_str());
        assert_eq!(l.limit(), Some(r));
    }
    let limits = LoadReason::ALL
        .into_iter()
        .filter(|r| r.limit().is_some())
        .count();
    assert_eq!(limits, LimitReason::ALL.len());
}

#[test]
fn parse_roundtrip_and_rejects_non_canonical() {
    for r in LoadReason::ALL {
        assert_eq!(LoadReason::parse(r.as_str()), Some(r));
    }
    for bad in [
        "",
        "TooLarge",
        "TOO_LARGE",
        " too_large",
        "too_large ",
        "foo",
        "open",
        "converted",
        "view_only",
        "refused",
    ] {
        assert_eq!(LoadReason::parse(bad), None, "{bad:?}");
    }
}

#[test]
fn codes_disjoint_from_disposition_names() {
    for d in Disposition::ALL {
        for r in LoadReason::ALL {
            assert_ne!(d.as_str(), r.as_str());
        }
    }
}

#[test]
fn reason_kind_mapping() {
    use LoadReason as L;
    use ReasonKind as K;
    use VersionAxis as A;
    let expected: [(L, Option<K>); 19] = [
        (L::TooLarge, None),
        (L::TooDeep, None),
        (L::TooManyOps, None),
        (L::Truncated, None),
        (L::NotJson, None),
        (L::MissingVersion, Some(K::MissingVersion)),
        (L::Malformed, None),
        (L::HistoryFormatNewer, None),
        (L::OtherPack, None),
        (L::BadVersion, Some(K::UnreadableVersion)),
        (L::FormatNewer, Some(K::HistoryNewer(A::IrFormat))),
        (L::VocabNewer, Some(K::HistoryNewer(A::Vocab))),
        (L::EngineNewer, Some(K::HistoryNewer(A::Engine))),
        (L::FormatOlder, Some(K::HistoryOlder(A::IrFormat))),
        (L::EngineMajor, Some(K::MajorDiffers(A::Engine))),
        (L::ContentMajor, Some(K::MajorDiffers(A::Content))),
        (L::StartMissing, None),
        (L::ReplayMismatch, None),
        (L::ReplayDiverged, Some(K::ReplayDiverged)),
    ];
    for (r, k) in expected {
        assert_eq!(ReasonCode::kind(&r), k, "{r}");
    }
    assert_eq!(expected.iter().filter(|(_, k)| k.is_some()).count(), 9);
}

#[test]
fn reason_kind_is_injective() {
    let kinds: Vec<ReasonKind> = LoadReason::ALL
        .iter()
        .filter_map(ReasonCode::kind)
        .collect();
    assert_eq!(kinds.len(), 9);
    let set: HashSet<ReasonKind> = kinds.iter().copied().collect();
    assert_eq!(set.len(), 9);
}

#[test]
fn verdict_accepts_load_reason() {
    let v = Verdict::<LoadReason>::Refused {
        code: LoadReason::OtherPack,
    };
    assert_eq!(v.code(), Some(&LoadReason::OtherPack));
    let v = Verdict::<LoadReason>::ViewOnly {
        code: LoadReason::ContentMajor,
    };
    assert_eq!(v.disposition(), Disposition::ViewOnly);
}

#[test]
fn display_equals_as_str() {
    for r in LoadReason::ALL {
        assert_eq!(r.to_string(), r.as_str());
    }
}

#[test]
fn uses_storage_contract_version_1() {
    assert_eq!(
        fandhe_workshop_contract::storage::STORAGE_CONTRACT_VERSION,
        1
    );
}
