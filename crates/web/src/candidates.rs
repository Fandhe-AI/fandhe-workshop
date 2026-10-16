//! 候補の一覧を開くかどうかの規則と、教材の IR からのコマンド表の作成（TASK-440 1/2。TERM-8・TERM-42・D-61）。
//!
//! 呼び出し元は、入力欄の部品（`hello::HelloView`）と、IR を持つホスト（`host::EngineHost`）である。
//! ホストが IR からコマンド表（[`CommandTable`]）を作って入力欄へ渡し、入力欄が下書きの変化のたびに
//! [`candidates`] を呼んで一覧の開閉を決める。DOM には触れず、native の試験で行ごとに固定できる。
//!
//! 開く条件（TERM-42）: 下書きがコマンド接頭辞で始まり、空白（改行・全角空白を含む）を含まず、
//! 接頭辞つきのコマンド名のうち下書きで始まって下書きと等しくないものが 1 件以上あること。
//! 下書きと完全に一致するだけの場合と、候補が無い場合は開かない（PoC-2 の K-05・K-08）。
//! `unsupported` のコマンドと、種類を読めない値は候補に出さない（`text_fill::nearest_command` と同じ扱い）。
//! 接頭辞を読めない場合と、入力欄を出さない設定（`input_field: hidden`）では、表を空にして一覧を開かない（fail-closed）。
//!
//! 閉じた後に下書きが変わったら再び開く規則は、spec に明記が無いため prototype の `closed_for` の挙動に合わせた
//! （`hello` が持つ）。
//!
//! 扱わないこと: Tab の補完の挿入（TASK-440 2/2 = #332）、候補ボタンのクリックで選ぶ操作、変換中の扱い。

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::profile::{read_profile, InputFieldMode};
use fandhe_workshop_core::command_class::CommandKind;

/// 候補の元になるコマンド表。名前は接頭辞なしで、名前順に並ぶ。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandTable {
    prefix: String,
    names: Vec<String>,
}

impl CommandTable {
    /// 接頭辞と名前から作る（試験用の口にもなる）。名前は整列して重複を除く。
    pub fn new(prefix: &str, names: impl IntoIterator<Item = String>) -> Self {
        let mut names: Vec<String> = names.into_iter().collect();
        names.sort();
        names.dedup();
        Self {
            prefix: prefix.to_owned(),
            names,
        }
    }

    /// 下書きに対する候補。
    pub fn candidates(&self, draft: &str) -> Vec<String> {
        candidates(draft, &self.names, &self.prefix)
    }

    /// 名前の件数。
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// 名前が無いか。
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

/// 下書きに対する候補を、名前順に返す。開かない場合は空。
pub fn candidates(draft: &str, names: &[String], prefix: &str) -> Vec<String> {
    if draft.is_empty() || !draft.starts_with(prefix) || draft.chars().any(char::is_whitespace) {
        return Vec::new();
    }
    let mut out: Vec<String> = names
        .iter()
        .map(|n| format!("{prefix}{n}"))
        .filter(|c| c.starts_with(draft) && c != draft)
        .collect();
    out.sort();
    out
}

/// IR からコマンド表を作る。読めない設定・入力欄を出さない設定では空の表にする。
pub fn command_table(ir: &Ir) -> CommandTable {
    let Ok(profile) = read_profile(ir.pack.profile.as_ref()) else {
        return CommandTable::default();
    };
    if profile.input_field == InputFieldMode::Hidden {
        return CommandTable::default();
    }
    let names = ir
        .commands
        .iter()
        .filter(|(_, kind)| {
            !matches!(
                CommandKind::from_value(kind),
                None | Some(CommandKind::Unsupported)
            )
        })
        .map(|(name, _)| name.clone());
    CommandTable::new(&profile.command_prefix, names)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> Vec<String> {
        ["help", "make-note", "publish-note"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect()
    }

    #[test]
    fn 接頭辞で始まる下書きは前方一致の候補を返す() {
        assert_eq!(candidates("/he", &names(), "/"), ["/help"]);
        assert_eq!(
            candidates("/", &names(), "/"),
            ["/help", "/make-note", "/publish-note"]
        );
    }

    #[test]
    fn 完全一致だけ_候補なし_空_空白入り_接頭辞違いは開かない() {
        for d in [
            "/help",
            "/zzz",
            "",
            "he",
            "/he lp",
            "/he\nx",
            "/he\u{3000}x",
            "/help ",
        ] {
            assert!(candidates(d, &names(), "/").is_empty(), "{d:?}");
        }
    }

    #[test]
    fn 完全一致でも他に前方一致があれば開く() {
        let n = vec!["help".to_owned(), "help-me".to_owned()];
        assert_eq!(candidates("/help", &n, "/"), ["/help-me"]);
    }

    #[test]
    fn 接頭辞なしと別の接頭辞() {
        assert_eq!(candidates("he", &names(), ""), ["help"]);
        assert!(candidates("", &names(), "").is_empty());
        assert_eq!(candidates(">m", &names(), ">"), [">make-note"]);
        assert!(candidates("/he", &names(), ">").is_empty());
    }

    #[test]
    fn 表は整列し重複を除く() {
        let t = CommandTable::new("/", ["b".to_owned(), "a".to_owned(), "a".to_owned()]);
        assert_eq!(t.candidates("/"), ["/a", "/b"]);
        assert!(CommandTable::default().candidates("/").is_empty());
    }
}
