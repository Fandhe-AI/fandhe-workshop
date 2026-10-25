//! 左の成果物エリアのビューの選択（TASK-470 1/2。ART-5・ART-12・PACK-48）。
//!
//! 呼び出し元は `host::HostModel`（選択の保持と `selected_view`）・`host::EngineHost`（`select-view` の action）・
//! `engine_parts::ArtifactPart`（選択のボタンの描画）である。`Node` は作らず、IR と選択の状態から
//! 「どの種類を見せるか」「どの種類を選べるか」を決める純関数だけを持つ。
//!
//! 規則:
//! - 今の Step が宣言したビュー（`step.views` を `ir.views` の `kind` で引き、一覧外の種類・未定義の ID は
//!   黙って飛ばす。fail-closed）を宣言の順・種類ごとに最初の 1 つで並べる（`artifact_view::declared_kinds`）。
//! - 選べる種類は `pack.left_pane.view_tabs` が決める（PACK-22・TASK-481 1/2）。`all`（既定）は 7 種すべて、
//!   `declared` は宣言した種類だけ、`hidden` は宣言の先頭の 1 つだけである（ボタンも出さない）。宣言に無い種類の
//!   選択は `declared`・`hidden` で無視する。読めない設定は既定の `all` に倒す（`host` が読む）。
//! - 選択は Step ごとに持つ（[`ViewChoice`]）。今の Step が変わると `host::EngineHost` が選択を捨て、
//!   宣言の先頭のビューに戻る（ART-12。元の Step へ戻っても以前の選択は戻らない）。[`effective_view`] は
//!   選んだ Step と今の Step の照合も残す（捨て忘れに対する二重の守り）。
//! - 宣言が無い Step の既定のビューは試作の仮置きである（TASK-470・PACK-48・ART-25。決まった規則は TASK-496）。
//!
//! 属性値・文字はすべて閉じた集合の `&'static str` で、教材が付けたビューの ID は属性にも文字にも出さない。

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::Session;

use crate::artifact_view::{declared_kinds, DeclaredView, ViewKind};
use crate::parts::{FieldKind, PartField};
use crate::profile::ViewTabs;

/// 選ぶ action の名前。payload は [`ViewKind::as_str`] の綴りの完全一致だけを受ける。
pub const ACTION_SELECT_VIEW: &str = "select-view";

/// 選択のボタンの keyed list の field。
pub const VIEW_SELECT_FIELD: PartField = PartField {
    name: "view_select",
    kind: FieldKind::KeyedList,
};

/// 選択のボタンの親の `data-testid`。
pub const VIEW_SELECT_TESTID: &str = "view-select";

/// 選択のボタンの `data-testid`。
pub const VIEW_SELECT_BUTTON_TESTID: &str = "view-select-button";

/// 選択の属性名。値は `"true"` か `"false"`。
pub const SELECTED_ATTR: &str = "data-selected";

/// 保持する選択。どの Step で選んだかを持ち、別の Step では読まない（Step が変われば捨てられる）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewChoice {
    /// 選んだときの Step の実体 ID。
    pub step: StepUid,
    /// 選んだ種類。
    pub kind: ViewKind,
}

/// ボタンの文字。種類ごとの固定の語である。
pub const fn view_label(kind: ViewKind) -> &'static str {
    match kind {
        ViewKind::FileTree => "ファイル",
        ViewKind::Document => "文書",
        ViewKind::Diff => "差分",
        ViewKind::TaskTree => "タスク",
        ViewKind::CardList => "カード",
        ViewKind::CheckList => "チェック",
        ViewKind::AppDemo => "アプリ",
    }
}

/// 今の Step が宣言したビュー。未解決の ID・一覧外の種類は含めない。今の Step が無ければ空。
pub fn declared_views(session: &Session) -> Vec<DeclaredView> {
    let Some(uid) = session.state().run.current.as_ref() else {
        return Vec::new();
    };
    let ir = session.ir();
    let Some(node) = ir.nodes.iter().find(|n| n.uid == uid.0) else {
        return Vec::new();
    };
    node.step
        .views
        .iter()
        .filter_map(|id| {
            let kind = ir
                .views
                .get(id)
                .and_then(|v| v.get("kind"))
                .and_then(|k| k.as_str())
                .and_then(ViewKind::parse)?;
            Some(DeclaredView {
                id: id.clone(),
                kind,
            })
        })
        .collect()
}

/// 選べる種類。`All` は 7 種すべて、`Declared` は宣言の順、`Hidden` は宣言の先頭の 1 つ（宣言が無ければ空）。
pub fn selectable(tabs: ViewTabs, declared: &[(ViewKind, &str)]) -> Vec<ViewKind> {
    match tabs {
        ViewTabs::All => ViewKind::ALL.to_vec(),
        ViewTabs::Declared => declared.iter().map(|(k, _)| *k).collect(),
        ViewTabs::Hidden => declared.iter().take(1).map(|(k, _)| *k).collect(),
    }
}

/// 切り替えのボタンに出す種類。`All` は 7 種、`Declared` は宣言が 2 種以上のときだけ宣言の順、`Hidden` は出さない。
pub fn tab_kinds(tabs: ViewTabs, declared: &[(ViewKind, &str)]) -> Vec<ViewKind> {
    match tabs {
        ViewTabs::All => ViewKind::ALL.to_vec(),
        ViewTabs::Declared if declared.len() >= 2 => declared.iter().map(|(k, _)| *k).collect(),
        ViewTabs::Declared | ViewTabs::Hidden => Vec::new(),
    }
}

/// 選ばれていないときのビュー。
///
/// 宣言があれば先頭の種類。宣言が無いとき、`tabs` が `All` ならファイルツリー、`Declared`・`Hidden` なら
/// `ir.views` を ID の辞書順に見た最初の種類（解決できなければファイルツリー）。後者は試作の仮置きで、
/// 決まった規則は TASK-496 が持つ。ホストは教材の `pack.left_pane.view_tabs` を渡す。
pub fn default_view(tabs: ViewTabs, declared: &[(ViewKind, &str)], ir: &Ir) -> ViewKind {
    if let Some((k, _)) = declared.first() {
        return *k;
    }
    match tabs {
        ViewTabs::All => ViewKind::FileTree,
        ViewTabs::Declared | ViewTabs::Hidden => ir
            .views
            .values()
            .next()
            .and_then(|v| v.get("kind"))
            .and_then(|k| k.as_str())
            .and_then(ViewKind::parse)
            .unwrap_or(ViewKind::FileTree),
    }
}

/// 今見せるビュー。保持した選択が今の Step のもので、かつ今の宣言に含まれるときだけそれを返す。
/// それ以外は [`default_view`]。session が無ければファイルツリー。
pub fn effective_view(
    session: Option<&Session>,
    choice: Option<&ViewChoice>,
    tabs: ViewTabs,
) -> ViewKind {
    let Some(session) = session else {
        return ViewKind::FileTree;
    };
    let views = declared_views(session);
    let declared = declared_kinds(&views);
    if let (Some(c), Some(cur)) = (choice, session.state().run.current.as_ref()) {
        if &c.step == cur && selectable(tabs, &declared).contains(&c.kind) {
            return c.kind;
        }
    }
    default_view(tabs, &declared, session.ir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_distinct_and_nonempty() {
        let mut seen = std::collections::HashSet::new();
        for k in ViewKind::ALL {
            assert!(!view_label(k).is_empty());
            assert!(seen.insert(view_label(k)));
        }
    }

    #[test]
    fn selectable_keeps_declared_order() {
        let d = [(ViewKind::Diff, "a"), (ViewKind::FileTree, "b")];
        assert_eq!(
            selectable(ViewTabs::Declared, &d),
            vec![ViewKind::Diff, ViewKind::FileTree]
        );
        assert_eq!(selectable(ViewTabs::Hidden, &d), vec![ViewKind::Diff]);
        assert_eq!(selectable(ViewTabs::All, &d).len(), 7);
    }

    #[test]
    fn tab_kinds_follow_setting() {
        let one = [(ViewKind::Diff, "a")];
        let two = [(ViewKind::Diff, "a"), (ViewKind::FileTree, "b")];
        assert_eq!(tab_kinds(ViewTabs::All, &[]).len(), 7);
        assert!(tab_kinds(ViewTabs::Declared, &one).is_empty());
        assert_eq!(tab_kinds(ViewTabs::Declared, &two).len(), 2);
        assert!(tab_kinds(ViewTabs::Hidden, &two).is_empty());
    }
}
