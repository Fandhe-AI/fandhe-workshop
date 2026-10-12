//! 性質テストの共通ヘルパ。
//!
//! 後続のエンジンの試験（状態機械・復元・古い進行の無効化など）が、乱数の入力列を決定的に流すための
//! 土台である。種・ケース数・失敗時の出力・再実行の一致の確かめ方を、試験ごとに書き直さないために集約する。
//!
//! # 使い方
//!
//! ```ignore
//! mod common;
//! use common::property::{check, op_sequence, DEFAULT_CASES};
//!
//! #[test]
//! fn 状態が壊れない() {
//!     check("state-invariant", DEFAULT_CASES, op_sequence(any_op()), |ops| {
//!         /* ops を適用し prop_assert! で性質を確かめる */
//!         Ok(())
//!     });
//! }
//! ```
//!
//! # 決定性と再現
//!
//! - 種は [`DEFAULT_SEED`] に固定する。同じコードなら毎回同じ列が流れ、失敗は必ず再現する
//! - 種を変えた探索は環境変数 `FANDHE_PROPTEST_SEED`（64 桁の 16 進）、ケース数の変更は
//!   `FANDHE_PROPTEST_CASES`（正の 10 進整数）で行う。不正な値は黙って既定へ戻さず、理由を示して失敗させる
//! - 失敗時の出力に試験名・ケース数・種・最小化後の入力を含める。同じ種とケース数で再実行すれば再現する
//! - `failure_persistence` は無効にする。有効だと `proptest-regressions/` をソースツリーへ書き出し、
//!   作業ツリーを書き換えてしまうため

use std::fmt::Debug;

use proptest::collection::vec;
use proptest::prelude::*;
use proptest::strategy::Strategy;
use proptest::test_runner::{Config, RngAlgorithm, TestCaseError, TestError, TestRng, TestRunner};

/// 既定の種。毎回同じ列を流し、失敗を再現できるようにする。
pub const DEFAULT_SEED: [u8; 32] = [
    0x46, 0x41, 0x4e, 0x44, 0x48, 0x45, 0x2d, 0x50, 0x52, 0x4f, 0x50, 0x54, 0x45, 0x53, 0x54, 0x2d,
    0x53, 0x45, 0x45, 0x44, 0x2d, 0x76, 0x31, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
];

/// 既定のケース数。サンプルを速く回すための値で、1,000 通りなどは各試験が明示で渡す。
pub const DEFAULT_CASES: u32 = 256;

/// 1 列の操作数の上限。列の長さをそろえ、縮小（shrink）の時間を抑える。
pub const MAX_OPS: usize = 50;

/// 種を上書きする環境変数名。
pub const ENV_SEED: &str = "FANDHE_PROPTEST_SEED";

/// ケース数を上書きする環境変数名。
pub const ENV_CASES: &str = "FANDHE_PROPTEST_CASES";

/// 種を 16 進文字列にする。報告行・失敗時の出力で使う。
pub fn seed_hex(seed: &[u8; 32]) -> String {
    seed.iter().map(|b| format!("{b:02x}")).collect()
}

/// 64 桁の 16 進文字列を種に戻す。不正なら理由を返す。
pub fn parse_seed(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64 || !text.is_ascii() {
        return Err(format!(
            "{ENV_SEED} must be 64 hex digits (got {} chars)",
            text.len()
        ));
    }
    let mut seed = [0u8; 32];
    for (i, slot) in seed.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16)
            .map_err(|_| format!("{ENV_SEED} contains a non-hex character"))?;
    }
    Ok(seed)
}

/// ケース数の文字列を解析する。0 や数値以外は理由を返す。
pub fn parse_cases(text: &str) -> Result<u32, String> {
    match text.parse::<u32>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(format!(
            "{ENV_CASES} must be a positive decimal integer (got {text:?})"
        )),
    }
}

fn env_override<T>(name: &str, parse: fn(&str) -> Result<T, String>) -> Result<Option<T>, String> {
    match std::env::var(name) {
        Ok(text) => parse(&text).map(Some),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{name} is not valid unicode")),
    }
}

/// 環境変数の上書きを反映した（種, ケース数）を返す。不正な値は `Err`。
pub fn resolve(default_cases: u32) -> Result<([u8; 32], u32), String> {
    let seed = env_override(ENV_SEED, parse_seed)?.unwrap_or(DEFAULT_SEED);
    let cases = env_override(ENV_CASES, parse_cases)?.unwrap_or(default_cases);
    Ok((seed, cases))
}

/// 種から決定的な `TestRunner` を作る。`failure_persistence` は無効。
pub fn runner_with(seed: &[u8; 32], cases: u32) -> TestRunner {
    let config = Config {
        cases,
        failure_persistence: None,
        ..Config::default()
    };
    TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, seed))
}

/// 1〜[`MAX_OPS`] 個の操作からなる入力列の戦略を作る。
pub fn op_sequence<S>(op: S) -> impl Strategy<Value = Vec<S::Value>>
where
    S: Strategy,
{
    vec(op, 1..=MAX_OPS)
}

/// 種とケース数を明示して性質を流す。失敗は整形済みの文字列で返す。
pub fn try_check_with<S, F>(
    name: &str,
    seed: &[u8; 32],
    cases: u32,
    strategy: &S,
    body: F,
) -> Result<(), String>
where
    S: Strategy,
    S::Value: Debug,
    F: Fn(S::Value) -> Result<(), TestCaseError>,
{
    let mut runner = runner_with(seed, cases);
    match runner.run(strategy, body) {
        Ok(()) => Ok(()),
        Err(TestError::Fail(reason, minimal)) => Err(format!(
            "property {name:?} failed: {reason}\n  cases={cases} seed={}\n  minimal input: {minimal:?}",
            seed_hex(seed)
        )),
        Err(TestError::Abort(reason)) => Err(format!(
            "property {name:?} aborted: {reason}\n  cases={cases} seed={}",
            seed_hex(seed)
        )),
    }
}

/// 環境変数の上書きを反映して性質を流す（panic しない版）。
pub fn try_check<S, F>(name: &str, cases: u32, strategy: &S, body: F) -> Result<(), String>
where
    S: Strategy,
    S::Value: Debug,
    F: Fn(S::Value) -> Result<(), TestCaseError>,
{
    let (seed, cases) = resolve(cases)?;
    try_check_with(name, &seed, cases, strategy, body)?;
    println!(
        "PROPERTY-TEST name={name} cases={cases} seed={}",
        seed_hex(&seed)
    );
    Ok(())
}

/// 性質を流し、失敗なら名前・ケース数・種・最小化後の入力を含めて panic する。
///
/// 各試験の `#[test]` 関数から呼ぶ入口。成功時は報告行を 1 行出力する。
pub fn check<S, F>(name: &str, cases: u32, strategy: S, body: F)
where
    S: Strategy,
    S::Value: Debug,
    F: Fn(S::Value) -> Result<(), TestCaseError>,
{
    if let Err(message) = try_check(name, cases, &strategy, body) {
        panic!("{message}");
    }
}

/// 同じ入力列を 2 回流し、最終状態が一致することを確かめる（再実行の一致）。
///
/// `run` は入力列から最終状態を作る純粋な関数であること。
pub fn assert_replay_eq<Op, State, F>(run: F, ops: &[Op]) -> Result<(), TestCaseError>
where
    State: PartialEq + Debug,
    F: Fn(&[Op]) -> State,
{
    let first = run(ops);
    let second = run(ops);
    prop_assert_eq!(first, second);
    Ok(())
}
