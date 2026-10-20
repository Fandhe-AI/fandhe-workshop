//! 1 リストを約 1,000 項目で畳む計画 `plan_fold` と要約行の契約（TASK-360 1/2。FEND-22・D-46）。
//!
//! 計画はグループの長さの列と目安だけで決まる純粋関数である。本試験は、(1) 目安以下では何も畳まない、
//! (2) どんな列でも出る行は目安以下、(3) 最新の行は残り、畳みは古い側だけ、(4) 同じ入力は同じ計画、
//! (5) 要約行のキー・文言・属性が契約どおり、を確かめる。試験では `unwrap` を使ってよい。

use fandhe_frontend_core::render;
use fandhe_workshop_web::list_policy::{
    fold_node, plan_fold, FoldEntry, FoldKind, LIST_ITEMS_TARGET,
};

/// 計画が出す行の数。
fn shown_rows(plan: &[FoldEntry], lens: &[usize]) -> usize {
    plan.iter()
        .map(|e| match e {
            FoldEntry::Rows { group, skip } => lens[*group] - skip,
            _ => 1,
        })
        .sum()
}

/// 最後に残るデータ行の（グループ, 行の位置）の列。
fn kept(plan: &[FoldEntry], lens: &[usize]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for e in plan {
        if let FoldEntry::Rows { group, skip } = e {
            out.extend((*skip..lens[*group]).map(|i| (*group, i)));
        }
    }
    out
}

fn check(lens: &[usize]) {
    let plan = plan_fold(lens, LIST_ITEMS_TARGET);
    let total: usize = lens.iter().sum();
    assert!(
        shown_rows(&plan, lens) <= LIST_ITEMS_TARGET.max(1),
        "上限を超えた: {lens:?}"
    );
    if total <= LIST_ITEMS_TARGET {
        assert_eq!(shown_rows(&plan, lens), total);
        assert!(plan
            .iter()
            .all(|e| matches!(e, FoldEntry::Rows { skip: 0, .. })));
    }
    if let (Some(last), true) = (
        lens.len().checked_sub(1),
        lens.last().is_some_and(|l| *l > 0),
    ) {
        let k = kept(&plan, lens);
        assert_eq!(
            k.last().copied(),
            Some((last, lens[last] - 1)),
            "最新の行が残らない: {lens:?}"
        );
        // 畳まれる行は古い側だけで、最後に畳まれた行より新しいグループは全行が残る。
        let dropped_max = (0..lens.len()).rev().find(|g| {
            let kept_in = k.iter().filter(|(kg, _)| kg == g).count();
            kept_in < lens[*g]
        });
        if let Some(d) = dropped_max {
            for g in d + 1..lens.len() {
                let kept_in = k.iter().filter(|(kg, _)| *kg == g).count();
                assert_eq!(kept_in, lens[g], "新しい側が畳まれた: {lens:?}");
            }
        }
    }
    assert_eq!(
        plan,
        plan_fold(lens, LIST_ITEMS_TARGET),
        "計画が決定的でない"
    );
}

#[test]
fn under_or_at_target_is_identity() {
    for n in [1usize, 999, 1_000] {
        let lens = vec![1; n];
        let plan = plan_fold(&lens, LIST_ITEMS_TARGET);
        assert_eq!(plan.len(), n);
        assert!(plan
            .iter()
            .all(|e| matches!(e, FoldEntry::Rows { skip: 0, .. })));
    }
    assert!(plan_fold(&[], LIST_ITEMS_TARGET).is_empty());
}

#[test]
fn over_target_folds_to_within_target() {
    let lens = vec![1; 1_001];
    let plan = plan_fold(&lens, LIST_ITEMS_TARGET);
    assert!(plan
        .iter()
        .any(|e| !matches!(e, FoldEntry::Rows { skip: 0, .. })));
    assert!(shown_rows(&plan, &lens) <= LIST_ITEMS_TARGET);
}

#[test]
fn shapes_stay_within_target() {
    for lens in [
        vec![10; 200],
        vec![1; 2_000],
        vec![4_097],
        vec![2_000, 2_000, 2_000],
        vec![2_000; 3],
        vec![3, 5_000],
        vec![5_000, 1],
        vec![1, 1, 1, 5_000],
    ] {
        check(&lens);
    }
}

#[test]
fn summaries_come_before_aggregation_and_old_side_only() {
    // 10 行 x 101 Step = 1,010 行。古い Step を 2 つ畳めば収まる（1,010 - 9 - 9）。
    let lens = vec![10; 101];
    let plan = plan_fold(&lens, LIST_ITEMS_TARGET);
    let summaries: Vec<usize> = plan
        .iter()
        .filter_map(|e| match e {
            FoldEntry::Summary { group, .. } => Some(*group),
            _ => None,
        })
        .collect();
    assert_eq!(summaries, vec![0, 1], "古い順の連続した要約になっていない");
    assert!(plan.iter().all(|e| !matches!(e, FoldEntry::Older { .. })));
}

#[test]
fn same_step_only_folds_head_and_keeps_tail() {
    let lens = vec![4_097];
    let plan = plan_fold(&lens, LIST_ITEMS_TARGET);
    assert!(matches!(plan[0], FoldEntry::Head { .. }));
    assert_eq!(shown_rows(&plan, &lens), LIST_ITEMS_TARGET);
}

#[test]
fn appending_a_row_keeps_newest_and_bound() {
    // 同じ Step に追記し続けても、最新の行は残り、出る行は目安を超えない。
    let mut lens = vec![7; 300];
    for _ in 0..50 {
        *lens.last_mut().unwrap() += 1;
        check(&lens);
    }
}

#[test]
fn pseudo_random_shapes_stay_within_target() {
    // 固定の種の線形合同法（外部の crate は足さない）。
    let mut s: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move |m: u64| {
        s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        usize::try_from((s >> 33) % m).unwrap()
    };
    for _ in 0..1_000 {
        let groups = next(400) + 1;
        let cap = [1u64, 3, 12, 80, 3_000][next(5)];
        let lens: Vec<usize> = (0..groups).map(|_| next(cap) + 1).collect();
        check(&lens);
    }
}

#[test]
fn zero_length_and_huge_groups_do_not_panic() {
    // 呼び出し元は長さ 0 のグループを作らないが、来ても panic しない。
    for lens in [
        vec![0],
        vec![0, 0, 0],
        vec![0, 2_000, 0],
        vec![usize::MAX, usize::MAX],
    ] {
        let _ = plan_fold(&lens, LIST_ITEMS_TARGET);
    }
}

#[test]
fn fold_node_has_fixed_text_and_no_event_attr() {
    let step = render(&fold_node(FoldKind::Step, 1, 1_234, Some("st-<x>"), false));
    assert!(step.contains("data-fold=\"step\""));
    assert!(step.contains("data-fold-rows=\"1234\""));
    assert!(step.contains("1,234"));
    assert!(!step.contains("data-event"));
    assert!(
        !step.contains("st-<x>"),
        "属性値がエスケープされていない: {step}"
    );
    let older = render(&fold_node(
        FoldKind::Older,
        12,
        3_000,
        Some("ignored"),
        false,
    ));
    assert!(older.contains("data-fold=\"older\""));
    assert!(
        !older.contains("data-step"),
        "step 以外に data-step が付いた"
    );
    let head = render(&fold_node(FoldKind::Head, 1, 5, None, false));
    assert!(head.contains("data-fold=\"head\""));
    let review = render(&fold_node(FoldKind::Older, 2, 2, None, true));
    assert!(review.contains("復習"));
}
