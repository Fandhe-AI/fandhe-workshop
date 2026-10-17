//! 左の成果物エリアのビューに共通する型と、変化の印の対応（TASK-467 1/2。契約の版 1）。
//!
//! 左エリアの 7 種のビュー（TASK-468〜TASK-481）が、同じ行データ・同じ印の定義を参照するための契約である。
//! 出所は spec の ART-2（前後の切り替え）・ART-3（印の 3 手段）・ART-5（ビューの種類）・ART-11（指せる行）、
//! PoC-8 の「本体の `view()` へ組み込むときの手順と注意」の 2・3、D-49（印は行単位）。
//!
//! # 固定するもの
//!
//! - ビューの種類（`ViewKind`）と、Step が宣言するビューとの対応（`declared_kinds`）
//! - 行データ（`Row`）。文書の本文の木のブロックは型パラメータで受け、具体の型は後続のタスクが結ぶ
//! - 世界状態の変化の印の対応（`change_mark_of`）と、前後の組から印を導く規則（`Row::shown`）
//! - 7 種のビューの keyed list の束縛点の名前（`ViewKind::list_field`・`VIEW_LIST_FIELDS`）と、親要素の属性
//!   （`list_attrs`・`view_list`。TASK-467 2/2）。7 本は `PartKind::ArtifactView` の 1 部品が持つ
//!   （`PartRegistry` は種類ごとに 1 部品だけを受け付けるため）
//! - 指せる行の属性の組（`PointAttrs`）と、`data-payload` の符号化・復号（`PointTarget`。TASK-467 2/2）
//! - 変種 (a)・(b) を選ぶ口（`Variant`・`VariantPolicy`。TASK-467 2/2）
//!
//! 印の記号・文字・`data-mark` の値の正は `a11y::ChangeMark` にあり、ここでは再定義しない。
//!
//! # 固定しないもの
//!
//! - 親要素の `aria-label`（`a11y` の閉じた一覧にラベルの文字列が無く、TASK-470 でも付けていない。付けるには a11y の契約の版を上げる）。ツリーの
//!   `role=tree` は `tree_view_list` だけが付け（a11y の版 2。TASK-472）、他の種類の親には `role` を付けない
//! - ART-16 の使い分けの規則（PoC-8 の「(a) はアプリのデモとカードに限る」案も固定しない）
//! - 変種ごとの `data-mark` の付け先と印の部品（`artifact_mark`。TASK-469）。どの行を指せるかは `artifact_rows` が決める（TASK-468）
//! - keyed list のキーの合計バイト数（上流の安全弁）に収める工夫（TASK-469 以降）
//! - host の `decode_action` への `ui` の結線（TASK-347 の後続）
//! - 宣言の無い Step の既定のビュー（試作の仮置きを `view_select::default_view` に置いた。TASK-470・TASK-76・TASK-496）
//! - 世界状態から行データを導く処理と、`MarkKind` からの対応と前後比較との照合（`artifact_rows` が持つ。TASK-468）
//! - 欄単位の差。D-49 で持たないと決めた。採る場合は版上げで足す
//!
//! # 信頼境界
//!
//! `RowId` と `Cell` の文字列は教材由来で信頼できない。運ぶだけで、属性名・クラス名・HTML には使わない
//! （エスケープは描画側の責務）。属性名・クラス名・`data-testid`・`id` は、閉じた集合の `&'static str` だけを使う。
//! 教材由来の文字列が属性の値に入るのは `data-row`・`data-payload` の 2 つだけで、`render` の既定の
//! エスケープを通す。生の HTML の経路は使わない（SAFE-11。`engine_parts` の `data-path` と同じ扱い）。
//!
//! 版は `ARTIFACT_VIEW_CONTRACT_VERSION` が持つ。`parts::PARTS_CONTRACT_VERSION`・
//! `a11y::A11Y_CONTRACT_VERSION` とは独立で、TASK-467 2/2 の追加は同じ版 1 の中に足す。
//! 型・綴り・対応を変えるときは、先に spec を直し、版を上げる。
//!
//! 後続のビュー（TASK-468〜TASK-481）の担当は、`crates/web/tests/artifact_view_sample.rs` のサンプルを
//! 真似る（7 本の keyed list を 1 部品に置く・親の属性は `view_list`・行の宛先は `PointAttrs`・
//! 変種は `VariantPolicy`）。契約適合試験は `crates/web/tests/artifact_view_contract.rs`。

use std::collections::HashSet;
use std::fmt;

use fandhe_frontend_core::Node;
use fandhe_workshop_core::input::Input;
use fandhe_workshop_core::projection::MarkKind;

use crate::a11y::{ChangeMark, MarkValue, TESTID_ATTR};
use crate::list_policy::{keyed_view, ListError};
use crate::parts::{FieldKind, PartField};

/// 左の成果物ビューの契約の版。
pub const ARTIFACT_VIEW_CONTRACT_VERSION: u32 = 1;

/// 本契約の誤り。文面は固定の英語で、入力値を埋め込まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactViewError {
    /// 行の `id` が空。
    EmptyRowId,
    /// 前後の両方が無い行。
    EmptyRow,
    /// 1 つのビューの中で `id` が重なっている。
    DuplicateRowId,
    /// ビューの識別子が空。
    EmptyViewId,
    /// 指せる行の payload が上限を超える。
    PayloadTooLong,
    /// そのビューの種類が許さない変種。
    VariantNotAllowed,
    /// 変種の指定で同じ種類が重なっている。
    DuplicateViewKind,
}

impl fmt::Display for ArtifactViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArtifactViewError::EmptyRowId => f.write_str("empty row id"),
            ArtifactViewError::EmptyRow => f.write_str("row has neither before nor after"),
            ArtifactViewError::DuplicateRowId => f.write_str("duplicate row id"),
            ArtifactViewError::EmptyViewId => f.write_str("empty view id"),
            ArtifactViewError::PayloadTooLong => f.write_str("point payload too long"),
            ArtifactViewError::VariantNotAllowed => {
                f.write_str("variant not allowed for view kind")
            }
            ArtifactViewError::DuplicateViewKind => f.write_str("duplicate view kind"),
        }
    }
}

impl std::error::Error for ArtifactViewError {}

/// ビューの種類。
///
/// `card_list` は語彙の拡張点 EP-5 で固定済み。残りの綴りは、語彙の文書が「画面の契約が持つ」と
/// 委ねた値で、本契約が持ち主になる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewKind {
    /// ファイルツリー。
    FileTree,
    /// 文書。
    Document,
    /// 差分。
    Diff,
    /// タスクツリー。
    TaskTree,
    /// カードの一覧。
    CardList,
    /// チェックの一覧。
    CheckList,
    /// アプリのデモ。
    AppDemo,
}

impl ViewKind {
    /// 全種類（標準の並び）。
    pub const ALL: [ViewKind; 7] = [
        ViewKind::FileTree,
        ViewKind::Document,
        ViewKind::Diff,
        ViewKind::TaskTree,
        ViewKind::CardList,
        ViewKind::CheckList,
        ViewKind::AppDemo,
    ];

    /// 語彙の綴り。
    pub const fn as_str(self) -> &'static str {
        match self {
            ViewKind::FileTree => "file_tree",
            ViewKind::Document => "document",
            ViewKind::Diff => "diff",
            ViewKind::TaskTree => "task_tree",
            ViewKind::CardList => "card_list",
            ViewKind::CheckList => "check_list",
            ViewKind::AppDemo => "app_demo",
        }
    }

    /// 綴りから変換する。完全一致だけで、一覧外（大文字小文字違いを含む）は `None`。
    /// 呼び出し側は `None` を拒否する（fail-closed）。
    pub fn parse(s: &str) -> Option<ViewKind> {
        ViewKind::ALL.iter().copied().find(|k| k.as_str() == s)
    }

    /// keyed list の束縛点（field）の名前。`view_` に語彙の綴りを続ける（ART-5・FEND-15）。
    ///
    /// `parts` の field 名の規則を満たし、他の部品の field 名と重ならない。
    pub const fn list_field(self) -> &'static str {
        match self {
            ViewKind::FileTree => "view_file_tree",
            ViewKind::Document => "view_document",
            ViewKind::Diff => "view_diff",
            ViewKind::TaskTree => "view_task_tree",
            ViewKind::CardList => "view_card_list",
            ViewKind::CheckList => "view_check_list",
            ViewKind::AppDemo => "view_app_demo",
        }
    }

    /// 束縛点の宣言（keyed list）。
    pub const fn list_part_field(self) -> PartField {
        PartField {
            name: self.list_field(),
            kind: FieldKind::KeyedList,
        }
    }

    /// ツリー（ファイルツリー・タスクツリー）か。変種 (b) で `role=tree` を付ける対象（ART-9）。
    pub const fn is_tree(self) -> bool {
        matches!(self, ViewKind::FileTree | ViewKind::TaskTree)
    }

    /// 変種 `v` をこの種類が許すか。文書・差分は (b) だけ（ART-7・ART-8）。
    pub const fn allows(self, v: Variant) -> bool {
        !matches!((self, v), (ViewKind::Document | ViewKind::Diff, Variant::A))
    }
}

/// 7 種の束縛点の宣言（`ViewKind::ALL` の順）。`PartKind::ArtifactView` の 1 部品の `fields()` に使う。
pub const VIEW_LIST_FIELDS: [PartField; 7] = [
    ViewKind::FileTree.list_part_field(),
    ViewKind::Document.list_part_field(),
    ViewKind::Diff.list_part_field(),
    ViewKind::TaskTree.list_part_field(),
    ViewKind::CardList.list_part_field(),
    ViewKind::CheckList.list_part_field(),
    ViewKind::AppDemo.list_part_field(),
];

/// 親要素の `data-testid` の値。
pub const VIEW_LIST_TESTID: &str = "view-list";
/// 親要素の、ビューの種類を示す属性名。値は `ViewKind::as_str`。
pub const VIEW_ATTR: &str = "data-view";
/// 選ばれていないビューを隠す属性名。
pub const HIDDEN_ATTR: &str = "hidden";

/// 親要素の属性。閉じた集合の値だけで、教材由来の値を入れない。
///
/// `role`・`aria-label` は付けない。ツリーの `role=tree` は [`tree_list_attrs`] が足す。
/// 選ばれていないビューにだけ `hidden=""` を足す（ART-12）。
pub fn list_attrs(kind: ViewKind, selected: bool) -> Vec<(&'static str, &'static str)> {
    let mut attrs = vec![(TESTID_ATTR, VIEW_LIST_TESTID), (VIEW_ATTR, kind.as_str())];
    if !selected {
        attrs.push((HIDDEN_ATTR, ""));
    }
    attrs
}

/// ビューの keyed list の親要素を作る。親の属性は必ずここ（つまり `view()`）で書く（FEND-15）。
///
/// 選ばれていないビューは `items` を捨てて空の一覧にする（ART-12）。`Err` は呼び出し側へ返し、
/// 黙って別の形に退避しない。
///
/// # Errors
///
/// 上流の安全弁（項目数・キーの重複など）が拒否した場合は `ListError`。
pub fn view_list(
    tag: &'static str,
    kind: ViewKind,
    selected: bool,
    items: Vec<(String, Node)>,
) -> Result<Node, ListError> {
    let items = if selected { items } else { Vec::new() };
    keyed_view(
        tag,
        list_attrs(kind, selected),
        &kind.list_part_field(),
        items,
    )
}

/// 親要素の属性に、ツリーの `role=tree` を足したもの（TASK-472。ART-9）。
///
/// [`list_attrs`] の結果に、次の 4 条件がすべて成り立つときだけ `role=tree` を足す。
/// 種類がツリー、変種が (b)、選ばれている、項目が 1 件以上ある。(a) は対照として `role` を付けない。
/// 項目が 0 件のときに付けないのは、子の無い `role=tree` が ARIA の必須の子に反するため。
/// `aria-label` は付けない（`a11y` の閉じた一覧にラベルの文字列が無い。TASK-470 の範囲）。
pub fn tree_list_attrs(
    kind: ViewKind,
    variant: Variant,
    selected: bool,
    has_items: bool,
) -> Vec<(&'static str, &'static str)> {
    let mut attrs = list_attrs(kind, selected);
    if kind.is_tree() && variant == Variant::B && selected && has_items {
        attrs.push(("role", crate::tree_attrs::TREE_ROLE));
    }
    attrs
}

/// ツリーのビューの keyed list の親要素を作る。親の属性は必ずここ（つまり `view()`）で書く（FEND-15）。
///
/// [`view_list`] と同じく、選ばれていないビューは `items` を捨てて空の一覧にする。`Err` は呼び出し側へ返す。
///
/// # Errors
///
/// 上流の安全弁（項目数・キーの重複など）が拒否した場合は `ListError`。
pub fn tree_view_list(
    tag: &'static str,
    kind: ViewKind,
    variant: Variant,
    selected: bool,
    items: Vec<(String, Node)>,
) -> Result<Node, ListError> {
    let items = if selected { items } else { Vec::new() };
    keyed_view(
        tag,
        tree_list_attrs(kind, variant, selected, !items.is_empty()),
        &kind.list_part_field(),
        items,
    )
}

/// 再描画で dirty にする束縛点（PoC-8 の手順 4）。ビューが変わったら旧と新の両方、
/// 同じビューで中身が変わったらそのビュー、それ以外は空。
pub fn dirty_list_fields(
    prev: Option<ViewKind>,
    next: Option<ViewKind>,
    content_changed: bool,
) -> Vec<&'static str> {
    if prev != next {
        return [prev, next]
            .into_iter()
            .flatten()
            .map(ViewKind::list_field)
            .collect();
    }
    match next {
        Some(k) if content_changed => vec![k.list_field()],
        _ => Vec::new(),
    }
}

/// 指せる行の `data-action` の値。
pub const POINT_ACTION: &str = "ui";
/// 指せる行の属性名（action）。
pub const ACTION_ATTR: &str = "data-action";
/// 指せる行の属性名（payload）。
pub const PAYLOAD_ATTR: &str = "data-payload";
/// 指せる行の属性名（行の識別子）。
pub const ROW_ATTR: &str = "data-row";
/// 指せる行の属性名（フォーカス）。`view()` に静的に書き、束縛では付けない。
pub const TABINDEX_ATTR: &str = "tabindex";
/// 指せる行の `tabindex` の値。
pub const POINTABLE_TABINDEX: &str = "0";
/// payload の上限（バイト）。符号化・復号のどちらも、超えたものを拒否する。
pub const MAX_POINT_PAYLOAD_BYTES: usize = 4_096;
/// payload の長さ部分の最大の桁数。
const MAX_LEN_DIGITS: usize = 4;

/// 指せる行の宛先。`view` は教材の `views` 表の識別子、`row` は行の識別子で、どちらも信頼できない値。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointTarget {
    view: String,
    row: RowId,
}

impl PointTarget {
    /// 検査して作る。`view` が空、または符号化した長さが上限を超える場合は拒否する。
    pub fn new(view: &str, row: RowId) -> Result<Self, ArtifactViewError> {
        if view.is_empty() {
            return Err(ArtifactViewError::EmptyViewId);
        }
        let t = Self {
            view: view.to_owned(),
            row,
        };
        if t.encode_payload().len() > MAX_POINT_PAYLOAD_BYTES {
            return Err(ArtifactViewError::PayloadTooLong);
        }
        Ok(t)
    }

    /// ビューの識別子。
    pub fn view(&self) -> &str {
        &self.view
    }

    /// 行の識別子。
    pub fn row(&self) -> &RowId {
        &self.row
    }

    /// `data-payload` の値。`<view のバイト数>:<view><row>` の長さ前置きで、`view` の文字種を制約せず単射にする。
    pub fn encode_payload(&self) -> String {
        format!("{}:{}{}", self.view.len(), self.view, self.row.as_str())
    }

    /// action の名前と payload から復元する。正規形だけを受け、それ以外は `None`（fail-closed）。
    /// どの入力でも panic しない。
    pub fn decode(name: &str, payload: &str) -> Option<PointTarget> {
        if name != POINT_ACTION || payload.len() > MAX_POINT_PAYLOAD_BYTES {
            return None;
        }
        let (digits, rest) = payload.split_once(':')?;
        if !is_canonical_len(digits) {
            return None;
        }
        let n: usize = digits.parse().ok()?;
        let view = rest.get(..n)?;
        let row = rest.get(n..)?;
        if view.is_empty() || row.is_empty() {
            return None;
        }
        Some(PointTarget {
            view: view.to_owned(),
            row: RowId(row.to_owned()),
        })
    }

    /// core の入力へ変換する。host の `decode_action` への結線は後続の範囲。
    pub fn to_input(&self) -> Input {
        Input::Ui {
            view: self.view.clone(),
            path: self.row.as_str().to_owned(),
        }
    }

    /// 行へ付ける属性の組。
    pub fn attrs(&self) -> PointAttrs {
        PointAttrs {
            payload: self.encode_payload(),
            row: self.row.as_str().to_owned(),
        }
    }
}

/// 長さ部分が正規の 10 進（ASCII 数字だけ・先頭 0 なし・桁数の上限内）か。
fn is_canonical_len(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_LEN_DIGITS
        && s.bytes().all(|b| b.is_ascii_digit())
        && !s.starts_with('0')
}

/// 指せる行の属性の組。4 つは常に組で付け、一部だけを付けない。指せない行には 1 つも付けない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointAttrs {
    payload: String,
    row: String,
}

impl PointAttrs {
    /// `data-action`・`data-payload`・`data-row`・`tabindex` の順の組。値は `render` がエスケープする。
    pub fn pairs(&self) -> [(&'static str, &str); 4] {
        [
            (ACTION_ATTR, POINT_ACTION),
            (PAYLOAD_ATTR, self.payload.as_str()),
            (ROW_ATTR, self.row.as_str()),
            (TABINDEX_ATTR, POINTABLE_TABINDEX),
        ]
    }
}

/// 変種。A は `wireframe-ui` の部品、B は `headless-ui` と自作の組み合わせ（ART-1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variant {
    /// `wireframe-ui` の部品。
    A,
    /// `headless-ui` と自作の組み合わせ。
    B,
}

impl Variant {
    /// 全種類。
    pub const ALL: [Variant; 2] = [Variant::A, Variant::B];
    /// 指定が無いときの変種。
    pub const DEFAULT: Variant = Variant::B;

    /// 綴り。
    pub const fn as_str(self) -> &'static str {
        match self {
            Variant::A => "a",
            Variant::B => "b",
        }
    }
}

/// 変種を選ぶ口。ホストが持ち、教材パックからは選ばせない。許されない組と重複は構築時に拒否する。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VariantPolicy {
    overrides: Vec<(ViewKind, Variant)>,
}

impl VariantPolicy {
    /// 検査して作る。
    ///
    /// # Errors
    ///
    /// 種類が許さない変種は `VariantNotAllowed`、同じ種類の重複は `DuplicateViewKind`。
    pub fn new(overrides: &[(ViewKind, Variant)]) -> Result<Self, ArtifactViewError> {
        let mut out: Vec<(ViewKind, Variant)> = Vec::new();
        for &(k, v) in overrides {
            if !k.allows(v) {
                return Err(ArtifactViewError::VariantNotAllowed);
            }
            if out.iter().any(|(o, _)| *o == k) {
                return Err(ArtifactViewError::DuplicateViewKind);
            }
            out.push((k, v));
        }
        Ok(Self { overrides: out })
    }

    /// 種類ごとの変種。指定が無ければ `Variant::DEFAULT`。
    pub fn variant_of(&self, kind: ViewKind) -> Variant {
        self.overrides
            .iter()
            .find(|(k, _)| *k == kind)
            .map_or(Variant::DEFAULT, |(_, v)| *v)
    }
}

/// Step の `views` の 1 件を、教材の `views` 表で解決した結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredView {
    /// 教材が付けたビューの識別子。信頼できない値で、運ぶだけにする。
    pub id: String,
    /// ビューの種類。
    pub kind: ViewKind,
}

/// 宣言の順を保ち、同じ種類は最初の 1 つだけ残す（各種類の最初のビュー）。
///
/// 宣言の無い Step の既定のビューは固定しない（TASK-470 の仮置き）。
pub fn declared_kinds(views: &[DeclaredView]) -> Vec<(ViewKind, &str)> {
    let mut out: Vec<(ViewKind, &str)> = Vec::new();
    for v in views {
        if !out.iter().any(|(k, _)| *k == v.kind) {
            out.push((v.kind, v.id.as_str()));
        }
    }
    out
}

/// 行の識別子。空でない文字列で、keyed list の鍵と指せる行の元になる。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RowId(String);

impl RowId {
    /// 検査して作る。
    pub fn new(s: &str) -> Result<Self, ArtifactViewError> {
        if s.is_empty() {
            return Err(ArtifactViewError::EmptyRowId);
        }
        Ok(Self(s.to_owned()))
    }

    /// 文字列。画面に出す場合は文字のノードだけにする（属性・HTML に埋め込まない）。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 行の種類。閉じた集合にして、属性へ入る値に任意の文字列を通さない。
///
/// 一覧は試作（PoC-8）の観察からの推論で、変えるときは版上げである。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowKind {
    /// ディレクトリ。
    Dir,
    /// ファイル。
    File,
    /// 差分のまとまり。
    Hunk,
    /// 差分の 1 行。
    Line,
    /// タスク。
    Task,
    /// カード。
    Card,
    /// チェック。
    Check,
    /// 見出し。
    Heading,
    /// タブ。
    Tab,
    /// 入力欄。
    Field,
    /// ボタン。
    Button,
    /// 項目。
    Item,
    /// 文字。
    Text,
    /// 文書の本文のブロック。
    Block,
}

impl RowKind {
    /// 全種類。
    pub const ALL: [RowKind; 14] = [
        RowKind::Dir,
        RowKind::File,
        RowKind::Hunk,
        RowKind::Line,
        RowKind::Task,
        RowKind::Card,
        RowKind::Check,
        RowKind::Heading,
        RowKind::Tab,
        RowKind::Field,
        RowKind::Button,
        RowKind::Item,
        RowKind::Text,
        RowKind::Block,
    ];

    /// 綴り。`data-kind` の値になりうる。
    pub const fn as_str(self) -> &'static str {
        match self {
            RowKind::Dir => "dir",
            RowKind::File => "file",
            RowKind::Hunk => "hunk",
            RowKind::Line => "line",
            RowKind::Task => "task",
            RowKind::Card => "card",
            RowKind::Check => "check",
            RowKind::Heading => "heading",
            RowKind::Tab => "tab",
            RowKind::Field => "field",
            RowKind::Button => "button",
            RowKind::Item => "item",
            RowKind::Text => "text",
            RowKind::Block => "block",
        }
    }

    /// 綴りから変換する。完全一致だけで、一覧外は `None`。
    pub fn parse(s: &str) -> Option<RowKind> {
        RowKind::ALL.iter().copied().find(|k| k.as_str() == s)
    }
}

/// 文書以外の 6 種のビューの行の中身。値は運ぶだけで、属性名・クラス名・HTML には使わない。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cell {
    /// ラベル。
    pub label: String,
    /// 補足。
    pub detail: String,
    /// 状態。
    pub state: String,
    /// タグ。
    pub tags: Vec<String>,
}

/// 前後の組を持つ行。差のある欄の一覧は持たない（D-49）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row<T> {
    id: RowId,
    depth: u8,
    kind: RowKind,
    before: Option<T>,
    after: Option<T>,
}

/// 文書以外のビューの行。
pub type ItemRow = Row<Cell>;
/// 文書のビューの行。`B` は本文の木のブロックで、具体の型は後続のタスクが結ぶ。
pub type DocRow<B> = Row<B>;

impl<T> Row<T> {
    /// 検査して作る。前後の両方が無い行は拒否する。
    pub fn new(
        id: RowId,
        depth: u8,
        kind: RowKind,
        before: Option<T>,
        after: Option<T>,
    ) -> Result<Self, ArtifactViewError> {
        if before.is_none() && after.is_none() {
            return Err(ArtifactViewError::EmptyRow);
        }
        Ok(Self {
            id,
            depth,
            kind,
            before,
            after,
        })
    }

    /// 行の識別子。
    pub fn id(&self) -> &RowId {
        &self.id
    }

    /// 木の深さ。
    pub fn depth(&self) -> u8 {
        self.depth
    }

    /// 行の種類。
    pub fn kind(&self) -> RowKind {
        self.kind
    }

    /// 前の状態の中身。
    pub fn before(&self) -> Option<&T> {
        self.before.as_ref()
    }

    /// 後の状態の中身。
    pub fn after(&self) -> Option<&T> {
        self.after.as_ref()
    }
}

/// 1 つのビューの行で `id` が重ならないことを確かめる（keyed list の鍵の重複なし）。
pub fn check_unique_ids<T>(rows: &[Row<T>]) -> Result<(), ArtifactViewError> {
    let mut seen = HashSet::new();
    for r in rows {
        if !seen.insert(r.id.as_str()) {
            return Err(ArtifactViewError::DuplicateRowId);
        }
    }
    Ok(())
}

/// 表示する状態。ART-2 の前後の切り替え。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// 前の状態。印は 0 件。
    Before,
    /// 後の状態。変化の行に印が付く。
    After,
}

/// ある状態で行を出すときの中身と印。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shown<'a, T> {
    /// 出す中身。
    pub content: &'a T,
    /// 印。無ければ `None`。
    pub mark: Option<ChangeMark>,
}

impl<T: PartialEq> Row<T> {
    /// 状態ごとの表示を返す。出さない行は `None`。
    ///
    /// 前は `before` がある行だけを印なしで出す。後は、追加は `Add`、削除は前の中身を残して `Remove`、
    /// 前後が違えば `Change`、同じなら印なしである。
    pub fn shown(&self, phase: Phase) -> Option<Shown<'_, T>> {
        match phase {
            Phase::Before => self.before.as_ref().map(|c| Shown {
                content: c,
                mark: None,
            }),
            Phase::After => match (&self.before, &self.after) {
                (None, Some(a)) => Some(Shown {
                    content: a,
                    mark: Some(ChangeMark::Add),
                }),
                (Some(b), None) => Some(Shown {
                    content: b,
                    mark: Some(ChangeMark::Remove),
                }),
                (Some(b), Some(a)) => Some(Shown {
                    content: a,
                    mark: if b != a {
                        Some(ChangeMark::Change)
                    } else {
                        None
                    },
                }),
                (None, None) => None,
            },
        }
    }
}

/// 世界状態の変化の印の種類から、画面の印への対応（ART-3）。
///
/// 未知の種類は黙って別の印に寄せず `None` にし、呼び出し側に扱いを決めさせる。
/// core に種類が増えたときは、本契約の版上げが要る。
pub fn change_mark_of(kind: MarkKind) -> Option<ChangeMark> {
    match kind {
        MarkKind::Put => Some(ChangeMark::Add),
        MarkKind::Removed => Some(ChangeMark::Remove),
        MarkKind::AttrChanged => Some(ChangeMark::Change),
        _ => None,
    }
}

/// 印の出力の形。記号・文字・`data-mark` は `a11y::MarkValue` に委ね、値を再定義しない。
pub fn mark_value(mark: Option<ChangeMark>) -> MarkValue {
    match mark {
        Some(m) => MarkValue::Mark(m),
        None => MarkValue::None,
    }
}
