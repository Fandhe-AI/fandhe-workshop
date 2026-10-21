//! 誤答の拒否に前後比較のビューを結ぶ語は足さず、拒否文で前後の切り替えを案内する形の固定（TASK-64 2/2、PACK-36・D-69）。
//!
//! 決定（D-69、2026-10-21）は「語は足さない。誤答の拒否文（`message`・`next_action`）で、左エリアの前後の切り替え（ART-2）を
//! 案内する。誤答のときに比較を自動で見せないことは限界として受け入れる」である。
//! 本試験は、実パック（`tests/data/wrong-answer-compare-pack`）で次を固定する。
//!
//! - 新しい語彙の版を宣言しなくても（`pack.vocab: '1.0'`）検査を通り、IR になる。名前つき拡張点も使わない
//! - 誤答が案内つきの拒否になり、Step は受付のまま達成にならない。誤答で出るイベントは判定と入力の記録だけである
//! - 対照として、`confirm.answer` に比較のビューを結ぶ欄を足すと `E-PARSE` になる（語を足していないこと）
//!
//! - `fandhe-pack validate` の CLI は TASK-403（#647）まで未実装（終了コード 2）のため、検査は `validate_dir` と、同じ読み込みを通る
//!   `compile`・`vocab` の CLI で代える。TASK-403 の結線後は CLI の `validate` に置き換えてよい
//! - 断定しないこと: 画面での前後の切り替えの見え方（ART-19。画面のタスクの持ち分）
//!
//! 呼び出し元は CI の `make test` のみである。試験では `expect` を使ってよい。

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::Tmp;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::Judgment;
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::rejection_text::{resolve_rejection, RejectionSource};
use fandhe_workshop_core::select_judge::{judge_confirm, ConfirmVerdict};
use fandhe_workshop_core::session::{RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::text_fill::filled_text;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::DiagCode;

const GUIDE: &str = "左の前後の切り替えで変更の前と後を見比べてから、選び直します。";

fn pack_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/wrong-answer-compare-pack")
}

fn ir_of(dir: &Path) -> Ir {
    let bytes = compile_dir(dir).unwrap_or_else(|d| panic!("compile {dir:?}: {d:?}"));
    load_ir(&bytes).expect("the engine loads the IR")
}

/// Step ID から実体 ID（uid）を引く。uid は `<章 ID>/<Step ID>` ではないので、決め打ちにしない。
fn uid_of(ir: &Ir, step_id: &str) -> String {
    ir.nodes
        .iter()
        .find(|n| n.step.id == step_id)
        .unwrap_or_else(|| panic!("step {step_id} がある"))
        .uid
        .clone()
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
}

#[test]
fn pack36_guide_pack_validates_without_newer_vocab() {
    let dir = pack_dir();
    let diags = validate_dir(&dir);
    assert!(diags.is_empty(), "{diags:?}");
    assert!(compile_dir(&dir).is_ok());
}

#[test]
fn pack36_cli_compile_and_vocab_accept_guide_pack() {
    let dir = pack_dir();
    // CLI の validate は TASK-403（#647）まで未実装のため、同じ読み込みを通る compile で代える。
    let o = bin()
        .arg("compile")
        .arg(&dir)
        .output()
        .expect("run compile");
    assert_eq!(o.status.code(), Some(0));
    assert!(!o.stdout.is_empty());
    assert!(
        o.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );

    let o = bin().arg("vocab").arg(&dir).output().expect("run vocab");
    assert_eq!(o.status.code(), Some(0));
    let out = String::from_utf8(o.stdout).expect("utf8");
    assert!(
        out.contains("(ii) 名前つき拡張点を使って書けた Step: 0"),
        "{out}"
    );
}

#[test]
fn pack36_wrong_answer_resolves_to_guide_text() {
    let ir = ir_of(&pack_dir());
    let uid = uid_of(&ir, "pick");
    let step = &ir.nodes.iter().find(|n| n.uid == uid).expect("node").step;
    match judge_confirm(step, &Input::Choice("pick-alpha".into())).expect("judge") {
        ConfirmVerdict::WrongAnswer { text, .. } => assert_eq!(text, "rj-unchanged"),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        judge_confirm(step, &Input::Choice("pick-beta".into())).expect("judge"),
        ConfirmVerdict::Confirmed
    ));
    let r = resolve_rejection(&ir, step, RejectionSource::Rule("rj-unchanged")).expect("resolve");
    assert_eq!(r.next_action.as_str(), GUIDE);
    assert!(r.next_action.as_str().contains("前後の切り替え"));
}

fn entered(uid: &str) -> SessionState {
    let u = StepUid(uid.to_string());
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), StepStatus::Entered);
    let mut run = RunState {
        generation: Generation(1),
        current: Some(u.clone()),
        ..RunState::default()
    };
    run.slices.insert(u.root(), slice);
    SessionState {
        run,
        ..SessionState::default()
    }
}

fn status(s: &Session, uid: &str) -> StepStatus {
    let u = StepUid(uid.to_string());
    s.state()
        .run
        .slices
        .get(&u.root())
        .and_then(|sl| sl.status.get(&u))
        .copied()
        .unwrap_or_default()
}

fn achieved_count(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Achieved { .. }))
        .count()
}

#[test]
fn pack36_session_shows_guide_and_stays_on_step() {
    let ir = ir_of(&pack_dir());
    let uid = uid_of(&ir, "pick");
    let mut s = Session::from_state(ir.clone(), entered(&uid));

    // 受付 → 誤答。
    s.apply(Op::Input(Input::Choice("pick-alpha".into())));
    assert_eq!(status(&s, &uid), StepStatus::Accepted);
    let before = s.log().len();
    let achieved = achieved_count(s.log());
    let events = s
        .apply(Op::Input(Input::Choice("pick-alpha".into())))
        .to_vec();

    // 誤答でも受付のまま。達成は増えない。
    assert_eq!(status(&s, &uid), StepStatus::Accepted);
    assert_eq!(achieved_count(s.log()), achieved);
    assert!(s.log().len() > before);

    // 案内の文が判定の結果として出る。
    let judgment = events
        .iter()
        .find_map(|e| match e {
            Event::Judged { judgment, .. } => Some(judgment.clone()),
            _ => None,
        })
        .expect("judged");
    assert!(matches!(judgment, Judgment::Rejected(_)), "{judgment:?}");
    let filled = filled_text(
        &ir,
        &s.state().run,
        &Input::Choice("pick-alpha".into()),
        &judgment,
    )
    .expect("fill")
    .expect("text");
    assert_eq!(filled.text, "rj-unchanged");
    assert_eq!(filled.message, "その箇所は変わっていません。");
    assert_eq!(filled.next_action.as_str(), GUIDE);

    // 比較を自動で見せない: 誤答のイベントは、判定と入力の記録などで、世界状態やビューを変えるものを含まない。
    for e in &events {
        assert!(
            matches!(e, Event::Judged { .. } | Event::InputReceived { .. }),
            "{e:?}"
        );
    }

    // 正答で達成になる。
    s.apply(Op::Input(Input::Choice("pick-beta".into())));
    assert_eq!(status(&s, &uid), StepStatus::Achieved);
    assert_eq!(achieved_count(s.log()), achieved + 1);
}

#[test]
fn pack36_binding_field_on_answer_is_rejected() {
    // 対照: 拒否に比較のビューを結ぶ欄は無い。`answer` の欄は閉じた集合で、足すと E-PARSE になる。
    let t = Tmp::new("pack36-binding");
    let src = pack_dir();
    for f in ["workshop.yaml", "chapters/c1/a.md"] {
        t.put(f, &std::fs::read(src.join(f)).expect("read"));
    }
    let ch = std::fs::read_to_string(src.join("chapters/c1/chapter.yaml")).expect("read");
    let bound = ch.replace(
        "confirm: {kind: answer, on_wrong: rj-unchanged,",
        "confirm: {kind: answer, compare_view: v-diff, on_wrong: rj-unchanged,",
    );
    assert_ne!(bound, ch, "置換が効いている");
    t.put("chapters/c1/chapter.yaml", bound.as_bytes());
    let diags = validate_dir(&t.0);
    assert!(
        diags.iter().any(|d| d.code() == DiagCode::Parse),
        "{diags:?}"
    );
    assert!(compile_dir(&t.0).is_err());
}
