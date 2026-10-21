//! 承認の「直す」の誤った対象の候補と表示名の欄の試験（TASK-62 2/2、PACK-42・D-69）。
//!
//! 決定（D-69、2026-10-21）は「`approval` の `input.revise` に、誤った対象の候補と表示名（`label`）を宣言する欄を足す」
//! である。語彙 v1.5 の 2 語（`input.revise.candidates`・`input.revise.candidates[].label`）として、実パック
//! （`tests/data/revise-candidates-pack`）について次を確かめる。
//!
//! - 宣言が `validate`・`compile` を診断 0 件で通り、IR の `input` に宣言の順・値のまま出ること
//! - 型で読めない宣言は `E-PARSE`、意味の制約の違反（空・重複・空白だけの `label`）と、規則の `target` が候補に無い
//!   宣言は `E-STRUCTURE` になり、診断に候補の ID・`label`・`target` の値が出ないこと
//! - `pack.vocab` が 1.5 より古いと `E-VOCAB-MISMATCH` が 1 件になること
//! - core の `Session` で、正しい対象の「直す」だけが受付になり、誤った候補と対象なしは受付にならないこと
//! - モジュールの Step で `${...}` を使った宣言は、展開後に検査されること
//!
//! 確かめないこと: 誤った候補への拒否の文面・分類（判定 TASK-265）、候補のボタンの表示（画面 TASK-448）。
//! 試験では `unwrap` を使ってよい。

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::{copy_dir, Tmp};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::{RunState, Session, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};
use serde_json::{json, Value};

const CHAPTER: &str = "chapters/c1/chapter.yaml";
const WORKSHOP: &str = "workshop.yaml";

const CANDIDATES: &str = "          - {id: expect, label: 期待する結果}\n          - {id: reason, label: 理由}\n          - {id: scope}\n";
const TARGET_RULE: &str = "on: {approval: revise, target: expect}";

fn pack_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/revise-candidates-pack")
}

/// 写しを作り、`file` の `from` を `to` に置き換える（置換元が 1 回だけあることを確かめる）。
fn edited(tag: &str, file: &str, from: &str, to: &str) -> Tmp {
    let tmp = Tmp::new(tag);
    copy_dir(&pack_dir(), &tmp.0);
    let path = tmp.0.join(file);
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.matches(from).count(), 1, "置換元がある: {from}");
    std::fs::write(&path, text.replace(from, to)).unwrap();
    tmp
}

fn codes(d: &[Diagnostic]) -> Vec<DiagCode> {
    d.iter().map(Diagnostic::code).collect()
}

fn compile_errors(dir: &Path) -> Vec<Diagnostic> {
    compile_dir(dir).expect_err("宣言の誤りは IR を出さない")
}

#[test]
fn pack42_declaration_passes_validate_and_compile() {
    let diags = validate_dir(&pack_dir());
    assert!(diags.is_empty(), "{diags:?}");
    assert!(compile_dir(&pack_dir()).is_ok());
}

#[test]
fn pack42_ir_keeps_the_declaration_as_written() {
    let ir: Value = serde_json::from_slice(&compile_dir(&pack_dir()).unwrap()).unwrap();
    let node = ir["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["uid"] == "main/st-pick")
        .unwrap();
    assert_eq!(
        node["step"]["input"],
        json!({
            "kind": "approval",
            "revise": {"candidates": [
                {"id": "expect", "label": "期待する結果"},
                {"id": "reason", "label": "理由"},
                {"id": "scope"},
            ]}
        })
    );
}

#[test]
fn pack42_semantic_violations_are_structure_errors() {
    let cases = [
        (
            "empty",
            "candidates:\n".to_string(),
            "candidates: []\n".to_string(),
            true,
        ),
        (
            "blank-id",
            "{id: scope}".to_string(),
            "{id: \"  \"}".to_string(),
            false,
        ),
        (
            "dup-id",
            "{id: scope}".to_string(),
            "{id: reason}".to_string(),
            false,
        ),
        (
            "blank-label",
            "{id: scope}".to_string(),
            "{id: scope, label: \"  \"}".to_string(),
            false,
        ),
    ];
    for (tag, from, to, whole) in cases {
        let tmp = if whole {
            // 候補の一覧をまるごと空の配列にする。
            let t = Tmp::new("revise-cand-empty");
            copy_dir(&pack_dir(), &t.0);
            let path = t.0.join(CHAPTER);
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(text.contains(CANDIDATES));
            std::fs::write(
                &path,
                text.replace(
                    &format!("        candidates:\n{CANDIDATES}"),
                    "        candidates: []\n",
                ),
            )
            .unwrap();
            t
        } else {
            edited(&format!("revise-cand-{tag}"), CHAPTER, &from, &to)
        };
        let validated = validate_dir(&tmp.0);
        assert_eq!(
            codes(&validated),
            [DiagCode::Structure],
            "{tag}: {validated:?}"
        );
        assert_eq!(
            codes(&compile_errors(&tmp.0)),
            [DiagCode::Structure],
            "{tag}"
        );
    }
}

#[test]
fn pack42_unreadable_declarations_are_parse_errors() {
    for (tag, from, to) in [
        ("unknown-key", "{id: scope}", "{id: scope, labels: x}"),
        ("label-type", "{id: scope}", "{id: scope, label: 1}"),
        (
            "revise-type",
            "revise:\n        candidates:",
            "revise: 1\n      x:",
        ),
    ] {
        let tmp = edited(&format!("revise-parse-{tag}"), CHAPTER, from, to);
        let d = validate_dir(&tmp.0);
        assert!(
            d.iter().any(|d| d.code() == DiagCode::Parse),
            "{tag}: {d:?}"
        );
        assert!(
            compile_errors(&tmp.0)
                .iter()
                .any(|d| d.code() == DiagCode::Parse),
            "{tag}"
        );
    }
}

#[test]
fn pack42_rule_target_must_be_a_declared_candidate() {
    let tmp = edited(
        "revise-target-missing",
        CHAPTER,
        TARGET_RULE,
        "on: {approval: revise, target: elsewhere}",
    );
    let d = validate_dir(&tmp.0);
    assert_eq!(codes(&d), [DiagCode::Structure], "{d:?}");
    // 候補に入れた対象の規則は通る。
    let ok = edited(
        "revise-target-ok",
        CHAPTER,
        TARGET_RULE,
        "on: {approval: revise, target: scope}",
    );
    assert!(validate_dir(&ok.0).is_empty());
}

#[test]
fn pack42_diagnostics_do_not_echo_declared_values() {
    let secret = "SECRET-CAND-4711";
    let all: Vec<Vec<Diagnostic>> = vec![
        validate_dir(
            &edited(
                "revise-echo-id",
                CHAPTER,
                "{id: scope}",
                &format!("{{id: reason, label: {secret}}}"),
            )
            .0,
        ),
        validate_dir(
            &edited(
                "revise-echo-label",
                CHAPTER,
                "{id: scope}",
                &format!("{{id: scope, label: \"  \", labels: {secret}}}"),
            )
            .0,
        ),
        validate_dir(
            &edited(
                "revise-echo-target",
                CHAPTER,
                TARGET_RULE,
                &format!("on: {{approval: revise, target: {secret}}}"),
            )
            .0,
        ),
    ];
    for d in all.iter().flatten() {
        let text = d.to_string();
        assert!(!text.contains(secret), "{text}");
        assert!(!text.contains("期待する結果"), "{text}");
    }
    assert!(all[2].iter().any(|d| d.code() == DiagCode::Structure));
}

#[test]
fn pack42_older_vocab_declaration_is_a_mismatch() {
    let old = edited("revise-vocab-old", WORKSHOP, "vocab: '1.5'", "vocab: '1.4'");
    let d = validate_dir(&old.0);
    assert_eq!(codes(&d), [DiagCode::VocabMismatch], "{d:?}");
    assert!(
        d[0].to_string().contains("input.revise.candidates"),
        "{}",
        d[0]
    );
    // 省略は検査しない。
    let none = edited("revise-vocab-none", WORKSHOP, ", vocab: '1.5'", "");
    assert!(validate_dir(&none.0).is_empty());
}

#[test]
fn pack42_other_input_kinds_do_not_count_as_revise() {
    // `choice` の欄 `revise` は語彙の語ではなく、型で読めず `E-PARSE` になる（`E-VOCAB-MISMATCH` ではない）。
    let tmp = edited(
        "revise-vocab-choice",
        WORKSHOP,
        "vocab: '1.5'",
        "vocab: '1.4'",
    );
    let path = tmp.0.join(CHAPTER);
    let text = std::fs::read_to_string(&path).unwrap();
    let start = text.find("    input:\n").unwrap();
    let end = text.find("    rules:\n").unwrap();
    let swapped = format!(
        "{}    input: {{kind: choice, options: [{{id: a, label: A}}], revise: 1}}\n{}",
        &text[..start],
        &text[end..]
    );
    std::fs::write(&path, swapped).unwrap();
    let d = validate_dir(&tmp.0);
    assert!(
        d.iter().all(|d| d.code() != DiagCode::VocabMismatch),
        "{d:?}"
    );
}

fn uid(step: &str) -> StepUid {
    StepUid(format!("main/{step}"))
}

fn session_at(step: &str) -> Session {
    let ir: Ir = load_ir(&compile_dir(&pack_dir()).unwrap()).unwrap();
    let u = uid(step);
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), StepStatus::Entered);
    let mut slices = BTreeMap::new();
    slices.insert(u.root(), slice);
    let run = RunState {
        generation: Generation(1),
        current: Some(u),
        slices,
        ..RunState::default()
    };
    Session::from_state(
        ir,
        SessionState {
            run,
            ..SessionState::default()
        },
    )
}

fn revise(target: Option<&str>) -> Input {
    Input::Approval {
        decision: Decision::Revise,
        target: target.map(str::to_owned),
    }
}

fn achieved(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Achieved { .. }))
        .count()
}

#[test]
fn pack42_only_the_declared_correct_target_is_accepted_in_core() {
    // 宣言が読めること（`InvalidDeclaration` で止まらないこと）は、受付になる入力が 1 つあることで確かめる。
    for wrong in [Some("reason"), Some("scope"), Some("unlisted"), None] {
        let mut s = session_at("st-pick");
        let ev = s.apply(Op::Input(revise(wrong))).to_vec();
        assert_eq!(achieved(&ev), 0, "{wrong:?}: {ev:?}");
        assert_eq!(s.state().run.current, Some(uid("st-pick")), "{wrong:?}");
    }
    let mut s = session_at("st-pick");
    let ev = s.apply(Op::Input(revise(Some("expect")))).to_vec();
    assert_eq!(achieved(&ev), 1, "{ev:?}");
    assert_eq!(s.state().run.current, Some(uid("st-end")));
}

const MODULE: &str = "id: m\ntitle: M\nparams: {t: expect}\nsteps:\n  - id: a\n    title: A\n    final: true\n    input: {kind: approval, revise: {candidates: [{id: expect}, {id: reason}]}}\n    rules:\n      - {id: r, on: {approval: revise, target: \"${t}\"}, outcome: {kind: accept}}\n    confirm: {kind: none, reason: 選ぶこと自体が意思表示のため。}\n";

fn module_pack(arg: &str) -> Tmp {
    let t = edited(
        "revise-module",
        CHAPTER,
        "  - id: st-end\n    title: おわり\n    body: b.md\n    final: true\n",
        &format!("  - id: st-end\n    title: おわり\n    body: b.md\n    final: true\n    use: {{module: m, instance: i, args: {{t: {arg}}}}}\n"),
    );
    // `use` を持つ Step は本文を持てないため、別の Step に分ける。
    let path = t.0.join(CHAPTER);
    let text = std::fs::read_to_string(&path).unwrap();
    let text = text.replace(
        "    body: b.md\n    final: true\n    use:",
        "    final: true\n    use:",
    );
    std::fs::write(&path, text).unwrap();
    t.put("modules/m/module.yaml", MODULE.as_bytes());
    t
}

#[test]
fn pack42_module_declaration_is_checked_after_expansion() {
    // 置換前の `${t}` は候補と突き合わせない。展開後の値だけを `compile` が検査する（`validate` は展開後の
    // 入力の検査を持たない。既存の `input` の展開後の検査と同じ）。
    let ok = module_pack("reason");
    assert!(validate_dir(&ok.0).is_empty());
    assert!(compile_dir(&ok.0).is_ok());
    let bad = module_pack("elsewhere");
    assert!(validate_dir(&bad.0).is_empty());
    let d = compile_errors(&bad.0);
    assert_eq!(codes(&d), [DiagCode::Structure], "{d:?}");
    assert_eq!(d[0].file(), "modules/m/module.yaml");
}
