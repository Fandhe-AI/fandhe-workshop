//! 上部の区画 `shell-hint` に置く、ヒントを 1 段ずつ開く操作と開いた段の表示（SHELL-14・GUIDE-17。TASK-530 1/2）の導出・復号・描画。
//!
//! 呼び出し元は `host::EngineHost` だけで、`view` が [`hint_section`] を上部の区画に置き、`update` が前後の [`offer`] を比べて
//! [`HINT_OPS_FIELD`] を dirty に積み、押下を `Control::OpenHint` として core へ送る。段数の上限（現在の案内レベルで出せる数。
//! D-55）と鍵（Step の実体 ID）の規則は core の `hint_open` が 1 か所で持つので、ここでは上限を先に判定しない。
//! 3 段の意味（repo・工程／skill 名／完全なコマンド）は教材側の約束で、エンジンの文言・コードには書かない。
//!
//! ヒントの文面は `ir.hints.<ID>.text` が文字列のときだけ読む。教材由来の未信頼の値なので文字ノードにだけ入れ、属性・キー・
//! ボタンの文言には入れない（SAFE-11）。文字列でない・欠けているときは固定の文を出す（fail-closed）。ヒントの ID は DOM に出さない。
//! 枠の契約（`shell`。版 2）は変えない。

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::hint_select::current_hints;
use fandhe_workshop_core::SessionState;

use crate::artifact_view::{ACTION_ATTR, PAYLOAD_ATTR};
use crate::list_policy::keyed_view_or_notice;
use crate::parts::{FieldKind, PartField};
use crate::shell::TopSection;

/// 「ヒントを開く」の action の名前。payload は空だけを受ける。
pub const ACTION_OPEN_HINT: &str = "open-hint";

/// 「ヒントを開く」のボタンの `data-testid`（0 か 1 個）。
pub const OPEN_TESTID: &str = "hint-open";

/// 開いた段数の状態の文字の `data-testid`（0 か 1 個）。
pub const STATUS_TESTID: &str = "hint-status";

/// 開いた段の `data-testid`（開いた数だけ）。
pub const STAGE_TESTID: &str = "hint-stage";

/// 段の文面の `data-testid`。
pub const TEXT_TESTID: &str = "hint-text";

/// 段の番号（1 始まり）の属性名。
pub const STAGE_ATTR: &str = "data-hint-stage";

/// 状態の文字に載せる、開いた段数の属性名（10 進の数だけ）。
pub const OPENED_ATTR: &str = "data-hint-opened";

/// 状態の文字に載せる、いま出せる段数の属性名（10 進の数だけ）。
pub const TOTAL_ATTR: &str = "data-hint-total";

/// ボタンの文言。
pub const OPEN_LABEL: &str = "ヒントを開く";

/// 文面を読めない段に出す固定の文。
pub const NO_TEXT_LABEL: &str = "（このヒントには文面がありません）";

/// 入れ物の CSS クラス。`static/app.css` の同名のクラスと一致させる。
pub const HINT_CLASS: &str = "sh-hint";

/// ヒントの区画の keyed list の field。提示が変わると `host::EngineHost` がこの名前を dirty に積む。
pub const HINT_OPS_FIELD: PartField = PartField {
    name: "hint_ops",
    kind: FieldKind::KeyedList,
};

/// 開いた段 1 つ。文面が読めないときは `None`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HintStage {
    /// 1 始まりの番号。
    pub index: u32,
    /// 文面。
    pub text: Option<String>,
}

/// 今提示するヒントの操作。前後で比べて dirty を積むための値で、出さないときは空の値。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HintOffer {
    /// ボタンを出すか（現在の Step に出せるヒントが 1 つ以上ある）。
    pub can_open: bool,
    /// 表示する段数（開いた段数を出せる数に丸めた値）。
    pub opened: u32,
    /// いま出せる段数。
    pub total: u32,
    /// 開いた段（先頭から `opened` 個）。
    pub stages: Vec<HintStage>,
}

/// ヒントの文面。`text` が文字列のときだけ返す。
pub fn hint_text<'a>(ir: &'a Ir, id: &str) -> Option<&'a str> {
    ir.hints.get(id)?.as_object()?.get("text")?.as_str()
}

/// 状態から提示を導く。現在の Step が無い・出せるヒントが無いときは空（core の `OpenHint` が no-op になるため）。
pub fn offer(ir: &Ir, state: &SessionState) -> HintOffer {
    let Some(uid) = &state.run.current else {
        return HintOffer::default();
    };
    let ids = current_hints(ir, &state.run);
    if ids.is_empty() {
        return HintOffer::default();
    }
    let total = u32::try_from(ids.len()).unwrap_or(u32::MAX);
    let opened = state.hints_opened_at(uid).min(total);
    let stages = ids
        .iter()
        .take(opened as usize)
        .enumerate()
        .map(|(i, id)| HintStage {
            index: u32::try_from(i + 1).unwrap_or(u32::MAX),
            text: hint_text(ir, id).map(str::to_owned),
        })
        .collect();
    HintOffer {
        can_open: true,
        opened,
        total,
        stages,
    }
}

/// `shell-hint` の入れ物。入れ物は常に描き、中の keyed list の項目の出し入れだけで表す（FEND-3）。
/// ボタンの項目は鍵を固定し、押下のたびに要素を作り直さずフォーカスを保つ。
pub fn hint_section(offer: &HintOffer) -> Node {
    let mut items: Vec<(String, Node)> = Vec::new();
    if offer.can_open {
        let opened = offer.opened.to_string();
        let total = offer.total.to_string();
        items.push((
            format!("status:{opened}:{total}"),
            el(
                "span",
                vec![
                    ("data-testid", STATUS_TESTID),
                    (OPENED_ATTR, opened.as_str()),
                    (TOTAL_ATTR, total.as_str()),
                ],
                vec![text(format!("開いたヒント {opened} / {total}"))],
            ),
        ));
        items.push((
            "open".to_owned(),
            el(
                "button",
                vec![
                    ("type", "button"),
                    ("data-testid", OPEN_TESTID),
                    (ACTION_ATTR, ACTION_OPEN_HINT),
                    (PAYLOAD_ATTR, ""),
                ],
                vec![text(OPEN_LABEL)],
            ),
        ));
        for s in &offer.stages {
            let idx = s.index.to_string();
            let body = s.text.clone().unwrap_or_else(|| NO_TEXT_LABEL.to_owned());
            items.push((
                format!("stage:{idx}:{}", s.text.is_some()),
                el(
                    "div",
                    vec![("data-testid", STAGE_TESTID), (STAGE_ATTR, idx.as_str())],
                    vec![
                        el("span", Vec::new(), vec![text(format!("ヒント {idx}"))]),
                        el("span", vec![("data-testid", TEXT_TESTID)], vec![text(body)]),
                    ],
                ),
            ));
        }
    }
    el(
        "div",
        vec![
            ("data-testid", TopSection::Hint.testid()),
            ("class", HINT_CLASS),
        ],
        vec![keyed_view_or_notice(
            "div",
            Vec::new(),
            &HINT_OPS_FIELD,
            items,
        )],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer_with(stages: Vec<HintStage>, total: u32) -> HintOffer {
        HintOffer {
            can_open: true,
            opened: u32::try_from(stages.len()).unwrap(),
            total,
            stages,
        }
    }

    #[test]
    fn 提示なしは入れ物だけで項目を持たない() {
        let n = hint_section(&HintOffer::default());
        let s = format!("{n:?}");
        assert!(s.contains("shell-hint"), "{s}");
        assert!(!s.contains(OPEN_TESTID), "{s}");
    }

    #[test]
    fn 文面はマークアップの形でも文字ノードのままで属性に入らない() {
        let o = offer_with(
            vec![HintStage {
                index: 1,
                text: Some("<b>x</b>\"onload=1".to_owned()),
            }],
            3,
        );
        let n = hint_section(&o);
        let s = format!("{n:?}");
        assert!(s.contains("<b>x</b>"), "{s}");
        assert_eq!(s.matches(&format!("\"{STAGE_TESTID}\"")).count(), 1, "{s}");
        assert_eq!(s.matches(&format!("\"{OPEN_TESTID}\"")).count(), 1, "{s}");
        assert!(crate::shell::check_no_undefined(&n).is_ok());
        assert!(crate::a11y::check_no_undefined(&n).is_ok());
    }

    #[test]
    fn 文面が無い段は固定の文を出す() {
        let o = offer_with(
            vec![HintStage {
                index: 1,
                text: None,
            }],
            1,
        );
        let s = format!("{:?}", hint_section(&o));
        assert!(s.contains(NO_TEXT_LABEL), "{s}");
    }
}
