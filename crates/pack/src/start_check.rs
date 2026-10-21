//! 開始点の `requires` と `unmet` を両方そろえて書くことの検査（TASK-51、PACK-18、EP-9）。
//!
//! `compile` と `validate` の構造の検査から呼ばれる。片方だけの開始点は、条件があるのに断る文面が無い、
//! または文面があるのに条件が無い状態になり、実行時（core の `start_gate`）の判定が成り立たないため
//! `E-STRUCTURE` で拒否する。人が書く層の `starts` を見るので、展開の有無・モジュールが呼ばれるかに依らない
//! （`expand::check_use_steps` と同じ扱い）。型の誤り・参照の誤りは `condition_check`・参照の検査が担う。
//!
//! `requires: null` は読み込みの型（`Option`）で省略と区別できないため、省略と同じ扱いになる。
//! 診断の文面は固定文で、条件・文面の値を写さない。

use crate::diag::{DiagCode, Diagnostic};
use crate::source::LoadedScope;
use crate::validate::diag;

/// 全スコープの開始点で、`requires` と `unmet` の片方だけが書かれたものを `E-STRUCTURE` にする。
pub(crate) fn check_start_pairs(scopes: &[LoadedScope]) -> Vec<Diagnostic> {
    scopes
        .iter()
        .flat_map(|scope| {
            scope.src.starts.iter().filter_map(move |s| {
                let missing = match (s.requires.is_some(), s.unmet.is_some()) {
                    (true, false) => "unmet",
                    (false, true) => "requires",
                    _ => return None,
                };
                Some(diag(
                    &scope.file,
                    &format!("start:{}", s.id),
                    DiagCode::Structure,
                    format!("requires and unmet must be written together ({missing} is missing)"),
                ))
            })
        })
        .collect()
}
