//! Step の `input`（入力方式 5 種）の宣言を検査する（TASK-44、PACK-10）。
//!
//! `compile` から 1 回だけ呼ばれる。IR の `Step.input` は未型付けの値のまま出す（golden のバイト一致を
//! 保つため）ので、ここでは型で読めるかと意味の制約だけを確かめ、値は変えない。型は contract の
//! [`fandhe_workshop_contract::input_spec`] が持ち、実行時の `text` の判定（core）と共有する。
//!
//! 診断のコードは新設しない。型で読めない場合（未知の欄・未知の `kind`・欄の欠け・型の違い）は
//! `E-PARSE`、意味の制約の違反は `E-STRUCTURE` にする。メッセージには、短い識別子として安全な
//! 欄・`kind` の名前だけを写し、入力の値と serde の元の文言（値を含みうる）は写さない。
//! `wait`（EP-1）は受け付けて欄を検査しない（型は別タスク）。

use fandhe_workshop_contract::input_spec::{
    ApprovalSpec, ChoiceSpec, CommandSpec, InputKind, SpecError, TextSpec, UiActionSpec,
};
use fandhe_workshop_contract::ir::Step;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::diag::{DiagCode, Diagnostic};
use crate::source::{safe_ident, LoadedScope};

fn diag(scope: &LoadedScope, step_id: &str, code: DiagCode, message: String) -> Diagnostic {
    let item = format!("step:{step_id}");
    let file = if scope.file.trim().is_empty() {
        "."
    } else {
        scope.file.as_str()
    };
    let item = if step_id.trim().is_empty() {
        "pack"
    } else {
        item.as_str()
    };
    // file と item を空でない値に直したので、Diagnostic::new は失敗しない。
    Diagnostic::new(file, item, code, message).expect("file and item are non-empty")
}

/// serde の失敗を、値を含まない固定文にする。欄の名前は識別子として安全なときだけ写す。
fn parse_message(kind: &str, err: &serde_json::Error) -> String {
    let text = err.to_string();
    let named = [
        ("unknown field", "unknown field `"),
        ("missing field", "missing field `"),
    ]
    .iter()
    .find_map(|(label, prefix)| {
        let rest = text.strip_prefix(prefix)?;
        let name = &rest[..rest.find('`')?];
        safe_ident(name).then(|| format!("{label} {name}"))
    });
    match named {
        Some(f) => format!("input ({kind}): {f}"),
        None => format!("input ({kind}): invalid declaration"),
    }
}

fn read<T: DeserializeOwned>(rest: Value, kind: &str) -> Result<T, String> {
    serde_json::from_value(rest).map_err(|e| parse_message(kind, &e))
}

/// 型で読み、意味の制約を検査する。`Err` は (コード, メッセージ)。
fn check_one<T: DeserializeOwned>(
    rest: Value,
    kind: &str,
    check: impl Fn(&T) -> Result<(), SpecError>,
) -> Result<(), (DiagCode, String)> {
    let spec: T = read(rest, kind).map_err(|m| (DiagCode::Parse, m))?;
    check(&spec).map_err(|e| (DiagCode::Structure, format!("input ({kind}): {e}")))
}

fn check_input(input: &Value) -> Result<(), (DiagCode, String)> {
    let Some(obj) = input.as_object() else {
        return Err((DiagCode::Parse, "input: must be a mapping".to_string()));
    };
    let Some(kind) = obj.get("kind").and_then(Value::as_str) else {
        return Err((
            DiagCode::Parse,
            "input: kind is missing or not a string".to_string(),
        ));
    };
    if kind == "wait" {
        return Ok(());
    }
    let Some(k) = InputKind::from_str_opt(kind) else {
        let msg = if safe_ident(kind) {
            format!("input: unknown kind {kind}")
        } else {
            "input: unknown kind".to_string()
        };
        return Err((DiagCode::Parse, msg));
    };
    let mut rest = obj.clone();
    rest.remove("kind");
    let rest = Value::Object(rest);
    let name = k.as_str();
    match k {
        InputKind::Command => check_one::<CommandSpec>(rest, name, CommandSpec::check),
        InputKind::Text => check_one::<TextSpec>(rest, name, TextSpec::check),
        InputKind::Choice => check_one::<ChoiceSpec>(rest, name, ChoiceSpec::check),
        InputKind::UiAction => check_one::<UiActionSpec>(rest, name, UiActionSpec::check),
        InputKind::Approval => check_one::<ApprovalSpec>(rest, name, |_| Ok(())),
    }
}

/// 引数を置換した後の Step の `input` を検査する。`use` の展開が `input` の値を変えうるため
/// （例: コマンド名が空になる・選択肢の ID が重なる）、置換前の検査とは別に展開後にも行う。
/// 診断は定義元の章・モジュールのファイルと Step に紐付ける。
pub(crate) fn check_step_input(scope: &LoadedScope, step: &Step) -> Option<Diagnostic> {
    let input = step.input.as_ref()?;
    let (code, msg) = check_input(input).err()?;
    Some(diag(scope, &step.id, code, msg))
}

/// 章・モジュールのすべての Step の `input` を検査し、診断を集める。
pub(crate) fn check_inputs(scopes: &[LoadedScope]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for scope in scopes {
        for step in &scope.src.steps {
            let Some(input) = &step.input else { continue };
            if let Err((code, msg)) = check_input(input) {
                out.push(diag(scope, &step.id, code, msg));
            }
        }
    }
    out
}
