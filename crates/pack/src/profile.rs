//! `pack.profile`（操作プロファイル）の検査と診断への変換（TASK-52 1/2）。
//!
//! `compile` が `ir_format` の検査の直後に呼ぶ。接頭辞の規則と入力の分けの実体は
//! `fandhe_workshop_contract::profile` にあり、画面（web）と教材テストが同じ関数で分けられるようにしてある。
//! 本モジュールはその結果を診断（`E-PARSE`・`E-PROFILE-PREFIX`・`E-PROFILE-COMMAND-NAME`）へ写す。
//!
//! 診断の文面には、接頭辞の値・コマンド名・未知のキーの生の値を写さない。item に名前を写すのは
//! `safe_ident` を通るときだけにする（`: ` や改行で 1 行の形を壊さないため）。名前が通らないコマンドは、
//! 名前順で 1 始まりの位置を使った `command:#<n>` で示す（PoC の出力 `command:ls a` とは形が違う。
//! 出力形の正は TASK-399 が固定する）。
//!
//! `input_field` の意味の検査（`E-PROFILE-TEXT` など）は TASK-52 2/2 が受け持つ。

use fandhe_workshop_contract::profile::{
    classify_input, is_typable_without_prefix, read_profile, InputClass, PrefixError,
    ProfileReadError, ProfileSetting,
};

use crate::diag::{DiagCode, Diagnostic};
use crate::source::{safe_ident, WorkshopSrc};

fn mk(file: &str, item: &str, code: DiagCode, message: String) -> Option<Diagnostic> {
    let file = if file.trim().is_empty() { "." } else { file };
    Diagnostic::new(file, item, code, message).ok()
}

/// `pack.profile` を読んで検査する。読めないときは既定値と診断を返す。
pub fn check_profile(workshop_file: &str, w: &WorkshopSrc) -> (ProfileSetting, Vec<Diagnostic>) {
    let mut diags = Vec::new();
    let setting = match read_profile(w.pack.profile.as_ref()) {
        Ok(s) => s,
        Err(e) => {
            let (code, item, message) = match e {
                ProfileReadError::NotAMapping => (
                    DiagCode::Parse,
                    "pack:profile",
                    "pack.profile must be a mapping".to_string(),
                ),
                ProfileReadError::UnknownKey(k) => (
                    DiagCode::Parse,
                    "pack:profile",
                    if safe_ident(&k) {
                        format!("unknown key `{k}` in pack.profile")
                    } else {
                        "unknown key in pack.profile".to_string()
                    },
                ),
                ProfileReadError::PrefixNotString => (
                    DiagCode::Parse,
                    "pack:profile",
                    "pack.profile.command_prefix must be a string".to_string(),
                ),
                ProfileReadError::InputFieldInvalid => (
                    DiagCode::Parse,
                    "pack:profile",
                    "pack.profile.input_field must be `shown` or `hidden`".to_string(),
                ),
                ProfileReadError::Prefix(pe) => (
                    DiagCode::ProfilePrefix,
                    "pack:profile.command_prefix",
                    prefix_message(pe),
                ),
            };
            diags.extend(mk(workshop_file, item, code, message));
            return (ProfileSetting::default(), diags);
        }
    };

    if setting.command_prefix.is_empty() {
        // BTreeMap の反復は名前順なので、位置は決定的である。
        for (i, name) in w.commands.keys().enumerate() {
            if is_typable_without_prefix(name) {
                continue;
            }
            let item = if safe_ident(name) {
                format!("command:{name}")
            } else {
                format!("command:#{}", i + 1)
            };
            diags.extend(mk(
                workshop_file,
                &item,
                DiagCode::ProfileCommandName,
                "command name cannot be typed as the first word when the command prefix is empty"
                    .to_string(),
            ));
        }
    }
    (setting, diags)
}

fn prefix_message(e: PrefixError) -> String {
    e.to_string()
}

/// 教材テストが入力を分ける入口。画面と同じ `contract::profile::classify_input` を呼ぶだけにする。
/// 実行器への結線は TASK-404 と TASK-52 2/2 が行う。
pub fn classify_play_input(profile: &ProfileSetting, raw: &str) -> InputClass {
    classify_input(&profile.command_prefix, raw)
}
