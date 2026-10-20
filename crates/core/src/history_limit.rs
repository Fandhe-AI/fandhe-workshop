//! 保存の前に、履歴の包みが読み込みの上限を超えていないかを判定する（Issue #1414、SAVE-19・SAFE-41・SAVE-24）。
//!
//! 呼び出し元は、履歴を保存先へ積む層（`persist`。TASK-318・TASK-319）と、保存しなかったことを知らせる画面（TASK-531）である。
//! 記録の側（[`crate::ops_record`]）は上限を参照しない。読み込みの側（[`crate::history_load`]）は `ops` が
//! [`MAX_HISTORY_OPS`] を超えると `too_many_ops` で断る。そのため、記録を続けた末に 20,001 件目以降を含む包みを保存すると、
//! 読み込めない履歴が最後に読み込める保存を上書きしうる。ここは、その保存を止める判定だけを持つ純関数である。
//! I/O・時刻・乱数を持たず、履歴の中身（文字列）を受けない。受けるのは長さだけである。
//!
//! 規則（推論）。spec に記録中に上限を超えたときの規則は無い。次を安全側の仮の規則として実装した。
//! D-n としての決定と SAVE-19・SAFE-41・SAVE-24 への記録は、人が spec のリポジトリで行う（未了）。
//! 1. 記録は止めない。[`crate::ops_record::OpsRecorder`] は 20,001 件目以降も順に追記し、切り詰めも並べ替えもしない。
//!    復元は初期状態からの適用し直しで `ops` の全体を要し、記録されない操作は開き直したときの写しとの食い違いになるため
//! 2. 保存は、読み込めない包みを書かない。[`check_save`] が `Err` を返したら保存先へ書かず、最後に読み込めた保存を残す
//! 3. 書き出し（[`crate::history_export`]）は止めない。受講者の唯一の持ち出し手段のため、印で知らせる方針を保つ
//! 4. 知らせは画面の責務である。ここは理由コード（[`LimitReason`]）までを返し、文面を持たない
//! 5. 判定の順は読み込みと同じにする。大きさ（`too_large`）を先に、件数（`too_many_ops`）を後に見る
//!
//! 境界。上限の値は契約（[`MAX_HISTORY_BYTES`]・[`MAX_HISTORY_OPS`]）の定数を使い、ここで定義しない。
//! 契約の版の対象外の module である。

use fandhe_workshop_contract::limits::{LimitReason, MAX_HISTORY_BYTES, MAX_HISTORY_OPS};

/// `ops` の件数が読み込みの上限（[`MAX_HISTORY_OPS`]）以内か。ちょうど上限は以内である。
#[must_use]
pub fn ops_within_limit(len: usize) -> bool {
    len <= MAX_HISTORY_OPS
}

/// 保存してよい包みか。直列化したバイト数と `ops` の件数から、読み込みの上限を超えるかを判定する。
///
/// 大きさ、件数の順に見る（読み込みの段の順。[`LimitReason::stage`]）。バイト数を `u64` に直せないときは超過として扱う
/// （fail-closed）。
///
/// # Errors
///
/// バイト数が [`MAX_HISTORY_BYTES`] を超えれば `too_large`、`ops` が [`MAX_HISTORY_OPS`] を超えれば `too_many_ops`。
pub fn check_save(bytes_len: usize, ops_len: usize) -> Result<(), LimitReason> {
    if u64::try_from(bytes_len).map_or(true, |n| n > MAX_HISTORY_BYTES) {
        return Err(LimitReason::TooLarge);
    }
    if !ops_within_limit(ops_len) {
        return Err(LimitReason::TooManyOps);
    }
    Ok(())
}
