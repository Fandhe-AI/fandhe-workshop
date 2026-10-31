//! 実体どうしの世界状態の経路の重なりの現状の動きの固定（TASK-253、MOD-18。記録は `docs/testing.md` の 5.423）。
//!
//! 決定（TASK-251。2026-10-31）で、経路の重なりを機械的に検査せず作成者の規約にとどめると決まった。
//! 意図した共有と誤った重なりは経路だけでは見分けられないためで、エンジンの動き・型・IR は変えない。
//! 本ファイルは、その「検査しない」現状を `Session` で動かして固定する。
//! 世界状態は教材全体で 1 つで、同じ経路に書く実体は上書きし合う。引数で分けた経路は交わらない。
//! 復帰は保存の後に持ち主が書いた経路だけを戻す（一般規則は core の `restore_owner_slice` と
//! `round_slice_init` の `mod9_*`、周回の側は `round_instance_restore` の `mod16_*`）。
//! 本ファイルは、コンパイルした教材の実体で、経路の重なりの側から同じ規則を確かめる。
//! core の依存に pack が無いため、pack crate の試験として置く。試験では `unwrap` を使ってよい。

mod common;
#[path = "common/rounds.rs"]
mod rounds;

use common::{assert_no_secret, testdata, Tmp, WORKSHOP};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{CheckpointId, Control, Input, Op};
use fandhe_workshop_core::instance::{owner_of, root_of};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::{Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use rounds::{begin_round2_plan, round1_plan, round2_plan, round_roots, run_plan};
use std::collections::BTreeSet;

const IN: &str = "input: {kind: choice, options: [{id: ok, label: OK}]}";
const CONF: &str = "confirm: {kind: none, reason: r}";

/// 実体が書く経路は `${area}/notes.md`（引数で分ける）と `shared/README.md`（固定の共有）の 2 つ。
fn module() -> String {
    format!(
        "id: m\ntitle: M\nparams: {{area: x, tag: x}}\nstarts: [{{id: s-w, node: w}}]\nsteps:\n  - {{id: w, title: W, next: [{{to: x}}], {IN}, rules: [{{id: r, on: {{option: ok}}, outcome: {{kind: accept, effects: [{{put: {{path: \"${{area}}/notes.md\", attrs: {{by: \"${{tag}}\"}}}}}}, {{set_attr: {{path: shared/README.md, attr: status, value: \"${{tag}}\"}}}}]}}}}], {CONF}}}\n  - {{id: x, title: X, next: [{{to: end}}], {IN}, rules: [{{id: r, on: {{option: ok}}, outcome: {{kind: accept}}}}], {CONF}}}\n  - {{id: end, title: E, final: true}}\n"
    )
}

fn use_step(id: &str, next: &str, instance: &str, area: &str) -> String {
    format!("  - {{id: {id}, title: {id}, use: {{module: m, instance: {instance}, args: {{area: {area}, tag: {instance}}}}}, next: [{{to: {next}}}]}}\n")
}

fn pack() -> Tmp {
    let t = Tmp::new("mod18");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    let mut c = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: u0}}]\nsteps:\n  - {{id: u0, title: U0, next: [{{to: ua}}], {IN}, rules: [{{id: r, on: {{option: ok}}, outcome: {{kind: accept, effects: [{{put: {{path: shared/README.md, attrs: {{status: init}}}}}}]}}}}], {CONF}}}\n"
    );
    c += &use_step("ua", "ub", "a", "a");
    c += &use_step("ub", "uc", "b", "b");
    c += &use_step("uc", "fin", "c", "a");
    c += "  - {id: fin, title: Fin, final: true}\n";
    t.put("chapters/c1/chapter.yaml", c.as_bytes());
    t.put("modules/m/module.yaml", module().as_bytes());
    t
}

fn ir_of(t: &Tmp) -> Ir {
    load_ir(&compile_dir(&t.0).expect("compile")).expect("load_ir")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn choice(s: &mut Session) -> Vec<Event> {
    s.apply(Op::Input(Input::Choice("ok".into()))).to_vec()
}

/// 呼び出しの Step `call`（`ua` など）へ `GoTo` で入り、実体の最初の Step に着く。
///
/// 実体の最後の Step から入力で呼び出し元の次の Step へ抜けられないため、実体どうしの移動は `GoTo` で補う
/// （`round_instance_restore.rs` と同じ事情。5.258・5.260）。
fn enter(s: &mut Session, call: &str) -> Vec<Event> {
    let target = s
        .ir()
        .nodes
        .iter()
        .find(|n| n.uid.ends_with(&format!("/{call}")) && n.scope == "main")
        .expect("call step")
        .uid
        .clone();
    s.apply(Op::Control(Control::GoTo(uid(&target)))).to_vec()
}

/// 実体の 2 つの Step（`w` の受付と `x` の受付）を通す。出たイベントは `w` の受付の分と `x` の分の連結。
fn pass(s: &mut Session) -> Vec<Event> {
    let mut ev = choice(s);
    ev.extend(choice(s));
    ev
}

/// `u0` の受付まで進めた状態（`m@a/w` に居る）。
fn started() -> (Session, Vec<Event>) {
    let ir = ir_of(&pack());
    let mut s = Session::from_state(ir, SessionState::default());
    let mut ev = s.apply(Op::Control(Control::Begin("s1".into()))).to_vec();
    ev.extend(choice(&mut s));
    assert_eq!(s.state().run.current, Some(uid("m@a/w")));
    (s, ev)
}

fn attr(s: &Session, path: &str, name: &str) -> Option<String> {
    s.state()
        .run
        .world
        .get(path)
        .and_then(|e| e.attrs.get(name))
        .map(|v| v.to_string())
}

/// `Changed` を（宛先の持ち主の根, 経路）で集める。
fn changed(ir: &Ir, ev: &[Event]) -> Vec<(Root, String)> {
    ev.iter()
        .filter_map(|e| match e {
            Event::Changed { at, path, .. } => Some((owner_of(ir, &at.step), path.clone())),
            _ => None,
        })
        .collect()
}

fn save(s: &mut Session) -> CheckpointId {
    match s.apply(Op::Control(Control::Save)).to_vec().as_slice() {
        [Event::CheckpointSaved { id, .. }] => *id,
        other => panic!("one CheckpointSaved expected: {other:?}"),
    }
}

fn node_text(ir: &Ir, u: &str) -> String {
    format!("{:?}", ir.nodes.iter().find(|n| n.uid == u).expect(u).step)
}

#[test]
fn mod18_validate_and_compile_report_no_overlap() {
    let t = pack();
    let d = validate_dir(&t.0);
    assert_no_secret(&d);
    assert!(d.is_empty(), "{d:?}");
    let ir = ir_of(&t);
    // 重なりが実在する（空振りの防止）。引数で分けた `b` だけ別の経路、共有の経路は全実体で同じ。
    let (a, b, c) = (
        node_text(&ir, "m@a/w"),
        node_text(&ir, "m@b/w"),
        node_text(&ir, "m@c/w"),
    );
    assert!(a.contains("a/notes.md") && c.contains("a/notes.md"));
    assert!(b.contains("b/notes.md") && !b.contains("a/notes.md"));
    for t in [&a, &b, &c] {
        assert!(t.contains("shared/README.md"), "{t}");
    }
}

#[test]
fn mod18_args_separate_paths_per_instance() {
    let (mut s, _) = started();
    pass(&mut s);
    enter(&mut s, "ub");
    let ev = pass(&mut s);
    let ir = s.ir().clone();
    let got: BTreeSet<_> = changed(&ir, &ev).into_iter().collect();
    let b = Root("m@b".into());
    let want: BTreeSet<_> = [
        (b.clone(), "b/notes.md".to_string()),
        (b, "shared/README.md".to_string()),
    ]
    .into();
    assert_eq!(got, want);
    assert_eq!(attr(&s, "a/notes.md", "by").as_deref(), Some("a"));
    assert_eq!(attr(&s, "b/notes.md", "by").as_deref(), Some("b"));
}

#[test]
fn mod18_same_path_is_overwritten_by_the_later_instance() {
    let (mut s, _) = started();
    pass(&mut s);
    enter(&mut s, "ub");
    pass(&mut s);
    enter(&mut s, "uc");
    let ev = choice(&mut s);
    assert!(
        !ev.iter()
            .any(|e| matches!(e, Event::Warning { .. } | Event::Refused { .. })),
        "{ev:?}"
    );
    let before = ev.iter().find_map(|e| match e {
        Event::Changed { path, before, .. } if path == "a/notes.md" => Some(before.clone()),
        _ => None,
    });
    let before = before.expect("a/notes.md changed").expect("before");
    assert_eq!(
        before.attrs.get("by").map(|v| v.to_string()).as_deref(),
        Some("a")
    );
    assert_eq!(attr(&s, "a/notes.md", "by").as_deref(), Some("c"));
}

#[test]
fn mod18_shared_path_keeps_the_last_update() {
    let (mut s, mut ev) = started();
    pass(&mut s);
    for call in ["ub", "uc"] {
        ev.extend(enter(&mut s, call));
        ev.extend(pass(&mut s));
    }
    // 先頭の `started` の分に加え、各実体の分も集めるため、ログ全体から数える。
    let ir = s.ir().clone();
    let all = changed(&ir, &s.state().log);
    let owners: Vec<Root> = all
        .iter()
        .filter(|(_, p)| p == "shared/README.md")
        .map(|(r, _)| r.clone())
        .collect();
    assert_eq!(owners.len(), 4, "{owners:?}");
    let want: BTreeSet<Root> = [
        root_of(&uid("main/u0")),
        Root("m@a".into()),
        Root("m@b".into()),
        Root("m@c".into()),
    ]
    .into();
    assert_eq!(owners.into_iter().collect::<BTreeSet<_>>(), want);
    assert_eq!(attr(&s, "shared/README.md", "status").as_deref(), Some("c"));
}

#[test]
fn mod18_restore_reverts_shared_path_the_owner_wrote() {
    let (mut s, _) = started();
    pass(&mut s);
    enter(&mut s, "ub");
    let cp = save(&mut s);
    assert_eq!(s.state().checkpoints[&cp].owner, Some(Root("m@b".into())));
    pass(&mut s);
    enter(&mut s, "uc");
    pass(&mut s);
    assert_eq!(attr(&s, "shared/README.md", "status").as_deref(), Some("c"));
    s.apply(Op::Control(Control::Restore(cp)));
    // 持ち主と `m@c` の両方が書いた共有の経路は保存の時点（`a`）に戻り、`m@c` の書き込みも消える。
    assert_eq!(attr(&s, "shared/README.md", "status").as_deref(), Some("a"));
    // 持ち主だけが書いた経路は、保存の時点に無いので取り除かれる。
    assert!(!s.state().run.world.contains_key("b/notes.md"));
    // `m@c` だけが書いた経路は、復帰の直前のまま。
    assert_eq!(attr(&s, "a/notes.md", "by").as_deref(), Some("c"));
    assert_eq!(s.state().run.current, Some(uid("m@b/w")));
}

#[test]
fn mod18_restore_keeps_shared_path_the_owner_did_not_write() {
    let (mut s, _) = started();
    choice(&mut s);
    assert_eq!(s.state().run.current, Some(uid("m@a/x")));
    let cp = save(&mut s);
    assert_eq!(s.state().checkpoints[&cp].owner, Some(Root("m@a".into())));
    enter(&mut s, "ub");
    pass(&mut s);
    s.apply(Op::Control(Control::Restore(cp)));
    // 持ち主 `m@a` は保存の後に何も書いていないので、共有の経路でも `b` のまま残る。
    assert_eq!(attr(&s, "shared/README.md", "status").as_deref(), Some("b"));
    assert_eq!(attr(&s, "b/notes.md", "by").as_deref(), Some("b"));
    assert_eq!(attr(&s, "a/notes.md", "by").as_deref(), Some("a"));
    assert_eq!(s.state().run.current, Some(uid("m@a/x")));
}

#[test]
fn mod18_testdata_rounds_write_disjoint_paths() {
    let ir =
        load_ir(&compile_dir(&testdata("module-reuse-rounds/test-data/pack")).unwrap()).unwrap();
    let (r1, _) = round_roots(&ir, "s-r1");
    let (r2, _) = round_roots(&ir, "s-r2");
    let mut s = Session::from_state(ir.clone(), SessionState::default());
    run_plan(&mut s, &round1_plan(&ir));
    run_plan(&mut s, &begin_round2_plan());
    run_plan(&mut s, &round2_plan(&ir));
    let paths = |roots: &[Root]| -> BTreeSet<String> {
        changed(&ir, &s.state().log)
            .into_iter()
            .filter(|(r, _)| roots.contains(r))
            .map(|(_, p)| p)
            .collect()
    };
    let (p1, p2) = (paths(&r1), paths(&r2));
    println!("OBS p1={p1:?} p2={p2:?}");
    assert!(!p1.is_empty() && !p2.is_empty());
    assert!(p1.is_disjoint(&p2), "{:?}", p1.intersection(&p2));
}
