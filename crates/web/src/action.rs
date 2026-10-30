//! Runtime が Component に届ける action の名前と宛先の形の契約（TASK-343 2/3）。
//!
//! 呼び出し元は、タイマーと補完を結線する glue（TASK-349。root の外の Esc は `glue::wire_outside_esc` が結ぶ。TASK-353）と、ホストの
//! `Component::decode_action`（[`RuntimeAction::decode`] に委ねる）である。native の rlib で試験できるよう、
//! wasm 専用にしない。
//!
//! 出所は spec の FEND-4（進行は `dispatch_action("tick", 宛先)`、補完は `dispatch_action("complete", "")`）、
//! FEND-5（root の中の Esc は `main` の keydown 属性で `esc` として届く）、FEND-8（root の外の Esc は glue の
//! `document` の keydown が受け持つ）、EVT-6・EVT-8・EVT-9（停止・世代・宛先の規則はこの口でも同じに成り立つ）である。
//!
//! 呼び出しの規則:
//! - `Runtime::dispatch_action` は `set_timeout` のコールバックの中（別のスタック）から呼ぶ。
//! - 配線のコールバックの中・`Component::update` の中・`Runtime::component()` の `Ref` を持っている間・
//!   `rerender` の最中に呼ぶと `Reentrant` で捨てられる。保留もキューイングもされない。
//! - 呼び出し側は `Reentrant` を受けても再送しない。宛先つきの進行は次のタイマーが新しい宛先で届け直し、
//!   古い宛先の再送は EVT-8 で「無視」になるだけだからである（[`DispatchEffect::Dropped`]）。
//! - 進行の宛先を DOM の `data-payload` に書いて click する経路と、隠し要素 `tick-trigger`・`esc-trigger`・
//!   `complete-trigger` は置かない（FEND-4）。`tick`・`complete` は `dispatch_action` からだけ届く。
//!
//! 契約の版は `parts::PARTS_CONTRACT_VERSION`（版 3）。変える場合は先に spec を直して版を上げる。
//!
//! 意図して固定しないもの（持ち主が別にあるため）: 宛先の照合（TASK-210）、タイマーの間隔、glue の結線
//! （TASK-349 で結線済み）、キー処理の受け持ち（FEND-7・FEND-8）。

use fandhe_frontend_wasm_full::DispatchOutcome;
#[cfg(target_arch = "wasm32")]
use fandhe_workshop_contract::storage::StorageError;
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};

/// 進行の action の名前。
pub const ACTION_TICK: &str = "tick";
/// 補完の action の名前。
pub const ACTION_COMPLETE: &str = "complete";
/// Esc の action の名前。
pub const ACTION_ESC: &str = "esc";

/// `esc` の payload。`main` に置く keydown 属性の `data-keys` の値でもある。
///
/// keydown 属性は `data-payload` を付けないと押したキーの正規トークンを payload にする。root の外の glue も
/// [`RuntimeAction::Esc`] の [`encode`](RuntimeAction::encode) で同じ値を渡し、経路ごとに payload が
/// 分かれないよう 1 つにそろえる。
pub const ESC_KEY_TOKEN: &str = "Escape";

/// ホストが `main` と `workshop-root` の属性列に足す、root の中の Esc の keydown 属性（FEND-5）。
///
/// `workshop-root` にも付けるのは、`main` の外の部品（`workshop-parts`）の Esc も上流の属性で届けるため。入れ子でも、
/// 上流は `closest` で最も近い 1 つにだけ届けるので二重にならない（TASK-353 2/2）。
///
/// `data-keydown-prevent-default`・`data-keydown-ignore-repeat` は付けない（FEND-5・FEND-18 の現状）。
pub const ESC_KEYDOWN_ATTRS: [(&str, &str); 2] = [
    ("data-action-keydown", ACTION_ESC),
    ("data-keys", ESC_KEY_TOKEN),
];

/// `tick` の payload の世代の最大桁数（`u64::MAX` は 20 桁）。
const MAX_GENERATION_DIGITS: usize = 20;
/// `tick` の payload の番号の最大桁数（`u32::MAX` は 10 桁）。
const MAX_INDEX_DIGITS: usize = 10;

/// action の名前。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionName {
    /// 進行。
    Tick,
    /// 補完。
    Complete,
    /// Esc。
    Esc,
}

impl ActionName {
    /// 全種類。
    pub const ALL: [ActionName; 3] = [ActionName::Tick, ActionName::Complete, ActionName::Esc];

    /// `dispatch_action` に渡す名前。
    pub const fn as_str(self) -> &'static str {
        match self {
            ActionName::Tick => ACTION_TICK,
            ActionName::Complete => ACTION_COMPLETE,
            ActionName::Esc => ACTION_ESC,
        }
    }

    /// 名前から種類を求める。未知の名前は `None`。
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.as_str() == name)
    }
}

/// Runtime がホストの Component に届ける action。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeAction {
    /// 進行。宛先は世代・Step の実体 ID・何番目の進行か（EVT-8）。
    Tick(TickAddr),
    /// 補完。
    Complete,
    /// Esc。
    Esc,
}

impl RuntimeAction {
    /// ホストの `Component::decode_action` から呼ぶ復号。
    ///
    /// payload は信頼できない入力として扱い、正規形だけを受理する。未知の名前・不正な payload は `None` で、
    /// 上流で `UnknownAction` になり状態も DOM も変えない。panic しない。
    ///
    /// payload の書式:
    /// - `tick`: `<世代> <番号> <Step の実体 ID>`（半角スペース 1 つで区切る 3 項目）。世代は `u64`、番号は
    ///   `u32` の 10 進で、ASCII 数字だけ・先頭の 0 なし（`0` 自体は可）・符号なし。実体 ID は空白を含まないため
    ///   区切りが曖昧にならず、[`StepUid::parse`] で検査する（長さの上限もそこで効く）。
    ///   JSON を使わないのは、配信用の WASM に serde_json を引き込まないため（FEND-16 の大きさの基準）。
    /// - `complete`: 空文字列だけ。
    /// - `esc`: [`ESC_KEY_TOKEN`] だけ。
    pub fn decode(name: &str, payload: &str) -> Option<RuntimeAction> {
        match ActionName::from_name(name)? {
            ActionName::Tick => decode_tick(payload).map(RuntimeAction::Tick),
            ActionName::Complete => payload.is_empty().then_some(RuntimeAction::Complete),
            ActionName::Esc => (payload == ESC_KEY_TOKEN).then_some(RuntimeAction::Esc),
        }
    }

    /// `dispatch_action` に渡す（名前, payload）の組。符号化の正はここだけに置く。
    pub fn encode(&self) -> (&'static str, String) {
        match self {
            RuntimeAction::Tick(a) => (
                ACTION_TICK,
                format!("{} {} {}", a.generation.0, a.index, a.step.0),
            ),
            RuntimeAction::Complete => (ACTION_COMPLETE, String::new()),
            RuntimeAction::Esc => (ACTION_ESC, ESC_KEY_TOKEN.to_owned()),
        }
    }
}

/// `schedule_action` が届けるもの。版つきの契約の action と、遅延保存の結果（ホストの私的な action。SAVE-10）。
#[cfg(target_arch = "wasm32")]
pub(crate) enum Scheduled {
    /// 進行・補完・Esc。
    Runtime(RuntimeAction),
    /// 遅延保存の結果（書き込んだ時点の保存世代つき）。
    SaveResult(u64, Result<(), StorageError>),
    /// 遅延保存の直前の検出（検出した時点の保存世代と、見つけた形だけ。値は持たない。SAFE-27）。
    SecretHold(u64, Vec<fandhe_workshop_core::secret_scan::SecretShape>),
    /// 「この入力を残さない」の保存先の取り直しの完了（`true` は取り直せた。SAFE-28）。
    SecretDiscardRead(bool, u64),
    /// 保存済みの履歴の削除の完了（頼んだときの世代つき。成功は消した件数。SAVE-32）。
    DeleteResult(u64, Result<usize, StorageError>),
    /// 枠の操作（読み込み。TASK-531 1/2）。符号化の正は `ShellAction::encode`。
    Shell(crate::shell::ShellAction),
}

#[cfg(target_arch = "wasm32")]
impl From<RuntimeAction> for Scheduled {
    fn from(a: RuntimeAction) -> Self {
        Scheduled::Runtime(a)
    }
}

#[cfg(target_arch = "wasm32")]
impl Scheduled {
    pub(crate) fn encode(&self) -> (&'static str, String) {
        match self {
            Scheduled::Runtime(a) => a.encode(),
            Scheduled::Shell(a) => a.encode(),
            Scheduled::SecretHold(epoch, shapes) => (
                crate::secret_notice::ACTION_SECRET_HOLD,
                crate::secret_notice::encode_secret_hold(*epoch, shapes),
            ),
            Scheduled::SecretDiscardRead(ok, generation) => (
                crate::secret_notice::ACTION_SECRET_DISCARD_READ,
                format!("{} {generation}", if *ok { "ok" } else { "fail" }),
            ),
            Scheduled::DeleteResult(generation, r) => (
                crate::delete_notice::ACTION_DELETE_RESULT,
                crate::delete_notice::encode_delete_result(*generation, *r),
            ),
            Scheduled::SaveResult(e, r) => (
                crate::save_notice::ACTION_SAVE_RESULT,
                crate::save_notice::encode_save_result(*e, *r),
            ),
        }
    }
}

/// 10 進の正規形（ASCII 数字だけ・先頭 0 なし・桁数の上限内）なら真。
fn is_canonical_decimal(s: &str, max_digits: usize) -> bool {
    !s.is_empty()
        && s.len() <= max_digits
        && s.bytes().all(|b| b.is_ascii_digit())
        && (s == "0" || !s.starts_with('0'))
}

fn decode_tick(payload: &str) -> Option<TickAddr> {
    let mut parts = payload.splitn(3, ' ');
    let generation = parts.next()?;
    let index = parts.next()?;
    let step = parts.next()?;
    if !is_canonical_decimal(generation, MAX_GENERATION_DIGITS)
        || !is_canonical_decimal(index, MAX_INDEX_DIGITS)
    {
        return None;
    }
    // 桁数を先に見たうえで checked な解析をする（あふれは `None`）。
    let generation = generation.parse::<u64>().ok()?;
    let index = index.parse::<u32>().ok()?;
    // 実体 ID は空白を含められないため、余りの項目（4 項目以上）もここで拒否される。
    let step = StepUid::parse(step).ok()?;
    Some(TickAddr {
        generation: Generation(generation),
        step,
        index,
    })
}

/// `dispatch_action` の結果を呼び出し側の対処で読み替えたもの。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchEffect {
    /// 受理され `update` が呼ばれた。
    Applied,
    /// 未知の名前または不正な payload で、状態も DOM も変わらなかった。
    Rejected,
    /// 再入で捨てられた。保留もキューイングもされず、再送もしない。
    Dropped,
}

/// [`DispatchOutcome`] を [`DispatchEffect`] に読み替える。再送やキューを作る関数は置かない。
pub fn dispatch_effect(outcome: DispatchOutcome) -> DispatchEffect {
    match outcome {
        DispatchOutcome::Dispatched => DispatchEffect::Applied,
        DispatchOutcome::UnknownAction => DispatchEffect::Rejected,
        DispatchOutcome::Reentrant => DispatchEffect::Dropped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 数値の正規形() {
        assert!(is_canonical_decimal("0", 10));
        assert!(is_canonical_decimal("10", 10));
        assert!(!is_canonical_decimal("01", 10));
        assert!(!is_canonical_decimal("+1", 10));
        assert!(!is_canonical_decimal("", 10));
        assert!(!is_canonical_decimal("12345678901", 10));
        assert!(!is_canonical_decimal("１", 10));
    }

    #[test]
    fn 世代と番号のあふれは拒否される() {
        assert_eq!(
            RuntimeAction::decode("tick", "18446744073709551616 0 main/a"),
            None
        );
        assert_eq!(RuntimeAction::decode("tick", "1 4294967296 main/a"), None);
    }
}
