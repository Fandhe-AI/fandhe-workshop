//! 履歴の入力の列 `ops` の 1 件と、書き出しの形の契約（TASK-309 1/3、版 1）。
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
//! 境界（本契約が定義しないもの）:
//! - 包みの欄（`format`・`history_format`・`pack_id`・`start`・`snapshot`・`versions`）は TASK-309 2/3。
//!   `snapshot` を必須の欄にすることで、`ops` だけの包みは読み込みで拒否される
//! - スキーマ・サンプル・往復の適合試験は TASK-309 3/3
//! - 保存先の口・鍵・拒否理由コードは TASK-310。早送り・案内レベルの操作の追加は版上げで行う
//!
//! 変更する場合は、先に spec を直し、[`HISTORY_CONTRACT_VERSION`] を上げる。TASK-309 の 2/3・3/3 は
//! 同じ版 1 の範囲で欄を足し、新たな版の定数を作らずこの定数を参照する。

use serde::{Deserialize, Serialize};

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
