//! Shift+Enter が送信せず、既定の動作（欄への改行）を妨げないことの契約試験（TASK-275 2/2。JUDGE-13）。
//!
//! `key_route::effect_for` の `Newline` の腕が変わる変異（JUDGE13-1）の検出先である。`src/` 内の試験は
//! 変異注入の対象にできないため、公開の経路（契約の `decide` と web の `effect_for`）だけで固定する。
//! 処理中の Esc を停止にする経路（`esc_route`）が外れる変異（JUDGE17-2）と、処理中でない Esc の 2 通り
//! （候補を閉じる・取り消しの入力）が外れる変異（JUDGE17-3・JUDGE17-4）の検出先でもある。
//! 候補が開いているときの Esc の 2 回（JUDGE-18）の遷移を外す変異（JUDGE18-2）と、補完の候補を先頭から変える変異（JUDGE21-2）の検出先でもある（TASK-282 1/2）。
//! DOM での結果（改行が欄に入り、複数行が 1 件で送信される）は `tools/browser/enter-submit.test.mjs` が確かめる。

use fandhe_workshop_contract::judgment::{Judgment, RejectCategory};
use fandhe_workshop_contract::key_decide::{decide, Decision, KeyAction, KeyDecision, KeyInput};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{apply, load_ir};
use fandhe_workshop_web::glue_ops::tab_prevents_default;
use fandhe_workshop_web::key_route::{
    after_esc, completion_choice, effect_for, esc_input, esc_route, field_state, EscRoute,
};
use std::path::PathBuf;

#[test]
fn shift_enter_は_prevent_default_せず送信もしない() {
    let input = KeyInput {
        key: "Enter".to_owned(),
        key_code: 0,
        is_composing: false,
        shift: true,
        ctrl: false,
        alt: false,
        meta: false,
    };
    for has_text in [true, false] {
        let effect = effect_for(decide(&input, &field_state(has_text)));
        assert!(!effect.prevent_default, "has_text={has_text}");
        assert!(!effect.click_submit, "has_text={has_text}");
    }
}

#[test]
fn ctrl_v_と_meta_v_は_prevent_default_せず送信もしない() {
    // JUDGE-25: 貼り付けはブラウザの既定動作に任せる。keydown 側で止めると貼り付けが失われる。
    for (ctrl, meta) in [(true, false), (false, true)] {
        let input = KeyInput {
            key: "v".to_owned(),
            key_code: 0,
            is_composing: false,
            shift: false,
            ctrl,
            alt: false,
            meta,
        };
        for has_text in [true, false] {
            let effect = effect_for(decide(&input, &field_state(has_text)));
            assert!(
                !effect.prevent_default,
                "ctrl={ctrl} meta={meta} has_text={has_text}"
            );
            assert!(
                !effect.click_submit,
                "ctrl={ctrl} meta={meta} has_text={has_text}"
            );
        }
    }
}

// ---- Tab の補完の条件（TASK-278 1/3。JUDGE-19・FEND-21・D-42） ----
//
// 変異 JUDGE19-2（`Complete` の腕を何もしない効果に戻す）の検出先。候補が開く状態は TASK-440 が結ぶまで
// 画面から作れないため、`FieldState` を直接組み立てて固定する。

fn tab(shift: bool, ctrl: bool, alt: bool, meta: bool) -> KeyInput {
    KeyInput {
        key: "Tab".to_owned(),
        key_code: 0,
        is_composing: false,
        shift,
        ctrl,
        alt,
        meta,
    }
}

#[test]
fn tab_は候補が開いていて文字があるときだけ_prevent_default_する() {
    // 修飾 4 通り（なし・ctrl・alt・meta）× shift × 候補 × 文字 の 32 セル（busy・変換中の印は偽）。
    for modifier in 0..4 {
        for shift in [false, true] {
            for candidates_open in [false, true] {
                for has_text in [false, true] {
                    let input = tab(shift, modifier == 1, modifier == 2, modifier == 3);
                    let mut state = field_state(has_text);
                    state.candidates_open = candidates_open;
                    let effect = effect_for(decide(&input, &state));
                    let cell = format!(
                        "modifier={modifier} shift={shift} candidates_open={candidates_open} has_text={has_text}"
                    );
                    let expected = modifier == 0 && !shift && candidates_open && has_text;
                    assert_eq!(effect.prevent_default, expected, "{cell}");
                    assert!(!effect.click_submit, "{cell}");
                    if modifier == 0 && !shift {
                        assert_eq!(
                            effect.prevent_default,
                            tab_prevents_default(candidates_open, has_text),
                            "{cell}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn 変換中の_tab_は候補が開いていても_prevent_default_しない() {
    let mut composing = tab(false, false, false, false);
    composing.is_composing = true;
    let mut code_229 = tab(false, false, false, false);
    code_229.key_code = 229;
    let mut state = field_state(true);
    state.candidates_open = true;
    for input in [composing, code_229] {
        let effect = effect_for(decide(&input, &state));
        assert!(!effect.prevent_default, "{input:?}");
        assert!(!effect.click_submit, "{input:?}");
    }
    state.composing_marker = true;
    let effect = effect_for(decide(&tab(false, false, false, false), &state));
    assert!(!effect.prevent_default);
}

// ---- Shift+Tab の逆向きのフォーカス移動（TASK-278 2/3。JUDGE-19・D-40） ----

#[test]
fn shift_tab_は候補が開いていて文字があっても_prevent_default_せず送信もしない() {
    for modifier in 0..4 {
        for candidates_open in [false, true] {
            for has_text in [false, true] {
                let input = tab(true, modifier == 1, modifier == 2, modifier == 3);
                let mut state = field_state(has_text);
                state.candidates_open = candidates_open;
                let cell = format!(
                    "modifier={modifier} candidates_open={candidates_open} has_text={has_text}"
                );
                let decision = decide(&input, &state);
                assert_eq!(
                    decision,
                    Decision::Defined(KeyDecision {
                        action: KeyAction::Nothing,
                        prevent_default: false,
                    }),
                    "{cell}"
                );
                let effect = effect_for(decision);
                assert!(!effect.prevent_default, "{cell}");
                assert!(!effect.click_submit, "{cell}");
            }
        }
    }
}

// ---- 処理中の Esc の停止（TASK-277 1/2。JUDGE-17 (1)・TERM-7・D-22） ----
//
// 変異 JUDGE17-2（`Stop` の腕を何もしない行き先に戻す）の検出先。処理中の状態は TASK-347 が結ぶまで
// 画面から作れないため、`FieldState` を直接組み立てて固定する。

#[test]
fn 処理中の_esc_は候補が開いていても停止になり候補も閉じる() {
    for candidates_open in [false, true] {
        for has_text in [false, true] {
            let mut state = field_state(has_text);
            state.busy = true;
            state.candidates_open = candidates_open;
            let decision = decide(&esc_input(), &state);
            let cell = format!("candidates_open={candidates_open} has_text={has_text}");
            let route = esc_route(decision);
            assert_eq!(route.op, Some(Op::Control(Control::Stop)), "{cell}");
            assert!(route.close_candidates, "{cell}");
            // glue の keydown では何もしない（root の中の Esc は上流の属性で 1 回だけ届く）。
            let effect = effect_for(decide(&esc_input(), &state));
            assert!(!effect.prevent_default, "{cell}");
            assert!(!effect.click_submit, "{cell}");
        }
    }
}

#[test]
fn 処理中でない_esc_は停止にならない() {
    for candidates_open in [false, true] {
        for has_text in [false, true] {
            let mut state = field_state(has_text);
            state.busy = false;
            state.candidates_open = candidates_open;
            let route = esc_route(decide(&esc_input(), &state));
            assert_ne!(
                route.op,
                Some(Op::Control(Control::Stop)),
                "{candidates_open} {has_text}"
            );
        }
    }
}

#[test]
fn 変換中の_esc_は処理中でも停止にならない() {
    let mut composing = esc_input();
    composing.is_composing = true;
    let mut code_229 = esc_input();
    code_229.key_code = 229;
    for (label, input, marker) in [
        ("is_composing", composing, false),
        ("key_code", code_229, false),
        ("marker", esc_input(), true),
    ] {
        let mut state = field_state(true);
        state.busy = true;
        state.candidates_open = true;
        state.composing_marker = marker;
        assert_eq!(esc_route(decide(&input, &state)), EscRoute::NONE, "{label}");
    }
}

// ---- 処理中でない Esc の 2 通り（TASK-277 2/2。JUDGE-17 (2)・(3)・EVT-10） ----
//
// 変異 JUDGE17-3（候補を閉じる腕を何もしない行き先に戻す）・JUDGE17-4（取り消しの腕を同様に戻す）の検出先。
// 候補が開く状態は TASK-440、Component の結線は TASK-347 が担うため、`FieldState` を直接組み立てて固定する。

fn idle_state(
    has_text: bool,
    candidates_open: bool,
) -> fandhe_workshop_contract::key_decide::FieldState {
    let mut state = field_state(has_text);
    state.busy = false;
    state.candidates_open = candidates_open;
    state
}

#[test]
fn 処理中でなく候補が開いている_esc_は候補を閉じるだけで操作を出さない() {
    for has_text in [false, true] {
        let state = idle_state(has_text, true);
        let route = esc_route(decide(&esc_input(), &state));
        assert_eq!(route.op, None, "has_text={has_text}");
        assert!(route.close_candidates, "has_text={has_text}");
        let effect = effect_for(decide(&esc_input(), &state));
        assert!(!effect.prevent_default, "has_text={has_text}");
        assert!(!effect.click_submit, "has_text={has_text}");
    }
}

#[test]
fn 処理中でなく候補が閉じている_esc_は取り消しの入力になる() {
    for has_text in [false, true] {
        let route = esc_route(decide(&esc_input(), &idle_state(has_text, false)));
        assert_eq!(
            route.op,
            Some(Op::Input(Input::Cancel)),
            "has_text={has_text}"
        );
        assert!(!route.close_candidates, "has_text={has_text}");
    }
}

#[test]
fn 変換中の_esc_は処理中でなくても何もしない() {
    let mut composing = esc_input();
    composing.is_composing = true;
    let mut code_229 = esc_input();
    code_229.key_code = 229;
    for (label, input, marker) in [
        ("is_composing", composing, false),
        ("key_code", code_229, false),
        ("marker", esc_input(), true),
    ] {
        for candidates_open in [false, true] {
            let mut state = idle_state(true, candidates_open);
            state.composing_marker = marker;
            assert_eq!(
                esc_route(decide(&input, &state)),
                EscRoute::NONE,
                "{label} {candidates_open}"
            );
        }
    }
}

#[test]
fn 取り消しの入力は規則の無い_step_で拒否として記録される() {
    let testdata = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata.join("contract/event-sequence.json")).expect("read sample"),
    )
    .expect("parse sample");
    let before: SessionState =
        serde_json::from_value(sample["initial"].clone()).expect("initial state");
    let ir_path = sample["ir"].as_str().expect("ir path");
    let ir = load_ir(&std::fs::read(testdata.join(ir_path)).expect("read ir")).expect("load ir");

    let route = esc_route(decide(&esc_input(), &field_state(false)));
    let op = route.op.expect("取り消しの入力が出るはず");
    let (next, events) = apply(&ir, before.clone(), op);
    match events.as_slice() {
        [Event::InputReceived {
            input: Input::Cancel,
            ..
        }, Event::Judged {
            judgment: Judgment::Rejected(r),
            ..
        }] => assert_eq!(r.category, RejectCategory::NotInThisStep),
        other => panic!("取り消しの拒否だけのはず: {other:?}"),
    }
    assert_eq!(next.run, before.run);
    assert_eq!(next.log.len(), before.log.len() + 2);
}

#[test]
fn judge23_上限ちょうどの下書きでも文字キーは止めず_enter_は送信になる() {
    // keydown の判定は下書きの長さを見ない。上限は `input` の段（`SetDraft` → `clamp_draft`）で切る設計で、
    // 上限ちょうどでも文字キーを止めない（止めると超過の案内が出せない）。
    let key = |k: &str| KeyInput {
        key: k.to_owned(),
        key_code: 0,
        is_composing: false,
        shift: false,
        ctrl: false,
        alt: false,
        meta: false,
    };
    let effect = effect_for(decide(&key("x"), &field_state(true)));
    assert!(!effect.prevent_default);
    assert!(!effect.click_submit);

    let effect = effect_for(decide(&key("Enter"), &field_state(true)));
    assert!(effect.prevent_default);
    assert!(effect.click_submit);
}

// ---- 候補が開いているときの Esc の 2 回（TASK-282 1/2。JUDGE-18・D-60） ----
//
// 変異 JUDGE18-2（`after_esc` が候補を閉じたままにしない）の検出先。候補が開く状態は TASK-440 が結ぶまで
// 画面から作れないため、`FieldState` を直接組み立てて固定する。

#[test]
fn 候補が開いているときの_esc_は1回目で閉じ2回目で取り消しになる() {
    for has_text in [false, true] {
        let first_state = idle_state(has_text, true);
        let first = esc_route(decide(&esc_input(), &first_state));
        // 1 回目は閉じるだけで、取り消しの操作は出ない。
        assert_eq!(first.op, None, "has_text={has_text}");
        assert!(first.close_candidates, "has_text={has_text}");

        let second_state = after_esc(first_state, &first);
        assert!(!second_state.candidates_open, "has_text={has_text}");
        assert_eq!(second_state.has_text, has_text);
        assert!(!second_state.busy);

        let second = esc_route(decide(&esc_input(), &second_state));
        assert_eq!(
            second.op,
            Some(Op::Input(Input::Cancel)),
            "has_text={has_text}"
        );
        assert!(!second.close_candidates, "has_text={has_text}");
        // 取り消した後も状態は変わらない。
        assert_eq!(after_esc(second_state, &second), second_state);
    }
}

#[test]
fn 閉じた後に候補がまた開けば_esc_は再び閉じるだけになる() {
    let first_state = idle_state(true, true);
    let first = esc_route(decide(&esc_input(), &first_state));
    let mut reopened = after_esc(first_state, &first);
    reopened.candidates_open = true;
    let again = esc_route(decide(&esc_input(), &reopened));
    assert_eq!(again.op, None);
    assert!(again.close_candidates);
}

#[test]
fn 処理中の_esc_は何回押しても停止で取り消しにならない() {
    let mut state = idle_state(true, true);
    state.busy = true;
    let first = esc_route(decide(&esc_input(), &state));
    assert_eq!(first.op, Some(Op::Control(Control::Stop)));
    assert!(first.close_candidates);
    let next = after_esc(state, &first);
    assert!(next.busy);
    assert!(!next.candidates_open);
    let second = esc_route(decide(&esc_input(), &next));
    assert_eq!(second.op, Some(Op::Control(Control::Stop)));
}

#[test]
fn 候補が閉じている状態と変換中の_esc_は状態を変えない() {
    for has_text in [false, true] {
        let closed = idle_state(has_text, false);
        let route = esc_route(decide(&esc_input(), &closed));
        assert_eq!(after_esc(closed, &route), closed);
    }
    let mut composing = esc_input();
    composing.is_composing = true;
    let open = idle_state(true, true);
    let route = esc_route(decide(&composing, &open));
    assert_eq!(route, EscRoute::NONE);
    assert_eq!(after_esc(open, &route), open);
}

#[test]
fn esc_は_prevent_default_も_click_もしない() {
    for busy in [false, true] {
        for candidates_open in [false, true] {
            let mut state = idle_state(true, candidates_open);
            state.busy = busy;
            let effect = effect_for(decide(&esc_input(), &state));
            assert!(!effect.prevent_default, "{busy} {candidates_open}");
            assert!(!effect.click_submit, "{busy} {candidates_open}");
        }
    }
}

// ---- 補完の選び方（TASK-282 1/2。JUDGE-21・D-60） ----
//
// 変異 JUDGE21-2（先頭ではなく末尾の候補で補完する）の検出先。矢印キーで候補を選ぶ操作は無い。

#[test]
fn 補完は先頭の候補を選ぶ() {
    let state = idle_state(true, true);
    let decision = decide(&tab(false, false, false, false), &state);
    assert_eq!(
        completion_choice(decision, &["/help", "/hint"]),
        Some(&"/help")
    );
    assert_eq!(completion_choice(decision, &["/only"]), Some(&"/only"));
    assert_eq!(
        completion_choice(decision, &["/a", "/b", "/c"]),
        Some(&"/a")
    );
    assert_eq!(completion_choice::<&str>(decision, &[]), None);
}

#[test]
fn 補完でない判定では候補を選ばない() {
    let candidates = ["/help", "/hint"];
    let open = idle_state(true, true);
    let closed = idle_state(true, false);
    let mut busy = idle_state(true, true);
    busy.busy = true;
    let mut composing_tab = tab(false, false, false, false);
    composing_tab.is_composing = true;
    let mut enter = esc_input();
    enter.key = "Enter".to_owned();
    let cases = [
        decide(&tab(false, false, false, false), &closed),
        decide(&tab(false, false, false, false), &idle_state(false, true)),
        decide(&tab(true, false, false, false), &open),
        decide(&composing_tab, &open),
        decide(&esc_input(), &open),
        decide(&esc_input(), &closed),
        decide(&esc_input(), &busy),
        decide(&enter, &open),
    ];
    for (i, decision) in cases.into_iter().enumerate() {
        assert_eq!(completion_choice(decision, &candidates), None, "{i}");
    }
}

#[test]
fn 矢印キーは候補を選ばずキャレット移動を奪わない() {
    let candidates = ["/help", "/hint"];
    for key in ["ArrowDown", "ArrowUp", "ArrowLeft", "ArrowRight"] {
        let mut input = esc_input();
        input.key = key.to_owned();
        for has_text in [false, true] {
            for candidates_open in [false, true] {
                let state = idle_state(has_text, candidates_open);
                let decision = decide(&input, &state);
                let cell = format!("{key} {has_text} {candidates_open}");
                assert_eq!(
                    decision,
                    Decision::Defined(KeyDecision {
                        action: KeyAction::Nothing,
                        prevent_default: false,
                    }),
                    "{cell}"
                );
                let effect = effect_for(decision);
                assert!(!effect.prevent_default, "{cell}");
                assert!(!effect.click_submit, "{cell}");
                assert_eq!(esc_route(decision), EscRoute::NONE, "{cell}");
                assert_eq!(completion_choice(decision, &candidates), None, "{cell}");
            }
        }
    }
}
