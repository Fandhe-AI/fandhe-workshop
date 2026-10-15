//! イベントのログからの導出 [`Projection`]（ログ・変化の印・復習。TASK-204 2/3）。
//!
//! 呼び出し元は web（ログ欄・変化の印・復習の表示）。イベントだけから導出し、表示文字列を持たず、
//! 読まない（EVT-2・W-24）。導出の中身と「増分と一括の一致」は TASK-215 が実装する。
//! 本件は型と API の署名、および `consumed`（処理済みのイベント数）を進める骨格までを固定する。
//! 確認の欄は判定の型（TASK-256）の後の版上げで足す。

use serde::{Deserialize, Serialize};

use crate::addr::{Stamp, StepUid};
use crate::event::Event;
use crate::world::Element;

/// ログ 1 行。表示文字列は持たず、イベントへの位置だけを持つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogRow {
    /// その時点の世代と Step。
    pub at: Stamp,
    /// ログ上のイベントの位置。
    pub event: u32,
}

/// 変化の印の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MarkKind {
    /// 要素が置かれた。
    Put,
    /// 要素が無くなった。
    Removed,
    /// 属性が変わった。
    AttrChanged,
}

/// 変化の印。経路と種類だけを持つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mark {
    /// 変わった経路。
    pub path: String,
    /// 変化の種類。
    pub kind: MarkKind,
}

/// 復習の 1 件の変化（変更前後）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewChange {
    /// 変わった経路。
    pub path: String,
    /// 変更前（無ければ `None`）。
    pub before: Option<Element>,
    /// 変更後（無くなれば `None`）。
    pub after: Option<Element>,
}

/// 復習の Step 単位の項目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewItem {
    /// 対象の Step。
    pub step: StepUid,
    /// その Step での変化。
    pub changes: Vec<ReviewChange>,
    /// 達成したか。
    pub achieved: bool,
    /// 省略したか。
    pub skipped: bool,
}

/// ログからの導出結果。欄は読み出し口（accessor）で読ませ、中身の持ち方を後から変えられるようにする。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Projection {
    consumed: usize,
    log: Vec<LogRow>,
    marks: Vec<Mark>,
    review: Vec<ReviewItem>,
}

impl Projection {
    /// 空の導出結果。
    pub fn new() -> Self {
        Projection::default()
    }

    /// イベント列から一括で導出する。
    pub fn from_events(events: &[Event]) -> Self {
        let mut p = Projection::new();
        p.extend(events);
        p
    }

    /// 未処理の分だけ足す。`events` はログの全体で、処理済みの先頭は読み飛ばす。
    pub fn extend(&mut self, events: &[Event]) {
        // TASK-215: ログ行・変化の印・復習の導出。骨格は処理済みの数を進めるだけ。
        self.consumed = self.consumed.max(events.len());
    }

    /// 処理済みのイベント数。
    pub fn consumed(&self) -> usize {
        self.consumed
    }

    /// ログ欄の行。
    pub fn log(&self) -> &[LogRow] {
        &self.log
    }

    /// 変化の印。
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    /// 復習の項目。
    pub fn review(&self) -> &[ReviewItem] {
        &self.review
    }
}
