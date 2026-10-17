//! 宛先の部品（世代・Step の実体 ID・進行の宛先）のデータの形と書式検査（TASK-204 1/3・2/3）。
//!
//! `Op::Tick` とすべてのイベントの `at` が参照する。呼び出し元は web・pack・保存の担当である。
//! 本モジュールは形に加えて、実体 ID の書式検査（[`StepUid::parse`]）と根の取り出し（[`StepUid::root`]）、
//! 世代の初期値と増やし方（[`Generation::INITIAL`]・[`Generation::next`]）を持つ。
//! 宛先の照合の実装は TASK-210、世代の変わり方の表は [`crate::input::GenerationRule`] にある。
//! 契約の版は [`crate::CONTRACT_VERSION`]。

use std::fmt;

use fandhe_workshop_contract::limits::MAX_STRING_CHARS;
use serde::{Deserialize, Serialize};

/// 実行の世代。入力の判定が古い結果を捨てるための印である。
///
/// 初期値は [`Generation::INITIAL`]（1）。増えるのは停止・移動・復帰・開始の成功時だけで、減らない
/// （表は [`crate::input::GenerationRule`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Generation(pub u64);

impl Generation {
    /// 実行環境を作った直後の世代。
    pub const INITIAL: Generation = Generation(1);

    /// 次の世代。あふれる場合は `None` を返し、panic しない（呼び出し側は世代を変えない側に倒す）。
    pub fn next(self) -> Option<Generation> {
        self.0.checked_add(1).map(Generation)
    }
}

/// 実体の根。Step の実体 ID の最後の `/` より前の部分で、`main`、`<モジュール ID>@<実体名>`、またはそれを `/` でつないだネストの形である。
///
/// IR の `Instance.id` と一致する。状態の切り身（[`crate::session::Slice`]）と下書きの持ち主の鍵になる
/// （MOD-3・MOD-4）。ファイルパスや URL には使わない。
///
/// MOD-4・TASK-236 の文言は「最初の `/` まで」だが、入れ子の無い試作から書かれた定義であり、入れ子の実体でだけ
/// 最後の `/` との読みが分かれる。本実装は `Instance.id`・`Node.scope`（最後の `/` より前）に従う。フラグは実体ごと
/// に持つ（語彙 v1 の 9 節）ため、最初の `/` にすると入れ子の子の状態が親に混ざる。仕様の決定待ち（#1413）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Root(pub String);

/// Step の実体 ID（`<モジュール ID>@<実体名>/<Step>`。章は `main/<Step>`。ネストした実体は `<根>/<モジュール ID>@<実体名>/<Step>`）。IR の `Node::uid` と同じ値を持つ。
///
/// 公開の欄は直列化と IR の値をそのまま持つための口で、組み立て時には書式を検査しない。
/// 外から来た値は [`StepUid::parse`] で検査する。値は識別子として持つだけで、ファイル I/O や HTML には使わない。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepUid(pub String);

/// [`StepUid::parse`] が拒否した理由。文面は固定で、入力値を埋め込まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StepUidError {
    /// 空である。
    Empty,
    /// 長さが上限（`MAX_STRING_CHARS`）を超える。
    TooLong,
    /// 制御文字または空白を含む。
    InvalidChar,
    /// `/` が無い。
    SlashCount,
    /// 根または Step の部分が空である。
    EmptyPart,
    /// 根が `main` でも、`<モジュール ID>@<実体名>`（`/` でつないだネストを含む）でもない。
    BadRoot,
}

impl fmt::Display for StepUidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            StepUidError::Empty => "step uid is empty",
            StepUidError::TooLong => "step uid is too long",
            StepUidError::InvalidChar => "step uid contains a control character or whitespace",
            StepUidError::SlashCount => "step uid must contain a '/'",
            StepUidError::EmptyPart => "step uid has an empty root or step part",
            StepUidError::BadRoot => "step uid root must be 'main' or '<module>@<instance>'",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for StepUidError {}

impl StepUid {
    /// 実体 ID の構造を検査して作る。文字種は pack 側の規則に任せ、ここでは構造だけを確かめる。
    ///
    /// 受理するのは `main/<Step>` と `<モジュール ID>@<実体名>/<Step>`、およびネストした
    /// `<モジュール ID>@<実体名>/<モジュール ID>@<実体名>/<Step>`。根と Step は最後の `/` で分ける。`/` の有無、各段の `@` の個数、空の部分、
    /// 制御文字・空白、長さを見る。拒否は `Result` で返し、panic しない（安全側）。
    pub fn parse(s: &str) -> Result<StepUid, StepUidError> {
        if s.is_empty() {
            return Err(StepUidError::Empty);
        }
        if s.chars().count() > MAX_STRING_CHARS {
            return Err(StepUidError::TooLong);
        }
        if s.chars().any(|c| c.is_control() || c.is_whitespace()) {
            return Err(StepUidError::InvalidChar);
        }
        // 最後の `/` で分ける。ネストした実体（`a@x/b@y/st`）は `/` を 2 つ以上含み、根はその前の全体になる。
        let (root, step) = s.rsplit_once('/').ok_or(StepUidError::SlashCount)?;
        if root.is_empty() || step.is_empty() {
            return Err(StepUidError::EmptyPart);
        }
        if root != "main" {
            let ok = root.split('/').all(|seg| {
                seg.matches('@').count() == 1
                    && seg
                        .split_once('@')
                        .is_some_and(|(m, i)| !m.is_empty() && !i.is_empty())
            });
            if !ok {
                return Err(StepUidError::BadRoot);
            }
        }
        Ok(StepUid(s.to_string()))
    }

    /// 最後の `/` より前を根（`Instance.id`）として返す（`/` が無ければ全体）。ネストした実体でも根は親ではなく自身になる。
    /// MOD-4 の文言（最初の `/` まで）との食い違いは [`Root`] の文書を参照する（#1413）。
    pub fn root(&self) -> Root {
        let root = self.0.rsplit_once('/').map_or(self.0.as_str(), |(r, _)| r);
        Root(root.to_string())
    }
}

/// すべてのイベントが持つ「その時点の世代と Step」。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stamp {
    /// その時点の世代。
    pub generation: Generation,
    /// その時点の Step の実体 ID。
    pub step: StepUid,
}

/// 進行（`Tick`）の宛先。世代・Step の実体 ID・何番目の進行かの 3 要素で届き先を一意にする（EVT-8）。
///
/// `index` は、同じ番号の進行が二重に届いても 1 回だけ効かせるために要る（F-5）。
///
/// 照合の規則: 3 要素がすべて現在の状態と一致したときだけ進む。食い違いはコマンドの分類より先に判定し、
/// 古い宛先は状態を変えない（EVT-8）。照合の実装は TASK-210 が持つ。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TickAddr {
    /// 世代。
    pub generation: Generation,
    /// Step の実体 ID。
    pub step: StepUid,
    /// その Step の何番目の進行か（0 始まり）。
    pub index: u32,
}
