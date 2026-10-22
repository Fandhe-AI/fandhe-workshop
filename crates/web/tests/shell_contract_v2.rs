//! 枠の契約の版 2（`shell`、TASK-516 1/2・2/2。SHELL-6・8・21・22・24・30・36・38。D-72・D-54・D-74・D-57・D-52・D-75）の適合試験とサンプル。
//!
//! 版 1 の値が同じ値・同じ順で残ること（版 1 を使う画面が壊れない根拠）、上部の区画の確定、早送りの区画、
//! ブラウザの案内、版 2 の検査器の正負の見本を確かめる。試験では `unwrap` を使ってよい。

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_web::a11y::{self, TESTID_ATTR};
use fandhe_workshop_web::artifact_view::{ACTION_ATTR, PAYLOAD_ATTR};
use fandhe_workshop_web::shell::*;
use fandhe_workshop_web::terminal_area::TERMINAL_AREA_TESTIDS;

#[test]
fn 版は_2() {
    assert_eq!(SHELL_CONTRACT_VERSION, 2);
}

#[test]
fn 版_1_の識別子と名前が先頭に同じ順で残る() {
    let top: Vec<_> = TopSection::ALL.iter().map(|s| s.testid()).collect();
    assert_eq!(
        top,
        [
            "shell-top",
            "shell-hint",
            "shell-checkpoint",
            "shell-title",
            "shell-position",
            "shell-mock"
        ]
    );
    let ops: Vec<_> = ShellOp::ALL.iter().map(|o| o.as_str()).collect();
    assert_eq!(
        ops[..4],
        [
            "export-history",
            "import-history",
            "discard-and-start",
            "dismiss-notice"
        ]
    );
    let kinds: Vec<_> = NoticeKind::ALL.iter().map(|k| k.as_str()).collect();
    assert_eq!(
        kinds[..5],
        [
            "save-failed",
            "corrupt-history",
            "import-refused",
            "version-converted",
            "pack-unloadable"
        ]
    );
    let msgs: Vec<_> = MessageId::ALL.iter().map(|m| m.as_str()).collect();
    assert_eq!(msgs[0], "msg-save-failed");
    assert_eq!(msgs[4], "msg-pack-unloadable");
}

#[test]
fn 上部の三区画は確定しモックだけが必須() {
    for s in TopSection::ALL {
        assert_eq!(s.status(), SectionStatus::Fixed);
        let want = if s == TopSection::Mock {
            Cardinality::ExactlyOne
        } else {
            Cardinality::AtMostOne
        };
        assert_eq!(s.cardinality(), want);
    }
    assert_eq!(MOCK_LABEL, "模擬");
}

#[test]
fn 位置は_1_以上_m_以下_k_だけを受ける() {
    for (m, k) in [(1, 1), (3, 6), (6, 6)] {
        let p = StepPosition::new(m, k).unwrap();
        assert_eq!((p.m(), p.k()), (m, k));
    }
    for (m, k) in [(0, 1), (7, 6), (0, 0), (1, 0)] {
        assert_eq!(StepPosition::new(m, k), None, "{m}/{k}");
    }
    let a = StepPosition::new(3, 6).unwrap().attrs();
    assert_eq!(a[0], ("data-position-m", "3".to_owned()));
    assert_eq!(a[1], ("data-position-k", "6".to_owned()));
}

#[test]
fn 早送りの区画の値と非重複() {
    assert_eq!(ControlSection::ALL.len(), 1);
    assert_eq!(ControlSection::FastForward.testid(), "shell-fast-forward");
    assert_eq!(
        ControlSection::parse("shell-fast-forward"),
        Some(ControlSection::FastForward)
    );
    for bad in ["", "fast-forward", "shell-fast-forward "] {
        assert_eq!(ControlSection::parse(bad), None);
    }
    assert_eq!(ControlSection::FastForward.status(), SectionStatus::Fixed);
    assert_eq!(
        ControlSection::FastForward.cardinality(),
        Cardinality::AtMostOne
    );
    assert!(is_defined_testid("shell-fast-forward"));
    assert!(!TERMINAL_AREA_TESTIDS.contains(&"shell-fast-forward"));
    // D-54・EVT-27: 早送りの入・切は履歴の ops に入らない。値は contract が持つ。
    const { assert!(!fandhe_workshop_contract::history::OPS_RECORDS_FAST_FORWARD) };
}

#[test]
fn ブラウザの案内の値() {
    let k = NoticeKind::BrowserNotice;
    assert_eq!(k.as_str(), "browser-notice");
    assert_eq!(NoticeKind::parse("browser-notice"), Some(k));
    assert_eq!(k.reason_rule(), ReasonRule::Optional);
    assert_eq!(k.allowed_ops(), [ShellOp::DismissNotice]);
    assert_eq!(k.messages(), [MessageId::BrowserNotice]);
    assert_eq!(MessageId::BrowserNotice.as_str(), "msg-browser-notice");
    assert_eq!(
        BROWSER_NOTICE_TEXT,
        "この教材は Chrome の最新版で確かめています"
    );
    assert_eq!(
        ShellAction::decode("dismiss-notice", "browser-notice"),
        Some(ShellAction::DismissNotice("browser-notice".to_owned()))
    );
    assert!(Notice::new(k, None, MessageId::BrowserNotice, &[ShellOp::DismissNotice]).is_ok());
}

#[test]
fn ブラウザの案内は_ua_らしい理由を拒否する() {
    for ua in ["Mozilla/5.0 (X11; Linux)", "a b", "a/b", "(x)"] {
        assert_eq!(
            Notice::new(
                NoticeKind::BrowserNotice,
                Some(ua),
                MessageId::BrowserNotice,
                &[]
            ),
            Err(NoticeError::ReasonFormat),
            "{ua}"
        );
    }
}

#[test]
fn 版_2_でもまだ無いもの() {
    for name in ["next-action", "reload", "delete-history", "stale-tab"] {
        assert_eq!(NoticeKind::parse(name), None);
    }
    assert!(!is_defined_testid("shell-reload"));
}

// ---- 見本 ----

fn tid(t: &'static str, children: Vec<Node>) -> Node {
    el("div", vec![(TESTID_ATTR, t)], children)
}

fn position(m: &str, k: &str) -> Node {
    el(
        "span",
        vec![
            (TESTID_ATTR, "shell-position"),
            (POSITION_M_ATTR, m),
            (POSITION_K_ATTR, k),
        ],
        vec![text("CH-1 Step 3 / 6")],
    )
}

fn top(children: Vec<Node>) -> Node {
    tid("shell-top", children)
}

fn good_top() -> Vec<Node> {
    vec![
        tid("shell-title", vec![text("題")]),
        position("3", "6"),
        tid("shell-mock", vec![text(MOCK_LABEL)]),
    ]
}

fn browser_notice() -> Node {
    let n = Notice::new(
        NoticeKind::BrowserNotice,
        None,
        MessageId::BrowserNotice,
        &[ShellOp::DismissNotice],
    )
    .unwrap();
    let attrs = n.attrs();
    let refs: Vec<(&str, &str)> = attrs.iter().map(|(k, v)| (*k, v.as_str())).collect();
    el(
        "div",
        refs,
        vec![
            el(
                "p",
                vec![(TESTID_ATTR, NOTICE_TEXT_TESTID)],
                vec![text(BROWSER_NOTICE_TEXT)],
            ),
            el(
                "button",
                vec![
                    (TESTID_ATTR, "shell-dismiss"),
                    (ACTION_ATTR, "dismiss-notice"),
                    (PAYLOAD_ATTR, "browser-notice"),
                ],
                vec![text("閉じる")],
            ),
        ],
    )
}

fn frame(top_children: Vec<Node>, outside: Vec<Node>, right_extra: Vec<Node>) -> Node {
    let mut c = vec![
        top(top_children),
        el("section", vec![(TESTID_ATTR, "shell-left")], vec![]),
        el("section", vec![(TESTID_ATTR, "shell-right")], right_extra),
        el(
            "div",
            vec![(TESTID_ATTR, NOTICE_AREA_TESTID), ("role", "status")],
            vec![browser_notice()],
        ),
    ];
    c.extend(outside);
    el("div", vec![], c)
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

fn good_frame() -> Node {
    frame(good_top(), vec![], vec![tid("shell-fast-forward", vec![])])
}

#[test]
fn 正の見本は全ての検査器を通る() {
    let f = good_frame();
    check_complete(&f).unwrap();
    check_complete_v2(&f).unwrap();
    check_no_undefined(&f).unwrap();
    a11y::check_no_undefined(&f).unwrap();
}

#[test]
fn 模擬が無い画面は版_2_だけが失敗する() {
    let f = frame(vec![tid("shell-title", vec![text("題")])], vec![], vec![]);
    check_complete(&f).unwrap();
    let v = check_complete_v2(&f).unwrap_err();
    assert!(matches!(&v[..], [Violation::Count { what, found: 0, .. }] if what == "shell-mock"));
}

#[test]
fn 重複は_duplicate() {
    for id in ["shell-mock", "shell-fast-forward"] {
        let f = frame(
            {
                let mut t = good_top();
                t.push(tid(id, vec![]));
                t
            },
            vec![],
            vec![tid("shell-fast-forward", vec![])],
        );
        assert!(rules(check_complete_v2(&f)).contains(&"duplicate"), "{id}");
    }
}

#[test]
fn 上部の区画が上部の外にあると違反() {
    for id in ["shell-mock", "shell-position", "shell-title"] {
        let f = frame(good_top(), vec![tid(id, vec![])], vec![]);
        assert!(
            rules(check_no_undefined(&f)).contains(&"top-section-outside-top"),
            "{id}"
        );
        assert!(rules(check_complete(&f)).contains(&"top-section-outside-top"));
    }
}

#[test]
fn 位置の属性の形の違反() {
    for (m, k) in [
        ("7", "6"),
        ("0", "6"),
        ("x", "6"),
        ("3", ""),
        ("+3", "6"),
        ("3", "-6"),
    ] {
        let f = frame(
            vec![position(m, k), tid("shell-mock", vec![])],
            vec![],
            vec![],
        );
        assert_eq!(
            rules(check_no_undefined(&f)),
            ["position-format"],
            "{m}/{k}"
        );
    }
}

#[test]
fn ブラウザの案内に許されない操作は違反() {
    let mut item = browser_notice();
    if let Node::Element { children, .. } = &mut item {
        children.push(el(
            "button",
            vec![
                (TESTID_ATTR, "shell-discard-start"),
                (ACTION_ATTR, "discard-and-start"),
            ],
            vec![],
        ));
    }
    let f = el(
        "div",
        vec![],
        vec![el(
            "div",
            vec![(TESTID_ATTR, NOTICE_AREA_TESTID), ("role", "status")],
            vec![item],
        )],
    );
    assert!(rules(check_no_undefined(&f)).contains(&"op-not-allowed"));
}

// ---- 2/2: 通知・操作の口（SHELL-21・22・36・38） ----

use fandhe_workshop_contract::load_next_action::next_action;
use fandhe_workshop_contract::load_reason::LoadReason;

#[test]
fn 削除の操作の値と往復() {
    let want = [
        (ShellOp::DeleteHistory, "delete-history", "shell-delete"),
        (
            ShellOp::ConfirmDelete,
            "confirm-delete-history",
            "shell-delete-confirm",
        ),
        (
            ShellOp::CancelDelete,
            "cancel-delete-history",
            "shell-delete-cancel",
        ),
    ];
    for (op, name, testid) in want {
        assert_eq!(op.as_str(), name);
        assert_eq!(op.testid(), testid);
        assert_eq!(ShellOp::from_name(name), Some(op));
        let a = ShellAction::decode(name, "").unwrap();
        assert_eq!(a.encode(), (name, String::new()));
        assert_eq!(ShellAction::decode(name, "x"), None, "{name}");
        assert_eq!(ShellAction::decode(name, " "), None, "{name}");
        assert!(!format!("{a:?}").is_empty());
    }
    assert_eq!(ShellOp::ALL.len(), 7);
}

#[test]
fn 書き出しの案内と削除の確認の値() {
    let e = NoticeKind::ExportDraft;
    assert_eq!(e.as_str(), "export-draft");
    assert_eq!(NoticeKind::parse("export-draft"), Some(e));
    assert_eq!(e.reason_rule(), ReasonRule::Optional);
    assert_eq!(e.messages(), [MessageId::ExportDraft]);
    assert_eq!(e.allowed_ops(), [ShellOp::DismissNotice]);
    assert_eq!(MessageId::ExportDraft.as_str(), "msg-export-draft");
    let d = NoticeKind::DeleteConfirm;
    assert_eq!(d.as_str(), "delete-confirm");
    assert_eq!(NoticeKind::parse("delete-confirm"), Some(d));
    assert_eq!(d.reason_rule(), ReasonRule::Optional);
    assert_eq!(d.messages(), [MessageId::DeleteConfirm]);
    assert_eq!(MessageId::DeleteConfirm.as_str(), "msg-delete-confirm");
    for k in [e, d] {
        assert!(matches!(
            ShellAction::decode("dismiss-notice", k.as_str()),
            Some(ShellAction::DismissNotice(_))
        ));
    }
    let n = Notice::new(
        d,
        None,
        MessageId::DeleteConfirm,
        &[ShellOp::ConfirmDelete, ShellOp::CancelDelete],
    )
    .unwrap();
    assert_eq!(
        n.actions(),
        [ShellAction::ConfirmDelete, ShellAction::CancelDelete]
    );
    assert_eq!(
        Notice::new(d, None, MessageId::DeleteConfirm, &[ShellOp::DeleteHistory]),
        Err(NoticeError::OpNotAllowed(ShellOp::DeleteHistory))
    );
}

#[test]
fn 理由コードの束縛() {
    for r in LoadReason::ALL {
        for k in [NoticeKind::ImportRefused, NoticeKind::CorruptHistory] {
            assert_eq!(k.reason_binding(), ReasonBinding::LoadReason);
            assert!(
                Notice::new(k, Some(r.as_str()), k.messages()[0], &[]).is_ok(),
                "{} {}",
                k.as_str(),
                r.as_str()
            );
        }
    }
    for bad in ["x", "Malformed", "too-large"] {
        let k = NoticeKind::ImportRefused;
        assert_eq!(
            Notice::new(k, Some(bad), k.messages()[0], &[]),
            Err(NoticeError::ReasonUnbound),
            "{bad}"
        );
    }
    for (k, r) in [
        (NoticeKind::SaveFailed, "quota_exceeded"),
        (NoticeKind::PackUnloadable, "x"),
    ] {
        assert_eq!(k.reason_binding(), ReasonBinding::Opaque);
        assert!(Notice::new(k, Some(r), k.messages()[0], &[]).is_ok());
    }
}

#[test]
fn 次の行動は理由から導かれ属性に出る() {
    for r in LoadReason::ALL {
        let k = NoticeKind::ImportRefused;
        let n = Notice::new(k, Some(r.as_str()), k.messages()[0], &[]).unwrap();
        let want = next_action(r);
        assert_eq!(n.next_action(), Some(want));
        assert!(n
            .attrs()
            .contains(&(NOTICE_NEXT_ACTION_ATTR, want.as_str().to_owned())));
    }
    for k in NoticeKind::ALL {
        if k == NoticeKind::ImportRefused {
            continue;
        }
        let reason = match k {
            NoticeKind::SaveFailed | NoticeKind::PackUnloadable => Some("x"),
            NoticeKind::CorruptHistory => Some("malformed"),
            _ => None,
        };
        let n = Notice::new(k, reason, k.messages()[0], &[]).unwrap();
        assert_eq!(n.next_action(), None, "{}", k.as_str());
        assert!(n.attrs().iter().all(|(a, _)| *a != NOTICE_NEXT_ACTION_ATTR));
    }
    assert_eq!(NOTICE_NEXT_ACTION_ATTR, "data-notice-next-action");
}

#[test]
fn 再読み込みの告知は枠に足さない() {
    assert_eq!(NoticeKind::parse("reload"), None);
    assert_eq!(NoticeKind::parse("reloaded"), None);
    assert_eq!(ShellOp::from_name("reload-notice"), None);
    assert!(!is_defined_testid("shell-reload"));
    // 運び手は terminal_area の行の種類 `reloaded`（D-57。TASK-322）。
    assert!(fandhe_workshop_web::terminal_area::LineKind::Reloaded.as_str() == "reloaded");
}

// ---- 2/2 の見本 ----

fn notice_el(
    kind: NoticeKind,
    reason: Option<&str>,
    ops: &[ShellOp],
    mutate: impl Fn(&mut Vec<(&'static str, String)>),
) -> Node {
    let n = Notice::new(kind, reason, kind.messages()[0], ops).unwrap();
    let mut attrs = n.attrs();
    mutate(&mut attrs);
    let refs: Vec<(&str, &str)> = attrs.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let mut children = vec![el(
        "p",
        vec![(TESTID_ATTR, NOTICE_TEXT_TESTID)],
        vec![text("文面")],
    )];
    for a in n.actions() {
        let (name, payload) = a.encode();
        let op = ShellOp::from_name(name).unwrap();
        let mut at = vec![(TESTID_ATTR, op.testid()), (ACTION_ATTR, name)];
        if !payload.is_empty() {
            at.push((PAYLOAD_ATTR, kind.as_str()));
        }
        children.push(el("button", at, vec![text("操作")]));
    }
    el("div", refs, children)
}

fn area(items: Vec<Node>) -> Node {
    el(
        "div",
        vec![(TESTID_ATTR, NOTICE_AREA_TESTID), ("role", "status")],
        items,
    )
}

fn delete_button() -> Node {
    el(
        "button",
        vec![
            (TESTID_ATTR, "shell-delete"),
            (ACTION_ATTR, "delete-history"),
        ],
        vec![text("削除")],
    )
}

fn page(top_children: Vec<Node>, right: Vec<Node>, items: Vec<Node>) -> Node {
    el(
        "div",
        vec![],
        vec![
            top(top_children),
            el("section", vec![(TESTID_ATTR, "shell-left")], vec![]),
            el("section", vec![(TESTID_ATTR, "shell-right")], right),
            area(items),
        ],
    )
}

fn confirm_item() -> Node {
    notice_el(
        NoticeKind::DeleteConfirm,
        None,
        &[ShellOp::ConfirmDelete, ShellOp::CancelDelete],
        |_| {},
    )
}

fn full_items() -> Vec<Node> {
    vec![
        notice_el(
            NoticeKind::ExportDraft,
            None,
            &[ShellOp::DismissNotice],
            |_| {},
        ),
        confirm_item(),
        notice_el(
            NoticeKind::ImportRefused,
            Some("format_newer"),
            &[ShellOp::DismissNotice],
            |_| {},
        ),
    ]
}

fn full_page() -> Node {
    page(good_top(), vec![delete_button()], full_items())
}

#[test]
fn 版_2_の_2_分の正の見本は全ての検査器を通る() {
    let f = full_page();
    check_complete(&f).unwrap();
    check_complete_v2(&f).unwrap();
    check_no_undefined(&f).unwrap();
    a11y::check_no_undefined(&f).unwrap();
}

fn refused_with(mutate: impl Fn(&mut Vec<(&'static str, String)>)) -> Node {
    notice_el(
        NoticeKind::ImportRefused,
        Some("format_newer"),
        &[ShellOp::DismissNotice],
        mutate,
    )
}

fn set_attr(a: &mut [(&'static str, String)], name: &str, val: &str) {
    for (k, v) in a.iter_mut() {
        if *k == name {
            *v = val.to_owned();
        }
    }
}

#[test]
fn 理由の束縛と次の行動の負の見本() {
    let bad_reason = refused_with(|a| set_attr(a, NOTICE_REASON_ATTR, "x"));
    assert!(rules(check_no_undefined(&page(
        good_top(),
        vec![],
        vec![bad_reason]
    )))
    .contains(&"reason-unbound"));
    let missing = refused_with(|a| a.retain(|(k, _)| *k != NOTICE_NEXT_ACTION_ATTR));
    assert!(
        rules(check_no_undefined(&page(good_top(), vec![], vec![missing])))
            .contains(&"next-action-missing")
    );
    let want = next_action(LoadReason::parse("format_newer").unwrap()).as_str();
    for val in [
        "start_fresh",
        "choose_another_file",
        "open_with_source_version",
        "nope",
        "",
    ] {
        let item = refused_with(|a| set_attr(a, NOTICE_NEXT_ACTION_ATTR, val));
        let res = check_no_undefined(&page(good_top(), vec![], vec![item]));
        if val == want {
            res.unwrap();
        } else {
            assert!(rules(res).contains(&"next-action-mismatch"), "{val}");
        }
    }
    let stray = notice_el(
        NoticeKind::CorruptHistory,
        Some("malformed"),
        &[ShellOp::DiscardAndStart],
        |a| a.push((NOTICE_NEXT_ACTION_ATTR, "start_fresh".to_owned())),
    );
    assert!(
        rules(check_no_undefined(&page(good_top(), vec![], vec![stray])))
            .contains(&"next-action-mismatch")
    );
}

#[test]
fn 削除の置き場の負の見本() {
    let mut t = good_top();
    t.push(delete_button());
    assert!(rules(check_no_undefined(&page(t, vec![], vec![]))).contains(&"delete-in-top"));
    for (id, name) in [
        ("shell-delete-confirm", "confirm-delete-history"),
        ("shell-delete-cancel", "cancel-delete-history"),
    ] {
        let b = el(
            "button",
            vec![(TESTID_ATTR, id), (ACTION_ATTR, name)],
            vec![],
        );
        assert!(
            rules(check_no_undefined(&page(good_top(), vec![b], vec![])))
                .contains(&"op-outside-notice"),
            "{id}"
        );
    }
    let mut item = notice_el(
        NoticeKind::ExportDraft,
        None,
        &[ShellOp::DismissNotice],
        |_| {},
    );
    if let Node::Element { children, .. } = &mut item {
        children.push(delete_button());
    }
    assert!(
        rules(check_no_undefined(&page(good_top(), vec![], vec![item])))
            .contains(&"op-not-allowed")
    );
    let bad_payload = el(
        "button",
        vec![
            (TESTID_ATTR, "shell-delete"),
            (ACTION_ATTR, "delete-history"),
            (PAYLOAD_ATTR, "x"),
        ],
        vec![],
    );
    assert!(rules(check_no_undefined(&page(
        good_top(),
        vec![bad_payload],
        vec![]
    )))
    .contains(&"op-action"));
    let undefined = tid("shell-reload", vec![]);
    assert!(rules(check_no_undefined(&page(
        good_top(),
        vec![undefined],
        vec![]
    )))
    .contains(&"undefined-testid"));
}

#[test]
fn 確認に確定か取り消しが欠けると版_2_だけが失敗する() {
    for keep in [ShellOp::ConfirmDelete, ShellOp::CancelDelete] {
        let n = Notice::new(
            NoticeKind::DeleteConfirm,
            None,
            MessageId::DeleteConfirm,
            &[keep],
        )
        .unwrap();
        let attrs = n.attrs();
        let refs: Vec<(&str, &str)> = attrs.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let item = el(
            "div",
            refs,
            vec![el(
                "button",
                vec![(TESTID_ATTR, keep.testid()), (ACTION_ATTR, keep.as_str())],
                vec![],
            )],
        );
        let f = page(good_top(), vec![], vec![item]);
        check_no_undefined(&f).unwrap();
        check_complete(&f).unwrap();
        assert_eq!(rules(check_complete_v2(&f)), ["delete-confirm-ops"]);
    }
}
