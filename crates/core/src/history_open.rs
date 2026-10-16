//! 履歴を開く側の版と突き合わせ、開く可否を判定する（TASK-174、VER-3〜VER-7）。
//!
//! 履歴は 4 軸の版（エンジン・教材形式・教材内容・参照元）を持つ。開く側と同一なら変換せずに開き（VER-3）、
//! 教材内容の副版・修正版、語彙の宣言、エンジンの副版、参照元の commit だけが違うなら、`ops` を適用し直して
//! 写しと一致したときに限り「変換して開く」とする（VER-4）。一致しなければ表示だけにする。副版で Step を足した
//! 教材では、足した Step より先へ進んだ履歴の適用し直しが写しと食い違うため、表示だけになる（VER-5）。
//! 写しだけを信じて開かないのは、履歴が未信頼入力だからである。
//!
//! 表示だけになる版の違いは 3 種ある（VER-6・VER-7）。`content_major` は教材内容の主版が違えば向きを問わず、履歴の
//! 位置にもよらず表示だけにする（適用し直しが写しと一致する手前の履歴でも開かない）。`format_older` は履歴の IR の版が
//! 開く教材より小さいときで、エンジンが変換の規則を持たない（D-52）。`engine_major` はエンジンの主版が違うときで、
//! 履歴の側が新しい場合は先に `engine_newer` が当たり開かない。順序は契約の段 6 のとおり
//! `engine_newer` → `format_older` → `engine_major` → `content_major` で、段 7・段 8 より先である。
//!
//! 開かない版の違いは 3 種ある（VER-7）。向きはいずれも「履歴の側が新しい」で、エンジンが知らない形を誤って描かないため
//! 閉じる。`format_newer` は履歴の IR の版が、エンジンの上限（[`OpeningCaps::ir_format_max`]）と開く教材の IR の版の
//! どちらかを超えたとき。`vocab_newer` は履歴または開く側の語彙の版が、エンジンの上限（[`VOCAB_MAX`]）を超えたとき
//! （主→副の数値の比較。等しい版は開く側に入る）。`engine_newer` は履歴のエンジンの版が開く側を主→副→修の順で
//! 超えたとき（主版が同じで副版・修正版だけ新しい場合も含む）。読めない版の文字列は、先に `bad_version` で開かない。
//! 順序は契約の段 6 のとおり `bad_version` → `format_newer` → `vocab_newer` → `engine_newer` で、段 7・段 8 より先である。
//! 「表示だけ」「開かない」のどの結果も、空でない理由コードを持つ。
//!
//! 線引きと決定の記録の照合（出所を分ける）。
//! - D-52 の 6（VER-18）と一致: 教材内容の主版の違いは向きを問わず表示だけ。副版以下は適用し直して写しと一致したときだけ変換して開く
//! - D-52 の 7・D-59 の 2 と一致: 履歴の IR の版が古ければ `format_older`（表示だけ）、新しければ `format_newer`（開かない）。
//!   ただし D-59 の 4 のとおり、理由は Claude Code の判断で、ユーザーの理由としては記録されていない。「開く側」を
//!   エンジンの上限と開く教材の両方に読むのは実装の読みである
//! - VER-7（PoC-6 の試作の線引き。V-iv-2・V-v-2）に従う: `vocab_newer`・`engine_newer`（開かない）と `engine_major`（表示だけ）。
//!   D-52・D-59 はこの 3 種を直接は決めていない
//!
//! エンジンが読める語彙の上限の正は [`VOCAB_MAX`] で、`docs/vocabulary/` の最新版（pack の `vocab::LATEST`）と
//! 一致することを pack の試験が見る。
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

/// エンジンが読める語彙の版の上限（正）。
///
/// 正は `docs/vocabulary/` の最新版で、pack の `vocab::LATEST` と一致することを pack の試験が見る。
/// 語彙の版を上げるときは、両方を同時に変える。契約（contract）ではなく判定が持つのは、開く側の上限が
/// 判定（TASK-174）の持ち物だからである（`history_version` の境界の節）。
pub const VOCAB_MAX: VocabVersion = VocabVersion { major: 1, minor: 3 };

/// 開く側が読める上限。履歴の外から渡し、既定値で黙って緩めない（`Default` は実装しない）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpeningCaps {
    /// 読める IR の構造の版の最大。
    pub ir_format_max: u32,
    /// エンジンが読める語彙の版の最大（現行は [`VOCAB_MAX`]）。
    pub vocab_max: VocabVersion,
}

impl OpeningCaps {
    /// 現行のエンジンの上限。IR の構造の版は契約の対応表の最大、語彙は [`VOCAB_MAX`]。
    pub fn current() -> Self {
        Self::with_vocab_max(VOCAB_MAX)
    }

    /// IR の構造の版は契約の対応表の最大にし、語彙の上限だけを呼び出し側が決める。
    /// 語彙 v1 の実装を模す試験（V-iv-2）の口で、通常は [`Self::current`] を使う。
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
