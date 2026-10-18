//! 教材のどこかの `put`・`replace` が置く経路の集合（`left_pane` の `E-LEFT-TARGET-ROW` と
//! `element_check` の `E-REF-ELEMENT` が共有する）。
//!
//! 世界状態は空で始まり、要素を作るのは `put`・`replace` だけである。このため「どこでも置かれない経路」は
//! 実行のどの順でも存在しない必要条件の判定になる。順序や枝は見ない静的な近似で、誤検出を出さない側に倒す。
//! 照合は正規表現を組み立てない線形の走査にする（教材データを信頼しない入力として扱うため）。

use std::collections::BTreeSet;

use fandhe_workshop_contract::ir::Step;
use serde_json::Value;

use crate::source::LoadedPack;
use crate::validate::{elements, get};

/// 教材のどこかの `put`・`replace` が置く経路。`${` を含む経路は、空でない任意の文字列を含む形として別に持つ。
pub(crate) struct Placed<'a> {
    exact: BTreeSet<&'a str>,
    patterns: Vec<&'a str>,
}

impl<'a> Placed<'a> {
    /// 全スコープの Step（`on_enter`・`on_achieved`・`processing[].effects`・`rules[].outcome.effects`）と
    /// `starts[].effects` の `put`・`replace` から、文字列の `path` を集める。語彙の誤りは別の検査が報告する。
    pub(crate) fn empty() -> Self {
        Placed {
            exact: BTreeSet::new(),
            patterns: Vec::new(),
        }
    }

    pub(crate) fn collect(pack: &'a LoadedPack) -> Self {
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

    pub(crate) fn add_step(&mut self, step: &'a Step) {
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

    pub(crate) fn add_effects(&mut self, effects: &'a [Value]) {
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

    pub(crate) fn contains(&self, target: &str) -> bool {
        self.exact.contains(target) || self.patterns.iter().any(|p| matches_pattern(p, target))
    }
}

/// `${...}` を空文字列を含む任意の文字列とみなした照合。`${` が無ければ完全一致。
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
        // 変数の部分は 0 文字以上（モジュールの引数は空文字列も受け付ける）。
        let Some(pos) = remain.find(lit) else {
            return false;
        };
        remain = &remain[pos + lit.len()..];
    }
    remain.ends_with(literals[last])
}

#[cfg(test)]
mod tests {
    use super::matches_pattern;

    #[test]
    fn pattern_matches_nonempty_wildcards() {
        assert!(matches_pattern("a/${x}/b", "a/zz/b"));
        assert!(matches_pattern("a/${x}/b", "a//b"));
        assert!(matches_pattern("${x}", "q"));
        assert!(matches_pattern("${x}", ""));
        assert!(matches_pattern("${a}${b}", "ab"));
        assert!(matches_pattern("${a}${b}", "a"));
        assert!(matches_pattern("f/${s}x", "f/x"));
        assert!(!matches_pattern("f/${s}x", "f/y"));
        assert!(matches_pattern("p/${a}.${b}", "p/x.y"));
        assert!(!matches_pattern("p/${a}.${b}", "p/x"));
        assert!(matches_pattern("a${x", "a${x"));
        assert!(!matches_pattern("a/${x}", "b/c"));
    }
}
