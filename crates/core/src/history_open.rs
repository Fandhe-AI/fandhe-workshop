//! 履歴を開く側の版と突き合わせ、開く可否を判定する（TASK-174 1/3、VER-3〜VER-5）。
//!
//! 履歴は 4 軸の版（エンジン・教材形式・教材内容・参照元）を持つ。開く側と同一なら変換せずに開き（VER-3）、
//! 教材内容の副版・修正版、語彙の宣言、エンジンの副版、参照元の commit だけが違うなら、`ops` を適用し直して
//! 写しと一致したときに限り「変換して開く」とする（VER-4）。一致しなければ表示だけにする。副版で Step を足した
//! 教材では、足した Step より先へ進んだ履歴の適用し直しが写しと食い違うため、表示だけになる（VER-5）。
//! 写しだけを信じて開かないのは、履歴が未信頼入力だからである。
//!
//! 呼び出し元は、読み込みの経路（TASK-331）、web の復元（TASK-176・TASK-178）、15 件の試験（TASK-175）である。
//! 判定の順序は [`HISTORY_LOAD_ORDER`](fandhe_workshop_contract::load_reason::HISTORY_LOAD_ORDER) の段 5〜8 に
//! 従い、1 段を 1 行の条件と早期 return で書く。I/O・時刻・乱数を持たない純関数で、全域で panic しない。
//!
//! 版の違いは [`ConversionNotes`] の構造のまま返し、文面には整形しない。履歴由来の文字列（参照元の名前・
//! commit・版）を文面に埋め込まず、表示層が文脈に応じてエスケープするためである（SAFE-40）。
//! 理由コードは `&'static str` だけで、読み込んだ内容を含まない。
//!
//! 境界（含まないもの）。
//! - 段 1〜5 の大きさ・深さ・JSON・版の欄・型の検査と `too_many_ops`（TASK-331）。呼び出し側が検査済みの包みを渡す
//!   前提で、適用し直しの時間は `ops` の件数に比例する
//! - `content_major`・`format_older`・`engine_major` の厳密な試験と境界（TASK-174 2/3）。本件は fail-closed の
//!   ため比較だけを置き、これらを「変換して開く」に到達させない
//! - `vocab_newer`・`format_newer`・`engine_newer` の厳密な試験と、線引きが D-52・D-59 と一致することの確認
//!   （TASK-174 3/3）。エンジンが読める語彙の上限の正の置き場もそこで決めるため、[`OpeningCaps`] で受ける
//! - 写しとの照合の性質テストと `replay_mismatch` の作り込み（TASK-316 2/3）。本件は同一版で一致しない履歴を
//!   `replay_diverged` と誤って分類しない振り分け 1 か所だけを持つ
//! - `start` からの初期状態の構築（TASK-217・TASK-294）。初期状態は引数で受ける
//!
//! 【推論】[`ReplayError`](crate::history_replay::ReplayError)（空白だけの送信・未実装の再読み込み・読めない
//! プロファイル）は、写しの不一致と同じ振り分けに入れる。型つきで拒否した適用し直しの失敗を「開く」に倒さない
//! ためで、TASK-316 2/3・TASK-320 で見直す。

use fandhe_workshop_contract::history::HISTORY_CONTRACT_VERSION;
use fandhe_workshop_contract::history_version::{
    ConversionNotes, HistoryVersions, Verdict, VocabVersion,
};
use fandhe_workshop_contract::ir::{Ir, SUPPORTED_IR_FORMATS};
use fandhe_workshop_contract::load_reason::LoadReason;

use crate::history_envelope::SessionEnvelope;
use crate::history_replay::replay;
use crate::session::SessionState;

/// 開く側が読める上限。履歴の外から渡し、既定値で黙って緩めない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningCaps {
    /// 読める IR の構造の版の最大。
    pub ir_format_max: u32,
    /// エンジンが読める語彙の版の最大（正の置き場は TASK-174 3/3 で決める）。
    pub vocab_max: VocabVersion,
}

impl OpeningCaps {
    /// IR の構造の版は契約の対応表の最大にし、語彙の上限だけを呼び出し側が決める。
    pub fn with_vocab_max(vocab_max: VocabVersion) -> Self {
        let ir_format_max = SUPPORTED_IR_FORMATS.iter().copied().max().unwrap_or(0);
        Self {
            ir_format_max,
            vocab_max,
        }
    }
}

/// 判定の結果。状態を持つのは開く 2 種だけで、表示だけ・開かないは状態を作らない（VER-2）。
#[derive(Debug, Clone, PartialEq)]
pub enum Opened {
    /// 変換なしで開く。
    Open {
        /// 適用し直して得た状態。
        state: SessionState,
    },
    /// 変換して開く。
    Converted {
        /// 適用し直して得た状態。
        state: SessionState,
        /// 版の違い（1 件以上）。
        notes: ConversionNotes,
    },
    /// 表示だけ。
    ViewOnly {
        /// 理由。
        code: LoadReason,
    },
    /// 開かない。
    Refused {
        /// 理由。
        code: LoadReason,
    },
}

impl Opened {
    /// 状態を持たない判定結果へ落とす。
    pub fn verdict(&self) -> Verdict<LoadReason> {
        match self {
            Opened::Open { .. } => Verdict::Open,
            Opened::Converted { notes, .. } => Verdict::Converted {
                notes: notes.clone(),
            },
            Opened::ViewOnly { code } => Verdict::ViewOnly { code: *code },
            Opened::Refused { code } => Verdict::Refused { code: *code },
        }
    }

    /// 開いた場合の状態。表示だけ・開かないでは `None`。
    pub fn state(&self) -> Option<&SessionState> {
        match self {
            Opened::Open { state } | Opened::Converted { state, .. } => Some(state),
            Opened::ViewOnly { .. } | Opened::Refused { .. } => None,
        }
    }
}

/// 履歴を開く側の版と突き合わせて、開く可否を判定する。
///
/// `opening` は開く側の 4 軸の版（[`record_versions`](crate::version_record::record_versions) 系で作る。
/// エンジンの版を模せるよう引数で受ける）。`initial` は初期状態、`history` は段 1〜5 の型の検査を済ませた包み。
pub fn open_history(
    ir: &Ir,
    opening: &HistoryVersions,
    caps: &OpeningCaps,
    initial: SessionState,
    history: &SessionEnvelope,
) -> Opened {
    let refuse = |code| Opened::Refused { code };
    let view_only = |code| Opened::ViewOnly { code };
    // 段 5 の残り（包みから判定できるもの）。
    if history.history_format > HISTORY_CONTRACT_VERSION {
        return refuse(LoadReason::HistoryFormatNewer);
    }
    if history.pack_id != ir.pack.id {
        return refuse(LoadReason::OtherPack);
    }
    // 段 6。
    let (Ok(h), Ok(o)) = (history.versions.parse(), opening.parse()) else {
        return refuse(LoadReason::BadVersion);
    };
    let hv = &history.versions;
    // 履歴の版は、エンジンが読める最大版だけでなく、開く教材の IR の版とも比べる（VER-4 は ir_format の差を
    // 変換の対象に含めないため、新しい側は閉じる）。
    if hv.ir_format > caps.ir_format_max || hv.ir_format > ir.format_version {
        return refuse(LoadReason::FormatNewer);
    }
    if h.vocab > caps.vocab_max || o.vocab > caps.vocab_max {
        return refuse(LoadReason::VocabNewer);
    }
    if h.engine > o.engine {
        return refuse(LoadReason::EngineNewer);
    }
    if hv.ir_format < ir.format_version {
        return view_only(LoadReason::FormatOlder);
    }
    if !h.engine.same_major(&o.engine) {
        return view_only(LoadReason::EngineMajor);
    }
    if !h.content.same_major(&o.content) {
        return view_only(LoadReason::ContentMajor);
    }
    // 段 7。
    if !ir.starts.iter().any(|s| s.id == history.start) {
        return view_only(LoadReason::StartMissing);
    }
    // 段 8。適用し直した結果が写しと一致したときだけ開く。
    if let Ok(state) = replay(ir, initial, &history.ops) {
        if state == history.snapshot {
            return match ConversionNotes::new(hv.diff(opening)) {
                None => Opened::Open { state },
                Some(notes) => Opened::Converted { state, notes },
            };
        }
    }
    // 版の違いで説明できない不一致は、書き換え・壊れとみなして開かない。比べるのは 6 欄すべてで、
    // 参照元の commit（`refs`）だけの違いも版の違いに数える（冒頭の VER-4 と同じ線引き）。
    let same_version = hv.content_hash == opening.content_hash
        && hv.content == opening.content
        && hv.engine == opening.engine
        && hv.ir_format == opening.ir_format
        && hv.vocab == opening.vocab
        && hv.refs == opening.refs;
    if same_version {
        refuse(LoadReason::ReplayMismatch)
    } else {
        view_only(LoadReason::ReplayDiverged)
    }
}
