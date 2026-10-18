//! 存在しない要素への `remove`・`set_attr` の検査（`E-REF-ELEMENT`。TASK-58 2/2、PACK-30・D-43）。
//!
//! `validate` だけが呼ぶ（`compile` は呼ばない。D-43 が `validate` での検出を求めるため）。世界状態は空で
//! 始まり、要素を作るのは `put`・`replace` だけである。実行時に存在しない要素への `remove`・`set_attr` は
//! `Event::Warning` になり教材テストが失敗する（H-6）ので、その手前の静的な必要条件として
//! 「教材のどこの `put`・`replace` も置かない経路」を持つ `remove`・`set_attr` を報告する。
//!
//! 判定は [`crate::placed::Placed`]（`E-LEFT-TARGET-ROW` と共有）による近似で、`when` つきの `put` も置くと数え、
//! `put` 側の `${...}` は空文字列を含む任意の文字列として照合する。誤検出を出さない側に倒す代わりに、
//! 次は検出しない限界である（実行時の `Event::Warning` と教材テストで補う）。
//!
//! - 順序に依る不在（`put` より前の `remove`、2 度目の `remove`）
//! - `put` が通らない枝での `set_attr`
//! - `replace` の対象（不在なら置くため、そもそも報告しない）
//!
//! モジュールの引数（`${`）を含む `remove`・`set_attr` の経路は展開の前に決まらないため展開の前には判定せず、
//! 展開の後の Step（[`check_missing_elements_expanded`]）で判定する。教材データは信頼しない入力として扱い、
//! 診断の文面は固定の英語で、経路は `safe_ident` を通るときだけ写す（属性値は写さない）。
//! 読めない効果の項目は飛ばす（語彙の誤りは `effect_check` が報告する）。

use std::collections::BTreeSet;

use fandhe_workshop_contract::effect::{Effect, EffectItem};
use fandhe_workshop_contract::ir::{Node, Start, Step};
use serde_json::Value;

use crate::diag::{DiagCode, Diagnostic};
use crate::placed::Placed;
use crate::source::{LoadedPack, ScopeKind};
use crate::validate::{diag, elements, get, shown};

/// 全スコープの Step と開始点を展開の前に検査する。`${` を含む経路は飛ばす（展開の後に検査する）。
///
/// 順は、スコープの順・開始点・Step の順で、Step の中は `on_enter`・`on_achieved`・`processing`・`rules`。
pub fn check_missing_elements(pack: &LoadedPack) -> Vec<Diagnostic> {
    let placed = Placed::collect(pack);
    let mut out = Vec::new();
    for scope in &pack.scopes {
        for start in &scope.src.starts {
            let item = format!("start:{}", start.id);
            let site = Site {
                file: &scope.file,
                item: &item,
                pre_expansion: true,
            };
            check_list(&mut out, &site, "effects", &start.effects, &placed);
        }
        for step in &scope.src.steps {
            check_step(&mut out, &scope.file, step, &placed, true);
        }
    }
    out
}

/// 展開の後（モジュールの引数の置換の後）の Step を検査する。
///
/// 置かれる経路は展開後の全 Step と開始点から集め、検査はモジュールの実体（`scope != "main"`）だけにする
/// （章の Step は置換されず、展開の前の検査で足りる）。診断は Step を定義したモジュールのファイルに付け、
/// 同じ内容は 1 件にする。
pub fn check_missing_elements_expanded(
    pack: &LoadedPack,
    nodes: &[Node],
    starts: &[Start],
) -> Vec<Diagnostic> {
    let mut placed = Placed::empty();
    for start in starts {
        placed.add_effects(&start.effects);
    }
    for node in nodes {
        placed.add_step(&node.step);
    }
    let mut out = Vec::new();
    for node in nodes.iter().filter(|n| n.scope != "main") {
        let file = pack
            .scopes
            .iter()
            .find(|s| s.kind == ScopeKind::Module && s.src.id == node.unit)
            .map_or(pack.workshop_file.as_str(), |s| s.file.as_str());
        check_step(&mut out, file, &node.step, &placed, false);
    }
    let mut seen = BTreeSet::new();
    out.retain(|d| seen.insert(format!("{d:?}")));
    out
}

/// 診断を積む場所。
struct Site<'a> {
    file: &'a str,
    item: &'a str,
    /// 展開の前か。展開の前は `${` を含む経路を判定しない。
    pre_expansion: bool,
}

fn check_step(
    out: &mut Vec<Diagnostic>,
    file: &str,
    step: &Step,
    placed: &Placed<'_>,
    pre_expansion: bool,
) {
    let item = format!("step:{}", step.id);
    let site = Site {
        file,
        item: &item,
        pre_expansion,
    };
    check_list(out, &site, "on_enter", &step.on_enter, placed);
    check_list(out, &site, "on_achieved", &step.on_achieved, placed);
    for (i, proc) in step.processing.iter().enumerate() {
        let field = format!("processing[{i}].effects");
        check_list(out, &site, &field, elements(get(proc, "effects")), placed);
    }
    for (i, rule) in step.rules.iter().enumerate() {
        if let Some(outcome) = get(rule, "outcome") {
            let field = format!("rules[{i}].outcome.effects");
            check_list(
                out,
                &site,
                &field,
                elements(get(outcome, "effects")),
                placed,
            );
        }
    }
}

/// 効果の列の `remove`・`set_attr` を調べ、置かれない経路ごとに `E-REF-ELEMENT` を積む。
fn check_list(
    out: &mut Vec<Diagnostic>,
    site: &Site<'_>,
    field: &str,
    list: &[Value],
    placed: &Placed<'_>,
) {
    for (i, v) in list.iter().enumerate() {
        let Ok(effect) = EffectItem::from_value(v) else {
            continue;
        };
        let (word, path) = match &effect.effect {
            Effect::Remove(path) => ("remove", path.as_str()),
            Effect::SetAttr(s) => ("set_attr", s.path.as_str()),
            _ => continue,
        };
        if (site.pre_expansion && path.contains("${")) || placed.contains(path) {
            continue;
        }
        out.push(diag(
            site.file,
            site.item,
            DiagCode::RefElement,
            format!(
                "{field}[{i}]: {word} target path {} is not placed by any put or replace",
                shown(path)
            ),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn run(effects: &[Value], put: &[Value], pre: bool) -> Vec<Diagnostic> {
        let mut placed = Placed::empty();
        placed.add_effects(put);
        let site = Site {
            file: "f",
            item: "step:a",
            pre_expansion: pre,
        };
        let mut out = Vec::new();
        check_list(&mut out, &site, "on_enter", effects, &placed);
        out
    }

    #[test]
    fn unplaced_remove_and_set_attr_are_reported() {
        let e = [
            json!({"remove": "gone"}),
            json!({"set_attr": {"path": "lost.x", "attr": "s", "value": "SECRET-VALUE"}}),
        ];
        let d = run(&e, &[], true);
        assert_eq!(d.len(), 2);
        assert!(d.iter().all(|d| d.code() == DiagCode::RefElement));
        assert_eq!(
            d[0].message(),
            "on_enter[0]: remove target path gone is not placed by any put or replace"
        );
        assert!(d[1]
            .message()
            .starts_with("on_enter[1]: set_attr target path lost.x"));
        assert!(!d[1].message().contains("SECRET-VALUE"));
    }

    #[test]
    fn put_and_replace_place_the_path() {
        let e = [
            json!({"remove": "a"}),
            json!({"set_attr": {"path": "b", "attr": "s", "value": "v"}}),
        ];
        let put = [
            json!({"put": {"path": "a"}}),
            json!({"replace": {"path": "b"}, "when": {"flag": "x"}}),
        ];
        assert!(run(&e, &put, true).is_empty());
    }

    #[test]
    fn templated_put_places_any_matching_path() {
        let e = [json!({"remove": "item-x"}), json!({"remove": "other-x"})];
        let put = [json!({"put": {"path": "item-${n}"}})];
        let d = run(&e, &put, true);
        assert_eq!(d.len(), 1);
        assert!(d[0].message().contains("other-x"));
    }

    #[test]
    fn templated_target_is_deferred_until_expansion() {
        let e = [json!({"remove": "item-${n}"})];
        assert!(run(&e, &[], true).is_empty());
        assert_eq!(run(&e, &[], false).len(), 1);
    }

    #[test]
    fn unsafe_path_is_not_echoed_and_bad_items_are_skipped() {
        let e = [
            json!({"remove": "bad path\nwith space"}),
            json!({"remove": 1}),
        ];
        let d = run(&e, &[], true);
        assert_eq!(d.len(), 1);
        assert!(d[0].message().contains("(id not shown)"));
        assert!(!d[0].message().contains("bad path"));
    }

    #[test]
    fn put_and_flags_are_not_targets() {
        let e = [json!({"put": {"path": "z"}}), json!({"set_flag": "f"})];
        assert!(run(&e, &[], true).is_empty());
    }
}
