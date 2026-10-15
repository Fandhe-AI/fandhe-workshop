//! コマンド入力の宣言との照合（JUDGE-1、TASK-262 2/2）。
//!
//! 字句化の結果（[`LexedCommand`]、`command_lex::lex_command` が作る）を、Step の宣言
//! （[`CommandSpec`]）と照らし、名前の一致・必須引数の有無・引数の形を決定的に判定する純関数を置く。
//! 呼び出し元は、規則の照合と拒否の組み立てを行う [`crate::judge`]（TASK-267）・教材テスト（TASK-404、pack）である。
//! 宣言と形の型は contract の `input_spec` が正で、ここでは再定義しない。
//!
//! # 規則（順に評価し、最初に当たったものを返す）
//!
//! 1. 名前が宣言と完全一致しなければ [`CommandVerdict::NameMismatch`]（大文字小文字・正規化はしない）
//! 2. 宣言の並び順に各引数の値を解決する。位置引数は「位置引数の宣言だけを数えた順番」で入力の位置引数を引き、
//!    名前つき引数は宣言と入力の名前の `-` を `_` にそろえて引く（`--external-checks` が `external_checks` に当たる）。
//!    同じ名前が複数あるときは最後の 1 つを使う
//! 3. 値が無ければ、必須なら [`CommandVerdict::MissingArg`]、任意なら値なしで通す
//! 4. 名前つき引数が値なし（`--x` だけ）のときは、必須なら [`CommandVerdict::BadArgShape`]、任意なら値なしで通す
//! 5. 値があれば形で検査し、合わなければ [`CommandVerdict::BadArgShape`]
//! 6. 宣言より多い位置引数と、宣言に無い名前つき引数は無視する
//!
//! 形は閉じた 3 値（`kebab_case`・`digits`・`any`）だけで、正規表現は評価しない。教材が書いた式による
//! 計算量の攻撃の面を作らないためである。処理は入力の字句数と宣言の数にほぼ線形で、
//! `CommandSpec::check` を通していない宣言（空の名前・重複）を受けてもパニックしない。
//!
//! # 扱わないこと
//!
//! - `wrong_order`・`not_in_this_step`・`not_supported` の分類（IR の `commands` の値の解釈は TASK-268・TASK-269）
//! - 規則の `on.missing` との照合と問い返し（[`crate::command_rule`]、TASK-272）
//! - 規則の `on.args` との照合、拒否の文面の解決（[`crate::judge`]、TASK-267）
//! - `--flag` の直後の位置引数の読み替え（`ArgSpec` に値を取らない引数の欄が無いため、字句化の結果のまま使う）
//! - 正規表現・候補の一覧の形（契約版 1 の `ArgShape` に無い）

use fandhe_workshop_contract::input_spec::{ArgShape, CommandSpec};
use fandhe_workshop_contract::judgment::{LexedArg, LexedCommand, RejectCategory};

/// 解決した引数 1 つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedArg {
    /// 宣言の名前（書かれたまま）。
    pub name: String,
    /// 入力の値。任意の引数が無いときは `None`。
    pub value: Option<String>,
}

/// 照合に通ったコマンド。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedCommand {
    /// 宣言の順の引数。
    pub args: Vec<ResolvedArg>,
}

/// 判定の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandVerdict {
    /// 宣言に合う。
    Matched(MatchedCommand),
    /// 名前が宣言と違う。拒否の分類は IR の `commands` の解釈（TASK-268・269）が決める。
    NameMismatch,
    /// 必須の引数が無い。
    MissingArg,
    /// 引数の形が宣言と違う。
    BadArgShape,
}

impl CommandVerdict {
    /// 拒否の分類。受付と、この層で分類が定まらない `NameMismatch` は `None`。
    pub fn category(&self) -> Option<RejectCategory> {
        match self {
            Self::MissingArg => Some(RejectCategory::MissingArg),
            Self::BadArgShape => Some(RejectCategory::BadArgShape),
            Self::Matched(_) | Self::NameMismatch => None,
        }
    }
}

/// 名前が、教材のコマンドの表（IR の `commands` のキー）に無いか。値の種類は読まない。
pub fn is_unknown_command<'a>(known: impl IntoIterator<Item = &'a str>, name: &str) -> bool {
    !known.into_iter().any(|k| k == name)
}

fn normalize_key(key: &str) -> String {
    key.replace('-', "_")
}

fn shape_ok(shape: ArgShape, v: &str) -> bool {
    if v.is_empty() {
        return false;
    }
    match shape {
        ArgShape::Any => true,
        ArgShape::Digits => v.bytes().all(|b| b.is_ascii_digit()),
        ArgShape::KebabCase => {
            v.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                && !v.starts_with('-')
                && !v.ends_with('-')
                && !v.contains("--")
        }
    }
}

/// 字句化の結果を宣言と照合する。
pub fn judge_command(spec: &CommandSpec, lexed: &LexedCommand) -> CommandVerdict {
    if lexed.name != spec.name {
        return CommandVerdict::NameMismatch;
    }
    let mut positionals: Vec<&str> = Vec::new();
    // 名前つき引数。同名は後勝ち。値なしは `None`。
    let mut named: Vec<(String, Option<&str>)> = Vec::new();
    for a in &lexed.args {
        match a {
            LexedArg::Positional(v) => positionals.push(v),
            LexedArg::Named { key, value } => {
                named.push((normalize_key(key), value.as_deref()));
            }
        }
    }

    let mut resolved = Vec::with_capacity(spec.args.len());
    let mut next_positional = 0usize;
    for arg in &spec.args {
        // `Some(None)` は名前つきで値なし、`None` は引数そのものが無い。
        let found: Option<Option<&str>> = if arg.positional {
            let v = positionals.get(next_positional).copied();
            next_positional += 1;
            v.map(Some)
        } else {
            let key = normalize_key(&arg.name);
            named.iter().rev().find(|(k, _)| *k == key).map(|(_, v)| *v)
        };
        let value = match found {
            None => {
                if arg.required {
                    return CommandVerdict::MissingArg;
                }
                None
            }
            Some(None) => {
                if arg.required {
                    return CommandVerdict::BadArgShape;
                }
                None
            }
            Some(Some(v)) => {
                if !shape_ok(arg.shape, v) {
                    return CommandVerdict::BadArgShape;
                }
                Some(v.to_string())
            }
        };
        resolved.push(ResolvedArg {
            name: arg.name.clone(),
            value,
        });
    }
    CommandVerdict::Matched(MatchedCommand { args: resolved })
}
