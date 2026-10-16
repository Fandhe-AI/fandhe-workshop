//! Shift+Enter が送信せず、既定の動作（欄への改行）を妨げないことの契約試験（TASK-275 2/2。JUDGE-13）。
//!
//! `key_route::effect_for` の `Newline` の腕が変わる変異（JUDGE13-1）の検出先である。`src/` 内の試験は
//! 変異注入の対象にできないため、公開の経路（契約の `decide` と web の `effect_for`）だけで固定する。
//! DOM での結果（改行が欄に入り、複数行が 1 件で送信される）は `tools/browser/enter-submit.test.mjs` が確かめる。

use fandhe_workshop_contract::key_decide::{decide, KeyInput};
use fandhe_workshop_web::glue_ops::tab_prevents_default;
use fandhe_workshop_web::key_route::{effect_for, field_state};

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
