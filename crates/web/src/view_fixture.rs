//! 左の成果物ビューの共通の検査器が使う、マウントする代役の部品（TASK-471 2/3。ART-1・ART-12。試験の設定だけ）。
//!
//! 呼び出し元は `test_hooks` の `debug_view_fixture_*` だけで、`tools/browser/artifact-view-check.mjs` の
//! 検査器が、その口を通して 7 種 × 2 変種の切り替えを実際の画面（`Runtime::mount`）で流す。製品の `view()` には
//! 7 本の keyed list が入った（TASK-470）が各ビューの本体がまだ無いため、検査器の枠が動くことを示すための仮の部品である。
//! 各ビューの本体（TASK-473〜TASK-479）と製品の結線（TASK-470）が入ったら、検査器は本体の画面へ向け直し、
//! この部品は役目を終える。
//!
//! # 何を再現し、何を再現しないか
//!
//! - 再現する: 7 本の keyed list（`view_list`）・選ばれていない list が空で `hidden`・前後の切り替えによる行の印
//!   （`Row::shown`・`mark_value`）・変種 (a)・(b) の印の付け先の違い（(b) は項目自身、(a) は項目の中の外枠）
//! - 切り替える: `fixture_control` で、7 本の list の外に束縛の無い要素を 1 つ足す（負の対照 N2。TASK-471 3/3）。
//!   上流は dirty field を束縛点に解決できないと root の子をまとめて差し替えるため、行の要素が置き換わる状態を作れる
//! - 本体を通す: タスクツリーの (a)・(b) は `task_tree`（(b) は木の属性・親の `role=tree`、(a) はチェックボックスと深さのクラス。どちらも状態のバッジ。TASK-474）で、ファイルツリーの (a)・(b) は `file_tree_view`（TASK-473）で、カードの (a)・(b) は `card_view`（TASK-477）で、チェックの一覧の (a)・(b) は `check_view`（TASK-478）で、アプリのデモの (a) は `app_demo_view`（TASK-479 1/2）で、差分の (b) は `diff_view`（TASK-475 1/2）で、差分の (a) は対照の口からだけ `diff_view` の対照専用の項目で描く（TASK-475 2/2）、文書の (b) は `document_view`（TASK-476 1/2。前後の突き合わせは `align_blocks`）で、文書の (a) は対照の口からだけ `document_view` の対照専用の項目で描く（TASK-476 2/2）
//! - 再現しない: `wireframe-ui`・`headless-ui` の部品（ファイルツリーの `item_text`・`rich_text` を除く）・他のビューの `role`・指せる行の属性。合格は検査器の枠が動く
//!   ことの証拠であり、タスクツリー・ファイルツリー・差分 (b) を除く各ビューの合格ではない
//!
//! # データ
//!
//! 文書以外の 6 種は、spec 由来の試験データ（`testdata/artifact-view-parts/test-data/sample/*.json`）を
//! `include_bytes!` で埋め込む。読めなければ `Err` を返し、空の一覧で黙って続けない（fail-closed）。値は
//! 運ぶだけで、`text()` の文字のノードにだけ入れる。文書は JSON が無いため、`doc-before.md`・`doc-after.md` を
//! ビルド時に木にした JSON の写しで持つ（実行時に Markdown は解釈しない）。
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
use fandhe_workshop_contract::doc::{Block, Doc};

use crate::a11y::TESTID_ATTR;
use crate::app_demo_view::app_demo_items_a;
use crate::artifact_doc_rows::align_blocks;
use crate::artifact_mark::marked_item;
use crate::artifact_view::{
    dirty_list_fields, tree_view_list, view_list, Cell, DocRow, ItemRow, Phase, Row, RowId,
    RowKind, Variant, VariantPolicy, ViewKind,
};
use crate::card_view::card_items;
use crate::check_view::check_items;
use crate::diff_view::{diff_items, diff_items_a, diff_list};
use crate::document_view;
use crate::file_tree_view::{file_tree_items, file_tree_items_a};
use crate::task_tree::task_items;

/// 根の要素の `data-testid`。
pub(crate) const FIXTURE_TESTID: &str = "view-fixture";
/// 一覧を作れなかったときの固定の通知の `data-testid`。
pub(crate) const FIXTURE_ERROR_TESTID: &str = "view-fixture-error";
/// 選ぶ action の名前。payload は `<kind>:<variant>`。
pub(crate) const ACTION_SELECT: &str = "fixture_select";
/// 前後を切り替える action の名前。payload は `before` か `after`。
pub(crate) const ACTION_PHASE: &str = "fixture_phase";
/// 行のデータを切り替える action の名前。payload は `cards:sample`・`cards:20`（TASK-477）・`checks:sample`・`checks:20`（TASK-478。規模の検査用）。
pub(crate) const ACTION_DATA: &str = "fixture_data";

/// 対照の口の action の名前。payload は `diff:a`・`document:a` だけを受ける（製品の契約が拒否する組を、試験の設定の中でだけ描く）。
pub(crate) const ACTION_CONTRAST: &str = "fixture_contrast";

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
/// カード 20 件の規模のデータ（`generate.mjs` の `cards-20.json` の写し。出所は同じ場所の `README.md`）。
const CARDS_20: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/artifact_view_scale/cards-20.json"
));
/// チェック 20 件の規模のデータ（`generate.mjs` の `checks-20.json` の写し。出所は同じ場所の `README.md`）。
const CHECKS_20: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/artifact_view_scale/checks-20.json"
));
const CHECKS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/checks.json"
));
const APPDEMO: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/appdemo.json"
));

/// `doc-before.md` を `markdown::parse` で木にした JSON（`Doc`）。実行時に Markdown を解釈しないため、ビルド時の出力を
/// 写して持つ。写しが元の Markdown と食い違わないことは、単体試験が `markdown::parse`（pack。dev-dependency）で確かめる。
const DOC_BEFORE_JSON: &str = r#"{"version":1,"blocks":[{"t":"heading","level":1,"children":[{"t":"text","text":"メモ帳アプリの使い方"}]},{"t":"paragraph","children":[{"t":"text","text":"メモ帳アプリは、短いメモを保存して一覧で見るための練習用アプリです。"}]},{"t":"heading","level":2,"children":[{"t":"text","text":"準備"}]},{"t":"list","ordered":true,"items":[[{"t":"paragraph","children":[{"t":"text","text":"リポジトリを開きます。"}]}],[{"t":"paragraph","children":[{"t":"code","text":"cargo run"},{"t":"text","text":" で起動します。"}]}],[{"t":"paragraph","children":[{"t":"text","text":"画面の「追加」を押します。"}]}]]},{"t":"heading","level":2,"children":[{"t":"text","text":"保存の仕組み"}]},{"t":"paragraph","children":[{"t":"text","text":"メモは "},{"t":"code","text":"notes/"},{"t":"text","text":" の下に 1 件 1 ファイルで保存します。"}]},{"t":"table","head":[[{"t":"text","text":"項目"}],[{"t":"text","text":"内容"}]],"rows":[[[{"t":"text","text":"形式"}],[{"t":"text","text":"Markdown"}]],[[{"t":"text","text":"場所"}],[{"t":"code","text":"notes/"}]]]},{"t":"quote","children":[{"t":"paragraph","children":[{"t":"text","text":"注意: 保存先を変えると、古いメモは見えなくなります。"}]}]},{"t":"heading","level":2,"children":[{"t":"text","text":"次の手順"}]},{"t":"list","ordered":false,"items":[[{"t":"paragraph","children":[{"t":"text","text":"メモを書く"}]}],[{"t":"paragraph","children":[{"t":"text","text":"保存する"}]}]]}]}"#;
/// `doc-after.md` の木の JSON。
const DOC_AFTER_JSON: &str = r#"{"version":1,"blocks":[{"t":"heading","level":1,"children":[{"t":"text","text":"メモ帳アプリの使い方"}]},{"t":"paragraph","children":[{"t":"text","text":"メモ帳アプリは、短いメモを保存して一覧で見るための練習用アプリです。"}]},{"t":"heading","level":2,"children":[{"t":"text","text":"準備"}]},{"t":"list","ordered":true,"items":[[{"t":"paragraph","children":[{"t":"text","text":"リポジトリを開きます。"}]}],[{"t":"paragraph","children":[{"t":"code","text":"cargo run"},{"t":"text","text":" で起動します。"}]}],[{"t":"paragraph","children":[{"t":"text","text":"画面の「追加」を押します。"}]}],[{"t":"paragraph","children":[{"t":"text","text":"一覧に新しいメモが出ることを確かめます。"}]}]]},{"t":"heading","level":2,"children":[{"t":"text","text":"保存の仕組み"}]},{"t":"paragraph","children":[{"t":"text","text":"メモは "},{"t":"code","text":"notes/"},{"t":"text","text":" の下に 1 件 1 ファイルで保存します。"}]},{"t":"table","head":[[{"t":"text","text":"項目"}],[{"t":"text","text":"内容"}]],"rows":[[[{"t":"text","text":"形式"}],[{"t":"text","text":"Markdown"}]],[[{"t":"text","text":"場所"}],[{"t":"code","text":"notes/"}]],[[{"t":"text","text":"文字コード"}],[{"t":"text","text":"UTF-8"}]]]},{"t":"quote","children":[{"t":"paragraph","children":[{"t":"text","text":"注意: 保存先を変えると、古いメモは見えなくなります。"}]}]},{"t":"heading","level":2,"children":[{"t":"text","text":"次の手順"}]},{"t":"list","ordered":false,"items":[[{"t":"paragraph","children":[{"t":"text","text":"メモを書く"}]}],[{"t":"paragraph","children":[{"t":"text","text":"保存する"}]}],[{"t":"paragraph","children":[{"t":"text","text":"一覧で確かめる"}]}]]}]}"#;

/// 1 種のビューの行。文書だけ中身が本文の木のブロック（`contract::doc::Block`。TASK-476）。
enum ViewRows {
    Items(Vec<ItemRow>),
    Doc(Vec<DocRow<Block>>),
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

fn doc_blocks(json: &str) -> Result<Vec<Block>, FixtureError> {
    serde_json::from_str::<Doc>(json)
        .map(|d| d.blocks)
        .map_err(|_| FixtureError::Data)
}

/// 前後の木のブロックを突き合わせ、`-` と `+` の行にする。突き合わせに失敗したら `Err`（fail-closed）。
fn doc_rows() -> Result<Vec<DocRow<Block>>, FixtureError> {
    align_blocks(
        "document",
        &doc_blocks(DOC_BEFORE_JSON)?,
        &doc_blocks(DOC_AFTER_JSON)?,
    )
    .map_err(|_| FixtureError::Data)
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
    /// データの切り替えで読み込みに失敗したか。立っている間は固定の通知を出す（古いデータを黙って見せない）。
    load_failed: bool,
    dirty: Vec<&'static str>,
}

/// 状態を変える action。`Select` は復号の段で `VariantPolicy` を通るため、許されない組は作れない。
/// `Contrast` は対照の口で、差分・文書の (a) だけを受ける。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FixtureAction {
    /// ビューの種類と変種を選ぶ。
    Select(ViewKind, Variant),
    /// 製品の契約が拒否する組を対照として選ぶ（差分・文書の (a) だけ）。
    Contrast(ViewKind),
    /// 前後を切り替える。
    Phase(Phase),
    /// 項目の外の部品を置く（`true`）か外す（`false`）。
    Outside(bool),
    /// 指定したビューの行のデータを差し替える（カード・チェックだけ）。
    Data(ViewKind, DataSet),
}

/// 行のデータの出所。閉じた集合で、それ以外の payload は復号の段で拒否する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DataSet {
    /// spec 由来の試験データ（3 行・後 4 行）。
    Sample,
    /// 規模の検査用の 20 行。
    Scale20,
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
            load_failed: false,
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

    fn items_of(&self, kind: ViewKind, rows: &ViewRows) -> Vec<(String, Node)> {
        match rows {
            // ファイルツリーは (a)・(b) とも本体のビュー（TASK-473）で描く。他は汎用の描き方のまま。
            ViewRows::Items(v) if kind == ViewKind::FileTree && self.variant == Variant::A => {
                file_tree_items_a(v, self.phase)
            }
            ViewRows::Items(v) if kind == ViewKind::FileTree && self.variant == Variant::B => {
                file_tree_items(v, self.phase)
            }
            // 差分の (b) は本体のビュー（TASK-475）で描く。(a) は `fixture_select` では拒否され、対照の口からだけ入る。
            ViewRows::Items(v) if kind == ViewKind::Diff && self.variant == Variant::B => {
                diff_items(v, self.phase)
            }
            ViewRows::Items(v) if kind == ViewKind::Diff && self.variant == Variant::A => {
                diff_items_a(v, self.phase)
            }
            // カードは (a)・(b) とも本体のビュー（TASK-477）で描く。
            ViewRows::Items(v) if kind == ViewKind::CardList => {
                card_items(v, self.phase, self.variant)
            }
            // チェックの一覧は (a)・(b) とも本体のビュー（TASK-478）で描く。
            ViewRows::Items(v) if kind == ViewKind::CheckList => {
                check_items(v, self.phase, self.variant)
            }
            // アプリのデモの (a) は本体のビュー（TASK-479 1/2）で描く。(b) は 2/2 まで汎用の描き方のまま。
            ViewRows::Items(v) if kind == ViewKind::AppDemo && self.variant == Variant::A => {
                app_demo_items_a(v, self.phase)
            }
            // タスクツリーは (a)・(b) とも本体を通す。他の種類は従来の汎用の項目のまま。
            ViewRows::Items(v) if kind == ViewKind::TaskTree => {
                task_items(v, self.phase, self.variant)
            }
            ViewRows::Items(v) => v
                .iter()
                .filter_map(|r| row_node(r, self.phase, self.variant, |c: &Cell| c.label.as_str()))
                .collect(),
            // 文書の (b) は本体のビュー（TASK-476）で描く。(a) は `fixture_select` では拒否され、対照の口からだけ入る。
            ViewRows::Doc(v) if self.variant == Variant::B => {
                document_view::doc_items(v, self.phase)
            }
            ViewRows::Doc(v) => document_view::doc_items_a(v, self.phase),
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
            FixtureAction::Contrast(kind) => {
                let prev = self.selected;
                let changed = self.variant != Variant::A;
                self.selected = kind;
                self.variant = Variant::A;
                self.dirty = dirty_list_fields(Some(prev), Some(kind), changed);
            }
            FixtureAction::Phase(phase) => {
                let changed = phase != self.phase;
                self.phase = phase;
                self.dirty = dirty_list_fields(Some(self.selected), Some(self.selected), changed);
            }
            FixtureAction::Data(kind, data) => {
                let bytes = match (kind, data) {
                    (ViewKind::CardList, DataSet::Sample) => CARDS,
                    (ViewKind::CardList, DataSet::Scale20) => CARDS_20,
                    (ViewKind::CheckList, DataSet::Sample) => CHECKS,
                    (ViewKind::CheckList, DataSet::Scale20) => CHECKS_20,
                    // 復号の段で拒否済み。来た場合は何も変えない。
                    _ => return,
                };
                // 読めなければ失敗の状態を立て、通知を出す（fail-closed。古いデータを黙って見せない）。
                let loaded = load_items(bytes);
                let failed = loaded.is_err();
                if failed != self.load_failed {
                    self.load_failed = failed;
                    // 束縛の無い field は全再描画になる。
                    self.dirty.push(OUTSIDE_FIELD);
                }
                if let Ok(rows) = loaded {
                    if let Some((_, slot)) = self.views.iter_mut().find(|(k, _)| *k == kind) {
                        let same = matches!(slot, ViewRows::Items(cur) if *cur == rows);
                        *slot = ViewRows::Items(rows);
                        // 選ばれていない list は空で `hidden` のため、選ばれているときだけ dirty にする。
                        if !same && self.selected == kind {
                            self.dirty = dirty_list_fields(Some(kind), Some(kind), true);
                        }
                    }
                }
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
        if self.load_failed {
            return el(
                "section",
                vec![(TESTID_ATTR, FIXTURE_ERROR_TESTID)],
                vec![text("データを読み込めません")],
            );
        }
        let mut lists = Vec::new();
        for (kind, rows) in &self.views {
            let selected = *kind == self.selected;
            let items = if selected {
                self.items_of(*kind, rows)
            } else {
                Vec::new()
            };
            // 木の 2 種類は親の `role=tree` を足す経路を通す。(b) で選ばれ項目があるときだけ付き、(a) は `view_list` と同じ属性になる。
            let list = if matches!(*kind, ViewKind::FileTree | ViewKind::TaskTree) {
                tree_view_list("ul", *kind, self.variant, selected, items)
            } else if *kind == ViewKind::Diff {
                diff_list("ul", selected, items)
            } else if *kind == ViewKind::Document {
                document_view::doc_list("ul", selected, items)
            } else {
                view_list("ul", *kind, selected, items)
            };
            match list {
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
            ACTION_CONTRAST => match payload {
                "diff:a" => Some(FixtureAction::Contrast(ViewKind::Diff)),
                "document:a" => Some(FixtureAction::Contrast(ViewKind::Document)),
                _ => None,
            },
            ACTION_PHASE => match payload {
                "before" => Some(FixtureAction::Phase(Phase::Before)),
                "after" => Some(FixtureAction::Phase(Phase::After)),
                _ => None,
            },
            ACTION_DATA => match payload {
                "cards:sample" => Some(FixtureAction::Data(ViewKind::CardList, DataSet::Sample)),
                "cards:20" => Some(FixtureAction::Data(ViewKind::CardList, DataSet::Scale20)),
                "checks:sample" => Some(FixtureAction::Data(ViewKind::CheckList, DataSet::Sample)),
                "checks:20" => Some(FixtureAction::Data(ViewKind::CheckList, DataSet::Scale20)),
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
    fn タスクツリーの_b_の後は本体を通り木の属性と状態のバッジが出る() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Select(ViewKind::TaskTree, Variant::B));
        f.update(FixtureAction::Phase(Phase::After));
        let html = render(&f.view());
        assert_eq!(html.matches("role=\"tree\"").count(), 1, "{html}");
        assert_eq!(html.matches("role=\"treeitem\"").count(), 6, "{html}");
        assert_eq!(
            html.matches("data-testid=\"state-badge\"").count(),
            6,
            "{html}"
        );
    }

    #[test]
    fn タスクツリーの_a_の後は本体を通り木の意味が無くチェックと状態のバッジが出る() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Select(ViewKind::TaskTree, Variant::A));
        f.update(FixtureAction::Phase(Phase::After));
        let html = render(&f.view());
        assert_eq!(html.matches("role=\"tree\"").count(), 0, "{html}");
        assert_eq!(html.matches("role=\"treeitem\"").count(), 0, "{html}");
        assert_eq!(
            html.matches("data-testid=\"state-badge\"").count(),
            6,
            "{html}"
        );
        assert_eq!(html.matches("data-checked=\"true\"").count(), 3, "{html}");
    }

    #[test]
    fn 差分の_b_の後は本体を通りコードの_pre_と記号の列が出る() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Select(ViewKind::Diff, Variant::B));
        f.update(FixtureAction::Phase(Phase::After));
        let html = render(&f.view());
        assert_eq!(
            html.matches("<pre data-testid=\"diff-code\"").count(),
            9,
            "{html}"
        );
        assert_eq!(
            html.matches("data-testid=\"diff-sign\"").count(),
            9,
            "{html}"
        );
    }

    #[test]
    fn 木の写しは_markdown_の解釈結果と一致する() {
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../testdata/artifact-view-parts/test-data/sample/"
        );
        for (file, json) in [
            ("doc-before.md", DOC_BEFORE_JSON),
            ("doc-after.md", DOC_AFTER_JSON),
        ] {
            let md = std::fs::read_to_string(format!("{dir}{file}")).unwrap();
            let parsed = fandhe_workshop_pack::markdown::parse(&md);
            let copy: Doc = serde_json::from_str(json).unwrap();
            assert_eq!(copy, parsed, "{file}");
        }
    }

    #[test]
    fn 文書の_b_の後は本体を通り構造の要素が出る() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Select(ViewKind::Document, Variant::B));
        f.update(FixtureAction::Phase(Phase::After));
        let html = render(&f.view());
        assert_eq!(
            html.matches("data-testid=\"doc-block\"").count(),
            13,
            "{html}"
        );
        assert_eq!(html.matches("<h1").count(), 1, "{html}");
        assert_eq!(html.matches("<h2").count(), 3, "{html}");
        assert_eq!(html.matches("<table").count(), 2, "{html}");
        assert_eq!(html.matches("<blockquote").count(), 1, "{html}");
        assert_eq!(html.matches("data-mark=\"add\"").count(), 3, "{html}");
        assert_eq!(html.matches("data-mark=\"remove\"").count(), 3, "{html}");
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
    fn 対照の口は_diff_a_と_document_a_だけを受け_他の_payload_は拒否される() {
        assert_eq!(
            ViewFixture::decode_action(ACTION_CONTRAST, "diff:a"),
            Some(FixtureAction::Contrast(ViewKind::Diff))
        );
        assert_eq!(
            ViewFixture::decode_action(ACTION_CONTRAST, "document:a"),
            Some(FixtureAction::Contrast(ViewKind::Document))
        );
        for payload in [
            "document:b",
            "Document:a",
            "document:a ",
            "document",
            "diff:b",
            "card_list:a",
            "Diff:a",
            "diff:a ",
            "diff",
            "",
        ] {
            assert_eq!(
                ViewFixture::decode_action(ACTION_CONTRAST, payload),
                None,
                "{payload}"
            );
        }
    }

    #[test]
    fn 対照の口で差分の_a_の後は_wire_text_で描かれ_pre_は無い() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Contrast(ViewKind::Diff));
        f.update(FixtureAction::Phase(Phase::After));
        assert_eq!((f.selected(), f.variant()), (ViewKind::Diff, Variant::A));
        let html = render(&f.view());
        assert_eq!(html.matches("fw-wire-text").count(), 9, "{html}");
        assert_eq!(html.matches("<pre").count(), 0, "{html}");
        assert_eq!(
            html.matches("data-testid=\"diff-sign\"").count(),
            9,
            "{html}"
        );
        // (a) の印の宿主は項目自身ではなく、内側の div である。
        assert!(!html.contains("<li data-mark"), "{html}");
        assert!(html.contains("<div"), "{html}");
    }

    #[test]
    fn 対照の後も_fixture_select_の_diff_a_は拒否される() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Contrast(ViewKind::Diff));
        assert_eq!(ViewFixture::decode_action(ACTION_SELECT, "diff:a"), None);
    }

    #[test]
    fn 対照の口で文書の_a_の後は構造の要素が無く平文の部品で描かれる() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Contrast(ViewKind::Document));
        f.update(FixtureAction::Phase(Phase::After));
        assert_eq!(
            (f.selected(), f.variant()),
            (ViewKind::Document, Variant::A)
        );
        let html = render(&f.view());
        assert_eq!(
            html.matches("data-testid=\"doc-block\"").count(),
            13,
            "{html}"
        );
        for tag in [
            "<h1",
            "<h2",
            "<ol",
            "<table",
            "<blockquote",
            "<code",
            "<pre",
        ] {
            assert_eq!(html.matches(tag).count(), 0, "{tag} {html}");
        }
        // ul は 7 種の keyed list の親だけで、本文の一覧は ul にならない。
        assert_eq!(html.matches("<ul").count(), 7, "{html}");
        assert!(html.contains("fw-wire-"), "{html}");
        assert!(!html.contains("<li data-mark"), "{html}");
    }

    #[test]
    fn 対照の後も_fixture_select_の_document_a_は拒否される() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Contrast(ViewKind::Document));
        assert_eq!(
            ViewFixture::decode_action(ACTION_SELECT, "document:a"),
            None
        );
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
    fn カードの_b_の後は本体を通り_article_と_marker_が出る() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Select(ViewKind::CardList, Variant::B));
        f.update(FixtureAction::Phase(Phase::After));
        let html = render(&f.view());
        assert_eq!(html.matches("<article").count(), 4, "{html}");
        assert_eq!(html.matches("data-scope=\"marker\"").count(), 8, "{html}");
        assert_eq!(
            html.matches("data-kind=\"card\" data-mark=\"change\"")
                .count(),
            2,
            "{html}"
        );
    }

    #[test]
    fn データの切り替えは閉じた_payload_だけを受ける() {
        assert_eq!(
            ViewFixture::decode_action(ACTION_DATA, "cards:20"),
            Some(FixtureAction::Data(ViewKind::CardList, DataSet::Scale20))
        );
        assert_eq!(
            ViewFixture::decode_action(ACTION_DATA, "cards:sample"),
            Some(FixtureAction::Data(ViewKind::CardList, DataSet::Sample))
        );
        for bad in ["Cards:20", "cards:21", "cards:", "", "cards:20 ", "diff:20"] {
            assert!(
                ViewFixture::decode_action(ACTION_DATA, bad).is_none(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn カードを選んでいるときのデータの切り替えで行が差し替わり_戻せる() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Select(ViewKind::CardList, Variant::B));
        f.update(FixtureAction::Data(ViewKind::CardList, DataSet::Scale20));
        assert_eq!(f.dirty_fields(), ["view_card_list"]);
        assert_eq!(row_counts(&f, ViewKind::CardList), (20, 20));
        f.update(FixtureAction::Data(ViewKind::CardList, DataSet::Scale20));
        assert!(f.dirty_fields().is_empty());
        f.update(FixtureAction::Data(ViewKind::CardList, DataSet::Sample));
        assert_eq!(row_counts(&f, ViewKind::CardList), (3, 4));
    }

    #[test]
    fn カードを選んでいないときのデータの切り替えは_dirty_を積まない() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Data(ViewKind::CardList, DataSet::Scale20));
        assert!(f.dirty_fields().is_empty());
        assert_eq!(row_counts(&f, ViewKind::CardList), (20, 20));
    }

    #[test]
    fn チェックの_a_と_b_は本体を通り_記号つきの状態が出る() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Select(ViewKind::CheckList, Variant::B));
        f.update(FixtureAction::Phase(Phase::After));
        let html = render(&f.view());
        assert_eq!(
            html.matches("data-testid=\"check-label\"").count(),
            6,
            "{html}"
        );
        assert_eq!(html.matches("data-scope=\"marker\"").count(), 12, "{html}");
        assert!(html.contains("✗ 失敗"), "{html}");
        f.update(FixtureAction::Select(ViewKind::CheckList, Variant::A));
        let html = render(&f.view());
        assert!(html.matches("fw-wire-rich-text").count() >= 6, "{html}");
        assert!(html.contains(">✓<") && html.contains(">成功<"), "{html}");
    }

    #[test]
    fn チェックのデータの切り替えは閉じた_payload_だけを受け_戻せる() {
        assert_eq!(
            ViewFixture::decode_action(ACTION_DATA, "checks:20"),
            Some(FixtureAction::Data(ViewKind::CheckList, DataSet::Scale20))
        );
        assert_eq!(
            ViewFixture::decode_action(ACTION_DATA, "checks:sample"),
            Some(FixtureAction::Data(ViewKind::CheckList, DataSet::Sample))
        );
        for bad in ["Checks:20", "checks:21", "checks:", "checks:20 "] {
            assert!(
                ViewFixture::decode_action(ACTION_DATA, bad).is_none(),
                "{bad:?}"
            );
        }
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Select(ViewKind::CheckList, Variant::A));
        f.update(FixtureAction::Data(ViewKind::CheckList, DataSet::Scale20));
        assert_eq!(f.dirty_fields(), ["view_check_list"]);
        assert_eq!(row_counts(&f, ViewKind::CheckList), (20, 20));
        f.update(FixtureAction::Data(ViewKind::CheckList, DataSet::Scale20));
        assert!(f.dirty_fields().is_empty());
        f.update(FixtureAction::Data(ViewKind::CheckList, DataSet::Sample));
        assert_eq!(row_counts(&f, ViewKind::CheckList), (5, 6));
        // カードの行は影響を受けない。
        assert_eq!(row_counts(&f, ViewKind::CardList), (3, 4));
    }

    #[test]
    fn チェックを選んでいないときの切り替えは_dirty_を積まない() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Data(ViewKind::CheckList, DataSet::Scale20));
        assert!(f.dirty_fields().is_empty());
        assert_eq!(row_counts(&f, ViewKind::CheckList), (20, 20));
    }

    #[test]
    fn アプリのデモの_a_の後は本体を通り_部品と内側の印が出る() {
        let mut f = ViewFixture::load().unwrap();
        f.update(FixtureAction::Select(ViewKind::AppDemo, Variant::A));
        f.update(FixtureAction::Phase(Phase::After));
        let html = render(&f.view());
        assert_eq!(html.matches("fw-wire-tag ").count(), 2, "{html}");
        assert_eq!(html.matches("fw-wire-button ").count(), 1, "{html}");
        assert_eq!(html.matches("data-mark=\"change\"").count(), 3, "{html}");
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
