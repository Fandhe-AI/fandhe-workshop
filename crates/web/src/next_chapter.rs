//! 章の完了後の「次の章へ進む」（SHELL-10・INTRO-3。TASK-526 1/2）の導出と描画。
//!
//! 呼び出し元は `host::EngineHost` だけで、`view` が [`next_chapter_list`] を右のエリアの末尾（登録した部品の後ろ。
//! 仮置き。最終の配置は TASK-534・TASK-548）に置き、`update` が前後の提示を [`next_chapter`] で比べて
//! [`NEXT_CHAPTER_FIELD`] を dirty に積む。識別子・表示の文字・個数の規則は `shell`（版 1）が固定済みで、本モジュールは従う側である。
//!
//! 「章の完了」はイベントでなく状態から導く（core は `ChapterCompleted` を出さず `completed` も書かない。TASK-233 以降）。
//! 状態から導くので、再読み込み・履歴の復元の後でも同じ表示になる。出す条件は次のすべてで、欠けたら出さない（fail-closed）。
//!
//! 1. 現在の Step が `main` の根の最後の Step（`chapter_end::is_main_final` と同じ判定）である。
//! 2. その Step が達成（`Achieved`）である。
//! 3. 処理中でない。
//! 4. 次の章があり、その最初の Step を指す開始点が IR にある（途中の Step を指す開始点へは代えない）。D-67（SHELL-12）は試行で配る IR から範囲外の開始点を除くので、
//!    開始点が無い章へは出さない。モジュールの展開で生える `#` を含む開始点は章の入口ではないので選ばない。
//!
//! 受講者の画面には全 Step 分の「移動: …」ボタンを出さない。本モジュールの出すボタンは 0 か 1 個である。
//!
//! 押下（TASK-526 2/2）は `host::decode_action` が payload を包み、`update` が [`press_target`] で今の提示と完全一致するときだけ
//! 次の章の入口を開始点として `Control::Begin` で始める（SAFE-33。`start_select` と同じ方針）。payload は DOM 上で書き換えられうるので、
//! 移動先の節点は payload から作らず、一致した開始点を IR の `starts` から引く。提示していない章・途中の Step・前の章へは跳べない。
//!
//! `Control::Begin`（TASK-254 1/2）は世代を +1 し、処理中・完了を解き、開始点より前の未着手を省略として記録して入口へ入り、
//! 記録（`ops`）に `begin` として残るので履歴の復元でも再現する。世界状態・前の周回の切り身は消さない（MOD-8）ので、
//! 1 周目を終えた後の同じボタンが 2 周目を新しい実体の根で始める。2 周目専用の案内文・確認は ROUND2-2（検討中）の決定まで
//! 足さず、SHELL-10 の「ボタン 1 つ・確認なし」のままにする（仮置き）。開始点の案内レベル（`guidance`）は `Begin` が反映しない（TASK-294）。
//!
//! 出す開始点は、`effects` が空で、開始条件（`requires`）が今の状態で成り立つものだけに絞る。`effects` を持つ開始点は
//! 開始用の状態を置き直す入口で、前の周回から引き継いだ世界状態を上書きするので続けて始める入口として選ばない。
//! 条件が成り立たない開始点は `Begin` が断るので、断られるボタンを最初から出さない（fail-closed）。
//!
//! 教材由来の文字列（題・開始点の ID）は文字ノードと属性値にだけ入れ、既定のエスケープを通す（SAFE-11）。

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::instance;
use fandhe_workshop_core::start_gate::check_start;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::SessionState;

use crate::artifact_view::{ACTION_ATTR, PAYLOAD_ATTR};
use crate::export_notice::is_main_final;
use crate::list_policy::keyed_view_or_notice;
use crate::parts::{FieldKind, PartField};
use crate::shell::{next_chapter_label, NEXT_CHAPTER_TESTID};

/// 「次の章へ進む」の action の名前。`host::decode_action` が payload（開始点の ID）を包み、`update` が [`press_target`] で照合する。
pub const ACTION_NEXT_CHAPTER: &str = "next-chapter";

/// 押下の payload が今の提示と完全一致するときだけ、始める開始点の ID を返す（SAFE-33）。
///
/// 提示なし・処理中・不一致は `None`（fail-closed）。ID は一致した開始点を `ir.starts` から引いた値で、payload そのものは使わない。
pub fn press_target(ir: &Ir, state: &SessionState, payload: &str) -> Option<String> {
    let offer = next_chapter(ir, state)?;
    if offer.start_id != payload {
        return None;
    }
    ir.starts
        .iter()
        .find(|s| s.id == offer.start_id)
        .map(|s| s.id.clone())
}

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
    // 章の最後が `use` の Step のとき、その状態は呼ばれた実体の切り身にある（MOD-3）。
    if instance::status_of(ir, &state.run, uid) != StepStatus::Achieved {
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
    let first = ir
        .nodes
        .iter()
        .find(|n| n.scope == "main" && n.unit == next_unit)?;
    // 先頭の Step を指す開始点だけを選ぶ。途中の Step を指す開始点は章の冒頭を飛ばす入口なので、代わりに使わない。
    // `effects` つき（前の周回の世界状態を上書きする入口）と、開始条件が成り立たない開始点（`Begin` が断る）も選ばない。
    let start = ir
        .starts
        .iter()
        .filter(|s| !s.id.contains('#'))
        .filter(|s| s.effects.is_empty())
        .filter(|s| check_start(ir, &state.run, &s.id).is_ok())
        .find(|s| s.node == first.uid)?;
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
