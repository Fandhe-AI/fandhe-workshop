//! Step の題・「いまやること」（`do`）・本文を画面に出す部品（TASK-525 1/2。SHELL-7）。
//!
//! 呼び出し元は `host::EngineHost` の `view`（描画）と `update`（前後の比較で dirty を積む）だけである。
//! 現在の節点の題・`do`・本文の木を IR と実行の状態から導き、registry の外の keyed list として描く。
//!
//! - 案内レベルは `RunState.guidance_level` を使い、`core::body_level::select_body`（本文）と
//!   `core::text_fill::step_do_at`（`do`）が同じ「C → B → A」の倒れを持つ。ここでは規則を二重に持たない。
//! - 本文は `doc_view::render_doc` だけを通す。リンク・画像の許可、深さの上限、木の版の検査はそちらが持つ（SAFE-9・SAFE-10）。
//! - 題・`do` は教材由来の信頼できない値で、文字ノードにだけ入れる（`RawHtml` は使わない）。属性値は固定の値と
//!   閉じた集合の水準の文字だけで、項目のキーも uid と水準から作る。
//! - `shell-` で始まる識別子は shell 契約の閉じた一覧が持つため使わない（契約は Step の題・本文・`do` を管轄外とする）。
//!   `role`・`aria-*` も付けない（a11y 契約の閉じた一覧を増やさない）。
//! - 節点が無い・現在の Step が無いときは何も出さない（fail-closed）。

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_contract::doc::Doc;
use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::body_level::select_body;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::text_fill::step_do_at;

use crate::list_policy::keyed_view_or_notice;
use crate::parts::{FieldKind, PartField};

/// 題・`do`・本文の keyed list の field。Step か水準が変わると `host::EngineHost` がこの名前を dirty に積む。
pub const STEP_BODY_FIELD: PartField = PartField {
    name: "step_body",
    kind: FieldKind::KeyedList,
};

/// 親の区画の `data-testid`。
pub const STEP_TEXT_TESTID: &str = "step-text";
/// 題の `data-testid`。
pub const STEP_TITLE_TESTID: &str = "step-title";
/// `do` の `data-testid`。
pub const STEP_DO_TESTID: &str = "step-do";
/// 本文の `data-testid`。
pub const STEP_BODY_TESTID: &str = "step-body";
/// 本文の版の水準の写しの属性名。値は `A`・`B`・`C` のどれかだけ。
pub const BODY_LEVEL_ATTR: &str = "data-body-level";
/// `do` の前に付ける固定の見出し。
pub const DO_LABEL: &str = "いまやること";

/// 親の区画の CSS クラス。`static/app.css` の同名のクラスと一致させる。
pub const STEP_CLASS: &str = "sh-step";
/// `do` の見出しの CSS クラス。
pub const DO_LABEL_CLASS: &str = "sh-do-label";

/// 画面に出す Step の文字と本文の木。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepText<'a> {
    /// 節点の uid（項目のキーに使う）。
    pub uid: &'a str,
    /// 要求した案内レベル。
    pub level: GuidanceLevel,
    /// 題。
    pub title: &'a str,
    /// 水準に応じた `do`。
    pub do_text: Option<&'a str>,
    /// 水準に応じた本文の木と、実際に見つかった版の水準。
    pub body: Option<(GuidanceLevel, &'a Doc)>,
}

/// 現在の Step の題・`do`・本文を導く。節点が無い・現在の Step が無いときは `None`。
pub fn step_text<'a>(ir: &'a Ir, state: &'a SessionState) -> Option<StepText<'a>> {
    let uid = &state.run.current.as_ref()?.0;
    let node = ir.nodes.iter().find(|n| &n.uid == uid)?;
    let level = state.run.guidance_level;
    Some(StepText {
        uid: node.uid.as_str(),
        level,
        title: node.step.title.as_str(),
        do_text: step_do_at(&node.step, level),
        body: select_body(node, level).map(|b| (b.level, b.doc)),
    })
}

/// 表示を決める値（現在の節点の uid と案内レベル）。前後で比べて dirty を積むのに使う。
/// 表示はこの 2 つと IR だけで決まり、IR の差し替えは `install_recorded` が全体を dirty にする。
pub fn step_text_key(state: &SessionState) -> Option<(String, GuidanceLevel)> {
    let uid = state.run.current.as_ref()?;
    Some((uid.0.clone(), state.run.guidance_level))
}

/// 題・`do`・本文を描く keyed list。親は常に描き、項目の出し入れだけで表す（要素の同一性。FEND-3）。
pub fn step_text_list(t: Option<&StepText<'_>>) -> Node {
    let mut items: Vec<(String, Node)> = Vec::new();
    if let Some(t) = t {
        let lv = t.level.as_str();
        items.push((
            format!("title:{}", t.uid),
            el(
                "h2",
                vec![("data-testid", STEP_TITLE_TESTID)],
                vec![text(t.title)],
            ),
        ));
        if let Some(d) = t.do_text {
            items.push((
                format!("do:{}:{lv}", t.uid),
                el(
                    "p",
                    vec![("data-testid", STEP_DO_TESTID)],
                    vec![
                        el(
                            "span",
                            vec![("class", DO_LABEL_CLASS)],
                            vec![text(DO_LABEL)],
                        ),
                        el("span", vec![], vec![text(d)]),
                    ],
                ),
            ));
        }
        if let Some((found, doc)) = t.body {
            items.push((
                format!("body:{}:{lv}", t.uid),
                el(
                    "div",
                    vec![
                        ("data-testid", STEP_BODY_TESTID),
                        (BODY_LEVEL_ATTR, found.as_str()),
                    ],
                    crate::doc_view::render_doc(doc),
                ),
            ));
        }
    }
    keyed_view_or_notice(
        "section",
        vec![("data-testid", STEP_TEXT_TESTID), ("class", STEP_CLASS)],
        &STEP_BODY_FIELD,
        items,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kids(n: &Node) -> &[Node] {
        match n {
            Node::Element { children, .. } => children,
            _ => &[],
        }
    }

    #[test]
    fn empty_has_parent_without_items() {
        let n = step_text_list(None);
        assert!(kids(&n).is_empty());
        assert_eq!(crate::shell::check_no_undefined(&n), Ok(()));
        assert_eq!(crate::a11y::check_no_undefined(&n), Ok(()));
    }

    #[test]
    fn omits_missing_do_and_body_and_keeps_markup_as_text() {
        let t = StepText {
            uid: "main/s",
            level: GuidanceLevel::A,
            title: "<b>題</b>",
            do_text: None,
            body: None,
        };
        let n = step_text_list(Some(&t));
        assert_eq!(kids(&n).len(), 1);
        let h = &kids(&n)[0];
        assert!(matches!(&kids(h)[0], Node::Text(s) if s == "<b>題</b>"));
        assert_eq!(crate::shell::check_no_undefined(&n), Ok(()));
        assert_eq!(crate::a11y::check_no_undefined(&n), Ok(()));
    }
}
