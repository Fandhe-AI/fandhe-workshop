//! エンジン側の IR の読み込みの入口。
//!
//! 実行時のエンジンは IR だけを読み、構造定義や Markdown を読まない（PACK-1）。そのため入力は
//! バイト列だけで、ファイルを扱う標準ライブラリの機能は使わない（結合試験が機械で確かめる）。
//! web から呼ばれ、pack の `compile` が書き出した IR を受ける。
//! 大きさ・入れ子の上限は契約（TASK-199、`contract::limits`）に従い、JSON の解釈より前に判定する。

use std::collections::BTreeSet;
use std::fmt;

use fandhe_workshop_contract::ir::{Ir, IR_FORMAT_VERSION};
use fandhe_workshop_contract::limits::{LimitReason, MAX_IR_BYTES, MAX_JSON_DEPTH};

/// IR を読めない理由。メッセージには ID だけを含め、IR の本文は含めない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrLoadError {
    /// 大きさまたは入れ子の上限を超えた（解析前に判定する）。
    Limit(LimitReason),
    /// JSON として解釈できない、または欄が合わない（分類と位置だけで、入力値は含まない）。
    Json(String),
    /// `format_version` が未対応。
    FormatVersion(u32),
    /// `pack.ir_format` が未対応。
    PackIrFormat(u32),
    /// `nodes[].uid` が重複している。
    DuplicateUid(String),
    /// 開始または遷移の宛先が存在しない。
    UnresolvedRef {
        /// 参照元（開始の ID または Step の `uid`）。
        from: String,
        /// 存在しない宛先。
        to: String,
    },
}

impl fmt::Display for IrLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrLoadError::Limit(r) => write!(f, "IR rejected: {r}"),
            IrLoadError::Json(m) => write!(f, "invalid IR JSON: {m}"),
            IrLoadError::FormatVersion(v) => write!(f, "unsupported format_version: {v}"),
            IrLoadError::PackIrFormat(v) => write!(f, "unsupported pack.ir_format: {v}"),
            IrLoadError::DuplicateUid(u) => write!(f, "duplicate node uid: {u}"),
            IrLoadError::UnresolvedRef { from, to } => {
                write!(f, "unresolved reference: {from} -> {to}")
            }
        }
    }
}

impl std::error::Error for IrLoadError {}

/// 入力値を含まないメッセージにする。`serde_json` のエラー文字列には型が合わない入力値が入るため、
/// 分類と位置（行・桁）だけを使う。
fn json_failure(e: &serde_json::Error) -> String {
    use serde_json::error::Category;
    let class = match e.classify() {
        Category::Io => "io error",
        Category::Syntax => "syntax error",
        Category::Data => "type or field mismatch",
        Category::Eof => "unexpected end of input",
    };
    format!("{class} at line {} column {}", e.line(), e.column())
}

/// JSON の入れ子の最大の段数が上限を超えるかを、解析せずに数える（文字列の中の括弧は数えない）。
fn exceeds_depth(bytes: &[u8]) -> bool {
    let (mut depth, mut in_str, mut esc) = (0usize, false, false);
    for &b in bytes {
        if in_str {
            if esc {
                esc = false;
            } else if b == b'\\' {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > MAX_JSON_DEPTH {
                    return true;
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    false
}

/// IR のバイト列が大きさと入れ子の上限（契約の `MAX_IR_BYTES`・`MAX_JSON_DEPTH`）に収まるかを、解析せずに判定する。
///
/// `load_ir` が最初に行う判定そのもので、pack の `compile` が成功を返す前に同じ判定を使い、
/// 「書き出せたが読めない IR」を作らないために公開する（上限の値は contract だけが持つ）。
pub fn check_ir_limits(bytes: &[u8]) -> Result<(), LimitReason> {
    if bytes.len() as u64 > MAX_IR_BYTES {
        return Err(LimitReason::TooLarge);
    }
    if exceeds_depth(bytes) {
        return Err(LimitReason::TooDeep);
    }
    Ok(())
}

/// バイト列の IR を読み、版と参照の整合を確かめて返す。
///
/// 判定は契約の順（大きさ、入れ子の深さ、JSON の解釈、型・内容）に行う。
/// 拒否する側に倒す。確かめるのは、JSON の形（未知の欄を含む）・`format_version`・`pack.ir_format`・
/// `uid` の重複・開始と遷移の宛先の解決である。
pub fn load_ir(bytes: &[u8]) -> Result<Ir, IrLoadError> {
    check_ir_limits(bytes).map_err(IrLoadError::Limit)?;
    let ir: Ir = serde_json::from_slice(bytes).map_err(|e| IrLoadError::Json(json_failure(&e)))?;
    if ir.format_version != IR_FORMAT_VERSION {
        return Err(IrLoadError::FormatVersion(ir.format_version));
    }
    if ir.pack.ir_format != IR_FORMAT_VERSION {
        return Err(IrLoadError::PackIrFormat(ir.pack.ir_format));
    }
    let mut uids = BTreeSet::new();
    for n in &ir.nodes {
        if !uids.insert(n.uid.as_str()) {
            return Err(IrLoadError::DuplicateUid(n.uid.clone()));
        }
    }
    for s in &ir.starts {
        if !uids.contains(s.node.as_str()) {
            return Err(IrLoadError::UnresolvedRef {
                from: s.id.clone(),
                to: s.node.clone(),
            });
        }
    }
    for n in &ir.nodes {
        for t in &n.step.next {
            if !uids.contains(t.to.as_str()) {
                return Err(IrLoadError::UnresolvedRef {
                    from: n.uid.clone(),
                    to: t.to.clone(),
                });
            }
        }
    }
    Ok(ir)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: &str = concat!(
        r#"{"format_version":1,"#,
        r#""pack":{"id":"p","title":"t","version":"0","ir_format":1,"guidance":"A"},"#,
        r#""nodes":[{"uid":"main/a","scope":"main","unit":"u","#,
        r#""step":{"id":"a","title":"t","next":[{"to":"main/b"}]}},"#,
        r#"{"uid":"main/b","scope":"main","unit":"u","step":{"id":"b","title":"t"}}],"#,
        r#""starts":[{"id":"s","node":"main/a"}]}"#,
    );

    fn load(s: &str) -> Result<Ir, IrLoadError> {
        load_ir(s.as_bytes())
    }

    #[test]
    fn l1_accepts_minimal() {
        assert_eq!(load(MIN).unwrap().nodes.len(), 2);
    }

    #[test]
    fn l2_rejects_broken_json() {
        assert!(matches!(load("{"), Err(IrLoadError::Json(_))));
    }

    #[test]
    fn l3_rejects_unknown_top_field() {
        let s = MIN.replacen("\"nodes\"", "\"extra\":1,\"nodes\"", 1);
        assert!(matches!(load(&s), Err(IrLoadError::Json(_))));
    }

    #[test]
    fn l4_rejects_versions() {
        let s = MIN.replacen("\"format_version\":1", "\"format_version\":2", 1);
        assert_eq!(load(&s).unwrap_err(), IrLoadError::FormatVersion(2));
        let s = MIN.replacen("\"ir_format\":1", "\"ir_format\":2", 1);
        assert_eq!(load(&s).unwrap_err(), IrLoadError::PackIrFormat(2));
    }

    #[test]
    fn l5_rejects_duplicate_uid() {
        let s = MIN.replace("main/b\",\"scope", "main/a\",\"scope");
        assert!(matches!(load(&s), Err(IrLoadError::DuplicateUid(_))));
    }

    #[test]
    fn l6_rejects_unresolved_refs() {
        let s = MIN.replace("\"to\":\"main/b\"", "\"to\":\"main/zz\"");
        assert!(matches!(load(&s), Err(IrLoadError::UnresolvedRef { .. })));
        let s = MIN.replace("\"node\":\"main/a\"", "\"node\":\"main/zz\"");
        assert!(matches!(load(&s), Err(IrLoadError::UnresolvedRef { .. })));
    }

    #[test]
    fn l7_error_message_has_ids_only() {
        let e = IrLoadError::UnresolvedRef {
            from: "a".into(),
            to: "b".into(),
        };
        assert_eq!(e.to_string(), "unresolved reference: a -> b");
    }

    #[test]
    fn l8_json_error_has_no_input_value() {
        let s = MIN.replacen(
            "\"format_version\":1",
            "\"format_version\":\"SECRET-VALUE\"",
            1,
        );
        let Err(e) = load(&s) else {
            panic!("must be rejected")
        };
        assert!(!e.to_string().contains("SECRET-VALUE"), "{e}");
    }

    #[test]
    fn l9_rejects_too_large_before_parse() {
        let v = vec![b' '; MAX_IR_BYTES as usize + 1];
        assert_eq!(
            load_ir(&v).unwrap_err(),
            IrLoadError::Limit(LimitReason::TooLarge)
        );
        // ちょうどは大きさでは拒否しない（空白だけなので JSON エラーになる）。
        let v = vec![b' '; MAX_IR_BYTES as usize];
        assert!(matches!(load_ir(&v), Err(IrLoadError::Json(_))));
    }

    #[test]
    fn l10_rejects_too_deep_before_parse() {
        let nest = |n: usize| format!("{}{}", "[".repeat(n), "]".repeat(n));
        assert_eq!(
            load(&nest(MAX_JSON_DEPTH + 1)).unwrap_err(),
            IrLoadError::Limit(LimitReason::TooDeep)
        );
        assert!(matches!(
            load(&nest(MAX_JSON_DEPTH)),
            Err(IrLoadError::Json(_))
        ));
        // 文字列の中の括弧は数えない。
        let s = format!("{{\"a\":\"{}\"}}", "[".repeat(100));
        assert!(matches!(load(&s), Err(IrLoadError::Json(_))));
    }
}
