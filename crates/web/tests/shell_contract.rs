//! 枠の区画と操作の名前の契約（`shell`、TASK-511 1/2）の適合試験。
//! 値の固定・`a11y` 等との識別子の非重複・action の名前の非衝突・decode と encode の往復を確かめる。
//! 試験では `unwrap` を使ってよい（失敗はそのまま試験の失敗になる）。

use fandhe_workshop_web::a11y::{
    LiveRegion, PaneTab, StatusNotice, ARIA_LABEL_VALUES, EXPORT_NOTICE_TESTID, LOG_LIST_TESTID,
    MARK_GLYPH_TESTID, MARK_TEXT_TESTID, SAVE_NOTICE_TESTID, SECRET_NOTICE_TESTID,
    STALE_NOTICE_TESTID, TABLIST_TESTID, TAB_TESTID, VIEW_ONLY_NOTICE_TESTID,
};
use fandhe_workshop_web::shell::*;
use fandhe_workshop_web::terminal_area::{InputPort, TERMINAL_AREA_TESTIDS};
use fandhe_workshop_web::{action, offers, save_notice, secret_notice, view_select};
use std::collections::HashSet;

#[test]
fn 版_1_の値は版_2_でも同じ値と順で残る() {
    // 版の値そのものは shell_contract_v2 が固定する。
    const { assert!(SHELL_CONTRACT_VERSION >= 1) };
    let kinds: Vec<_> = NoticeKind::ALL.iter().take(5).map(|k| k.as_str()).collect();
    assert_eq!(
        kinds,
        [
            "save-failed",
            "corrupt-history",
            "import-refused",
            "version-converted",
            "pack-unloadable"
        ]
    );
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
        ops[..4],
        [
            "shell-export",
            "shell-import",
            "shell-discard-start",
            "shell-dismiss"
        ]
    );
    let names: Vec<_> = ShellOp::ALL.iter().map(|o| o.as_str()).collect();
    assert_eq!(
        names[..4],
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
    // 版 2 で Title・Position・Mock は確定し、Mock だけが必須になった（shell_contract_v2 が詳細を見る）。
    for s in [TopSection::Title, TopSection::Position, TopSection::Mock] {
        assert_eq!(s.status(), SectionStatus::Fixed);
    }
    for s in TopSection::ALL {
        let want = if s == TopSection::Mock {
            Cardinality::ExactlyOne
        } else {
            Cardinality::AtMostOne
        };
        assert_eq!(s.cardinality(), want);
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
    assert_eq!(ids.len(), 6 + 2 + 1 + 1 + 3 + 7);
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
        VIEW_ONLY_NOTICE_TESTID,
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
fn 通知の識別子と種類の値の固定() {
    assert_eq!(NOTICE_AREA_TESTID, "shell-notice");
    assert_eq!(NOTICE_ITEM_TESTID, "shell-notice-item");
    assert_eq!(NOTICE_TEXT_TESTID, "shell-notice-text");
    assert_eq!(NOTICE_KIND_ATTR, "data-notice-kind");
    assert_eq!(NOTICE_REASON_ATTR, "data-notice-reason");
    assert_eq!(NOTICE_MESSAGE_ATTR, "data-notice-message");
    let kinds: Vec<_> = NoticeKind::ALL.iter().map(|k| k.as_str()).collect();
    assert_eq!(
        kinds[..6],
        [
            "save-failed",
            "corrupt-history",
            "import-refused",
            "version-converted",
            "pack-unloadable",
            "browser-notice"
        ]
    );
    let msgs: Vec<_> = MessageId::ALL.iter().map(|m| m.as_str()).collect();
    assert_eq!(
        msgs[..6],
        [
            "msg-save-failed",
            "msg-corrupt-history",
            "msg-import-refused",
            "msg-version-converted",
            "msg-pack-unloadable",
            "msg-browser-notice"
        ]
    );
}

#[test]
fn 通知の_parse_と_as_str_は往復し未知を拒否する() {
    for k in NoticeKind::ALL {
        assert_eq!(NoticeKind::parse(k.as_str()), Some(k));
    }
    for m in MessageId::ALL {
        assert_eq!(MessageId::parse(m.as_str()), Some(m));
    }
    for bad in ["", "Save-failed", "delete-history", "save-failed "] {
        assert_eq!(NoticeKind::parse(bad), None);
    }
    assert_eq!(MessageId::parse("save-failed"), None);
}

#[test]
fn 全ての種類の値は_dismiss_notice_で受理される() {
    for k in NoticeKind::ALL {
        assert_eq!(
            ShellAction::decode(ACTION_DISMISS_NOTICE, k.as_str()),
            Some(ShellAction::DismissNotice(k.as_str().to_owned()))
        );
    }
}

#[test]
fn 許す操作の表の固定() {
    assert_eq!(NoticeKind::SaveFailed.allowed_ops(), []);
    assert_eq!(NoticeKind::PackUnloadable.allowed_ops(), []);
    assert_eq!(
        NoticeKind::CorruptHistory.allowed_ops(),
        [ShellOp::DiscardAndStart]
    );
    assert_eq!(
        NoticeKind::ImportRefused.allowed_ops(),
        [ShellOp::DismissNotice]
    );
    assert_eq!(
        NoticeKind::VersionConverted.allowed_ops(),
        [ShellOp::DismissNotice]
    );
    assert_eq!(
        NoticeKind::BrowserNotice.allowed_ops(),
        [ShellOp::DismissNotice]
    );
    assert_eq!(
        NoticeKind::ExportDraft.allowed_ops(),
        [ShellOp::DismissNotice]
    );
    assert_eq!(
        NoticeKind::DeleteConfirm.allowed_ops(),
        [ShellOp::ConfirmDelete, ShellOp::CancelDelete]
    );
    for k in NoticeKind::ALL {
        assert!(!k.allowed_ops().contains(&ShellOp::Export));
        assert!(!k.allowed_ops().contains(&ShellOp::Import));
        assert_eq!(k.messages().len(), 1);
    }
}

#[test]
fn 理由の規則() {
    for k in NoticeKind::ALL {
        let want = if matches!(
            k,
            NoticeKind::VersionConverted
                | NoticeKind::BrowserNotice
                | NoticeKind::ExportDraft
                | NoticeKind::DeleteConfirm
        ) {
            ReasonRule::Optional
        } else {
            ReasonRule::Required
        };
        assert_eq!(k.reason_rule(), want, "{}", k.as_str());
    }
}

fn msg(k: NoticeKind) -> MessageId {
    k.messages()[0]
}

#[test]
fn notice_new_の_5_つのエラー() {
    let k = NoticeKind::ImportRefused;
    assert_eq!(
        Notice::new(k, None, msg(k), &[]),
        Err(NoticeError::ReasonRequired)
    );
    assert_eq!(
        Notice::new(k, Some("<b>"), msg(k), &[]),
        Err(NoticeError::ReasonFormat)
    );
    assert_eq!(
        Notice::new(k, Some("format_newer"), MessageId::SaveFailed, &[]),
        Err(NoticeError::MessageNotAllowed(MessageId::SaveFailed))
    );
    assert_eq!(
        Notice::new(k, Some("format_newer"), msg(k), &[ShellOp::Export]),
        Err(NoticeError::OpNotAllowed(ShellOp::Export))
    );
    assert_eq!(
        Notice::new(
            k,
            Some("format_newer"),
            msg(k),
            &[ShellOp::DismissNotice, ShellOp::DismissNotice]
        ),
        Err(NoticeError::DuplicateOp(ShellOp::DismissNotice))
    );
    let ok = Notice::new(k, Some("format_newer"), msg(k), &[ShellOp::DismissNotice]).unwrap();
    assert_eq!(ok.kind(), k);
    assert_eq!(ok.reason().map(|r| r.as_str()), Some("format_newer"));
    assert_eq!(ok.message(), msg(k));
    assert_eq!(ok.ops(), [ShellOp::DismissNotice]);
    let v = NoticeKind::VersionConverted;
    assert!(Notice::new(v, None, msg(v), &[]).is_ok());
}

#[test]
fn 理由コードの正規形() {
    for ok in [
        "a",
        "missing_version",
        "format_newer",
        "QuotaExceededError",
        "a-b_1",
    ] {
        assert!(ReasonCode::parse(ok).is_some(), "{ok}");
    }
    assert!(ReasonCode::parse(&"a".repeat(64)).is_some());
    for bad in ["", "1a", "_a", "a b", "a<b", "あ", "a\n", "a.b"] {
        assert!(ReasonCode::parse(bad).is_none(), "{bad:?}");
    }
    assert!(ReasonCode::parse(&"a".repeat(65)).is_none());
}

#[test]
fn 理由コードは_task_310_の列挙の値を結ばずに受ける() {
    for r in fandhe_workshop_contract::load_reason::LoadReason::ALL {
        assert!(ReasonCode::parse(r.as_str()).is_some(), "{}", r.as_str());
    }
}

#[test]
fn notice_の属性と宛先() {
    let k = NoticeKind::CorruptHistory;
    let n = Notice::new(k, Some("malformed"), msg(k), &[ShellOp::DiscardAndStart]).unwrap();
    let attrs = n.attrs();
    assert!(attrs.contains(&("data-testid", NOTICE_ITEM_TESTID.to_owned())));
    assert!(attrs.contains(&(NOTICE_KIND_ATTR, "corrupt-history".to_owned())));
    assert!(attrs.contains(&(NOTICE_REASON_ATTR, "malformed".to_owned())));
    assert!(attrs.contains(&(NOTICE_MESSAGE_ATTR, "msg-corrupt-history".to_owned())));
    assert_eq!(n.actions(), [ShellAction::DiscardAndStart]);
    let k = NoticeKind::ImportRefused;
    let n = Notice::new(k, Some("format_newer"), msg(k), &[ShellOp::DismissNotice]).unwrap();
    assert_eq!(
        n.actions(),
        [ShellAction::DismissNotice("import-refused".to_owned())]
    );
}

#[test]
fn 検討中の通知の項目は版_1_に無い() {
    for name in ["next-action", "reload", "delete-history", "stale-tab"] {
        assert_eq!(NoticeKind::parse(name), None);
    }
    for id in MessageId::ALL {
        assert!(!id.as_str().contains("next-action"));
    }
    assert_eq!(NoticeKind::ALL.len(), 8);
}

#[test]
fn 検討中の項目は版_1_に無い() {
    for name in ["reload-notice", "fast-forward"] {
        assert_eq!(ShellOp::from_name(name), None);
    }
    assert!(!is_defined_testid("shell-reload"));
}
