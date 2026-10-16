//! keydown の判定結果を、glue が行う効果へ変える純関数（TASK-275。JUDGE-12・JUDGE-13・FEND-14・D-42）。
//!
//! 呼び出し元は `glue` の keydown リスナーだけである。判定表 `decide`（TASK-257）の結果のうち、
//! 修飾なしの Enter（`Submit`）を「送信ボタンへの click」に結び、Tab の補完（`Complete`。JUDGE-19・FEND-21）を
//! `preventDefault` に結ぶ。click は上流の Runtime の配線を通り、
//! 送信ボタンを押した場合と同じ `update` に届くため、入力も記録も同じになる。ボタンが無効なら何も起きない。
//!
//! この層は DOM に触れない。wasm 専用にしないのは、native の試験で行ごとの効果を固定するためである。
//!
//! 受け持たない行（glue は何もしない）と担当:
//! - `Newline`: textarea の既定動作で改行する。`preventDefault` しない（JUDGE-13。`tools/browser/enter-submit.test.mjs` が確かめる）
//! - `DelegateToIme`: IME に任せる。自前の変換中の印は TASK-276
//! - `Pending(_)`: 修飾キー付き Enter（JUDGE-22）などは版 1 で未決。送信も `preventDefault` もしない（版 2 は TASK-281）
//! - `CloseCandidates`・`Stop`・`CancelInput`・`Nothing`: TASK-276・TASK-277・TASK-347
//!
//! Tab の補完は、候補の一覧が開いていて入力欄に文字があるときだけ `preventDefault` を呼ぶ（TASK-278 1/3）。
//! それ以外の Tab は呼ばず、ブラウザの通常のフォーカス移動に任せる。補完の挿入そのものは TASK-440 2/2 の担当で、
//! ここでは行わない。

use fandhe_workshop_contract::key_decide::{Decision, FieldState, KeyAction, KeyDecision};

/// glue が keydown に対して行う効果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeydownEffect {
    /// `preventDefault` を呼ぶか。
    pub prevent_default: bool,
    /// 送信ボタンへ click を送るか。
    pub click_submit: bool,
}

impl KeydownEffect {
    const NONE: Self = Self {
        prevent_default: false,
        click_submit: false,
    };

    /// click を伴わず、`preventDefault` だけを判定のとおりに行う効果（Tab の補完）。
    const fn prevent_only(prevent_default: bool) -> Self {
        Self {
            prevent_default,
            click_submit: false,
        }
    }
}

/// 判定表へ渡す入力欄の状態。`has_text` は App の下書きから取る（DOM の値は読まない）。
///
/// 他の値は、対応する状態がまだ存在しないため偽にする。状態が加わるタスクが、この関数の 1 か所を差し替える。
/// - `composing_marker`: 自前の変換中の印は TASK-276 の担当。印が無くても、`isComposing`・keyCode 229・
///   `Process` のいずれかで判定表が変換中と見なすため、変換中の Enter は送信されない
/// - `busy`: 進行の待機の状態が未実装（TASK-347 以降）
/// - `candidates_open`: 補完の候補の一覧が未実装。TASK-440 が本関数の 1 か所で差し替える。
///   差し替えるまで Tab の補完の行には到達せず、Tab は常に通常のフォーカス移動になる
pub fn field_state(has_text: bool) -> FieldState {
    FieldState {
        composing_marker: false,
        busy: false,
        candidates_open: false,
        has_text,
    }
}

/// 判定結果から効果を求める。`Submit` は click と `preventDefault`、`Complete` は `preventDefault` だけを返す。
/// それ以外はすべて何もしない。
///
/// `match` は網羅的に書く。`KeyAction` に行が増えたとき、ここでコンパイルが失敗して気づけるようにするため。
pub fn effect_for(decision: Decision) -> KeydownEffect {
    match decision {
        Decision::Defined(KeyDecision {
            action,
            prevent_default,
        }) => match action {
            KeyAction::Submit => KeydownEffect {
                prevent_default,
                click_submit: true,
            },
            // JUDGE-13: Shift+Enter は送信せず、既定の動作（欄への改行）を妨げない。
            KeyAction::Newline => KeydownEffect::NONE,
            // JUDGE-19: Tab は候補が開いていて文字があるときだけ補完。判定が `preventDefault` を求める。
            KeyAction::Complete => KeydownEffect::prevent_only(prevent_default),
            KeyAction::CloseCandidates
            | KeyAction::Stop
            | KeyAction::CancelInput
            | KeyAction::DelegateToIme
            | KeyAction::Nothing => KeydownEffect::NONE,
        },
        Decision::Pending(_) => KeydownEffect::NONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fandhe_workshop_contract::key_decide::{decide, KeyInput, PendingRow, PENDING_ROWS};

    fn key(k: &str) -> KeyInput {
        KeyInput {
            key: k.to_owned(),
            key_code: 0,
            is_composing: false,
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
        }
    }

    fn effect(input: &KeyInput, has_text: bool) -> KeydownEffect {
        effect_for(decide(input, &field_state(has_text)))
    }

    const SUBMIT: KeydownEffect = KeydownEffect {
        prevent_default: true,
        click_submit: true,
    };

    #[test]
    fn 修飾なしの_enter_は送信と_prevent_default() {
        assert_eq!(effect(&key("Enter"), true), SUBMIT);
        // 空でも click に任せる（空の扱いは送信ボタンと同じ経路が決める）。
        assert_eq!(effect(&key("Enter"), false), SUBMIT);
    }

    #[test]
    fn shift_enter_は何もしない() {
        let mut k = key("Enter");
        k.shift = true;
        let newline = Decision::Defined(KeyDecision {
            action: KeyAction::Newline,
            prevent_default: false,
        });
        for has_text in [true, false] {
            assert_eq!(decide(&k, &field_state(has_text)), newline);
            assert_eq!(effect(&k, has_text), KeydownEffect::NONE);
        }
    }

    #[test]
    fn 修飾キー付きの_enter_は何もしない() {
        for m in 0..3 {
            let mut k = key("Enter");
            match m {
                0 => k.ctrl = true,
                1 => k.alt = true,
                _ => k.meta = true,
            }
            assert_eq!(effect(&k, true), KeydownEffect::NONE, "{m}");
        }
    }

    #[test]
    fn 変換中の_enter_は何もしない() {
        let mut a = key("Enter");
        a.is_composing = true;
        let mut b = key("Enter");
        b.key_code = 229;
        let c = key("Process");
        for k in [a, b, c] {
            assert_eq!(effect(&k, true), KeydownEffect::NONE, "{k:?}");
        }
        // 自前の印だけが立っている場合（TASK-276 が結線した後の形）。
        let mut state = field_state(true);
        state.composing_marker = true;
        assert_eq!(
            effect_for(decide(&key("Enter"), &state)),
            KeydownEffect::NONE
        );
    }

    #[test]
    fn enter_以外のキーは何もしない() {
        for k in ["Tab", "Escape", "a", "Unidentified", ""] {
            assert_eq!(effect(&key(k), true), KeydownEffect::NONE, "{k:?}");
        }
    }

    #[test]
    fn 送信になるのは_submit_だけで_prevent_default_は_submit_と_complete() {
        let actions = [
            KeyAction::Submit,
            KeyAction::Newline,
            KeyAction::Complete,
            KeyAction::CloseCandidates,
            KeyAction::Stop,
            KeyAction::CancelInput,
            KeyAction::DelegateToIme,
            KeyAction::Nothing,
        ];
        for action in actions {
            let e = effect_for(Decision::Defined(KeyDecision {
                action,
                prevent_default: true,
            }));
            assert_eq!(e.click_submit, action == KeyAction::Submit, "{action:?}");
            assert_eq!(
                e.prevent_default,
                matches!(action, KeyAction::Submit | KeyAction::Complete),
                "{action:?}"
            );
        }
        for row in PENDING_ROWS {
            assert_eq!(effect_for(Decision::Pending(row)), KeydownEffect::NONE);
        }
        assert_eq!(
            effect_for(Decision::Pending(PendingRow::ModifiedEnter)),
            KeydownEffect::NONE
        );
    }

    #[test]
    fn 状態の既定は処理中と候補と印が偽() {
        let s = field_state(true);
        assert!(s.has_text && !s.busy && !s.candidates_open && !s.composing_marker);
        assert!(!field_state(false).has_text);
    }
}
