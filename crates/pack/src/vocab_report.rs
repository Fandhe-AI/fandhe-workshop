//! `fandhe-pack vocab` の数え上げ（TASK-408、AUTHOR-9）。
//!
//! 作成者が、書いた教材が既存の語彙の組み合わせだけで書けたのか、名前つき拡張点に頼ったのかを機械で確かめる。
//! `cli::run_vocab` から呼ばれ、表と集計の文字列を返す。行・集計行の形は `cli_contract`（`docs/cli.md` 版 6 の
//! 12 節）が持ち、拡張点の ID は [`crate::vocab::EXTENSION_POINTS`] から引いて、ここに書き写さない。
//!
//! - 関門: 読み込み（`source::load`）と `compile::compile` のどちらかが診断を返したら、表は出さずその診断を返す
//!   （fail-closed）。`compile` を通ると条件・効果・入力・確認・`pack.profile`・`appearance`・`left_pane`・
//!   `views.body` が読める状態だと保証され、数え上げ側が壊れた値を扱わずに済む。`validate` 専用の検査
//!   （`E-VOCAB-MISMATCH`・`E-REF-ELEMENT`・到達性）は関門に入れない。作成者が `pack.vocab` に何を宣言すべきかを、
//!   `vocab` で確かめられるようにするためである
//! - 数える単位: `pack.scopes` の順（章、続いてモジュール）にある Step の定義。呼ばれないモジュールも含め、
//!   展開後の複製は数えない。`use` を持つ Step は `use` 行として出し、（i）（ii）に入れない
//! - 数える拡張点: EP-1〜EP-9・EP-13（TASK-409、AUTHOR-17、D-64）。EP-9 は `requires` または `unmet` を持つ
//!   開始点が指す Step（`node`）の行に、Step 単位で 1 回数える。`use` の Step を指す場合は `use` 行を空に保つため
//!   数えない。`confirm.options[].label` は拡張点の番号を持たないため数えない。EP-7 は `do` だけ、EP-8 は Step の `body_levels` とヒントの `levels` だけを数え、教材パック全体の
//!   `guidance_levels`・開始点の `guidance` は数えない（試作と同じ）
//! - 教材パック全体の EP-10〜EP-12: 読んだ値が既定と違うときだけ末尾に出し、Step ごとの数（i）（ii）に入れない。
//!   欄の有無ではなく値で判定する（`vocab_check` の「欄がある」とは規則が違う）
//!
//! 出力に写すのは ID・EP の ID・固定の欄名だけで、本文・練習データ・条件の値は写さない。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fandhe_workshop_contract::appearance::{read_appearance, Theme};
use fandhe_workshop_contract::input_spec::InputKind;
use fandhe_workshop_contract::ir::Step;
use fandhe_workshop_contract::limits::MAX_JSON_DEPTH;
use fandhe_workshop_contract::profile::ProfileSetting;
use fandhe_workshop_contract::view_body::{read_view_body, ViewBody};
use serde_json::Value;

use crate::cli_contract::{
    vocab_pack_line, vocab_row, vocab_summary_lines, VocabRowKind, VOCAB_HEADER,
};
use crate::diag::Diagnostic;
use crate::source::{self, LoadedPack};
use crate::vocab::EXTENSION_POINTS;
use crate::vocab_check::start_uses_ep9;

/// 読み込み・`compile` の関門を通してから、表と集計を作る。診断があれば表は作らずに返す。
pub fn report_dir(dir: &Path) -> Result<String, Vec<Diagnostic>> {
    let pack = source::load(dir)?;
    // IR は使わない。診断の有無だけを関門にする。
    crate::compile::compile(&pack)?;
    report(&pack)
}

/// 読み込み済みのパックから表と集計を作る。教材パック全体の欄が読めないときは、その診断を返す。
pub fn report(pack: &LoadedPack) -> Result<String, Vec<Diagnostic>> {
    let mut out = String::new();
    out.push_str(VOCAB_HEADER);
    out.push('\n');
    let (mut plain, mut with_ep) = (0usize, 0usize);
    let mut per_ep: BTreeMap<&'static str, usize> = BTreeMap::new();
    for scope in &pack.scopes {
        // EP-9 は開始点が指す Step の行に数える（先頭の Step ではない）。
        let started: BTreeSet<&str> = scope
            .src
            .starts
            .iter()
            .filter(|st| start_uses_ep9(st))
            .map(|st| st.node.as_str())
            .collect();
        for step in &scope.src.steps {
            if step.use_module.is_some() {
                out.push_str(&vocab_row(&scope.src.id, &step.id, VocabRowKind::Use, &[]));
                out.push('\n');
                continue;
            }
            let eps = eps_of(
                step,
                &pack.workshop.views,
                &pack.workshop.hints,
                started.contains(step.id.as_str()),
            );
            if eps.is_empty() {
                plain += 1;
            } else {
                with_ep += 1;
            }
            for id in &eps {
                *per_ep.entry(id).or_default() += 1;
            }
            out.push_str(&vocab_row(
                &scope.src.id,
                &step.id,
                VocabRowKind::Step,
                &eps,
            ));
            out.push('\n');
        }
    }
    // 並びは EP の番号順（文字列順では EP-13 が EP-2 より前に来る）。
    let counts: Vec<(&str, usize)> = EXTENSION_POINTS
        .iter()
        .filter_map(|e| per_ep.get(e.id).map(|n| (e.id, *n)))
        .collect();
    for line in vocab_summary_lines(plain, with_ep, &counts) {
        out.push_str(&line);
        out.push('\n');
    }
    for line in pack_wide_lines(pack)? {
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}

/// 教材パック全体の EP-10〜EP-12 の行。既定から変えた欄だけを EP の番号順に返す。
fn pack_wide_lines(pack: &LoadedPack) -> Result<Vec<String>, Vec<Diagnostic>> {
    let w = &pack.workshop;
    let file = &pack.workshop_file;
    let mut lines = Vec::new();
    let (profile, d) = crate::profile::check_profile(file, w);
    if !d.is_empty() {
        return Err(d);
    }
    if profile != ProfileSetting::default() {
        lines.push(vocab_pack_line(ep_id("pack.profile"), "pack.profile"));
    }
    let d = crate::appearance::check_appearance(file, w);
    if !d.is_empty() {
        return Err(d);
    }
    if read_appearance(w.pack.appearance.as_ref()).is_ok_and(|t| t != Theme::default()) {
        lines.push(vocab_pack_line(ep_id("pack.appearance"), "pack.appearance"));
    }
    let (left, d) = crate::left_pane::check_left_pane(file, w);
    if !d.is_empty() {
        return Err(d);
    }
    if !left.is_default() {
        lines.push(vocab_pack_line(ep_id("pack.left_pane"), "pack.left_pane"));
    }
    Ok(lines)
}

/// 拡張点の名前から ID を引く。表に無い名前は出さない（単体試験で全項目が引けることを保証する）。
fn ep_id(name: &str) -> &'static str {
    EXTENSION_POINTS
        .iter()
        .find(|e| e.name == name)
        .map_or("", |e| e.id)
}

/// 1 つの Step が使った拡張点の ID を、`EXTENSION_POINTS` の順で返す（EP-1〜EP-9・EP-13）。
///
/// `views`・`hints` は `workshop.views`・`workshop.hints`。`started` は、この Step を指す開始点が EP-9 を使うか。`use` の Step は呼び出し側が除くため、ここでは見ない。
pub(crate) fn eps_of(
    step: &Step,
    views: &BTreeMap<String, Value>,
    hints: &BTreeMap<String, Value>,
    started: bool,
) -> Vec<&'static str> {
    let mut used: Vec<&str> = Vec::new();
    if started {
        used.push("starts.requires");
    }
    if step
        .input
        .as_ref()
        .and_then(|i| i.get("kind"))
        .and_then(Value::as_str)
        .and_then(InputKind::from_str_opt)
        == Some(InputKind::Wait)
    {
        used.push("input.wait");
    }
    if step.rules.iter().any(|r| {
        r.get("on")
            .and_then(|o| o.get("cancel"))
            .and_then(Value::as_bool)
            == Some(true)
    }) {
        used.push("input.cancel");
    }
    let words = condition_words(step);
    if words.param {
        used.push("condition.param");
    }
    if words.clarifying {
        used.push("condition.clarifying");
    }
    let declared: Vec<&Value> = step.views.iter().filter_map(|id| views.get(id)).collect();
    if declared
        .iter()
        .any(|v| v.get("kind").and_then(Value::as_str) == Some("card_list"))
    {
        used.push("view.card_list");
    }
    if uses_option_on_wrong(step) {
        used.push("confirm.answer.option_on_wrong");
    }
    if step.do_.is_some() {
        used.push("text.slot");
    }
    if !step.body_levels.is_empty() || step.hints.iter().any(|id| hint_has_levels(hints.get(id))) {
        used.push("guidance.variant");
    }
    if declared
        .iter()
        .any(|v| read_view_body(v) == Ok(ViewBody::Shown))
    {
        used.push("view.document_body");
    }
    EXTENSION_POINTS
        .iter()
        .filter(|e| used.contains(&e.name))
        .map(|e| e.id)
        .collect()
}

/// `confirm.kind` が `answer` で、選択肢のどれかが `null` でない `on_wrong` を持つか（EP-6）。
fn uses_option_on_wrong(step: &Step) -> bool {
    let Some(c) = step.confirm.as_ref() else {
        return false;
    };
    c.get("kind").and_then(Value::as_str) == Some("answer")
        && c.get("options")
            .and_then(Value::as_array)
            .is_some_and(|opts| {
                opts.iter()
                    .any(|o| o.get("on_wrong").is_some_and(|w| !w.is_null()))
            })
}

/// ヒントの定義が空でない `levels` を持つか（EP-8）。
fn hint_has_levels(def: Option<&Value>) -> bool {
    def.and_then(|d| d.get("levels"))
        .and_then(Value::as_array)
        .is_some_and(|l| !l.is_empty())
}

/// 条件の中に現れた語（EP-3・EP-4 に関わるもの）。
#[derive(Default)]
struct ConditionWords {
    param: bool,
    clarifying: bool,
}

/// Step の条件の欄（`next[].when`・規則の `when`・効果の `when`）を走査して、`param`・`clarifying` の有無を返す。
fn condition_words(step: &Step) -> ConditionWords {
    let mut w = ConditionWords::default();
    for t in &step.next {
        if let Some(c) = &t.when {
            walk(c, 0, &mut w);
        }
    }
    for r in &step.rules {
        if let Some(c) = r.get("when") {
            walk(c, 0, &mut w);
        }
        if let Some(effects) = r
            .get("outcome")
            .and_then(|o| o.get("effects"))
            .and_then(Value::as_array)
        {
            walk_effects(effects, &mut w);
        }
    }
    for list in [&step.on_enter, &step.on_achieved] {
        walk_effects(list, &mut w);
    }
    // `processing` の要素は `name` と `effects` を持つ処理単位で、条件は `effects` の各効果の `when` にある。
    for p in &step.processing {
        if let Some(effects) = p.get("effects").and_then(Value::as_array) {
            walk_effects(effects, &mut w);
        }
    }
    w
}

fn walk_effects(effects: &[Value], w: &mut ConditionWords) {
    for e in effects {
        if let Some(c) = e.get("when") {
            walk(c, 0, w);
        }
    }
}

/// 条件を `all`・`any`・`not` の入れ子でたどる。深さは `MAX_JSON_DEPTH` で止める（`compile` を通った値は
/// 超えないが、数え上げ側でも再帰を有限に保つ）。
fn walk(v: &Value, depth: usize, w: &mut ConditionWords) {
    if depth > MAX_JSON_DEPTH {
        return;
    }
    let Some(obj) = v.as_object() else {
        return;
    };
    for (k, val) in obj {
        match k.as_str() {
            "param" => w.param = true,
            "clarifying" => w.clarifying = true,
            "all" | "any" => {
                if let Some(items) = val.as_array() {
                    for item in items {
                        walk(item, depth + 1, w);
                    }
                }
            }
            "not" => walk(val, depth + 1, w),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn step(v: Value) -> Step {
        let mut base = json!({"id": "s", "title": "S"});
        base.as_object_mut()
            .unwrap()
            .extend(v.as_object().unwrap().clone());
        serde_json::from_value(base).unwrap()
    }

    fn eps(v: Value) -> Vec<&'static str> {
        let views: BTreeMap<String, Value> = [
            ("v-cards".to_string(), json!({"kind": "card_list"})),
            (
                "v-doc".to_string(),
                json!({"kind": "document", "body": "shown"}),
            ),
            (
                "v-doc-hidden".to_string(),
                json!({"kind": "document", "body": "hidden"}),
            ),
        ]
        .into();
        let hints: BTreeMap<String, Value> = [
            ("h-plain".to_string(), json!({"text": "t"})),
            ("h-empty".to_string(), json!({"levels": []})),
            ("h-lv".to_string(), json!({"levels": ["B"]})),
        ]
        .into();
        eps_of(&step(v), &views, &hints, false)
    }

    #[test]
    fn e1_plain_step_has_no_extension_point() {
        assert!(eps(json!({})).is_empty());
        assert!(eps(json!({"input": {"kind": "approval"}})).is_empty());
    }

    #[test]
    fn e2_each_extension_point_is_detected() {
        assert_eq!(eps(json!({"input": {"kind": "wait"}})), ["EP-1"]);
        assert_eq!(
            eps(json!({"rules": [{"id": "r", "on": {"cancel": true}}]})),
            ["EP-2"]
        );
        assert_eq!(
            eps(
                json!({"next": [{"to": "x", "when": {"all": [{"flag": "a"}, {"not": {"param": {"name": "n", "eq": "v"}}}]}}]})
            ),
            ["EP-3"]
        );
        assert_eq!(
            eps(
                json!({"rules": [{"id": "r", "outcome": {"effects": [{"set_flag": "a", "when": {"any": [{"clarifying": "c"}]}}]}}]})
            ),
            ["EP-4"]
        );
        assert_eq!(eps(json!({"views": ["v-cards"]})), ["EP-5"]);
        assert_eq!(
            eps(json!({"confirm": {"kind": "answer", "options": [{"id": "a", "on_wrong": "rj"}]}})),
            ["EP-6"]
        );
        assert_eq!(eps(json!({"do": "x"})), ["EP-7"]);
        assert_eq!(eps(json!({"body_levels": {"B": "b.md"}})), ["EP-8"]);
        assert_eq!(eps(json!({"hints": ["h-lv"]})), ["EP-8"]);
        assert_eq!(eps(json!({"views": ["v-doc"]})), ["EP-13"]);
        let got = eps_of(&step(json!({})), &BTreeMap::new(), &BTreeMap::new(), true);
        assert_eq!(got, ["EP-9"]);
    }

    #[test]
    fn e3_unused_forms_are_not_counted() {
        // `null` の on_wrong・answer 以外の確認・levels の無い（空の）ヒント・本文を出さないビューは数えない。
        assert!(eps(
            json!({"confirm": {"kind": "answer", "options": [{"id": "a", "on_wrong": null}]}})
        )
        .is_empty());
        assert!(eps(
            json!({"confirm": {"kind": "approve", "options": [{"id": "a", "on_wrong": "rj"}]}})
        )
        .is_empty());
        assert!(eps(json!({"hints": ["h-plain", "h-empty", "h-missing"]})).is_empty());
        assert!(eps(json!({"views": ["v-doc-hidden", "v-undefined"]})).is_empty());
        assert!(eps(json!({"rules": [{"id": "r", "on": {"cancel": false}}]})).is_empty());
        // label は数えない（拡張点の番号を持たない。D-64）。
        assert!(eps(
            json!({"confirm": {"kind": "answer", "options": [{"id": "a", "label": "A"}]}})
        )
        .is_empty());
    }

    #[test]
    fn e4_order_follows_extension_point_table_not_strings() {
        let got = eps(json!({
            "do": "x",
            "input": {"kind": "wait"},
            "views": ["v-doc"],
            "body_levels": {"B": "b.md"},
            "rules": [{"id": "r", "on": {"cancel": true}}],
        }));
        assert_eq!(got, ["EP-1", "EP-2", "EP-7", "EP-8", "EP-13"]);
    }

    #[test]
    fn e5_condition_walk_is_depth_bounded() {
        let mut v = json!({"param": {"name": "n", "eq": "v"}});
        for _ in 0..(MAX_JSON_DEPTH + 10) {
            v = json!({"not": v});
        }
        let mut w = ConditionWords::default();
        walk(&v, 0, &mut w);
        assert!(!w.param, "walk stops at the depth limit");
    }

    #[test]
    fn e6_every_counted_name_resolves_to_an_id() {
        for name in [
            "input.wait",
            "input.cancel",
            "condition.param",
            "condition.clarifying",
            "view.card_list",
            "confirm.answer.option_on_wrong",
            "starts.requires",
            "text.slot",
            "guidance.variant",
            "view.document_body",
            "pack.profile",
            "pack.appearance",
            "pack.left_pane",
        ] {
            assert!(!ep_id(name).is_empty(), "{name}");
        }
    }
}
