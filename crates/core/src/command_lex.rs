//! コマンド入力の字句化（JUDGE-1、TASK-262 1/2）。
//!
//! 受講者の 1 行と操作プロファイルの接頭辞から、契約の [`LexedCommand`] を作る純関数を置く。
//! 呼び出し元は、宣言との照合（TASK-262 2/2）・教材テスト（TASK-404、pack）・表示（web）である。
//! 結果の型は `fandhe_workshop_contract::judgment`、接頭辞による入力の分けは
//! `fandhe_workshop_contract::profile::classify_input` が正で、ここでは再定義も再実装もしない。
//!
//! # 規則
//!
//! 1. `classify_input` が自然文と分けた入力は [`CommandLexError::NotCommand`]
//! 2. 空白（Unicode の White_Space）で字句に分ける。最初の字句から接頭辞を除いたものが名前で、
//!    接頭辞の直後に名前が続く必要がある
//! 3. `--` で始まる字句は名前つき引数で、`--` を除いた書かれたとおりの名前を `key` にする（`=` では分けない）。
//!    次の字句が `--` で始まらなければ値として消費し、無ければ値なし（`None`）にする
//! 4. ほかの字句は位置引数。順序と重複はそのまま残す
//! 5. 結果は必ず `validate()` に通し、通らなければ [`CommandLexError::Malformed`]
//!
//! 同じ入力からは常に同じ結果になる（LLM・乱数・時刻に依存しない）。
//!
//! # 扱わないこと
//!
//! - 宣言（`CommandSpec`）との照合・個数と形の判定・拒否の分類への写像（`command_judge`、TASK-262 2/2）
//! - 名前の `-`／`_` の同一視、大文字小文字・Unicode の正規化
//! - 引用符で空白を含む引数（契約版 1 の対象外）
//! - 入力の長さの上限（受け口の責務。処理は長さに対して線形）

use std::fmt;

use fandhe_workshop_contract::judgment::{LexError, LexedArg, LexedCommand};
use fandhe_workshop_contract::profile::{classify_input, InputClass};

/// 字句化の失敗。入力値は埋め込まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandLexError {
    /// 接頭辞で始まらない（自然文として扱うべき入力）。
    NotCommand,
    /// 字句が契約の不変条件に反する（名前が空、`--` だけ、`---x` など）。
    Malformed(LexError),
}

impl fmt::Display for CommandLexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCommand => f.write_str("input is not a command"),
            Self::Malformed(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for CommandLexError {}

/// `raw` を `prefix` つきのコマンドとして名前と引数に分ける。
///
/// `prefix` は `check_command_prefix` を通した値を前提とするが、全域でパニックしない。
pub fn lex_command(prefix: &str, raw: &str) -> Result<LexedCommand, CommandLexError> {
    if classify_input(prefix, raw) == InputClass::Text {
        return Err(CommandLexError::NotCommand);
    }
    let mut tokens = raw.split_whitespace();
    let name = match tokens.next() {
        Some(first) => first
            .strip_prefix(prefix)
            .ok_or(CommandLexError::NotCommand)?,
        None => "",
    };

    let mut args = Vec::new();
    while let Some(token) = tokens.next() {
        if let Some(key) = token.strip_prefix("--") {
            // 値の終わりを決めるのは `--` だけ。`-5` や `-x` は値として読む。
            let mut ahead = tokens.clone();
            let value = match ahead.next() {
                Some(next) if !next.starts_with("--") => {
                    tokens = ahead;
                    Some(next.to_string())
                }
                _ => None,
            };
            args.push(LexedArg::Named {
                key: key.to_string(),
                value,
            });
        } else {
            args.push(LexedArg::Positional(token.to_string()));
        }
    }

    let lexed = LexedCommand {
        name: name.to_string(),
        args,
    };
    lexed.validate().map_err(CommandLexError::Malformed)?;
    Ok(lexed)
}
