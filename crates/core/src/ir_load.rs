//! エンジン側の IR の読み込みの入口。
//!
//! 実行時のエンジンは IR だけを読み、構造定義や Markdown を読まない（PACK-1）。そのため入力は
//! バイト列だけで、ファイルを扱う標準ライブラリの機能は使わない（結合試験が機械で確かめる）。
//! web から呼ばれ、pack の `compile` が書き出した IR を受ける。
//! 大きさ・入れ子の上限は契約（TASK-199、`contract::limits`）に従う。判定の順序（大きさ、入れ子の深さ、
//! JSON の解釈、型・内容）は contract の `LOAD_STAGE_ORDER` が正で、最初の 3 段と型の段の分離は
//! 共有の関門（[`crate::load_gate`]）が持つ。ここでは関門の後ろで、版と参照の整合だけを確かめる。

use std::collections::BTreeSet;
use std::fmt;

use fandhe_workshop_contract::ir::{is_supported_ir_format, Ir};
use fandhe_workshop_contract::limits::{LimitReason, LoadEntry};

use crate::load_gate::{check_limits, parse_json_value, LoadError};

/// IR を読めない理由。メッセージには ID だけを含め、IR の本文は含めない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrLoadError {
    /// 大きさまたは入れ子の上限を超えた（解析前に判定する）。
    Limit(LimitReason),
    /// JSON として解釈できない、または欄が合わない（分類だけで、入力値・位置は含まない）。
    Json(String),
    /// `format_version` が未対応。
    FormatVersion(u32),
    /// `pack.ir_format` が未対応。
    PackIrFormat(u32),
    /// 版の整合が取れていない（`format_version` と `pack.ir_format` が違う、または版 1 と名乗りながら
    /// 版 2 の欄を持つ）。メッセージには版の数値だけを入れる。
    FormatInconsistent {
        /// `format_version`。
        format_version: u32,
        /// `pack.ir_format`。
        pack_ir_format: u32,
    },
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
            IrLoadError::FormatInconsistent {
                format_version,
                pack_ir_format,
            } => write!(
                f,
                "inconsistent IR format: format_version {format_version}, pack.ir_format {pack_ir_format}"
            ),
            IrLoadError::DuplicateUid(u) => write!(f, "duplicate node uid: {u}"),
            IrLoadError::UnresolvedRef { from, to } => {
                write!(f, "unresolved reference: {from} -> {to}")
            }
        }
    }
}

impl std::error::Error for IrLoadError {}

/// 関門の解釈の段の失敗（`NotJson`）を表す分類。
const SYNTAX_ERROR: &str = "syntax error";
/// 型・内容の段の失敗（関門の `Malformed`、または `Ir` への変換の失敗）を表す分類。
const TYPE_MISMATCH: &str = "type or field mismatch";

/// 関門の理由を IR の読み込みの理由へ写す。`serde_json` のエラー文は入力値を含みうるため使わない。
fn from_gate(e: LoadError) -> IrLoadError {
    match e {
        LoadError::Limit(r) => IrLoadError::Limit(r),
        LoadError::NotJson => IrLoadError::Json(SYNTAX_ERROR.to_owned()),
        LoadError::Malformed => IrLoadError::Json(TYPE_MISMATCH.to_owned()),
    }
}

/// IR のバイト列が大きさと入れ子の上限（契約の `MAX_IR_BYTES`・`MAX_JSON_DEPTH`）に収まるかを、解析せずに判定する。
///
/// `load_ir` が通す関門と同じ判定（`load_gate::check_limits(LoadEntry::Ir, _)` をそのまま呼ぶ）で、
/// pack の `compile` が成功を返す前に使い、「書き出せたが読めない IR」を作らないために公開する
/// （上限の値は contract だけが持ち、深さの数え方は `json_depth` だけが持つ）。両者の一致は試験で確かめる。
///
/// # Errors
///
/// 大きさが `MAX_IR_BYTES` を超えれば `TooLarge`、入れ子が `MAX_JSON_DEPTH` を超えれば `TooDeep`。
pub fn check_ir_limits(bytes: &[u8]) -> Result<(), LimitReason> {
    check_limits(LoadEntry::Ir, bytes)
}

/// 解釈済みの値から版を先読みし、読める版（1・2）でなければその理由を返す。
///
/// 版が違う IR は、この版の知らない欄を持ちうる。型付きの変換の後に版を見ると、未知の欄の拒否が先に出て
/// 「版が未対応」という分類が失われる（PACK-24）。そのため型付きの変換より前に見る。整数として読めて
/// `u32` に収まる値だけを判定し、それ以外（欠落・非整数・範囲外）は型付きの変換に任せて拒否させる。
/// 範囲外の値を切り詰めて別の版として報告しない。
fn probe_version(value: &serde_json::Value) -> Option<IrLoadError> {
    let version = |v: Option<&serde_json::Value>| {
        v.and_then(serde_json::Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
    };
    if let Some(v) = version(value.get("format_version")).filter(|v| !is_supported_ir_format(*v)) {
        return Some(IrLoadError::FormatVersion(v));
    }
    if let Some(v) = version(value.get("pack").and_then(|p| p.get("ir_format")))
        .filter(|v| !is_supported_ir_format(*v))
    {
        return Some(IrLoadError::PackIrFormat(v));
    }
    None
}

/// バイト列の IR を読み、版と参照の整合を確かめて返す。
///
/// 判定は契約の順（大きさ、入れ子の深さ、JSON の解釈、型・内容）に行う。最初の 3 段は共有の関門に任せ、
/// 解釈と `Ir` への変換を分ける（`from_slice::<Ir>` の 1 回だと、構文の誤りより先に型の誤りが返りうる）。
/// 型・内容の段の中では、版の先読み、型付きの変換、版の整合、参照の整合の順に確かめる（先読みの理由は
/// [`probe_version`]）。拒否する側に倒す。確かめるのは、JSON の形（未知の欄を含む）・`format_version`・`pack.ir_format`（1 と 2 を読む。
/// 2 つは同じ値で、版 2 の欄を使うなら 2）・`uid` の重複・開始と遷移の宛先の解決である。
pub fn load_ir(bytes: &[u8]) -> Result<Ir, IrLoadError> {
    let value = parse_json_value(LoadEntry::Ir, bytes).map_err(from_gate)?;
    if let Some(e) = probe_version(&value) {
        return Err(e);
    }
    let ir: Ir =
        serde_json::from_value(value).map_err(|_| IrLoadError::Json(TYPE_MISMATCH.to_owned()))?;
    if !is_supported_ir_format(ir.format_version) {
        return Err(IrLoadError::FormatVersion(ir.format_version));
    }
    if !is_supported_ir_format(ir.pack.ir_format) {
        return Err(IrLoadError::PackIrFormat(ir.pack.ir_format));
    }
    // 2 つの版は同じ値でなければならない。版 1 と名乗りながら版 2 の欄（本文・練習データの木）を持つ IR は、
    // 版を偽っているので拒否する（安全側）。判定の正は `Ir::has_consistent_format`。
    if !ir.has_consistent_format() {
        return Err(IrLoadError::FormatInconsistent {
            format_version: ir.format_version,
            pack_ir_format: ir.pack.ir_format,
        });
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
    use fandhe_workshop_contract::limits::{
        LoadStage, LOAD_STAGE_ORDER, MAX_IR_BYTES, MAX_JSON_DEPTH,
    };

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
        let s = MIN.replacen("\"format_version\":1", "\"format_version\":3", 1);
        assert_eq!(load(&s).unwrap_err(), IrLoadError::FormatVersion(3));
        let s = MIN.replacen("\"ir_format\":1", "\"ir_format\":3", 1);
        assert_eq!(load(&s).unwrap_err(), IrLoadError::PackIrFormat(3));
    }

    /// `MIN` を版 2 にする（`format_version` と `pack.ir_format` の両方）。
    fn min_v2() -> String {
        MIN.replacen("\"format_version\":1", "\"format_version\":2", 1)
            .replacen("\"ir_format\":1", "\"ir_format\":2", 1)
    }

    /// 最初の Step の `uid` の直後（`step` を閉じた後）に、Node の欄を足す。
    fn with_node_field(base: &str, field: &str) -> String {
        base.replacen(
            "}},{\"uid\":\"main/b\"",
            &format!("}},{field}}},{{\"uid\":\"main/b\""),
            1,
        )
    }

    const BODY_DOC: &str = r#""body_doc":{"version":1,"blocks":[{"t":"rule"}]}"#;

    #[test]
    fn l16_accepts_version_two() {
        assert_eq!(load(&min_v2()).unwrap().format_version, 2);
        // 木を持つ版 2 の IR。
        let ir = load(&with_node_field(&min_v2(), BODY_DOC)).unwrap();
        assert!(ir.nodes[0].body_doc.is_some());
    }

    #[test]
    fn l17_rejects_inconsistent_versions() {
        let only_top = MIN.replacen("\"format_version\":1", "\"format_version\":2", 1);
        assert_eq!(
            load(&only_top).unwrap_err(),
            IrLoadError::FormatInconsistent {
                format_version: 2,
                pack_ir_format: 1
            }
        );
        let only_pack = MIN.replacen("\"ir_format\":1", "\"ir_format\":2", 1);
        assert_eq!(
            load(&only_pack).unwrap_err(),
            IrLoadError::FormatInconsistent {
                format_version: 1,
                pack_ir_format: 2
            }
        );
        // 版 1 と名乗りながら木を持つ IR。
        assert!(matches!(
            load(&with_node_field(MIN, BODY_DOC)),
            Err(IrLoadError::FormatInconsistent { .. })
        ));
    }

    #[test]
    fn l18_inconsistent_message_has_numbers_only() {
        let e = IrLoadError::FormatInconsistent {
            format_version: 1,
            pack_ir_format: 2,
        };
        assert_eq!(
            e.to_string(),
            "inconsistent IR format: format_version 1, pack.ir_format 2"
        );
    }

    #[test]
    fn l19_rejects_unknown_tree_content() {
        let bad = r#""body_doc":{"version":1,"blocks":[{"t":"script","text":"SECRET-VALUE"}]}"#;
        let Err(e) = load(&with_node_field(&min_v2(), bad)) else {
            panic!("must be rejected")
        };
        assert_eq!(e, IrLoadError::Json(TYPE_MISMATCH.to_owned()));
        assert!(!e.to_string().contains("SECRET-VALUE"));
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

    #[test]
    fn l14_version_is_probed_before_unknown_fields() {
        let s = MIN.replacen(
            "\"format_version\":1",
            "\"format_version\":3,\"future\":1",
            1,
        );
        assert_eq!(load(&s).unwrap_err(), IrLoadError::FormatVersion(3));
        let s = MIN
            .replacen("\"ir_format\":1", "\"ir_format\":3,\"future\":1", 1)
            .replacen("\"id\":\"a\"", "\"id\":\"a\",\"future\":1", 1);
        assert_eq!(load(&s).unwrap_err(), IrLoadError::PackIrFormat(3));
        let s = MIN.replacen("\"nodes\"", "\"future\":1,\"nodes\"", 1);
        assert_eq!(
            load(&s).unwrap_err(),
            IrLoadError::Json(TYPE_MISMATCH.to_owned())
        );
    }

    #[test]
    fn l15_unprobeable_versions_fall_through_to_type_mismatch() {
        for v in ["4294967296", "-1", "1.5", "3.0"] {
            let s = MIN.replacen(
                "\"format_version\":1",
                &format!("\"format_version\":{v}"),
                1,
            );
            assert_eq!(
                load(&s).unwrap_err(),
                IrLoadError::Json(TYPE_MISMATCH.to_owned()),
                "{v}"
            );
        }
        for s in [
            r#"{"format_version":1}"#,
            r#"{"format_version":1,"pack":[]}"#,
        ] {
            assert_eq!(
                load(s).unwrap_err(),
                IrLoadError::Json(TYPE_MISMATCH.to_owned())
            );
        }
    }

    /// 拒否した段。`Json` は分類の文字列で解釈の段と型・内容の段を分ける。
    fn stage_of(e: &IrLoadError) -> LoadStage {
        match e {
            IrLoadError::Limit(r) => r.stage(),
            IrLoadError::Json(m) if m == SYNTAX_ERROR => LoadStage::JsonParse,
            _ => LoadStage::TypeAndContent,
        }
    }

    fn max_ir_len() -> usize {
        usize::try_from(MAX_IR_BYTES).expect("limit fits usize")
    }

    /// 末尾を空白で埋めて `len` バイトにする（空白は JSON として意味を変えない）。
    fn pad_to(s: &str, len: usize) -> Vec<u8> {
        let mut v = s.as_bytes().to_vec();
        v.resize(len.max(v.len()), b' ');
        v
    }

    #[test]
    fn l11_syntax_error_precedes_type_mismatch() {
        // 前半（`format_version`）に型の不一致、後半（末尾の `}` の欠落）に構文の誤りがある。
        let s = MIN.replacen("\"format_version\":1", "\"format_version\":\"x\"", 1);
        let s = s.strip_suffix('}').expect("MIN ends with }");
        // `from_slice::<Ir>` の 1 回では、構文の誤りより先に型の誤りが返る（直した不具合の再現）。
        assert_eq!(
            serde_json::from_slice::<Ir>(s.as_bytes())
                .unwrap_err()
                .classify(),
            serde_json::error::Category::Data
        );
        let e = load(s).unwrap_err();
        assert_eq!(e, IrLoadError::Json(SYNTAX_ERROR.to_owned()));
        assert_eq!(stage_of(&e), LoadStage::JsonParse);
        // 構文が正しければ、同じ型の不一致は型・内容の段で断る。
        let s = MIN.replacen("\"format_version\":1", "\"format_version\":\"x\"", 1);
        let e = load(&s).unwrap_err();
        assert_eq!(e, IrLoadError::Json(TYPE_MISMATCH.to_owned()));
        assert_eq!(stage_of(&e), LoadStage::TypeAndContent);
    }

    /// 違反の組み合わせごとに、返る段は違反した段のうち契約の順で最も前のものになる。
    /// 型の不一致は常に入れ、それより前の段の違反を足し引きする。
    #[test]
    fn l12_rejecting_stage_is_the_first_violated_in_contract_order() {
        let typed = MIN.replacen("\"format_version\":1", "\"format_version\":\"x\"", 1);
        let deep = format!(
            "{{\"format_version\":\"x\",\"pack\":{}{}}}",
            "[".repeat(MAX_JSON_DEPTH + 1),
            "]".repeat(MAX_JSON_DEPTH + 1)
        );
        for size in [false, true] {
            for is_deep in [false, true] {
                for broken in [false, true] {
                    let base = if is_deep { &deep } else { &typed };
                    let text = if broken {
                        base.strip_suffix('}').expect("ends with }")
                    } else {
                        base.as_str()
                    };
                    let bytes = if size {
                        pad_to(text, max_ir_len() + 1)
                    } else {
                        text.as_bytes().to_vec()
                    };
                    let violated = |s: LoadStage| match s {
                        LoadStage::Size => size,
                        LoadStage::Depth => is_deep,
                        LoadStage::JsonParse => broken,
                        LoadStage::TypeAndContent => true,
                    };
                    let expected = LOAD_STAGE_ORDER.into_iter().find(|s| violated(*s));
                    let got = stage_of(&load_ir(&bytes).unwrap_err());
                    assert_eq!(
                        Some(got),
                        expected,
                        "size={size} deep={is_deep} broken={broken}"
                    );
                }
            }
        }
    }

    /// `check_ir_limits` は、`load_ir` が大きさ・深さで断る入力とちょうど同じものを断る。
    #[test]
    fn l13_check_ir_limits_agrees_with_load_ir() {
        let nest = |n: usize| format!("{}{}", "[".repeat(n), "]".repeat(n));
        let inputs = [
            MIN.as_bytes().to_vec(),
            b"{".to_vec(),
            pad_to(MIN, max_ir_len()),
            pad_to(MIN, max_ir_len() + 1),
            nest(MAX_JSON_DEPTH).into_bytes(),
            nest(MAX_JSON_DEPTH + 1).into_bytes(),
            pad_to(&nest(MAX_JSON_DEPTH + 1), max_ir_len() + 1),
        ];
        for b in &inputs {
            let by_load = match load_ir(b) {
                Err(IrLoadError::Limit(r)) => Some(r),
                _ => None,
            };
            assert_eq!(check_ir_limits(b).err(), by_load, "len={}", b.len());
        }
    }
}
