//! `pack.vocab` の宣言と、実際に使った語彙の版の一致の検査（PACK-31・VER-14・D-43、TASK-59 1/2）。
//!
//! `validate` だけが呼ぶ（`compile` は行わない。IR・golden は変わらない）。宣言より新しい語彙を使う
//! 教材は、履歴の「語彙の軸」が宣言に頼るため、版違いの履歴の理由を特定できない。その穴を診断で塞ぐ。
//!
//! - `pack.vocab` を省略した教材は検査しない。PACK-31 の前提は「`pack.vocab` を書く」であり、書き方の
//!   規約は TASK-179 の持ち分である（contract の `DEFAULT` は履歴を書くときの値で、検査の既定ではない）
//! - 検査する語は番号つき拡張点 EP-9（`starts[].requires`・`unmet`）と EP-10〜EP-12（`pack.profile`・
//!   `appearance`・`left_pane`）。`confirm.options[].label`（v1.3）は TASK-59 2/2、v1.1 の語は対象外
//! - 必要な版は [`crate::vocab::EXTENSION_POINTS`] から引き、ここに書き写さない
//! - 読めない宣言（文字列でない・正規の `主.副` でない）は `E-PARSE` とし、値は診断に写さない（fail-closed）
//!
//! 拡張点ごとに 1 件だけ報告する。診断の文面は静的な ID・名前と、読み取った数値の宣言だけで組み立てる。

use fandhe_workshop_contract::history_version::VocabVersion as Declared;
use serde_json::Value;

use crate::diag::{DiagCode, Diagnostic};
use crate::source::LoadedPack;
use crate::validate::diag;
use crate::vocab::extension_point;

const ITEM: &str = "pack:vocab";

/// 拡張点の名前と、教材がそれを使っているかの判定。EP 番号の順に並べる。
type Probe = (&'static str, fn(&LoadedPack) -> bool);

const PROBES: &[Probe] = &[
    ("starts.requires", uses_start_condition),
    ("pack.profile", |p| p.workshop.pack.profile.is_some()),
    ("pack.appearance", |p| p.workshop.pack.appearance.is_some()),
    ("pack.left_pane", |p| p.workshop.pack.left_pane.is_some()),
];

/// 章・モジュール（呼ばれないものを含む）の開始点に `requires`・`unmet` があるか。
fn uses_start_condition(pack: &LoadedPack) -> bool {
    pack.scopes.iter().any(|s| {
        s.src
            .starts
            .iter()
            .any(|st| st.requires.is_some() || st.unmet.is_some())
    })
}

/// 宣言を読んで、使用と比べる。`pack.vocab` が無いときは空を返す。
pub(crate) fn check_vocab_declaration(pack: &LoadedPack) -> Vec<Diagnostic> {
    let Some(raw) = pack.workshop.pack.vocab.as_ref() else {
        return Vec::new();
    };
    let declared = match raw {
        Value::String(s) => Declared::parse(s),
        _ => None,
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
    for (name, used) in PROBES {
        let Some(ep) = extension_point(name) else {
            continue;
        };
        let (major, minor) = ep.version.major_minor();
        let needed = Declared { major, minor };
        if declared < needed && used(pack) {
            out.push(diag(
                file,
                ITEM,
                DiagCode::VocabMismatch,
                format!(
                    "{} ({}) needs vocab {needed} or later (declared {declared})",
                    ep.id, ep.name
                ),
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
    fn probes_name_real_extension_points_in_ep_order() {
        let mut last = 0usize;
        for (name, _) in PROBES {
            let i = EXTENSION_POINTS
                .iter()
                .position(|e| e.name == *name)
                .expect("probe names an extension point");
            assert!(i >= last, "probes must follow EP order");
            last = i;
        }
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
