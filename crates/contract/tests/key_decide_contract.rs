//! キー判定表 `decide` の入出力の型の適合試験（TASK-257 版 1、TASK-281 版 2）。
//!
//! JUDGE-12・13・14・16・17・18・19・21・22 の行を、期待表（`tests/data/key_decide_2/table.json`）と
//! 入力空間の総なめで固定する。版 1 の期待表（`key_decide_1/table.json`）は、版 2 との差分の照合だけに使う。
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

fn load(rel: &str) -> Table {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    let b = std::fs::read(&p).unwrap();
    serde_json::from_slice(&b).unwrap()
}

fn table() -> Table {
    load("tests/data/key_decide_2/table.json")
}

fn table_v1() -> Table {
    load("tests/data/key_decide_1/table.json")
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
fn version_is_two() {
    assert_eq!(KEY_DECIDE_CONTRACT_VERSION, 2);
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
        "JUDGE-12", "JUDGE-13", "JUDGE-14", "JUDGE-16", "JUDGE-17", "JUDGE-18", "JUDGE-19",
        "JUDGE-21", "JUDGE-22",
    ] {
        assert!(t.rows.iter().any(|r| r.source.contains(id)), "{id}");
    }
}

#[test]
fn no_pending_rows_in_v2() {
    assert!(PENDING_ROWS.is_empty());
    // 版 1 で未決だった行の名前は、版 1 の表を読むために残している。
    let ids: BTreeSet<_> = [
        PendingRow::ModifiedEnter,
        PendingRow::CompletionChoice,
        PendingRow::EscCountWithCandidates,
    ]
    .iter()
    .map(|p| p.judge_id())
    .collect();
    assert_eq!(ids, BTreeSet::from(["JUDGE-18", "JUDGE-21", "JUDGE-22"]));
}

#[test]
fn decide_never_returns_pending() {
    for (i, s) in sweep() {
        assert!(
            matches!(decide(&i, &s), Decision::Defined(_)),
            "{i:?} {s:?}"
        );
    }
}

/// 版 1 との差分の記録。版 2 で値が変わったのは、版 1 の `Pending`（JUDGE-22）の行だけである。
#[test]
fn v2_differs_from_v1_only_in_pending_rows() {
    let v1 = table_v1();
    let v2 = table();
    assert_eq!(v1.contract_version, 1);
    let nothing = Decision::Defined(KeyDecision {
        action: KeyAction::Nothing,
        prevent_default: false,
    });
    let mut pending = 0;
    for r1 in &v1.rows {
        let r2 = v2
            .rows
            .iter()
            .find(|r| r.id == r1.id)
            .unwrap_or_else(|| panic!("{} が版 2 に無い", r1.id));
        assert_eq!(r1.input, r2.input, "{}", r1.id);
        assert_eq!(r1.state, r2.state, "{}", r1.id);
        match r1.expect {
            Decision::Pending(p) => {
                pending += 1;
                assert_eq!(p, PendingRow::ModifiedEnter, "{}", r1.id);
                assert_eq!(r2.expect, nothing, "{}", r1.id);
            }
            Decision::Defined(_) => assert_eq!(r1.expect, r2.expect, "{}", r1.id),
        }
    }
    assert_eq!(pending, 4);
}

#[test]
fn judge22_modified_enter_is_nothing() {
    for (i, s) in sweep() {
        if i.key == "Enter" && !composing(&i, &s) && (i.ctrl || i.alt || i.meta) {
            assert_eq!(
                decide(&i, &s),
                Decision::Defined(KeyDecision {
                    action: KeyAction::Nothing,
                    prevent_default: false
                }),
                "{i:?} {s:?}"
            );
        }
    }
}

#[test]
fn judge18_two_escapes_close_then_cancel() {
    let esc = input("Escape", 0, 0);
    let first = FieldState {
        busy: false,
        candidates_open: true,
        has_text: true,
        ..FieldState::default()
    };
    assert_eq!(
        decide(&esc, &first),
        Decision::Defined(KeyDecision {
            action: KeyAction::CloseCandidates,
            prevent_default: false
        })
    );
    // 呼び出し側が候補を閉じた状態で、2 回目の Esc を送る。
    let second = FieldState {
        candidates_open: false,
        ..first
    };
    assert_eq!(
        decide(&esc, &second),
        Decision::Defined(KeyDecision {
            action: KeyAction::CancelInput,
            prevent_default: false
        })
    );
}

#[test]
fn judge21_completes_with_first_candidate() {
    assert_eq!(first_completion(&["/help", "/hint"]), Some(&"/help"));
    assert_eq!(first_completion(&["/help"]), Some(&"/help"));
    assert_eq!(first_completion::<&str>(&[]), None);
}

#[test]
fn judge21_no_selection_state_in_contract() {
    let j = r#"{"composing_marker":false,"busy":false,"candidates_open":true,"has_text":true,"selected_candidate":1}"#;
    assert!(serde_json::from_str::<FieldState>(j).is_err());
    for (i, s) in sweep() {
        if i.key == "ArrowDown" && s.candidates_open {
            assert_eq!(
                decide(&i, &s),
                Decision::Defined(KeyDecision {
                    action: KeyAction::Nothing,
                    prevent_default: false
                }),
                "{i:?} {s:?}"
            );
        }
    }
    let open = FieldState {
        candidates_open: true,
        has_text: true,
        ..FieldState::default()
    };
    assert_eq!(
        decide(&input("ArrowUp", 0, 0), &open),
        Decision::Defined(KeyDecision {
            action: KeyAction::Nothing,
            prevent_default: false
        })
    );
}

/// JUDGE-24 は検討中で、`decide` の対象外。貼り付けの keydown（Ctrl+V・Meta+V）は止めない。
#[test]
fn judge24_paste_keydown_is_not_intercepted() {
    for mods in [2u8, 8u8] {
        for st in 0..8u8 {
            let i = input("v", mods, 0);
            assert_eq!(
                decide(&i, &state(st, false)),
                Decision::Defined(KeyDecision {
                    action: KeyAction::Nothing,
                    prevent_default: false
                }),
                "{i:?} {st}"
            );
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
