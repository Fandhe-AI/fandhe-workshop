//! 操作プロファイル・外観・左エリアの設定が、表示部品の見え方へ渡る口の契約（TASK-343 2/3）。
//!
//! 出所は語彙の文書の EP-10（`pack.profile`）・EP-11（`pack.appearance`）・EP-12（`pack.left_pane`）と、
//! spec の PACK-20・PACK-21・PACK-22・PACK-48 である。呼び出し元は、設定を読む表示部品（TASK-434 など）と、
//! ホストの Component（TASK-347）である。native の rlib で試験できるよう、wasm 専用にしない。
//!
//! 本モジュールは、検査済みの設定を表示層へ渡す型だけを持つ。IR の値からの変換と診断は pack 側
//! （TASK-52・TASK-53）の責務で、本モジュールの型に合わせて値を作る。
//!
//! 外観の閉じた集合と読み取りの規則の正は `contract::appearance` に、左エリアの設定の正は `contract::left_pane` に
//! ある（`Theme`・`LeftPane`・`WorldList`・`ViewTabs` はその再公開）。
//!
//! 外観は見た目だけを変え、入力判定・状態変化・遷移・履歴に影響しない（PACK-21）。外観は `main` の
//! `data-theme` 属性だけで表し、`style` 属性・`<style>` 要素は使わない。
//!
//! 契約の版は `parts::PARTS_CONTRACT_VERSION`（版 3）である。
//!
//! 既定値: `command_prefix` は `"/"`、`theme` は `standard`、`view_tabs` は `all`（PACK-48）、`world_list` は
//! `shown`（語彙 v1.3 の 5 節）。`input_field` の既定は spec の本文に明記が無く、prototype の試験の観察から
//! `shown` と置く（未確認の仮定）。

use core::fmt;

pub use fandhe_workshop_contract::appearance::Theme;
pub use fandhe_workshop_contract::left_pane::{LeftPane, ViewTabs, WorldList};
pub use fandhe_workshop_contract::profile::InputClass;
use fandhe_workshop_contract::profile::{check_command_prefix, classify_input};

use crate::parts::PartKind;

/// 接頭辞の最大の文字数。
pub const MAX_PREFIX_CHARS: usize = fandhe_workshop_contract::profile::MAX_COMMAND_PREFIX_CHARS;

/// 設定の拒否の理由。文面は固定で、入力値を埋め込まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileError {
    /// コマンド接頭辞が規則に合わない（長さ・空白・制御文字）。
    InvalidPrefix,
}

impl fmt::Display for ProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfileError::InvalidPrefix => f.write_str("invalid command prefix"),
        }
    }
}

impl std::error::Error for ProfileError {}

/// コマンド接頭辞。`""`（接頭辞なし）か 1〜4 文字で、制御文字・空白・ASCII の英数字を含まず、`-` で始まらない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPrefix(String);

impl CommandPrefix {
    /// 検査して作る。
    pub fn new(s: &str) -> Result<Self, ProfileError> {
        // 規則の正は contract にあり、教材の読み込み側（pack）と同じ検査を通す（TASK-52）。
        check_command_prefix(s).map_err(|_| ProfileError::InvalidPrefix)?;
        Ok(Self(s.to_owned()))
    }

    /// 入力をコマンドと自然文に分ける。画面の送信経路（TASK-275）は、教材テストと同じこの分けを使う。
    pub fn classify(&self, raw: &str) -> InputClass {
        classify_input(&self.0, raw)
    }

    /// 接頭辞の文字列。画面に出す場合は文字のノードだけにする（属性・HTML に埋め込まない）。
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 接頭辞なしか。
    pub fn is_none(&self) -> bool {
        self.0.is_empty()
    }
}

impl Default for CommandPrefix {
    fn default() -> Self {
        Self("/".to_owned())
    }
}

/// 入力欄の表示。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputField {
    /// 出す（既定）。
    #[default]
    Shown,
    /// 出さない。
    Hidden,
}

impl InputField {
    /// 語彙の綴り。
    pub const fn as_str(self) -> &'static str {
        match self {
            InputField::Shown => "shown",
            InputField::Hidden => "hidden",
        }
    }

    /// 入力欄・送信ボタン・補完の候補・キーの案内・OS の選択を出すか。`Hidden` でも、選択肢・画面操作・
    /// 承認・確認の操作と停止は出る。呼び出し元は `hello::HelloView`（TASK-450 2/3）。
    pub const fn shows_input_area(self) -> bool {
        matches!(self, InputField::Shown)
    }
}

/// 操作プロファイル（EP-10）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OperationProfile {
    /// コマンド接頭辞。
    pub command_prefix: CommandPrefix,
    /// 入力欄の表示。
    pub input_field: InputField,
}

/// 表示層へ渡す設定の全体。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DisplaySettings {
    /// 操作プロファイル。
    pub profile: OperationProfile,
    /// 外観。
    pub theme: Theme,
    /// 左エリア。
    pub left_pane: LeftPane,
}

impl DisplaySettings {
    /// `main` に付ける属性。値は閉じた集合の `&'static str` だけで、任意の文字列が属性に入らない。
    pub fn root_attrs(&self) -> [(&'static str, &'static str); 1] {
        [("data-theme", self.theme.as_str())]
    }

    /// 設定に従って、この種類の部品を出すか。入力欄なしのときは候補の一覧を出さない。
    /// 他の種類（選択肢・チェックポイント等）は `input_field: hidden` でも出る。
    pub fn shows_part(&self, kind: PartKind) -> bool {
        match kind {
            PartKind::Candidates => self.profile.input_field.shows_input_area(),
            _ => true,
        }
    }
}

/// 表示部品への設定の渡し方の口。ホストの表示用の値 `M` が実装し、設定を読む部品は
/// `impl<M: SettingsSource> DisplayPart<M> for ...` のように境界で求める。
///
/// この形にしたのは、`DisplayPart::view(&self, model: &M)` の形を変えずに済み、既存の試験と後続の部品の
/// 実装を壊さないためである。設定は教材ごとに固定で、`M` に持たせれば `view` の引数を増やさずに読める。
/// 設定は `update` で変わらない前提のため dirty field にしない（変わる設計にする場合は、先に spec を直して
/// `parts::PARTS_CONTRACT_VERSION` を上げる）。
pub trait SettingsSource {
    /// 現在の設定。
    fn display_settings(&self) -> &DisplaySettings;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 接頭辞の境界() {
        assert!(CommandPrefix::new("").unwrap().is_none());
        assert!(CommandPrefix::new("/").is_ok());
        assert!(CommandPrefix::new("abcd").is_err());
        assert!(CommandPrefix::new("-x").is_err());
        assert!(CommandPrefix::new("::").is_ok());
        assert!(CommandPrefix::new("＄").is_ok());
        assert!(CommandPrefix::new("!!!!").is_ok());
        assert!(CommandPrefix::new("!!!!!").is_err());
        assert!(CommandPrefix::new("あいうえ").is_ok());
        assert!(CommandPrefix::new("abcde").is_err());
        assert!(CommandPrefix::new("a b").is_err());
        assert!(CommandPrefix::new("\n").is_err());
    }
}
