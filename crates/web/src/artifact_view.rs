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
//!
//! 印の記号・文字・`data-mark` の値の正は `a11y::ChangeMark` にあり、ここでは再定義しない。
//!
//! # 固定しないもの
//!
//! - keyed list の束縛点の名前・親要素の属性・指せる行の属性（TASK-467 2/2）
//! - 宣言の無い Step の既定のビュー（試作の仮置き。TASK-470・TASK-76・TASK-496）
//! - 世界状態から行データを導く処理と、`MarkKind` からの対応と前後比較との照合（TASK-468）
//! - 欄単位の差。D-49 で持たないと決めた。採る場合は版上げで足す
//!
//! # 信頼境界
//!
//! `RowId` と `Cell` の文字列は教材由来で信頼できない。運ぶだけで、属性名・クラス名・HTML には使わない
//! （エスケープは描画側の責務）。属性へ入る値は、閉じた enum の `&'static str` だけにする。
//!
//! 版は `ARTIFACT_VIEW_CONTRACT_VERSION` が持つ。`parts::PARTS_CONTRACT_VERSION`・
//! `a11y::A11Y_CONTRACT_VERSION` とは独立で、TASK-467 2/2 の追加は同じ版 1 の中に足す。
//! 型・綴り・対応を変えるときは、先に spec を直し、版を上げる。

use std::collections::HashSet;
use std::fmt;

use fandhe_workshop_core::projection::MarkKind;

use crate::a11y::{ChangeMark, MarkValue};

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
}

impl fmt::Display for ArtifactViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArtifactViewError::EmptyRowId => f.write_str("empty row id"),
            ArtifactViewError::EmptyRow => f.write_str("row has neither before nor after"),
            ArtifactViewError::DuplicateRowId => f.write_str("duplicate row id"),
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
