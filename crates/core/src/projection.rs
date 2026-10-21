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
//!   控えへ戻す。処理中かどうかと入口の控え（受付の時点の復習）も保存の時点のものへ戻す（core の復帰が `busy` を
//!   戻し、受付の時点を保存の時点から導くのと揃え、復帰後の上限停止で入口の時点へ戻せるようにする。SAFE-35）。
//!   `SessionState.checkpoints` の写しは読まない（EVT-13）。`log_len` が保存の位置を指さない不正なログでは、
//!   持ち主の根の項目を空にして他の根の項目だけを残し、処理中と入口の控えを無しに倒す（panic せず決定的）。
//!   再帰で作り直さないのは、保存から読んだログが信頼できない入力であり、入れ子の復帰で計算量が爆発しないように
//!   するため（A04）。
//!   `apply()` が出すログでの確認は `tests/restore_review.rs`（TASK-214 2/3）
//!
//! # 操作の先頭・入口の控えと取り消し差分
//!
//! 上限で止めた操作（SAFE-35）の復習と印の元を戻すため、操作の先頭（`InputReceived`・`Progress`・`Moved`）と
//! 入口（受付の `Accepted`）の時点を控える。控えは復習・印の元の写しではなく、取り消し差分（undo log）の長さで
//! 持つ。復習と印の元を変える箇所はすべて、変える前の値（項目の追加・変化の追加と削除・変更後・達成と省略・
//! 経路 1 件の印の元・全体の置き換え）を差分へ 1 件積み、上限で止めたときは控えた長さまで差分を逆に当てる。
//!
//! - 計算量: 控えの取得は O(1)。変更 1 件ごとに積む差分は、その変更で変わる値の大きさに比例する。戻すのは
//!   控えの後に積んだ差分の数に比例する。差分は生きている控えのうち最も古いものより前を捨てるので、量は
//!   その控えの後の変更の数で抑えられる。操作ごとに復習全体・印の元全体を複製しない
//! - 保存の時点の復習は、現在の復習と `Arc` を共有する（保存の後の最初の変更で 1 回だけ複製する。保存 1 件に
//!   つき高々 1 回）。処理中の保存は、入口の控えの後に積んだ差分のうち復習の分だけを写して持ち、復帰のときに
//!   保存の時点の復習へ逆に当てて入口の時点の復習を作る。復帰そのものは、復習の大きさに比例する
//! - 入口の控えが操作の先頭より後にある列（受付の前に変更がある不正な列）でだけ、戻す途中で入口の時点の写し
//!   （`Start::Snapshot`）へ切り替え、控えの意味を保つ
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
    pending: BTreeMap<String, PendingEntry>,
    /// 保存の時点の復習・入口の控え・処理中。鍵は `CheckpointSaved` のログ上の位置（その `log_len`）。
    /// 復習は現在の復習と共有し、保存そのものは参照の複製に限る（復習を変えない保存の連続で全体を複製しない）。
    saved: BTreeMap<usize, SavedPoint>,
    /// 取り消し差分。復習と印の元を変える前の値を、古いものから順に持つ。`op_start`・`entry_start` の
    /// `Start::Mark` がこの列の長さを指す。どちらの印も無いときは積まない（モジュールの文書）。
    undo: Vec<Undo>,
    /// 直近の操作の先頭（`InputReceived`・`Progress`・`Moved`）の時点の復習と印の元。
    /// 上限で止めた操作を操作の前へ戻す（SAFE-35）ので、`TransitionLimitReached` で復習と印の元をここへ戻す。
    op_start: Option<Start>,
    /// 入口の写しに対応する、直近の入力の受付（`Accepted`）の時点の復習と印の元。core は入口の写しを状態に書かず、
    /// 処理中の区間の最新の `Accepted` をログから探して受付の時点を導く（`entry_derive`）ので、受付のたびに取り直す。
    /// 処理中の操作（進行・移動）の途中で止めたときは操作の前ではなくここへ戻す（入口へ戻す `return_after_limit` と
    /// 揃える）。処理中に拒否された入力（`Accepted` が出ない）では更新しない。
    /// 移動・停止などで core の導出が区間を越えない箇所で捨てる。復帰では保存の時点の控えへ戻す（core の導出が
    /// `Restored` から保存の時点へ起点を移すのと揃える）。
    entry_start: Option<Start>,
    /// 進行（`Progress`）が始まり、まだ完了していない（処理中である）。処理中の移動は入口へ戻るので、
    /// その判断に使う。達成・上限での停止・入口の写しの無効化で下ろす。
    progressing: bool,
}

/// 印の元の 1 経路の値（最初の変更前, 最後の変更後）。
type PendingEntry = (Option<Element>, Option<Element>);

/// 控えた時点。通常は取り消し差分の長さ（`Start::Mark`）で持ち、写しは不正な列でだけ使う。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Start {
    /// 取り消し差分の長さ。差分をこの長さまで逆に当てると、控えた時点の復習と印の元に戻る。
    Mark(usize),
    /// 復習と印の元の写し。入口の控えが戻す先より後にあり、差分を戻すと失われるときにだけ作る。
    Snapshot(Arc<Vec<ReviewItem>>, BTreeMap<String, PendingEntry>),
}

/// 取り消し差分の 1 件。復習と印の元への 1 回の変更を、変える前の値で打ち消す。
/// 項目と変化の位置は、その変更の時点の添字である（差分を新しいものから逆に当てる限り、添字は合う）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Undo {
    /// 末尾に足した項目を取り除く。
    PopItem,
    /// 項目の達成・省略を戻す。
    Flags {
        item: usize,
        achieved: bool,
        skipped: bool,
    },
    /// 項目の位置 `pos` に足した変化を取り除く。
    RemoveChange { item: usize, pos: usize },
    /// 取り除いた変化を位置 `pos` へ戻す。
    InsertChange {
        item: usize,
        pos: usize,
        change: ReviewChange,
    },
    /// 変化の変更後を戻す。
    SetAfter {
        item: usize,
        pos: usize,
        after: Option<Element>,
    },
    /// 項目の変化の列を戻す（`Stopped` の初期化の取り消し）。
    SetChanges {
        item: usize,
        changes: Vec<ReviewChange>,
    },
    /// 復習の全体を戻す（復帰・写しへの戻しの取り消し）。
    SetReview(Arc<Vec<ReviewItem>>),
    /// 印の元の 1 経路を戻す（`None` は無かった）。
    SetPending {
        path: String,
        entry: Option<PendingEntry>,
    },
    /// 印の元の全体を戻す（基準点での消去の取り消し）。
    SetPendingAll(BTreeMap<String, PendingEntry>),
}

impl Undo {
    /// 復習への差分か（印の元への差分でないか）。
    fn is_review(&self) -> bool {
        !matches!(self, Undo::SetPending { .. } | Undo::SetPendingAll(_))
    }
}

/// 復習への差分 1 件を `items` へ当てる。印の元への差分は無視する。添字が合わない（差分と復習が食い違う）
/// ときは何もしない（全域性のため。差分は本モジュールが積むので、通常は起きない）。
fn undo_review(items: &mut Vec<ReviewItem>, undo: Undo) {
    match undo {
        Undo::PopItem => {
            items.pop();
        }
        Undo::Flags {
            item,
            achieved,
            skipped,
        } => {
            if let Some(r) = items.get_mut(item) {
                r.achieved = achieved;
                r.skipped = skipped;
            }
        }
        Undo::RemoveChange { item, pos } => {
            if let Some(r) = items.get_mut(item).filter(|r| pos < r.changes.len()) {
                r.changes.remove(pos);
            }
        }
        Undo::InsertChange { item, pos, change } => {
            if let Some(r) = items.get_mut(item).filter(|r| pos <= r.changes.len()) {
                r.changes.insert(pos, change);
            }
        }
        Undo::SetAfter { item, pos, after } => {
            if let Some(c) = items.get_mut(item).and_then(|r| r.changes.get_mut(pos)) {
                c.after = after;
            }
        }
        Undo::SetChanges { item, changes } => {
            if let Some(r) = items.get_mut(item) {
                r.changes = changes;
            }
        }
        Undo::SetReview(review) => *items = review.to_vec(),
        Undo::SetPending { .. } | Undo::SetPendingAll(_) => {}
    }
}

/// 保存（`CheckpointSaved`）の時点の控え。core の `Checkpoint` が `run`（`busy` を含む）を写し、復帰の後の
/// 受付の時点をその `run` とログから導くのと対応させる。入口の控えは復習だけを持つ。印の元は `Restored` が
/// 基準点として空にするので控えない。
#[derive(Debug, Clone, PartialEq, Eq)]
struct SavedPoint {
    review: Arc<Vec<ReviewItem>>,
    entry: Option<SavedEntry>,
    progressing: bool,
}

/// 保存の時点の入口の控え（復習だけ）。
#[derive(Debug, Clone, PartialEq, Eq)]
enum SavedEntry {
    /// 入口の控えの後に積んだ復習の差分（古いものから順）。保存の時点の復習へ新しいものから逆に当てると、
    /// 入口の時点の復習になる。量は受付から保存までの変更の数に比例する。
    Undo(Vec<Undo>),
    /// 入口の時点の復習の写し（入口の控えが写しのとき）。
    Snapshot(Arc<Vec<ReviewItem>>),
}

impl SavedEntry {
    /// 保存の時点の復習 `saved` から、入口の時点の復習を作る。
    fn review(&self, saved: &[ReviewItem]) -> Vec<ReviewItem> {
        match self {
            SavedEntry::Snapshot(review) => review.to_vec(),
            SavedEntry::Undo(undo) => {
                let mut items = saved.to_vec();
                for u in undo.iter().rev() {
                    undo_review(&mut items, u.clone());
                }
                items
            }
        }
    }
}

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

    /// イベント 1 件（ログ上の位置 `i`）を導出へ反映する。控えを取り、反映し、要らなくなった差分を捨てる。
    fn step(&mut self, i: usize, event: &Event) {
        self.take_starts(event);
        self.step_event(i, event);
        self.compact();
    }

    /// 操作の先頭と入口の控えを取る。上限で止めた操作の `Achieved`・`Changed` を復習から取り除くために使う。
    /// 保存（`CheckpointSaved`）も先頭に数える。保存を入力とする Step（`env_action`。TASK-64 1/2）では
    /// `InputReceived` を伴わない `Accepted` が保存の直後に続くため、先頭が無いと前の操作の控えが残る。
    /// 控えは取り消し差分の長さで、復習・印の元を複製しない。
    fn take_starts(&mut self, event: &Event) {
        if matches!(
            event,
            Event::InputReceived { .. }
                | Event::CheckpointSaved { .. }
                | Event::Accepted { .. }
                | Event::Progress { .. }
                | Event::Moved { .. }
        ) {
            let now = Start::Mark(self.undo.len());
            match event {
                // 受付のとき（拒否された入力ではなく）だけ入口の控えを取る。core の導出と同じく、最新の受付を使う。
                Event::Accepted { .. } => {
                    self.entry_start = Some(now.clone());
                }
                // 処理中の操作を途中で止めると入口の写しへ戻るので、控えは入口の時点のものにする。
                Event::Progress { index, total, .. } => {
                    self.op_start = Some(self.entry_start.clone().unwrap_or(now));
                    // 最後の進行（確認待ちの Step では `Achieved` が出ない）で処理は終わる。以後の移動は移動直前の控えを使う。
                    self.progressing = index.saturating_add(1) < *total;
                    return;
                }
                Event::Moved { .. } if self.progressing => {
                    self.op_start = Some(self.entry_start.clone().unwrap_or(now));
                    return;
                }
                _ => {}
            }
            // 受付は操作の先頭ではない（`InputReceived` が先頭）ので、控えを取り直さない。
            if !matches!(event, Event::Accepted { .. }) {
                self.op_start = Some(now);
            }
        }
    }

    /// 控えを取った後の、イベント 1 件の反映。
    fn step_event(&mut self, i: usize, event: &Event) {
        if matches!(
            event,
            Event::Entered { .. }
                | Event::Begun { .. }
                | Event::Moved { .. }
                | Event::Stopped { .. }
                | Event::Restored { .. }
        ) {
            self.entry_start = None;
            self.progressing = false;
        }
        match event {
            Event::Entered { .. } => {
                self.push_row(i, event);
                self.clear_pending();
            }
            Event::Begun { .. } => self.clear_pending(),
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
                if let Some(start) = self.op_start.take() {
                    self.return_to(start);
                }
                self.progressing = false;
            }
            Event::Achieved { at, .. } => {
                self.progressing = false;
                self.push_row(i, event);
                let item = self.item_index(&at.step);
                if let Some(skipped) = self.review.get(item).map(|r| r.skipped) {
                    self.set_flags(item, true, skipped);
                }
            }
            Event::Skipped { at } => {
                let item = self.item_index(&at.step);
                if let Some(achieved) = self.review.get(item).map(|r| r.achieved) {
                    self.set_flags(item, achieved, true);
                }
            }
            Event::Flag { .. } | Event::HintOpened { .. } => {}
            Event::Changed {
                at,
                path,
                before,
                after,
                cause,
            } => {
                match cause {
                    Cause::Input => self.merge_change(&at.step, path, before, after),
                    // 上限で止めて入口へ戻した差分（SAFE-35）。学習者の変更として足さず、併合済みの変更を打ち消す。
                    Cause::Stop => self.unmerge_change(&at.step, path, after),
                    _ => {}
                }
            }
            Event::Stopped { at, .. } => {
                self.push_row(i, event);
                self.clear_pending();
                self.reset_item(&at.step);
            }
            Event::Moved { .. } => {
                self.push_row(i, event);
                self.clear_pending();
            }
            Event::CheckpointSaved { .. } => {
                self.push_row(i, event);
                let entry = match &self.entry_start {
                    Some(Start::Mark(mark)) => Some(SavedEntry::Undo(
                        self.undo
                            .get(*mark..)
                            .unwrap_or_default()
                            .iter()
                            .filter(|u| u.is_review())
                            .cloned()
                            .collect(),
                    )),
                    Some(Start::Snapshot(review, _)) => Some(SavedEntry::Snapshot(review.clone())),
                    None => None,
                };
                self.saved.insert(
                    i,
                    SavedPoint {
                        review: self.review.clone(),
                        entry,
                        progressing: self.progressing,
                    },
                );
            }
            Event::Restored { log_len, owner, .. } => {
                self.push_row(i, event);
                self.clear_pending();
                self.restore_review(i, *log_len, &owner.root());
            }
        }
    }

    /// 取り消し差分を 1 件積む。生きている控え（`Start::Mark`）が無ければ、戻す先が無いので積まない。
    fn record(&mut self, undo: Undo) {
        if matches!(self.op_start, Some(Start::Mark(_)))
            || matches!(self.entry_start, Some(Start::Mark(_)))
        {
            self.undo.push(undo);
        }
    }

    /// 生きている控えのうち最も古いものより前の差分を捨て、控えの長さをずらす。控えが無ければ差分を空にする。
    /// `step` の終わりに毎回呼ぶ（増分と一括で同じ時点に捨て、内部の状態を一致させる）。
    fn compact(&mut self) {
        let oldest = [&self.op_start, &self.entry_start]
            .into_iter()
            .filter_map(|s| match s {
                Some(Start::Mark(mark)) => Some(*mark),
                _ => None,
            })
            .min();
        match oldest {
            None => self.undo.clear(),
            Some(0) => {}
            Some(n) => {
                let n = n.min(self.undo.len());
                self.undo.drain(..n);
                for start in [&mut self.op_start, &mut self.entry_start] {
                    if let Some(Start::Mark(mark)) = start {
                        *mark = mark.saturating_sub(n);
                    }
                }
            }
        }
    }

    /// 控えた時点へ復習と印の元を戻す（上限で止めたとき）。
    fn return_to(&mut self, start: Start) {
        match start {
            Start::Mark(mark) => {
                // 入口の控えが戻す先より後にある（受付の前に変更がある不正な列）ときは、差分を戻すと入口の時点が
                // 失われるので、入口の時点まで戻したところで写しへ切り替える。
                if let Some(Start::Mark(entry)) = self.entry_start {
                    if entry > mark {
                        self.pop_to(entry);
                        self.entry_start =
                            Some(Start::Snapshot(self.review.clone(), self.pending.clone()));
                    }
                }
                self.pop_to(mark);
            }
            Start::Snapshot(review, pending) => {
                // 写しへの戻しも変更として差分に積み、他の控えの長さの意味を保つ。
                self.replace_review(review);
                let old = std::mem::replace(&mut self.pending, pending);
                self.record(Undo::SetPendingAll(old));
            }
        }
    }

    /// 取り消し差分を、長さが `len` になるまで新しいものから逆に当てる。当てた差分は捨てる。
    fn pop_to(&mut self, len: usize) {
        while self.undo.len() > len {
            let Some(undo) = self.undo.pop() else {
                break;
            };
            match undo {
                Undo::SetReview(review) => self.review = review,
                Undo::SetPending { path, entry } => match entry {
                    Some(entry) => {
                        self.pending.insert(path, entry);
                    }
                    None => {
                        self.pending.remove(&path);
                    }
                },
                Undo::SetPendingAll(pending) => self.pending = pending,
                other => undo_review(Arc::make_mut(&mut self.review), other),
            }
        }
    }

    /// 印の元を空にする（基準点）。
    fn clear_pending(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let old = std::mem::take(&mut self.pending);
        self.record(Undo::SetPendingAll(old));
    }

    /// 復習の全体を置き換える（復帰・写しへの戻し）。前の全体は差分へ移す（複製しない）。
    fn replace_review(&mut self, review: Arc<Vec<ReviewItem>>) {
        let old = std::mem::replace(&mut self.review, review);
        self.record(Undo::SetReview(old));
    }

    /// ログ行を足す。位置は `u32` に収まる範囲（履歴の大きさの上限から 2^32 件には届かない）だが、
    /// 全域性のため飽和させる。
    fn push_row(&mut self, i: usize, event: &Event) {
        self.log.push(LogRow {
            at: stamp_of(event).clone(),
            event: u32::try_from(i).unwrap_or(u32::MAX),
        });
    }

    /// `step` の復習の項目の位置を返す。無ければ末尾に作る（項目の順は最初に現れた順）。
    fn item_index(&mut self, step: &StepUid) -> usize {
        if let Some(pos) = self.review.iter().position(|r| &r.step == step) {
            return pos;
        }
        let items = Arc::make_mut(&mut self.review);
        items.push(ReviewItem {
            step: step.clone(),
            changes: Vec::new(),
            achieved: false,
            skipped: false,
        });
        let pos = items.len().saturating_sub(1);
        self.record(Undo::PopItem);
        pos
    }

    /// 項目の達成・省略を置き換える。変わらなければ何もしない。
    fn set_flags(&mut self, item: usize, achieved: bool, skipped: bool) {
        let Some(old) = self.review.get(item).map(|r| (r.achieved, r.skipped)) else {
            return;
        };
        if old == (achieved, skipped) {
            return;
        }
        if let Some(r) = Arc::make_mut(&mut self.review).get_mut(item) {
            r.achieved = achieved;
            r.skipped = skipped;
        }
        self.record(Undo::Flags {
            item,
            achieved: old.0,
            skipped: old.1,
        });
    }

    /// `Stopped` で Step の項目を初期化する（変化を空にし、達成・省略を下ろす）。項目が無ければ何もしない。
    fn reset_item(&mut self, step: &StepUid) {
        let Some(item) = self.review.iter().position(|r| &r.step == step) else {
            return;
        };
        let Some(r) = Arc::make_mut(&mut self.review).get_mut(item) else {
            return;
        };
        let changes = std::mem::take(&mut r.changes);
        let flags = Undo::Flags {
            item,
            achieved: r.achieved,
            skipped: r.skipped,
        };
        r.achieved = false;
        r.skipped = false;
        self.record(Undo::SetChanges { item, changes });
        self.record(flags);
    }

    /// 学習者の変化を印の元と復習へ併合する。最初の変更前・最後の変更後を保つ。
    fn merge_change(
        &mut self,
        step: &StepUid,
        path: &str,
        before: &Option<Element>,
        after: &Option<Element>,
    ) {
        let first = match self.pending.get(path) {
            Some((first, _)) => first.clone(),
            None => before.clone(),
        };
        let prev = self
            .pending
            .insert(path.to_string(), (first, after.clone()));
        self.record(Undo::SetPending {
            path: path.to_string(),
            entry: prev,
        });
        let item = self.item_index(step);
        let Some(found) = self
            .review
            .get(item)
            .map(|r| r.changes.iter().position(|c| c.path == path))
        else {
            return;
        };
        let Some(r) = Arc::make_mut(&mut self.review).get_mut(item) else {
            return;
        };
        match found {
            Some(pos) => {
                let Some(change) = r.changes.get_mut(pos) else {
                    return;
                };
                let old = std::mem::replace(&mut change.after, after.clone());
                self.record(Undo::SetAfter {
                    item,
                    pos,
                    after: old,
                });
            }
            None => {
                let pos = r.changes.len();
                r.changes.push(ReviewChange {
                    path: path.to_string(),
                    before: before.clone(),
                    after: after.clone(),
                });
                self.record(Undo::RemoveChange { item, pos });
            }
        }
    }

    /// 取り消しの差分（原因 `Stop`）を印の元と復習へ当てる。併合済みの変更だけが対象で、新しい項目は作らない。
    /// 取り消し後の値が最初の変更前に等しければ変更ごと取り除き、そうでなければ変更後を差し替える。
    fn unmerge_change(&mut self, step: &StepUid, path: &str, after: &Option<Element>) {
        match self.pending.get(path).map(|(first, _)| first == after) {
            Some(true) => {
                let prev = self.pending.remove(path);
                self.record(Undo::SetPending {
                    path: path.to_string(),
                    entry: prev,
                });
            }
            Some(false) => {
                if let Some(entry) = self.pending.get_mut(path) {
                    let old_after = std::mem::replace(&mut entry.1, after.clone());
                    let prev = (entry.0.clone(), old_after);
                    self.record(Undo::SetPending {
                        path: path.to_string(),
                        entry: Some(prev),
                    });
                }
            }
            None => {}
        }
        let Some(item) = self.review.iter().position(|r| &r.step == step) else {
            return;
        };
        let Some(found) = self.review.get(item).and_then(|r| {
            r.changes
                .iter()
                .position(|c| c.path == path)
                .map(|pos| (pos, r.changes.get(pos).map(|c| c.before == *after)))
        }) else {
            return;
        };
        let Some(r) = Arc::make_mut(&mut self.review).get_mut(item) else {
            return;
        };
        match found {
            (pos, Some(true)) => {
                let change = r.changes.remove(pos);
                self.record(Undo::InsertChange { item, pos, change });
            }
            (pos, Some(false)) => {
                let Some(change) = r.changes.get_mut(pos) else {
                    return;
                };
                let old = std::mem::replace(&mut change.after, after.clone());
                self.record(Undo::SetAfter {
                    item,
                    pos,
                    after: old,
                });
            }
            (_, None) => {}
        }
    }

    /// 復帰（EVT-14）。持ち主の根の項目を、保存の時点のものへ戻す。他の根の項目は変えない。
    /// 保存の位置を指さない `log_len`（範囲外・未来・不一致）は「写しなし」で、持ち主の根の項目を空にし、
    /// 処理中と入口の控えも無しに倒す（呼び出し元 `step_event` が先に下ろしている）。
    ///
    /// 処理中かどうかと入口の控えも保存の時点のものへ戻す。core の復帰は `busy` を保存時点へ戻し、受付の時点も
    /// 保存の時点のログから導くので、
    /// 復帰後の最後の進行・移動で上限に達すると入口（受付の前）へ戻る。控えを戻さないと、導出だけが進行の直前へ
    /// 戻り、世界状態の差分が無い（受付と進行の変更が相殺された）ときは取り消しの `Changed` も出ず、取り消した
    /// 変更が復習に残る（SAFE-35）。入口の控えの他の根の項目は、復習と同じく復帰の時点のものを使う。
    ///
    /// 入口の控えは、入口の時点の復習へ置き換えたところで取り消し差分の長さを控え、続けて保存の時点の復習へ
    /// 置き換える（どちらも全体の置き換えで、前の全体は差分へ移す）。上限で入口へ戻すときは、後の置き換えを
    /// 取り消すだけで入口の時点の復習になる。
    fn restore_review(&mut self, i: usize, log_len: u32, own: &crate::addr::Root) {
        let saved = usize::try_from(log_len)
            .ok()
            .filter(|at| *at < i)
            .and_then(|at| self.saved.get(&at));
        let others: Vec<ReviewItem> = self
            .review
            .iter()
            .filter(|r| &r.step.root() != own)
            .cloned()
            .collect();
        // 持ち主の根の項目は控えから、他の根の項目は復帰の時点の復習から取る。
        let rebuild = |items: Option<&[ReviewItem]>| -> Arc<Vec<ReviewItem>> {
            let mut restored: Vec<ReviewItem> = items
                .map(|items| {
                    items
                        .iter()
                        .filter(|r| &r.step.root() == own)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            restored.extend(others.iter().cloned());
            Arc::new(restored)
        };
        let restored = rebuild(saved.map(|s| s.review.as_slice()));
        let mut entry_review = None;
        if let Some(point) = saved {
            self.progressing = point.progressing;
            entry_review = point
                .entry
                .as_ref()
                .map(|entry| rebuild(Some(&entry.review(&point.review))));
        }
        if let Some(entry_review) = entry_review {
            self.replace_review(entry_review);
            self.entry_start = Some(Start::Mark(self.undo.len()));
        }
        self.replace_review(restored);
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

/// 控えの持ち方の試験。控えを取っても復習全体を共有・複製せず、取り消し差分が操作で変えた量で抑えられることを、
/// 外から見えない欄で確かめる（`tests/projection_derive.rs` は公開の欄で挙動を確かめる）。
#[cfg(test)]
mod undo_tests {
    use super::*;
    use crate::addr::Generation;
    use crate::input::Input;

    fn stamp(step: &str) -> Stamp {
        Stamp {
            generation: Generation::INITIAL,
            step: StepUid(step.to_string()),
        }
    }

    fn el(n: u8) -> Element {
        Element {
            fixture: Some(format!("f{n}")),
            attrs: BTreeMap::new(),
        }
    }

    fn input(step: &str) -> Event {
        Event::InputReceived {
            at: stamp(step),
            input: Input::Text("go".into()),
        }
    }

    fn changed(step: &str, path: &str, b: Option<u8>, a: Option<u8>) -> Event {
        Event::Changed {
            at: stamp(step),
            path: path.to_string(),
            before: b.map(el),
            after: a.map(el),
            cause: Cause::Input,
        }
    }

    fn progress(step: &str, index: u32, total: u32) -> Event {
        Event::Progress {
            at: stamp(step),
            name: "p".into(),
            index,
            total,
        }
    }

    /// Step `n` 個を、入る・受付・変更 2 件・達成の順に通す。
    fn steps(n: usize) -> Vec<Event> {
        let mut events = Vec::new();
        for k in 0..n {
            let s = format!("main/s{k}");
            events.push(Event::Entered { at: stamp(&s) });
            events.push(input(&s));
            events.push(Event::Accepted { at: stamp(&s) });
            events.push(changed(&s, &format!("x{k}"), None, Some(1)));
            events.push(changed(&s, &format!("y{k}"), None, Some(2)));
            events.push(Event::Achieved {
                at: stamp(&s),
                explicit_skip: false,
            });
        }
        events
    }

    #[test]
    fn taking_starts_does_not_share_or_copy_the_review() {
        let mut p = Projection::from_events(&steps(3));
        let a = "main/a";
        let rest = [
            Event::Entered { at: stamp(a) },
            input(a),
            Event::Accepted { at: stamp(a) },
            changed(a, "x", None, Some(1)),
            progress(a, 0, 2),
            changed(a, "y", None, Some(1)),
        ];
        let mut events = steps(3);
        for e in rest {
            events.push(e);
            p.extend(&events);
            // 控えは差分の長さだけを持ち、復習の `Arc` を共有しない（変更のたびの全体の複製が起きない）。
            assert_eq!(Arc::strong_count(&p.review), 1, "{:?}", events.last());
        }
        // 入口の控えの後の変更は 2 件で、差分はその分（項目の追加・変化の追加・印の元）だけを持つ。
        assert!(p.undo.len() <= 6, "{:?}", p.undo);
        assert_eq!(p.entry_start, Some(Start::Mark(0)));
        assert_eq!(p.op_start, Some(Start::Mark(0)));
    }

    #[test]
    fn undo_log_does_not_grow_with_the_number_of_steps() {
        let small = Projection::from_events(&steps(2));
        let large = Projection::from_events(&steps(200));
        assert_eq!(large.review().len(), 200);
        // 差分は直近の受付の後の変更だけで、通った Step の数に比例しない。
        assert_eq!(small.undo.len(), large.undo.len());
        assert!(large.undo.len() <= 6, "{:?}", large.undo);
    }

    #[test]
    fn limit_rewind_empties_the_undo_log() {
        let a = "main/a";
        let mut events = steps(50);
        events.extend([
            Event::Entered { at: stamp(a) },
            input(a),
            Event::Accepted { at: stamp(a) },
            changed(a, "x", None, Some(1)),
            progress(a, 0, 2),
            changed(a, "x", Some(1), None),
            Event::TransitionLimitReached {
                at: stamp(a),
                landings: 1000,
            },
        ]);
        let p = Projection::from_events(&events);
        assert!(p.undo.is_empty(), "{:?}", p.undo);
        assert!(p.pending.is_empty());
        assert_eq!(p.review().len(), 50);
        assert_eq!(Arc::strong_count(&p.review), 1);
    }

    #[test]
    fn nothing_is_recorded_without_a_start() {
        let a = "main/a";
        let p = Projection::from_events(&[
            Event::Entered { at: stamp(a) },
            changed(a, "x", None, Some(1)),
            changed(a, "x", Some(1), Some(2)),
        ]);
        assert!(p.undo.is_empty(), "{:?}", p.undo);
        assert_eq!(p.review().len(), 1);
    }
}
