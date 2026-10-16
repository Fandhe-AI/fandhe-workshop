//! キー判定表 `decide` の入出力の型と、行の判定（TASK-257 版 1、TASK-281 版 2）。
//!
//! 入力欄のキー処理（Enter の送信、Shift+Enter の改行、IME の変換中の除外、Esc の 3 通り、Tab の補完）を、
//! DOM に依存しない純関数の表として 1 か所に固定する。入力・表示の担当（TASK-275〜278）と web の glue は、
//! keydown から [`KeyInput`] と [`FieldState`] を作って [`decide`] を呼び、返った動作に従う。
//! 表を属性・上流・glue のどれが受け持つか（TASK-1）は本契約の対象にしない。
//!
//! 版 2 は、TASK-258 の決定（D-60、2026-10-15）を反映した。版 1 が未決として名指しした 3 行は、次のとおり確定した。
//! - JUDGE-22: Ctrl・Alt・Meta 付きの Enter は、送信にも改行にもしない。[`decide`] は [`KeyAction::Nothing`]・
//!   `preventDefault` なしを返す。`preventDefault` の要否は 04 が明記しないセルで、現行の観察（版 1 の web は何もしない）から取った。
//!   改行の挿入などが観察されたら、spec を先に直す
//! - JUDGE-21: Tab の補完は先頭の候補だけである。矢印キーで選ぶ操作（TERM-26）は無く、将来の拡張とする。
//!   [`KeyAction::Complete`] は番号を持たず、どの候補かは [`first_completion`] が決める
//! - JUDGE-18: 1 回目の Esc は候補を閉じるだけで、2 回目の Esc で取り消す。1 回の keydown のセルは版 1 の JUDGE-17 (2) のままで、
//!   呼び出し側が `candidates_open` を偽に戻した後の Esc が [`KeyAction::CancelInput`] になる
//!
//! 版の履歴は次のとおり。
//! - 版 2（2026-10-16、TASK-281）: 上の 3 行を確定した。[`decide`] の値が変わったのは JUDGE-22 だけ
//!   （[`Decision::Pending`] から [`KeyAction::Nothing`]・`preventDefault` なしへ）。
//!   [`PENDING_ROWS`] は空になり、[`decide`] は [`Decision::Pending`] を返さない。
//!   [`Decision::Pending`]・[`PendingRow`] は、版 1 の期待表（`tests/data/key_decide_1/table.json`）を読むためと、
//!   将来の未決の行の拡張点として残す。[`PendingRow`] の 3 つは「版 1 で未決だった行」である。
//!   版 1 との差分は、期待表の突き合わせの試験（`v2_differs_from_v1_only_in_pending_rows`）で固定する
//! - 版 1（TASK-257）: 確定している行（JUDGE-12・13・14・16・17・19）だけを持ち、3 行を未決として名指しした
//!
//! 入力・表示の担当（TASK-282・TASK-283・TASK-440 など）は、版 2 を前提にする。
//! 04 が明記していないセル（Esc と Shift+Enter の `preventDefault`、変換中の Tab）は PoC-2 の記録から取った。
//! 食い違いが見つかったら spec を先に直す。
//!
//! 対象外は次のとおり。
//! - 左の画面の行の Enter・Space（JUDGE の対象行に無い）
//! - 貼り付けの上限（`limits` の担当）
//! - 貼り付けの切り捨て位置（JUDGE-24）。`beforeinput` で貼り付けた分だけを切る案は、採用の条件の自動ブラウザ試験が未実施で、
//!   検討中のままである。`input_limit` の担当で、[`decide`] は貼り付けの keydown（Ctrl+V・Meta+V）を止めない
//!
//! `key` は DOM 由来の未信頼の値である。既知の綴りとの完全一致だけで扱い、未知の値は「その他のキー」として
//! [`KeyAction::Nothing`] に落とす。全域の純関数で、panic しない。

use serde::{Deserialize, Serialize};

/// この契約の版。値・規則・型を変えたら上げる。公開 API の `CONTRACT_VERSION` とは別に管理する。
pub const KEY_DECIDE_CONTRACT_VERSION: u32 = 2;

/// keydown から作るキーの入力。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyInput {
    /// `KeyboardEvent.key`（`"Enter"`・`"Tab"`・`"Escape"`・`"Process"` など）。
    pub key: String,
    /// `KeyboardEvent.keyCode`。変換中は 229 になる環境がある。
    pub key_code: u32,
    /// `KeyboardEvent.isComposing`。
    pub is_composing: bool,
    /// Shift が押されている。
    pub shift: bool,
    /// Ctrl が押されている。
    pub ctrl: bool,
    /// Alt が押されている。
    pub alt: bool,
    /// Meta が押されている。
    pub meta: bool,
}

impl KeyInput {
    /// 変換中か。`isComposing`・キーコード 229・`key == "Process"`・自前の印のどれか 1 つでも立てば真（JUDGE-14）。
    ///
    /// 1 つの条件に頼らないのは、ブラウザごとに変換確定の keydown の見え方が違うためである。
    pub fn in_composition(&self, marker: bool) -> bool {
        self.is_composing || self.key_code == 229 || self.key == "Process" || marker
    }

    fn modified(&self) -> bool {
        self.ctrl || self.alt || self.meta
    }
}

/// 入力欄の状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldState {
    /// 自前の変換中の印（compositionstart から compositionend まで立てる）。
    pub composing_marker: bool,
    /// 処理中（進行の待機中）。
    pub busy: bool,
    /// 補完候補の一覧が開いている。
    pub candidates_open: bool,
    /// 入力欄に文字がある。
    pub has_text: bool,
}

/// 動作の 8 種。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyAction {
    /// 送信する。送信ボタンの click と同じ経路に任せる（中身が空かどうかの扱いも同じ。JUDGE-12）。
    Submit,
    /// 改行する。textarea の既定動作に任せる（JUDGE-13）。
    Newline,
    /// 先頭の候補で補完する（JUDGE-21・D-60）。どの候補かは [`first_completion`] が決め、この型は番号を持たない。
    Complete,
    /// 候補の一覧を閉じる（JUDGE-17 (2)）。
    CloseCandidates,
    /// 処理を停止する。候補も閉じる（JUDGE-17 (1)）。
    Stop,
    /// 入力の取り消し（JUDGE-17 (3)）。
    CancelInput,
    /// IME に任せる。エンジンには何も渡さない（JUDGE-14・16）。
    DelegateToIme,
    /// 何もしない。ブラウザの通常の動作に任せる。
    Nothing,
}

/// 確定した出力。動作と `preventDefault` の要否の組。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyDecision {
    /// 行う動作。
    pub action: KeyAction,
    /// `preventDefault` を呼ぶか。
    pub prevent_default: bool,
}

/// 版 1 で動作を定めていなかった行。版 2 ですべて確定し、[`decide`] は [`Decision::Pending`] を返さない。
///
/// 版 1 の期待表を読むためと、将来の未決の行の拡張点として残す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingRow {
    /// 修飾キー付き Enter（JUDGE-22）。
    ModifiedEnter,
    /// 補完の選び方（JUDGE-21）。
    CompletionChoice,
    /// 候補が開いているときの Esc の回数（JUDGE-18）。
    EscCountWithCandidates,
}

impl PendingRow {
    /// 仕様の判定の ID。
    pub fn judge_id(self) -> &'static str {
        match self {
            PendingRow::ModifiedEnter => "JUDGE-22",
            PendingRow::CompletionChoice => "JUDGE-21",
            PendingRow::EscCountWithCandidates => "JUDGE-18",
        }
    }
}

/// 未決の行の一覧。版 2 では空である（3 行は D-60 で確定した）。将来、未決の行ができたらここへ足し、版を上げる。
pub const PENDING_ROWS: [PendingRow; 0] = [];

/// [`decide`] の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// 確定している行。
    Defined(KeyDecision),
    /// 未決の行。版 2 の [`decide`] は返さない。返ったら、呼び出し側は何もせず、既定の動作にも倒さない。
    Pending(PendingRow),
}

fn defined(action: KeyAction, prevent_default: bool) -> Decision {
    Decision::Defined(KeyDecision {
        action,
        prevent_default,
    })
}

/// キー判定表。上の行から順に評価し、最初に当てはまった行を採る。
///
/// 1. 変換中で Enter・Tab・Escape・`Process` なら IME に任せる（JUDGE-14・16）
/// 2. Escape: 処理中なら停止、候補が開いていれば閉じる、それ以外は入力の取り消し（JUDGE-17）。
///    1 回目で候補を閉じ、呼び出し側が `candidates_open` を偽にした後の Esc が取り消しになる（JUDGE-18）
/// 3. Enter: Ctrl・Alt・Meta 付きは何もしない（JUDGE-22。Shift の有無に関わらない）、Shift 付きは改行、修飾なしは送信（JUDGE-12・13）
/// 4. Tab: 修飾なしで候補が開いていて文字があるときだけ補完。それ以外は何もしない（JUDGE-19）
/// 5. その他のキーは何もしない
pub fn decide(input: &KeyInput, state: &FieldState) -> Decision {
    let key = input.key.as_str();
    let composing = input.in_composition(state.composing_marker);
    if composing && matches!(key, "Enter" | "Tab" | "Escape" | "Process") {
        return defined(KeyAction::DelegateToIme, false);
    }
    match key {
        "Escape" => {
            if state.busy {
                defined(KeyAction::Stop, false)
            } else if state.candidates_open {
                defined(KeyAction::CloseCandidates, false)
            } else {
                defined(KeyAction::CancelInput, false)
            }
        }
        "Enter" => {
            if input.modified() {
                // JUDGE-22（D-60）: 送信にも改行にもしない。
                defined(KeyAction::Nothing, false)
            } else if input.shift {
                defined(KeyAction::Newline, false)
            } else {
                defined(KeyAction::Submit, true)
            }
        }
        "Tab" => {
            if !input.shift && !input.modified() && state.candidates_open && state.has_text {
                defined(KeyAction::Complete, true)
            } else {
                defined(KeyAction::Nothing, false)
            }
        }
        _ => defined(KeyAction::Nothing, false),
    }
}

/// 補完に使う候補（JUDGE-21・D-60）。先頭の候補だけを使い、空なら `None`。
///
/// 矢印キーで選ぶ操作は無いため、選択の状態を [`FieldState`] に持たない。呼び出し側は、
/// [`KeyAction::Complete`] を受けたら、候補の一覧をこの関数に渡して補完する候補を得る。
pub fn first_completion<T>(candidates: &[T]) -> Option<&T> {
    candidates.first()
}
