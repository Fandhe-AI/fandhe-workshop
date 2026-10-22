//! `pack.vocab` の宣言と、実際に使った語彙の版の一致の検査（PACK-31・VER-14・D-43、TASK-59 1/2・2/2）。
//!
//! `validate` だけが呼ぶ（`compile` は行わない。IR・golden は変わらない）。宣言より新しい語彙を使う
//! 教材は、履歴の「語彙の軸」が宣言に頼るため、版違いの履歴の理由を特定できない。その穴を診断で塞ぐ。
//!
//! - `pack.vocab` を省略した教材は検査しない。PACK-31 の前提は「`pack.vocab` を書く」であり、書き方の
//!   規約は TASK-179 の持ち分である（contract の `DEFAULT` は履歴を書くときの値で、検査の既定ではない）
//! - 検査する語は番号つき拡張点 EP-9（`starts[].requires`・`unmet`）と EP-10〜EP-13（`pack.profile`・
//!   `appearance`・`left_pane`・`views.<ID>.body`）、番号を持たない `confirm.options[].label`（v1.3。TASK-59 2/2）と `input.revise.candidates`（v1.5。TASK-62 2/2。`label` は候補の中の欄なので候補で代表する）・`do_levels`（v1.6。TASK-70 2/2）。v1.1 の語は対象外
//! - `confirm.options[].label` は確認の `confirm` だけを見る。入力方式 `choice` の `input.options[].label`
//!   （v1 の語。PACK-23）は別の欄であり、この判定は `step.input` を読まない。`input.revise`（`kind: approval`）と `env_action`（PACK-34）だけは `step.input` を読む
//! - 必要な版は [`crate::vocab::EXTENSION_POINTS`]・[`crate::vocab::term`] から引き、ここに書き写さない
//! - 読めない宣言（文字列でない・正規の `主.副` でない）は `E-PARSE` とし、値は診断に写さない（fail-closed）
//!
//! 語（拡張点・欄）ごとに 1 件だけ報告する。診断の文面は静的な ID・名前と、読み取った数値の宣言だけで組み立てる。

use fandhe_workshop_contract::history_version::VocabVersion as Declared;
use fandhe_workshop_contract::ir::Start;
use serde_json::Value;

use fandhe_workshop_contract::limits::MAX_IR_BYTES;

use crate::compile::Budget;
use crate::diag::{DiagCode, Diagnostic};
use crate::expand::Expander;
use crate::source::{LoadedPack, ScopeKind};
use crate::validate::diag;
use crate::vocab::{extension_point, term};

const ITEM: &str = "pack:vocab";

/// 必要な版の出所。拡張点は名前で、番号を持たない語は（分類, 語）で表を引く。
#[derive(Clone, Copy)]
enum Source {
    Ep(&'static str),
    Term(&'static str, &'static str),
}

/// 語の出所と、教材がそれを使っているかの判定。EP 番号の順に並べ、番号を持たない語を最後に置く。
type Probe = (Source, fn(&LoadedPack) -> bool);

const LABEL: Source = Source::Term("field", "confirm.options[].label");

const REVISE: Source = Source::Term("field", "input.revise.candidates");

const ENV_ACTION: Source = Source::Term("input", "env_action");

const DO_LEVELS: Source = Source::Term("field", "do_levels");

const PROBES: &[Probe] = &[
    (Source::Ep("starts.requires"), uses_start_condition),
    (Source::Ep("pack.profile"), |p| {
        p.workshop.pack.profile.is_some()
    }),
    (Source::Ep("pack.appearance"), |p| {
        p.workshop.pack.appearance.is_some()
    }),
    (Source::Ep("pack.left_pane"), |p| {
        p.workshop.pack.left_pane.is_some()
    }),
    (Source::Ep("view.document_body"), uses_view_body),
    (LABEL, uses_answer_label),
    (REVISE, uses_revise_candidates),
    (ENV_ACTION, uses_env_action),
    (DO_LEVELS, uses_do_levels),
];

/// 語の版（主・副）と、診断の文面の接頭を表から引く。引けない項目は `None`（単体試験で全項目が引けることを保証する）。
fn resolve(src: Source) -> Option<(Declared, String)> {
    match src {
        Source::Ep(name) => {
            let ep = extension_point(name)?;
            let (major, minor) = ep.version.major_minor();
            Some((
                Declared { major, minor },
                format!("{} ({})", ep.id, ep.name),
            ))
        }
        Source::Term(kind, t) => {
            let vt = term(kind, t)?;
            let (major, minor) = vt.version.major_minor();
            Some((Declared { major, minor }, vt.term.to_string()))
        }
    }
}

/// 全 Step の `confirm.options[]` に `label` があるか。生の値を辿り、`kind` は見ない（置換前の値や他の検査で
/// 落ちる宣言でも使用を取りこぼさない）。`null` は使用に数えない。`step.input` は読まない（入力方式 `choice` の
/// `label` は v1 の語で別の欄）。
fn uses_answer_label(pack: &LoadedPack) -> bool {
    pack.scopes.iter().any(|s| {
        s.src.steps.iter().any(|step| {
            step.confirm
                .as_ref()
                .and_then(|c| c.get("options"))
                .and_then(Value::as_array)
                .is_some_and(|opts| {
                    opts.iter()
                        .any(|o| o.get("label").is_some_and(|l| !l.is_null()))
                })
        })
    })
}

/// 全 Step の `input`（`kind: approval`）が `revise` を持つか。生の値を辿り、`revise` の中身は見ない（読めない
/// 宣言は別に `E-PARSE`）。`null` は使用に数えない。他の入力方式の `revise` は語彙の語ではないので数えない。
fn uses_revise_candidates(pack: &LoadedPack) -> bool {
    pack.scopes.iter().any(|s| {
        s.src.steps.iter().any(|step| {
            step.input.as_ref().is_some_and(|i| {
                i.get("kind").and_then(Value::as_str) == Some("approval")
                    && i.get("revise").is_some_and(|r| !r.is_null())
            })
        })
    })
}

/// いずれかの Step が空でない `do_levels` を持つか（v1.6。PACK-50）。置換前の Step を見るので、モジュールの
/// 引数経由の値でも使用を取りこぼさない。
fn uses_do_levels(pack: &LoadedPack) -> bool {
    pack.scopes
        .iter()
        .any(|s| s.src.steps.iter().any(|step| !step.do_levels.is_empty()))
}

/// 全 Step の `input` が `kind: env_action`（PACK-34）か。生の値で `kind` だけを見る（欄の中身が読めない宣言は
/// 別に `E-PARSE`）。置換前の値でも使用を取りこぼさない。
fn uses_env_action(pack: &LoadedPack) -> bool {
    uses_env_action_raw(pack) || uses_env_action_expanded(pack)
}

/// 置換前の Step の `input` が `kind: env_action` か。
fn uses_env_action_raw(pack: &LoadedPack) -> bool {
    pack.scopes.iter().any(|s| {
        s.src.steps.iter().any(|step| {
            step.input
                .as_ref()
                .is_some_and(|i| i.get("kind").and_then(Value::as_str) == Some("env_action"))
        })
    })
}

/// モジュールの `input` の `kind` が引数に由来する（`${` を含む）教材だけ、展開後の Step の `input` も見る。
/// 引数経由で `env_action` が渡る場合、置換前の値では使用を取りこぼすため。含まない教材は展開しない。
fn uses_env_action_expanded(pack: &LoadedPack) -> bool {
    let templated = pack
        .scopes
        .iter()
        .filter(|s| s.kind == ScopeKind::Module)
        .any(|s| {
            s.src.steps.iter().any(|step| {
                step.input
                    .as_ref()
                    .and_then(|i| i.get("kind"))
                    .is_some_and(|k| k.to_string().contains("${"))
            })
        });
    if !templated {
        return false;
    }
    let (profile, _) = crate::profile::check_profile(&pack.workshop_file, &pack.workshop);
    let mut ex = Expander::new(pack, &profile, Budget::new(MAX_IR_BYTES));
    for scope in pack.scopes.iter().filter(|s| s.kind == ScopeKind::Chapter) {
        ex.expand_chapter(scope);
    }
    ex.nodes.iter().any(|n| {
        n.step
            .input
            .as_ref()
            .is_some_and(|i| i.get("kind").and_then(Value::as_str) == Some("env_action"))
    })
}

/// `workshop.views` のどれかが `body` を持つか（EP-13）。値の型や綴りは見ない（読めない値は別に `E-PARSE`）。
/// `null` は使用に数えない。
fn uses_view_body(pack: &LoadedPack) -> bool {
    pack.workshop
        .views
        .values()
        .any(|v| v.get("body").is_some_and(|b| !b.is_null()))
}

/// 章・モジュール（呼ばれないものを含む）の開始点に `requires`・`unmet` があるか。
fn uses_start_condition(pack: &LoadedPack) -> bool {
    pack.scopes
        .iter()
        .any(|s| s.src.starts.iter().any(start_uses_ep9))
}

/// 開始点 1 件が EP-9（開始条件）を使うか。`vocab_report` の数え上げと使用の判定を食い違わせないため共有する。
pub(crate) fn start_uses_ep9(st: &Start) -> bool {
    st.requires.is_some() || st.unmet.is_some()
}

/// 宣言を読んで、使用と比べる。`pack.vocab` が無いときは空を返す。
pub(crate) fn check_vocab_declaration(pack: &LoadedPack) -> Vec<Diagnostic> {
    // 省略は検査しない。値のない `null`（`vocab:`・`~`）は明示した不正な宣言なので `E-PARSE` にする。
    let declared = match pack.workshop.pack.vocab.as_ref() {
        None if pack.vocab_explicit_null => None,
        None => return Vec::new(),
        Some(Value::String(s)) => Declared::parse(s),
        Some(_) => None,
    };
    let file = pack.workshop_file.as_str();
    let Some(declared) = declared else {
        return vec![diag(
            file,
            ITEM,
            DiagCode::Parse,
            "pack.vocab must be a string of the form major.minor",
        )];
    };
    let mut out = Vec::new();
    for (src, used) in PROBES {
        let Some((needed, head)) = resolve(*src) else {
            continue;
        };
        if declared < needed && used(pack) {
            out.push(diag(
                file,
                ITEM,
                DiagCode::VocabMismatch,
                format!("{head} needs vocab {needed} or later (declared {declared})"),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vocab::EXTENSION_POINTS;

    #[test]
    fn probes_resolve_and_follow_ep_order_then_terms() {
        use crate::vocab::TERMS;
        let mut last = 0usize;
        let mut seen_term = false;
        for (src, _) in PROBES {
            assert!(resolve(*src).is_some(), "every probe resolves a version");
            match src {
                Source::Ep(name) => {
                    assert!(!seen_term, "extension points come before plain terms");
                    let i = EXTENSION_POINTS
                        .iter()
                        .position(|e| e.name == *name)
                        .expect("probe names an extension point");
                    assert!(i >= last, "probes must follow EP order");
                    last = i;
                }
                Source::Term(kind, t) => {
                    seen_term = true;
                    let vt = TERMS
                        .iter()
                        .find(|x| x.kind == *kind && x.term == *t)
                        .expect("term exists");
                    assert!(vt.extension_point.is_none(), "terms here have no EP number");
                }
            }
        }
        assert!(PROBES
            .iter()
            .any(|(s, _)| matches!(s, Source::Term(_, "confirm.options[].label"))));
        assert!(matches!(
            PROBES.last(),
            Some((Source::Term(_, "do_levels"), _))
        ));
    }

    #[test]
    fn declared_versions_compare_major_then_minor() {
        let v = |s| Declared::parse(s).expect("parse");
        assert!(v("1.0") < v("1.2"));
        assert!(v("1.10") > v("1.3"));
        assert!(v("2.0") > v("1.9"));
        assert_eq!(Declared::parse("1.2.0"), None);
        assert_eq!(Declared::parse("v1.2"), None);
        assert_eq!(Declared::parse("01.2"), None);
    }
}
