//! 文書のブロックの前後を突き合わせ、`-` と `+` の行にする（TASK-468 2/2。ART-2・ART-3）。
//!
//! 呼び出し元は、文書のビュー本体（TASK-476。ブロックの具体の型 `contract::doc::Block` を結ぶ）と、
//! 製品の `view()` への結線（TASK-470）の予定である。ブロックの型は型パラメータ `B` で受け、
//! 文書を世界状態のどの経路・どの属性に持たせるかと、本文のブロックへの読み方は呼び出し側の `read` が持つ。
//!
//! # 規則
//!
//! - 前後のブロックの並びを最長共通部分列（LCS）で突き合わせる。前にだけあるブロックは `-`（削除）、
//!   後にだけあるブロックは `+`（追加）、共通のブロックは印なしの 1 行にする
//! - 変更は `-` と `+` の組になり、`~` は作らない（前後が違う組の行を作らないため、構造上 0 件になる）
//! - 不変条件は「未変更 + 追加 = 後のブロック数」「未変更 + 削除 = 前のブロック数」「`~` が 0 件」（PoC-8）
//! - 同点のときは `-` を `+` より先に出す（diff の慣習。spec は順を決めておらず、仮置きである）
//! - 先頭と末尾の共通部分を先に除き、残りの中央だけを動的計画法で突き合わせる
//! - 行の ID は `"{doc_path}#{k}"`（`k` は突き合わせた並びの 0 始まりの位置）。一意で決定的である。
//!   `depth` は 0、`kind` は `RowKind::Block`
//!
//! # 指せる行
//!
//! 文書の行の ID は世界状態の経路ではなく「文書の経路 + `#` + 位置」である。`PointTarget::new(view, id)` で属性は作れるが、
//! `Input::Ui` の `path` にどう対応させるか（ブロックの位置を宛先にするか）は TASK-476（ART-11 の文書の行）が決める。
//! 位置は操作ごとに変わりうるが、同じ前後の組の中では固定である。
//!
//! # 上限（A04）
//!
//! LCS は `O(n * m)` なので、片側のブロック数が `LIST_ITEMS_LIMIT` を超えるとき、または中央の表のセル数が
//! `MAX_ALIGN_CELLS` を超えるときは `DeriveError::TooLarge` を返す。黙って別の突き合わせに退避しない。
//!
//! # 範囲外
//!
//! ブロックの型の結び・指した宛先の意味・描画は TASK-476。基準点での「前」の取り直しは TASK-470・TASK-347。
//!
//! # 信頼境界
//!
//! ブロックの中身と経路は教材由来で信頼できない。運ぶだけで、HTML・属性名には使わない。`Node` を作らない。
//! 全域（panic しない。`unwrap`・`expect`・添字を使わない）で決定的で、誤りの文面は固定の英語である。

use fandhe_workshop_core::changed::replay_changes;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::world::{Element, World};

use crate::artifact_rows::DeriveError;
use crate::artifact_view::{check_unique_ids, ArtifactViewError, DocRow, Row, RowId, RowKind};
use crate::list_policy::LIST_ITEMS_LIMIT;

/// 中央の表のセル数の上限。`u32` で約 4MB。
pub const MAX_ALIGN_CELLS: usize = 1 << 20;

/// 突き合わせの 1 手。
enum Step {
    /// 前後で共通（前の位置, 後の位置）。
    Keep(usize, usize),
    /// 前にだけある。
    Del(usize),
    /// 後にだけある。
    Ins(usize),
}

/// 表の `(i, j)` の値。範囲外は `None`。
fn cell(table: &[u32], width: usize, i: usize, j: usize) -> Option<u32> {
    table.get(i.checked_mul(width)?.checked_add(j)?).copied()
}

/// 中央 `a`・`b` の LCS の手順を、先頭からの順で返す。位置は `a`・`b` の中の 0 始まり。
fn middle_steps<B: PartialEq>(a: &[B], b: &[B]) -> Result<Vec<Step>, DeriveError> {
    let (n, m) = (a.len(), b.len());
    let width = m + 1;
    let cells = (n + 1)
        .checked_mul(width)
        .filter(|c| *c <= MAX_ALIGN_CELLS)
        .ok_or(DeriveError::TooLarge)?;
    // table[i][j] は a[i..] と b[j..] の LCS の長さ。
    let mut table = vec![0u32; cells];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let (x, y) = (a.get(i), b.get(j));
            let v = if x.is_some() && x == y {
                cell(&table, width, i + 1, j + 1).ok_or(DeriveError::TooLarge)? + 1
            } else {
                let down = cell(&table, width, i + 1, j).ok_or(DeriveError::TooLarge)?;
                let right = cell(&table, width, i, j + 1).ok_or(DeriveError::TooLarge)?;
                down.max(right)
            };
            let slot = table.get_mut(i * width + j).ok_or(DeriveError::TooLarge)?;
            *slot = v;
        }
    }
    let mut steps = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && a.get(i) == b.get(j) {
            steps.push(Step::Keep(i, j));
            i += 1;
            j += 1;
        } else if j >= m {
            steps.push(Step::Del(i));
            i += 1;
        } else if i >= n {
            steps.push(Step::Ins(j));
            j += 1;
        } else {
            let down = cell(&table, width, i + 1, j).ok_or(DeriveError::TooLarge)?;
            let right = cell(&table, width, i, j + 1).ok_or(DeriveError::TooLarge)?;
            // 同点は `-` を先に出す。
            if down >= right {
                steps.push(Step::Del(i));
                i += 1;
            } else {
                steps.push(Step::Ins(j));
                j += 1;
            }
        }
    }
    Ok(steps)
}

/// 前後のブロックの並びを突き合わせ、文書の行にする。
///
/// 空の `doc_path` は `Err`。上限を超える列は `Err(TooLarge)`。
pub fn align_blocks<B: PartialEq + Clone>(
    doc_path: &str,
    before: &[B],
    after: &[B],
) -> Result<Vec<DocRow<B>>, DeriveError> {
    if doc_path.is_empty() {
        return Err(DeriveError::Row(ArtifactViewError::EmptyRowId));
    }
    if before.len() > LIST_ITEMS_LIMIT || after.len() > LIST_ITEMS_LIMIT {
        return Err(DeriveError::TooLarge);
    }
    let prefix = before
        .iter()
        .zip(after.iter())
        .take_while(|(x, y)| x == y)
        .count();
    let (b_rest, a_rest) = match (before.get(prefix..), after.get(prefix..)) {
        (Some(b), Some(a)) => (b, a),
        _ => return Err(DeriveError::TooLarge),
    };
    let suffix = b_rest
        .iter()
        .rev()
        .zip(a_rest.iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (b_mid, a_mid) = match (
        b_rest.get(..b_rest.len() - suffix),
        a_rest.get(..a_rest.len() - suffix),
    ) {
        (Some(b), Some(a)) => (b, a),
        _ => return Err(DeriveError::TooLarge),
    };

    let mut steps: Vec<Step> = (0..prefix).map(|i| Step::Keep(i, i)).collect();
    for s in middle_steps(b_mid, a_mid)? {
        steps.push(match s {
            Step::Keep(i, j) => Step::Keep(i + prefix, j + prefix),
            Step::Del(i) => Step::Del(i + prefix),
            Step::Ins(j) => Step::Ins(j + prefix),
        });
    }
    for t in 0..suffix {
        steps.push(Step::Keep(
            prefix + b_mid.len() + t,
            prefix + a_mid.len() + t,
        ));
    }

    let mut rows = Vec::with_capacity(steps.len());
    for (k, step) in steps.iter().enumerate() {
        let (b, a) = match *step {
            Step::Keep(i, j) => (before.get(i), after.get(j)),
            Step::Del(i) => (before.get(i), None),
            Step::Ins(j) => (None, after.get(j)),
        };
        rows.push(Row::new(
            RowId::new(&format!("{doc_path}#{k}"))?,
            0,
            RowKind::Block,
            b.cloned(),
            a.cloned(),
        )?);
    }
    check_unique_ids(&rows)?;
    Ok(rows)
}

/// 前後の世界状態から文書の行を作る。一括と増分が共有する。
///
/// `doc_path` の要素が無い側は空の列として扱う（文書が新しくできたなら全部 `+`、消えたなら全部 `-`）。
pub(crate) fn doc_rows_between<B, F>(
    before: &World,
    after: &World,
    doc_path: &str,
    read: F,
) -> Result<Vec<DocRow<B>>, DeriveError>
where
    B: PartialEq + Clone,
    F: Fn(&Element) -> Result<Vec<B>, DeriveError>,
{
    let b = match before.get(doc_path) {
        Some(el) => read(el)?,
        None => Vec::new(),
    };
    let a = match after.get(doc_path) {
        Some(el) => read(el)?,
        None => Vec::new(),
    };
    align_blocks(doc_path, &b, &a)
}

/// 前の世界状態と同じ操作のイベント列から、`doc_path` の文書の行を導く。
///
/// `read` は文書の要素をブロックの列にする（形が合わなければ `Err`。そのまま返す）。
/// 後の世界状態は `replay_changes` で作り、`Changed` 以外のイベントは無視する。
pub fn derive_doc_rows<B, F>(
    before: &World,
    events: &[Event],
    doc_path: &str,
    read: F,
) -> Result<Vec<DocRow<B>>, DeriveError>
where
    B: PartialEq + Clone,
    F: Fn(&Element) -> Result<Vec<B>, DeriveError>,
{
    let mut after_world = before.clone();
    replay_changes(&mut after_world, events);
    doc_rows_between(before, &after_world, doc_path, read)
}
