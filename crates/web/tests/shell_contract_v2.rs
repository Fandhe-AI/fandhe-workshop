//! 枠の契約の版 2（`shell`、TASK-516 1/2。SHELL-6・8・24・30。D-72・D-54・D-74）の適合試験とサンプル。
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
        ops,
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
    for id in ["shell-delete", "shell-reload"] {
        assert!(!is_defined_testid(id));
    }
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
