//! 枠の区画と操作の名前の契約（`shell`、TASK-511 1/2）の適合試験。
//! 値の固定・`a11y` 等との識別子の非重複・action の名前の非衝突・decode と encode の往復を確かめる。
//! 試験では `unwrap` を使ってよい（失敗はそのまま試験の失敗になる）。

use fandhe_workshop_web::a11y::{
    LiveRegion, PaneTab, StatusNotice, ARIA_LABEL_VALUES, EXPORT_NOTICE_TESTID, LOG_LIST_TESTID,
    MARK_GLYPH_TESTID, MARK_TEXT_TESTID, SAVE_NOTICE_TESTID, SECRET_NOTICE_TESTID,
    STALE_NOTICE_TESTID, TABLIST_TESTID, TAB_TESTID,
};
use fandhe_workshop_web::shell::*;
use fandhe_workshop_web::terminal_area::{InputPort, TERMINAL_AREA_TESTIDS};
use fandhe_workshop_web::{action, offers, save_notice, secret_notice, view_select};
use std::collections::HashSet;

#[test]
fn 版は_1() {
    assert_eq!(SHELL_CONTRACT_VERSION, 1);
}

#[test]
fn 識別子と名前の値の固定() {
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
    let mounts: Vec<_> = PaneMount::ALL.iter().map(|m| m.testid()).collect();
    assert_eq!(mounts, ["shell-left", "shell-right"]);
    assert_eq!(NEXT_CHAPTER_TESTID, "shell-next-chapter");
    let ops: Vec<_> = ShellOp::ALL.iter().map(|o| o.testid()).collect();
    assert_eq!(
        ops,
        [
            "shell-export",
            "shell-import",
            "shell-discard-start",
            "shell-dismiss"
        ]
    );
    let names: Vec<_> = ShellOp::ALL.iter().map(|o| o.as_str()).collect();
    assert_eq!(
        names,
        [
            "export-history",
            "import-history",
            "discard-and-start",
            "dismiss-notice"
        ]
    );
    assert_eq!(SHELL_PREFIX, "shell-");
    assert_eq!(NEXT_CHAPTER_LABEL_PREFIX, "次の章へ進む: ");
}

#[test]
fn 区画の確定と予約と個数の規則() {
    for s in [TopSection::Top, TopSection::Hint, TopSection::Checkpoint] {
        assert_eq!(s.status(), SectionStatus::Fixed);
    }
    for s in [TopSection::Title, TopSection::Position, TopSection::Mock] {
        assert_eq!(s.status(), SectionStatus::Reserved);
    }
    for s in TopSection::ALL {
        assert_eq!(s.cardinality(), Cardinality::AtMostOne);
    }
    for m in PaneMount::ALL {
        assert_eq!(m.cardinality(), Cardinality::ExactlyOne);
    }
}

#[test]
fn parse_と_from_name_は往復し未知を拒否する() {
    for s in TopSection::ALL {
        assert_eq!(TopSection::parse(s.testid()), Some(s));
    }
    for m in PaneMount::ALL {
        assert_eq!(PaneMount::parse(m.testid()), Some(m));
    }
    for o in ShellOp::ALL {
        assert_eq!(ShellOp::from_name(o.as_str()), Some(o));
    }
    for bad in ["", "Shell-top", "shell-top ", " shell-top", "shell-"] {
        assert_eq!(TopSection::parse(bad), None);
        assert_eq!(PaneMount::parse(bad), None);
    }
    for bad in ["", "Export-History", "export-history ", " export-history"] {
        assert_eq!(ShellOp::from_name(bad), None);
    }
}

#[test]
fn 本契約の識別子は_shell_で始まり互いに重複しない() {
    let ids: Vec<_> = shell_testids().collect();
    assert_eq!(ids.len(), 6 + 2 + 1 + 4);
    assert!(ids.iter().all(|i| i.starts_with(SHELL_PREFIX)));
    let set: HashSet<_> = ids.iter().collect();
    assert_eq!(set.len(), ids.len());
    assert!(ids.iter().all(|i| is_defined_testid(i)));
}

#[test]
fn a11y_と_terminal_area_の識別子と重複しない() {
    let mut other: Vec<&str> = Vec::new();
    other.extend(LiveRegion::ALL.iter().map(|l| l.testid()));
    other.extend(StatusNotice::ALL.iter().map(|n| n.testid()));
    other.extend([
        SAVE_NOTICE_TESTID,
        EXPORT_NOTICE_TESTID,
        SECRET_NOTICE_TESTID,
        STALE_NOTICE_TESTID,
        LOG_LIST_TESTID,
        TABLIST_TESTID,
        TAB_TESTID,
        MARK_GLYPH_TESTID,
        MARK_TEXT_TESTID,
    ]);
    for t in PaneTab::ALL {
        other.extend([t.tab_id(), t.panel_id(), t.id()]);
    }
    other.extend(TERMINAL_AREA_TESTIDS);
    other.extend(InputPort::ALL.iter().map(|p| p.testid()));
    other.extend(ARIA_LABEL_VALUES);
    assert!(!other.is_empty());
    for id in shell_testids() {
        assert!(!other.contains(&id), "重複: {id}");
        assert!(!id.starts_with("live-") && !id.starts_with("line-"));
    }
}

#[test]
fn 取り付け点は_a11y_のタブへ結ばれ全単射() {
    assert_eq!(PaneMount::Left.pane_tab(), PaneTab::Artifacts);
    assert_eq!(PaneMount::Right.pane_tab(), PaneTab::Ops);
    let tabs: HashSet<_> = PaneMount::ALL.iter().map(|m| m.pane_tab().id()).collect();
    assert_eq!(tabs.len(), PaneTab::ALL.len());
    for t in PaneTab::ALL {
        assert!(PaneMount::ALL.iter().any(|m| m.pane_tab() == t));
    }
}

#[test]
fn 次の章へ進むの文字の形() {
    assert_eq!(next_chapter_label("題"), "次の章へ進む: 題");
    assert_eq!(next_chapter_label(""), "次の章へ進む: ");
}

#[test]
fn action_の名前は既存の名前と衝突しない() {
    let mut existing = vec![
        action::ACTION_TICK,
        action::ACTION_COMPLETE,
        action::ACTION_ESC,
        offers::ACTION_CHOOSE,
        offers::ACTION_DECIDE,
        offers::ACTION_REVISE_TARGET,
        view_select::ACTION_SELECT_VIEW,
        fandhe_workshop_web::artifact_view::POINT_ACTION,
        save_notice::ACTION_SAVE_RESULT,
        secret_notice::ACTION_SECRET_HOLD,
        secret_notice::ACTION_SECRET_KEEP,
        secret_notice::ACTION_SECRET_DISCARD,
        secret_notice::ACTION_SECRET_DISCARD_READ,
    ];
    existing.extend(["os", "draft", "submit", "stop", "raw", "keydown-probe"]);
    for o in ShellOp::ALL {
        assert!(!existing.contains(&o.as_str()), "衝突: {}", o.as_str());
    }
}

#[test]
fn decode_と_encode_は全種で往復する() {
    let all = [
        ShellAction::Export,
        ShellAction::Import("{\"a\":1}".to_owned()),
        ShellAction::Import(String::new()),
        ShellAction::DiscardAndStart,
        ShellAction::DismissNotice("save-failed-2".to_owned()),
    ];
    for a in all {
        let (name, payload) = a.encode();
        assert_eq!(ShellAction::decode(name, &payload), Some(a));
    }
}

#[test]
fn 書き出しと捨てて始めるは空でない_payload_を拒否する() {
    for name in [ACTION_EXPORT_HISTORY, ACTION_DISCARD_AND_START] {
        assert!(ShellAction::decode(name, "").is_some());
        for bad in [" ", "x", "\n"] {
            assert_eq!(ShellAction::decode(name, bad), None);
        }
    }
}

#[test]
fn 通知を閉じるは正規形だけを受ける() {
    let ok = |p: &str| ShellAction::decode(ACTION_DISMISS_NOTICE, p).is_some();
    assert!(ok("a"));
    assert!(ok("save-failed"));
    assert!(ok("a1-b2"));
    assert!(ok(&"a".repeat(32)));
    for bad in ["", "Save", "a b", "あ", "-a", "1a", "a_b", "a\n"] {
        assert!(!ok(bad), "受理された: {bad:?}");
    }
    assert!(!ok(&"a".repeat(33)));
}

#[test]
fn 読み込みは任意の本文を受け落ちない() {
    let big = "x".repeat(1024 * 1024);
    for body in ["", "not json", "\u{0}\u{1}\u{7f}", "日本語", big.as_str()] {
        assert_eq!(
            ShellAction::decode(ACTION_IMPORT_HISTORY, body),
            Some(ShellAction::Import(body.to_owned()))
        );
    }
}

#[test]
fn 未知の名前は拒否する() {
    assert_eq!(ShellAction::decode("", ""), None);
    assert_eq!(ShellAction::decode("export", ""), None);
}

#[test]
fn 読み込みの_debug_は本文を出さない() {
    let secret = "SECRET-BODY-MARKER";
    let dbg = format!("{:?}", ShellAction::Import(secret.to_owned()));
    assert!(!dbg.contains(secret));
    assert!(dbg.contains(&secret.len().to_string()));
}

#[test]
fn 検討中の項目は版_1_に無い() {
    for name in ["delete-history", "reload-notice", "fast-forward"] {
        assert_eq!(ShellOp::from_name(name), None);
    }
    for id in ["shell-delete", "shell-fast-forward", "shell-reload"] {
        assert!(!is_defined_testid(id));
    }
}
