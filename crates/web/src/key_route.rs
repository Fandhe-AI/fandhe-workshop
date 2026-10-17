//! keydown の判定結果を、glue が行う効果へ変える純関数（TASK-275。JUDGE-12・JUDGE-13・FEND-14・D-42）。
//!
//! 呼び出し元は `glue` の keydown リスナー（`effect_for`）と、Component の `esc` action の処理
//! （`esc_route`。TASK-347 が結ぶ）である。判定表 `decide`（TASK-257）の結果のうち、
//! 修飾なしの Enter（`Submit`）を「送信ボタンへの click」に結び、Tab の補完（`Complete`。JUDGE-19・FEND-21）を
//! `preventDefault` に結ぶ。click は上流の Runtime の配線を通り、
//! 送信ボタンを押した場合と同じ `update` に届くため、入力も記録も同じになる。ボタンが無効なら何も起きない。
//!
//! この層は DOM に触れない。wasm 専用にしないのは、native の試験で行ごとの効果を固定するためである。
//!
//! 受け持たない行（glue は何もしない）と担当:
//! - `Newline`: textarea の既定動作で改行する。`preventDefault` しない（JUDGE-13。`tools/browser/enter-submit.test.mjs` が確かめる）
//! - `DelegateToIme`: IME に任せる。自前の変換中の印は TASK-276
//! - `Pending(_)`: 版 2 の `decide` は返さない（修飾キー付き Enter は `Nothing`。TASK-281）。版 1 の表を読むために残る変種で、返ったら送信も `preventDefault` もしない
//! - `CloseCandidates`・`CancelInput`: `effect_for` では何もしない。Esc の行き先は `esc_route` が求める（候補を閉じる・取り消しの入力。TASK-277 2/2）
//! - `Stop`: `effect_for` では何もしない。root の中の Esc は上流の `esc` 属性で Component に届くため、glue で重ねて
//!   届けない（FEND-5・FEND-6）。停止への結び付けは `esc_route` が受け持つ（TASK-277 1/2）
//! - `Nothing`: 何もしない。修飾キー（Ctrl・Alt・Meta。Shift との組み合わせを含む）付きの Enter もここに当たり、
//!   送信も `preventDefault` もしない（JUDGE-22・D-60。TASK-282 2/2）。OS による分岐は持たない（D-48）。
//!   Shift+Tab もここに当たり、`preventDefault` せず逆向きのフォーカス移動に任せる
//!   （JUDGE-19。TASK-278 2/3。ブラウザ試験は `tools/browser/shift-tab.test.mjs`）
//!   入力欄の Space（`KeyboardEvent.key` は `" "`）もここに当たる。`preventDefault` せず、textarea の既定動作で空白が入る
//!   （FEND-7・D-42。TASK-352 1/3。ブラウザ試験は `tools/browser/field-space.test.mjs`）
//!
//! 標準でない要素（左の行）の Enter・Space は `row_key_effect` が受け持つ（TASK-352 2/3。A11Y-18・D-42）。結線は `glue::wire_row_keys` である（TASK-358 1/2）。
//!
//! 候補が開いているときの Esc の 2 回と補完の選び方は、`after_esc`・`completion_choice` が受け持つ
//! （TASK-282 1/2。JUDGE-18・JUDGE-21・D-60）。矢印キーで候補を選ぶ操作は無い（複数行の下書きでのキャレット移動と衝突するため）。
//!
//! Tab の補完は、候補の一覧が開いていて入力欄に文字があるときだけ `preventDefault` を呼ぶ（TASK-278 1/3）。
//! それ以外の Tab は呼ばず、ブラウザの通常のフォーカス移動に任せる。補完の挿入そのものは `HelloView::complete` が持つ
//! （TASK-440 2/2）。ここでは行わない。

use fandhe_workshop_contract::key_decide::{
    first_completion, Decision, FieldState, KeyAction, KeyDecision, KeyInput,
};
use fandhe_workshop_core::input::{Control, Input, Op};

/// glue が keydown に対して行う効果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeydownEffect {
    /// `preventDefault` を呼ぶか。
    pub prevent_default: bool,
    /// 送信ボタンへ click を送るか。
    pub click_submit: bool,
    /// 補完の action を Runtime の口で届けるか（FEND-4。TASK-349）。
    pub deliver_complete: bool,
}

impl KeydownEffect {
    const NONE: Self = Self {
        prevent_default: false,
        click_submit: false,
        deliver_complete: false,
    };

    /// click を伴わず、`preventDefault` と補完の action の配送を行う効果（Tab の補完）。
    const fn complete(prevent_default: bool) -> Self {
        Self {
            prevent_default,
            click_submit: false,
            deliver_complete: true,
        }
    }
}

/// 判定表へ渡す入力欄の状態。`has_text` は App の下書きから取る（DOM の値は読まない）。
///
/// 他の値は、対応する状態がまだ存在しないため偽にする。状態が加わるタスクが、この関数の 1 か所を差し替える。
/// - `composing_marker`: 自前の変換中の印は TASK-276 の担当。印が無くても、`isComposing`・keyCode 229・
///   `Process` のいずれかで判定表が変換中と見なすため、変換中の Enter は送信されない
/// - `busy`: Esc の経路ではホストが Session から取って差し替える（TASK-436 2/2）。glue の Enter・Tab の経路では偽のままでよい
///   （処理中の送信は core が EVT-7 の busy で拒否する）
/// - `candidates_open`: 呼び出し元が一覧の開閉の状態（`HelloView::candidates_open`）から渡す。
///   Tab の経路（glue）と、補完の挿入（`HelloView::complete`）と、Esc の経路（`hello`。`after_esc` で状態を進める）が
///   同じ値を使う（TASK-440）
pub fn field_state(has_text: bool, candidates_open: bool) -> FieldState {
    FieldState {
        composing_marker: false,
        busy: false,
        candidates_open,
        has_text,
    }
}

/// Tab の判定の入力（修飾なし・変換中でない）。
///
/// 補完の配送の時点で判定をやり直すための入力である（`HelloView::complete`）。glue は keydown の時点で
/// 実イベントから判定しており、変換中・Shift 付きの Tab は配送まで届かない（JUDGE-19）。
pub fn tab_input() -> KeyInput {
    KeyInput {
        key: "Tab".to_owned(),
        key_code: 0,
        is_composing: false,
        shift: false,
        ctrl: false,
        alt: false,
        meta: false,
    }
}

/// 標準でない要素（`data-action` を持つ左の行など）の Enter・Space に対する効果（TASK-352 2/3。A11Y-18・FEND-7・FEND-13）。
///
/// 入力欄の `KeydownEffect` とは別の型にする。行は click を送る先が送信ボタンではなく行そのものであり、
/// 契約 `key_decide` の版（`KEY_DECIDE_CONTRACT_VERSION`）を上げずに表を保つためである。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowKeyEffect {
    /// `preventDefault` を呼ぶか。Space のページのスクロールと Enter の既定動作を止める。
    pub prevent_default: bool,
    /// 行へ click を送るか。押しっぱなしの自動リピートでは送らない。
    pub click_row: bool,
}

impl RowKeyEffect {
    const NONE: Self = Self {
        prevent_default: false,
        click_row: false,
    };
}

/// 標準でない要素の Enter・Space から、`preventDefault` と click の要否を求める純関数。
///
/// 呼び出し元は `glue::wire_row_keys`（TASK-358 1/2）である。
/// glue は属性を読む口（`closest`・`get_attribute`）を持たないため、`pointable` は keydown の target が `li` かどうかで
/// 決める。宛先を受け付けるかの判断は下流が持つ。
/// D-42 により受け持ちは glue に残り、属性化も上流の口も採らない。
///
/// 判定は次の順に評価する。
/// 1. 宛先の無い行（`pointable` が偽）は表示だけで、何もしない。Space の既定のスクロールも妨げない
/// 2. 変換中は何もしない
/// 3. Ctrl・Alt・Meta・Shift 付きは何もしない。A11Y-18 は修飾キー付きの扱いを定めていないため保守側に倒した。
///    食い違いが見つかったら spec を先に直す
/// 4. `key` が `"Enter"` か `" "`（Space）なら `preventDefault` し、`repeat` でなければ click する
/// 5. それ以外のキーは何もしない。`key` は未信頼の値なので完全一致だけで扱う
pub fn row_key_effect(input: &KeyInput, repeat: bool, pointable: bool) -> RowKeyEffect {
    if !pointable || input.in_composition(false) {
        return RowKeyEffect::NONE;
    }
    if input.ctrl || input.alt || input.meta || input.shift {
        return RowKeyEffect::NONE;
    }
    match input.key.as_str() {
        "Enter" | " " => RowKeyEffect {
            prevent_default: true,
            click_row: !repeat,
        },
        _ => RowKeyEffect::NONE,
    }
}

/// 判定結果から効果を求める。`Submit` は click と `preventDefault`、`Complete` は `preventDefault` と補完の配送を返す。
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
                deliver_complete: false,
            },
            // JUDGE-13: Shift+Enter は送信せず、既定の動作（欄への改行）を妨げない。
            KeyAction::Newline => KeydownEffect::NONE,
            // JUDGE-19: Tab は候補が開いていて文字があるときだけ補完。判定が `preventDefault` を求める。
            KeyAction::Complete => KeydownEffect::complete(prevent_default),
            // TASK-277: 停止は上流の `esc` 属性で Component に届き、`esc_route` が結ぶ。glue では重ねて届けない。
            KeyAction::Stop => KeydownEffect::NONE,
            KeyAction::CloseCandidates
            | KeyAction::CancelInput
            | KeyAction::DelegateToIme
            | KeyAction::Nothing => KeydownEffect::NONE,
        },
        Decision::Pending(_) => KeydownEffect::NONE,
    }
}

/// Component が `esc` action を受けたときの行き先。
///
/// `KeydownEffect` には載せない。root の中の Esc に glue は作用しない（FEND-5・FEND-6）ため、
/// glue の効果の型と混ぜると二重配送を誘うからである。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscRoute {
    /// Session へ渡す操作。停止なら `Op::Control(Control::Stop)`。停止の意味は core（EVT-9）が持つ。
    pub op: Option<Op>,
    /// 開いている候補の一覧を閉じるか。
    pub close_candidates: bool,
}

impl EscRoute {
    /// 何もしない行き先。
    pub const NONE: Self = Self {
        op: None,
        close_candidates: false,
    };

    /// 停止の行き先。core の停止（`Control::Stop`）に結び、候補も閉じる。
    fn stop() -> Self {
        Self {
            op: Some(Op::Control(Control::Stop)),
            close_candidates: true,
        }
    }

    /// 候補の一覧を閉じるだけの行き先。エンジンへは何も渡さないため、ログも拒否も生まない。
    fn close_candidates() -> Self {
        Self {
            op: None,
            close_candidates: true,
        }
    }

    /// 取り消しの入力の行き先。規則が無い Step での拒否と記録は core（EVT-10・TASK-212）が持つ。
    fn cancel_input() -> Self {
        Self {
            op: Some(Op::Input(Input::Cancel)),
            close_candidates: false,
        }
    }
}

/// Esc の判定の入力。上流が `isComposing`・229 を除外して届けるため、変換中の信号は持たせない。
pub fn esc_input() -> KeyInput {
    KeyInput {
        key: "Escape".to_owned(),
        key_code: 0,
        is_composing: false,
        shift: false,
        ctrl: false,
        alt: false,
        meta: false,
    }
}

/// 判定結果から Esc の行き先を求める（JUDGE-17）。
/// (1) 処理中は候補が開いていても必ず停止にし、候補も閉じる（TERM-7）。
/// (2) 処理中でなく候補が開いていれば、候補を閉じるだけにする（エンジンへは何も渡さない）。
/// (3) どちらでもなければ取り消しの入力にする（規則が無い Step では core が拒否として記録する）。
/// 優先順位そのものは判定表 `decide` が持ち、ここでは作り直さない。
///
/// 画面から (2) に届くのは候補の一覧が結ばれた TASK-440 1/2 の後、(1) がホストから Session に届くのは TASK-436 2/2 の後である。
/// (3) の Session への適用もホスト（`host::EngineHost`）が行う（TASK-441 2/2）。取り消しの規則が無い Step では拒否の行が出る。
/// `match` は網羅的に書く。`KeyAction` に行が増えたとき、ここでコンパイルが失敗して気づけるようにするため。
pub fn esc_route(decision: Decision) -> EscRoute {
    match decision {
        Decision::Defined(KeyDecision { action, .. }) => match action {
            KeyAction::Stop => EscRoute::stop(),
            KeyAction::CloseCandidates => EscRoute::close_candidates(),
            KeyAction::CancelInput => EscRoute::cancel_input(),
            KeyAction::Submit
            | KeyAction::Newline
            | KeyAction::Complete
            | KeyAction::DelegateToIme
            | KeyAction::Nothing => EscRoute::NONE,
        },
        Decision::Pending(_) => EscRoute::NONE,
    }
}

/// Esc の行き先を適用した後の入力欄の状態を求める（JUDGE-18・D-60）。
///
/// 契約の `decide` は Esc 1 回ごとの判定で、「2 回」は呼び出し側が 1 回目の後に `candidates_open` を偽へ戻して
/// 初めて成り立つ（`key_decide` の版 2 の注記）。本関数がその遷移を受け持つ。呼び出し元は TASK-440 の候補の一覧の部品である。
/// Esc を押した回数は持たない。候補がまた開けば、次の Esc は再び閉じるだけになる（JUDGE-17 (2)）。
/// 処理中（`busy`）は Esc では変えない。停止の結果は core の担当である。DOM には触れない。
pub fn after_esc(state: FieldState, route: &EscRoute) -> FieldState {
    FieldState {
        candidates_open: state.candidates_open && !route.close_candidates,
        ..state
    }
}

/// Tab の補完に使う候補を選ぶ（JUDGE-21・D-60）。判定が `Complete` のときだけ先頭の候補を返し、他は `None`。
///
/// 矢印キーで候補を選ぶ操作は無く（TERM-26 は将来の拡張）、選択の状態も持たない。どの候補かは契約の
/// `first_completion` が決める。下書きへの挿入そのものは `HelloView::complete` が持ち、ここでは行わない。
/// `match` は網羅的に書く。`KeyAction` に行が増えたとき、ここでコンパイルが失敗して気づけるようにするため。
pub fn completion_choice<T>(decision: Decision, candidates: &[T]) -> Option<&T> {
    match decision {
        Decision::Defined(KeyDecision { action, .. }) => match action {
            KeyAction::Complete => first_completion(candidates),
            KeyAction::Submit
            | KeyAction::Newline
            | KeyAction::Stop
            | KeyAction::CloseCandidates
            | KeyAction::CancelInput
            | KeyAction::DelegateToIme
            | KeyAction::Nothing => None,
        },
        Decision::Pending(_) => None,
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
        effect_for(decide(input, &field_state(has_text, false)))
    }

    const SUBMIT: KeydownEffect = KeydownEffect {
        prevent_default: true,
        click_submit: true,
        deliver_complete: false,
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
            assert_eq!(decide(&k, &field_state(has_text, false)), newline);
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
        let mut state = field_state(true, false);
        state.composing_marker = true;
        assert_eq!(
            effect_for(decide(&key("Enter"), &state)),
            KeydownEffect::NONE
        );
    }

    #[test]
    fn enter_以外のキーは何もしない() {
        for k in ["Tab", "Escape", "a", " ", "Unidentified", ""] {
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
                e.deliver_complete,
                action == KeyAction::Complete,
                "{action:?}"
            );
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
    fn 状態は候補の開閉を引数どおりに返し処理中と印は偽() {
        let s = field_state(true, false);
        assert!(s.has_text && !s.busy && !s.candidates_open && !s.composing_marker);
        assert!(!field_state(false, false).has_text);
        let open = field_state(true, true);
        assert!(open.candidates_open && !open.busy && !open.composing_marker);
    }

    #[test]
    fn tab_は候補が開いていて文字があるときだけ補完になる() {
        let complete = Decision::Defined(KeyDecision {
            action: KeyAction::Complete,
            prevent_default: true,
        });
        assert_eq!(decide(&tab_input(), &field_state(true, true)), complete);
        for (t, o) in [(true, false), (false, true), (false, false)] {
            assert!(
                completion_choice(decide(&tab_input(), &field_state(t, o)), &["a"]).is_none(),
                "{t} {o}"
            );
        }
    }

    #[test]
    fn 行の_enter_と_space_は_prevent_default_と_click() {
        for k in ["Enter", " "] {
            let e = row_key_effect(&key(k), false, true);
            assert!(e.prevent_default && e.click_row, "{k:?}");
            let held = row_key_effect(&key(k), true, true);
            assert!(held.prevent_default && !held.click_row, "{k:?}");
        }
    }

    #[test]
    fn 行は宛先が無い_修飾付き_変換中_他のキーなら何もしない() {
        assert_eq!(
            row_key_effect(&key("Enter"), false, false),
            RowKeyEffect::NONE
        );
        for m in 0..4 {
            let mut k = key(" ");
            match m {
                0 => k.ctrl = true,
                1 => k.alt = true,
                2 => k.meta = true,
                _ => k.shift = true,
            }
            assert_eq!(row_key_effect(&k, false, true), RowKeyEffect::NONE, "{m}");
        }
        let mut c = key("Enter");
        c.is_composing = true;
        assert_eq!(row_key_effect(&c, false, true), RowKeyEffect::NONE);
        for k in ["Tab", "Escape", "a", "", "Unidentified"] {
            assert_eq!(
                row_key_effect(&key(k), false, true),
                RowKeyEffect::NONE,
                "{k:?}"
            );
        }
    }

    #[test]
    fn esc_の行き先は判定の行ごとに決まる() {
        let decide_row = |action| {
            esc_route(Decision::Defined(KeyDecision {
                action,
                prevent_default: false,
            }))
        };
        let stop = decide_row(KeyAction::Stop);
        assert_eq!(stop.op, Some(Op::Control(Control::Stop)));
        assert!(stop.close_candidates);
        let close = decide_row(KeyAction::CloseCandidates);
        assert_eq!(close.op, None);
        assert!(close.close_candidates);
        let cancel = decide_row(KeyAction::CancelInput);
        assert_eq!(cancel.op, Some(Op::Input(Input::Cancel)));
        assert!(!cancel.close_candidates);
        for action in [
            KeyAction::Submit,
            KeyAction::Newline,
            KeyAction::Complete,
            KeyAction::DelegateToIme,
            KeyAction::Nothing,
        ] {
            assert_eq!(decide_row(action), EscRoute::NONE, "{action:?}");
        }
        for row in PENDING_ROWS {
            assert_eq!(esc_route(Decision::Pending(row)), EscRoute::NONE);
        }
    }
}
