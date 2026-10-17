//! 世界状態とイベント列から、左の成果物ビューの行の前後の組を導く（TASK-468 1/2。ART-2・ART-3）。
//!
//! 呼び出し元は、各ビュー本体（TASK-473 以降）と、製品の `view()` への結線（TASK-470）の予定である。
//! `artifact_view` の契約（版 1）は行の型と印の規則（`Row::shown`）だけを決めており、世界状態から
//! 行を作る処理は持たない。本モジュールはその隙間を埋める純関数で、契約の型・綴り・対応は変えない。
//!
//! # 導き方
//!
//! 前の世界状態 `before` に、同じ操作のイベント列の `Changed` を `replay_changes` で当てて後の世界状態を
//! 作る（P-1b）。イベント列を web で独自に解釈しない。両方の鍵の和集合を経路の辞書順に走査し、
//! 呼び出し側の読み方 `read` で要素を行の中身に変える。並びは仮置きで、ビュー本体が並べ替えてよい。
//! 印は `Row::shown` と `mark_value` が導くため、本モジュールは印の規則を持たない。
//!
//! # 指せる行の規則
//!
//! 本節は `derive_rows`（6 種）の規則である。文書の行は `artifact_doc_rows` が「文書の経路 + `#` + 位置」から ID を合成する。
//!
//! `RowId` は世界状態の経路そのものにする。`PointTarget::to_input` は経路をそのまま `Input::Ui` の
//! `path` に入れるため、別の値にすると指した入力が世界状態の経路と食い違う。導出した行はすべて
//! `PointTarget::new(<ビューの ID>, row.id().clone())` で指せる。後の状態で前の中身を残して出す削除の行も指せる。
//! 導出以外で合成する行（ファイルツリーの途中の行など）は指せず、本モジュールは行を合成しない。
//! `PointTarget::new` が上限超過で拒否したときの扱いは描画側（TASK-469・TASK-470）が決め、導出は行を落とさない。
//!
//! # 境界（前後の世界状態の差を正とし、`Projection` の印には依らない）
//!
//! - 同じイベント列の中で値を変えて元へ戻した経路は、前後が同じなので印なし（`Projection` は `AttrChanged`）
//! - 同じイベント列の中で置いてから消した経路は、前後とも無いので行にしない（`Projection` は `Removed`）
//! - `read` が読まない属性だけが変わった経路は、中身が同じなので印なし（D-49 の行単位の印）
//!
//! # 範囲外
//!
//! 属性名の読み方（`label`・`state` など）とビューの選別は呼び出し側の `read` が持つ（各ビュー本体と Phase 4 の決定）。
//! 文書のブロックの突き合わせ（変更を `-` と `+` の 2 行にする処理）は `artifact_doc_rows` が持つ（TASK-468 2/2）。
//! 基準点（`Entered`・`Stopped`・`Moved`・`Restored`・`Begun`）で「前」を取り直す規則は spec に無く、TASK-470・TASK-347 の結線が決める。
//! 増分（`PhaseWorlds`）は、呼び出し側が渡した固定の「前」に対してログを伸ばすことだけを指す。
//!
//! # 増分と一括
//!
//! 右のログ・復習の `Projection` と同じく、増分の口（`PhaseWorlds::extend`）はログの全体を受け、処理済みの先頭を
//! 読み飛ばす。一括の口（`derive_rows`・`derive_doc_rows`）と増分の口は、前後の世界状態から行を作る同じ関数
//! （`rows_between`・`doc_rows_between`）を通すため、`replay_changes` が絶対値を当てることと合わせて、分け方によらず結果が一致する。
//!
//! # 信頼境界
//!
//! 経路と属性の値は教材・利用者由来で信頼できない。運ぶだけで、属性名・クラス名・HTML には使わない。
//! 本モジュールは `Node` を作らない。誤りの文面は固定の英語で、入力値を埋め込まない。
//! 全域（panic しない）で決定的で、計算量は経路の数に線形である。

use std::collections::BTreeSet;
use std::fmt;

use fandhe_workshop_core::changed::replay_changes;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::world::{Element, World};

use crate::a11y::ChangeMark;
use crate::artifact_doc_rows::doc_rows_between;
use crate::artifact_view::{
    check_unique_ids, ArtifactViewError, DocRow, Phase, Row, RowId, RowKind,
};

/// 要素 1 つから作る行の部品。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowParts<T> {
    /// 木の深さ。
    pub depth: u8,
    /// 行の種類。
    pub kind: RowKind,
    /// 行の中身。
    pub content: T,
}

/// 導出の誤り。文面は固定の英語で、入力値を含まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeriveError {
    /// 要素の形がビューの読み方に合わない（呼び出し側の `read` が返す）。
    Element,
    /// 行の検査に通らない（空の ID・重複した ID など）。
    Row(ArtifactViewError),
    /// 文書のブロックが多すぎて突き合わせられない（計算量の上限。黙って別の突き合わせに退避しない）。
    TooLarge,
}

impl fmt::Display for DeriveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeriveError::Element => f.write_str("element does not match the view reader"),
            DeriveError::Row(_) => f.write_str("row is invalid"),
            DeriveError::TooLarge => f.write_str("document is too large to align"),
        }
    }
}

impl std::error::Error for DeriveError {}

impl From<ArtifactViewError> for DeriveError {
    fn from(e: ArtifactViewError) -> Self {
        DeriveError::Row(e)
    }
}

/// 前の世界状態と、同じ操作のイベント列から、1 つのビューの行を導く。
///
/// `read(経路, 要素)` は、要素をそのビューの行の部品にする。そのビューに出さない要素は `Ok(None)`、
/// 形が合わなければ `Err`。前後で別々に呼ぶため、ビューの外から入った要素は追加、外へ出た要素は削除になる。
/// 1 件でも `Err` なら全体を `Err` で返し、行を黙って落とさない。
pub fn derive_rows<T, F>(
    before: &World,
    events: &[Event],
    read: F,
) -> Result<Vec<Row<T>>, DeriveError>
where
    T: PartialEq,
    F: Fn(&str, &Element) -> Result<Option<RowParts<T>>, DeriveError>,
{
    let mut after = before.clone();
    replay_changes(&mut after, events);
    rows_between(before, &after, read)
}

/// 前後の世界状態から行を作る。一括と増分が共有する（どちらも同じ関数を通す）。
fn rows_between<T, F>(before: &World, after: &World, read: F) -> Result<Vec<Row<T>>, DeriveError>
where
    T: PartialEq,
    F: Fn(&str, &Element) -> Result<Option<RowParts<T>>, DeriveError>,
{
    let paths: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    let mut rows = Vec::new();
    for path in paths {
        let b = match before.get(path) {
            Some(el) => read(path, el)?,
            None => None,
        };
        let a = match after.get(path) {
            Some(el) => read(path, el)?,
            None => None,
        };
        // 深さと種類は、後があれば後から、無ければ前から取る。
        let (depth, kind) = match (&a, &b) {
            (Some(p), _) | (None, Some(p)) => (p.depth, p.kind),
            (None, None) => continue,
        };
        rows.push(Row::new(
            RowId::new(path)?,
            depth,
            kind,
            b.map(|p| p.content),
            a.map(|p| p.content),
        )?);
    }
    check_unique_ids(&rows)?;
    Ok(rows)
}

/// 固定の「前」の世界状態と、ログを少しずつ当てた「後」の世界状態を持つ増分の口。
///
/// `Projection::extend` と同じ契約で、`events` はログの全体を渡す。処理済みの先頭は読み飛ばし、
/// 短い列では何もしない（`consumed` は縮めない）。呼び出し元は結線（TASK-470）で、右のログ・復習と同じイベント列を渡す。
#[derive(Debug, Clone)]
pub struct PhaseWorlds {
    before: World,
    after: World,
    consumed: usize,
}

impl PhaseWorlds {
    /// 前の世界状態から始める。後は前の写しで、処理済みは 0 件。
    pub fn new(before: World) -> Self {
        let after = before.clone();
        PhaseWorlds {
            before,
            after,
            consumed: 0,
        }
    }

    /// 未処理の分だけ後の世界状態に当てる。
    pub fn extend(&mut self, events: &[Event]) {
        if let Some(rest) = events.get(self.consumed..) {
            replay_changes(&mut self.after, rest);
        }
        self.consumed = self.consumed.max(events.len());
    }

    /// 前の世界状態。
    pub fn before(&self) -> &World {
        &self.before
    }

    /// 後の世界状態。
    pub fn after(&self) -> &World {
        &self.after
    }

    /// 処理済みのイベントの数。
    pub fn consumed(&self) -> usize {
        self.consumed
    }

    /// 6 種のビューの行。`derive_rows` と同じ結果になる。
    pub fn rows<T, F>(&self, read: F) -> Result<Vec<Row<T>>, DeriveError>
    where
        T: PartialEq,
        F: Fn(&str, &Element) -> Result<Option<RowParts<T>>, DeriveError>,
    {
        rows_between(&self.before, &self.after, read)
    }

    /// 文書のブロックの行。`derive_doc_rows` と同じ結果になる。
    pub fn doc_rows<B, F>(&self, doc_path: &str, read: F) -> Result<Vec<DocRow<B>>, DeriveError>
    where
        B: PartialEq + Clone,
        F: Fn(&Element) -> Result<Vec<B>, DeriveError>,
    {
        doc_rows_between(&self.before, &self.after, doc_path, read)
    }
}

/// ある状態の行の数と印の数。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MarkCounts {
    /// 出る行の数。
    pub rows: usize,
    /// 追加の印の数。
    pub add: usize,
    /// 削除の印の数。
    pub remove: usize,
    /// 変更の印の数。
    pub change: usize,
}

impl MarkCounts {
    /// `Row::shown(phase)` だけから数える。印の規則は契約に任せ、ここでは再定義しない。
    pub fn of<T: PartialEq>(rows: &[Row<T>], phase: Phase) -> MarkCounts {
        let mut c = MarkCounts::default();
        for shown in rows.iter().filter_map(|r| r.shown(phase)) {
            c.rows += 1;
            match shown.mark {
                Some(ChangeMark::Add) => c.add += 1,
                Some(ChangeMark::Remove) => c.remove += 1,
                Some(ChangeMark::Change) => c.change += 1,
                None => {}
            }
        }
        c
    }
}
