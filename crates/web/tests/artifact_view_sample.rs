//! 左の成果物ビューの契約（`artifact_view`）を使うサンプルと、その試験（TASK-467 2/2。ART-5・ART-11・ART-12）。
//!
//! TASK-468〜TASK-481 のビューの担当が真似る手順は次のとおり。
//!
//! 1. 7 種のビューの keyed list（`VIEW_LIST_FIELDS`）を `PartKind::ArtifactView` の 1 部品に置く
//!    （`PartRegistry` は種類ごとに 1 部品だけを受け付ける）。
//! 2. 親要素は `view_list` で作る。親の属性は `view()` に書く（FEND-15）。選ばれていないビューは空で `hidden`。
//! 3. 行の変化の印は `Row::shown` と `mark_value` から導き、`data-mark`・`mark-glyph`・`mark-text` を付ける。
//! 4. 指せる行には `PointAttrs::pairs` の 4 つを組で付ける。指せない行には付けない。
//! 5. 変種は `VariantPolicy` を通して選ぶ（このサンプルは変種を描き分けない）。
//! 6. 一覧を作れなかったときは、空の一覧で黙って続けず、固定の通知の要素を返す。
//!
//! ダミーの部品とデータは試験の側にだけ置き、配信物には入れない。値はすべて汎用のダミーの文字列である。

use std::path::PathBuf;

use fandhe_frontend_core::{el, render, text, Node};
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_web::a11y::{
    check_no_undefined, MARK_ATTR, MARK_GLYPH_TESTID, MARK_TEXT_TESTID, TESTID_ATTR,
};
use fandhe_workshop_web::artifact_view::{
    mark_value, view_list, Cell, DocRow, ItemRow, Phase, PointAttrs, PointTarget, Row, RowId,
    RowKind, ViewKind, VIEW_LIST_FIELDS,
};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostModel};
use fandhe_workshop_web::parts::{DisplayPart, PartField, PartKind, PartRegistry};

/// 文書の本文のブロックの代わりのダミー。具体の型は後続のタスクが結ぶ。
#[derive(Debug, Clone, PartialEq, Eq)]
struct DummyBlock {
    body: String,
}

/// 行の元になる表。前後の中身は文字列で、同じなら変化なし。
struct Spec {
    id: &'static str,
    before: Option<&'static str>,
    after: Option<&'static str>,
    point: bool,
}

/// 追加・削除・変更・不変を各 1 件以上含む。1 行は危険な文字列を含み、描画のエスケープを確かめる。
const SPECS: [Spec; 5] = [
    Spec {
        id: "r-same",
        before: Some("same"),
        after: Some("same"),
        point: true,
    },
    Spec {
        id: "r-add",
        before: None,
        after: Some("new"),
        point: false,
    },
    Spec {
        id: "r-del",
        before: Some("old"),
        after: None,
        point: true,
    },
    Spec {
        id: "r-chg",
        before: Some("one"),
        after: Some("two"),
        point: false,
    },
    Spec {
        id: "<b>\"x\"&",
        before: Some("a"),
        after: Some("b"),
        point: true,
    },
];

fn view_id(kind: ViewKind) -> String {
    format!("sample-{}", kind.as_str())
}

fn build_rows<T>(conv: impl Fn(&str) -> T) -> Vec<Row<T>> {
    SPECS
        .iter()
        .map(|s| {
            Row::new(
                RowId::new(s.id).unwrap(),
                0,
                RowKind::Item,
                s.before.map(&conv),
                s.after.map(&conv),
            )
            .unwrap()
        })
        .collect()
}

fn item_rows() -> Vec<ItemRow> {
    build_rows(|s| Cell {
        label: s.to_owned(),
        ..Cell::default()
    })
}

fn doc_rows() -> Vec<DocRow<DummyBlock>> {
    build_rows(|s| DummyBlock { body: s.to_owned() })
}

fn row_node<T: PartialEq>(
    row: &Row<T>,
    phase: Phase,
    target: Option<&PointTarget>,
    label: impl Fn(&T) -> String,
) -> Option<(String, Node)> {
    let shown = row.shown(phase)?;
    let value = mark_value(shown.mark);
    let attrs_owned: Option<PointAttrs> = target.map(PointTarget::attrs);
    let mut attrs: Vec<(&str, &str)> = vec![(MARK_ATTR, value_attr(value))];
    if let Some(p) = &attrs_owned {
        attrs.extend(p.pairs());
    }
    let node = el(
        "li",
        attrs,
        vec![
            el(
                "span",
                vec![(TESTID_ATTR, MARK_GLYPH_TESTID)],
                vec![text(value.glyph())],
            ),
            el(
                "span",
                vec![(TESTID_ATTR, MARK_TEXT_TESTID)],
                vec![text(value.text())],
            ),
            el("span", vec![], vec![text(label(shown.content))]),
        ],
    );
    Some((format!("k{}", row.id().as_str()), node))
}

fn value_attr(v: fandhe_workshop_web::a11y::MarkValue) -> &'static str {
    use fandhe_workshop_web::a11y::MarkValue;
    match v {
        MarkValue::None => "",
        MarkValue::Mark(m) => m.data_value(),
    }
}

/// 7 本の keyed list を 1 部品に置くダミーの左部品。表示するビューと状態は構築時に決める。
struct SampleArtifactPart {
    selected: ViewKind,
    phase: Phase,
}

impl SampleArtifactPart {
    fn items_for(&self, kind: ViewKind) -> Vec<(String, Node)> {
        let make_target = |s: &Spec| {
            s.point
                .then(|| PointTarget::new(&view_id(kind), RowId::new(s.id).unwrap()).unwrap())
        };
        let targets: Vec<Option<PointTarget>> = SPECS.iter().map(make_target).collect();
        if kind == ViewKind::Document {
            doc_rows()
                .iter()
                .zip(&targets)
                .filter_map(|(r, t)| row_node(r, self.phase, t.as_ref(), |b| b.body.clone()))
                .collect()
        } else {
            item_rows()
                .iter()
                .zip(&targets)
                .filter_map(|(r, t)| row_node(r, self.phase, t.as_ref(), |c| c.label.clone()))
                .collect()
        }
    }
}

impl DisplayPart<HostModel> for SampleArtifactPart {
    fn kind(&self) -> PartKind {
        PartKind::ArtifactView
    }

    fn fields(&self) -> &'static [PartField] {
        &VIEW_LIST_FIELDS
    }

    fn view(&self, _model: &HostModel) -> Node {
        let lists = ViewKind::ALL
            .iter()
            .map(|&kind| {
                let selected = kind == self.selected;
                let items = if selected {
                    self.items_for(kind)
                } else {
                    Vec::new()
                };
                match view_list("ul", kind, selected, items) {
                    Ok(n) => n,
                    // 空の一覧で黙って続けず、固定の通知を返す。
                    Err(_) => el(
                        "section",
                        vec![(TESTID_ATTR, "view-list-error")],
                        vec![text("一覧を表示できません")],
                    ),
                }
            })
            .collect();
        el("section", vec![(TESTID_ATTR, "part-artifact-view")], lists)
    }
}

fn registry(selected: ViewKind, phase: Phase) -> PartRegistry<HostModel> {
    let mut r = PartRegistry::new();
    r.register(Box::new(SampleArtifactPart { selected, phase }))
        .unwrap();
    r
}

fn html_of(selected: ViewKind, phase: Phase) -> String {
    let reg = registry(selected, phase);
    let model = EngineHost::new();
    render(&reg.view(PartKind::ArtifactView, model.model()).unwrap())
}

fn count(h: &str, needle: &str) -> usize {
    h.matches(needle).count()
}

#[test]
fn 新しい登録簿に登録でき_標準の登録簿は使わない() {
    let r = registry(ViewKind::FileTree, Phase::After);
    assert_eq!(r.len(), 1);
    assert_eq!(r.fields().count(), 7);
    // 標準の登録簿には ArtifactView が登録済みなので、重ねると拒否される。
    let mut std_reg = standard_registry();
    assert!(std_reg
        .register(Box::new(SampleArtifactPart {
            selected: ViewKind::Diff,
            phase: Phase::After,
        }))
        .is_err());
}

#[test]
fn 親は_7_つあり_hidden_のないものは_1_つだけ() {
    for kind in ViewKind::ALL {
        let h = html_of(kind, Phase::After);
        assert_eq!(count(&h, "data-testid=\"view-list\""), 7, "{h}");
        assert_eq!(count(&h, "hidden"), 6, "{h}");
        assert_eq!(
            count(&h, &format!("data-view=\"{}\"", kind.as_str())),
            1,
            "{h}"
        );
    }
}

#[test]
fn 印は_before_で_0_件_after_で前後の組から数えた件数() {
    let expect_add = SPECS
        .iter()
        .filter(|s| s.before.is_none() && s.after.is_some())
        .count();
    let expect_remove = SPECS
        .iter()
        .filter(|s| s.before.is_some() && s.after.is_none())
        .count();
    let expect_change = SPECS
        .iter()
        .filter(|s| s.before.is_some() && s.after.is_some() && s.before != s.after)
        .count();
    assert!(expect_add >= 1 && expect_remove >= 1 && expect_change >= 1);
    for kind in ViewKind::ALL {
        let before = html_of(kind, Phase::Before);
        for v in ["add", "remove", "change"] {
            assert_eq!(
                count(&before, &format!("{MARK_ATTR}=\"{v}\"")),
                0,
                "{kind:?}"
            );
        }
        let after = html_of(kind, Phase::After);
        assert_eq!(count(&after, "data-mark=\"add\""), expect_add, "{kind:?}");
        assert_eq!(count(&after, "data-mark=\"remove\""), expect_remove);
        assert_eq!(count(&after, "data-mark=\"change\""), expect_change);
    }
}

#[test]
fn 指せる行の数は宛先を付けた行の数で_4_つの属性が組でそろう() {
    for kind in ViewKind::ALL {
        let before_n = SPECS
            .iter()
            .filter(|s| s.point && s.before.is_some())
            .count();
        let after_n = SPECS.iter().filter(|s| s.point).count();
        for (phase, n) in [(Phase::Before, before_n), (Phase::After, after_n)] {
            let h = html_of(kind, phase);
            assert_eq!(count(&h, "data-action=\"ui\""), n, "{kind:?}");
            assert_eq!(count(&h, "data-payload="), n);
            assert_eq!(count(&h, "data-row="), n);
            assert_eq!(count(&h, "tabindex=\"0\""), n);
        }
    }
}

#[test]
fn 危険な文字列は属性の値でも本文でもエスケープされる() {
    let h = html_of(ViewKind::FileTree, Phase::After);
    assert!(!h.contains("<b>"), "{h}");
    assert!(!h.contains("\"x\"&"), "{h}");
}

#[test]
fn 描画した木は_a11y_の未定義の検査を通る() {
    let reg = registry(ViewKind::CardList, Phase::After);
    let model = EngineHost::new();
    let tree = reg.view(PartKind::ArtifactView, model.model()).unwrap();
    assert!(
        check_no_undefined(&tree).is_ok(),
        "{:?}",
        check_no_undefined(&tree)
    );
}

#[test]
fn ホストに_mount_でき_7_つの親が出る() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    let ir = load_ir(&std::fs::read(path).unwrap()).unwrap();
    let host = EngineHost::with_ir(ir, registry(ViewKind::TaskTree, Phase::After)).unwrap();
    let h = render_component_html(&host);
    assert_eq!(count(&h, "data-testid=\"view-list\""), 7, "{h}");
}
