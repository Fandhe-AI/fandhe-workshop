//! 上限契約（`limits`）の適合試験。値・理由コード・判定の順序・文面・境界サンプルが、
//! spec の決定（D-44・D-45・D-25、SAFE-17〜23・25・41〜43・45）と一致することを固定する。
//! 試験では `unwrap` を使ってよい（失敗はそのまま試験の失敗になる）。

use fandhe_workshop_contract::limits::*;
use std::collections::HashSet;

fn group(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[test]
fn constants_match_decisions() {
    assert_eq!(LIMITS_CONTRACT_VERSION, 3);
    assert_eq!(MAX_IR_BYTES, 20 * 1024 * 1024);
    assert_eq!(MAX_HISTORY_BYTES, 10 * 1024 * 1024);
    assert_eq!(MAX_HISTORY_OPS, 20_000);
    assert_eq!(MAX_JSON_DEPTH, 64);
    assert_eq!(MAX_INPUT_CODE_POINTS, 20_000);
    assert_eq!(MAX_PACK_BYTES, 20 * 1024 * 1024);
    assert_eq!(MAX_MARKDOWN_BYTES, 512 * 1024);
    assert_eq!(MAX_PACK_DIR_DEPTH, 32);
    assert_eq!(MAX_TRANSITIONS_PER_INPUT, 1_000);
    assert_eq!(MAX_COLLECTION_ELEMENTS, 100_000);
    assert_eq!(MAX_STRING_CHARS, 100_000);
    assert_eq!(MAX_CHECKPOINTS, 200);
    // SAFE-20: serde_json の再帰の上限より前で効く。
    const { assert!(MAX_JSON_DEPTH < 128) };
}

#[test]
fn limit_ids_are_complete_and_consistent() {
    let set: HashSet<_> = LimitId::ALL.iter().copied().collect();
    assert_eq!(set.len(), 12);
    let expected = [
        (LimitId::IrBytes, MAX_IR_BYTES, LimitUnit::Bytes),
        (LimitId::HistoryBytes, MAX_HISTORY_BYTES, LimitUnit::Bytes),
        (LimitId::HistoryOps, MAX_HISTORY_OPS as u64, LimitUnit::Ops),
        (
            LimitId::JsonDepth,
            MAX_JSON_DEPTH as u64,
            LimitUnit::JsonLevels,
        ),
        (
            LimitId::InputCodePoints,
            MAX_INPUT_CODE_POINTS as u64,
            LimitUnit::CodePoints,
        ),
        (LimitId::PackBytes, MAX_PACK_BYTES, LimitUnit::Bytes),
        (LimitId::MarkdownBytes, MAX_MARKDOWN_BYTES, LimitUnit::Bytes),
        (
            LimitId::PackDirDepth,
            MAX_PACK_DIR_DEPTH as u64,
            LimitUnit::DirLevels,
        ),
        (
            LimitId::TransitionsPerInput,
            MAX_TRANSITIONS_PER_INPUT as u64,
            LimitUnit::Transitions,
        ),
        (
            LimitId::CollectionElements,
            MAX_COLLECTION_ELEMENTS as u64,
            LimitUnit::Elements,
        ),
        (
            LimitId::StringChars,
            MAX_STRING_CHARS as u64,
            LimitUnit::Chars,
        ),
        (
            LimitId::Checkpoints,
            MAX_CHECKPOINTS as u64,
            LimitUnit::Checkpoints,
        ),
    ];
    for (id, v, u) in expected {
        assert_eq!(id.value(), v);
        assert_eq!(id.unit(), u);
    }
}

#[test]
fn reason_codes_are_exact_and_unique() {
    let mut all = HashSet::new();
    for r in LimitReason::ALL {
        assert!(all.insert(r.as_str()));
        assert_eq!(r.to_string(), r.as_str());
    }
    for c in PackLimitCode::ALL {
        assert!(all.insert(c.as_str()));
        assert_eq!(c.to_string(), c.as_str());
    }
    assert_eq!(all.len(), 7);
    assert_eq!(LimitReason::TooLarge.as_str(), "too_large");
    assert_eq!(LimitReason::TooDeep.as_str(), "too_deep");
    assert_eq!(LimitReason::TooManyOps.as_str(), "too_many_ops");
    assert_eq!(PackLimitCode::PackSize.as_str(), "E-LIMIT-PACK-SIZE");
    assert_eq!(PackLimitCode::MdSize.as_str(), "E-LIMIT-MD-SIZE");
    assert_eq!(PackLimitCode::DirDepth.as_str(), "E-LIMIT-DEPTH");
    assert_eq!(PackLimitCode::Symlink.as_str(), "E-LIMIT-SYMLINK");
}

#[test]
fn serde_round_trips_and_rejects_unknown() {
    for r in LimitReason::ALL {
        let j = serde_json::to_string(&r).unwrap();
        assert_eq!(j, format!("\"{}\"", r.as_str()));
        assert_eq!(serde_json::from_str::<LimitReason>(&j).unwrap(), r);
    }
    for c in PackLimitCode::ALL {
        let j = serde_json::to_string(&c).unwrap();
        assert_eq!(j, format!("\"{}\"", c.as_str()));
        assert_eq!(serde_json::from_str::<PackLimitCode>(&j).unwrap(), c);
    }
    assert!(serde_json::from_str::<LimitReason>("\"TooLarge\"").is_err());
    assert!(serde_json::from_str::<LimitReason>("\"not_json\"").is_err());
    assert!(serde_json::from_str::<PackLimitCode>("\"E-LIMIT\"").is_err());
}

#[test]
fn load_stage_order_is_fixed() {
    assert_eq!(
        LOAD_STAGE_ORDER,
        [
            LoadStage::Size,
            LoadStage::Depth,
            LoadStage::JsonParse,
            LoadStage::TypeAndContent
        ]
    );
    assert!(LOAD_STAGE_ORDER.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(LimitReason::TooLarge.stage(), LoadStage::Size);
    assert_eq!(LimitReason::TooDeep.stage(), LoadStage::Depth);
    assert_eq!(LimitReason::TooManyOps.stage(), LoadStage::TypeAndContent);
}

#[test]
fn messages_are_fixed_and_match_constants() {
    for e in [LoadEntry::Ir, LoadEntry::History] {
        for r in LimitReason::ALL {
            let m = rejection_message(e, r);
            let none = e == LoadEntry::Ir && r == LimitReason::TooManyOps;
            assert_eq!(m.is_none(), none);
            if let Some(m) = m {
                assert!(!m.is_empty());
                assert!(!m.contains('{') && !m.contains('}'));
            }
        }
    }
    let m = |e, r| rejection_message(e, r).unwrap();
    assert!(
        m(LoadEntry::Ir, LimitReason::TooLarge).contains("ファイルが上限の大きさを超えています")
    );
    assert!(m(LoadEntry::History, LimitReason::TooLarge)
        .contains("履歴ファイルが 10MB の上限を超えています"));
    let depth = format!("入れ子が深すぎます（{MAX_JSON_DEPTH} 段まで）");
    assert!(m(LoadEntry::Ir, LimitReason::TooDeep).contains(&depth));
    assert!(m(LoadEntry::History, LimitReason::TooDeep).contains(&depth));
    let ops = format!("{} 件", group(MAX_HISTORY_OPS as u64));
    assert!(m(LoadEntry::History, LimitReason::TooManyOps).contains(&ops));
    assert_eq!(
        INPUT_TOO_LONG_MESSAGE,
        format!(
            "入力は {} 文字までです。超えた分は受け付けていません。",
            group(MAX_INPUT_CODE_POINTS as u64)
        )
    );
}

#[test]
fn pack_codes_map_to_limits() {
    assert_eq!(PackLimitCode::PackSize.limit(), Some(LimitId::PackBytes));
    assert_eq!(PackLimitCode::MdSize.limit(), Some(LimitId::MarkdownBytes));
    assert_eq!(PackLimitCode::DirDepth.limit(), Some(LimitId::PackDirDepth));
    assert_eq!(PackLimitCode::Symlink.limit(), None);
    for c in PackLimitCode::ALL {
        let m = c.message();
        assert!(!m.is_empty());
        assert!(!m.contains('{') && !m.contains('}'));
    }
    assert!(PackLimitCode::DirDepth
        .message()
        .contains(&MAX_PACK_DIR_DEPTH.to_string()));
}

#[test]
fn boundary_samples_cover_all_limits() {
    assert_eq!(BOUNDARY_SAMPLES.len(), LimitId::ALL.len());
    for (s, id) in BOUNDARY_SAMPLES.iter().zip(LimitId::ALL) {
        assert_eq!(s.limit, id);
        assert_eq!(s.exact, id.value());
        assert_eq!(s.over_by_one, s.exact + 1);
        assert_eq!(s.tenfold, s.exact * 10);
    }
    let by = |id| BOUNDARY_SAMPLES.iter().find(|s| s.limit == id).unwrap();
    assert_eq!(by(LimitId::IrBytes).tenfold, 209_715_200);
    assert_eq!(by(LimitId::HistoryBytes).tenfold, 104_857_600);
    assert_eq!(by(LimitId::HistoryOps).tenfold, 200_000);
    assert_eq!(by(LimitId::MarkdownBytes).tenfold, 5_242_880);
}

#[test]
fn ir_value_limit_reasons_are_fixed() {
    let codes: Vec<&str> = IrValueLimitReason::ALL.iter().map(|r| r.as_str()).collect();
    assert_eq!(codes, ["too_many_elements", "string_too_long"]);
    for r in IrValueLimitReason::ALL {
        assert_eq!(r.to_string(), r.as_str());
        assert_eq!(r.stage(), LoadStage::TypeAndContent);
        let m = ir_value_rejection_message(r);
        assert!(m.contains("100,000"), "{m}");
    }
    assert_eq!(
        IrValueLimitReason::TooManyElements.limit(),
        LimitId::CollectionElements
    );
    assert_eq!(
        IrValueLimitReason::StringTooLong.limit(),
        LimitId::StringChars
    );
    // 履歴の経路の理由には足していない。
    assert_eq!(LimitReason::ALL.len(), 3);
}

#[test]
fn value_limit_boundaries_fit_in_ir_budget() {
    for id in [LimitId::CollectionElements, LimitId::StringChars] {
        let s = BOUNDARY_SAMPLES.iter().find(|s| s.limit == id).unwrap();
        assert_eq!(s.exact, 100_000);
        assert_eq!(s.tenfold, 1_000_000);
        assert!(s.tenfold < MAX_IR_BYTES);
    }
}
