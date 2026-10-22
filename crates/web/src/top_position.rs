//! 上部の「章の ID と Step の位置（M / K）」の導出（TASK-524。SHELL-8・D-72）。
//!
//! 呼び出し元は `host::EngineHost` の `top_place` だけである。IR と現在の節点の uid から、章の ID・M・K を求める
//! 純粋関数を持つ。DOM の値は読まない（状態と IR から導く。fail-closed）。
//!
//! 「全 K 件中の M 番目」は、章（`scope == "main"` で `unit` が同じ節点）を IR の節点の並び（章の定義順）で数えた値である。
//! モジュールの展開した Step は、呼び出した `use` の Step（`Instance::caller`）をたどって章の Step に読み替える。
//! 分岐のある章では、分岐の Step も K に含めるので、M が 2 以上飛ぶことがある（`docs/known-gaps.md`）。

use fandhe_workshop_contract::ir::Ir;

use crate::shell::StepPosition;

/// 章の Step の `scope`。
const MAIN_SCOPE: &str = "main";

/// 現在の節点の（章の ID, 位置）。導けないとき（節点が無い・caller をたどれない・循環）は `None`（出さない）。
///
/// 章の ID は IR の `unit` をそのまま返す（綴りを書き換えない。`docs/known-gaps.md`）。
pub fn top_position(ir: &Ir, current_uid: &str) -> Option<(String, StepPosition)> {
    let mut node = ir.nodes.iter().find(|n| n.uid == current_uid)?;
    // caller をたどる回数の上限。循環する IR でも止まる。
    let mut budget = ir.instances.len() + 1;
    while node.scope != MAIN_SCOPE {
        if budget == 0 {
            return None;
        }
        budget -= 1;
        let caller = ir
            .instances
            .iter()
            .find(|i| i.id == node.scope)?
            .caller
            .as_deref()?;
        node = ir.nodes.iter().find(|n| n.uid == caller)?;
    }
    let chapter = node.unit.as_str();
    let mut m = 0u32;
    let mut k = 0u32;
    for n in ir
        .nodes
        .iter()
        .filter(|n| n.scope == MAIN_SCOPE && n.unit == chapter)
    {
        k += 1;
        if n.uid == node.uid {
            m = k;
        }
    }
    StepPosition::new(m, k).map(|p| (chapter.to_owned(), p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ir_from(path: &str) -> Ir {
        let p = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), path);
        serde_json::from_slice(&std::fs::read(p).expect("read")).expect("parse")
    }

    #[test]
    fn ch0_3_positions_match_chapter_order() {
        let ir = ir_from("tests/data/ch0_3/ir.json");
        let mut seen = std::collections::BTreeMap::<String, Vec<u32>>::new();
        for n in ir.nodes.iter().filter(|n| n.scope == "main") {
            let (c, p) = top_position(&ir, &n.uid).expect("position");
            assert_eq!(c, n.unit);
            seen.entry(c).or_default().push(p.m());
            let k = ir
                .nodes
                .iter()
                .filter(|x| x.scope == "main" && x.unit == n.unit)
                .count() as u32;
            assert_eq!(p.k(), k);
        }
        assert_eq!(seen["ch-1"], vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(seen["ch-0"].len(), 4);
        assert_eq!(seen["ch-2"].len(), 8);
        assert_eq!(seen["ch-3"].len(), 2);
    }

    #[test]
    fn module_steps_resolve_to_caller_chapter() {
        let ir = ir_from(
            "../../testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir/cycle-d07-flat.json",
        );
        let nested = ir
            .nodes
            .iter()
            .find(|n| n.scope.contains("merge-gate@gate"))
            .expect("nested node");
        let (c, p) = top_position(&ir, &nested.uid).expect("nested");
        let caller = ir
            .nodes
            .iter()
            .find(|n| n.uid == "main/u-impl")
            .expect("caller");
        assert_eq!(c, caller.unit);
        let (_, direct) = top_position(&ir, "main/u-impl").expect("direct");
        assert_eq!(p, direct);
    }

    #[test]
    fn unresolvable_is_none_without_panic() {
        let mut ir = ir_from("tests/data/ch0_3/ir.json");
        assert!(top_position(&ir, "no-such").is_none());
        let mut n = ir.nodes[0].clone();
        n.uid = "loop@a/x".into();
        n.scope = "loop@a".into();
        ir.nodes.push(n);
        // caller の無い実体
        ir.instances.push(fandhe_workshop_contract::ir::Instance {
            id: "loop@a".into(),
            module: Some("m".into()),
            args: Default::default(),
            caller: None,
        });
        assert!(top_position(&ir, "loop@a/x").is_none());
        // caller が自分自身を指す循環
        ir.instances.last_mut().expect("last").caller = Some("loop@a/x".into());
        assert!(top_position(&ir, "loop@a/x").is_none());
    }
}
