//! 案内レベル（EP-8。PACK-17）の語の読み取りと、水準ごとの本文の選択（TASK-49 1/2）。
//!
//! 呼び出し元は読み込み層 `source::load`（`check_workshop_levels`・`check_scope_levels`）と、後続の
//! 静的検査（`E-GUIDANCE-LEAK`・`E-HINT-HIDDEN`。TASK-49 2/2）である。語彙 v1.1 の 5.3 節に従い、
//! 水準の名前が `A`・`B`・`C` でない教材は読み込みで `E-PARSE` にする。読めない水準は IR に載せない
//! （教材データは信頼しない入力のため、失敗は安全側へ倒す）。
//!
//! 探す順は契約 `GuidanceLevel::fallback_chain` をそのまま使い、規則を二重に持たない。ヒントの出し分けは
//! 契約の `visible_hints` が持つ。実行時の本文の木の選択は core（TASK-292）の持ち分である。
//! 診断の文面は固定の英語で、書かれた値・`hide_words`・本文は写さない。

use std::collections::BTreeMap;

use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::Step;
use fandhe_workshop_core::command_class::CommandKind;
use serde_json::Value;

use crate::diag::{DiagCode, Diagnostic};
use crate::source::{safe_ident, ScopeSrc, WorkshopSrc};

/// 水準ごとに隠す語（`pack.guidance_levels.<水準>`）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuideRule {
    /// 隠すコマンドの種類。
    pub hide_command_kinds: Vec<CommandKind>,
    /// 隠す語。
    pub hide_words: Vec<String>,
}

/// `guidance_levels` を読めなかった理由。値は含めない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuidanceLevelsError {
    /// 表でない。
    NotTable,
    /// 水準の名前が `A`・`B`・`C` でない。
    BadLevel,
    /// 水準の値が表でない。
    BadRule,
    /// 未知の欄がある。
    UnknownField,
    /// `hide_command_kinds` が閉じた 4 語の配列でない。
    BadKinds,
    /// `hide_words` が文字列の配列でない。
    BadWords,
}

impl GuidanceLevelsError {
    fn message(self) -> &'static str {
        match self {
            Self::NotTable => "guidance_levels must be a table",
            Self::BadLevel => "guidance_levels keys must be A, B or C",
            Self::BadRule => "guidance_levels values must be tables",
            Self::UnknownField => "unknown key in guidance_levels",
            Self::BadKinds => "hide_command_kinds must be an array of command kinds",
            Self::BadWords => "hide_words must be an array of strings",
        }
    }
}

/// 水準 `level` で出す本文のパスを選ぶ。C → B → A の順に探す。
///
/// 水準 A の本文は `body` に書く。`body_levels` の鍵 `A` は本文として使わない（`E-STRUCTURE` は TASK-49 2/2）。
/// ディスクには触れず、パスの文字列だけを返す純関数である。
pub fn body_for(step: &Step, level: GuidanceLevel) -> Option<&str> {
    for l in level.fallback_chain() {
        if *l == GuidanceLevel::A {
            return step.body.as_deref();
        }
        if let Some(p) = step.body_levels.get(l.as_str()) {
            return Some(p.as_str());
        }
    }
    None
}

/// `pack.guidance_levels` を型つきで読む。`None` は空の表。
pub fn read_guidance_levels(
    v: Option<&Value>,
) -> Result<BTreeMap<GuidanceLevel, GuideRule>, GuidanceLevelsError> {
    let mut out = BTreeMap::new();
    let Some(v) = v else {
        return Ok(out);
    };
    let table = v.as_object().ok_or(GuidanceLevelsError::NotTable)?;
    for (k, rule) in table {
        let level = GuidanceLevel::parse(k).ok_or(GuidanceLevelsError::BadLevel)?;
        let fields = rule.as_object().ok_or(GuidanceLevelsError::BadRule)?;
        let mut r = GuideRule::default();
        for (name, val) in fields {
            match name.as_str() {
                "hide_command_kinds" => {
                    let arr = val.as_array().ok_or(GuidanceLevelsError::BadKinds)?;
                    for e in arr {
                        r.hide_command_kinds
                            .push(CommandKind::from_value(e).ok_or(GuidanceLevelsError::BadKinds)?);
                    }
                }
                "hide_words" => {
                    let arr = val.as_array().ok_or(GuidanceLevelsError::BadWords)?;
                    for e in arr {
                        r.hide_words
                            .push(e.as_str().ok_or(GuidanceLevelsError::BadWords)?.to_string());
                    }
                }
                _ => return Err(GuidanceLevelsError::UnknownField),
            }
        }
        out.insert(level, r);
    }
    Ok(out)
}

fn parse_diag(file: &str, item: &str, message: &str) -> Diagnostic {
    let file = if file.trim().is_empty() { "." } else { file };
    let item = if item.trim().is_empty() { "file" } else { item };
    // file と item を空でない値に直したので、Diagnostic::new は失敗しない。
    Diagnostic::new(file, item, DiagCode::Parse, message).expect("file and item are non-empty")
}

fn is_level(v: &Value) -> bool {
    v.as_str().and_then(GuidanceLevel::parse).is_some()
}

/// ヒントの `levels` が `A`・`B`・`C` の文字列の配列か。`levels` が無い定義は対象外。
fn hint_levels_ok(def: &Value) -> bool {
    let Some(levels) = def.as_object().and_then(|o| o.get("levels")) else {
        return true;
    };
    levels
        .as_array()
        .is_some_and(|arr| arr.iter().all(is_level))
}

/// `workshop.*` の水準の語（`pack.guidance`・`pack.guidance_levels`・ヒントの `levels`）を検査する。
pub(crate) fn check_workshop_levels(workshop_file: &str, w: &WorkshopSrc) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if GuidanceLevel::parse(&w.pack.guidance).is_none() {
        out.push(parse_diag(
            workshop_file,
            "pack:guidance",
            "pack.guidance must be one of A, B, C",
        ));
    }
    if let Err(e) = read_guidance_levels(w.pack.guidance_levels.as_ref()) {
        out.push(parse_diag(
            workshop_file,
            "pack:guidance_levels",
            e.message(),
        ));
    }
    for (id, def) in &w.hints {
        if !hint_levels_ok(def) {
            let item = if safe_ident(id) {
                format!("hint:{id}")
            } else {
                "hint".to_string()
            };
            out.push(parse_diag(
                workshop_file,
                &item,
                "hints.levels must be an array of A, B, C",
            ));
        }
    }
    out
}

/// 章・モジュールの水準の語（`starts[].guidance`・`body_levels` の鍵）を検査する。
///
/// 鍵 `A` はここでは通す（`E-STRUCTURE` にするのは TASK-49 2/2 の静的検査）。
pub(crate) fn check_scope_levels(file: &str, src: &ScopeSrc) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for s in &src.starts {
        if s.guidance.as_ref().is_some_and(|g| !is_level(g)) {
            out.push(parse_diag(
                file,
                "scope",
                "starts[].guidance must be one of A, B, C",
            ));
        }
    }
    for st in &src.steps {
        if st
            .body_levels
            .keys()
            .any(|k| GuidanceLevel::parse(k).is_none())
        {
            out.push(parse_diag(
                file,
                "scope",
                "body_levels keys must be A, B or C",
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn step(body: Option<&str>, levels: &[(&str, &str)]) -> Step {
        let mut v = json!({"id": "s", "title": "t"});
        if let Some(b) = body {
            v["body"] = json!(b);
        }
        let m: BTreeMap<&str, &str> = levels.iter().copied().collect();
        v["body_levels"] = json!(m);
        serde_json::from_value(v).expect("step")
    }

    #[test]
    fn body_falls_back_c_b_a() {
        let s = step(Some("a.md"), &[("B", "b.md"), ("C", "c.md")]);
        assert_eq!(body_for(&s, GuidanceLevel::A), Some("a.md"));
        assert_eq!(body_for(&s, GuidanceLevel::B), Some("b.md"));
        assert_eq!(body_for(&s, GuidanceLevel::C), Some("c.md"));
        let s = step(Some("a.md"), &[("B", "b.md")]);
        assert_eq!(body_for(&s, GuidanceLevel::C), Some("b.md"));
        let s = step(Some("a.md"), &[("C", "c.md")]);
        assert_eq!(body_for(&s, GuidanceLevel::B), Some("a.md"));
        let s = step(None, &[("A", "x.md")]);
        assert_eq!(body_for(&s, GuidanceLevel::C), None);
    }

    #[test]
    fn levels_table_reads_and_rejects() {
        let ok = json!({"B": {"hide_command_kinds": ["skill", "flow"], "hide_words": ["語 1"]}, "A": {}});
        let m = read_guidance_levels(Some(&ok)).expect("ok");
        assert_eq!(m[&GuidanceLevel::B].hide_command_kinds.len(), 2);
        assert!(read_guidance_levels(None).expect("none").is_empty());
        for (v, e) in [
            (json!([]), GuidanceLevelsError::NotTable),
            (json!({"D": {}}), GuidanceLevelsError::BadLevel),
            (json!({"B": 1}), GuidanceLevelsError::BadRule),
            (json!({"B": {"x": 1}}), GuidanceLevelsError::UnknownField),
            (
                json!({"B": {"hide_command_kinds": ["shell"]}}),
                GuidanceLevelsError::BadKinds,
            ),
            (
                json!({"B": {"hide_words": [1]}}),
                GuidanceLevelsError::BadWords,
            ),
        ] {
            assert_eq!(read_guidance_levels(Some(&v)), Err(e));
        }
    }

    #[test]
    fn hint_levels_shapes() {
        assert!(hint_levels_ok(&json!({"text": "x"})));
        assert!(hint_levels_ok(&json!({"levels": []})));
        assert!(hint_levels_ok(&json!({"levels": ["A", "C"]})));
        for bad in [
            json!("A"),
            json!(["a"]),
            json!([1]),
            json!([["A"]]),
            json!(["D"]),
        ] {
            assert!(!hint_levels_ok(&json!({"levels": bad})));
        }
    }
}
