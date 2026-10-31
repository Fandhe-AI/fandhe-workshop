//! 2 周分の履歴の大きさを 3 条件で測り、どれも 5MB 以下であることを確かめる（TASK-336。SAVE-26。記録は `docs/testing.md` の 5.344）。
//!
//! TASK-216（`history_size_rounds.rs`）の計画（誤入力なしの 2 周分）に、誤入力と付加操作を差し込んで 3 条件を作る。
//! 本試験は SAVE-26 の判定の担当で、`history_size_rounds.rs`（`HISTORY-SIZE ` の 1 行を `tools/measure/history-size.sh` が前提にする）とは
//! 別の試験バイナリに置き、出力の接頭辞も `HISTORY-SIZE-CONDITIONS ` に分ける。
//!
//! 条件の定義:
//! - A: 計画どおり。チェックポイントは各周の終わりの 2 件。
//! - B: A に付加操作を足す。誘導の手 6 手ごとに `Control::Save`、4 手ごとに `Control::OpenHint`（開けるときだけ）、12 手ごとに下書き。
//! - C: B に加えて、送った非 `Save` の操作（誘導の手・誤入力・`OpenHint`・`GoTo`・`Begin`）の直後に `Control::Save` を 1 回送る。下書きの記録は操作ではないので対象外。
//!
//! 誘導の手は `Op::Input(_)` と `Tick`。`Control` は未実装の部品の代用か実行環境の操作で、学習者の手ではない。
//! 誤入力は「誘導の手 1 つにつき 0.3 回」（`synth-inputs.md` §1）。手の直前に 0〜999 を引き、300 未満なら 6 候補から一様に 1 つ選んで先に送る。
//!
//! 乱数（基点）: `proptest-seed.txt`（32 バイト）の末尾 8 バイトを `serial = (条件番号 << 8) | 種の番号` の big-endian で置き換え、
//! `TestRng::from_seed(ChaCha, ..)` を作る。引くのは `RngCore::next_u64` だけで（rand の版で値が揺れる `gen_range` 系は使わない）、
//! 誘導の手ごとに判定用に 1 回、入れると決めたときだけ候補用に 1 回引く。条件 A=1・B=2・C=3、種の番号 0〜4。
//!
//! 閾値: SAVE-26 の 5MB は `5 * 1024 * 1024 = 5,242,880` バイト（`synth-inputs.md` §3）。`contract::limits::MAX_HISTORY_BYTES`
//! （10 MiB。読み込みの上限 SAFE-18）とは別の値で、ここでは使わない。
//! `synth-inputs.md` は「判定は A だけ。B・C は記録」とするが、SAVE-26 は 3 条件すべてを求めるため、SAVE-26 を正として 15 本すべてを判定する。
//!
//! 限界: `GoTo` で飛ばす場所が 5 か所ある（1 周目 3・2 周目 2。`common/rounds.rs`）。全章の履歴は TASK-337（`crates/web/tests/history_size_all_chapters.rs`。章の形の写しでの測定）の担当。

mod common;
#[path = "common/history_size.rs"]
mod history_size;
#[path = "common/rounds.rs"]
mod rounds;
#[path = "common/sha256.rs"]
mod sha256;

use std::sync::OnceLock;

use common::testdata;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::hint_select::current_hints;
use fandhe_workshop_core::history_envelope::SessionEnvelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::Session;
use fandhe_workshop_pack::compile::compile_dir;
use history_size::{measure, HistorySize};
use proptest::prelude::Rng;
use proptest::test_runner::{RngAlgorithm, TestRng};
use rounds::{begin_round2_plan, goto_count, round1_plan, round2_plan, run_plan, PlanStep};

const SOURCE: &str = "testdata/module-reuse-rounds/test-data/pack";
const SAVE26_THRESHOLD_BYTES: u64 = 5 * 1024 * 1024;
const SEEDS: u64 = 5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cond {
    A,
    B,
    C,
}

impl Cond {
    const ALL: [Cond; 3] = [Cond::A, Cond::B, Cond::C];
    fn no(self) -> u64 {
        match self {
            Cond::A => 1,
            Cond::B => 2,
            Cond::C => 3,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Cond::A => "A",
            Cond::B => "B",
            Cond::C => "C",
        }
    }
}

fn hex(d: &[u8]) -> String {
    sha256::hex(&sha256::sha256(d))
}

fn save() -> PlanStep {
    PlanStep::Op(Op::Control(Control::Save))
}

/// 基の計画（TASK-216 の条件 A の形）: 1 周目 → 保存 → 2 周目の開始 → 2 周目 → 保存。
fn base_plan(ir: &Ir) -> Vec<PlanStep> {
    let mut p = round1_plan(ir);
    p.push(save());
    p.extend(begin_round2_plan());
    p.extend(round2_plan(ir));
    p.push(save());
    p
}

fn wrong_input(i: u64) -> Input {
    match i {
        0 => Input::Command("/advance-phase".into()),
        1 => Input::Command("/no-such-command".into()),
        2 => Input::Command("/new-idea".into()),
        3 => Input::Choice("no-such-option".into()),
        4 => Input::Ui {
            view: "v-files".into(),
            path: "nonexistent/path".into(),
        },
        _ => Input::Text("短".into()),
    }
}

fn make_rng(cond: Cond, seed_idx: u64) -> (TestRng, u64) {
    let raw = std::fs::read(testdata(
        "save-restore-versions/test-data/proptest-seed.txt",
    ))
    .expect("seed file");
    assert_eq!(raw.len(), 32, "seed file must be 32 bytes");
    let serial = (cond.no() << 8) | seed_idx;
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&raw);
    seed[24..].copy_from_slice(&serial.to_be_bytes());
    (TestRng::from_seed(RngAlgorithm::ChaCha, &seed), serial)
}

struct Outcome {
    cond: Cond,
    seed_idx: u64,
    serial: u64,
    env: SessionEnvelope,
    bytes: Vec<u8>,
    events: Vec<Event>,
    current: String,
    guided_hands: u64,
    wrong_inputs: u64,
    hints_opened: u64,
    drafts: u64,
    goto_substitutions: usize,
    /// 誤入力の前後で位置・世代・処理中・完了・世界が動いた手の番号と候補の番号（空であるべき）。
    wrong_moved: Vec<(u64, u64)>,
    /// 誤入力で `Changed` が出た手の番号と候補の番号（空であるべき）。
    wrong_changed: Vec<(u64, u64)>,
}

fn progress_key(rs: &RecordedSession) -> String {
    let r = &rs.session().state().run;
    format!(
        "{:?}|{:?}|{:?}|{:?}|{:?}",
        r.current, r.generation, r.busy, r.completed, r.world
    )
}

/// 条件 C で、非 `Save` の操作の直後に `Save` を送る。
fn after_op(rs: &mut RecordedSession, cond: Cond, events: &mut Vec<Event>) {
    if cond == Cond::C {
        events.extend(rs.apply(Op::Control(Control::Save)).to_vec());
    }
}

fn run_condition(ir: &Ir, cond: Cond, seed_idx: u64) -> Outcome {
    let (mut rng, serial) = make_rng(cond, seed_idx);
    let plan = base_plan(ir);
    let mut rs = RecordedSession::new(Session::new(ir.clone()), RecordedSession::first_start(ir));
    let mut events: Vec<Event> = Vec::new();
    let (mut n, mut wrong, mut hints, mut drafts) = (0u64, 0u64, 0u64, 0u64);
    let mut wrong_moved = Vec::new();
    let mut wrong_changed = Vec::new();

    for step in &plan {
        let guided = matches!(step, PlanStep::Op(Op::Input(_)) | PlanStep::Tick);
        if guided {
            n += 1;
            if rng.next_u64() % 1000 < 300 {
                let cand = rng.next_u64() % 6;
                let before = progress_key(&rs);
                let ev = rs.apply(Op::Input(wrong_input(cand))).to_vec();
                if progress_key(&rs) != before {
                    wrong_moved.push((n, cand));
                }
                if ev.iter().any(|e| matches!(e, Event::Changed { .. })) {
                    wrong_changed.push((n, cand));
                }
                events.extend(ev);
                wrong += 1;
                after_op(&mut rs, cond, &mut events);
            }
        }
        events.extend(run_plan(&mut rs, std::slice::from_ref(step)));
        let non_save = !matches!(step, PlanStep::Op(Op::Control(Control::Save)))
            && !matches!(step, PlanStep::ExpectCurrentSuffix(_));
        if non_save {
            after_op(&mut rs, cond, &mut events);
        }
        if guided && cond != Cond::A {
            if n % 6 == 0 {
                events.extend(rs.apply(Op::Control(Control::Save)).to_vec());
            }
            if n % 4 == 0 {
                let st = rs.session().state();
                let can = match &st.run.current {
                    Some(cur) => {
                        let total = current_hints(ir, &st.run).len() as u32;
                        st.hints_opened.get(cur).copied().unwrap_or(0) < total
                    }
                    None => false,
                };
                if can {
                    events.extend(rs.apply(Op::Control(Control::OpenHint)).to_vec());
                    hints += 1;
                    after_op(&mut rs, cond, &mut events);
                }
            }
            if n % 12 == 0 {
                rs.draft(format!("下書き {n}"));
                drafts += 1;
            }
        }
    }
    let current = rs.session().state().run.current.clone().expect("current").0;
    let env = rs.envelope_for_save().expect("envelope");
    let bytes = export_bytes(&env).expect("export");
    Outcome {
        cond,
        seed_idx,
        serial,
        env,
        bytes,
        events,
        current,
        guided_hands: n,
        wrong_inputs: wrong,
        hints_opened: hints,
        drafts,
        goto_substitutions: goto_count(&plan),
        wrong_moved,
        wrong_changed,
    }
}

struct Fixture {
    ir: Ir,
    ir_bytes: Vec<u8>,
    outs: Vec<Outcome>,
    base_changed: u64,
}

fn fixture() -> &'static Fixture {
    static F: OnceLock<Fixture> = OnceLock::new();
    F.get_or_init(|| {
        let ir_bytes =
            compile_dir(&testdata("module-reuse-rounds/test-data/pack")).expect("compile");
        let ir = load_ir(&ir_bytes).expect("load_ir");
        let mut outs = Vec::new();
        for c in Cond::ALL {
            for s in 0..SEEDS {
                outs.push(run_condition(&ir, c, s));
            }
        }
        // 誤入力なしの基の計画の `Changed` の件数（誤入力・付加操作は世界を変えない）。
        let mut rs =
            RecordedSession::new(Session::new(ir.clone()), RecordedSession::first_start(&ir));
        let ev = run_plan(&mut rs, &base_plan(&ir));
        let base_changed = ev
            .iter()
            .filter(|e| matches!(e, Event::Changed { .. }))
            .count() as u64;
        Fixture {
            ir,
            ir_bytes,
            outs,
            base_changed,
        }
    })
}

fn size_of(o: &Outcome) -> HistorySize {
    measure(&o.env, &o.bytes)
}

fn get(f: &Fixture, c: Cond, s: u64) -> &Outcome {
    f.outs
        .iter()
        .find(|o| o.cond == c && o.seed_idx == s)
        .expect("outcome")
}

#[test]
fn three_conditions_within_5mb() {
    let f = fixture();
    let first = &f.outs[0];
    println!(
        "\nHISTORY-SIZE-CONDITIONS-PACK {}",
        serde_json::json!({
            "id": f.ir.pack.id,
            "version": f.ir.pack.version,
            "nodes": f.ir.nodes.len(),
            "ir_sha256": hex(&f.ir_bytes),
            "content_hash": first.env.versions.content_hash,
            "source": SOURCE,
            "threshold_bytes": SAVE26_THRESHOLD_BYTES,
        })
    );
    for o in &f.outs {
        let size = size_of(o);
        assert_eq!(
            o.current,
            "main/r2-impl",
            "{} seed {}",
            o.cond.name(),
            o.seed_idx
        );
        assert!(
            size.total_bytes <= SAVE26_THRESHOLD_BYTES,
            "SAVE-26 exceeded: cond {} seed {} total_bytes {} checkpoints {} checkpoint_bytes {}",
            o.cond.name(),
            o.seed_idx,
            size.total_bytes,
            size.checkpoints,
            size.checkpoint_bytes
        );
        assert_eq!(
            size.changed_entries, f.base_changed,
            "Changed unaffected by extras"
        );
        let from_events = o
            .events
            .iter()
            .filter(|e| matches!(e, Event::Changed { .. }))
            .count() as u64;
        assert_eq!(size.changed_entries, from_events);
        match o.cond {
            // 手動の 2 件に章の終わりの自動保存 2 件が加わる（SAVE-31、TASK-339 1/2）。
            Cond::A => assert_eq!(size.checkpoints, 4),
            Cond::B => assert!(size.checkpoints > 2),
            Cond::C => assert!(size.checkpoints > 2),
        }
        let line = serde_json::json!({
            "condition": o.cond.name(),
            "seed": o.seed_idx,
            "seed_serial": o.serial,
            "ops_sha256": hex(&serde_json::to_vec(&o.env.ops).expect("ops json")),
            "guided_hands": o.guided_hands,
            "wrong_inputs": o.wrong_inputs,
            "hints_opened": o.hints_opened,
            "drafts": o.drafts,
            "goto_substitutions": o.goto_substitutions,
            "total_bytes": size.total_bytes,
            "ops": size.ops,
            "ops_bytes": size.ops_bytes,
            "snapshot_bytes": size.snapshot_bytes,
            "log_entries": size.log_entries,
            "changed_entries": size.changed_entries,
            "changed_bytes": size.changed_bytes,
            "changed_ratio_ppm": size.changed_ratio_ppm(),
            "checkpoints": size.checkpoints,
            "checkpoint_bytes": size.checkpoint_bytes,
            "threshold_ratio_ppm": size.total_bytes * 1_000_000 / SAVE26_THRESHOLD_BYTES,
        });
        println!("\nHISTORY-SIZE-CONDITIONS {line}");
    }

    // 履歴を大きくする要因はチェックポイントで、`Changed` の割合は小さい。
    for s in 0..SEEDS {
        let (a, b, c) = (
            size_of(get(f, Cond::A, s)),
            size_of(get(f, Cond::B, s)),
            size_of(get(f, Cond::C, s)),
        );
        assert!(
            a.total_bytes < b.total_bytes && b.total_bytes < c.total_bytes,
            "seed {s}"
        );
        assert!(
            a.checkpoint_bytes < b.checkpoint_bytes && b.checkpoint_bytes < c.checkpoint_bytes,
            "seed {s}"
        );
        assert!(c.checkpoint_bytes > c.changed_bytes, "seed {s}");
        assert!(c.changed_ratio_ppm() < a.changed_ratio_ppm(), "seed {s}");
        let per = (c.total_bytes - b.total_bytes) / (c.checkpoints - b.checkpoints);
        println!("\nHISTORY-SIZE-CONDITIONS-PER-CHECKPOINT seed {s} bytes {per}");
    }
}

#[test]
fn wrong_inputs_do_not_move_progress() {
    let f = fixture();
    for o in &f.outs {
        assert!(
            o.wrong_inputs > 0,
            "cond {} seed {}",
            o.cond.name(),
            o.seed_idx
        );
        assert!(
            o.wrong_moved.is_empty(),
            "wrong input moved progress: cond {} seed {} (hand, candidate) {:?}",
            o.cond.name(),
            o.seed_idx,
            o.wrong_moved
        );
        assert!(
            o.wrong_changed.is_empty(),
            "wrong input produced Changed: cond {} seed {} (hand, candidate) {:?}",
            o.cond.name(),
            o.seed_idx,
            o.wrong_changed
        );
    }
}

#[test]
fn same_seed_same_bytes_and_round_trip() {
    let f = fixture();
    for c in Cond::ALL {
        let again = run_condition(&f.ir, c, 0);
        let o = get(f, c, 0);
        assert_eq!(again.bytes, o.bytes, "cond {}", c.name());
        let value = parse_history_json(&o.bytes).expect("stages 1-3");
        assert_eq!(parse_history_envelope(value).expect("stages 4-5"), o.env);
    }
}
