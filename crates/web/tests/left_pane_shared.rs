//! web と pack が同じ `left_pane` の型・綴りを使うことの試験（TASK-53 2/2、PACK-22）。

use fandhe_workshop_contract::left_pane as contract;
use fandhe_workshop_web::profile::{DisplaySettings, LeftPane, ViewTabs, WorldList};

#[test]
fn web_reexports_the_contract_types() {
    // 型が同一でなければこの代入は通らない。
    let lp: contract::LeftPane = LeftPane::default();
    let tabs: contract::ViewTabs = ViewTabs::Declared;
    let list: contract::WorldList = WorldList::Hidden;
    assert!(lp.is_default());
    assert_eq!((tabs.as_str(), list.as_str()), ("declared", "hidden"));
}

#[test]
fn read_result_goes_into_display_settings() {
    for w in WorldList::ALL {
        for t in ViewTabs::ALL {
            let v = serde_json::json!({"world_list": w.as_str(), "view_tabs": t.as_str()});
            let lp = contract::read_left_pane(Some(&v)).expect("read");
            let settings = DisplaySettings {
                left_pane: lp,
                ..Default::default()
            };
            assert_eq!(settings.left_pane.world_list, w);
            assert_eq!(settings.left_pane.view_tabs, t);
        }
    }
}
