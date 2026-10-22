//! 画面の枠の組み立て（TASK-517。SHELL-1）。幅 960px 以上の 2 列の骨格と、960px 未満のタブの切り替えを DOM で作る。
//!
//! 呼び出し元は `host::EngineHost::view`（描画）と `host::EngineHost::update`（`select-pane`・`pane-key` の
//! action）だけである。部品の左右の振り分けと、`tablist`・tabpanel・取り付け点（`shell-left`・`shell-right`）の描画を持つ。
//! 従う契約は `shell`（取り付け点の識別子と個数）と `a11y`（tab・tabpanel の id・属性・順）で、本モジュールは
//! 契約の版を持たない（実装であって契約ではない）。2 列にする幅・`tablist` を隠す規則・960px 未満で選んでいない
//! 領域を隠す規則は `static/app.css` が持ち、クラス名の正は [`PANES_CLASS`]、選択の写しの属性の正は [`PANE_ATTR`] である。
//!
//! 選択の持ち方:
//! - 選択は `host::EngineHost` が表示の状態として持つ（`ops`・履歴・保存先には書かない）。本モジュールは引数で受ける。
//! - a11y 版 2 は束縛による aria・role・`tabindex` を禁じるので、`tablist` は keyed list（[`PANE_TABS_FIELD`]）にする。
//!   項目の root を `button[role=tab]` そのものにするのは、子の作り直しでボタンのフォーカスが失われないようにするためである。
//! - `.sh-panes`・tabpanel は静的なまま描き直さない（要素の同一性。FEND-3）。隠すのは `tablist` の `data-pane` を
//!   読む CSS だけが行う。
//! - キーの action（[`ACTION_PANE_KEY`]）の `data-keys` に Esc を含めるのは、keydown の属性が最も近い 1 つにしか届かず、
//!   含めないとタブにフォーカスがある Esc が root の Esc に届かなくなる（ちょうど 1 回、FEND-6）ためである。
//!   `data-keydown-prevent-default` は Esc にも掛かるが、ボタンの上の Esc に既定の動作は無く、観察できる差は無い。
//!   Home・End はこれが無いとページを先頭・末尾へ動かす。
//! - キーで切り替えたときは、選んだタブへフォーカスも移す（`a11y::tab_key_target` の契約）。移さないと、元のタブに
//!   フォーカスが残り Enter・Space で元の選択へ戻る。ホストが `set_pane_focus_request` の通知先へ要求し、glue が
//!   描画の後に `aria-selected="true"` のタブへ `focus()` する。クリックでは移さない。置き方の細部は A11Y-21 が未決である
//!   （`docs/known-gaps.md`）。
//!
//! TASK-450 への申し送り: ライブの欄（`live-response`・`live-left`）は tabpanel の外（root の直下）に置く。
//! tabpanel の中に置くと a11y の `live-in-tabpanel` に違反する。

use fandhe_frontend_core::{el, text, Node};

use crate::a11y::Region;
use crate::a11y::{PaneTab, TABLIST_LABEL, TABLIST_TESTID, TAB_TESTID};
use crate::artifact_view::{ACTION_ATTR, PAYLOAD_ATTR};
use crate::list_policy::keyed_view_or_notice;
use crate::parts::{FieldKind, PartField, PartKind};
use crate::shell::{PaneMount, TopSection, MOCK_LABEL};

/// タブのクリック（と Enter・Space による click）の action 名。payload は [`PaneTab::id`] の完全一致だけを受ける。
pub const ACTION_SELECT_PANE: &str = "select-pane";

/// tablist の上のキーの action 名。payload は `KeyboardEvent.key` で、[`PANE_KEYS`] の値だけが届く。
pub const ACTION_PANE_KEY: &str = "pane-key";

/// tablist が受けるキー。矢印・Home・End に加え、Esc を root へ届けるために含める（モジュールの doc）。
pub const PANE_KEYS: &str = "ArrowLeft ArrowRight Home End Escape";

/// tablist の keyed list の field。選択が変わると `host::EngineHost` がこの名前を dirty に積む。
pub const PANE_TABS_FIELD: PartField = PartField {
    name: "pane_tabs",
    kind: FieldKind::KeyedList,
};

/// 選択の写しの属性名。値は [`PaneTab::id`]。`static/app.css` が 960px 未満の表示の切り替えに読む。
pub const PANE_ATTR: &str = "data-pane";

/// payload からタブを引く。[`PaneTab::id`] の完全一致だけを受け、それ以外は `None`（fail-closed）。
pub fn pane_from_payload(payload: &str) -> Option<PaneTab> {
    PaneTab::ALL.into_iter().find(|t| t.id() == payload)
}

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

/// `tablist`。タブの順は `a11y::PaneTab::ALL`（操作→成果物）で、`selected` のタブが選択の状態になる。
///
/// 項目が 2 件・キーが固定なので `keyed_view` は拒否しないが、拒否しても panic せず、親の `data-bind-list` を保った
/// 一覧に退く（`keyed_view_or_notice`。全再描画を避ける）。
fn tablist(selected: PaneTab) -> Node {
    let items = PaneTab::ALL
        .into_iter()
        .map(|t| {
            let on = t == selected;
            let button = el(
                "button",
                vec![
                    ("type", "button"),
                    ("role", "tab"),
                    ("data-testid", TAB_TESTID),
                    ("id", t.tab_id()),
                    ("aria-controls", t.panel_id()),
                    ("aria-selected", if on { "true" } else { "false" }),
                    ("tabindex", if on { "0" } else { "-1" }),
                    (ACTION_ATTR, ACTION_SELECT_PANE),
                    (PAYLOAD_ATTR, t.id()),
                ],
                vec![text(t.label())],
            );
            (t.id().to_owned(), button)
        })
        .collect();
    keyed_view_or_notice(
        "div",
        vec![
            ("role", "tablist"),
            ("data-testid", TABLIST_TESTID),
            ("aria-label", TABLIST_LABEL),
            (PANE_ATTR, selected.id()),
            ("data-action-keydown", ACTION_PANE_KEY),
            ("data-keys", PANE_KEYS),
            ("data-keydown-prevent-default", ""),
        ],
        &PANE_TABS_FIELD,
        items,
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
/// 入れ物の中の DOM 順は左→右で、2 列の grid の 1 列目が左になる。`selected` は 960px 未満で見せる領域である。
pub fn frame(left: Vec<Node>, right: Vec<Node>, selected: PaneTab) -> [Node; 2] {
    let panes = el(
        "div",
        vec![("class", PANES_CLASS)],
        vec![pane(PaneMount::Left, left), pane(PaneMount::Right, right)],
    );
    [tablist(selected), panes]
}

/// 上部の区画の CSS クラス。`static/app.css` の同名のクラスと一致させる（識別子は `data-testid` が持つ）。
pub const TOP_CLASS: &str = "sh-top";

/// 「模擬」の表示の CSS クラス。`static/app.css` の同名のクラスと一致させる。
pub const MOCK_CLASS: &str = "sh-mock";

/// 画面の枠の上部（`shell-top`）。「模擬」（`shell-mock`）を常に先頭に置く（TASK-523。SHELL-6・D-72）。
///
/// 教材が本物の環境と取り違えられないよう、どの Step・どの幅・通知の有無でも消えない表示にする。
/// 呼び出し元は `host::EngineHost::view` で、root の先頭の子として置く。tabpanel の外なので 960px 未満でも隠れない。
/// 静的なノードで、dirty の field にも registry にも入れない（要素の同一性。FEND-3）。
/// `role`・`aria-label` は付けない（a11y 契約の閉じた一覧を増やさない）。見えている文字ノードとして
/// アクセシビリティツリーに載る。文字は定数 [`MOCK_LABEL`] だけで、教材由来の値・利用者の入力は使わない。
///
/// TASK-524 への申し送り: 教材名（`shell-title`）・位置（`shell-position`）は `rest` に渡して「模擬」の後ろに足す。
pub fn top_bar(rest: Vec<Node>) -> Node {
    let mut children = vec![el(
        "span",
        vec![
            ("data-testid", TopSection::Mock.testid()),
            ("class", MOCK_CLASS),
        ],
        vec![text(MOCK_LABEL)],
    )];
    children.extend(rest);
    el(
        "div",
        vec![
            ("data-testid", TopSection::Top.testid()),
            ("class", TOP_CLASS),
        ],
        children,
    )
}
