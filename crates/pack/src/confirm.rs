//! Step の `confirm`（確認 4 種）の宣言と、確認の未指定を検査する（TASK-47、PACK-13、R-22）。
//!
//! `compile` から 1 回だけ呼ばれる。入力を持つ Step は、確認の操作（`point_at`・`answer`・`approve`）か、
//! 理由つきの省略（`confirm: { kind: none, reason: ... }`）のどちらかを必ず持つ。どちらも無い Step は
//! 受付だけで達成になりうるため `E-NO-CONFIRM` で拒否する。型は contract の
//! [`fandhe_workshop_contract::confirm_spec`] が持ち、実行時の判定（core）・選択肢の表示（web）と共有する。
//! IR の `Step.confirm` は未型付けの値のまま出す（golden のバイト一致を保つ）ので、ここでは検査だけを行う。
//!
//! 診断の振り分けは `input.rs` と同じである。型で読めない場合（未知の欄・未知の `kind`・欄の欠け・型の違い）は
//! `E-PARSE`、意味の制約の違反は `E-STRUCTURE` にする。メッセージには、識別子として安全な欄・`kind` の名前だけを写し、
//! `reason`・経路・選択肢の ID・文面の ID・serde の元の文言（値を含みうる）は写さない。
//!
//! 決定事項（2026-10-15）:
//! - `none` の `reason` の欠け・空・空白だけは、PACK-13 の「理由つき」の違反として `E-NO-CONFIRM` にする
//!   （`E-PARSE` にしない）
//! - `use` を持つ Step は検査しない（展開は TASK-50 の担当）
//! - 入力の無い Step の `confirm` は、形だけを検査し、それ自体は診断にしない（該当する診断コードが spec に無い）
//! - `wait`（EP-1）の Step も入力を持つ Step として確認を要する
//!
//! `answer` の選択肢の `label`（表示名）が空白だけの場合は、contract の `AnswerSpec::check` が `EmptyName` を返し、
//! `E-STRUCTURE` の固定文になる（2026-10-15、TASK-47 2/2）。`on_wrong` は形として受け付けるだけで、意味の検査は TASK-46 が持つ。
//! `on_miss`・`on_wrong` の参照先の解決は参照検査（TASK-46・PACK-28）の担当である。

use fandhe_workshop_contract::confirm_spec::{
    AnswerSpec, ApproveSpec, ConfirmKind, ConfirmSpecError, NoneSpec, PointAtSpec,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::diag::{DiagCode, Diagnostic};
use crate::input::diag;
use crate::source::{safe_ident, LoadedScope};

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
        Some(f) => format!("confirm ({kind}): {f}"),
        None => format!("confirm ({kind}): invalid declaration"),
    }
}

fn read<T: DeserializeOwned>(rest: Value, kind: &str) -> Result<T, (DiagCode, String)> {
    serde_json::from_value(rest).map_err(|e| (DiagCode::Parse, parse_message(kind, &e)))
}

/// 型で読み、意味の制約を検査する。
fn check_one<T: DeserializeOwned>(
    rest: Value,
    kind: &str,
    check: impl Fn(&T) -> Result<(), ConfirmSpecError>,
) -> Result<(), (DiagCode, String)> {
    let spec: T = read(rest, kind)?;
    check(&spec).map_err(|e| (DiagCode::Structure, format!("confirm ({kind}): {e}")))
}

/// `confirm` を検査する。`Err` は (コード, メッセージ)。
fn check_confirm(confirm: &Value) -> Result<(), (DiagCode, String)> {
    let Some(obj) = confirm.as_object() else {
        return Err((DiagCode::Parse, "confirm: must be a mapping".to_string()));
    };
    let Some(kind) = obj.get("kind").and_then(Value::as_str) else {
        return Err((
            DiagCode::Parse,
            "confirm: kind is missing or not a string".to_string(),
        ));
    };
    let Some(k) = ConfirmKind::from_str_opt(kind) else {
        let msg = if safe_ident(kind) {
            format!("confirm: unknown kind {kind}")
        } else {
            "confirm: unknown kind".to_string()
        };
        return Err((DiagCode::Parse, msg));
    };
    let mut rest = obj.clone();
    rest.remove("kind");
    let rest = Value::Object(rest);
    let name = k.as_str();
    match k {
        ConfirmKind::PointAt => check_one::<PointAtSpec>(rest, name, PointAtSpec::check),
        ConfirmKind::Answer => check_one::<AnswerSpec>(rest, name, AnswerSpec::check),
        ConfirmKind::Approve => check_one::<ApproveSpec>(rest, name, |_| Ok(())),
        ConfirmKind::None => {
            let spec: NoneSpec = read(rest, name)?;
            if spec.has_reason() {
                Ok(())
            } else {
                Err((
                    DiagCode::NoConfirm,
                    "confirm (none): reason is required".to_string(),
                ))
            }
        }
    }
}

/// 章・モジュールのすべての Step の `confirm` を検査し、診断を集める。
pub(crate) fn check_confirms(scopes: &[LoadedScope]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for scope in scopes {
        for step in &scope.src.steps {
            if step.use_module.is_some() {
                continue;
            }
            let result = match (&step.confirm, &step.input) {
                (Some(confirm), _) => check_confirm(confirm),
                (None, Some(_)) => Err((
                    DiagCode::NoConfirm,
                    "confirm is missing; declare a confirmation or confirm: { kind: none } with a reason"
                        .to_string(),
                )),
                (None, None) => Ok(()),
            };
            if let Err((code, msg)) = result {
                out.push(diag(scope, &step.id, code, msg));
            }
        }
    }
    out
}
