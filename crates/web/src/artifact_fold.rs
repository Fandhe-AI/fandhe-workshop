//! 左の成果物の 1 リストを約 1,000 項目以内に保つ畳み（TASK-484 1/2。FEND-22・ART-14・決定 D-46 の案 3）。
//!
//! 呼び出し元は `engine_parts::ArtifactPart::view` で、行が [`LIST_ITEMS_TARGET`] を超えたときだけ [`fold_rows`]
//! を通す。経路を「最後に変えた Step の連続範囲（ラン）」でグループにし、`list_policy::plan_fold_older` に従って
//! 古いグループを Step の要約 1 行か集約 1 行へ畳む。畳まれなかったデータ行は元の順とキーのまま残る
//! （要素の同一性を保ち、`app-root` の全再描画を起こさない）。
//!
//! 状態を持たない純粋関数で、入力はログと行の列だけである。同じログからは同じ出力になり、増分と一括の
//! 導出が一致する（EVT-1）。計算量は O(ログ長 + 行数 log 行数) である。
//!
//! 信頼境界: 経路は教材由来の信頼できない文字列で、この関数は読むだけで連結・出力しない（行のノードは呼び出し元が
//! 作る）。要約行の文言は固定で、数と検証済みの `StepUid` だけが入る（SAFE-11）。native の rlib で試験できるよう
//! wasm 専用にしない。

use std::collections::{BTreeMap, HashMap};

use fandhe_frontend_core::Node;
use fandhe_workshop_core::addr::Stamp;
use fandhe_workshop_core::event::Event;

use crate::list_policy::{fold_node, plan_fold_older, FoldEntry, FoldKind, LIST_ITEMS_TARGET};

/// 左の 1 行。経路・キー・ノード・指せる行か否かの組。
///
/// 指せる行（`PointTarget` を持つ行）は、畳むと受講者が対象を指せず Step を達成できなくなるため、畳みの対象にしない。
pub type ArtifactRow = (String, String, Node, bool);

/// ランの情報。
struct Run {
    /// ランの先頭の `Changed` のイベント番号。
    first_event: usize,
    /// ランの Step の実体 ID。
    step: String,
}

/// 行を畳んだ keyed list の項目列にする。行の総数が目安以下なら畳まず、元の順で返す。
///
/// `log` は世界状態の導出が読み終えた範囲（`world_consumed` まで）である。経路に対応する `Changed` が見つからない
/// 行は、最古の仮のグループに入れて畳みの対象にする（安全側）。
pub fn fold_rows(log: &[Event], rows: Vec<ArtifactRow>) -> Vec<(String, Node)> {
    if rows.len() <= LIST_ITEMS_TARGET {
        return rows.into_iter().map(|(_, k, n, _)| (k, n)).collect();
    }
    // 指せる行は畳まず残す。畳みの目安は、残す分を引いた数で計画する（計画側が下限を持つ）。
    let pinned = rows.iter().filter(|r| r.3).count();
    let target = LIST_ITEMS_TARGET.saturating_sub(pinned);
    // ログを先頭から走査し、(世代, Step) が変わるたびにラン番号を 1 増やす。番号 0 は「見つからない」用。
    let mut runs: Vec<Run> = Vec::new();
    let mut last_run: HashMap<&str, usize> = HashMap::new();
    let mut prev: Option<&Stamp> = None;
    for (i, ev) in log.iter().enumerate() {
        if let Event::Changed { at, path, .. } = ev {
            if prev != Some(at) {
                runs.push(Run {
                    first_event: i,
                    step: at.step.0.clone(),
                });
                prev = Some(at);
            }
            last_run.insert(path.as_str(), runs.len());
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (idx, (path, _, _, pin)) in rows.iter().enumerate() {
        if *pin {
            continue;
        }
        let run = last_run.get(path.as_str()).copied().unwrap_or(0);
        groups.entry(run).or_default().push(idx);
    }
    let ordered: Vec<(usize, Vec<usize>)> = groups.into_iter().collect();
    let lens: Vec<usize> = ordered.iter().map(|(_, v)| v.len()).collect();

    let mut keep: Vec<bool> = rows.iter().map(|r| r.3).collect();
    let mut summaries: Vec<(String, Node)> = Vec::new();
    for entry in plan_fold_older(&lens, target) {
        match entry {
            FoldEntry::Rows { group, skip } => {
                if let Some((_, idxs)) = ordered.get(group) {
                    for i in idxs.iter().skip(skip) {
                        if let Some(k) = keep.get_mut(*i) {
                            *k = true;
                        }
                    }
                }
            }
            FoldEntry::Summary { group, rows: n } => {
                let run = ordered
                    .get(group)
                    .and_then(|(r, _)| r.checked_sub(1))
                    .and_then(|r| runs.get(r));
                let key = run.map_or_else(|| "s-pre".to_owned(), |r| format!("s{}", r.first_event));
                summaries.push((
                    key,
                    fold_node(FoldKind::Step, 1, n, run.map(|r| r.step.as_str()), false),
                ));
            }
            FoldEntry::Older { groups, rows: n } => summaries.push((
                "fold-older".to_owned(),
                fold_node(FoldKind::Older, groups, n, None, false),
            )),
            // 左では生じない（plan_fold_older は返さない）。来ても行を落とさず無視する。
            FoldEntry::Head { .. } => {}
        }
    }
    let mut out = summaries;
    out.extend(
        rows.into_iter()
            .zip(keep)
            .filter(|(_, k)| *k)
            .map(|((_, key, node, _), _)| (key, node)),
    );
    out
}
