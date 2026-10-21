//! 複数の対象を 1 つずつ分類する確認を、Step に分けて書ける（足す語は無い）ことの固定（TASK-63 1/2、PACK-33・D-69）。
//!
//! 決定（D-69、2026-10-21）は「`answer` に複数の問いを持たせる語は足さず、対象ごとに Step を分けて書く」である。
//! 本試験は、実パック（`tests/data/split-answer-pack`。3 枚のカードを `card-a`・`card-b`・`card-c` の 3 Step に分けた形）で次を固定する。
//!
//! - 新しい語彙の版を宣言しなくても（`pack.vocab: '1.3'`。`answer` の `label` の必須化〔PACK-37〕で、1.3 未満では書けない）検査を通り、IR になる。名前つき拡張点も使わない
//! - 1 枚ごとの正誤が、Step ごとの確認の結果として別々に取れる（判定の表と、セッションの通し）
//! - 対照として、3 枚をまとめた 1 つの `answer` の Step では記録が 1 件しかできず、1 枚ずつの正誤を区別できない
//!   （PACK-33 の「決定の前の現状の観察」に当たる）
//!
//! - `fandhe-pack validate` の CLI は TASK-403（#647）まで未実装（終了コード 2）のため、検査は `validate_dir` と、同じ読み込みを通る
//!   `compile`・`vocab` の CLI で代える。TASK-403 の結線後は CLI の `validate` に置き換えてよい
//! - `Event` から `CheckObservation` への写像は TASK-304 の担当で未実装のため、`choose` が代用する
//!   （`crates/core/tests/confirm_record_boundary.rs` と同じ規則）
//! - 断定しないこと: 学習目標の数え方（L4 など。spec の TASK-3）、入力のない確認だけの Step（TASK-63 2/2）、画面の表示
//!
//! 呼び出し元は CI の `make test` のみである。試験では `expect` を使ってよい。

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::Tmp;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::lo_check::{
    derive_records, CheckMethod, CheckObservation, CheckTarget, FirstAnswer, LoCheckRecord,
    Reselection,
};
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::select_judge::{judge_confirm, ConfirmVerdict};
use fandhe_workshop_core::session::{RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;

const KINDS: [&str; 3] = ["kind-defect", "kind-look", "kind-wish"];
/// カードの Step ID と、その正答の選択肢 ID。
const CARDS: [(&str, &str); 3] = [
    ("card-a", "kind-defect"),
    ("card-b", "kind-look"),
    ("card-c", "kind-wish"),
];

fn pack_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/split-answer-pack")
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
fn pack33_split_pack_validates_without_newer_vocab() {
    let dir = pack_dir();
    let diags = validate_dir(&dir);
    assert!(diags.is_empty(), "{diags:?}");
    assert!(compile_dir(&dir).is_ok());
}

#[test]
fn pack33_cli_compile_and_vocab_accept_split_pack() {
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
    for (step, _) in CARDS {
        // 第 4 欄（extension_points）が空 = 名前つき拡張点を使っていない。
        assert!(out.contains(&format!("c1,{step},step,\n")), "{out}");
    }
    assert!(
        out.contains("(ii) 名前つき拡張点を使って書けた Step: 0"),
        "{out}"
    );
}

#[test]
fn pack33_each_card_has_its_own_verdict() {
    let ir = ir_of(&pack_dir());
    for (step_id, correct) in CARDS {
        let uid = uid_of(&ir, step_id);
        let step = &ir.nodes.iter().find(|n| n.uid == uid).expect("node").step;
        let mut confirmed = Vec::new();
        for kind in KINDS {
            match judge_confirm(step, &Input::Choice(kind.into())).expect("judge") {
                ConfirmVerdict::Confirmed => confirmed.push(kind),
                ConfirmVerdict::WrongAnswer { text, .. } => {
                    assert_eq!(text, "rj-wrong-kind", "{step_id}/{kind}");
                }
                other => panic!("{step_id}/{kind}: {other:?}"),
            }
        }
        // 正答はカードごとにちょうど 1 つで、3 枚で互いに違う。1 枚ごとの正誤が別々の判定として取れる。
        assert_eq!(confirmed, vec![correct], "{step_id}");
    }
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

/// `uid` の Step 向けの選択を送り、記録に使う観測を返す。
///
/// 送る前が `Accepted`（確認の段階）のときだけ回答として数える。正誤は戻りに `Achieved` があるかで決める
/// （`confirm_record_boundary.rs` の `choose` と同じ規則）。
fn choose(s: &mut Session, uid: &str, choice: &str) -> Option<CheckObservation> {
    let confirming = status(s, uid) == StepStatus::Accepted;
    let events = s.apply(Op::Input(Input::Choice(choice.into()))).to_vec();
    confirming.then(|| CheckObservation::Answered {
        step: uid.into(),
        correct: achieved_count(&events) > 0,
    })
}

fn targets(ir: &Ir, step_ids: &[&str]) -> Vec<CheckTarget> {
    step_ids
        .iter()
        .map(|id| {
            let uid = uid_of(ir, id);
            let node = ir.nodes.iter().find(|n| n.uid == uid).expect("node");
            CheckTarget {
                step: uid,
                lo: node.step.lo.clone(),
                method: CheckMethod::Answer,
            }
        })
        .collect()
}

#[test]
fn pack33_session_records_per_card_correctness() {
    let ir = ir_of(&pack_dir());
    let ids: Vec<&str> = CARDS.iter().map(|(id, _)| *id).collect();
    let uids: Vec<String> = ids.iter().map(|id| uid_of(&ir, id)).collect();
    let tg = targets(&ir, &ids);
    let mut s = Session::from_state(ir, entered(&uids[0]));
    let mut obs = Vec::new();

    // card-a: 受付 → 正答。
    assert_eq!(choose(&mut s, &uids[0], "kind-defect"), None);
    assert_eq!(status(&s, &uids[0]), StepStatus::Accepted);
    obs.extend(choose(&mut s, &uids[0], "kind-defect"));
    assert_eq!(status(&s, &uids[0]), StepStatus::Achieved);
    assert_eq!(status(&s, &uids[1]), StepStatus::Entered);

    // card-b: 受付 → 誤答（受付済みのまま）→ 正答。
    assert_eq!(choose(&mut s, &uids[1], "kind-defect"), None);
    assert_eq!(status(&s, &uids[1]), StepStatus::Accepted);
    let before = achieved_count(s.log());
    obs.extend(choose(&mut s, &uids[1], "kind-wish"));
    assert_eq!(status(&s, &uids[1]), StepStatus::Accepted);
    assert_eq!(achieved_count(s.log()), before);
    obs.extend(choose(&mut s, &uids[1], "kind-look"));
    assert_eq!(status(&s, &uids[1]), StepStatus::Achieved);
    assert_eq!(status(&s, &uids[2]), StepStatus::Entered);

    // card-c: 受付 → 正答。
    assert_eq!(choose(&mut s, &uids[2], "kind-look"), None);
    assert_eq!(status(&s, &uids[2]), StepStatus::Accepted);
    obs.extend(choose(&mut s, &uids[2], "kind-wish"));
    assert_eq!(status(&s, &uids[2]), StepStatus::Achieved);

    let records = derive_records(&tg, &obs);
    assert_eq!(records.len(), 3);
    let firsts: Vec<FirstAnswer> = records.iter().map(|r| r.first_answer).collect();
    assert_eq!(
        firsts,
        vec![
            FirstAnswer::Correct,
            FirstAnswer::Wrong,
            FirstAnswer::Correct
        ]
    );
    assert_eq!(records[1].reselection, Reselection::Correct);
}

/// 対照: 3 枚の分け方をまとめた組み合わせを 1 つの `answer` の選択肢にした 1 Step の教材。
fn merged_pack() -> Tmp {
    let t = Tmp::new("pack33-merged");
    t.put(
        "workshop.yaml",
        concat!(
            "pack: {id: merged-answer-pack, title: まとめた分類, version: '0.0.1', ir_format: 1, guidance: A, vocab: '1.3'}\n",
            "rejections:\n",
            "  rj-wrong-kind: {category: not_in_this_step, message: その分類ではありません。, next_action: もう一度送ります。}\n",
        )
        .as_bytes(),
    );
    t.put("chapters/c1/a.md", b"# a\n");
    let mut ch = String::from(
        "id: c1\ntitle: まとめた分類\nstarts: [{id: s1, node: card-all}]\nsteps:\n  - id: card-all\n    title: All\n    body: a.md\n    lo: [lo-sort]\n",
    );
    ch.push_str("    input: {kind: choice, options: [{id: combo-1, label: 組 1}, {id: combo-2, label: 組 2}]}\n");
    ch.push_str("    rules:\n");
    for c in ["combo-1", "combo-2"] {
        ch.push_str(&format!(
            "      - {{id: r-{c}, on: {{option: {c}}}, outcome: {{kind: accept}}}}\n"
        ));
    }
    ch.push_str("    confirm: {kind: answer, on_wrong: rj-wrong-kind, options: [{id: combo-1, label: 組 1, correct: true}, {id: combo-2, label: 組 2, correct: false}]}\n");
    ch.push_str("    next: [{to: end}]\n  - {id: end, title: End, body: a.md, final: true}\n");
    t.put("chapters/c1/chapter.yaml", ch.as_bytes());
    t
}

#[test]
fn pack33_merged_step_yields_a_single_record() {
    // 決定の前の形の観察: まとめた 1 Step も検査を通るが、記録は 1 件で、3 枚ずつの正誤は区別できない。
    let t = merged_pack();
    assert!(validate_dir(&t.0).is_empty());
    let ir = ir_of(&t.0);
    let uid = uid_of(&ir, "card-all");
    let tg = targets(&ir, &["card-all"]);
    let mut s = Session::from_state(ir, entered(&uid));
    let mut obs = Vec::new();
    assert_eq!(choose(&mut s, &uid, "combo-1"), None);
    obs.extend(choose(&mut s, &uid, "combo-2"));
    obs.extend(choose(&mut s, &uid, "combo-1"));
    let records: Vec<LoCheckRecord> = derive_records(&tg, &obs);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].first_answer, FirstAnswer::Wrong);
}
