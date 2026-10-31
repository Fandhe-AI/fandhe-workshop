//! 2 周分の履歴の大きさと `Changed` の割合の測定（TASK-216。EVT-13。記録は `docs/testing.md` の 5.322）。
//!
//! `module-reuse-rounds` の 2 周分（条件 A の形。誤入力なし）を `RecordedSession` で流し、各周の終わりに
//! `Control::Save` を送り、書き出しの包み（`ops` と `snapshot`）の大きさと、`snapshot.log` の `Changed` の
//! 件数・バイト数・割合を求める。`--nocapture` のとき `HISTORY-SIZE ` に続く 1 行の JSON を出し、
//! `tools/measure/history-size.sh` がそれを基点つきの結果に包む。
//!
//! TASK-336（SAVE-26）との分担。5MB 以下の判定、条件 B／C、誤入力 3 割の合成は `history_size_conditions.rs` が持つ。本試験は 5MB の
//! 閾値を assert しない。計画（`common/rounds.rs` の `PlanStep` の列）に操作を差し込めば、本試験を変えずに条件を足せる。
//! 未実装の部品のため `GoTo` で飛ばす場所があり（1 周目 3・2 周目 2）、`Changed` の件数は真の通しと違いうる。

mod common;
#[path = "common/history_size.rs"]
mod history_size;
#[path = "common/rounds.rs"]
mod rounds;
#[path = "common/sha256.rs"]
mod sha256;

use common::testdata;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_envelope::SessionEnvelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::Session;
use fandhe_workshop_pack::compile::compile_dir;
use history_size::{measure, HistorySize};
use rounds::{goto_count, plan_a, run_plan, PlanStep};

const SOURCE: &str = "testdata/module-reuse-rounds/test-data/pack";
const PLAN_ID: &str = "rounds-plan-a-v1";

fn hex(d: &[u8]) -> String {
    sha256::hex(&sha256::sha256(d))
}

fn compiled() -> (Ir, Vec<u8>) {
    let bytes = compile_dir(&testdata("module-reuse-rounds/test-data/pack")).expect("compile");
    (load_ir(&bytes).expect("load_ir"), bytes)
}

struct Run {
    env: SessionEnvelope,
    bytes: Vec<u8>,
    events: Vec<Event>,
    current: String,
}

fn run(ir: &Ir, plan: &[PlanStep]) -> Run {
    let mut rs = RecordedSession::new(Session::new(ir.clone()), RecordedSession::first_start(ir));
    let events = run_plan(&mut rs, plan);
    let current = rs.session().state().run.current.clone().expect("current").0;
    let env = rs.envelope_for_save().expect("envelope");
    let bytes = export_bytes(&env).expect("export");
    Run {
        env,
        bytes,
        events,
        current,
    }
}

#[test]
fn two_rounds_history_size_and_changed_share() {
    let (ir, ir_bytes) = compiled();
    let plan = plan_a(&ir);
    let r = run(&ir, &plan);
    let size: HistorySize = measure(&r.env, &r.bytes);

    assert_eq!(r.current, "main/r2-impl");
    // 周回ごとの手動の保存 1 件に、章の終わり（`main` の最後の Step の達成）の自動保存が加わる（SAVE-31、TASK-339 1/2）。
    assert_eq!(
        size.checkpoints, 4,
        "one manual checkpoint per round plus chapter-end autosaves"
    );
    assert!(size.changed_entries > 0);
    assert!(size.changed_bytes > 0 && size.changed_bytes < size.snapshot_bytes);
    assert!(size.snapshot_bytes < size.total_bytes);
    assert!(size.ops_bytes < size.total_bytes);

    // 数え漏れの防止: ログの `Changed` は、操作ごとに出たイベントの `Changed` と同数。
    let from_events = r
        .events
        .iter()
        .filter(|e| matches!(e, Event::Changed { .. }))
        .count() as u64;
    assert_eq!(size.changed_entries, from_events);

    // 決定性: 同じ計画を 2 回流すと書き出しが一致する。
    assert_eq!(run(&ir, &plan).bytes, r.bytes);

    // 測った対象が本物の履歴であること: 書き出しが読み込みの段 1〜5 を通って元の包みに戻る。
    let value = parse_history_json(&r.bytes).expect("stages 1-3");
    assert_eq!(parse_history_envelope(value).expect("stages 4-5"), r.env);

    let line = serde_json::json!({
        "pack": {
            "id": ir.pack.id,
            "version": ir.pack.version,
            "nodes": ir.nodes.len(),
            "ir_sha256": hex(&ir_bytes),
            "content_hash": r.env.versions.content_hash,
            "source": SOURCE,
        },
        "ops_sequence": {
            "id": PLAN_ID,
            "ops": size.ops,
            "ops_sha256": hex(&serde_json::to_vec(&r.env.ops).expect("ops json")),
            "goto_substitutions": goto_count(&plan),
        },
        "size": {
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
        },
    });
    // 先頭の改行は、`--test-threads=1` で試験名の表示（`test x ... `）と同じ行に出て行頭の照合が外れるのを防ぐ。
    println!("\nHISTORY-SIZE {line}");
}
