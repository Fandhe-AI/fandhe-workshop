//! イベントのログからの導出 [`Projection`]（ログ・変化の印・復習。TASK-204 2/3・TASK-215 2/2）。
//!
//! 呼び出し元は web（ログ欄・変化の印・復習の表示）。イベントだけから導出し、表示文字列を持たず、
//! 読まない（EVT-2・W-24）。
//!
//! # 導出の規則
//!
//! - ログ行: 行にするイベントは [`Projection::extend`] 内の `match` の表のとおり。行は `at` とログ上の位置だけを持ち、
//!   ログは追記だけで、復帰でも行を消さない。`Skipped`・`Flag`・`Begun`・`HintOpened` と `Changed` は行にしない
//!   （版 1 の画面に出さない・左の印で表す・状態であり出来事ではない）。`Accepted` は `Judged`（TASK-256）が
//!   未導入のため行にする。`Judged` を足す版上げのときに TASK-256 が見直す
//!   `Reloaded` は再読み込み 1 回につき 1 行にする（SAVE-6・D-57）。処理が続くので変化の印は消さない。
//!   `TransitionLimitReached` は上限で止めるたびに 1 行にする（SAFE-34）
//! - 変化の印: 基準点（`Entered`・`Stopped`・`Moved`・`Restored`・`Begun`）の後に、原因が `Input` の `Changed` が触れた
//!   経路を、（最初の変更前, 最後の変更後）から種類を決めて出す。最後の変化が削除なら `Removed` に倒す
//! - 復習: Step ごとに最初に現れた順で並べ、経路ごとに最初の変更前・最後の変更後を持つ。原因が `Input` でない
//!   `Changed`（停止・移動・復帰・開始の巻き戻し）は学習者の変化ではないため入れない。`Stopped` は Step の入口へ
//!   戻すため、その Step の項目を初期化する
//! - 復帰（EVT-14）: `CheckpointSaved` の時点の復習をイベントから導いて控え、`Restored` で持ち主の根の項目だけを
//!   控えへ戻す。`SessionState.checkpoints` の写しは読まない（EVT-13）。`log_len` が保存の位置を指さない不正な
//!   ログでは、持ち主の根の項目を空にして他の根の項目だけを残す（panic せず決定的）。再帰で作り直さないのは、
//!   保存から読んだログが信頼できない入力であり、入れ子の復帰で計算量が爆発しないようにするため（A04）。
//!   `apply()` が出すログでの確認は `tests/restore_review.rs`（TASK-214 2/3）
//!
//! # 不変条件
//!
//! - 全域で決定的。I/O・時刻・乱数を使わない。本番コードで `unwrap`・`expect`・範囲外の添字を使わない
//! - 増分（1 件ずつ・操作ごと）と一括の結果が一致する（EVT-1）。どちらも同じ `step` を通る
//! - 経路・値は教材・利用者由来で信頼できない。値は運ぶだけで、HTML へのエスケープは web 層の責務
//!
//! 確認の欄は判定の型（TASK-256）の後の版上げで足す。

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::addr::{Stamp, StepUid};
use crate::event::{Cause, Event};
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
    /// 復習の項目。全体を `Arc` で持ち、保存の控えと共有する（保存は参照の複製だけで、変更時に初めて複製する）。
    review: Arc<Vec<ReviewItem>>,
    /// 変化の印の元。基準点からの経路ごとの（最初の変更前, 最後の変更後）。
    pending: BTreeMap<String, (Option<Element>, Option<Element>)>,
    /// 保存の時点の復習。鍵は `CheckpointSaved` のログ上の位置（その `log_len`）。現在の復習と共有し、
    /// 保存そのものは参照の複製に限る（復習を変えない保存の連続で全体を複製しない）。
    saved_review: BTreeMap<usize, Arc<Vec<ReviewItem>>>,
    /// 直近の操作の先頭（`InputReceived`・`Progress`・`Moved`）の時点の復習と印の元。
    /// 上限で止めた操作を操作の前へ戻す（SAFE-35）ので、`TransitionLimitReached` で復習と印の元をここへ戻す。
    op_start: Option<OpStart>,
}

/// 操作の先頭で控える復習と印の元。
type OpStart = (
    Arc<Vec<ReviewItem>>,
    BTreeMap<String, (Option<Element>, Option<Element>)>,
);

/// 変化の印の種類を（最初の変更前, 最後の変更後）から決める。
fn mark_kind(before: &Option<Element>, after: &Option<Element>) -> MarkKind {
    match (before, after) {
        (None, Some(_)) => MarkKind::Put,
        (_, None) => MarkKind::Removed,
        (Some(_), Some(_)) => MarkKind::AttrChanged,
    }
}

/// すべてのイベントが持つ `at` を取り出す。網羅の `match` で、variant の追加時に導出の扱いを決めさせる。
fn stamp_of(event: &Event) -> &Stamp {
    match event {
        Event::Entered { at }
        | Event::ModuleEntered { at, .. }
        | Event::ModuleExited { at, .. }
        | Event::InputReceived { at, .. }
        | Event::Progress { at, .. }
        | Event::Judged { at, .. }
        | Event::Accepted { at }
        | Event::Achieved { at, .. }
        | Event::Skipped { at }
        | Event::Transitioned { at, .. }
        | Event::ChapterCompleted { at }
        | Event::Flag { at, .. }
        | Event::Changed { at, .. }
        | Event::Stopped { at, .. }
        | Event::Moved { at, .. }
        | Event::CheckpointSaved { at, .. }
        | Event::Restored { at, .. }
        | Event::Begun { at, .. }
        | Event::HintOpened { at, .. }
        | Event::Reloaded { at, .. }
        | Event::Refused { at, .. }
        | Event::Ignored { at, .. }
        | Event::TransitionLimitReached { at, .. }
        | Event::Warning { at, .. } => at,
    }
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
    ///
    /// 処理済みより短い列を渡されても何もしない（`consumed` は縮めない）。
    pub fn extend(&mut self, events: &[Event]) {
        for i in self.consumed..events.len() {
            if let Some(event) = events.get(i) {
                self.step(i, event);
            }
        }
        self.consumed = self.consumed.max(events.len());
        self.marks = self
            .pending
            .iter()
            .map(|(path, (before, after))| Mark {
                path: path.clone(),
                kind: mark_kind(before, after),
            })
            .collect();
    }

    /// イベント 1 件（ログ上の位置 `i`）を導出へ反映する。
    fn step(&mut self, i: usize, event: &Event) {
        // 操作の先頭の控え。上限で止めた操作の `Achieved`・`Changed` を復習から取り除くために使う。
        if matches!(
            event,
            Event::InputReceived { .. } | Event::Progress { .. } | Event::Moved { .. }
        ) {
            self.op_start = Some((self.review.clone(), self.pending.clone()));
        }
        match event {
            Event::Entered { .. } => {
                self.push_row(i, event);
                self.pending.clear();
            }
            Event::Begun { .. } => self.pending.clear(),
            Event::ModuleEntered { .. }
            | Event::ModuleExited { .. }
            | Event::InputReceived { .. }
            | Event::Progress { .. }
            | Event::Judged { .. }
            | Event::Accepted { .. }
            | Event::Transitioned { .. }
            | Event::ChapterCompleted { .. }
            | Event::Refused { .. }
            | Event::Ignored { .. }
            | Event::Warning { .. }
            | Event::Reloaded { .. } => self.push_row(i, event),
            Event::TransitionLimitReached { .. } => {
                self.push_row(i, event);
                // 止めた操作は操作の前の状態へ戻る（SAFE-35）。復習と印の元も操作の前へ戻し、Step の状態と揃える。
                // 先頭の控えが無い（操作の先頭を持たない列）ときは、何も戻さない。
                if let Some((review, pending)) = self.op_start.take() {
                    self.review = review;
                    self.pending = pending;
                }
            }
            Event::Achieved { at, .. } => {
                self.push_row(i, event);
                self.review_item_mut(&at.step).achieved = true;
            }
            Event::Skipped { at } => self.review_item_mut(&at.step).skipped = true,
            Event::Flag { .. } | Event::HintOpened { .. } => {}
            Event::Changed {
                at,
                path,
                before,
                after,
                cause,
            } => {
                if *cause == Cause::Input {
                    self.merge_change(&at.step, path, before, after);
                }
            }
            Event::Stopped { at, .. } => {
                self.push_row(i, event);
                self.pending.clear();
                if let Some(item) = Arc::make_mut(&mut self.review)
                    .iter_mut()
                    .find(|r| r.step == at.step)
                {
                    item.changes.clear();
                    item.achieved = false;
                    item.skipped = false;
                }
            }
            Event::Moved { .. } => {
                self.push_row(i, event);
                self.pending.clear();
            }
            Event::CheckpointSaved { .. } => {
                self.push_row(i, event);
                self.saved_review.insert(i, self.review.clone());
            }
            Event::Restored { log_len, owner, .. } => {
                self.push_row(i, event);
                self.pending.clear();
                self.restore_review(i, *log_len, &owner.root());
            }
        }
    }

    /// ログ行を足す。位置は `u32` に収まる範囲（履歴の大きさの上限から 2^32 件には届かない）だが、
    /// 全域性のため飽和させる。
    fn push_row(&mut self, i: usize, event: &Event) {
        self.log.push(LogRow {
            at: stamp_of(event).clone(),
            event: u32::try_from(i).unwrap_or(u32::MAX),
        });
    }

    /// `step` の復習の項目を返す。無ければ末尾に作る（項目の順は最初に現れた順）。
    fn review_item_mut(&mut self, step: &StepUid) -> &mut ReviewItem {
        let items = Arc::make_mut(&mut self.review);
        let pos = match items.iter().position(|r| &r.step == step) {
            Some(pos) => pos,
            None => {
                items.push(ReviewItem {
                    step: step.clone(),
                    changes: Vec::new(),
                    achieved: false,
                    skipped: false,
                });
                items.len() - 1
            }
        };
        &mut items[pos]
    }

    /// 学習者の変化を印の元と復習へ併合する。最初の変更前・最後の変更後を保つ。
    fn merge_change(
        &mut self,
        step: &StepUid,
        path: &str,
        before: &Option<Element>,
        after: &Option<Element>,
    ) {
        match self.pending.get_mut(path) {
            Some(entry) => entry.1 = after.clone(),
            None => {
                self.pending
                    .insert(path.to_string(), (before.clone(), after.clone()));
            }
        }
        let item = self.review_item_mut(step);
        match item.changes.iter_mut().find(|c| c.path == path) {
            Some(change) => change.after = after.clone(),
            None => item.changes.push(ReviewChange {
                path: path.to_string(),
                before: before.clone(),
                after: after.clone(),
            }),
        }
    }

    /// 復帰（EVT-14）。持ち主の根の項目を、保存の時点のものへ戻す。他の根の項目は変えない。
    /// 保存の位置を指さない `log_len`（範囲外・未来・不一致）は「写しなし」で、持ち主の根の項目を空にする。
    fn restore_review(&mut self, i: usize, log_len: u32, own: &crate::addr::Root) {
        let saved = usize::try_from(log_len)
            .ok()
            .filter(|at| *at < i)
            .and_then(|at| self.saved_review.get(&at));
        let mut restored: Vec<ReviewItem> = saved
            .map(|items| {
                items
                    .iter()
                    .filter(|r| &r.step.root() == own)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        restored.extend(
            self.review
                .iter()
                .filter(|r| &r.step.root() != own)
                .cloned(),
        );
        self.review = Arc::new(restored);
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
