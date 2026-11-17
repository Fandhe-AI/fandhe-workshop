//! 右の区画 `shell-fast-forward` に置く、早送りの入・切の切り替えと状態の文字（SHELL-24。TASK-540 1/2）の定数・提示・描画。
//!
//! 呼び出し元は `host::EngineHost` だけで、`view` が [`fast_forward_section`] を停止ボタン（入力欄の `main`）の直後に置き、
//! `update` が前後の提示を比べて [`FAST_FORWARD_FIELD`] を dirty に積む。入・切はホストの表示の状態で、`ops`・履歴・保存先には
//! 書かない（SHELL-27・D-54。`contract::history::OPS_RECORDS_FAST_FORWARD`）。入の間の連続適用は core の
//! `RecordedSession::fast_forward` が持ち、ここは文字とボタンを描くだけである。
//! 状態は「早送り: 入」「早送り: 切」の文字で出す（色だけで示さない）。`aria-pressed` は a11y の閉じた一覧に無いので使わない。
//! 欄 `normal_speed` を持つ Step（SHELL-26）ではボタンを出さず、文言 [`PINNED_LABEL`] を出して入・切を動かさない。
//! 初期値（SHELL-25・SESS-3）は扱わない。

use fandhe_frontend_core::{el, text, Node};

use crate::artifact_view::ACTION_ATTR;
use crate::list_policy::keyed_view_or_notice;
use crate::parts::{FieldKind, PartField};
use crate::shell::ControlSection;

/// 切り替えの action の名前。payload は空だけを受ける。
pub const ACTION_TOGGLE_FAST_FORWARD: &str = "toggle-fast-forward";

/// 1 回の連続適用で適用する進行の上限（EVT-17 の `limit`）。区切りの間に停止を受ける。
///
/// 4 は提案値で実測していない（2026-11-16）。20 件の進行が 5 区切りになり、5 件の進行が 4 + 1 の 2 区切りになる。
pub const FAST_FORWARD_LIMIT: u32 = 4;

/// 状態の文字の `data-testid`（常に 1 個）。
pub const STATUS_TESTID: &str = "fast-forward-status";

/// 切り替えのボタンの `data-testid`（0 か 1 個。表示だけの間は出さない）。
pub const TOGGLE_TESTID: &str = "fast-forward-toggle";

/// 「早送り中」の `data-testid`（入かつ処理中だけ 1 個）。
pub const ACTIVE_TESTID: &str = "fast-forward-active";

/// 状態の文字に載せる、入・切の属性名（値は `on` か `off`）。
pub const STATE_ATTR: &str = "data-fast-forward";

/// 入のときの状態の文字。
pub const ON_LABEL: &str = "早送り: 入";

/// 切のときの状態の文字。
pub const OFF_LABEL: &str = "早送り: 切";

/// 切り替えのボタンの文言。
pub const TOGGLE_LABEL: &str = "早送りを切り替える";

/// 入の間の処理中に出す文字。
pub const ACTIVE_LABEL: &str = "早送り中";

/// 通常の速さに固定した Step で出す文言の `data-testid`（固定の間だけ 1 個）。
pub const PINNED_TESTID: &str = "fast-forward-pinned";

/// 通常の速さに固定した Step で出す文言。
pub const PINNED_LABEL: &str = "この場面は通常の速さです";

/// 入れ物の CSS クラス。
pub const FAST_FORWARD_CLASS: &str = "sh-fast-forward";

/// 早送りの区画の keyed list の field。提示が変わると `host::EngineHost` がこの名前を dirty に積む。
pub const FAST_FORWARD_FIELD: PartField = PartField {
    name: "fast_forward",
    kind: FieldKind::KeyedList,
};

/// 今提示する早送りの区画。前後で比べて dirty を積むための値。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FastForwardOffer {
    /// 入か。通常の速さに固定した Step の間も入のまま（固定は切り替えを捨てるだけで、入・切の値は変えない）。
    pub on: bool,
    /// ボタンを出すか（表示だけの間は出さない）。
    pub can_toggle: bool,
    /// 通常の速さに固定した Step か（SHELL-26。文言を出す）。
    pub pinned: bool,
    /// 「早送り中」を出すか（入かつ処理中）。
    pub active: bool,
}

/// `shell-fast-forward` の入れ物。入れ物は常に描き、中の keyed list の項目の出し入れだけで表す（FEND-3）。
/// ボタンの項目は鍵を固定し、押下のたびに要素を作り直さずフォーカスを保つ。
pub fn fast_forward_section(offer: &FastForwardOffer) -> Node {
    let (state, label) = if offer.on {
        ("on", ON_LABEL)
    } else {
        ("off", OFF_LABEL)
    };
    let mut items: Vec<(String, Node)> = vec![(
        format!("status:{state}"),
        el(
            "span",
            vec![("data-testid", STATUS_TESTID), (STATE_ATTR, state)],
            vec![text(label)],
        ),
    )];
    if offer.can_toggle {
        items.push((
            "toggle".to_owned(),
            el(
                "button",
                vec![
                    ("type", "button"),
                    ("data-testid", TOGGLE_TESTID),
                    (ACTION_ATTR, ACTION_TOGGLE_FAST_FORWARD),
                ],
                vec![text(TOGGLE_LABEL)],
            ),
        ));
    }
    if offer.pinned {
        items.push((
            "pinned".to_owned(),
            el(
                "span",
                vec![("data-testid", PINNED_TESTID)],
                vec![text(PINNED_LABEL)],
            ),
        ));
    }
    if offer.active {
        items.push((
            "active".to_owned(),
            el(
                "span",
                vec![("data-testid", ACTIVE_TESTID)],
                vec![text(ACTIVE_LABEL)],
            ),
        ));
    }
    el(
        "div",
        vec![
            ("data-testid", ControlSection::FastForward.testid()),
            ("class", FAST_FORWARD_CLASS),
        ],
        vec![keyed_view_or_notice(
            "div",
            Vec::new(),
            &FAST_FORWARD_FIELD,
            items,
        )],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(o: FastForwardOffer) -> String {
        format!("{:?}", fast_forward_section(&o))
    }

    #[test]
    fn 切は文字とボタンだけで早送り中を出さない() {
        let t = s(FastForwardOffer {
            on: false,
            can_toggle: true,
            pinned: false,
            active: false,
        });
        assert!(t.contains(OFF_LABEL) && t.contains("\"off\""), "{t}");
        assert!(t.contains(TOGGLE_TESTID), "{t}");
        assert!(!t.contains(ACTIVE_TESTID), "{t}");
    }

    #[test]
    fn 入かつ処理中は早送り中を出す() {
        let t = s(FastForwardOffer {
            on: true,
            can_toggle: true,
            pinned: false,
            active: true,
        });
        assert!(t.contains(ON_LABEL) && t.contains("\"on\""), "{t}");
        assert!(t.contains(ACTIVE_LABEL), "{t}");
    }

    #[test]
    fn 固定のstepは文言を出しボタンを出さない() {
        let t = s(FastForwardOffer {
            on: true,
            can_toggle: false,
            pinned: true,
            active: false,
        });
        assert!(t.contains(PINNED_LABEL) && t.contains(PINNED_TESTID), "{t}");
        assert!(t.contains(ON_LABEL) && !t.contains(TOGGLE_TESTID), "{t}");
    }

    #[test]
    fn 表示だけの間はボタンを出さず文字は残し検査器を通る() {
        let o = FastForwardOffer {
            on: true,
            can_toggle: false,
            pinned: false,
            active: false,
        };
        let n = fast_forward_section(&o);
        let t = format!("{n:?}");
        assert!(!t.contains(TOGGLE_TESTID), "{t}");
        assert!(t.contains(STATUS_TESTID), "{t}");
        assert!(crate::shell::check_no_undefined(&n).is_ok());
        assert!(crate::a11y::check_no_undefined(&n).is_ok());
    }
}
