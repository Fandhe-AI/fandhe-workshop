//! 入力の判定結果・拒否の分類・次の行動・コマンドの字句化の型の契約（TASK-256、版 1）。
//!
//! 入力の判定（JUDGE-1・JUDGE-3・JUDGE-4）の内側の型を 1 か所で固定する。呼び出し元は、入力・表示（web）、
//! 教材形式・検証（pack、TASK-44・TASK-45）、共通エンジン（core、TASK-262・TASK-267）である。
//! 各担当が別々に定義すると統合のときに食い違うため、先に契約として置く。
//!
//! 1 回の判定の結果は、[`JudgmentKind`] の 6 種のどれか 1 つに定まる（JUDGE-3）。問い返しは拒否の分類ではなく、
//! 結果の種類の 1 つである。拒否の分類は [`RejectCategory`] の 7 語で、8 語目の `free_text_unjudged` は
//! 受付に付く分類であり拒否ではない（JUDGE-2・JUDGE-4・PACK-11）。IR や語彙の文書が使う 8 語の集合は
//! [`Category`] が持つ。`result` の 3 語（`accepted`・`rejected`・`clarified`）と `rejection` の 8 語は
//! `docs/vocabulary/vocabulary.md` 7 節が正で、ここでは重複して定義せず、適合試験で一致を確かめる。
//! 残る 3 語（`confirmed`・`progressed`・`ignored`）は語彙ではなくエンジン側の名前である。
//!
//! 拒否・問い返し・受付の注記には、空でない次の行動 [`NextAction`] が必ず付く（JUDGE-4）。空の値は
//! [`NextAction::new`] でも逆直列化でも作れない。`next_action` は教材の差し込み（EP-7）を済ませた後の値で、
//! 文面の解決は core の `judge`（TASK-267）が行い、実行時の差し込みは core の `text_fill`（TASK-274）が担う。
//!
//! 直列化は外部タグ（`snake_case`）で、core の `Event` と形をそろえる。未知の欄・未知の分類・
//! 未知の種類は逆直列化の失敗にする（失敗を拒否に倒す）。欄の順は `tests/data/judgment_1/` の golden で固定する。
//!
//! 文字列の欄（次の行動・文面の ID・字句）は、教材と受講者に由来する未信頼の値である。この型は保持するだけで、
//! HTML へ埋め込むときのエスケープは web の責務とする。長さの上限は [`crate::limits`] と各読み込み口が判定し、
//! ここでは検査しない。エラーの `Display` は入力値を埋め込まない固定文にする。
//!
//! 境界（本契約が定義しないもの）:
//! - `Event::Refused.rejection` の型への置き換え（次の core の契約の版上げ）。core の `Event::Judged` は契約の版 3（TASK-267）で追加済み
//! - `Ignored` の理由（`IgnoreReason`）は core の `Event::Ignored` が正で、contract は core に依存できないため持たない
//! - 字句化の処理・接頭辞（EP-10・PACK-20）の除去・個数と形の判定は TASK-262 と TASK-44
//! - 文面の本文・既定の文面の登録の検査は pack。実行時の差し込みは core の `text_fill`（TASK-274）。キー判定表は TASK-257
//! - 引用符で空白を含む引数は版 1 では扱わない（必要になれば版上げ）
//!
//! 変更する場合は、先に spec（JUDGE-3・JUDGE-4・PACK-11）を直し、[`JUDGMENT_CONTRACT_VERSION`] を上げる。
//! 分類は増やさない（D-21）ため `#[non_exhaustive]` は付けず、利用側が網羅 match できるようにする。

use core::fmt;
use serde::{Deserialize, Serialize};

/// この契約の版。欄・分類・直列化の形を変えたら上げる。
pub const JUDGMENT_CONTRACT_VERSION: u32 = 1;

/// 判定結果の種類（JUDGE-3）。値を持たない 6 値で、1 回の判定はちょうど 1 つに定まる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JudgmentKind {
    /// 受付。
    Accepted,
    /// 拒否。
    Rejected,
    /// 問い返し。
    Clarified,
    /// 確認の操作が受け付けられた。
    Confirmed,
    /// 進行が 1 つ進んだ。
    Progressed,
    /// 無視。
    Ignored,
}

impl JudgmentKind {
    /// 全 6 種。
    pub const ALL: [JudgmentKind; 6] = [
        Self::Accepted,
        Self::Rejected,
        Self::Clarified,
        Self::Confirmed,
        Self::Progressed,
        Self::Ignored,
    ];

    /// 直列化と同じ正準の文字列。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Clarified => "clarified",
            Self::Confirmed => "confirmed",
            Self::Progressed => "progressed",
            Self::Ignored => "ignored",
        }
    }
}

/// IR の `rejections[].category` と `defaults` のキーに使う 8 語（PACK-11）。
///
/// 拒否の分類は 7 語で、`free_text_unjudged` は受付に付く（[`Self::is_rejection`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// 未知のコマンド。
    UnknownCommand,
    /// 順序違い。
    WrongOrder,
    /// 引数不足。
    MissingArg,
    /// 引数の形が違う。
    BadArgShape,
    /// 処理中。
    Busy,
    /// この Step では使わない。
    NotInThisStep,
    /// 教材未対応。
    NotSupported,
    /// 自然文の内容は判定しない（受付に付き、拒否ではない）。
    FreeTextUnjudged,
}

impl Category {
    /// 全 8 語。
    pub const ALL: [Category; 8] = [
        Self::UnknownCommand,
        Self::WrongOrder,
        Self::MissingArg,
        Self::BadArgShape,
        Self::Busy,
        Self::NotInThisStep,
        Self::NotSupported,
        Self::FreeTextUnjudged,
    ];

    /// 直列化と同じ正準の文字列。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnknownCommand => "unknown_command",
            Self::WrongOrder => "wrong_order",
            Self::MissingArg => "missing_arg",
            Self::BadArgShape => "bad_arg_shape",
            Self::Busy => "busy",
            Self::NotInThisStep => "not_in_this_step",
            Self::NotSupported => "not_supported",
            Self::FreeTextUnjudged => "free_text_unjudged",
        }
    }

    /// 正準の文字列だけを受ける。大文字・空白・未知の語は `None`。
    pub fn from_str_opt(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == s)
    }

    /// 拒否の分類か。`free_text_unjudged` だけが `false`。
    pub fn is_rejection(self) -> bool {
        !matches!(self, Self::FreeTextUnjudged)
    }
}

/// 拒否の 7 分類。[`Rejection`] はこの型だけを持つため、`free_text_unjudged` の拒否は型の上で作れない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectCategory {
    /// 未知のコマンド。
    UnknownCommand,
    /// 順序違い。
    WrongOrder,
    /// 引数不足。
    MissingArg,
    /// 引数の形が違う。
    BadArgShape,
    /// 処理中。
    Busy,
    /// この Step では使わない。
    NotInThisStep,
    /// 教材未対応。
    NotSupported,
}

impl RejectCategory {
    /// 全 7 語。
    pub const ALL: [RejectCategory; 7] = [
        Self::UnknownCommand,
        Self::WrongOrder,
        Self::MissingArg,
        Self::BadArgShape,
        Self::Busy,
        Self::NotInThisStep,
        Self::NotSupported,
    ];

    /// 直列化と同じ正準の文字列。
    pub fn as_str(self) -> &'static str {
        Category::from(self).as_str()
    }
}

impl From<RejectCategory> for Category {
    fn from(c: RejectCategory) -> Self {
        match c {
            RejectCategory::UnknownCommand => Self::UnknownCommand,
            RejectCategory::WrongOrder => Self::WrongOrder,
            RejectCategory::MissingArg => Self::MissingArg,
            RejectCategory::BadArgShape => Self::BadArgShape,
            RejectCategory::Busy => Self::Busy,
            RejectCategory::NotInThisStep => Self::NotInThisStep,
            RejectCategory::NotSupported => Self::NotSupported,
        }
    }
}

/// `free_text_unjudged` を拒否の分類へ変換しようとしたことを表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotARejection;

impl fmt::Display for NotARejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("category is not a rejection")
    }
}

impl std::error::Error for NotARejection {}

impl TryFrom<Category> for RejectCategory {
    type Error = NotARejection;

    fn try_from(c: Category) -> Result<Self, Self::Error> {
        Ok(match c {
            Category::UnknownCommand => Self::UnknownCommand,
            Category::WrongOrder => Self::WrongOrder,
            Category::MissingArg => Self::MissingArg,
            Category::BadArgShape => Self::BadArgShape,
            Category::Busy => Self::Busy,
            Category::NotInThisStep => Self::NotInThisStep,
            Category::NotSupported => Self::NotSupported,
            Category::FreeTextUnjudged => return Err(NotARejection),
        })
    }
}

/// 次の行動が空（空白だけを含む）であることを表す。入力値は埋め込まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyNextAction;

impl fmt::Display for EmptyNextAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("next_action must not be empty")
    }
}

impl std::error::Error for EmptyNextAction {}

/// 空でない次の行動（JUDGE-4）。中身の欄は閉じており、生成の口は [`Self::new`] と逆直列化だけである。
///
/// 空の判定は `trim` の後（全角空白・改行・タブを含む Unicode の空白）で行い、pack の
/// `E-EMPTY-NEXT-ACTION` と同じ基準にする。値は書き換えず、そのまま保持する。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NextAction(String);

impl NextAction {
    /// 空・空白だけの値は [`EmptyNextAction`]。
    pub fn new(s: impl Into<String>) -> Result<Self, EmptyNextAction> {
        let s = s.into();
        if s.trim().is_empty() {
            return Err(EmptyNextAction);
        }
        Ok(Self(s))
    }

    /// 保持している文字列。
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 中身の文字列を取り出す。
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl TryFrom<String> for NextAction {
    type Error = EmptyNextAction;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl From<NextAction> for String {
    fn from(n: NextAction) -> Self {
        n.0
    }
}

/// 拒否。分類と、IR の `rejections` の文面の ID と、差し込み後の次の行動を持つ。文面の本文は教材が持つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rejection {
    /// 拒否の分類（`free_text_unjudged` は持てない）。
    pub category: RejectCategory,
    /// IR の `rejections` の ID。存在の検査は pack の参照の検査（`E-REF-REJECTION`）が担う。
    pub text: String,
    /// 次の行動（差し込み後。空でない）。
    pub next_action: NextAction,
}

/// 問い返し。拒否ではなく、結果の種類の 1 つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clarification {
    /// IR の `clarifications` の ID。
    pub text: String,
    /// 次の行動（差し込み後。空でない）。
    pub next_action: NextAction,
}

/// 受付の注記。分類は `free_text_unjudged` に固定される（[`Self::CATEGORY`]）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptNote {
    /// 文面の ID（IR の `rejections` の `free_text_unjudged` の文面）。
    pub text: String,
    /// 次の行動（差し込み後。空でない）。
    pub next_action: NextAction,
}

impl AcceptNote {
    /// 注記に付く分類。
    pub const CATEGORY: Category = Category::FreeTextUnjudged;
}

/// 判定 1 件の結果。[`Self::kind`] で 6 種のどれか 1 つに写る。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Judgment {
    /// 受付。自然文の受付には注記を付ける。
    Accepted(Option<AcceptNote>),
    /// 拒否。
    Rejected(Rejection),
    /// 問い返し。
    Clarified(Clarification),
    /// 確認の操作が受け付けられた。
    Confirmed,
    /// 進行が 1 つ進んだ。詳細は core の `Event::Progress` が持つ。
    Progressed,
    /// 無視。理由は core の `Event::Ignored` が持つ。
    Ignored,
}

impl Judgment {
    /// 結果の種類。全域で写す。
    pub fn kind(&self) -> JudgmentKind {
        match self {
            Self::Accepted(_) => JudgmentKind::Accepted,
            Self::Rejected(_) => JudgmentKind::Rejected,
            Self::Clarified(_) => JudgmentKind::Clarified,
            Self::Confirmed => JudgmentKind::Confirmed,
            Self::Progressed => JudgmentKind::Progressed,
            Self::Ignored => JudgmentKind::Ignored,
        }
    }
}

/// 字句化の結果の引数 1 つ。順序と重複を失わない形で持つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum LexedArg {
    /// 位置引数。
    Positional(String),
    /// 名前付き引数（`--key value` または値なしの `--flag`）。
    Named {
        /// `--` を除いた、書かれたとおりの名前。
        key: String,
        /// 値。`None` は値を伴わない指定で、空の文字列とは区別する。
        value: Option<String>,
    },
}

/// コマンドを名前と引数に分けた字句化の結果（JUDGE-1）。処理は TASK-262 が持ち、不変条件は [`Self::validate`] で確かめる。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LexedCommand {
    /// コマンド名（接頭辞を除いた後。空白を含まない）。
    pub name: String,
    /// 書かれた順の引数。重複も残す。
    pub args: Vec<LexedArg>,
}

/// 字句化の結果が不変条件に反する理由。入力値は埋め込まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexError {
    /// `name` が空。
    EmptyName,
    /// `name` に空白がある。
    NameHasWhitespace,
    /// 位置引数が空。
    EmptyPositional,
    /// 位置引数に空白がある。
    PositionalHasWhitespace,
    /// `key` が空。
    EmptyKey,
    /// `key` に空白がある。
    KeyHasWhitespace,
    /// `key` が `-` で始まる。
    KeyStartsWithDash,
    /// `value` が `Some` で空。
    EmptyValue,
    /// `value` に空白がある。
    ValueHasWhitespace,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyName => "lexed command name is empty",
            Self::NameHasWhitespace => "lexed command name contains whitespace",
            Self::EmptyPositional => "lexed positional argument is empty",
            Self::PositionalHasWhitespace => "lexed positional argument contains whitespace",
            Self::EmptyKey => "lexed argument key is empty",
            Self::KeyHasWhitespace => "lexed argument key contains whitespace",
            Self::KeyStartsWithDash => "lexed argument key starts with a dash",
            Self::EmptyValue => "lexed argument value is empty",
            Self::ValueHasWhitespace => "lexed argument value contains whitespace",
        })
    }
}

impl std::error::Error for LexError {}

fn token_error(s: &str, empty: LexError, ws: LexError) -> Result<(), LexError> {
    if s.is_empty() {
        Err(empty)
    } else if s.chars().any(char::is_whitespace) {
        Err(ws)
    } else {
        Ok(())
    }
}

impl LexedCommand {
    /// 不変条件を検査する。最初に見つかった違反を返す。個数・形・重複の判定は含まない（TASK-262）。
    pub fn validate(&self) -> Result<(), LexError> {
        token_error(&self.name, LexError::EmptyName, LexError::NameHasWhitespace)?;
        for arg in &self.args {
            match arg {
                LexedArg::Positional(s) => {
                    token_error(
                        s,
                        LexError::EmptyPositional,
                        LexError::PositionalHasWhitespace,
                    )?;
                }
                LexedArg::Named { key, value } => {
                    token_error(key, LexError::EmptyKey, LexError::KeyHasWhitespace)?;
                    if key.starts_with('-') {
                        return Err(LexError::KeyStartsWithDash);
                    }
                    if let Some(v) = value {
                        token_error(v, LexError::EmptyValue, LexError::ValueHasWhitespace)?;
                    }
                }
            }
        }
        Ok(())
    }
}
