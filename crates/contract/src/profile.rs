//! 操作プロファイルの接頭辞の規則と、入力の分け（TASK-52 1/2）。
//!
//! 出所は語彙の文書の EP-10（`pack.profile`）、spec の PACK-20、PoC の語彙 v1.3 の 3.1〜3.3 節である。
//! 呼び出し元は、pack の `profile`（`pack.profile` の検査と診断）と、web の `profile::CommandPrefix`
//! （画面の送信経路）である。後続の TASK-262（字句化）・TASK-275（入力欄の結線）・TASK-404（教材テスト）も
//! 同じ関数を呼ぶ。
//!
//! contract に置く理由: 画面と教材テストは同じ関数で入力を分けなければならないが、web から pack への
//! normal 依存は禁じられている（`docs/architecture.md` 第 4 節）。2 つ以上の責務が参照する純粋な関数は
//! contract に置く（同第 6 節の判定規則 1。前例は `content_hash`）。
//!
//! I/O も状態も持たない純関数だけで構成し、wasm32 でも動く。

use serde_json::Value;

/// 接頭辞の既定値。
pub const DEFAULT_COMMAND_PREFIX: &str = "/";

/// 空でない接頭辞の最大の文字数（コードポイント）。
pub const MAX_COMMAND_PREFIX_CHARS: usize = 4;

/// 接頭辞が規則に合わない理由。文面は固定で、入力値を埋め込まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixError {
    /// 5 文字以上である。
    TooLong,
    /// 制御文字を含む。
    Control,
    /// 空白を含む。
    Whitespace,
    /// ASCII の英数字を含む（コマンド名と取り違えるため）。
    AsciiAlphanumeric,
    /// `-` で始まる（名前つき引数 `--name` と取り違えるため）。
    LeadingDash,
}

impl core::fmt::Display for PrefixError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            PrefixError::TooLong => "command prefix must be at most 4 characters",
            PrefixError::Control => "command prefix must not contain control characters",
            PrefixError::Whitespace => "command prefix must not contain whitespace",
            PrefixError::AsciiAlphanumeric => {
                "command prefix must not contain ASCII letters or digits"
            }
            PrefixError::LeadingDash => "command prefix must not start with `-`",
        })
    }
}

impl std::error::Error for PrefixError {}

/// 接頭辞を検査する。`""`（接頭辞なし）は通す。
///
/// 返す理由を決定的にするため、文字数・制御文字・空白・ASCII の英数字・先頭の `-` の順に調べる。
pub fn check_command_prefix(s: &str) -> Result<(), PrefixError> {
    if s.chars().count() > MAX_COMMAND_PREFIX_CHARS {
        return Err(PrefixError::TooLong);
    }
    if s.chars().any(char::is_control) {
        return Err(PrefixError::Control);
    }
    if s.chars().any(char::is_whitespace) {
        return Err(PrefixError::Whitespace);
    }
    if s.chars().any(|c| c.is_ascii_alphanumeric()) {
        return Err(PrefixError::AsciiAlphanumeric);
    }
    if s.starts_with('-') {
        return Err(PrefixError::LeadingDash);
    }
    Ok(())
}

/// 入力の分け。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputClass {
    /// コマンドとして扱う。
    Command,
    /// 自然文として扱う。
    Text,
}

/// 入力を、コマンドと自然文のどちらかに分ける。
///
/// 前後の空白（全角空白を含む Unicode の空白）を除いた入力が接頭辞で始まればコマンド、始まらなければ
/// 自然文である。接頭辞が `""` のときは、すべての入力がコマンドになる。`prefix` は
/// [`check_command_prefix`] を通した値を前提とするが、関数自体は全域でパニックしない。
/// 名前・引数への字句化はしない（TASK-262）。
pub fn classify_input(prefix: &str, raw: &str) -> InputClass {
    let trimmed = raw.trim();
    if prefix.is_empty() || trimmed.starts_with(prefix) {
        InputClass::Command
    } else {
        InputClass::Text
    }
}

/// 接頭辞なしのとき、コマンド名が入力の最初の語として打てるか。空でなく、空白も制御文字も含まないと真。
pub fn is_typable_without_prefix(name: &str) -> bool {
    !name.is_empty() && !name.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// `pack.profile.input_field` の値。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputFieldMode {
    /// 入力欄を出す（既定）。
    #[default]
    Shown,
    /// 入力欄を出さない。
    Hidden,
}

/// 読み取った `pack.profile`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileSetting {
    /// コマンド接頭辞。
    pub command_prefix: String,
    /// 入力欄の表示。
    pub input_field: InputFieldMode,
}

impl Default for ProfileSetting {
    fn default() -> Self {
        Self {
            command_prefix: DEFAULT_COMMAND_PREFIX.to_owned(),
            input_field: InputFieldMode::Shown,
        }
    }
}

/// `pack.profile` の読み取りの失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileReadError {
    /// 表（マッピング）でない。
    NotAMapping,
    /// 未知のキー。名前は未検査の値なので、診断へ写す側が `safe_ident` を通す。
    UnknownKey(String),
    /// `command_prefix` が文字列でない（`null` を含む）。
    PrefixNotString,
    /// `input_field` が `shown`・`hidden` 以外である。
    InputFieldInvalid,
    /// 接頭辞が規則に合わない。
    Prefix(PrefixError),
}

/// `pack.profile` の値を読む。`None` と空の表は既定値になる。
pub fn read_profile(v: Option<&Value>) -> Result<ProfileSetting, ProfileReadError> {
    let mut out = ProfileSetting::default();
    let Some(v) = v else {
        return Ok(out);
    };
    let map = v.as_object().ok_or(ProfileReadError::NotAMapping)?;
    for (k, val) in map {
        match k.as_str() {
            "command_prefix" => {
                let s = val.as_str().ok_or(ProfileReadError::PrefixNotString)?;
                check_command_prefix(s).map_err(ProfileReadError::Prefix)?;
                out.command_prefix = s.to_owned();
            }
            "input_field" => {
                out.input_field = match val.as_str() {
                    Some("shown") => InputFieldMode::Shown,
                    Some("hidden") => InputFieldMode::Hidden,
                    _ => return Err(ProfileReadError::InputFieldInvalid),
                };
            }
            other => return Err(ProfileReadError::UnknownKey(other.to_owned())),
        }
    }
    Ok(out)
}
