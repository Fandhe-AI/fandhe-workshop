//! 案内レベル（EP-8。PACK-17）の語の読み取りと、水準ごとの本文の選択（TASK-49 1/2）。
//!
//! 呼び出し元は読み込み層 `source::load`（`check_workshop_levels`・`check_scope_levels`）と、静的検査
//! （`E-STRUCTURE`・`E-GUIDANCE-LEAK`・`E-HINT-HIDDEN`。TASK-49 2/2）である。
//! `body_levels`・`do_levels` の構造の拒否（鍵 `A`・空の値・`do` の欠落。`do_levels` は TASK-70 2/2）は `validate`・`compile` の両方、漏れとヒントの検査は `validate` だけが行う。語彙 v1.1 の 5.3 節に従い、
//! 水準の名前が `A`・`B`・`C` でない教材は読み込みで `E-PARSE` にする。読めない水準は IR に載せない
//! （教材データは信頼しない入力のため、失敗は安全側へ倒す）。
//!
//! 探す順は契約 `GuidanceLevel::fallback_chain` をそのまま使い、規則を二重に持たない。ヒントの出し分けは
//! 契約の `visible_hints` が持つ。実行時の本文の木の選択は core の `body_level::select_body`（TASK-292）が持ち、同じ版を選ぶことは `tests/body_level_runtime.rs` で確かめる。
//! 診断の文面は固定の英語で、書かれた値・`hide_words`・本文は写さない。

use std::collections::{BTreeMap, BTreeSet};

use fandhe_workshop_contract::guidance::{visible_hints, GuidanceLevel};
use fandhe_workshop_contract::ir::{Ir, Step};
use fandhe_workshop_core::command_class::CommandKind;
use fandhe_workshop_core::text_fill::step_do_at;
use serde_json::Value;

use crate::diag::{DiagCode, Diagnostic};
use crate::source::{safe_ident, LoadedPack, LoadedScope, ScopeKind, ScopeSrc, WorkshopSrc};
use crate::validate::diag;

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
/// 水準 A の本文は `body` に書く。`body_levels` の鍵 `A` は本文として使わない（`check_body_levels_a` が `E-STRUCTURE` にする）。
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
/// 鍵 `A` はここでは通す（`E-STRUCTURE` にするのは `check_body_levels_a`。`validate`・`compile` の段）。
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
        if st
            .do_levels
            .keys()
            .any(|k| GuidanceLevel::parse(k).is_none())
        {
            out.push(parse_diag(
                file,
                "scope",
                "do_levels keys must be A, B or C",
            ));
        }
    }
    out
}

/// `body_levels` の鍵 `A` を `E-STRUCTURE` にする。水準 A の本文は `body` に書くため、鍵 `A` は
/// 使われないまま残り、書き手の意図と実行時の本文が食い違う。`validate` と `compile` の両方から呼ぶ
/// （IR に `body_levels.A` を載せない。fail-closed）。
pub(crate) fn check_body_levels_a(scopes: &[LoadedScope]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for scope in scopes {
        for st in &scope.src.steps {
            if st.body_levels.contains_key("A") {
                out.push(diag(
                    &scope.file,
                    &format!("step:{}", st.id),
                    DiagCode::Structure,
                    "body_levels must not have key A (write the level A body in body)",
                ));
            }
        }
    }
    out
}

/// `do_levels` の構造を `E-STRUCTURE` にする（語彙 v1.6。PACK-50。TASK-70 2/2）。`validate` と `compile` の両方から呼ぶ。
///
/// - 鍵 `A`: 水準 A の文は `do` に書くため、鍵 `A` は使われないまま残る
/// - 空・空白だけの値: B でだけ次の行動が崩れる（黙って A の文へ倒れず、書き手の誤りとして止める）
/// - `do` が無い・空白だけ: 探す順が必ず A の `do` に行き着くことを前提にしているため
///
/// 鍵が A・B・C でないものは読み込みの `E-PARSE`（`check_scope_levels`）の担当で、ここでは見ない。
/// 診断は固定の英語で、書かれた値を写さない。
pub(crate) fn check_do_levels(scopes: &[LoadedScope]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for scope in scopes {
        for st in &scope.src.steps {
            if st.do_levels.is_empty() {
                continue;
            }
            let item = format!("step:{}", st.id);
            if st.do_levels.contains_key("A") {
                out.push(diag(
                    &scope.file,
                    &item,
                    DiagCode::Structure,
                    "do_levels must not have key A (write the level A text in do)",
                ));
            }
            if st.do_levels.values().any(|v| v.trim().is_empty()) {
                out.push(diag(
                    &scope.file,
                    &item,
                    DiagCode::Structure,
                    "do_levels values must not be empty",
                ));
            }
            if st.do_.as_deref().is_none_or(|d| d.trim().is_empty()) {
                out.push(diag(
                    &scope.file,
                    &item,
                    DiagCode::Structure,
                    "do_levels requires do",
                ));
            }
        }
    }
    out
}

/// 展開の後（モジュールの引数の置換の後）の node で、`do_levels` の空の値と `do` の欠落を `E-STRUCTURE` にする
/// （PACK-50。`step_do` を使う文面の有無に依らない）。`check_do_levels` は置換の前の Step しか見ないため、
/// 引数が空になって初めて違反する組をここで捕まえる。置換の前から同じ違反がある Step は `check_do_levels` が
/// 報告するので、ここでは重ねない。診断は Step を定義した章・モジュールの定義ファイルに付け、書かれた値を写さない。
pub(crate) fn check_do_levels_expanded(
    pack: &LoadedPack,
    nodes: &[fandhe_workshop_contract::ir::Node],
) -> Vec<Diagnostic> {
    let blank = |st: &Step| st.do_levels.values().any(|v| v.trim().is_empty());
    let no_do = |st: &Step| st.do_.as_deref().is_none_or(|d| d.trim().is_empty());
    let mut out = Vec::new();
    for node in nodes {
        let st = &node.step;
        if st.do_levels.is_empty() {
            continue;
        }
        let Some(scope) = pack.scopes.iter().find(|s| s.src.id == node.unit) else {
            continue;
        };
        let orig = scope.src.steps.iter().find(|x| x.id == st.id);
        let item = format!("step:{}", st.id);
        if blank(st) && !orig.is_some_and(|o| !o.do_levels.is_empty() && blank(o)) {
            out.push(diag(
                &scope.file,
                &item,
                DiagCode::Structure,
                "do_levels values must not be empty",
            ));
        }
        if no_do(st) && !orig.is_some_and(|o| !o.do_levels.is_empty() && no_do(o)) {
            out.push(diag(
                &scope.file,
                &item,
                DiagCode::Structure,
                "do_levels requires do",
            ));
        }
    }
    out
}

/// `check_do_levels_expanded` が出す診断の文言か（`validate` が展開の後の診断を選ぶのに使う）。
pub(crate) fn is_do_levels_expanded_message(m: &str) -> bool {
    m == "do_levels values must not be empty" || m == "do_levels requires do"
}

/// 水準ごとの隠す語の列。`workshop.commands` のうち種類が `hide_command_kinds` に含まれるコマンドの
/// 名前と、`hide_words`。空・空白だけの語は、どの文字列にも含まれて全件を誤検出するため飛ばす。
fn hidden_words(w: &WorkshopSrc, rule: &GuideRule) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for (name, def) in &w.commands {
        if CommandKind::from_value(def).is_some_and(|k| rule.hide_command_kinds.contains(&k)) {
            words.push(name.clone());
        }
    }
    words.extend(rule.hide_words.iter().cloned());
    words.retain(|x| !x.trim().is_empty());
    words
}

/// `use.module` が指すモジュールの ID。文字列でない・引数（`${`）を含むものは `None`。
fn used_module(step: &Step) -> Option<&str> {
    let m = step.use_module.as_ref()?.get("module")?.as_str()?;
    (!m.contains("${")).then_some(m)
}

/// `scope` の Step が入れ子を含めて呼ぶモジュールの Step を、`out` に (スコープの添字, Step の添字) で足す。
/// 巡回は訪問済みの集合で止める（教材データは信頼しない入力）。
fn collect_module_steps(
    pack: &LoadedPack,
    scope: &LoadedScope,
    visited: &mut BTreeSet<String>,
    out: &mut Vec<(usize, usize)>,
) {
    for st in &scope.src.steps {
        let Some(m) = used_module(st) else {
            continue;
        };
        if !visited.insert(m.to_string()) {
            continue;
        }
        for (i, ms) in pack.scopes.iter().enumerate() {
            if ms.kind == ScopeKind::Module && ms.src.id == m {
                out.extend((0..ms.src.steps.len()).map(|j| (i, j)));
                collect_module_steps(pack, ms, visited, out);
            }
        }
    }
}

/// 水準 B・C ごとの隠す語の表（語が 1 つも無い水準は除く）。`check_guidance_leak` と、展開器が置換後の `do` を
/// 調べる側（`expand::Expander::check_guidance_leak`）が共有する。読めない `guidance_levels` は空（別に `E-PARSE`）。
pub(crate) fn leak_rules(pack: &LoadedPack) -> BTreeMap<GuidanceLevel, Vec<String>> {
    let Ok(rules) = read_guidance_levels(pack.workshop.pack.guidance_levels.as_ref()) else {
        return BTreeMap::new();
    };
    rules
        .iter()
        .filter(|(l, _)| **l != GuidanceLevel::A)
        .map(|(l, r)| (*l, hidden_words(&pack.workshop, r)))
        .filter(|(_, w)| !w.is_empty())
        .collect()
}

/// 章の開始点の `guidance` が B・C である水準の集合。その章の全 Step と、呼ぶモジュールの全 Step が
/// この水準で検査される対象になる（PACK-17・GUIDE-5）。
pub(crate) fn chapter_start_levels(scope: &LoadedScope) -> BTreeSet<GuidanceLevel> {
    scope
        .src
        .starts
        .iter()
        .filter_map(|s| {
            s.guidance
                .as_ref()
                .and_then(Value::as_str)
                .and_then(GuidanceLevel::parse)
        })
        .filter(|l| *l != GuidanceLevel::A)
        .collect()
}

/// モジュールの引数を置換した後の `do`・`do_levels` で、隠す語が出る水準の列（PACK-52・D-43）。
/// `before` は置換の前の Step、`after` は置換の後の Step、`levels` は調べる水準。置換の前の字面にすでに
/// 語がある水準は、定義の段の `check_guidance_leak` が報告するので重ねない。ただし重ねないのは、その Step・水準が
/// 実際に定義の段の対象（`covered`）だった場合に限る。引数つきのモジュール名で呼ばれた Step は定義の段が
/// 辿らないので、置換の前に語があっても置換の後の診断を抑えない。
pub(crate) fn expanded_do_leak_levels(
    rules: &BTreeMap<GuidanceLevel, Vec<String>>,
    levels: &BTreeSet<GuidanceLevel>,
    before: &Step,
    after: &Step,
    covered: &dyn Fn(GuidanceLevel) -> bool,
) -> Vec<GuidanceLevel> {
    let leaks = |st: &Step, l: GuidanceLevel, words: &[String]| {
        step_do_at(st, l).is_some_and(|d| words.iter().any(|w| d.contains(w.as_str())))
    };
    levels
        .iter()
        .filter_map(|l| {
            let words = rules.get(l)?;
            (!(covered(*l) && leaks(before, *l, words)) && leaks(after, *l, words)).then_some(*l)
        })
        .collect()
}

/// 置換後の `do` の漏れの診断（語・値・本文は写さない）。
pub(crate) fn expanded_do_leak_diag(file: &str, item: &str, level: GuidanceLevel) -> Diagnostic {
    diag(
        file,
        item,
        DiagCode::GuidanceLeak,
        format!(
            "a word hidden at guidance level {} appears in do after argument substitution",
            level.as_str()
        ),
    )
}

/// 定義の段の検査の対象 (スコープの添字, Step の添字, 水準)。前者は本文と `do` の両方を見る組、後者は
/// `do_levels` だけを持つ Step の B・C（本文は宣言が無いので見ず、`do` だけを見る）。
/// 引数（`${`）を含むモジュール名の呼び出しは `collect_module_steps` が辿らないため、その先の Step は
/// 開始点の水準では入らない。
type LeakTargets = BTreeSet<(usize, usize, GuidanceLevel)>;

pub(crate) fn definition_leak_targets(pack: &LoadedPack) -> (LeakTargets, LeakTargets) {
    let mut targets: BTreeSet<(usize, usize, GuidanceLevel)> = BTreeSet::new();
    let mut do_targets: BTreeSet<(usize, usize, GuidanceLevel)> = BTreeSet::new();
    for (i, scope) in pack.scopes.iter().enumerate() {
        for (j, st) in scope.src.steps.iter().enumerate() {
            if !st.body_levels.is_empty() {
                for l in [GuidanceLevel::B, GuidanceLevel::C] {
                    targets.insert((i, j, l));
                }
            }
            if !st.do_levels.is_empty() {
                for l in [GuidanceLevel::B, GuidanceLevel::C] {
                    do_targets.insert((i, j, l));
                }
            }
        }
        if scope.kind != ScopeKind::Chapter {
            continue;
        }
        let start_levels = chapter_start_levels(scope);
        if start_levels.is_empty() {
            continue;
        }
        let mut steps: Vec<(usize, usize)> = (0..scope.src.steps.len()).map(|j| (i, j)).collect();
        let mut visited = BTreeSet::new();
        collect_module_steps(pack, scope, &mut visited, &mut steps);
        for (si, sj) in steps {
            for l in &start_levels {
                targets.insert((si, sj, *l));
            }
        }
    }
    (targets, do_targets)
}

/// 隠すと宣言した語が、その水準で出る本文または `do` に含まれていれば `E-GUIDANCE-LEAK`（`validate` だけ）。
///
/// 対象は、(i) `body_levels` を持つ Step の水準 B・C（本文と `do`）と、`do_levels` を持つ Step の水準 B・C（`do` だけ）と、(ii) 開始点の `guidance` が B・C の章の全 Step と
/// その章が呼ぶモジュールの全 Step の同じ水準。単純な部分文字列の包含で判定する近似の検査で、
/// ここは置換の前（定義の段）の文字列を見る。実体の引数で渡す `do`・`do_levels` の置換後は、展開器が
/// 同じ対象で調べる（PACK-52。TASK-71 3/3。`validate::check_module_expansion`）。ヒントの文面（段 2・3）と
/// 開始点の `guidance` も `body_levels` も持たない章の本文は見ない（D-43 で作成者の規約として受け入れた）。
/// ディスクには触れず `pack.bodies` だけを読む。診断は語・本文を写さない。
pub(crate) fn check_guidance_leak(pack: &LoadedPack) -> Vec<Diagnostic> {
    let rules = leak_rules(pack);
    if rules.is_empty() {
        return Vec::new();
    }
    let (targets, do_targets) = definition_leak_targets(pack);
    let mut out = Vec::new();
    let all: BTreeSet<_> = targets.iter().chain(do_targets.iter()).copied().collect();
    for (i, j, level) in all {
        let in_body_scope = targets.contains(&(i, j, level));
        let Some(words) = rules.get(&level) else {
            continue;
        };
        let scope = &pack.scopes[i];
        let step = &scope.src.steps[j];
        let item = format!("step:{}", step.id);
        let in_body = in_body_scope
            && body_for(step, level)
                .and_then(|p| pack.bodies.get(&format!("{}/{p}", scope.dir)))
                .is_some_and(|text| words.iter().any(|w| text.contains(w.as_str())));
        if in_body {
            out.push(diag(
                &scope.file,
                &item,
                DiagCode::GuidanceLeak,
                format!(
                    "a word hidden at guidance level {} appears in the body",
                    level.as_str()
                ),
            ));
        }
        let in_do =
            step_do_at(step, level).is_some_and(|d| words.iter().any(|w| d.contains(w.as_str())));
        if in_do {
            out.push(diag(
                &scope.file,
                &item,
                DiagCode::GuidanceLeak,
                format!(
                    "a word hidden at guidance level {} appears in do",
                    level.as_str()
                ),
            ));
        }
    }
    out
}

/// ヒントを持つ Step で、ある水準ではヒントが 1 つも出なければ `E-HINT-HIDDEN`（`validate` だけ）。
/// 出し分けの規則は契約 `visible_hints` をそのまま使う。未定義のヒント ID は `E-REF-HINT` の担当で、
/// 定義のあるヒントを 1 つも持たない Step は対象にしない。
pub(crate) fn check_hint_hidden(pack: &LoadedPack) -> Vec<Diagnostic> {
    let w = &pack.workshop;
    let view = Ir {
        format_version: w.pack.ir_format,
        pack: w.pack.clone(),
        refs: Vec::new(),
        commands: BTreeMap::new(),
        views: BTreeMap::new(),
        fixtures: BTreeMap::new(),
        hints: w.hints.clone(),
        rejections: BTreeMap::new(),
        clarifications: BTreeMap::new(),
        defaults: BTreeMap::new(),
        instances: Vec::new(),
        nodes: Vec::new(),
        starts: Vec::new(),
        fixture_docs: BTreeMap::new(),
    };
    let mut out = Vec::new();
    for scope in &pack.scopes {
        for st in &scope.src.steps {
            if !st.hints.iter().any(|h| w.hints.contains_key(h)) {
                continue;
            }
            for level in GuidanceLevel::ALL {
                if visible_hints(&view, st, level).is_empty() {
                    out.push(diag(
                        &scope.file,
                        &format!("step:{}", st.id),
                        DiagCode::HintHidden,
                        format!("no hint is shown at guidance level {}", level.as_str()),
                    ));
                }
            }
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
        let ok = json!({"B": {"hide_command_kinds": ["builtin", "flow"], "hide_words": ["語 1"]}, "A": {}});
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
