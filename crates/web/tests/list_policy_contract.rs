//! keyed list の親の属性と項目数の方針（`list_policy`、TASK-343 3/3）の契約適合試験（FEND-15・FEND-22・D-46）。
//! ダミーの部品は試験の側にだけ置き、配信物には入れない。

use fandhe_frontend_core::{el, render, text, Node};
use fandhe_workshop_web::list_policy::{
    classify_list_len, keyed_view, ListError, ListSize, LIST_ITEMS_LIMIT, LIST_ITEMS_TARGET,
};
use fandhe_workshop_web::parts::{DisplayPart, FieldKind, PartField, PartKind, PartRegistry};

const LIST_FIELD: PartField = PartField {
    name: "rows",
    kind: FieldKind::KeyedList,
};
const BOUND_FIELD: PartField = PartField {
    name: "label",
    kind: FieldKind::Bound,
};

fn items(n: usize) -> Vec<(String, Node)> {
    (0..n)
        .map(|i| (format!("k{i}"), el("li", vec![], vec![text("x")])))
        .collect()
}

#[test]
fn 目安と安全弁の値() {
    assert_eq!(LIST_ITEMS_TARGET, 1_000);
    // 上流の上限が変わったら、方針を見直すために失敗させる。
    assert_eq!(LIST_ITEMS_LIMIT, 4_096);
}

#[test]
fn 項目数の区分の境界() {
    assert_eq!(classify_list_len(0), ListSize::Within);
    assert_eq!(classify_list_len(1_000), ListSize::Within);
    assert_eq!(classify_list_len(1_001), ListSize::OverTarget);
    assert_eq!(classify_list_len(4_096), ListSize::OverTarget);
    assert_eq!(classify_list_len(4_097), ListSize::OverLimit);
}

#[test]
fn 安全弁ちょうどは通り_超えると上流の拒否を返す() {
    assert!(keyed_view("ul", vec![], &LIST_FIELD, items(4_096)).is_ok());
    let err = keyed_view("ul", vec![], &LIST_FIELD, items(4_097)).unwrap_err();
    assert!(matches!(err, ListError::Upstream(_)));
}

#[test]
fn keyed_list_でない_field_は拒否する() {
    let err = keyed_view("ul", vec![], &BOUND_FIELD, items(1)).unwrap_err();
    assert_eq!(err, ListError::NotKeyedField);
}

#[test]
fn 親の属性は描画結果の親要素に出る() {
    let node = keyed_view(
        "ul",
        vec![("aria-live", "polite"), ("data-testid", "log-rows")],
        &LIST_FIELD,
        items(2),
    )
    .unwrap();
    let html = render(&node);
    assert!(html.starts_with("<ul "), "{html}");
    assert!(html.contains("aria-live=\"polite\""), "{html}");
    assert!(html.contains("data-testid=\"log-rows\""), "{html}");
    assert!(html.contains("data-bind-list=\"rows\""), "{html}");
}

#[test]
fn 予約属性を混ぜると握りつぶさず拒否する() {
    let err = keyed_view(
        "ul",
        vec![("data-bind-list", "other")],
        &LIST_FIELD,
        items(1),
    )
    .unwrap_err();
    assert!(matches!(err, ListError::Upstream(_)));
}

#[test]
fn 拒否の文面にキーを含めない() {
    let mut list = items(1);
    list.push(("secret-key-value".to_owned(), el("li", vec![], vec![])));
    list.push(("secret-key-value".to_owned(), el("li", vec![], vec![])));
    let err = keyed_view("ul", vec![], &LIST_FIELD, list).unwrap_err();
    let msg = err.to_string();
    assert!(!msg.contains("secret-key-value"), "{msg}");
    assert!(!ListError::NotKeyedField.to_string().contains("rows"));
}

struct Dummy;

const FIELDS: &[PartField] = &[LIST_FIELD];

impl DisplayPart<()> for Dummy {
    fn kind(&self) -> PartKind {
        PartKind::Log
    }
    fn fields(&self) -> &'static [PartField] {
        FIELDS
    }
    fn view(&self, _model: &()) -> Node {
        match keyed_view("ul", vec![("aria-live", "polite")], &FIELDS[0], items(3)) {
            Ok(n) => n,
            Err(_) => text(""),
        }
    }
}

#[test]
fn 登録した部品の_keyed_list_の_field_が_dirty_の検査を通る() {
    let mut reg: PartRegistry<()> = PartRegistry::new();
    reg.register(Box::new(Dummy)).unwrap();
    assert!(reg.check_dirty(&["rows"]).is_ok());
    let html = render(&reg.view(PartKind::Log, &()).unwrap());
    assert!(html.contains("aria-live=\"polite\""), "{html}");
}
