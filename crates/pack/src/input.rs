//! Step の `input`（入力方式 5 種と待機）の宣言と、規則の `on.cancel` を検査する（TASK-44、PACK-10）。
//!
//! `compile` から 1 回だけ呼ばれる。IR の `Step.input` は未型付けの値のまま出す（golden のバイト一致を
//! 保つため）ので、ここでは型で読めるかと意味の制約だけを確かめ、値は変えない。型は contract の
//! [`fandhe_workshop_contract::input_spec`] が持ち、実行時の `text` の判定（core）と共有する。
//!
//! 診断のコードは新設しない。型で読めない場合（未知の欄・未知の `kind`・欄の欠け・型の違い）は
//! `E-PARSE`、意味の制約の違反は `E-STRUCTURE` にする。メッセージには、短い識別子として安全な
//! 欄・`kind` の名前だけを写し、入力の値と serde の元の文言（値を含みうる）は写さない。
//! `wait`（EP-1）は欄を持たず、Step の `processing` が空だと進行の完了で受付になれず先へ進めないので
//! `E-STRUCTURE` にする（向きは片方だけで、`processing` があって `wait` でない Step は許す）。`processing` の
//! 中身は TASK-43 の担当で、ここでは件数だけを見る。取り消し（EP-2）は `input.kind` ではなく規則の
//! `on: { cancel: true }` で受けるため、ここで見るのは `on.cancel` の値だけである。`on` の他のキーとの
//! 組み合わせ・入力方式との整合は TASK-45 が受け持つ。
//!
//! `approval` の `revise`（TASK-62 2/2、PACK-42）は「直す」の誤った対象の候補と表示名の宣言である。型と意味の
//! 制約（空・重複・空白だけの `label`）に加え、`revise` を宣言した Step では、対象つきの `revise` の規則
//! （`on: { approval: revise, target }`）の `target` がすべて候補に入っていることを確かめる。正しい対象が候補から
//! 漏れると、画面（候補のボタン）と判定（規則）が食い違うためである。どの候補が正しいかは宣言せず、規則が決める。

use fandhe_workshop_contract::input_spec::{
    ApprovalSpec, ChoiceSpec, CommandSpec, InputKind, SpecError, TextSpec, UiActionSpec, WaitSpec,
    CANCEL_RULE_KEY,
};
use fandhe_workshop_contract::ir::Step;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::BTreeSet;

use crate::diag::{DiagCode, Diagnostic};
use crate::source::{safe_ident, LoadedScope, ScopeKind};

pub(crate) fn diag(
    scope: &LoadedScope,
    step_id: &str,
    code: DiagCode,
    message: String,
) -> Diagnostic {
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

/// `revise` を宣言した `approval` で、対象つきの `revise` の規則の `target` が候補に無いものを探す。
/// `rest` は `kind` を除いた宣言。型で読めない場合は呼び出し元が先に報告済みなので何も返さない。
/// 値（候補の ID・`target`）はメッセージに写さない。
fn check_revise_targets(rest: &Value, rules: &[Value]) -> Result<(), (DiagCode, String)> {
    let Ok(spec) = serde_json::from_value::<ApprovalSpec>(rest.clone()) else {
        return Ok(());
    };
    let Some(revise) = spec.revise else {
        return Ok(());
    };
    let declared: BTreeSet<&str> = revise.candidates.iter().map(|c| c.id.as_str()).collect();
    for rule in rules {
        let Some(on) = rule.get("on").and_then(Value::as_object) else {
            continue;
        };
        if on.get("approval").and_then(Value::as_str) != Some("revise") {
            continue;
        }
        let Some(target) = on.get("target").and_then(Value::as_str) else {
            continue;
        };
        if !declared.contains(target) {
            return Err((
                DiagCode::Structure,
                "input (approval): revise rule target is not a declared candidate".to_string(),
            ));
        }
    }
    Ok(())
}

fn check_input(
    input: &Value,
    rules: Option<&[Value]>,
    has_processing: bool,
    redact: bool,
) -> Result<(), (DiagCode, String)> {
    let Some(obj) = input.as_object() else {
        return Err((DiagCode::Parse, "input: must be a mapping".to_string()));
    };
    let Some(kind) = obj.get("kind").and_then(Value::as_str) else {
        return Err((
            DiagCode::Parse,
            "input: kind is missing or not a string".to_string(),
        ));
    };
    let Some(k) = InputKind::from_str_opt(kind) else {
        let msg = if !redact && safe_ident(kind) {
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
        InputKind::Approval => {
            check_one::<ApprovalSpec>(rest.clone(), name, ApprovalSpec::check)?;
            match rules {
                Some(r) => check_revise_targets(&rest, r),
                None => Ok(()),
            }
        }
        InputKind::Wait => {
            check_one::<WaitSpec>(rest, name, |_| Ok(()))?;
            if has_processing {
                Ok(())
            } else {
                Err((
                    DiagCode::Structure,
                    "input (wait): processing is required".to_string(),
                ))
            }
        }
    }
}

/// 引数を置換した後の Step の `input` を検査する。`use` の展開が `input` の値を変えうるため
/// （例: コマンド名が空になる・選択肢の ID が重なる）、置換前の `input` が `${` を含む Step について
/// 展開が呼ぶ（含まない Step は置換前の検査だけで足り、同じ違反を 2 回報告しない）。
/// 診断は定義元の章・モジュールのファイルと Step に紐付ける。
pub(crate) fn check_step_input(
    scope: &LoadedScope,
    step: &Step,
    redact: bool,
) -> Option<Diagnostic> {
    let input = step.input.as_ref()?;
    let (code, msg) = check_input(
        input,
        Some(&step.rules),
        !step.processing.is_empty(),
        redact,
    )
    .err()?;
    Some(diag(scope, &step.id, code, msg))
}

/// 規則だけが置換記号 `${` を含む Step の、展開後の `revise` の対象の突き合わせ。
/// `input` が `${` を含む Step は `check_step_input` が同じ検査を含むので、ここは呼ばない。
pub(crate) fn check_step_revise_targets(scope: &LoadedScope, step: &Step) -> Option<Diagnostic> {
    let input = step.input.as_ref()?;
    let obj = input.as_object()?;
    if obj.get("kind").and_then(Value::as_str) != Some("approval") {
        return None;
    }
    let mut rest = obj.clone();
    rest.remove("kind");
    let (code, msg) = check_revise_targets(&Value::Object(rest), &step.rules).err()?;
    Some(diag(scope, &step.id, code, msg))
}

/// 規則が置換記号 `${` を含むか。
pub(crate) fn rules_have_placeholder(rules: &[Value]) -> bool {
    rules.iter().any(|r| r.to_string().contains("${"))
}

/// 規則の `on.cancel`（EP-2）の値だけを検査する。メッセージに規則の ID・値は写さない。
fn check_cancel_rules(rules: &[Value]) -> Vec<(DiagCode, String)> {
    rules
        .iter()
        .filter_map(|r| {
            let v = r.get("on")?.as_object()?.get(CANCEL_RULE_KEY)?;
            match v.as_bool() {
                Some(true) => None,
                Some(false) => Some((
                    DiagCode::Structure,
                    "rule (cancel): on.cancel must be true".to_string(),
                )),
                None => Some((
                    DiagCode::Parse,
                    "rule (cancel): on.cancel must be a boolean".to_string(),
                )),
            }
        })
        .collect()
}

/// 宣言（`input`・`confirm`）が引数の置換記号 `${` を含むか。含まない宣言は置換で変わらないので、
/// 置換前の検査の結果が置換後にもそのまま当てはまる。
pub(crate) fn has_placeholder(decl: &Option<Value>) -> bool {
    decl.as_ref().is_some_and(|v| v.to_string().contains("${"))
}

/// 章・モジュールのすべての Step の `input` を検査し、診断を集める。
pub(crate) fn check_inputs(scopes: &[LoadedScope]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for scope in scopes {
        for step in &scope.src.steps {
            // モジュールの `input` が `${...}` を含むときは、置換前の値が語彙として読めないことがある。
            // その宣言は検査せず、展開後の宣言を `check_step_input` が検査する。
            if scope.kind == ScopeKind::Chapter || !has_placeholder(&step.input) {
                if let Some(input) = &step.input {
                    // 規則の `target` が置換記号を含むモジュールは、置換前の値で突き合わせない（展開後に見る）。
                    let rules = (scope.kind == ScopeKind::Chapter
                        || !rules_have_placeholder(&step.rules))
                    .then_some(step.rules.as_slice());
                    if let Err((code, msg)) =
                        check_input(input, rules, !step.processing.is_empty(), false)
                    {
                        out.push(diag(scope, &step.id, code, msg));
                    }
                }
            }
            for (code, msg) in check_cancel_rules(&step.rules) {
                out.push(diag(scope, &step.id, code, msg));
            }
        }
    }
    out
}
