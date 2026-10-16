//! 左の成果物ビューの契約適合試験の続き（TASK-467 2/2。ART-1・ART-5・ART-7・ART-8・ART-11・ART-12・FEND-13・SAFE-6）。
//!
//! 束縛点の名前・親要素の属性・指せる行の属性と payload・変種の口を native で確かめる。
//! 1/2 の試験（`artifact_view_contract.rs`）と同じ契約の版 1 の中にある。

use fandhe_frontend_core::{el, render, text, Node};
use fandhe_workshop_core::input::Input;
use fandhe_workshop_web::a11y::{check_no_undefined, TESTID_ATTR};
use fandhe_workshop_web::artifact_view::*;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::HostModel;
use fandhe_workshop_web::list_policy::ListError;
use fandhe_workshop_web::parts::{DisplayPart, FieldKind, PartField, PartKind, PartRegistry};

fn rid(s: &str) -> RowId {
    RowId::new(s).unwrap()
}

fn items(n: usize) -> Vec<(String, Node)> {
    (0..n)
        .map(|i| (format!("k{i}"), el("li", vec![], vec![text("x")])))
        .collect()
}

struct Seven;

impl DisplayPart<HostModel> for Seven {
    fn kind(&self) -> PartKind {
        PartKind::ArtifactView
    }
    fn fields(&self) -> &'static [PartField] {
        &VIEW_LIST_FIELDS
    }
    fn view(&self, _model: &HostModel) -> Node {
        el("section", vec![], vec![])
    }
}

#[test]
fn 契約の版は_1_のまま() {
    assert_eq!(ARTIFACT_VIEW_CONTRACT_VERSION, 1);
}

#[test]
fn 束縛点の名前の表と順() {
    let table = [
        (ViewKind::FileTree, "view_file_tree"),
        (ViewKind::Document, "view_document"),
        (ViewKind::Diff, "view_diff"),
        (ViewKind::TaskTree, "view_task_tree"),
        (ViewKind::CardList, "view_card_list"),
        (ViewKind::CheckList, "view_check_list"),
        (ViewKind::AppDemo, "view_app_demo"),
    ];
    assert_eq!(VIEW_LIST_FIELDS.len(), ViewKind::ALL.len());
    for (i, (k, name)) in table.iter().enumerate() {
        assert_eq!(ViewKind::ALL[i], *k);
        assert_eq!(k.list_field(), *name);
        assert_eq!(VIEW_LIST_FIELDS[i].name, *name);
        assert_eq!(VIEW_LIST_FIELDS[i].kind, FieldKind::KeyedList);
        assert_eq!(k.list_part_field(), VIEW_LIST_FIELDS[i]);
        for other in &table[i + 1..] {
            assert_ne!(name, &other.1);
        }
    }
}

#[test]
fn 束縛点は登録でき_dirty_を受け_標準の部品の_field_と重ならない() {
    let mut r: PartRegistry<HostModel> = PartRegistry::new();
    r.register(Box::new(Seven)).unwrap();
    let names: Vec<&'static str> = VIEW_LIST_FIELDS.iter().map(|f| f.name).collect();
    assert!(r.check_dirty(&names).is_ok());
    let std_reg = standard_registry();
    for f in std_reg.fields() {
        assert!(!names.contains(&f.name), "{}", f.name);
    }
    // 標準の登録簿へ重ねると、種類の重複で拒否される。
    let mut std_reg = standard_registry();
    assert!(std_reg.register(Box::new(Seven)).is_err());
}

#[test]
fn 親の属性の表() {
    let on = list_attrs(ViewKind::Diff, true);
    assert_eq!(
        on,
        vec![(TESTID_ATTR, VIEW_LIST_TESTID), (VIEW_ATTR, "diff")]
    );
    let off = list_attrs(ViewKind::Diff, false);
    assert_eq!(off.len(), 3);
    assert_eq!(off[2], (HIDDEN_ATTR, ""));
    for attrs in [on, off] {
        for (name, _) in attrs {
            assert!(name != "role" && !name.starts_with("aria-"), "{name}");
        }
    }
}

#[test]
fn view_list_の選択と件数() {
    let sel = view_list("ul", ViewKind::CardList, true, items(3)).unwrap();
    let html = render(&sel);
    assert!(html.contains("data-view=\"card_list\""), "{html}");
    assert!(!html.contains("hidden"), "{html}");
    assert_eq!(html.matches("<li").count(), 3);
    assert!(check_no_undefined(&sel).is_ok());

    let off = view_list("ul", ViewKind::CardList, false, items(3)).unwrap();
    let html = render(&off);
    assert!(html.contains("hidden"), "{html}");
    assert_eq!(html.matches("<li").count(), 0);
    assert!(check_no_undefined(&off).is_ok());
}

#[test]
fn view_list_は安全弁を超えると_err() {
    let err = view_list("ul", ViewKind::Diff, true, items(4_097)).unwrap_err();
    assert!(matches!(err, ListError::Upstream(_)));
    let dup = vec![
        ("k".to_owned(), el("li", vec![], vec![])),
        ("k".to_owned(), el("li", vec![], vec![])),
    ];
    assert!(view_list("ul", ViewKind::Diff, true, dup).is_err());
}

#[test]
fn dirty_list_fields_の場合分け() {
    use ViewKind::{Diff, FileTree};
    assert_eq!(
        dirty_list_fields(Some(FileTree), Some(Diff), false),
        vec!["view_file_tree", "view_diff"]
    );
    assert_eq!(
        dirty_list_fields(Some(Diff), Some(Diff), true),
        vec!["view_diff"]
    );
    assert!(dirty_list_fields(Some(Diff), Some(Diff), false).is_empty());
    assert!(dirty_list_fields(None, None, true).is_empty());
    assert_eq!(
        dirty_list_fields(None, Some(Diff), false),
        vec!["view_diff"]
    );
    assert_eq!(
        dirty_list_fields(Some(Diff), None, false),
        vec!["view_diff"]
    );
}

fn target(view: &str, row: &str) -> PointTarget {
    PointTarget::new(view, rid(row)).unwrap()
}

#[test]
fn payload_は往復する() {
    for (v, r) in [
        ("view-a", "row-1"),
        ("a|b", "c|d"),
        ("a:b", "1:2"),
        ("v w", " r "),
        ("表示", "行あ"),
        ("<img src=x onerror=alert(1)>", "\"><script>"),
        ("1", "2"),
    ] {
        let t = target(v, r);
        let back = PointTarget::decode(POINT_ACTION, &t.encode_payload()).unwrap();
        assert_eq!(back, t, "{v:?} {r:?}");
        assert_eq!(back.view(), v);
        assert_eq!(back.row().as_str(), r);
    }
}

#[test]
fn 宛先の作成は空と上限超えを拒否する() {
    assert_eq!(
        PointTarget::new("", rid("r")).unwrap_err(),
        ArtifactViewError::EmptyViewId
    );
    let long = "x".repeat(MAX_POINT_PAYLOAD_BYTES);
    assert_eq!(
        PointTarget::new("v", rid(&long)).unwrap_err(),
        ArtifactViewError::PayloadTooLong
    );
    assert!(PointTarget::new("v", rid(&"x".repeat(100))).is_ok());
}

#[test]
fn 壊れた_payload_は_none() {
    let over = format!("1:a{}", "b".repeat(MAX_POINT_PAYLOAD_BYTES));
    let cases: Vec<String> = vec![
        "".into(),
        "abc".into(),
        "99:x".into(),
        "1:".into(),
        "0:x".into(),
        "01:ab".into(),
        "+1:ab".into(),
        "-1:ab".into(),
        "99999999999999999999:ab".into(),
        "１:ab".into(),
        "{\"view\":\"a\"}".into(),
        ":ab".into(),
        "1 :ab".into(),
        "2:あい".into(),
        "1:あ".into(),
        "4:あい".into(),
        over,
    ];
    for c in &cases {
        assert_eq!(PointTarget::decode(POINT_ACTION, c), None, "{c:?}");
    }
    for name in ["raw", "__proto__", "", "UI", "ui "] {
        assert_eq!(PointTarget::decode(name, "1:ab"), None, "{name:?}");
    }
    assert!(PointTarget::decode(POINT_ACTION, "1:ab").is_some());
}

#[test]
fn 入力へ変換できる() {
    let t = target("v", "p/q");
    assert_eq!(
        t.to_input(),
        Input::Ui {
            view: "v".to_owned(),
            path: "p/q".to_owned()
        }
    );
}

#[test]
fn 属性の組は_4_つで順が固定_値はエスケープされる() {
    let t = target("v\"<i>", "r'&<b>");
    let a = t.attrs();
    let pairs = a.pairs();
    let names: Vec<&str> = pairs.iter().map(|(n, _)| *n).collect();
    assert_eq!(
        names,
        ["data-action", "data-payload", "data-row", "tabindex"]
    );
    assert_eq!(pairs[0].1, "ui");
    assert_eq!(pairs[3].1, "0");
    let node = el("li", pairs.to_vec(), vec![text("x")]);
    let html = render(&node);
    assert!(!html.contains("<b>") && !html.contains("<i>"), "{html}");
    assert!(!html.contains("v\"<"), "{html}");
}

#[test]
fn 変種の許可の表() {
    for k in ViewKind::ALL {
        assert!(k.allows(Variant::B), "{k:?}");
        let a_ok = !matches!(k, ViewKind::Document | ViewKind::Diff);
        assert_eq!(k.allows(Variant::A), a_ok, "{k:?}");
    }
    assert_eq!(Variant::DEFAULT, Variant::B);
    assert_eq!(Variant::A.as_str(), "a");
    assert_eq!(Variant::B.as_str(), "b");
}

#[test]
fn 変種の方針は許されない組と重複を拒否する() {
    for k in [ViewKind::Document, ViewKind::Diff] {
        assert_eq!(
            VariantPolicy::new(&[(k, Variant::A)]).unwrap_err(),
            ArtifactViewError::VariantNotAllowed
        );
    }
    assert!(VariantPolicy::new(&[
        (ViewKind::CardList, Variant::A),
        (ViewKind::CardList, Variant::B)
    ])
    .is_err());
    let p = VariantPolicy::new(&[(ViewKind::AppDemo, Variant::A)]).unwrap();
    assert_eq!(p.variant_of(ViewKind::AppDemo), Variant::A);
    assert_eq!(p.variant_of(ViewKind::Document), Variant::B);
    assert_eq!(
        VariantPolicy::default().variant_of(ViewKind::FileTree),
        Variant::B
    );
}

#[test]
fn 追加した誤りの文面は固定で入力値を含まない() {
    for (e, s) in [
        (ArtifactViewError::EmptyViewId, "empty view id"),
        (ArtifactViewError::PayloadTooLong, "point payload too long"),
        (
            ArtifactViewError::VariantNotAllowed,
            "variant not allowed for view kind",
        ),
        (ArtifactViewError::DuplicateViewKind, "duplicate view kind"),
    ] {
        assert_eq!(e.to_string(), s);
    }
}
