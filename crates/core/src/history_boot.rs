//! 起動時の分岐。保存済みの履歴が無い・未操作なら新しく始め、あれば復元する（TASK-319 1/3、SAVE-3・SAVE-28）。
//!
//! 呼び出し元は web のホストの起動（保存先から読んだ値を渡す）である。web の表示層は状態を作り直さず、
//! 再読み込みも呼ばない（`crates/web/tests/no_progress.rs`）ため、`Session::from_state` と `reload` を伴う分岐は core に置く。
//! 判定そのもの（段 1〜8）は [`crate::history_load`] が持ち、ここでは複製しない。
//!
//! 規則（上から評価する）。
//! 1. 保存済みの値が無い → [`BootOutcome::Fresh`]。初期状態のまま、再読み込みは行わず、世代は上がらない（SAVE-3）
//! 2. 開けて、`ops` が 0 件 → [`BootOutcome::Fresh`]。未操作のページの再読み込みは新しく始まるだけで世代は上がらない
//!    （SAVE-28。最初の状態は最初の変更まで保存しないため害は無い。0 件の包みは通常は保存されず、外から置かれた場合への防御）
//! 3. 開けた（変換を含む）→ [`BootOutcome::Restored`]。適用し直した状態に再読み込みを 1 回かけ、記録の末尾に `Reload` を足す
//!    （世代 +1・`Reloaded` 1 件。SAVE-4）
//! 4. 表示だけ・開かない → [`BootOutcome::NotOpened`]。初期状態で始める。呼び出し側は保存済みの値を上書きしない（fail-closed）
//!
//! 純関数で、I/O・時刻・乱数を持たない。理由は固定のコードだけで、履歴の中身を含めない（SAFE-40）。

use fandhe_workshop_contract::history_version::{ConversionNotes, HistoryVersions, Verdict};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::load_reason::LoadReason;

use crate::history_load::load_history_with_envelope;
use crate::history_open::{Opened, OpeningCaps};
use crate::ops_record::OpsRecorder;
use crate::recorded_session::RecordedSession;
use crate::session::{Session, SessionState};

/// 起動の結果。
#[derive(Debug)]
pub enum BootOutcome {
    /// 新しく始める。初期状態のまま、世代・ログは変わらない。
    Fresh(RecordedSession),
    /// 保存済みの履歴を開き、再読み込みをかけた。
    Restored {
        /// 復元した実行環境。記録は読み込んだ `ops` に `Reload` を 1 件足したもの。
        session: RecordedSession,
        /// 版の違いの記録。変換なしで開いたときは `None`。
        notes: Option<ConversionNotes>,
    },
    /// 開かなかった。初期状態で始めるが、呼び出し側は保存済みの値を上書きしない。
    NotOpened {
        /// 初期状態の実行環境。
        session: RecordedSession,
        /// 開かなかった判定（表示だけ・開かない）。
        verdict: Verdict<LoadReason>,
    },
}

/// 保存済みの値（あれば）から、新しく始めるか復元するかを決めて実行環境を作る。
///
/// `start` は新しく始めるときの最初の開始点の ID。復元では履歴の開始点を使う。
pub fn boot_session(
    saved: Option<&[u8]>,
    ir: Ir,
    opening: &HistoryVersions,
    caps: &OpeningCaps,
    initial: SessionState,
    start: &str,
) -> BootOutcome {
    let fresh = |ir: Ir, initial: SessionState| {
        RecordedSession::new(Session::from_state(ir, initial), start)
    };
    let Some(bytes) = saved else {
        return BootOutcome::Fresh(fresh(ir, initial));
    };
    let (opened, envelope) = load_history_with_envelope(bytes, &ir, opening, caps, initial.clone());
    let (state, notes) = match opened {
        Opened::Open { state } => (state, None),
        Opened::Converted { state, notes } => (state, Some(notes)),
        other => {
            return BootOutcome::NotOpened {
                session: fresh(ir, initial),
                verdict: other.verdict(),
            }
        }
    };
    // 開けたなら包みはある。無ければ開けた扱いにせず、新しく始める側（上書きしない側）に倒す。
    let Some(envelope) = envelope else {
        return BootOutcome::NotOpened {
            session: fresh(ir, initial),
            verdict: Verdict::Refused {
                code: LoadReason::Malformed,
            },
        };
    };
    if envelope.ops.is_empty() {
        return BootOutcome::Fresh(fresh(ir, initial));
    }
    let mut session = RecordedSession::with_recorder(
        Session::from_state(ir, state),
        OpsRecorder::from_ops(envelope.ops),
        envelope.start,
    );
    session.reload();
    BootOutcome::Restored { session, notes }
}
