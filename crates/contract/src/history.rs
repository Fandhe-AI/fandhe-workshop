//! 履歴の入力の列 `ops` の 1 件・書き出しの形・履歴の包み（TASK-309 1・2/3、版 1）。
//!
//! 履歴は、学習者の入力を順に並べた `ops` と、実行環境の写し `snapshot` を一緒に書き出す。
//! 呼び出し元は、`ops` に記録する側（core、TASK-315）、読み込んで適用し直す側（TASK-174・TASK-310）、
//! 書き出しを行う側（web）である。core の `Op` は公開 API 版 1 の範囲で下書き・送信・再読み込みを
//! 含まないため、この crate は操作 `E` を包む総称の列挙 [`HistoryOp`] を置き、`E` には使う側が
//! core の `Op` を束縛する（contract は core に依存しない）。
//!
//! 決定との対応（ID と要旨だけを書く）:
//!
//! | 決定・振る舞い | 型・定数での表れ |
//! |---|---|
//! | D-52・SAVE-17: 書き出しは `ops` と写しの両方 | [`ExportForm::OpsAndSnapshot`]・[`EXPORT_FORM`]。`ops` だけの値は無い |
//! | D-52・SAVE-18: 未送信の下書きは書き出しに入る | [`HistoryOp::Draft`]（`{"draft": …}`）・[`EXPORT_INCLUDES_DRAFT`] |
//! | SAVE-24: Enter の送信は送信ボタンと同じ `submit` | [`HistoryOp::Submit`] だけを持つ。Enter 専用の variant は無い |
//! | D-54・EVT-27: 早送りの入・切は `ops` に入れない | 早送りの variant は無い。[`OPS_RECORDS_FAST_FORWARD`] は `false` |
//! | 履歴の形式 版 1・SAVE-4: 再読み込みは `reload` として残る | [`HistoryOp::Reload`]（`"reload"`） |
//! | SAVE-2: 包みは写しと 4 軸の版を持ち、ログの並びを変えない | [`HistoryEnvelope`]（`ops` は `Vec`、`log` は写しの中の `Vec`） |
//! | SAVE-17: `ops` だけの包みは読み込みで拒否する | `snapshot` は必須の欄。欠落は逆直列化の失敗 |
//!
//! 直列化の形は外部タグ・snake_case で、`{"submit":"…"}`・`{"draft":"…"}`・`"reload"` と、
//! `E` の形をそのまま出す `input`・`tick`・`control` である。`E` が `submit`・`draft`・`reload` の
//! 名前のタグを持ってはならない（タグつきの 3 種を先に読むため）。未知のタグは逆直列化の失敗にする
//! （失敗を拒否に倒す）。
//!
//! 記録側の規則（型では強制しない。TASK-315 が実装する）:
//! - 連続する `draft` は最後の 1 つに畳む。空白だけの送信は記録しない
//! - 拒否された操作・無視された進行も状態を変えるので記録する
//! - `submit` は送った時点の下書きをそのまま持つ。コマンドか自然文かの分類は適用し直しのときに
//!   教材の操作プロファイル（EP-10）で行う
//! - `ops` の要素の順は書き出し・読み込みで変えない
//! - 20,000 文字の送信は 20,000 文字の `submit` として残る。長さ・件数の上限は読み込み口
//!   （[`crate::limits`]、core の `history_size`・`json_depth`・`load_gate`）の責務で、この型は値を保持するだけ
//!
//! 注意: 下書きは未送信の文字列であり、書き出すと利用者が渡す相手にも渡る。書き出す前の案内は画面の責務である。
//! 文字列は利用者由来で信頼できない。HTML へ埋め込むときのエスケープは web 層の責務で、この型は加工しない。
//!
//! 包み [`HistoryEnvelope`] の欄は、直列化の順に `format`・`history_format`・`pack_id`・`versions`・`start`・`ops`・
//! `snapshot` である。`versions` は [`crate::history_version::HistoryVersions`] をそのまま使い、再定義しない。
//! 写し `snapshot` の中身は core の `SessionState` で、contract は core に依存しないため、操作 `E` と同じく
//! 総称の型引数 `S` にして使う側が束縛する（`HistoryEnvelope<Op, SessionState>`）。
//!
//! 写しの 6 要素と `SessionState` の欄の対応（版 1）:
//!
//! | 写しの要素 | `SessionState` の欄 |
//! |---|---|
//! | 実行の状態（世代を含む） | `run`（`RunState`、その中の `generation`） |
//! | ログ | `log`（`Vec<Event>`。並びを変えない。チェックポイントの `log_len` が位置を指す） |
//! | 持ち主の根つきのチェックポイントの表 | `checkpoints`（各 `Checkpoint` の `owner`）・`next_checkpoint` |
//! | 現在の入口の写し | `entry` |
//! | 根ごとの下書き | `drafts` |
//!
//! PoC-6 の固定の履歴の `snapshot` は `state`・`draft`・`draft_root` を持つが、版 1 の写しは core の
//! `SessionState` の形とする（`state` は `run` に、現在の根の下書きも `drafts` に入る）。この差は意図したもので、
//! 固定の履歴（TASK-180 のハッシュ用）を新しい形で開くかどうかは TASK-174・TASK-310 が決める。
//!
//! `format`・`history_format` は `String`・`u32` のまま受け、値を逆直列化では検査しない。読み込みの判定は
//! 「形式名の違い・欄の欠落」を「型の不一致」より先に、「`history_format` が新しい」をその後に分けて行うため、
//! serde の 1 回の失敗では区別できない。値の照合は TASK-310・TASK-174 の責務である（`history_version` が版の
//! 文字列を `String` のまま受けて `parse` で検査するのと同じ方針）。
//!
//! 注意: `S` に `Option<_>` を束縛してはならない。serde は欠落した欄を `Option` なら `None` で受けるため、
//! `snapshot` の欠落（`ops` だけの包み）が通ってしまう。
//!
//! JSON の形（版 1。スキーマはこの表・型の `deny_unknown_fields`・サンプルとのバイト一致で固定する）:
//!
//! | 欄 | 型 | 必須 | 制約・例 |
//! |---|---|---|---|
//! | `format` | 文字列 | 必須 | 書くときは `"fandhe-workshop-history"` |
//! | `history_format` | 整数 | 必須 | 書くときは `1` |
//! | `pack_id` | 文字列 | 必須 | 教材の ID。例 `"min-3step"` |
//! | `versions` | 版の欄 | 必須 | `engine`・`ir_format`・`vocab`・`content`・`content_hash`・`refs`。`refs` は空でも `{}` |
//! | `start` | 文字列 | 必須 | 最初の開始点の ID。例 `"s-1"` |
//! | `ops` | 配列 | 必須 | 空でもよい。要素は `{"submit":"…"}`・`{"draft":"…"}`・`"reload"`・`{"input":…}`・`{"tick":…}`・`{"control":…}` の 6 種 |
//! | `snapshot` | 写し | 必須 | core の `SessionState`（`run`・`log`・`entry`・`checkpoints`・`next_checkpoint`・`drafts`・`hints_opened`。`run` 内の `RunState` には `guidance_level`）。欠落は拒否 |
//!
//! 最上位に未知の欄は無い（あれば拒否）。`ops` に無いもの: Enter 専用の送信・早送りの入切・`ops` だけの包み。
//! サンプルは `testdata/contract/history/`（説明は同じ場所の `README.md`）にある。
//!
//! 境界（本契約が定義しないもの）:
//! - サンプル（`testdata/contract/history/`）・契約適合試験（core の `history_envelope_conformance`）・
//!   ログの並びを変える変異（`SAVE2-1`）は TASK-309 3/3 で置いた。`ops` を適用し直した結果と写しの一致は含まない
//! - 保存先の口・鍵・拒否理由コードと判定の順序は TASK-310。早送り・案内レベルの操作の追加は版上げで行う
//! - core 側の公開の別名と `ops` への記録は TASK-315（core の `ops_record`）
//!
//! 包みの文字列（`pack_id`・`start`・下書き・送信）は利用者・教材由来で信頼できず、この型は加工しない。
//! 大きさ・入れ子の深さ・件数の上限もこの型は持たない。
//!
//! 変更する場合は、先に spec を直し、[`HISTORY_CONTRACT_VERSION`] を上げる。TASK-309 の 2/3・3/3 は
//! 同じ版 1 の範囲で欄を足し、新たな版の定数を作らずこの定数を参照する。

use serde::{Deserialize, Serialize};

use crate::history_version::HistoryVersions;

/// この契約の版。履歴の包みを含む TASK-309 全体で共有する。
pub const HISTORY_CONTRACT_VERSION: u32 = 1;

/// 履歴の入力の列 `ops` の 1 件。`E` はエンジンの操作（core の `Op`）。
///
/// 列挙の追加は契約の版上げで行うため、使う側の網羅 match を保つよう `non_exhaustive` にしない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryOp<E> {
    /// 下書きの送信。送った時点の下書きの文字列（切り詰め・分類をしない）。
    Submit(String),
    /// 下書きの置き換え。
    Draft(String),
    /// 再読み込み。
    Reload,
    /// エンジンの操作（`input`・`tick`・`control`）。タグを足さず `E` の形をそのまま出す。
    #[serde(untagged)]
    Engine(E),
}

/// 履歴の書き出しの形（D-52、2026-10-12）。`ops` だけの形は選べる値として置かない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExportForm {
    /// 入力の列 `ops` と、実行環境の写し `snapshot` を両方持つ。
    OpsAndSnapshot,
}

/// 書き出しの形の決定値（D-52、2026-10-12）。
pub const EXPORT_FORM: ExportForm = ExportForm::OpsAndSnapshot;

/// 未送信の下書きを書き出しに含めるか（D-52・SAVE-18、2026-10-12）。
pub const EXPORT_INCLUDES_DRAFT: bool = true;

/// 早送りの入・切を `ops` に記録するか（D-54・EVT-27）。実行環境の設定値は履歴に入れない。
pub const OPS_RECORDS_FAST_FORWARD: bool = false;

/// 包みの `format` に書く形式名。
pub const HISTORY_FORMAT_NAME: &str = "fandhe-workshop-history";

/// 履歴の包み（SAVE-2・D-52）。`E` はエンジンの操作、`S` は実行環境の全体の写し（core の `SessionState`）。
///
/// 全欄が必須で、未知の欄は拒否する。`snapshot` が必須なので `ops` だけの包みは作れず、読み込みでも拒否される。
/// 直列化の順は欄の宣言の順で、固定の履歴の最上位のキー順に合わせている。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryEnvelope<E, S> {
    /// 形式名。書くときは [`HISTORY_FORMAT_NAME`]。
    pub format: String,
    /// 包みの形の版。書くときは [`HISTORY_CONTRACT_VERSION`]。
    pub history_format: u32,
    /// 教材の ID。
    pub pack_id: String,
    /// 4 軸の版の欄。
    pub versions: HistoryVersions,
    /// 最初の開始点の ID。
    pub start: String,
    /// 入力の列。順を変えない。
    pub ops: Vec<HistoryOp<E>>,
    /// 実行環境の全体の写し。
    pub snapshot: S,
}

impl<E, S> HistoryEnvelope<E, S> {
    /// `format`・`history_format` を定数で埋めて包みを作る（書く側が値を間違えないため）。
    pub fn new(
        pack_id: String,
        versions: HistoryVersions,
        start: String,
        ops: Vec<HistoryOp<E>>,
        snapshot: S,
    ) -> Self {
        Self {
            format: HISTORY_FORMAT_NAME.to_string(),
            history_format: HISTORY_CONTRACT_VERSION,
            pack_id,
            versions,
            start,
            ops,
            snapshot,
        }
    }
}
