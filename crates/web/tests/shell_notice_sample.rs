//! 枠の通知の口の契約（`shell`）を使うサンプルと、その適合試験（TASK-511 2/2。SHELL-16・17・18・20・31）。
//!
//! 画面・保存復元・公開の担当が真似る手順は次のとおり。
//!
//! 1. 通知は `Notice::new(kind, reason, message, ops)` で作る。種類ごとの理由の要否・文面の識別子・押せる操作の
//!    閉じた表に反すると `Err` になる。
//! 2. 通知 1 件の要素には `Notice::attrs()` の属性の組を付け、子に `shell-notice-text`（文字ノードだけ）と、
//!    `Notice::ops()` の順の操作のボタンを置く。ボタンには `ShellOp::testid`・`data-action`・`data-payload` を付け、
//!    宛先は `Notice::actions()` の `encode()` から得る。
//! 3. 通知は `role="status"` の `shell-notice` の中に並べる。
//! 4. `view()` の結果を `shell::check_complete`（完成した画面）または `shell::check_no_undefined`（部分の木）と、
//!    `a11y::check_no_undefined` の両方に通す。`role=status` の規則は `a11y` が検査する。
//!
//! ダミーの値は試験の側にだけ置き、配信物には入れない。値はすべて汎用のダミーの文字列である。

use fandhe_frontend_core::{el, render, text, Node};
use fandhe_workshop_web::a11y::{self, TESTID_ATTR};
use fandhe_workshop_web::artifact_view::{ACTION_ATTR, PAYLOAD_ATTR};
use fandhe_workshop_web::shell::{
    self, Notice, NoticeKind, PaneMount, ShellAction, ShellOp, Violation, NOTICE_AREA_TESTID,
    NOTICE_ITEM_TESTID, NOTICE_KIND_ATTR, NOTICE_MESSAGE_ATTR, NOTICE_REASON_ATTR,
    NOTICE_TEXT_TESTID,
};

const DANGER: &str = "<img src=x onerror=alert(1)>";

fn button(op: ShellOp, action: &ShellAction) -> Node {
    let (name, payload) = action.encode();
    let mut attrs = vec![(TESTID_ATTR, op.testid()), (ACTION_ATTR, name)];
    if !payload.is_empty() {
        attrs.push((PAYLOAD_ATTR, payload.as_str()));
    }
    el("button", attrs, vec![text("ok")])
}

fn item_with(attrs: Vec<(&str, &str)>, children: Vec<Node>) -> Node {
    el("div", attrs, children)
}

fn notice_node(n: &Notice, body: &str) -> Node {
    let attrs = n.attrs();
    let refs: Vec<(&str, &str)> = attrs.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let mut children = vec![el(
        "p",
        vec![(TESTID_ATTR, NOTICE_TEXT_TESTID)],
        vec![text(body)],
    )];
    for (op, action) in n.ops().iter().zip(n.actions()) {
        children.push(button(*op, &action));
    }
    item_with(refs, children)
}

fn dummy_notices() -> Vec<Notice> {
    let mk = |k: NoticeKind, reason: Option<&str>, ops: &[ShellOp]| {
        Notice::new(k, reason, k.messages()[0], ops).unwrap()
    };
    vec![
        mk(NoticeKind::SaveFailed, Some("quota_exceeded"), &[]),
        mk(
            NoticeKind::CorruptHistory,
            Some("malformed"),
            &[ShellOp::DiscardAndStart],
        ),
        mk(
            NoticeKind::ImportRefused,
            Some("format_newer"),
            &[ShellOp::DismissNotice],
        ),
        mk(
            NoticeKind::VersionConverted,
            None,
            &[ShellOp::DismissNotice],
        ),
        mk(NoticeKind::PackUnloadable, Some("not_json"), &[]),
    ]
}

fn area(role: bool, children: Vec<Node>) -> Node {
    let mut attrs = vec![(TESTID_ATTR, NOTICE_AREA_TESTID)];
    if role {
        attrs.push(("role", "status"));
    }
    el("div", attrs, children)
}

fn pane(m: PaneMount) -> Node {
    el("section", vec![(TESTID_ATTR, m.testid())], vec![])
}

/// 枠全体。上部・左・右・通知欄の順に組む。
fn frame(notice_children: Vec<Node>) -> Node {
    frame_with(
        vec![pane(PaneMount::Left), pane(PaneMount::Right)],
        true,
        notice_children,
    )
}

fn frame_with(panes: Vec<Node>, role: bool, notice_children: Vec<Node>) -> Node {
    let mut children = vec![el("header", vec![(TESTID_ATTR, "shell-top")], vec![])];
    children.extend(panes);
    children.push(area(role, notice_children));
    el("div", vec![], children)
}

fn good_items() -> Vec<Node> {
    dummy_notices()
        .iter()
        .map(|n| notice_node(n, "dummy"))
        .collect()
}

fn rules(r: Result<(), Vec<Violation>>) -> Vec<&'static str> {
    r.unwrap_err()
        .iter()
        .map(|v| match v {
            Violation::RawHtml { .. } => "raw-html",
            Violation::Element { rule, .. } => rule,
            Violation::Count { .. } => "count",
        })
        .collect()
}

fn items_with(f: impl FnOnce(&mut Vec<Node>)) -> Node {
    let mut items = good_items();
    f(&mut items);
    frame(items)
}

fn assert_rule(view: &Node, rule: &str) {
    let r = rules(shell::check_complete(view));
    assert!(r.contains(&rule), "{rule} が出ない: {r:?}");
    let _ = &r;
}

#[test]
fn 正の見本は二つの検査器を通る() {
    let view = frame(good_items());
    assert_eq!(shell::check_complete(&view), Ok(()));
    assert_eq!(shell::check_no_undefined(&view), Ok(()));
    assert_eq!(a11y::check_no_undefined(&view), Ok(()));
}

#[test]
fn 文面の危険な文字列はエスケープされて出る() {
    let n = &dummy_notices()[2];
    let view = frame(vec![notice_node(n, DANGER)]);
    assert_eq!(shell::check_complete(&view), Ok(()));
    let html = render(&view);
    assert!(!html.contains("<img"), "{html}");
    assert!(html.contains("&lt;img"));
}

#[test]
fn 未定義の区画は失敗する() {
    let mut children = vec![el("aside", vec![(TESTID_ATTR, "shell-sidebar")], vec![])];
    children.extend([pane(PaneMount::Left), pane(PaneMount::Right)]);
    let view = frame_with(children, true, good_items());
    assert_rule(&view, "undefined-testid");
}

#[test]
fn 通知要素に_role_を付けると失敗する() {
    for role in ["tree", "status", "alert"] {
        let view = items_with(|v| {
            v[0] = item_with(
                vec![
                    (TESTID_ATTR, NOTICE_ITEM_TESTID),
                    (NOTICE_KIND_ATTR, "save-failed"),
                    (NOTICE_REASON_ATTR, "x"),
                    (NOTICE_MESSAGE_ATTR, "msg-save-failed"),
                    ("role", role),
                ],
                vec![],
            );
        });
        assert_rule(&view, "notice-item-has-role");
    }
}

#[test]
fn 未定義の通知の種類は失敗する() {
    let view = items_with(|v| {
        v.push(item_with(
            vec![
                (TESTID_ATTR, NOTICE_ITEM_TESTID),
                (NOTICE_KIND_ATTR, "delete-history"),
            ],
            vec![],
        ));
    });
    assert_rule(&view, "undefined-notice-kind");
}

#[test]
fn 未定義の文面の識別子は失敗する() {
    let view = frame(vec![item_with(
        vec![
            (TESTID_ATTR, NOTICE_ITEM_TESTID),
            (NOTICE_KIND_ATTR, "save-failed"),
            (NOTICE_REASON_ATTR, "x"),
            (NOTICE_MESSAGE_ATTR, "msg-unknown"),
        ],
        vec![],
    )]);
    assert_rule(&view, "undefined-message");
}

#[test]
fn 種類に許されない操作は失敗する() {
    let dismiss = el(
        "button",
        vec![
            (TESTID_ATTR, ShellOp::DismissNotice.testid()),
            (ACTION_ATTR, "dismiss-notice"),
            (PAYLOAD_ATTR, "save-failed"),
        ],
        vec![],
    );
    let view = frame(vec![item_with(
        vec![
            (TESTID_ATTR, NOTICE_ITEM_TESTID),
            (NOTICE_KIND_ATTR, "save-failed"),
            (NOTICE_REASON_ATTR, "x"),
            (NOTICE_MESSAGE_ATTR, "msg-save-failed"),
        ],
        vec![dismiss],
    )]);
    assert_rule(&view, "op-not-allowed");
}

#[test]
fn dismiss_の_payload_の食い違いは失敗する() {
    let dismiss = el(
        "button",
        vec![
            (TESTID_ATTR, ShellOp::DismissNotice.testid()),
            (ACTION_ATTR, "dismiss-notice"),
            (PAYLOAD_ATTR, "corrupt-history"),
        ],
        vec![],
    );
    let view = frame(vec![item_with(
        vec![
            (TESTID_ATTR, NOTICE_ITEM_TESTID),
            (NOTICE_KIND_ATTR, "import-refused"),
            (NOTICE_REASON_ATTR, "x"),
            (NOTICE_MESSAGE_ATTR, "msg-import-refused"),
        ],
        vec![dismiss],
    )]);
    assert_rule(&view, "op-action");
}

#[test]
fn 理由の欠落は失敗する() {
    let view = frame(vec![item_with(
        vec![
            (TESTID_ATTR, NOTICE_ITEM_TESTID),
            (NOTICE_KIND_ATTR, "import-refused"),
            (NOTICE_MESSAGE_ATTR, "msg-import-refused"),
        ],
        vec![],
    )]);
    assert_rule(&view, "reason-required");
}

#[test]
fn 不正な理由は失敗する() {
    let view = frame(vec![item_with(
        vec![
            (TESTID_ATTR, NOTICE_ITEM_TESTID),
            (NOTICE_KIND_ATTR, "import-refused"),
            (NOTICE_REASON_ATTR, "<b>"),
            (NOTICE_MESSAGE_ATTR, "msg-import-refused"),
        ],
        vec![],
    )]);
    assert_rule(&view, "reason-format");
}

#[test]
fn 通知欄の外の通知は失敗する() {
    let stray = notice_node(&dummy_notices()[0], "dummy");
    let mut children = vec![
        el("header", vec![(TESTID_ATTR, "shell-top")], vec![]),
        pane(PaneMount::Left),
        pane(PaneMount::Right),
        area(true, vec![]),
    ];
    children.push(stray);
    let view = el("div", vec![], children);
    assert_rule(&view, "notice-item-outside-area");
}

#[test]
fn 同じ種類の通知の重複は失敗する() {
    let view = items_with(|v| v.push(notice_node(&dummy_notices()[0], "dummy")));
    assert_rule(&view, "duplicate-notice-kind");
}

#[test]
fn 文面の部品に要素を入れると失敗する() {
    let bad = item_with(
        vec![
            (TESTID_ATTR, NOTICE_ITEM_TESTID),
            (NOTICE_KIND_ATTR, "save-failed"),
            (NOTICE_REASON_ATTR, "x"),
            (NOTICE_MESSAGE_ATTR, "msg-save-failed"),
        ],
        vec![el(
            "p",
            vec![(TESTID_ATTR, NOTICE_TEXT_TESTID)],
            vec![el("b", vec![], vec![])],
        )],
    );
    assert_rule(&frame(vec![bad]), "notice-text-content");
}

#[test]
fn 取り付け点の重複と欠落は失敗する() {
    let dup = frame_with(
        vec![
            pane(PaneMount::Left),
            pane(PaneMount::Left),
            pane(PaneMount::Right),
        ],
        true,
        good_items(),
    );
    assert_rule(&dup, "duplicate");
    let none = frame_with(vec![pane(PaneMount::Right)], true, good_items());
    assert_rule(&none, "count");
    assert_eq!(shell::check_no_undefined(&none), Ok(()));
    // 空の木は fail-closed で通らない。
    assert!(shell::check_complete(&el("div", vec![], vec![])).is_err());
}

#[test]
fn 生_html_は失敗する() {
    let view = items_with(|v| v.push(Node::RawHtml("<b>x</b>".to_owned())));
    assert_rule(&view, "raw-html");
}

#[test]
fn 操作の部品の置き方の違反() {
    // 通知の外の dismiss。
    let outside = frame_with(
        vec![
            pane(PaneMount::Left),
            pane(PaneMount::Right),
            el(
                "button",
                vec![
                    (TESTID_ATTR, ShellOp::DismissNotice.testid()),
                    (ACTION_ATTR, "dismiss-notice"),
                    (PAYLOAD_ATTR, "save-failed"),
                ],
                vec![],
            ),
        ],
        true,
        good_items(),
    );
    assert_rule(&outside, "op-outside-notice");
    // 読み込みの本文を DOM に置く。
    let import = frame_with(
        vec![
            pane(PaneMount::Left),
            pane(PaneMount::Right),
            el(
                "button",
                vec![
                    (TESTID_ATTR, ShellOp::Import.testid()),
                    (ACTION_ATTR, "import-history"),
                    (PAYLOAD_ATTR, "{}"),
                ],
                vec![],
            ),
        ],
        true,
        good_items(),
    );
    assert_rule(&import, "import-payload-in-dom");
    // testid の無い操作の action。
    let bare = frame_with(
        vec![
            pane(PaneMount::Left),
            pane(PaneMount::Right),
            el("button", vec![(ACTION_ATTR, "export-history")], vec![]),
        ],
        true,
        good_items(),
    );
    assert_rule(&bare, "shell-action-without-testid");
}

#[test]
fn 通知欄の役割が無いと_a11y_の検査が失敗する() {
    let view = frame_with(
        vec![pane(PaneMount::Left), pane(PaneMount::Right)],
        false,
        good_items(),
    );
    assert_eq!(shell::check_complete(&view), Ok(()));
    assert!(a11y::check_no_undefined(&view)
        .unwrap_err()
        .iter()
        .any(|v| matches!(
            v,
            a11y::Violation::Element {
                rule: "notice-role",
                ..
            }
        )));
}

#[test]
fn 違反の文面に文字ノードの中身を含めない() {
    let n = &dummy_notices()[0];
    let view = frame_with(
        vec![pane(PaneMount::Left), pane(PaneMount::Right)],
        true,
        vec![
            notice_node(n, "SECRET-MARKER"),
            notice_node(n, "SECRET-MARKER"),
        ],
    );
    let errs = shell::check_complete(&view).unwrap_err();
    assert!(!errs.is_empty());
    for v in errs {
        assert!(!format!("{v} {v:?}").contains("SECRET-MARKER"));
    }
}
