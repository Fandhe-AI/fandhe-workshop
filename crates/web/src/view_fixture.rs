//! 左の成果物ビューの共通の検査器が使う、マウントする代役の部品（TASK-471 2/3。ART-1・ART-12。試験の設定だけ）。
//!
//! 呼び出し元は `test_hooks` の `debug_view_fixture_*` だけで、`tools/browser/artifact-view-check.mjs` の
//! 検査器が、その口を通して 7 種 × 2 変種の切り替えを実際の画面（`Runtime::mount`）で流す。製品の `view()` には
//! 7 本の keyed list を描く部品がまだ無い（TASK-470）ため、検査器の枠が動くことを示すための仮の部品である。
//! 各ビューの本体（TASK-473〜TASK-479）と製品の結線（TASK-470）が入ったら、検査器は本体の画面へ向け直し、
//! この部品は役目を終える。
//!
//! # 何を再現し、何を再現しないか
//!
//! - 再現する: 7 本の keyed list（`view_list`）・選ばれていない list が空で `hidden`・前後の切り替えによる行の印
//!   （`Row::shown`・`mark_value`）・変種 (a)・(b) の印の付け先の違い（(b) は項目自身、(a) は項目の中の外枠）
//! - 切り替える: `fixture_control` で、7 本の list の外に束縛の無い要素を 1 つ足す（負の対照 N2。TASK-471 3/3）。
//!   上流は dirty field を束縛点に解決できないと root の子をまとめて差し替えるため、行の要素が置き換わる状態を作れる
//! - 再現しない: `wireframe-ui`・`headless-ui` の部品・ツリーの `role`・指せる行の属性。合格は検査器の枠が動く
//!   ことの証拠であり、各ビューの合格ではない
//!
//! # データ
//!
//! 文書以外の 6 種は、spec 由来の試験データ（`testdata/artifact-view-parts/test-data/sample/*.json`）を
//! `include_bytes!` で埋め込む。読めなければ `Err` を返し、空の一覧で黙って続けない（fail-closed）。値は
//! 運ぶだけで、`text()` の文字のノードにだけ入れる。文書は JSON が無いため、ブロックの並びを表で持つ。
//! 文書の前後の数（前 10・後 13 ブロック）の出所は `testdata/artifact-view-parts/test-data/sample/` の
//! `doc-before.md`・`doc-after.md` の最上位のブロックで、変更されたブロックは削除と追加の 2 行になる。

// native では `Runtime` に載せる口（`test_hooks`）が無いため、口からだけ使う項目は未使用になる。
#![cfg_attr(
    not(target_arch = "wasm32"),
    allow(dead_code, reason = "口は wasm32 の test_hooks だけが呼ぶため")
)]

use fandhe_frontend_core::{el, text, Node};
use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_frontend_wasm_client::{BindingSource, BoundValue};

use crate::a11y::TESTID_ATTR;
use crate::artifact_mark::marked_item;
use crate::artifact_view::{
    dirty_list_fields, view_list, Cell, DocRow, ItemRow, Phase, Row, RowId, RowKind, Variant,
    VariantPolicy, ViewKind,
};

/// 根の要素の `data-testid`。
pub(crate) const FIXTURE_TESTID: &str = "view-fixture";
/// 一覧を作れなかったときの固定の通知の `data-testid`。
pub(crate) const FIXTURE_ERROR_TESTID: &str = "view-fixture-error";
/// 選ぶ action の名前。payload は `<kind>:<variant>`。
pub(crate) const ACTION_SELECT: &str = "fixture_select";
/// 前後を切り替える action の名前。payload は `before` か `after`。
pub(crate) const ACTION_PHASE: &str = "fixture_phase";

/// 項目の外の部品を出し入れする action の名前。payload は `outside` か `inside`。
pub(crate) const ACTION_CONTROL: &str = "fixture_control";
/// 項目の外の部品の `data-testid`。
pub(crate) const FIXTURE_OUTSIDE_TESTID: &str = "view-fixture-outside";
/// 項目の外の部品を再描画させる dirty field の名前。束縛点にも keyed list にも対応しないため、上流は全再描画に倒れる。
pub(crate) const OUTSIDE_FIELD: &str = "fixture_outside";

/// 埋め込みの試験データの読み込み・構築の失敗。文面に入力の値を含めない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FixtureError {
    /// JSON として読めない、または形が違う。
    Data,
}

const FILETREE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/filetree.json"
));
const DIFF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/diff.json"
));
const TASKS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/tasks.json"
));
const CARDS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/cards.json"
));
const CHECKS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/checks.json"
));
const APPDEMO: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/appdemo.json"
));

/// 文書のブロックの表。`(id, 前, 後)`。変更されたブロックは、前だけの行と後だけの行の 2 行にする。
/// 前は 10 ブロック、後は 13 ブロック（未変更 7・追加 3・削除 3）。中身は汎用の短い文字列である。
const DOC_BLOCKS: [(&str, Option<&str>, Option<&str>); 13] = [
    ("d01", Some("見出し 1"), Some("見出し 1")),
    ("d02", Some("段落 1"), Some("段落 1")),
    ("d03", Some("見出し 2"), Some("見出し 2")),
    ("d04-old", Some("一覧 1"), None),
    ("d04-new", None, Some("一覧 1 改")),
    ("d05", Some("見出し 3"), Some("見出し 3")),
    ("d06", Some("段落 2"), Some("段落 2")),
    ("d07-old", Some("表 1"), None),
    ("d07-new", None, Some("表 1 改")),
    ("d08", Some("引用 1"), Some("引用 1")),
    ("d09", Some("見出し 4"), Some("見出し 4")),
    ("d10-old", Some("一覧 2"), None),
    ("d10-new", None, Some("一覧 2 改")),
];

/// 1 種のビューの行。文書だけ中身が文字列（本文の木の結線は TASK-476）。
enum ViewRows {
    Items(Vec<ItemRow>),
    Doc(Vec<DocRow<String>>),
}

fn json_cell(v: &serde_json::Value) -> Result<Option<Cell>, FixtureError> {
    if v.is_null() {
        return Ok(None);
    }
    let s = |k: &str| {
        v.get(k)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or(FixtureError::Data)
    };
    let tags = v
        .get("tags")
        .and_then(serde_json::Value::as_array)
        .ok_or(FixtureError::Data)?
        .iter()
        .map(|t| t.as_str().map(str::to_owned).ok_or(FixtureError::Data))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(Cell {
        label: s("label")?,
        detail: s("detail")?,
        state: s("state")?,
        tags,
    }))
}

fn load_items(bytes: &[u8]) -> Result<Vec<ItemRow>, FixtureError> {
    let root: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| FixtureError::Data)?;
    let rows = root
        .get("rows")
        .and_then(serde_json::Value::as_array)
        .ok_or(FixtureError::Data)?;
    rows.iter()
        .map(|r| {
            let id = r
                .get("id")
                .and_then(serde_json::Value::as_str)
                .ok_or(FixtureError::Data)?;
            let depth = r
                .get("depth")
                .and_then(serde_json::Value::as_u64)
                .and_then(|d| u8::try_from(d).ok())
                .ok_or(FixtureError::Data)?;
            let kind = r
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .and_then(RowKind::parse)
                .ok_or(FixtureError::Data)?;
            let before = json_cell(r.get("before").ok_or(FixtureError::Data)?)?;
            let after = json_cell(r.get("after").ok_or(FixtureError::Data)?)?;
            Row::new(
                RowId::new(id).map_err(|_| FixtureError::Data)?,
                depth,
                kind,
                before,
                after,
            )
            .map_err(|_| FixtureError::Data)
        })
        .collect()
}

fn doc_rows() -> Result<Vec<DocRow<String>>, FixtureError> {
    DOC_BLOCKS
        .iter()
        .map(|&(id, before, after)| {
            Row::new(
                RowId::new(id).map_err(|_| FixtureError::Data)?,
                0,
                RowKind::Block,
                before.map(str::to_owned),
                after.map(str::to_owned),
            )
            .map_err(|_| FixtureError::Data)
        })
        .collect()
}

/// 項目 1 件。(b) は項目自身が `data-mark` を持ち、(a) は項目の中の外枠が持つ（印の付け先の違いだけを再現する）。
fn row_node<T: PartialEq>(
    row: &Row<T>,
    phase: Phase,
    variant: Variant,
    label: impl Fn(&T) -> &str,
) -> Option<(String, Node)> {
    let shown = row.shown(phase)?;
    let node = marked_item(
        "li",
        variant,
        shown.mark,
        Some(row.depth()),
        vec![],
        vec![el("span", vec![], vec![text(label(shown.content))])],
    );
    Some((format!("k{}", row.id().as_str()), node))
}

/// 代役の部品の状態。`Runtime::mount` に載せる `Component` である。
pub(crate) struct ViewFixture {
    views: Vec<(ViewKind, ViewRows)>,
    selected: ViewKind,
    variant: Variant,
    phase: Phase,
    /// 項目の外に部品を置くか（負の対照 N2 の切り替え）。既定は置かない。
    outside: bool,
    dirty: Vec<&'static str>,
}

/// 状態を変える action。復号の段で `VariantPolicy` を通るため、許されない組は作れない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FixtureAction {
    /// ビューの種類と変種を選ぶ。
    Select(ViewKind, Variant),
    /// 前後を切り替える。
    Phase(Phase),
    /// 項目の外の部品を置く（`true`）か外す（`false`）。
    Outside(bool),
}

impl ViewFixture {
    /// 埋め込みの試験データから初期状態（ファイルツリー・(b)・前）を作る。
    ///
    /// # Errors
    ///
    /// 試験データが読めない、または行の検査を通らない場合は [`FixtureError::Data`]。
    pub(crate) fn load() -> Result<Self, FixtureError> {
        let mut views = Vec::new();
        for kind in ViewKind::ALL {
            let rows = match kind {
                ViewKind::Document => ViewRows::Doc(doc_rows()?),
                ViewKind::FileTree => ViewRows::Items(load_items(FILETREE)?),
                ViewKind::Diff => ViewRows::Items(load_items(DIFF)?),
                ViewKind::TaskTree => ViewRows::Items(load_items(TASKS)?),
                ViewKind::CardList => ViewRows::Items(load_items(CARDS)?),
                ViewKind::CheckList => ViewRows::Items(load_items(CHECKS)?),
                ViewKind::AppDemo => ViewRows::Items(load_items(APPDEMO)?),
            };
            views.push((kind, rows));
        }
        Ok(Self {
            views,
            selected: ViewKind::FileTree,
            variant: Variant::B,
            phase: Phase::Before,
            outside: false,
            dirty: Vec::new(),
        })
    }

    /// 選ばれているビューの種類。
    pub(crate) fn selected(&self) -> ViewKind {
        self.selected
    }

    /// 選ばれている変種。
    pub(crate) fn variant(&self) -> Variant {
        self.variant
    }

    /// 表示している状態。
    pub(crate) fn phase(&self) -> Phase {
        self.phase
    }

    fn items_of(&self, rows: &ViewRows) -> Vec<(String, Node)> {
        match rows {
            ViewRows::Items(v) => v
                .iter()
                .filter_map(|r| row_node(r, self.phase, self.variant, |c: &Cell| c.label.as_str()))
                .collect(),
            ViewRows::Doc(v) => v
                .iter()
                .filter_map(|r| row_node(r, self.phase, self.variant, |b: &String| b.as_str()))
                .collect(),
        }
    }
}

impl Component for ViewFixture {
    type Action = FixtureAction;

    fn update(&mut self, action: Self::Action) {
        self.dirty.clear();
        match action {
            FixtureAction::Select(kind, variant) => {
                let prev = self.selected;
                let changed = variant != self.variant;
                self.selected = kind;
                self.variant = variant;
                self.dirty = dirty_list_fields(Some(prev), Some(kind), changed);
            }
            FixtureAction::Phase(phase) => {
                let changed = phase != self.phase;
                self.phase = phase;
                self.dirty = dirty_list_fields(Some(self.selected), Some(self.selected), changed);
            }
            FixtureAction::Outside(on) => {
                if on != self.outside {
                    self.outside = on;
                    self.dirty.push(OUTSIDE_FIELD);
                }
            }
        }
        // 項目の外の部品があると、何かが変わる更新は束縛の無い field を伴い、全再描画になる。
        if self.outside && !self.dirty.is_empty() && !self.dirty.contains(&OUTSIDE_FIELD) {
            self.dirty.push(OUTSIDE_FIELD);
        }
    }

    fn view(&self) -> Node {
        let mut lists = Vec::new();
        for (kind, rows) in &self.views {
            let selected = *kind == self.selected;
            let items = if selected {
                self.items_of(rows)
            } else {
                Vec::new()
            };
            match view_list("ul", *kind, selected, items) {
                Ok(n) => lists.push(n),
                // 空の一覧で黙って続けず、固定の通知を返す。
                Err(_) => {
                    return el(
                        "section",
                        vec![(TESTID_ATTR, FIXTURE_ERROR_TESTID)],
                        vec![text("一覧を表示できません")],
                    )
                }
            }
        }
        if self.outside {
            let label = match self.phase {
                Phase::Before => "before",
                Phase::After => "after",
            };
            lists.insert(
                0,
                el(
                    "p",
                    vec![(TESTID_ATTR, FIXTURE_OUTSIDE_TESTID)],
                    vec![text(label)],
                ),
            );
        }
        el("div", vec![(TESTID_ATTR, FIXTURE_TESTID)], lists)
    }

    fn decode_action(name: &str, payload: &str) -> Option<Self::Action> {
        match name {
            ACTION_SELECT => {
                let (kind, variant) = payload.split_once(':')?;
                let kind = ViewKind::parse(kind)?;
                let variant = Variant::ALL
                    .iter()
                    .copied()
                    .find(|v| v.as_str() == variant)?;
                // 許されない組（文書・差分の (a)）は拒否し、状態を変えない。
                VariantPolicy::new(&[(kind, variant)]).ok()?;
                Some(FixtureAction::Select(kind, variant))
            }
            ACTION_PHASE => match payload {
                "before" => Some(FixtureAction::Phase(Phase::Before)),
                "after" => Some(FixtureAction::Phase(Phase::After)),
                _ => None,
            },
            ACTION_CONTROL => match payload {
                "outside" => Some(FixtureAction::Outside(true)),
                "inside" => Some(FixtureAction::Outside(false)),
                _ => None,
            },
            _ => None,
        }
    }
}

impl DirtyTracked for ViewFixture {
    fn dirty_fields(&self) -> &[&'static str] {
        &self.dirty
    }
}

impl BindingSource for ViewFixture {
    /// 束縛点は keyed list だけで、束縛する値は無い。
    fn bound_value(&self, _field: &str) -> Option<BoundValue> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fandhe_frontend_core::render;

    fn row_counts(f: &ViewFixture, kind: ViewKind) -> (usize, usize) {
        let rows = &f.views.iter().find(|(k, _)| *k == kind).unwrap().1;
        match rows {
            ViewRows::Items(v) => (v.iter().filter(|r| r.before().is_some()).count(), v.len()),
            ViewRows::Doc(v) => (v.iter().filter(|r| r.before().is_some()).count(), v.len()),
        }
    }

    #[test]
    fn 試験データの行の数は手で数えた値と一致する() {
        let f = ViewFixture::load().unwrap();
        // （前に出る行の数, 全行の数）。全行は後の状態で出る行の数に等しい（削除は前の中身を残して出す）。
        for (kind, want) in [
            (ViewKind::FileTree, (10, 11)),
            (ViewKind::Diff, (7, 9)),
            (ViewKind::TaskTree, (5, 6)),
            (ViewKind::CardList, (3, 4)),
            (ViewKind::CheckList, (5, 6)),
            (ViewKind::AppDemo, (8, 9)),
            (ViewKind::Document, (10, 13)),
        ] {
            assert_eq!(row_counts(&f, kind), want, "{kind:?}");
        }
    }

    #[test]
    fn 初期状態はファイルツリーの_b_の前() {
        let f = ViewFixture::load().unwrap();
        assert_eq!(f.selected(), ViewKind::FileTree);
        assert_eq!(f.variant(), Variant::B);
        assert_eq!(f.phase(), Phase::Before);
        let html = render(&f.view());
        assert_eq!(
            html.matches("data-testid=\"view-list\"").count(),
            7,
            "{html}"
        );
        assert_eq!(html.matches("hidden").count(), 6, "{html}");
        assert_eq!(html.matches("data-mark=\"\"").count(), 10, "{html}");
    }

    #[test]
    fn 文書と差分の_a_は復号の段で拒否される() {
        for kind in ["document", "diff"] {
            assert_eq!(
                ViewFixture::decode_action(ACTION_SELECT, &format!("{kind}:a")),
                None,
                "{kind}"
            );
            assert!(
                ViewFixture::decode_action(ACTION_SELECT, &format!("{kind}:b")).is_some(),
                "{kind}"
            );
        }
        assert!(ViewFixture::decode_action(ACTION_SELECT, "card_list:a").is_some());
    }

    #[test]
    fn 未知の名前と不正な_payload_は拒否される() {
        for (name, payload) in [
            ("unknown", "file_tree:b"),
            (ACTION_SELECT, ""),
            (ACTION_SELECT, "file_tree"),
            (ACTION_SELECT, "file_tree:c"),
            (ACTION_SELECT, "File_Tree:b"),
            (ACTION_SELECT, "file_tree:b "),
            (ACTION_PHASE, "Before"),
            (ACTION_PHASE, ""),
        ] {
            assert_eq!(
                ViewFixture::decode_action(name, payload),
                None,
                "{name} {payload}"
            );
        }
    }

    #[test]
    fn 後の状態の印は前後の組から導いた件数になる() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Phase(Phase::After));
        let html = render(&f.view());
        assert_eq!(html.matches("data-mark=\"add\"").count(), 1, "{html}");
        assert_eq!(html.matches("data-mark=\"remove\"").count(), 1, "{html}");
        assert_eq!(html.matches("data-mark=\"change\"").count(), 2, "{html}");
        assert_eq!(f.dirty_fields(), ["view_file_tree"]);
    }

    #[test]
    fn 選び直すと旧と新の両方が_dirty_になり_選ばれない側は空で_hidden() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Select(ViewKind::CardList, Variant::A));
        assert_eq!(f.dirty_fields(), ["view_file_tree", "view_card_list"]);
        let html = render(&f.view());
        assert_eq!(html.matches("hidden").count(), 6, "{html}");
        // (a) は項目の中の外枠が印を持つ。
        assert!(html.contains("><div data-mark=\"\" class=\"d"), "{html}");
    }

    #[test]
    fn 同じ選択の繰り返しは_dirty_を積まない() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Select(ViewKind::FileTree, Variant::B));
        assert!(f.dirty_fields().is_empty());
        f.update(FixtureAction::Phase(Phase::Before));
        assert!(f.dirty_fields().is_empty());
    }

    #[test]
    fn 項目の外の部品の切り替えは復号され_不正な_payload_は拒否される() {
        assert_eq!(
            ViewFixture::decode_action(ACTION_CONTROL, "outside"),
            Some(FixtureAction::Outside(true))
        );
        assert_eq!(
            ViewFixture::decode_action(ACTION_CONTROL, "inside"),
            Some(FixtureAction::Outside(false))
        );
        for payload in ["", "Outside", "outside ", "on"] {
            assert_eq!(
                ViewFixture::decode_action(ACTION_CONTROL, payload),
                None,
                "{payload}"
            );
        }
    }

    #[test]
    fn 外に置いたときだけ_束縛の無い_dirty_field_が積まれ_外の要素が出る() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Phase(Phase::After));
        assert_eq!(f.dirty_fields(), ["view_file_tree"]);
        assert_eq!(render(&f.view()).matches(FIXTURE_OUTSIDE_TESTID).count(), 0);
        f.update(FixtureAction::Outside(true));
        assert_eq!(f.dirty_fields(), [OUTSIDE_FIELD]);
        assert_eq!(render(&f.view()).matches(FIXTURE_OUTSIDE_TESTID).count(), 1);
        f.update(FixtureAction::Phase(Phase::Before));
        assert_eq!(f.dirty_fields(), ["view_file_tree", OUTSIDE_FIELD]);
        // 変化の無い更新は積まない。
        f.update(FixtureAction::Phase(Phase::Before));
        assert!(f.dirty_fields().is_empty());
        f.update(FixtureAction::Outside(false));
        assert_eq!(f.dirty_fields(), [OUTSIDE_FIELD]);
        f.update(FixtureAction::Phase(Phase::After));
        assert_eq!(f.dirty_fields(), ["view_file_tree"]);
        assert_eq!(render(&f.view()).matches(FIXTURE_OUTSIDE_TESTID).count(), 0);
    }

    #[test]
    fn 壊れた試験データは_err_になる() {
        assert_eq!(load_items(b"not json").err(), Some(FixtureError::Data));
        assert_eq!(
            load_items(b"{\"rows\":[{}]}").err(),
            Some(FixtureError::Data)
        );
        assert_eq!(load_items(b"{\"rows\":[]}").map(|v| v.len()), Ok(0));
    }
}
