//! 章の完了後の「次の章へ進む」（SHELL-10・INTRO-3。TASK-526 1/2）の導出と描画。
//!
//! 呼び出し元は `host::EngineHost` だけで、`view` が [`next_chapter_list`] を右のエリアの末尾（登録した部品の後ろ。
//! 仮置き。最終の配置は TASK-534・TASK-548）に置き、`update` が前後の提示を [`next_chapter`] で比べて
//! [`NEXT_CHAPTER_FIELD`] を dirty に積む。識別子・表示の文字・個数の規則は `shell`（版 1）が固定済みで、本モジュールは従う側である。
//!
//! 「章の完了」はイベントでなく状態から導く（core は `ChapterCompleted` を出さず `completed` も書かない。TASK-233 以降）。
//! 状態から導くので、再読み込み・履歴の復元の後でも同じ表示になる。出す条件は次のすべてで、欠けたら出さない（fail-closed）。
//!
//! 1. 現在の Step が `main` の根の最後の Step（`export_notice::is_main_final` と同じ判定）である。
//! 2. その Step が達成（`Achieved`）である。
//! 3. 処理中でない。
//! 4. 次の章があり、その最初の Step を指す開始点が IR にある。D-67（SHELL-12）は試行で配る IR から範囲外の開始点を除くので、
//!    開始点が無い章へは出さない。モジュールの展開で生える `#` を含む開始点は章の入口ではないので選ばない。
//!
//! 受講者の画面には全 Step 分の「移動: …」ボタンを出さない。本モジュールの出すボタンは 0 か 1 個である。
//!
//! 1/2 では押しても何も起きない。[`ACTION_NEXT_CHAPTER`] は `host::decode_action` に結線せず、未知の action として捨てられる。
//! 次の章への移動は core の `Control::Begin`（TASK-217。未実装）に依存するため #715（2/2）が決める。
//! 結線するときは payload（DOM 上で書き換えられうる開始点の ID）を `starts` と完全一致で照らすこと（SAFE-33。`start_select` と同じ方針）。
//!
//! 教材由来の文字列（題・開始点の ID）は文字ノードと属性値にだけ入れ、既定のエスケープを通す（SAFE-11）。

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::SessionState;

use crate::artifact_view::{ACTION_ATTR, PAYLOAD_ATTR};
use crate::export_notice::is_main_final;
use crate::list_policy::keyed_view_or_notice;
use crate::parts::{FieldKind, PartField};
use crate::shell::{next_chapter_label, NEXT_CHAPTER_TESTID};

/// 「次の章へ進む」の action の名前。1/2 では `decode_action` に結線しない（#715 が結線する）。
pub const ACTION_NEXT_CHAPTER: &str = "next-chapter";

/// 「次の章へ進む」の keyed list の field。提示が変わると `host::EngineHost` がこの名前を dirty に積む。
pub const NEXT_CHAPTER_FIELD: PartField = PartField {
    name: "next_chapter",
    kind: FieldKind::KeyedList,
};

/// 提示する次の章の入口。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextChapter {
    /// 開始点の ID（`ir.starts[].id`）。
    pub start_id: String,
    /// 開始点の Step の題。
    pub title: String,
}

/// 今の状態で提示する次の章。出さないときは `None`。
pub fn next_chapter(ir: &Ir, state: &SessionState) -> Option<NextChapter> {
    if state.run.busy.is_some() {
        return None;
    }
    let uid = state.run.current.as_ref()?;
    if !is_main_final(ir, uid.0.as_str()) {
        return None;
    }
    let status = state.run.slices.get(&uid.root())?.status.get(uid)?;
    if *status != StepStatus::Achieved {
        return None;
    }
    let current = ir.nodes.iter().find(|n| n.uid == uid.0)?;
    // 章の順序は `main` の節点に `unit` が初めて現れる順。
    let mut units: Vec<&str> = Vec::new();
    for n in ir.nodes.iter().filter(|n| n.scope == "main") {
        if !units.contains(&n.unit.as_str()) {
            units.push(n.unit.as_str());
        }
    }
    let at = units.iter().position(|u| *u == current.unit)?;
    let next_unit = *units.get(at + 1)?;
    let in_next = |uid: &str| {
        ir.nodes
            .iter()
            .any(|n| n.scope == "main" && n.unit == next_unit && n.uid == uid)
    };
    let first = ir
        .nodes
        .iter()
        .find(|n| n.scope == "main" && n.unit == next_unit)?;
    let mut entries = ir.starts.iter().filter(|s| !s.id.contains('#'));
    let start = entries
        .clone()
        .find(|s| s.node == first.uid)
        .or_else(|| entries.find(|s| in_next(&s.node)))?;
    let node = ir.nodes.iter().find(|n| n.uid == start.node)?;
    Some(NextChapter {
        start_id: start.id.clone(),
        title: node.step.title.clone(),
    })
}

/// 「次の章へ進む」の keyed list（項目は 0 か 1 件）。親は常に描き、項目の出し入れだけで表す（FEND-3）。
/// 項目の要素はボタンそのもので、フォーカスを保つため子にしない。
pub fn next_chapter_list(offer: Option<&NextChapter>) -> Node {
    let items = offer
        .map(|o| {
            let label = next_chapter_label(&o.title);
            let button = el(
                "button",
                vec![
                    ("type", "button"),
                    ("data-testid", NEXT_CHAPTER_TESTID),
                    (ACTION_ATTR, ACTION_NEXT_CHAPTER),
                    (PAYLOAD_ATTR, o.start_id.as_str()),
                ],
                vec![text(&label)],
            );
            vec![(o.start_id.clone(), button)]
        })
        .unwrap_or_default();
    keyed_view_or_notice("div", Vec::new(), &NEXT_CHAPTER_FIELD, items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 提示なしは項目を持たない() {
        let s = format!("{:?}", next_chapter_list(None));
        assert!(!s.contains(NEXT_CHAPTER_TESTID), "{s}");
    }

    #[test]
    fn 提示ありはボタンが1つで文字は契約の形() {
        let o = NextChapter {
            start_id: "s-1".into(),
            title: "<b>題</b>".into(),
        };
        let n = next_chapter_list(Some(&o));
        let s = format!("{n:?}");
        assert_eq!(s.matches(NEXT_CHAPTER_TESTID).count(), 1, "{s}");
        assert!(s.contains("次の章へ進む: "), "{s}");
        assert!(crate::shell::check_no_undefined(&n).is_ok());
        assert!(crate::a11y::check_no_undefined(&n).is_ok());
    }
}
