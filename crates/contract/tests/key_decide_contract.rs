//! キー判定表 `decide` の入出力の型の適合試験（TASK-257、版 1）。
//!
//! JUDGE-12・13・14・16・17・19 の確定している行を、期待表（`tests/data/key_decide_1/table.json`）と
//! 入力空間の総なめで固定する。未決の行（JUDGE-18・21・22）は名指しの一覧だけを確かめる。
//! 期待表は contract の試験だけが使うため、`testdata/contract/` には置かず、`guidance_1`・`judgment_1` と同じ場所に置く。
//! 試験は作業ツリーを書き換えない。試験では `unwrap` を使ってよい。

use std::collections::BTreeSet;
use std::path::Path;

use fandhe_workshop_contract::key_decide::*;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    id: String,
    source: String,
    input: KeyInput,
    state: FieldState,
    expect: Decision,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    contract_version: u32,
    rows: Vec<Row>,
}

fn table() -> Table {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/key_decide_1/table.json");
    let b = std::fs::read(&p).unwrap();
    serde_json::from_slice(&b).unwrap()
}

fn input(key: &str, mods: u8, signals: u8) -> KeyInput {
    KeyInput {
        key: key.into(),
        shift: mods & 1 != 0,
        ctrl: mods & 2 != 0,
        alt: mods & 4 != 0,
        meta: mods & 8 != 0,
        is_composing: signals & 1 != 0,
        key_code: if signals & 2 != 0 { 229 } else { 0 },
        // しるし 4 つ目（key == "Process"）は key 側で表すため、ここでは使わない。
    }
}

fn state(bits: u8, marker: bool) -> FieldState {
    FieldState {
        composing_marker: marker,
        busy: bits & 1 != 0,
        candidates_open: bits & 2 != 0,
        has_text: bits & 4 != 0,
    }
}

/// 入力空間の総なめ。キー × 修飾キー 16 通り × しるし 8 通り（isComposing・229・自前の印）× 状態 8 通り。
fn sweep() -> Vec<(KeyInput, FieldState)> {
    let keys = ["Enter", "Tab", "Escape", "Process", "a", "ArrowDown"];
    let mut v = Vec::new();
    for k in keys {
        for mods in 0..16u8 {
            for sig in 0..8u8 {
                for st in 0..8u8 {
                    v.push((input(k, mods, sig & 3), state(st, sig & 4 != 0)));
                }
            }
        }
    }
    v
}

fn composing(i: &KeyInput, s: &FieldState) -> bool {
    i.in_composition(s.composing_marker)
}

#[test]
fn version_is_one() {
    assert_eq!(KEY_DECIDE_CONTRACT_VERSION, 1);
    assert_eq!(table().contract_version, KEY_DECIDE_CONTRACT_VERSION);
}

#[test]
fn expected_table_matches() {
    for r in table().rows {
        assert_eq!(
            decide(&r.input, &r.state),
            r.expect,
            "{} ({})",
            r.id,
            r.source
        );
    }
}

#[test]
fn table_covers_confirmed_rows() {
    let t = table();
    for id in [
        "JUDGE-12", "JUDGE-13", "JUDGE-14", "JUDGE-16", "JUDGE-17", "JUDGE-19",
    ] {
        assert!(t.rows.iter().any(|r| r.source.contains(id)), "{id}");
    }
}

#[test]
fn pending_rows_are_named() {
    assert_eq!(PENDING_ROWS.len(), 3);
    let ids: BTreeSet<_> = PENDING_ROWS.iter().map(|p| p.judge_id()).collect();
    assert_eq!(ids, BTreeSet::from(["JUDGE-18", "JUDGE-21", "JUDGE-22"]));
}

#[test]
fn pending_only_for_modified_enter() {
    for (i, s) in sweep() {
        let d = decide(&i, &s);
        let expect_pending = i.key == "Enter" && !composing(&i, &s) && (i.ctrl || i.alt || i.meta);
        match d {
            Decision::Pending(p) => {
                assert!(expect_pending, "{i:?} {s:?}");
                assert_eq!(p, PendingRow::ModifiedEnter);
            }
            Decision::Defined(_) => assert!(!expect_pending, "{i:?} {s:?}"),
        }
    }
}

#[test]
fn judge19_prevent_default_on_tab_only_when_open_and_text() {
    for (i, s) in sweep() {
        if i.key != "Tab" {
            continue;
        }
        let d = decide(&i, &s);
        if let Decision::Defined(k) = d {
            if k.prevent_default {
                assert_eq!(k.action, KeyAction::Complete);
                assert!(s.candidates_open && s.has_text && !i.shift && !composing(&i, &s));
            }
            if i.shift && !composing(&i, &s) {
                assert_eq!(k.action, KeyAction::Nothing, "{i:?} {s:?}");
                assert!(!k.prevent_default);
            }
        }
    }
}

#[test]
fn judge17_three_ways_with_busy_priority() {
    for (i, s) in sweep() {
        if i.key != "Escape" || composing(&i, &s) {
            continue;
        }
        let want = if s.busy {
            KeyAction::Stop
        } else if s.candidates_open {
            KeyAction::CloseCandidates
        } else {
            KeyAction::CancelInput
        };
        assert_eq!(
            decide(&i, &s),
            Decision::Defined(KeyDecision {
                action: want,
                prevent_default: false
            }),
            "{i:?} {s:?}"
        );
    }
}

#[test]
fn judge14_any_single_signal_means_composing() {
    for text in [false, true] {
        {
            let base = FieldState {
                has_text: text,
                ..FieldState::default()
            };
            let enter = |i: KeyInput, s: FieldState| decide(&i, &s);
            let plain = input("Enter", 0, 0);
            let cases = [
                enter(input("Enter", 0, 1), base),
                enter(input("Enter", 0, 2), base),
                enter(input("Process", 0, 0), base),
                enter(
                    plain.clone(),
                    FieldState {
                        composing_marker: true,
                        ..base
                    },
                ),
            ];
            let want = Decision::Defined(KeyDecision {
                action: KeyAction::DelegateToIme,
                prevent_default: false,
            });
            for c in cases {
                assert_eq!(c, want);
            }
        }
    }
}

#[test]
fn judge16_composing_esc_never_reaches_engine() {
    for (i, s) in sweep() {
        if i.key == "Escape" && composing(&i, &s) {
            assert_eq!(
                decide(&i, &s),
                Decision::Defined(KeyDecision {
                    action: KeyAction::DelegateToIme,
                    prevent_default: false
                }),
                "{i:?} {s:?}"
            );
        }
    }
}

#[test]
fn decision_serde_roundtrip() {
    let d = Decision::Defined(KeyDecision {
        action: KeyAction::CloseCandidates,
        prevent_default: false,
    });
    let s = serde_json::to_string(&d).unwrap();
    assert_eq!(
        s,
        r#"{"defined":{"action":"close_candidates","prevent_default":false}}"#
    );
    assert_eq!(serde_json::from_str::<Decision>(&s).unwrap(), d);
    let p = Decision::Pending(PendingRow::EscCountWithCandidates);
    let s = serde_json::to_string(&p).unwrap();
    assert_eq!(s, r#"{"pending":"esc_count_with_candidates"}"#);
    assert_eq!(serde_json::from_str::<Decision>(&s).unwrap(), p);
    assert!(serde_json::from_str::<KeyAction>(r#""Submit""#).is_err());
    assert!(serde_json::from_str::<PendingRow>(r#""unknown""#).is_err());
}
