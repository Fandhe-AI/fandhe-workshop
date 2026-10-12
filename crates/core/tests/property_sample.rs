//! 性質テストの共通ヘルパ（`common::property`）の使い方を示す見本と、ヘルパの自己試験。
//!
//! 下の `Model` はヘルパの使い方を示すための見本であり、エンジンの実装ではない。
//! core の公開 API に依存せず、試験の中で閉じている。後続のエンジンの試験は、
//! この形で自分の状態機械を `check` に渡す。

mod common;

use common::property::{
    assert_replay_eq, check, op_sequence, parse_cases, parse_seed, runner_with, seed_hex,
    try_check_with, DEFAULT_CASES, DEFAULT_SEED,
};
use proptest::prelude::*;
use proptest::strategy::ValueTree;

/// 見本モデルの最終位置。
const LAST: u8 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Model {
    generation: u32,
    step: u8,
    completed: bool,
    saved: Option<(u32, u8, bool)>,
}

#[derive(Debug, Clone)]
enum Op {
    /// 現世代の進行。1 進む。
    Advance,
    /// 古い世代の進行。何も変えないはず。
    Stale,
    /// 世代を上げて位置を戻す。
    Restart,
    Save,
    Restore,
}

impl Model {
    fn new() -> Self {
        Model {
            generation: 0,
            step: 0,
            completed: false,
            saved: None,
        }
    }

    /// 操作を適用し、「古い進行を適用した」「復元に成功した」かを返す。
    fn apply(&mut self, op: &Op) -> (bool, bool) {
        match op {
            Op::Advance => {
                if self.step < LAST {
                    self.step += 1;
                }
                self.completed = self.step == LAST;
            }
            Op::Stale => return (true, false),
            Op::Restart => {
                self.generation += 1;
                self.step = 0;
                self.completed = false;
            }
            Op::Save => self.saved = Some((self.generation, self.step, self.completed)),
            Op::Restore => {
                if let Some((g, s, c)) = self.saved {
                    self.generation = g;
                    self.step = s;
                    self.completed = c;
                    return (false, true);
                }
            }
        }
        (false, false)
    }
}

fn run(ops: &[Op]) -> Model {
    let mut model = Model::new();
    for op in ops {
        model.apply(op);
    }
    model
}

fn any_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => Just(Op::Advance),
        2 => Just(Op::Stale),
        1 => Just(Op::Restart),
        2 => Just(Op::Save),
        2 => Just(Op::Restore),
    ]
}

/// 操作列を流して不変条件を検査する。古い進行・復元に成功した回数・完了に達したかを返す。
fn verify(ops: &[Op]) -> Result<(u64, u64, bool), TestCaseError> {
    let mut model = Model::new();
    let (mut stale, mut restored) = (0u64, 0u64);
    for op in ops {
        let before = model.clone();
        let (was_stale, was_restored) = model.apply(op);
        // 不変条件: 位置は範囲内で、完了は最終位置と一致する。
        prop_assert!(model.step <= LAST);
        prop_assert_eq!(model.completed, model.step == LAST);
        if was_stale {
            // 古い進行は状態を変えない。
            prop_assert_eq!(&model, &before);
            stale += 1;
        }
        if was_restored {
            // 復元は、保存した時点の状態に一致する。
            let (g, s, c) = model.saved.expect("restored implies saved");
            prop_assert_eq!((model.generation, model.step, model.completed), (g, s, c));
            restored += 1;
        }
    }
    Ok((stale, restored, model.completed))
}

#[test]
fn 乱数の入力列で状態が壊れない() {
    // 成功条件は不変条件のみ。FANDHE_PROPTEST_CASES / SEED で探索を変えても、
    // 操作の網羅性には依存しない（網羅性は下の固定列の試験で確かめる）。
    check(
        "sample-state-invariant",
        DEFAULT_CASES,
        op_sequence(any_op()),
        |ops| verify(&ops).map(|_| ()),
    );
}

#[test]
fn 固定列で古い進行と復元と完了を通る() {
    // 環境変数の影響を受けない明示列。検査が空振りでないことを保証する。
    let ops = [
        Op::Advance,
        Op::Save,
        Op::Stale,
        Op::Advance,
        Op::Restore,
        Op::Advance,
        Op::Advance,
        Op::Advance,
    ];
    let (stale, restored, completed) = verify(&ops).expect("invariants hold");
    assert!(stale > 0, "stale ops were never exercised");
    assert!(restored > 0, "restore was never exercised");
    assert!(completed, "sequence did not reach completion");
}

#[test]
fn 同じ列を流し直すと最終状態が一致する() {
    check(
        "sample-replay",
        DEFAULT_CASES,
        op_sequence(any_op()),
        |ops| assert_replay_eq(run, &ops),
    );
}

#[test]
fn 同じ種なら同じ列が出る() {
    let strategy = op_sequence(any_op());
    let draw = || {
        let mut runner = runner_with(&DEFAULT_SEED, 8);
        (0..8)
            .map(|_| {
                let tree = strategy.new_tree(&mut runner).expect("generate");
                format!("{:?}", tree.current())
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(draw(), draw());
}

#[test]
fn 壊れた性質を検出して最小化後の入力を報告する() {
    let strategy = op_sequence(any_op());
    let result = try_check_with("broken", &DEFAULT_SEED, 64, &strategy, |ops| {
        prop_assert!(ops.len() < 3);
        Ok(())
    });
    let message = result.expect_err("a broken property must be detected");
    assert!(message.contains("minimal input"), "{message}");
    assert!(message.contains(&seed_hex(&DEFAULT_SEED)), "{message}");
}

#[test]
fn 環境変数の不正値は理由付きで拒否する() {
    assert!(parse_cases("abc").is_err());
    assert!(parse_cases("0").is_err());
    assert_eq!(parse_cases("12"), Ok(12));
    assert!(parse_seed("zz").is_err());
    assert!(parse_seed(&"g".repeat(64)).is_err());
    assert_eq!(parse_seed(&seed_hex(&DEFAULT_SEED)), Ok(DEFAULT_SEED));
}
