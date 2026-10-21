//! 画面の枠の組み立て（TASK-517 1/2。SHELL-1）。幅 960px 以上の 2 列の骨格を DOM で作る。
//!
//! 呼び出し元は `host::EngineHost::view` だけである。部品の左右の振り分けと、`tablist`・tabpanel・取り付け点
//! （`shell-left`・`shell-right`）の静的な描画を持つ。従う契約は `shell`（取り付け点の識別子と個数）と
//! `a11y`（tab・tabpanel の id・属性・順）で、本モジュールは契約の版を持たない（実装であって契約ではない）。
//! 2 列にする幅と `tablist` を隠す規則は `static/app.css` が持ち、クラス名の正は [`PANES_CLASS`] である。
//!
//! 2/2（TASK-517 2/2）への申し送り:
//! - 960px 未満で片方だけを表示する CSS・クリックと矢印・Home・End による切り替えは 2/2 が足す。
//!   本版の `tablist` は 960px 未満で見えるが、押しても何も起きない。
//! - 選択の状態を持たせて tab の `aria-selected`・`tabindex` を描き直すときは、束縛では書けない
//!   （a11y 版 2 が束縛による aria・role・`tabindex` を禁じる）ので `keyed_list` を使う見込みである。
//!   保持している要素の同一性を崩さないよう、`tablist` は root の直下の独立した子に置いている。
//!
//! TASK-450 への申し送り: ライブの欄（`live-response`・`live-left`）は tabpanel の外（root の直下）に置く。
//! tabpanel の中に置くと a11y の `live-in-tabpanel` に違反する。

use fandhe_frontend_core::{el, text, Node};

use crate::a11y::Region;
use crate::a11y::{PaneTab, TABLIST_LABEL, TABLIST_TESTID, TAB_TESTID};
use crate::parts::PartKind;
use crate::shell::PaneMount;

/// 2 つの領域を並べる入れ物の CSS クラス。`shell-` で始まる識別子は shell 契約が閉じた一覧で持つため、
/// 入れ物には `data-testid` を付けずクラスだけを付ける。`static/app.css` の同名のクラスと一致させる。
pub const PANES_CLASS: &str = "sh-panes";

/// 部品の種類から、置く領域を求める。
///
/// 網羅的に `match` し、種類が増えたらコンパイルで気づけるようにする。左は成果物だけで、他は右（操作）に置く。
/// `Review`・`Checkpoints` の置き場は暫定で、REVIEW-10 と上部の区画の担当が決めたら、ここの 1 行を移す。
pub fn pane_of(kind: PartKind) -> PaneMount {
    match kind {
        PartKind::ArtifactView => PaneMount::Left,
        PartKind::Log => PaneMount::Right,
        PartKind::Offers => PaneMount::Right,
        PartKind::Candidates => PaneMount::Right,
        PartKind::Review => PaneMount::Right,
        PartKind::Checkpoints => PaneMount::Right,
    }
}

/// 静的な `tablist`。タブの順は `a11y::PaneTab::ALL`（操作→成果物）で、既定の選択は操作である。
fn tablist() -> Node {
    let tabs = PaneTab::ALL
        .into_iter()
        .map(|t| {
            let selected = t == PaneTab::Ops;
            el(
                "button",
                vec![
                    ("type", "button"),
                    ("role", "tab"),
                    ("data-testid", TAB_TESTID),
                    ("id", t.tab_id()),
                    ("aria-controls", t.panel_id()),
                    ("aria-selected", if selected { "true" } else { "false" }),
                    ("tabindex", if selected { "0" } else { "-1" }),
                ],
                vec![text(t.label())],
            )
        })
        .collect();
    el(
        "div",
        vec![
            ("role", "tablist"),
            ("data-testid", TABLIST_TESTID),
            ("aria-label", TABLIST_LABEL),
        ],
        tabs,
    )
}

/// 取り付け点の `section` を tabpanel で包む。id と `aria-labelledby` は `PaneMount::pane_tab` から求める。
fn pane(mount: PaneMount, children: Vec<Node>) -> Node {
    let tab = mount.pane_tab();
    let mut section_attrs = vec![("data-testid", mount.testid())];
    // 領域の `aria-label` は左（成果物）だけに付ける。右の「案内とログ」「コマンドの入力」は TERM の担当である。
    if mount == PaneMount::Left {
        section_attrs.push(("aria-label", Region::Artifacts.aria_label()));
    }
    el(
        "div",
        vec![
            ("role", "tabpanel"),
            ("id", tab.panel_id()),
            ("aria-labelledby", tab.tab_id()),
        ],
        vec![el("section", section_attrs, children)],
    )
}

/// 枠を組む。`[tablist, 入れ物]` を返し、呼び出し側が root の子として並べる。
/// 入れ物の中の DOM 順は左→右で、2 列の grid の 1 列目が左になる。
pub fn frame(left: Vec<Node>, right: Vec<Node>) -> [Node; 2] {
    let panes = el(
        "div",
        vec![("class", PANES_CLASS)],
        vec![pane(PaneMount::Left, left), pane(PaneMount::Right, right)],
    );
    [tablist(), panes]
}
