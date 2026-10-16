//! Shift+Enter が送信せず、既定の動作（欄への改行）を妨げないことの契約試験（TASK-275 2/2。JUDGE-13）。
//!
//! `key_route::effect_for` の `Newline` の腕が変わる変異（JUDGE13-1）の検出先である。`src/` 内の試験は
//! 変異注入の対象にできないため、公開の経路（契約の `decide` と web の `effect_for`）だけで固定する。
//! DOM での結果（改行が欄に入り、複数行が 1 件で送信される）は `tools/browser/enter-submit.test.mjs` が確かめる。

use fandhe_workshop_contract::key_decide::{decide, KeyInput};
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
