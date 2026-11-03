//! `view_kind` の契約（TASK-796、D-97）。7 種の受理と、誤りの分類を固定する。

use fandhe_workshop_contract::view_kind::{read_view_kind, ViewKindReadError, VIEW_KINDS};
use serde_json::json;

#[test]
fn all_seven_kinds_are_accepted() {
    assert_eq!(VIEW_KINDS.len(), 7);
    for k in VIEW_KINDS {
        assert_eq!(read_view_kind(&json!({ "kind": k })), Ok(k));
    }
}

#[test]
fn near_misses_are_unknown() {
    for s in [
        "task-tree",
        "cardlist",
        "File_Tree",
        "document ",
        " document",
        "",
        "files",
    ] {
        assert_eq!(
            read_view_kind(&json!({ "kind": s })),
            Err(ViewKindReadError::Unknown),
            "{s:?}"
        );
    }
}

#[test]
fn non_strings_and_missing_are_classified() {
    for v in [json!(1), json!(null), json!(true), json!([]), json!({})] {
        assert_eq!(
            read_view_kind(&json!({ "kind": v })),
            Err(ViewKindReadError::NotAString)
        );
    }
    assert_eq!(
        read_view_kind(&json!({ "body": "shown" })),
        Err(ViewKindReadError::Missing)
    );
    assert_eq!(
        read_view_kind(&json!("document")),
        Err(ViewKindReadError::Missing)
    );
}
