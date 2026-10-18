//! `pack.left_pane` の閉じた集合と読み取りの契約試験（TASK-53 2/2、PACK-22）。

use fandhe_workshop_contract::left_pane::{
    read_left_pane, LeftPane, LeftPaneReadError, ViewTabs, WorldList,
};
use serde_json::json;

#[test]
fn absent_and_empty_are_default() {
    let d = LeftPane::default();
    assert_eq!(d.world_list, WorldList::Shown);
    assert_eq!(d.view_tabs, ViewTabs::All);
    assert!(d.is_default());
    assert_eq!(read_left_pane(None), Ok(d));
    assert_eq!(read_left_pane(Some(&json!({}))), Ok(d));
}

#[test]
fn every_combination_is_read() {
    for w in WorldList::ALL {
        for t in ViewTabs::ALL {
            let v = json!({"world_list": w.as_str(), "view_tabs": t.as_str()});
            let lp = read_left_pane(Some(&v)).expect("read");
            assert_eq!((lp.world_list, lp.view_tabs), (w, t));
            assert_eq!(lp.is_default(), w == WorldList::Shown && t == ViewTabs::All);
        }
    }
}

#[test]
fn single_field_keeps_the_other_default() {
    let lp = read_left_pane(Some(&json!({"view_tabs": "hidden"}))).unwrap();
    assert_eq!(lp.world_list, WorldList::Shown);
    assert_eq!(lp.view_tabs, ViewTabs::Hidden);
}

#[test]
fn as_str_and_from_vocab_round_trip() {
    for w in WorldList::ALL {
        assert_eq!(WorldList::from_vocab(w.as_str()), Some(w));
    }
    for t in ViewTabs::ALL {
        assert_eq!(ViewTabs::from_vocab(t.as_str()), Some(t));
    }
}

#[test]
fn predicates_follow_the_vocabulary() {
    assert!(WorldList::Shown.is_shown() && !WorldList::Hidden.is_shown());
    assert!(ViewTabs::All.shows_buttons() && ViewTabs::Declared.shows_buttons());
    assert!(!ViewTabs::Hidden.shows_buttons());
    assert!(ViewTabs::Declared.limits_to_declared());
    assert!(!ViewTabs::All.limits_to_declared() && !ViewTabs::Hidden.limits_to_declared());
}

#[test]
fn unknown_values_are_rejected() {
    for s in ["x", "Hidden", "ALL", "", "show"] {
        assert_eq!(
            read_left_pane(Some(&json!({"view_tabs": s}))),
            Err(LeftPaneReadError::ViewTabsUnknown),
            "view_tabs {s:?}"
        );
        assert_eq!(
            read_left_pane(Some(&json!({"world_list": s}))),
            Err(LeftPaneReadError::WorldListUnknown),
            "world_list {s:?}"
        );
    }
    // 他方の語彙の値は通さない。
    assert_eq!(
        read_left_pane(Some(&json!({"world_list": "all"}))),
        Err(LeftPaneReadError::WorldListUnknown)
    );
}

#[test]
fn non_string_values_are_rejected() {
    for v in [json!(null), json!(5), json!(["all"]), json!(true)] {
        assert_eq!(
            read_left_pane(Some(&json!({ "view_tabs": v }))),
            Err(LeftPaneReadError::ViewTabsNotString)
        );
        assert_eq!(
            read_left_pane(Some(&json!({ "world_list": v }))),
            Err(LeftPaneReadError::WorldListNotString)
        );
    }
}

#[test]
fn unknown_keys_and_non_mappings_are_rejected() {
    for k in ["tabs", "style", "View_tabs"] {
        assert_eq!(
            read_left_pane(Some(&json!({ k: "all" }))),
            Err(LeftPaneReadError::UnknownKey(k.to_owned())),
            "{k}"
        );
    }
    for v in [json!(null), json!([1]), json!("all"), json!(3)] {
        assert_eq!(
            read_left_pane(Some(&v)),
            Err(LeftPaneReadError::NotAMapping)
        );
    }
}
