//! `pack.profile`（操作プロファイル）の検査と診断への変換（TASK-52 1/2）。
//!
//! `compile` が `ir_format` の検査の直後に呼ぶ。接頭辞の規則と入力の分けの実体は
//! `fandhe_workshop_contract::profile` にあり、画面（web）と教材テストが同じ関数で分けられるようにしてある。
//! 本モジュールはその結果を診断（`E-PARSE`・`E-PROFILE-PREFIX`・`E-PROFILE-COMMAND-NAME`）へ写す。
//!
//! 診断の文面には、接頭辞の値・コマンド名・未知のキーの生の値を写さない。item に名前を写すのは
//! `safe_ident` を通るときだけにする（`: ` や改行で 1 行の形を壊さないため）。名前が通らないコマンドは、
//! 名前順で 1 始まりの位置を使った `command:#<n>` で示す（PoC の出力 `command:ls a` とは形が違う。
//! 出力形の正は `docs/cli.md` 版 4 の 6〜7 節）。
//!
//! 接頭辞と `input_field` に基づく Step の検査（`E-PROFILE-TEXT`・`E-PROFILE-NO-INPUT`。TASK-52 2/2）は
//! [`check_profile_steps`] が `compile` から呼ばれて行う。教材テストの入力の分けの検査（`E-PLAY-INPUT-CLASS`）は
//! `test` の診断なので `compile` には結線せず、[`check_play_input_class`] を `play_run::run_loaded` が呼ぶ（TASK-406 2/2）。
//! どちらも、読めない `pack.profile` は既定値に倒れて新しい診断を出さない（`check_profile` が 1 回だけ報告する）。

use fandhe_workshop_contract::input_spec::InputKind;
use fandhe_workshop_contract::ir::Step;
use fandhe_workshop_contract::profile::{
    classify_input, is_typable_without_prefix, read_profile, InputClass, InputFieldMode,
    PrefixError, ProfileReadError, ProfileSetting,
};
use serde_json::Value;

use crate::diag::{DiagCode, Diagnostic};
use crate::input::has_placeholder;
use crate::source::{safe_ident, LoadedPack, LoadedScope, ScopeKind, TestsSrc, WorkshopSrc};

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

fn input_kind(input: Option<&Value>) -> Option<InputKind> {
    InputKind::from_str_opt(input?.as_object()?.get("kind")?.as_str()?)
}

/// 規則の `on.text` が真偽値の true か（`false` は自然文の規則ではない）。
fn is_text_rule(rule: &Value) -> bool {
    rule.get("on")
        .and_then(Value::as_object)
        .is_some_and(|on| on.get("text") == Some(&Value::Bool(true)))
}

/// 規則が `on.command` を持つか（値は問わない）。
fn is_command_rule(rule: &Value) -> bool {
    rule.get("on")
        .and_then(Value::as_object)
        .is_some_and(|on| on.contains_key("command"))
}

/// 接頭辞と `input_field` に反する Step の入力・規則を検査する（`compile` から呼ばれる）。
///
/// 1 Step につき各コードは最大 1 件。型で読めない `input` は `input::check_inputs` が `E-PARSE` にするので、
/// ここでは該当なしとして扱う。診断の文面には入力の値・接頭辞を写さない。
/// モジュールの `input` が引数の置換記号 `${` を含む Step は、置換前の `input.kind` が置換後と違いうる
/// （例: `kind: "${k}"` が `text` になる）ため、ここでは飛ばし、展開（`expand`）が置換後の Step を
/// [`check_profile_step`] で検査する。規則の `on.text`（真偽値）・`on.command`（キーの有無）は置換で変わらない。
pub fn check_profile_steps(setting: &ProfileSetting, scopes: &[LoadedScope]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for scope in scopes {
        for step in &scope.src.steps {
            if scope.kind == ScopeKind::Module && has_placeholder(&step.input) {
                continue;
            }
            out.extend(check_profile_step(setting, scope, step));
        }
    }
    out
}

/// 1 つの Step を接頭辞と `input_field` に照らして検査する。展開が、引数を置換した後の Step にも使う。
/// 診断は定義元の章・モジュールのファイルと Step に紐付け、文面は値を含まない固定文にする。
pub(crate) fn check_profile_step(
    setting: &ProfileSetting,
    scope: &LoadedScope,
    step: &Step,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let empty_prefix = setting.command_prefix.is_empty();
    let hidden = setting.input_field == InputFieldMode::Hidden;
    if !empty_prefix && !hidden {
        return out;
    }
    let kind = input_kind(step.input.as_ref());
    let text_rule = step.rules.iter().any(is_text_rule);
    let command_rule = step.rules.iter().any(is_command_rule);
    let item = if safe_ident(&step.id) {
        format!("step:{}", step.id)
    } else {
        "pack".to_string()
    };
    if empty_prefix && (kind == Some(InputKind::Text) || text_rule) {
        out.extend(mk(
            &scope.file,
            &item,
            DiagCode::ProfileText,
            "natural-language input or rule is not allowed when the command prefix is empty"
                .to_string(),
        ));
    }
    let typing = matches!(kind, Some(InputKind::Command | InputKind::Text));
    if hidden && (typing || command_rule || text_rule) {
        out.extend(mk(
            &scope.file,
            &item,
            DiagCode::ProfileNoInput,
            "command or natural-language input or rule is not allowed when input_field is hidden"
                .to_string(),
        ));
    }
    out
}

/// 読み込んだパックの教材テストを検査する。`pack.profile` が読めない・教材テストが無いときは空を返す。
pub fn check_play_inputs(pack: &LoadedPack) -> Vec<Diagnostic> {
    let (Ok(setting), Some(file), Some(tests)) = (
        read_profile(pack.workshop.pack.profile.as_ref()),
        pack.tests_file.as_deref(),
        pack.tests.as_ref(),
    ) else {
        return Vec::new();
    };
    check_play_input_class(&setting, file, tests)
}

/// 教材テストの `cmd:`・`text:` が、画面では別の種類の入力に分けられるものを `E-PLAY-INPUT-CLASS` にする。
///
/// 既定の接頭辞 `/` では何も検査しない（`cmd: hello` も通す）。分けは必ず [`classify_play_input`]
/// （画面と同じ関数）で行う。形が想定と違う要素は読み飛ばす（型の検査は TASK-404 の担当）。
/// `test` コマンドへの結線は `play_run::run_loaded` が行う（TASK-406 2/2）。
pub fn check_play_input_class(
    setting: &ProfileSetting,
    tests_file: &str,
    tests: &TestsSrc,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if setting.command_prefix == fandhe_workshop_contract::profile::DEFAULT_COMMAND_PREFIX {
        return out;
    }
    for (n, v) in tests.playthroughs.iter().enumerate() {
        let item = item_of("playthrough", n, v);
        let segments = v.get("segments").and_then(Value::as_array);
        for (si, seg) in segments.into_iter().flatten().enumerate() {
            let steps = seg.get("steps").and_then(Value::as_array);
            for (ii, input) in steps.into_iter().flatten().enumerate() {
                let place = format!("segment {}, input {}", si + 1, ii + 1);
                check_input_value(setting, tests_file, &item, &place, input, &mut out);
            }
        }
    }
    for (label, list) in [
        ("error_input", &tests.error_inputs),
        ("text_check", &tests.text_checks),
    ] {
        for (n, v) in list.iter().enumerate() {
            let item = item_of(label, n, v);
            let pre = v.get("pre").and_then(Value::as_array);
            for (pi, input) in pre.into_iter().flatten().enumerate() {
                let place = format!("pre {}", pi + 1);
                check_input_value(setting, tests_file, &item, &place, input, &mut out);
            }
            if let Some(input) = v.get("input") {
                check_input_value(setting, tests_file, &item, "input", input, &mut out);
            }
        }
    }
    out
}

/// 一覧の要素の item。id は `safe_ident` を通るときだけ写し、そうでなければ 1 始まりの位置にする。
fn item_of(label: &str, index: usize, v: &Value) -> String {
    match v.get("id").and_then(Value::as_str) {
        Some(id) if safe_ident(id) => format!("{label}:{id}"),
        _ => format!("{label}:#{}", index + 1),
    }
}

fn check_input_value(
    setting: &ProfileSetting,
    file: &str,
    item: &str,
    place: &str,
    input: &Value,
    out: &mut Vec<Diagnostic>,
) {
    if let Some(cmd) = input.get("cmd").and_then(Value::as_str) {
        if classify_play_input(setting, cmd) != InputClass::Command {
            out.extend(mk(
                file,
                item,
                DiagCode::PlayInputClass,
                format!("cmd: input at {place} does not start with the command prefix and is sent as natural language on the screen"),
            ));
        }
    }
    if let Some(text) = input.get("text").and_then(Value::as_str) {
        if classify_play_input(setting, text) == InputClass::Command {
            let message = if setting.command_prefix.is_empty() {
                format!("text: input at {place} cannot be natural language because the command prefix is empty")
            } else {
                format!("text: input at {place} starts with the command prefix and is sent as a command on the screen")
            };
            out.extend(mk(file, item, DiagCode::PlayInputClass, message));
        }
    }
}
