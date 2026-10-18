//! 受講者が選んだ値の差し込みが語彙に無いこと（忠実さの限界）の観察の固定（TASK-66 2/2、PACK-39・D-50）。
//!
//! 決定（D-50、2026-10-12）は「選んだ値を差し込む語は足さない」である。教材は、起こりうる入力を規則として
//! 列挙するか、選択ごとに属性へ固定値を書く回避で書く。本試験は、その現状を実パック
//! （`tests/data/chosen-value-pack`）と一時ディレクトリの写しで固定する。
//!
//! - 差し込みらしい欄は、Step の効果（`on_enter` など）に書くと `E-PARSE`（未知の欄）で止まり、IR を出さない。
//!   規則の `outcome.effects` の中では読み込み時に止まらないが、実行時に入力ごと拒否され（fail-closed）、
//!   受講者の値は世界状態に届かない
//! - 拒否の文面の枠は閉じた集合で、受講者の値を指す枠は `E-SLOT-UNKNOWN` になる
//! - 章の中の `${名前}` は文字のまま残り、受講者の値は世界状態の経路にも属性にも入らない
//! - 回避（列挙・固定値の属性）が書ける。列挙に無い値は拒否され、世界状態は変わらない
//!
//! 確かめないこと: 受講者の文が原文のまま保たれること（`crates/core/tests/text_judge.rs`）、画面の表示、
//! `fandhe-pack test` の結線。試験では `unwrap` を使ってよい。

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::{assert_no_secret, copy_dir, Tmp, SECRET};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::{Judgment, RejectCategory};
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::{RunState, Session, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const CHAPTER: &str = "chapters/c1/chapter.yaml";
const PUT_ALPHA: &str = "put: {path: work/alpha, attrs: {label: first}}";

fn pack_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/chosen-value-pack")
}

/// 実パックの写しを作り、`rel` の文字列を置き換える。置換元が無ければ試験の側の誤りなので落とす。
fn variant(tag: &str, rel: &str, from: &str, to: &str) -> Tmp {
    let tmp = Tmp::new(tag);
    copy_dir(&pack_dir(), &tmp.0);
    let path = tmp.0.join(rel);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(from), "置換元がある: {from}");
    std::fs::write(&path, text.replace(from, to)).unwrap();
    tmp
}

fn ir_of(dir: &Path) -> Ir {
    let bytes = compile_dir(dir).unwrap_or_else(|d| panic!("compile {dir:?}: {d:?}"));
    load_ir(&bytes).expect("the engine loads the IR")
}

/// `compile_dir` が失敗し、診断がちょうど 1 件であることを確かめて返す。
fn one_diag(dir: &Path) -> Diagnostic {
    let mut diags = match compile_dir(dir) {
        Ok(_) => panic!("IR を出さない: {dir:?}"),
        Err(d) => d,
    };
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_no_secret(&diags);
    diags.remove(0)
}

/// 開始点の Step を `Entered` にした状態で作る（`Control::Begin` は未実装のため、開始点の効果は当てない）。
fn session_at(ir: &Ir, step: &str, world: &[(&str, Element)]) -> Session {
    let uid = StepUid(format!("main/{step}"));
    let mut slice = Slice::default();
    slice.status.insert(uid.clone(), StepStatus::Entered);
    let mut slices = BTreeMap::new();
    slices.insert(uid.root(), slice);
    let mut run = RunState {
        current: Some(uid),
        slices,
        ..RunState::default()
    };
    for (path, el) in world {
        run.world.insert((*path).to_string(), el.clone());
    }
    Session::from_state(
        ir.clone(),
        SessionState {
            run,
            ..SessionState::default()
        },
    )
}

fn send(session: &mut Session, input: Input) -> Vec<Event> {
    session.apply(Op::Input(input)).to_vec()
}

fn changed_paths(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Changed { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect()
}

fn judgment_of(events: &[Event]) -> &Judgment {
    events
        .iter()
        .find_map(|e| match e {
            Event::Judged { judgment, .. } => Some(judgment),
            _ => None,
        })
        .expect("Judged が 1 件ある")
}

/// 世界状態の経路・属性値のどこにも `needle` が無い。
fn world_lacks(session: &Session, needle: &str) -> bool {
    session.state().run.world.iter().all(|(path, el)| {
        !path.contains(needle)
            && el
                .attrs
                .iter()
                .all(|(k, v)| !k.contains(needle) && !v.contains(needle))
    })
}

#[test]
fn pack39_pack_validates_and_compiles() {
    let dir = pack_dir();
    assert_eq!(validate_dir(&dir), vec![], "validate は診断 0 件");
    let ir = ir_of(&dir);
    assert!(ir.nodes.iter().any(|n| n.uid == "main/a"));
    assert!(ir.nodes.iter().any(|n| n.uid == "main/c"));
}

#[test]
fn pack39_no_insertion_field_on_effects() {
    // 差し込みらしい欄を、Step の `on_enter` の効果の語の中に足す。どれも未知の欄で、受講者の値を取り込む
    // 経路にならない。
    let cases = [
        ("put: {path: w, attrs_from_input: name}", "attrs_from_input"),
        ("put: {path: w, path_from: input}", "path_from"),
        (
            "set_attr: {path: w, attr: k, value: x, value_from: input}",
            "value_from",
        ),
    ];
    for (i, (effect, field)) in cases.iter().enumerate() {
        let tmp = variant(
            &format!("chosen-effect-{i}"),
            CHAPTER,
            "    body: a.md\n    next: [{to: b}]\n",
            &format!("    body: a.md\n    next: [{{to: b}}]\n    on_enter: [{{{effect}}}]\n"),
        );
        let d = one_diag(&tmp.0);
        assert_eq!(d.code(), DiagCode::Parse, "{field}: {d}");
        assert!(
            d.message().contains(&format!("unknown field `{field}`")),
            "{d}"
        );
    }
    // 最上位の未知の語。値は診断に写さない。
    let top = Tmp::new("chosen-top");
    copy_dir(&pack_dir(), &top.0);
    let path = top.0.join("workshop.yaml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(&format!("insert: {SECRET}\n"));
    std::fs::write(&path, text).unwrap();
    let d = one_diag(&top.0);
    assert_eq!(d.code(), DiagCode::Parse, "{d}");
}

#[test]
fn pack39_insertion_field_in_rule_effects_never_takes_the_learner_value() {
    // 観察: `rules[].outcome.effects` の中の未知の欄は、読み込み時には止まらない（`on_enter` などとは違う）。
    // 差し込みの語が無いことには変わりがなく、受講者の値は世界状態に届かない。
    let tmp = variant(
        "chosen-rule-effect",
        CHAPTER,
        PUT_ALPHA,
        "put: {path: work/alpha, attrs_from_input: name}",
    );
    let ir = ir_of(&tmp.0);
    let mut session = session_at(&ir, "a", &[]);
    let events = send(&mut session, Input::Command("/make-item alpha".into()));
    // 実行時は fail-closed で、入力は受け付けられず世界状態は変わらない。
    assert!(events.iter().any(|e| matches!(e, Event::Warning { .. })));
    assert!(changed_paths(&events).is_empty());
    assert!(session.state().run.world.is_empty());
}

#[test]
fn pack39_no_insertion_field_in_format_a() {
    let toml = |effects: &str| {
        let t = Tmp::new("chosen-toml");
        t.put(
            "workshop.toml",
            b"[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n",
        );
        t.put(
            "chapters/c1/chapter.toml",
            format!(
                "id = \"c1\"\ntitle = \"C\"\n[[starts]]\nid = \"s1\"\nnode = \"a\"\neffects = {effects}\n[[steps]]\nid = \"a\"\ntitle = \"A\"\nfinal = true\n"
            )
            .as_bytes(),
        );
        t
    };
    assert!(compile_dir(&toml("[{put = {path = \"w\"}}]").0).is_ok());
    let t = toml(&format!(
        "[{{set_attr = {{path = \"w\", attr = \"k\", value = \"v\", value_from = \"{SECRET}\"}}}}]"
    ));
    let d = one_diag(&t.0);
    assert_eq!(d.code(), DiagCode::Parse, "{d}");
}

#[test]
fn pack39_learner_value_slot_is_unknown() {
    // 拒否の文面の枠は閉じた集合（PACK-16）。受講者の値を指す枠は足せない。
    for (i, slot) in ["{{chosen}}", "{{input_value}}"].iter().enumerate() {
        let tmp = variant(
            &format!("chosen-slot-{i}"),
            "workshop.yaml",
            "message: その値は、この Step の候補にありません。",
            &format!("message: \"その値 {slot} は、この Step の候補にありません。\""),
        );
        let d = one_diag(&tmp.0);
        assert_eq!(d.code(), DiagCode::SlotUnknown, "{slot}: {d}");
    }
}

#[test]
fn pack39_chapter_placeholder_stays_literal_in_world() {
    // `${名前}` はモジュールの引数をコンパイル時に置き換えるだけで、章の中では文字のまま残る。
    let tmp = variant(
        "chosen-placeholder",
        CHAPTER,
        PUT_ALPHA,
        "put: {path: 'work/${name}', attrs: {label: '${name}'}}",
    );
    let ir = ir_of(&tmp.0);
    let node = ir.nodes.iter().find(|n| n.uid == "main/a").unwrap();
    let rule_text = serde_json::to_string(&node.step.rules).unwrap();
    assert!(rule_text.contains("work/${name}"), "{rule_text}");

    let mut session = session_at(&ir, "a", &[]);
    let events = send(&mut session, Input::Command("/make-item alpha".into()));
    assert!(matches!(judgment_of(&events), Judgment::Accepted(_)));
    assert_eq!(changed_paths(&events), vec!["work/${name}".to_string()]);
    let world = &session.state().run.world;
    assert_eq!(world["work/${name}"].attrs["label"], "${name}");
    assert!(world_lacks(&session, "alpha"));
}

#[test]
fn pack39_enumerated_values_apply_their_fixed_effects() {
    let ir = ir_of(&pack_dir());
    for (value, put, other) in [
        ("alpha", "work/alpha", "work/beta"),
        ("beta", "work/beta", "work/alpha"),
    ] {
        let mut session = session_at(&ir, "a", &[]);
        let events = send(&mut session, Input::Command(format!("/make-item {value}")));
        assert!(
            matches!(judgment_of(&events), Judgment::Accepted(_)),
            "{value}"
        );
        let world = &session.state().run.world;
        assert!(world.contains_key(put), "{value}");
        assert!(!world.contains_key(other), "{value}");
    }
}

#[test]
fn pack39_unlisted_value_is_rejected_and_world_unchanged() {
    let ir = ir_of(&pack_dir());
    let mut session = session_at(&ir, "a", &[]);
    let before = session.state().run.world.clone();
    let events = send(&mut session, Input::Command("/make-item gamma".into()));
    match judgment_of(&events) {
        Judgment::Rejected(r) => assert_eq!(r.category, RejectCategory::NotInThisStep),
        other => panic!("拒否のはず: {other:?}"),
    }
    assert!(changed_paths(&events).is_empty());
    assert_eq!(session.state().run.world, before);
    assert!(world_lacks(&session, "gamma"));
}

#[test]
fn pack39_catch_all_rule_applies_only_the_fixed_effect() {
    // 引数の条件を持たない規則を最後に置けば、列挙に無い値も受け付けられる。ただし効果は固定値で、
    // 受講者の値は世界状態のどこにも入らない。
    let tmp = variant(
        "chosen-catch-all",
        CHAPTER,
        "    confirm: {kind: none, reason: 送った結果がそのまま次の Step の前提になるため、別の確認を置かない。}\n  - id: c\n",
        concat!(
            "      - id: r-other\n",
            "        on: {command: make-item}\n",
            "        outcome:\n",
            "          kind: accept\n",
            "          effects:\n",
            "            - put: {path: work/other, attrs: {label: fixed}}\n",
            "    confirm: {kind: none, reason: 送った結果がそのまま次の Step の前提になるため、別の確認を置かない。}\n",
            "  - id: c\n",
        ),
    );
    assert_eq!(validate_dir(&tmp.0), vec![]);
    let ir = ir_of(&tmp.0);
    let mut session = session_at(&ir, "a", &[]);
    let events = send(&mut session, Input::Command("/make-item gamma".into()));
    assert!(matches!(judgment_of(&events), Judgment::Accepted(_)));
    assert_eq!(changed_paths(&events), vec!["work/other".to_string()]);
    assert_eq!(
        session.state().run.world["work/other"].attrs["label"],
        "fixed"
    );
    assert!(world_lacks(&session, "gamma"));
}

#[test]
fn pack39_choice_rewrites_an_attribute_with_a_fixed_value() {
    // 属性を書き換える回避。選択ごとの規則が、宣言した固定値を属性に書く。
    let ir = ir_of(&pack_dir());
    let draft = Element {
        fixture: None,
        attrs: [("tone".to_string(), "初期".to_string())].into(),
    };
    for (option, want) in [("opt-a", "丁寧な文面"), ("opt-b", "簡潔な文面")] {
        let mut session = session_at(&ir, "c", &[("draft", draft.clone())]);
        let events = send(&mut session, Input::Choice(option.into()));
        assert!(
            matches!(judgment_of(&events), Judgment::Accepted(_)),
            "{option}"
        );
        assert_eq!(
            session.state().run.world["draft"].attrs["tone"],
            want,
            "{option}"
        );
        let chosen = session
            .state()
            .run
            .slices
            .values()
            .find_map(|s| s.choices.get(&StepUid("main/c".into())))
            .cloned();
        assert_eq!(chosen.as_deref(), Some(option));
    }
}
