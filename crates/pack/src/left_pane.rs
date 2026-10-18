//! `pack.left_pane`（左エリアの設定）の検査と、設定と矛盾する Step の診断（TASK-53 2/2、PACK-22）。
//!
//! `compile` が外観の検査の直後に、`validate` が静的検査の一部として呼ぶ。閉じた集合（`world_list` 2 値・
//! `view_tabs` 3 値）と読み取りの規則の実体は `fandhe_workshop_contract::left_pane` にあり、画面（web）が
//! 同じ型を使う。本モジュールは、その読み取りの失敗を `E-PARSE`（item は `pack:left_pane`）へ写し（[`check_left_pane`]）、
//! 設定を既定から変えたときだけ、次の 3 種を Step ごとに報告する（[`check_left_targets`]）。
//!
//! | コード | 検査する条件 | 報告する場合 |
//! |---|---|---|
//! | `E-LEFT-MULTI-VIEW` | `view_tabs: hidden` | `views` に 2 つ以上のビューを宣言した Step |
//! | `E-LEFT-TARGET-VIEW` | `view_tabs` が `all` 以外 | 宛先のビューが、その Step の画面に出ない |
//! | `E-LEFT-TARGET-ROW` | 設定が既定でない | 宛先の経路が、どの `put`・`replace` の `path` にも現れない |
//!
//! 宛先は 3 か所（`input` の `ui_action`・`confirm` の `point_at` の `targets[]`、`rules[].on.ui`）である。
//! 画面に出るビューは web の `artifact_view::declared_kinds`・`view_select::default_view` に合わせる。宣言があれば
//! 宣言の順に種類ごとの最初の 1 つ（`hidden` なら先頭の 1 つ）、宣言が無ければ `workshop.views` を ID の辞書順に
//! 見た最初の 1 つである（宣言の無い Step の既定は試作の仮置きで、決まった規則は TASK-496 が持つ）。
//!
//! `E-LEFT-TARGET-ROW` は必要条件だけを調べる静的な近似で、「経路がどこかで置かれる」ことしか確かめない。
//! 練習データの種別とビューの種類の対応（その種類で行になるか）は、教材に載らない試作の約束で、ここでは
//! 判定しない。文書の行の ID を宛先にする書き方は TASK-476 が決める。このため、経路は完全一致（`put` 側の
//! `${...}` は空でない任意の文字列）だけで照合する。モジュールの引数（`${`）を含む宛先は展開前に決まらない
//! ため展開の前には判定せず、展開の後の Step（[`check_left_expanded`]）で判定する。
//!
//! 教材データは信頼しない入力として扱う。診断の文面は固定の英語で、ビュー ID・経路は `safe_ident` を通る
//! ときだけ写す。未知の欄の名前も同様である。照合は正規表現を組み立てず、区切りの線形の走査で行う。

use std::collections::BTreeSet;

use fandhe_workshop_contract::ir::{Node, Start, Step};
use fandhe_workshop_contract::left_pane::{read_left_pane, LeftPane, LeftPaneReadError, ViewTabs};
use serde_json::Value;

use crate::diag::{DiagCode, Diagnostic};
use crate::source::{safe_ident, LoadedPack, ScopeKind, WorkshopSrc};
use crate::validate::{diag, elements, get, kind_is, shown};

/// `pack.left_pane` を読んで検査する。読めないときは既定の設定と `E-PARSE` を 1 件返す。
///
/// 呼び出し側は、診断が空のときだけ [`check_left_targets`] を呼ぶ（モードが決まらないため）。
pub fn check_left_pane(workshop_file: &str, w: &WorkshopSrc) -> (LeftPane, Vec<Diagnostic>) {
    let e = match read_left_pane(w.pack.left_pane.as_ref()) {
        Ok(lp) => return (lp, Vec::new()),
        Err(e) => e,
    };
    let message = match e {
        LeftPaneReadError::NotAMapping => "pack.left_pane must be a mapping".to_string(),
        LeftPaneReadError::UnknownKey(k) => {
            if safe_ident(&k) {
                format!("unknown key `{k}` in pack.left_pane")
            } else {
                "unknown key in pack.left_pane".to_string()
            }
        }
        LeftPaneReadError::WorldListNotString => {
            "pack.left_pane.world_list must be a string".to_string()
        }
        LeftPaneReadError::WorldListUnknown => {
            "pack.left_pane.world_list must be one of shown, hidden".to_string()
        }
        LeftPaneReadError::ViewTabsNotString => {
            "pack.left_pane.view_tabs must be a string".to_string()
        }
        LeftPaneReadError::ViewTabsUnknown => {
            "pack.left_pane.view_tabs must be one of all, declared, hidden".to_string()
        }
    };
    (
        LeftPane::default(),
        vec![diag(
            workshop_file,
            "pack:left_pane",
            DiagCode::Parse,
            message,
        )],
    )
}

/// 設定が既定でないとき、全スコープの全 Step を展開の前に検査して `E-LEFT-*` を返す。既定なら空。
///
/// 順は、スコープの順・ファイルの中の Step の順・Step の中では `E-LEFT-MULTI-VIEW`、続いて宛先ごとに
/// `E-LEFT-TARGET-VIEW`・`E-LEFT-TARGET-ROW`（決定的）。同じ宛先が複数あれば同じ診断が重なる。
/// モジュールの引数（`${`）に依存する判定は保留し、展開の後に [`check_left_expanded`] が行う。
pub fn check_left_targets(pack: &LoadedPack, lp: LeftPane) -> Vec<Diagnostic> {
    if lp.is_default() {
        return Vec::new();
    }
    let placed = Placed::collect(pack);
    let mut out = Vec::new();
    for scope in &pack.scopes {
        for step in &scope.src.steps {
            check_step(&mut out, pack, &scope.file, step, lp, &placed, true);
        }
    }
    out
}

/// 展開の後（モジュールの引数の置換の後）の Step で、宛先のビューと経路を検査する。既定の設定なら空。
///
/// 展開の前の検査は、`${` を含む宛先と、`views` に `${` を含む Step の画面を判定できない。引数を置換した
/// 後の Step と、展開した Step・開始点が置く経路で、モジュールの実体だけを検査する（章の Step は置換されず、
/// 展開の前の検査で足りる）。診断は Step を定義したモジュールのファイルに付け、同じ内容は 1 件にする。
pub fn check_left_expanded(
    pack: &LoadedPack,
    lp: LeftPane,
    nodes: &[Node],
    starts: &[Start],
) -> Vec<Diagnostic> {
    if lp.is_default() {
        return Vec::new();
    }
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
        check_step(&mut out, pack, file, &node.step, lp, &placed, false);
    }
    let mut seen = BTreeSet::new();
    out.retain(|d| seen.insert(format!("{d:?}")));
    out
}

fn check_step(
    out: &mut Vec<Diagnostic>,
    pack: &LoadedPack,
    file: &str,
    step: &Step,
    lp: LeftPane,
    placed: &Placed<'_>,
    pre_expansion: bool,
) {
    let item = format!("step:{}", step.id);
    if pre_expansion && lp.view_tabs == ViewTabs::Hidden && step.views.len() >= 2 {
        out.push(diag(
            file,
            &item,
            DiagCode::LeftMultiView,
            format!(
                "view_tabs is hidden but the step declares {} views",
                step.views.len()
            ),
        ));
    }
    // `views` が引数に依存するうちは、画面に出るビューが決まらない（展開の後に検査する）。
    let views_pending = pre_expansion && step.views.iter().any(|v| v.contains("${"));
    let on_screen = (lp.view_tabs != ViewTabs::All && !views_pending)
        .then(|| shown_views(&pack.workshop, step, lp.view_tabs));
    for (view, path) in step_targets(step) {
        // モジュールの引数は展開の前に決まらず、未定義のビューは `E-REF-VIEW` が報告する。
        if view.contains("${") || path.contains("${") || !pack.workshop.views.contains_key(view) {
            continue;
        }
        if let Some(on_screen) = &on_screen {
            if !on_screen.contains(view) {
                out.push(diag(
                    file,
                    &item,
                    DiagCode::LeftTargetView,
                    format!(
                        "target view {} is not shown on this step (view_tabs: {})",
                        shown(view),
                        lp.view_tabs.as_str()
                    ),
                ));
            }
        }
        if !placed.contains(path) {
            out.push(diag(
                file,
                &item,
                DiagCode::LeftTargetRow,
                format!(
                    "target path {} is not placed by any put or replace",
                    shown(path)
                ),
            ));
        }
    }
}

/// その Step の画面に出るビューの ID。`views` のうち `workshop.views` で解決でき、`kind` が文字列のものを
/// 宣言の順に見る。`hidden` なら先頭の 1 つ、`declared` なら `kind` ごとの最初の 1 つ。解決できる宣言が
/// 無いときは `workshop.views` を ID の辞書順に見た最初の 1 つ（TASK-496 で決まるまでの仮置き）。
fn shown_views<'a>(w: &'a WorkshopSrc, step: &'a Step, tabs: ViewTabs) -> BTreeSet<&'a str> {
    let mut kinds: Vec<&str> = Vec::new();
    let mut ids: Vec<&str> = Vec::new();
    for id in &step.views {
        let Some(kind) = w
            .views
            .get(id)
            .and_then(|v| get(v, "kind"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        if !kinds.contains(&kind) {
            kinds.push(kind);
            ids.push(id.as_str());
        }
    }
    if ids.is_empty() {
        return w
            .views
            .keys()
            .next()
            .map(String::as_str)
            .into_iter()
            .collect();
    }
    if tabs == ViewTabs::Hidden {
        ids.truncate(1);
    }
    ids.into_iter().collect()
}

/// Step の宛先（`view`, `path`）を集める。`input`（`ui_action`）・`confirm`（`point_at`）・`rules[].on.ui`
/// の順。どちらかが文字列でない宛先は飛ばす（構造の検査の持ち分）。
fn step_targets(step: &Step) -> Vec<(&str, &str)> {
    fn push<'a>(out: &mut Vec<(&'a str, &'a str)>, t: &'a Value) {
        let view = get(t, "view").and_then(Value::as_str);
        let path = get(t, "path").and_then(Value::as_str);
        if let (Some(v), Some(p)) = (view, path) {
            out.push((v, p));
        }
    }
    let mut out = Vec::new();
    for (holder, kind) in [(&step.input, "ui_action"), (&step.confirm, "point_at")] {
        if let Some(h) = holder.as_ref().filter(|h| kind_is(h, kind)) {
            for t in elements(get(h, "targets")) {
                push(&mut out, t);
            }
        }
    }
    for rule in &step.rules {
        if let Some(ui) = get(rule, "on").and_then(|on| get(on, "ui")) {
            push(&mut out, ui);
        }
    }
    out
}

/// 教材のどこかの `put`・`replace` が置く経路。`${` を含む経路は、空でない任意の文字列を含む形として別に持つ。
struct Placed<'a> {
    exact: BTreeSet<&'a str>,
    patterns: Vec<&'a str>,
}

impl<'a> Placed<'a> {
    /// 全スコープの Step（`on_enter`・`on_achieved`・`processing[].effects`・`rules[].outcome.effects`）と
    /// `starts[].effects` の `put`・`replace` から、文字列の `path` を集める。語彙の誤りは別の検査が報告する。
    fn empty() -> Self {
        Placed {
            exact: BTreeSet::new(),
            patterns: Vec::new(),
        }
    }

    fn collect(pack: &'a LoadedPack) -> Self {
        let mut p = Placed::empty();
        for scope in &pack.scopes {
            for start in &scope.src.starts {
                p.add_effects(&start.effects);
            }
            for step in &scope.src.steps {
                p.add_step(step);
            }
        }
        p
    }

    fn add_step(&mut self, step: &'a Step) {
        self.add_effects(&step.on_enter);
        self.add_effects(&step.on_achieved);
        for proc in &step.processing {
            self.add_effects(elements(get(proc, "effects")));
        }
        for rule in &step.rules {
            if let Some(outcome) = get(rule, "outcome") {
                self.add_effects(elements(get(outcome, "effects")));
            }
        }
    }

    fn add_effects(&mut self, effects: &'a [Value]) {
        for e in effects {
            for op in ["put", "replace"] {
                let path = get(e, op)
                    .and_then(|body| get(body, "path"))
                    .and_then(Value::as_str);
                if let Some(path) = path {
                    if path.contains("${") {
                        self.patterns.push(path);
                    } else {
                        self.exact.insert(path);
                    }
                }
            }
        }
    }

    fn contains(&self, target: &str) -> bool {
        self.exact.contains(target) || self.patterns.iter().any(|p| matches_pattern(p, target))
    }
}

/// `${...}` を空でない任意の文字列とみなした照合。`${` が無ければ完全一致。
///
/// 正規表現を組み立てず、リテラルの断片を左から順に探す線形の走査にする（教材データ由来の文字列で
/// 計算量が膨らまないため）。閉じ `}` が無い `${` 以降はリテラルとして扱う。
fn matches_pattern(pattern: &str, target: &str) -> bool {
    let mut literals: Vec<&str> = Vec::new();
    let mut rest = pattern;
    while let Some(start) = rest.find("${") {
        let Some(len) = rest[start + 2..].find('}') else {
            break;
        };
        literals.push(&rest[..start]);
        rest = &rest[start + 2 + len + 1..];
    }
    literals.push(rest);
    let last = literals.len() - 1;
    if last == 0 {
        return pattern == target;
    }
    let Some(mut remain) = target.strip_prefix(literals[0]) else {
        return false;
    };
    for lit in &literals[1..last] {
        // 変数の部分は 1 文字以上。先頭の 1 文字を飛ばしてから断片を探す。
        let Some(first) = remain.chars().next() else {
            return false;
        };
        let after = &remain[first.len_utf8()..];
        let Some(pos) = after.find(lit) else {
            return false;
        };
        remain = &after[pos + lit.len()..];
    }
    let Some(first) = remain.chars().next() else {
        return false;
    };
    remain[first.len_utf8()..].ends_with(literals[last])
}

#[cfg(test)]
mod tests {
    use super::matches_pattern;

    #[test]
    fn pattern_matches_nonempty_wildcards() {
        assert!(matches_pattern("a/${x}/b", "a/zz/b"));
        assert!(!matches_pattern("a/${x}/b", "a//b"));
        assert!(matches_pattern("${x}", "q"));
        assert!(!matches_pattern("${x}", ""));
        assert!(matches_pattern("${a}${b}", "ab"));
        assert!(!matches_pattern("${a}${b}", "a"));
        assert!(matches_pattern("p/${a}.${b}", "p/x.y"));
        assert!(!matches_pattern("p/${a}.${b}", "p/x"));
        assert!(matches_pattern("a${x", "a${x"));
        assert!(!matches_pattern("a/${x}", "b/c"));
    }
}
