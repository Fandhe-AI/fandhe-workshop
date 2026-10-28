//! 全章を通した履歴の大きさを、章の形の写しで測り 5MB 以下を確かめる（TASK-337。SAVE-27。記録は `docs/testing.md` の 5.345）。
//!
//! 解釈（最大の限界）: 本物の CH-0〜CH-15 の教材は本リポジトリに無い。ここで通すのは形だけの写しである。
//! - S1: `tests/data/ch0_3/ir.json`（CH-0〜CH-3 の形の 20 Step）
//! - S2: `tests/data/ch4_9/ir.json`（CH-4〜CH-9 の形の 17 Step・6 章）
//! - S3: `module-reuse-rounds` の 2 周分（2 周目を CH-10 以降の代役に読み替える。CH-11〜CH-15 に当たる写しは無い）
//!
//! 本物の全章での測定は未実施で、教材の実装の後に、この試験の IR と操作列を差し替えて流し直す。数値を本物の全章の値として読まない。
//!
//! 条件（乱数は使わない。決定的）:
//! - A: 章の境目を越えた直後と区間の終わりに `Control::Save` を 1 回（章の終わりの自動保存 SAVE-31 の代用。SAVE-31 は未実装）。
//! - C: A に加えて、送った操作（保存を除く）の直後に毎回 `Control::Save`（チェックポイントの数に比例して増える上限側）。
//!
//! 各区間に 20,000 コードポイントの自然文を 1 回、画面と同じ経路（下書き → 送信）で差し込む。位置を動かさない手の前で送り、
//! 進行が動かないことと、ちょうど 20,000 コードポイントの送信が `ops` に残ることを確かめる。
//!
//! 閾値は `5 * 1024 * 1024 = 5,242,880` バイト（`MAX_HISTORY_BYTES` の 10 MiB とは別の値）。区間ごとの判定に加え、
//! 本物の 1 本の履歴では後の章のチェックポイントも前の章の世界状態を持つため、合算の目安
//! `Σ total_bytes + (Σ checkpoints + 1) × Σ world_bytes` も判定する（上限側の推定であり、厳密な上限ではない）。
//! 出力には大きさ・件数・ハッシュだけを出し、本文は出さない。試験では `unwrap` を使ってよい。

#[path = "common/ch0_3_journey.rs"]
mod ch0_3_journey;
#[path = "common/ch4_9_journey.rs"]
mod ch4_9_journey;
#[path = "../../pack/tests/common/history_size.rs"]
mod history_size;
#[path = "../../pack/tests/common/rounds.rs"]
mod rounds;
#[path = "../../pack/tests/common/sha256.rs"]
mod sha256;

use std::path::PathBuf;

use fandhe_workshop_contract::limits::MAX_INPUT_CODE_POINTS;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_envelope::SessionEnvelope;
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::history_export::ExportGate;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;
use history_size::{measure, HistorySize};
use rounds::{begin_round2_plan, round1_plan, round2_plan, run_plan, Drive, PlanStep};
use serde_json::Value;

const SAVE27_THRESHOLD_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cond {
    A,
    C,
}

impl Cond {
    const ALL: [Cond; 2] = [Cond::A, Cond::C];
    fn name(self) -> &'static str {
        match self {
            Cond::A => "A",
            Cond::C => "C",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Section {
    S1,
    S2,
    S3,
}

impl Section {
    const ALL: [Section; 3] = [Section::S1, Section::S2, Section::S3];
    fn name(self) -> &'static str {
        match self {
            Section::S1 => "S1-ch0-3",
            Section::S2 => "S2-ch4-9",
            Section::S3 => "S3-rounds",
        }
    }
}

fn hex(d: &[u8]) -> String {
    sha256::hex(&sha256::sha256(d))
}

fn len_of<T: serde::Serialize + ?Sized>(v: &T) -> u64 {
    serde_json::to_vec(v).unwrap().len() as u64
}

/// 20,000 コードポイントの和文。英数字・base64 に似た列は使わない（秘密情報の検査で止まらないように）。
fn long_text() -> String {
    "履歴の大きさを測るための長い入力です。"
        .chars()
        .cycle()
        .take(MAX_INPUT_CODE_POINTS)
        .collect()
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

fn progress_key(h: &EngineHost) -> String {
    let r = &h.model().session().unwrap().state().run;
    format!(
        "{:?}|{:?}|{:?}|{:?}|{:?}",
        r.current, r.generation, r.busy, r.completed, r.world
    )
}

fn save(h: &mut EngineHost) {
    h.update(HostAction::Engine(Op::Control(Control::Save)));
}

/// 位置を動かさない手の前で、20,000 コードポイントの自然文を下書き → 送信で送る。
fn send_long(h: &mut EngineHost) {
    let before = progress_key(h);
    h.update(act("draft", &long_text()));
    h.update(act("submit", ""));
    assert_eq!(progress_key(h), before, "長い入力で進行が動いた");
}

struct Outcome {
    env: SessionEnvelope,
    bytes: Vec<u8>,
    ir_sha256: String,
    pack_id: String,
    long_submit_bytes: u64,
    world_bytes: u64,
    changed_events: u64,
}

fn finish(h: &EngineHost, ir_bytes: &[u8], pack_id: String) -> Outcome {
    let ExportGate::Ready(bytes) = h.export_live() else {
        panic!("書き出しが Ready でない（Held か Empty）");
    };
    let value = parse_history_json(&bytes).expect("stages 1-3");
    let env = parse_history_envelope(value).expect("stages 4-5");
    // ちょうど 20,000 コードポイントの本文が `ops` に残る（切られていない）。
    let ops: Value = serde_json::to_value(&env.ops).unwrap();
    let mut longs = Vec::new();
    collect_long(&ops, &mut longs);
    assert!(
        !longs.is_empty(),
        "20,000 コードポイントの送信が残っていない"
    );
    // 下書きの記録と送信の記録など、ちょうど 20,000 コードポイントの値を持つ要素の JSON のバイト数の合計。
    let long_submit_bytes: u64 = longs.iter().sum();
    let world_bytes = len_of(&env.snapshot.run.world);
    let changed_events = env
        .snapshot
        .log
        .iter()
        .filter(|e| matches!(e, Event::Changed { .. }))
        .count() as u64;
    Outcome {
        env,
        bytes,
        ir_sha256: hex(ir_bytes),
        pack_id,
        long_submit_bytes,
        world_bytes,
        changed_events,
    }
}

/// 文字列がちょうど 20,000 コードポイントの値を含む最小の JSON 片のバイト数を集める。
fn collect_long(v: &Value, out: &mut Vec<u64>) {
    match v {
        Value::String(s) if s.chars().count() == MAX_INPUT_CODE_POINTS => {
            out.push(len_of(v));
        }
        Value::Array(a) => a.iter().for_each(|x| collect_long(x, out)),
        Value::Object(m) => m.values().for_each(|x| collect_long(x, out)),
        _ => {}
    }
}

fn chapter_of(uid: &str) -> &str {
    uid.strip_prefix("main/st-")
        .and_then(|r| r.split('-').next())
        .unwrap_or("")
}

fn host_at(ir_bytes: &[u8], first: &str) -> EngineHost {
    let ir = load_ir(ir_bytes).unwrap();
    let sample: Value = serde_json::from_slice(
        &std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../testdata/contract/event-sequence.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let text = serde_json::to_string(&sample["initial"])
        .unwrap()
        .replace("\"main/st-1\"", &format!("\"{first}\""));
    let state: SessionState = serde_json::from_str(&text).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

fn settle(h: &mut EngineHost) {
    let mut guard = 0;
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        guard += 1;
        assert!(guard < 50, "進行が終わらない");
    }
}

/// 章の形の写し（S1・S2）を通す。`moves` は `(手, 手の後の現在の Step)`。
fn run_moves<M: Copy>(
    h: &mut EngineHost,
    cond: Cond,
    first: &str,
    moves: &[(M, &str)],
    apply: impl Fn(M, &mut EngineHost),
) {
    send_long(h);
    let mut prev = chapter_of(first).to_owned();
    for (i, (mv, current)) in moves.iter().enumerate() {
        apply(*mv, h);
        settle(h);
        if cond == Cond::C {
            save(h);
        }
        let ch = chapter_of(current).to_owned();
        let last = i + 1 == moves.len();
        if cond == Cond::A && (ch != prev || last) {
            save(h);
        }
        prev = ch;
    }
}

struct HostDrive<'a>(&'a mut EngineHost, Cond);

impl Drive for HostDrive<'_> {
    fn apply(&mut self, op: Op) -> Vec<Event> {
        let is_save = matches!(op, Op::Control(Control::Save));
        let before = self.0.model().session().unwrap().log().len();
        self.0.update(HostAction::Engine(op));
        let ev = self.0.model().session().unwrap().log()[before..].to_vec();
        if self.1 == Cond::C && !is_save {
            save(self.0);
        }
        ev
    }
    fn state(&self) -> &SessionState {
        self.0.model().session().unwrap().state()
    }
}

fn run_section(section: Section, cond: Cond) -> Outcome {
    match section {
        Section::S1 => {
            let bytes = std::fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/ch0_3/ir.json"),
            )
            .unwrap();
            let mut h = host_at(&bytes, ch0_3_journey::FIRST);
            let moves: Vec<_> = ch0_3_journey::journey()
                .into_iter()
                .map(|(m, _, cur, _)| (m, cur))
                .collect();
            run_moves(&mut h, cond, ch0_3_journey::FIRST, &moves, |m, h| {
                m.apply(h)
            });
            finish(&h, &bytes, "ch0-3-run".into())
        }
        Section::S2 => {
            let bytes = std::fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/ch4_9/ir.json"),
            )
            .unwrap();
            let mut h = host_at(&bytes, ch4_9_journey::FIRST);
            let moves: Vec<_> = ch4_9_journey::journey()
                .into_iter()
                .map(|(m, _, cur, _)| (m, cur))
                .collect();
            run_moves(&mut h, cond, ch4_9_journey::FIRST, &moves, |m, h| {
                m.apply(h)
            });
            finish(&h, &bytes, "ch4-9-run".into())
        }
        Section::S3 => {
            let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../testdata/module-reuse-rounds/test-data/pack");
            let bytes = compile_dir(&dir).expect("compile");
            let ir = load_ir(&bytes).unwrap();
            let session = Session::from_state(ir.clone(), SessionState::default());
            let mut h = EngineHost::with_session(session, standard_registry()).unwrap();
            let mut plan = round1_plan(&ir);
            plan.push(PlanStep::Op(Op::Control(Control::Save)));
            plan.extend(begin_round2_plan());
            plan.extend(round2_plan(&ir));
            plan.push(PlanStep::Op(Op::Control(Control::Save)));
            let mut sent = false;
            for step in &plan {
                run_plan(&mut HostDrive(&mut h, cond), std::slice::from_ref(step));
                if !sent && h.model().session().unwrap().state().run.busy.is_none() {
                    // 1 周目の最初の手の後（位置があり処理中でない）。次の手は自然文を受け付けない。
                    if h.model().session().unwrap().state().run.current.is_some() {
                        send_long(&mut h);
                        sent = true;
                    }
                }
            }
            assert!(sent, "長い入力を差し込めなかった");
            finish(&h, &bytes, ir.pack.id.clone())
        }
    }
}

fn size_of(o: &Outcome) -> HistorySize {
    measure(&o.env, &o.bytes)
}

#[test]
fn all_chapters_within_5mb() {
    let mut totals = [(0u64, 0u64, 0u64); 2];
    for (ci, cond) in Cond::ALL.into_iter().enumerate() {
        for section in Section::ALL {
            let o = run_section(section, cond);
            let size = size_of(&o);
            let line = serde_json::json!({
                "section": section.name(),
                "condition": cond.name(),
                "pack_id": o.pack_id,
                "ir_sha256": o.ir_sha256,
                "ops_sha256": hex(&serde_json::to_vec(&o.env.ops).unwrap()),
                "total_bytes": size.total_bytes,
                "ops": size.ops,
                "ops_bytes": size.ops_bytes,
                "snapshot_bytes": size.snapshot_bytes,
                "log_entries": size.log_entries,
                "changed_entries": o.changed_events,
                "changed_ratio_ppm": size.changed_ratio_ppm(),
                "checkpoints": size.checkpoints,
                "checkpoint_bytes": size.checkpoint_bytes,
                "long_submit_bytes": o.long_submit_bytes,
                "world_bytes": o.world_bytes,
                "threshold_bytes": SAVE27_THRESHOLD_BYTES,
                "threshold_ratio_ppm": size.total_bytes * 1_000_000 / SAVE27_THRESHOLD_BYTES,
            });
            println!("\nHISTORY-SIZE-ALL-CHAPTERS {line}");
            assert!(
                size.total_bytes <= SAVE27_THRESHOLD_BYTES,
                "SAVE-27 exceeded: {} cond {} total_bytes {}",
                section.name(),
                cond.name(),
                size.total_bytes
            );
            assert!(size.checkpoints > 0);
            assert!(o.long_submit_bytes < size.total_bytes);
            totals[ci].0 += size.total_bytes;
            totals[ci].1 += size.checkpoints;
            totals[ci].2 += o.world_bytes;
        }
    }
    for (ci, cond) in Cond::ALL.into_iter().enumerate() {
        let (sum, checkpoints, worlds) = totals[ci];
        let estimate = sum + (checkpoints + 1) * worlds;
        println!(
            "\nHISTORY-SIZE-ALL-CHAPTERS-ESTIMATE {}",
            serde_json::json!({
                "condition": cond.name(),
                "sum_total_bytes": sum,
                "sum_checkpoints": checkpoints,
                "sum_world_bytes": worlds,
                "estimate_bytes": estimate,
                "threshold_bytes": SAVE27_THRESHOLD_BYTES,
                "threshold_ratio_ppm": estimate * 1_000_000 / SAVE27_THRESHOLD_BYTES,
            })
        );
        assert!(
            estimate <= SAVE27_THRESHOLD_BYTES,
            "SAVE-27 estimate exceeded: cond {} estimate {}",
            cond.name(),
            estimate
        );
    }
}

#[test]
fn checkpoints_dominate_and_long_input_is_kept() {
    for section in Section::ALL {
        let a = size_of(&run_section(section, Cond::A));
        let c = size_of(&run_section(section, Cond::C));
        assert!(a.checkpoints < c.checkpoints, "{}", section.name());
        assert!(a.total_bytes < c.total_bytes, "{}", section.name());
        assert!(c.checkpoint_bytes > c.changed_bytes, "{}", section.name());
    }
}

#[test]
fn same_run_same_bytes_and_round_trip() {
    for section in Section::ALL {
        for cond in Cond::ALL {
            let first = run_section(section, cond);
            let again = run_section(section, cond);
            assert_eq!(
                first.bytes,
                again.bytes,
                "{} {}",
                section.name(),
                cond.name()
            );
            let value = parse_history_json(&first.bytes).expect("stages 1-3");
            assert_eq!(
                parse_history_envelope(value).expect("stages 4-5"),
                first.env
            );
        }
    }
}
