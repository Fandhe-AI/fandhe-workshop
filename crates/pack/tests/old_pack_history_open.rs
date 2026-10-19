//! 旧い教材と旧い履歴が新しい版のエンジンで開くこと（04-behavior の EXT-9、TASK-190 の 1/2）。
//!
//! 受け口や拡張点を後から足した版のエンジンでも、公開済みの教材（IR）と受講者の履歴が開けることを、凍結した
//! 成果物で固定する。IR の項目の追加は加算だけで行い、受け口ごとの版は持たない（D-53）。版の違いの扱いは D-52
//! に従う。ここは EXT-9 の「開く」の側で、新しい項目を使わない教材の IR がバイト単位で同じになることは
//! #463（TASK-190 の 2/2）が受け持つ。
//!
//! 「新しい版のエンジン」とは、凍結物を作った後に EP-13（語彙 1.4）が加わった今のエンジンを指す。凍結物は
//! すべて EP-13 より前に作った（版 1 の IR は語彙の宣言が無く、記録上の語彙は既定の 1.0）。次の受け口の追加は、
//! 既存の試験用の口（`OpeningCaps::with_vocab_max`・`record_versions_with`）で模す。模擬であり、過去の版の
//! エンジンの実物はこの repo に無い。
//!
//! 凍結物の置き場と作り方は `tests/data/old_histories_1/README.md`。履歴は読むだけで書き戻さない。
//! 他の試験との分担: 版 1 の golden の読みとバイト一致は `ir_compat.rs`、旧いエンジンの模型は
//! `ir_old_engine.rs`、判定の単体は `crates/core/tests/history_open.rs`。ここでは重ねない。
//!
//! 限界:
//! - 版 1 の履歴を、今 compile し直した版 2 の IR で開くと「表示だけ」になる（D-52 は変換の規則を持たない）。
//!   受け口の追加が原因ではないが、教材の配信が版 2 に替わると続きから再開できないため、境界として固定する
//! - 契約の試料 `two-rounds`・`draft-rejected` は、再読み込みの意味の実装前の写しで、受け口とは別の理由で開かない
//! - 試作の写しの形の `testdata/save-restore-versions/**/h-*.json` は今の型で読めず、対象外
//!
//! 失敗の文言には理由コード・欄の名前・ファイル名だけを出し、履歴の本文や入力値は出さない（SAFE-40）。
//! 試験では `unwrap` を使ってよい。

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::testdata;
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history_version::{
    HistoryVersions, Verdict, Version3, VersionAxis, VocabVersion,
};
use fandhe_workshop_contract::ir::{Ir, SUPPORTED_IR_FORMATS};
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_boot::{boot_session, BootOutcome};
use fandhe_workshop_core::history_load::load_session;
use fandhe_workshop_core::history_open::{OpeningCaps, VOCAB_MAX};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::{RunState, Session, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::version_record::{record_versions, record_versions_with};
use serde_json::Value;

const V1_GOLDENS: [&str; 4] = ["min-3step-pack", "ch1-pack", "ime-pack", "min-3step-pack-b"];
const FROZEN_HISTORY: &str = "tests/data/old_histories_1/min-3step-v1-progress.json";

fn manifest(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn v1_golden_bytes(name: &str) -> Vec<u8> {
    std::fs::read(manifest(&format!("tests/data/ir_format_1/{name}.ir.json"))).unwrap()
}

fn v2_golden_bytes(name: &str) -> Vec<u8> {
    std::fs::read(manifest(&format!("tests/data/golden/{name}.ir.json"))).unwrap()
}

fn frozen_history() -> Vec<u8> {
    std::fs::read(manifest(FROZEN_HISTORY)).unwrap()
}

/// 開始点の Step を `Entered` にした初期状態（`Control::Begin` は未実装のため、開始点の効果は当てない）。
/// 凍結した履歴は、同じ作り方の初期状態から記録した。
fn initial_at_start(ir: &Ir) -> SessionState {
    let uid = StepUid(ir.starts[0].node.clone());
    let mut slice = Slice::default();
    slice.status.insert(uid.clone(), StepStatus::Entered);
    let mut slices = BTreeMap::new();
    slices.insert(uid.root(), slice);
    let run = RunState {
        current: Some(uid),
        slices,
        ..RunState::default()
    };
    SessionState {
        run,
        ..SessionState::default()
    }
}

fn opening(ir: &Ir) -> HistoryVersions {
    record_versions(ir, &content_hash(ir).unwrap()).unwrap()
}

/// 現行のエンジンの上限で `bytes` を開き、状態か判定を返す。
fn open_with(
    bytes: &[u8],
    ir: &Ir,
    opening: &HistoryVersions,
    caps: &OpeningCaps,
    initial: SessionState,
) -> Result<(SessionState, Option<Vec<VersionAxis>>), Verdict<LoadReason>> {
    load_session(bytes, ir.clone(), opening, caps, initial).map(|l| {
        let axes = l
            .notes
            .map(|n| n.diffs().iter().map(|d| d.axis).collect::<Vec<_>>());
        (l.session.state().clone(), axes)
    })
}

fn snapshot_of(bytes: &[u8]) -> SessionState {
    let v: Value = serde_json::from_slice(bytes).unwrap();
    serde_json::from_value(v["snapshot"].clone()).unwrap()
}

fn expect_view_only(
    r: Result<(SessionState, Option<Vec<VersionAxis>>), Verdict<LoadReason>>,
) -> LoadReason {
    match r {
        Err(Verdict::ViewOnly { code }) => code,
        Err(other) => panic!("表示だけを期待した: {:?}", other.code()),
        Ok(_) => panic!("開かないことを期待した"),
    }
}

#[test]
fn ext9_premise_engine_is_newer_than_frozen_artifacts() {
    // 前提が崩れたら（凍結物の語彙が今の上限に並んだら）、この試験群の意味を見直す。
    let frozen: Value = serde_json::from_slice(&frozen_history()).unwrap();
    let recorded = VocabVersion::parse(frozen["versions"]["vocab"].as_str().unwrap()).unwrap();
    assert!(recorded < VOCAB_MAX, "凍結した履歴の語彙は今の上限より古い");
    for name in V1_GOLDENS {
        let v: Value = serde_json::from_slice(&v1_golden_bytes(name)).unwrap();
        assert_eq!(v["format_version"], 1, "{name}: 版 1");
        assert!(
            v["pack"].get("vocab").is_none(),
            "{name}: 語彙の宣言なし（既定 1.0）"
        );
    }
    assert!(
        SUPPORTED_IR_FORMATS.contains(&1),
        "今のエンジンは版 1 の IR を読む"
    );
}

#[test]
fn ext9_frozen_v1_irs_open_as_sessions() {
    for name in V1_GOLDENS {
        let ir = load_ir(&v1_golden_bytes(name)).unwrap_or_else(|e| panic!("{name}: load_ir {e}"));
        assert_eq!(ir.format_version, 1, "{name}: 版 1 のまま読む");
        for s in &ir.starts {
            assert!(
                ir.nodes.iter().any(|n| n.uid == s.node),
                "{name}: 開始点 {} の node が nodes にある",
                s.id
            );
        }
        let initial = initial_at_start(&ir);
        let session = Session::from_state(ir, initial.clone());
        assert_eq!(
            session.state(),
            &initial,
            "{name}: 開始点から実行環境を作れる"
        );
    }
}

#[test]
fn ext9_frozen_history_opens_with_its_v1_ir() {
    let ir = load_ir(&v1_golden_bytes("min-3step-pack")).unwrap();
    let bytes = frozen_history();
    let (state, axes) = open_with(
        &bytes,
        &ir,
        &opening(&ir),
        &OpeningCaps::current(),
        initial_at_start(&ir),
    )
    .unwrap_or_else(|v| panic!("開く: {:?}", v.code()));
    assert_eq!(state, snapshot_of(&bytes), "状態が写しと等しい");
    assert!(axes.is_none(), "変換なしで開く（Open）");

    let outcome = boot_session(
        Some(&bytes),
        ir.clone(),
        &opening(&ir),
        &OpeningCaps::current(),
        initial_at_start(&ir),
        "s-1",
    );
    let BootOutcome::Restored { session, notes } = outcome else {
        panic!("復元される");
    };
    assert!(notes.is_none());
    let recorded: Value = serde_json::from_slice(&bytes).unwrap();
    let n_ops = recorded["ops"].as_array().unwrap().len();
    assert_eq!(
        session.ops().len(),
        n_ops + 1,
        "記録の末尾に再読み込みが 1 件足される"
    );
    let reloads = session
        .session()
        .state()
        .log
        .iter()
        .filter(|e| matches!(e, Event::Reloaded { .. }))
        .count();
    assert_eq!(reloads, 1, "再読み込みの事実が 1 件");
}

#[test]
fn ext9_frozen_history_against_recompiled_v2_ir_is_view_only() {
    // 今 compile し直した同じ教材は版 2 になる。版 1 の履歴は、D-52 のとおり変換の規則を持たないため表示だけになる。
    // 受け口の追加が原因ではない既知の境界で、続きからの再開は版 2 の履歴を新しく記録する場合に限られる。
    let v2 = load_ir(&v2_golden_bytes("min-3step-pack")).unwrap();
    assert_eq!(v2.format_version, 2, "今の compile の出力は版 2");
    let v1 = load_ir(&v1_golden_bytes("min-3step-pack")).unwrap();
    assert_eq!(v1.pack.id, v2.pack.id, "同じ教材");
    let code = expect_view_only(open_with(
        &frozen_history(),
        &v2,
        &opening(&v2),
        &OpeningCaps::current(),
        initial_at_start(&v2),
    ));
    assert_eq!(code, LoadReason::FormatOlder);
}

#[test]
fn ext9_frozen_history_opens_when_pack_redeclares_newer_vocab() {
    // 教材が新しい語彙の版を宣言し直しても（新しい語は使わない）、旧い履歴は変換して開く。
    let mut v: Value = serde_json::from_slice(&v1_golden_bytes("min-3step-pack")).unwrap();
    v["pack"]["vocab"] = Value::String(VOCAB_MAX.to_string());
    let ir = load_ir(&serde_json::to_vec(&v).unwrap()).unwrap();
    let bytes = frozen_history();
    let (state, axes) = open_with(
        &bytes,
        &ir,
        &opening(&ir),
        &OpeningCaps::current(),
        initial_at_start(&ir),
    )
    .unwrap_or_else(|v| panic!("開く: {:?}", v.code()));
    assert_eq!(state, snapshot_of(&bytes), "状態が写しと等しい");
    let axes = axes.expect("変換して開く（Converted）");
    assert!(
        axes.contains(&VersionAxis::Vocab),
        "違いの軸に vocab を含む"
    );
    assert!(
        !axes.contains(&VersionAxis::IrFormat) && !axes.contains(&VersionAxis::Engine),
        "版 1 のまま・同じエンジン: {axes:?}"
    );
}

#[test]
fn ext9_frozen_history_opens_in_a_next_engine_with_one_more_extension_point() {
    // 次の受け口の追加を、既存の試験用の口で模す（語彙の上限を 1.5、エンジンの副版を 1 つ上げる）。実物の別版ではない。
    let ir = load_ir(&v1_golden_bytes("min-3step-pack")).unwrap();
    let next_vocab = VocabVersion {
        major: VOCAB_MAX.major,
        minor: VOCAB_MAX.minor + 1,
    };
    let current = Version3::parse(&opening(&ir).engine).unwrap();
    let next_engine =
        Version3::parse(&format!("{}.{}.0", current.major, current.minor + 1)).unwrap();
    let next = record_versions_with(&ir, next_engine, &content_hash(&ir).unwrap()).unwrap();
    let bytes = frozen_history();
    let (state, axes) = open_with(
        &bytes,
        &ir,
        &next,
        &OpeningCaps::with_vocab_max(next_vocab),
        initial_at_start(&ir),
    )
    .unwrap_or_else(|v| panic!("開く: {:?}", v.code()));
    assert_eq!(state, snapshot_of(&bytes), "状態が写しと等しい");
    assert_eq!(
        axes,
        Some(vec![VersionAxis::Engine]),
        "違いは engine の軸だけ"
    );
}

/// 契約の試料の教材（min-3step の JSON）と、試料が使う初期状態。
fn contract_ir_and_initial() -> (Ir, SessionState) {
    let ir = load_ir(
        &std::fs::read(testdata(
            "engine-event-consistency/test-data/min-3step.json",
        ))
        .unwrap(),
    )
    .unwrap();
    let v: Value =
        serde_json::from_slice(&std::fs::read(testdata("contract/event-sequence.json")).unwrap())
            .unwrap();
    (ir, serde_json::from_value(v["initial"].clone()).unwrap())
}

fn contract_history(name: &str) -> Vec<u8> {
    std::fs::read(testdata(&format!("contract/history/{name}.json"))).unwrap()
}

#[test]
fn ext9_contract_empty_history_sample_opens_in_current_engine() {
    let (ir, initial) = contract_ir_and_initial();
    let bytes = contract_history("empty");
    let (state, axes) = open_with(
        &bytes,
        &ir,
        &opening(&ir),
        &OpeningCaps::current(),
        initial.clone(),
    )
    .unwrap_or_else(|v| panic!("開く: {:?}", v.code()));
    assert_eq!(state, snapshot_of(&bytes), "状態が写しと等しい");
    assert!(axes.is_none(), "変換なしで開く");
    let outcome = boot_session(
        Some(&bytes),
        ir.clone(),
        &opening(&ir),
        &OpeningCaps::current(),
        initial,
        "s-1",
    );
    assert!(
        matches!(outcome, BootOutcome::Fresh(_)),
        "ops が 0 件なら新しく始める"
    );
}

#[test]
fn ext9_stale_contract_samples_are_closed_for_a_non_version_reason() {
    // `two-rounds`・`draft-rejected` の写しは、再読み込みの意味を実装する前に作られた（`history_load_session.rs` の注）。
    // 受け口の追加ではなく、適用し直した結果が写しと一致しないこと（段 8）で開かない。段 8 に届くのは段 6 の版の検査を
    // すべて通った後なので、版の違いが原因でないことも同時に言える。限界として固定し、ここで写しを直さない。
    let (ir, initial) = contract_ir_and_initial();
    let version_reasons = [
        LoadReason::FormatOlder,
        LoadReason::FormatNewer,
        LoadReason::VocabNewer,
        LoadReason::EngineNewer,
        LoadReason::EngineMajor,
        LoadReason::ContentMajor,
        LoadReason::BadVersion,
    ];
    for name in ["two-rounds", "draft-rejected"] {
        let r = open_with(
            &contract_history(name),
            &ir,
            &opening(&ir),
            &OpeningCaps::current(),
            initial.clone(),
        );
        let Err(Verdict::Refused { code }) = r else {
            panic!("{name}: 開かない（Refused）ことを期待した");
        };
        assert_eq!(code, LoadReason::ReplayMismatch, "{name}");
        assert!(!version_reasons.contains(&code), "{name}: 版の理由ではない");
    }
}

#[test]
fn ext9_history_versions_have_no_per_extension_point_version() {
    // 受け口ごとの版は持たない（D-53）。`versions` の欄は 4 軸と `content_hash`・`refs` だけである。
    let bytes = frozen_history();
    let mut v: Value = serde_json::from_slice(&bytes).unwrap();
    let keys: Vec<&str> = v["versions"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let mut keys = keys;
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "content",
            "content_hash",
            "engine",
            "ir_format",
            "refs",
            "vocab"
        ],
        "versions の欄の集合"
    );

    // 受け口ごとの版の欄を足した包みは、開かずに断られる。理由は欄の値を含まない固定のコードだけ。
    v["versions"]["ep_sentinel_zz"] = Value::String("1.0".to_owned());
    let ir = load_ir(&v1_golden_bytes("min-3step-pack")).unwrap();
    let verdict = load_session(
        &serde_json::to_vec(&v).unwrap(),
        ir.clone(),
        &opening(&ir),
        &OpeningCaps::current(),
        initial_at_start(&ir),
    )
    .map(|_| ())
    .expect_err("受け口ごとの版の欄は開かない");
    assert_eq!(
        verdict,
        Verdict::Refused {
            code: LoadReason::Malformed
        }
    );
    assert!(
        !format!("{verdict:?}").contains("sentinel"),
        "理由に欄の名前の値を含めない"
    );
}
