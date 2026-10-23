//! 案内レベル別の本文の木の選択と、濃い側への倒れ（TASK-292、GUIDE-1）。
//!
//! Step の本文は水準 A を `body`、水準 B・C を `body_levels` に書く。IR 版 2 ではそれぞれが木
//! （[`Node::body_doc`]・[`Node::body_levels_doc`]）として載る。本モジュールは、現在の案内レベルに合う木を
//! 選ぶ純関数だけを持つ。選ぶ順は契約 [`GuidanceLevel::fallback_chain`] をそのまま使い、規則を二重に持たない
//! （C は C → B → A、B は B → A、A は A だけ）。水準の版が無ければ、薄い側ではなく濃い側の本文へ倒れる。
//!
//! 呼び出し元は、本文の区画に出す木を決める web（SHELL-7 の結線）、実行時の選択が静的検査（pack の
//! `guidance::body_for`）と一致することを確かめる pack の試験、教材の文書生成である。パス文字列から本文を
//! 選ぶ `body_for` とは別物で、こちらは読み込み済みの木を選ぶ。
//!
//! # 安全側への倒し方（IR は未信頼の入力）
//!
//! - `body_levels_doc` の鍵 `A` は読まない。A の本文は `body_doc` だけで、pack が `E-STRUCTURE` にする規則を
//!   手で作った IR でも守る（B・C で伏せた語を含む版を誤って出さない）
//! - `A`・`B`・`C` 以外の鍵は探さないので読まれない
//! - 木を持たない IR 版 1 の Node は常に `None`。`step.body` のパスは実行時に解釈しない
//!
//! # 扱わないこと
//!
//! - 画面への表示とエスケープ（web）。返すのは検査済みの木の参照だけで、HTML を組み立てない
//! - ヒントの出し分け（TASK-293）、開始点の水準と切り替え（TASK-294）
//! - 本文が水準の定義に合うかの検査（pack の `E-GUIDANCE-LEAK`）
//!
//! 公開 API の契約の版（`CONTRACT_VERSION`）の対象に含まない。panic せず、I/O を持たない。

use fandhe_workshop_contract::doc::Doc;
use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::Node;

/// 選ばれた本文の木と、実際にどの水準の版だったか。
///
/// `level` は要求した水準ではなく、見つかった版の水準である。倒れたかどうかは `level` と要求の水準の
/// 比較で分かる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedBody<'a> {
    /// 見つかった版の水準。
    pub level: GuidanceLevel,
    /// 本文の木。
    pub doc: &'a Doc,
}

/// 水準 `level` で出す本文の木を選ぶ。無い水準の版は濃い側へ倒れる。どの版も無ければ `None`。
pub fn select_body(node: &Node, level: GuidanceLevel) -> Option<SelectedBody<'_>> {
    for l in level.fallback_chain() {
        let doc = if *l == GuidanceLevel::A {
            node.body_doc.as_ref()
        } else {
            node.body_levels_doc.get(l.as_str())
        };
        if let Some(doc) = doc {
            return Some(SelectedBody { level: *l, doc });
        }
    }
    None
}

/// [`select_body`] の木だけを返す。
pub fn body_doc_at(node: &Node, level: GuidanceLevel) -> Option<&Doc> {
    select_body(node, level).map(|s| s.doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc(t: &str) -> serde_json::Value {
        json!({"version":1,"blocks":[{"t":"paragraph","children":[{"t":"text","text":t}]}]})
    }

    fn node(a: Option<&str>, levels: &[(&str, &str)]) -> Node {
        let mut v = json!({"uid":"main/s","scope":"main","unit":"u","step":{"id":"s","title":"t"}});
        if let Some(a) = a {
            v["body_doc"] = doc(a);
        }
        if !levels.is_empty() {
            let m: serde_json::Map<String, serde_json::Value> = levels
                .iter()
                .map(|(k, t)| ((*k).to_string(), doc(t)))
                .collect();
            v["body_levels_doc"] = m.into();
        }
        serde_json::from_value(v).expect("node")
    }

    fn pick(n: &Node, l: GuidanceLevel) -> Option<GuidanceLevel> {
        select_body(n, l).map(|s| s.level)
    }

    #[test]
    fn level_a_key_in_levels_is_ignored() {
        let n = node(None, &[("A", "x")]);
        for l in GuidanceLevel::ALL {
            assert_eq!(pick(&n, l), None);
        }
        let n = node(Some("a"), &[("A", "x")]);
        assert_eq!(body_doc_at(&n, GuidanceLevel::A), n.body_doc.as_ref());
    }

    #[test]
    fn unknown_keys_are_never_read() {
        let n = node(None, &[("D", "x"), ("b", "y")]);
        for l in GuidanceLevel::ALL {
            assert_eq!(pick(&n, l), None);
        }
    }

    #[test]
    fn missing_trees_give_none() {
        let n = node(None, &[("C", "c")]);
        assert_eq!(pick(&n, GuidanceLevel::A), None);
        assert_eq!(pick(&n, GuidanceLevel::B), None);
        assert_eq!(pick(&n, GuidanceLevel::C), Some(GuidanceLevel::C));
    }
}
