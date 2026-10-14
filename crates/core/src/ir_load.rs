//! エンジン側の IR の読み込みの入口。
//!
//! 実行時のエンジンは IR だけを読み、構造定義や Markdown を読まない（PACK-1）。そのため入力は
//! バイト列だけで、ファイルを扱う標準ライブラリの機能は使わない（結合試験が機械で確かめる）。
//! web から呼ばれ、pack の `compile` が書き出した IR を受ける。
//! IR の大きさ・入れ子の上限は TASK-199 の契約を待つ。

use std::collections::BTreeSet;
use std::fmt;

use fandhe_workshop_contract::ir::{Ir, IR_FORMAT_VERSION};

/// IR を読めない理由。メッセージには ID だけを含め、IR の本文は含めない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrLoadError {
    /// JSON として解釈できない、または欄が合わない。
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

/// バイト列の IR を読み、版と参照の整合を確かめて返す。
///
/// 拒否する側に倒す。確かめるのは、JSON の形（未知の欄を含む）・`format_version`・`pack.ir_format`・
/// `uid` の重複・開始と遷移の宛先の解決である。
pub fn load_ir(bytes: &[u8]) -> Result<Ir, IrLoadError> {
    let ir: Ir = serde_json::from_slice(bytes).map_err(|e| IrLoadError::Json(e.to_string()))?;
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
}
