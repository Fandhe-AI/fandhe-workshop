//! 2 周分の通しでの右のログの項目数の測定（TASK-454 1/2。TERM-35・FEND-22・D-46。記録は `docs/testing.md` の 5.349）。
//!
//! `module-reuse-rounds` の 2 周分（計画 A。`rounds-plan-a-v1`。誤入力なし）を `EngineHost` に 1 手ずつ流し、手ごとに
//! 右のログ（`part-log-list`）を描画して `<li` の数を数え、最大を求める。最大が約 1,000 項目（`LIST_ITEMS_TARGET`）以内で、
//! 安全弁（`LIST_LIMIT_ATTR`）にも上流の拒否（`LIST_ERROR_ATTR`）にも当たらないことを確かめる。
//! `--nocapture` のとき `LOG-ROUNDS ` に続く 1 行の JSON を出し、`tools/measure/log-items.sh` が基点つきの結果に包む。
//!
//! 数えるのは native の描画結果で、ブラウザの DOM の値ではない（keyed list の構成で一致する見込み。限界は `docs/testing.md`）。
//! 4,096 件の安全弁と上限を超えたときの見え方は #819（TASK-454 2/2）が受け持つ。操作列の定義は pack の共有部品を借りる
//! （二重定義をして食い違うのを避けるため）。試験では `unwrap` を使ってよい。

#[path = "../../pack/tests/common/rounds.rs"]
mod rounds;
#[path = "../../pack/tests/common/sha256.rs"]
mod sha256;

use std::path::PathBuf;

use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::list_policy::{LIST_ERROR_ATTR, LIST_ITEMS_TARGET, LIST_LIMIT_ATTR};
use fandhe_workshop_web::parts::Component;
use rounds::{goto_count, plan_a, run_plan, Drive, PlanStep};

const SOURCE: &str = "testdata/module-reuse-rounds/test-data/pack";
const PLAN_ID: &str = "rounds-plan-a-v1";

/// 1 手ごとの右のログの観察。
#[derive(Default, Clone, Debug, PartialEq, Eq)]
struct Stats {
    items_max: usize,
    items_last: usize,
    folded_steps: usize,
    limit_marks: usize,
    nested_lists: usize,
    steps_observed: usize,
}

/// 測定用の `Drive`。`round_begin_host.rs` の `HostDrive` と違い、手ごとに描画して数える処理を含むので共有しない。
struct CountingDrive {
    host: EngineHost,
    stats: Stats,
}

fn list_html(html: &str) -> &str {
    let from = html.find("part-log-list").expect("part-log-list");
    let rest = &html[from..];
    &rest[..rest.find("</ul>").expect("</ul>")]
}

impl CountingDrive {
    fn new(ir: &Ir) -> Self {
        let session = Session::new(ir.clone());
        Self {
            host: EngineHost::with_session(session, standard_registry()).unwrap(),
            stats: Stats::default(),
        }
    }

    fn observe(&mut self) {
        let html = render_component_html(&self.host);
        let list = list_html(&html);
        let items = list.matches("<li").count();
        let s = &mut self.stats;
        s.items_max = s.items_max.max(items);
        s.items_last = items;
        s.steps_observed += 1;
        if list.contains("data-fold=") {
            s.folded_steps += 1;
        }
        s.limit_marks +=
            html.matches(LIST_LIMIT_ATTR).count() + html.matches(LIST_ERROR_ATTR).count();
        s.nested_lists += list.matches("<ul").count().saturating_sub(1);
    }

    fn events(&self) -> usize {
        self.host.model().session().unwrap().log().len()
    }
}

impl Drive for CountingDrive {
    fn apply(&mut self, op: Op) -> Vec<Event> {
        let before = self.events();
        self.host.update(HostAction::Engine(op));
        let new = self.host.model().session().unwrap().log()[before..].to_vec();
        self.observe();
        new
    }
    fn state(&self) -> &SessionState {
        self.host.model().session().unwrap().state()
    }
}

fn hex(d: &[u8]) -> String {
    sha256::hex(&sha256::sha256(d))
}

fn compiled() -> (Ir, Vec<u8>) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/module-reuse-rounds/test-data/pack");
    let bytes = compile_dir(&dir).expect("compile");
    (load_ir(&bytes).expect("load_ir"), bytes)
}

struct Measured {
    stats: Stats,
    items_round1_end: usize,
    events_round2_end: usize,
    groups_round2_end: usize,
    current: String,
    last_list: String,
}

fn measure(ir: &Ir) -> Measured {
    let plan = plan_a(ir);
    // 周の境目は 2 周目の開始（`Control::Begin`）の直前。1 周目の終わりの値を控えるため 2 回に分けて流す。
    let split = plan
        .iter()
        .position(|s| matches!(s, PlanStep::Op(Op::Control(Control::Begin { .. }))))
        .expect("Begin");
    let mut d = CountingDrive::new(ir);
    run_plan(&mut d, &plan[..split]);
    let items_round1_end = d.stats.items_last;
    run_plan(&mut d, &plan[split..]);
    let html = render_component_html(&d.host);
    let list = list_html(&html).to_owned();
    let groups: std::collections::BTreeSet<(String, String)> = list
        .split("<li")
        .skip(1)
        .map(|t| {
            let tag = &t[..t.find('>').unwrap_or(t.len())];
            let pick = |k: &str| {
                tag.split(k)
                    .nth(1)
                    .and_then(|r| r.split('"').next())
                    .unwrap_or("")
                    .to_owned()
            };
            (pick("data-gen=\""), pick("data-step=\""))
        })
        .collect();
    Measured {
        stats: d.stats.clone(),
        items_round1_end,
        events_round2_end: d.events(),
        groups_round2_end: groups.len(),
        current: d.state().run.current.clone().expect("current").0,
        last_list: list,
    }
}

#[test]
fn two_rounds_right_log_items_stay_within_target() {
    let (ir, ir_bytes) = compiled();
    let m = measure(&ir);

    assert!(
        m.stats.items_max <= LIST_ITEMS_TARGET,
        "目安を超えた: {}",
        m.stats.items_max
    );
    assert_eq!(m.stats.limit_marks, 0, "安全弁か上流の拒否に当たった");
    assert_eq!(
        m.stats.nested_lists, 0,
        "入れ子の ul があり、<li の数がログの項目数と一致しない"
    );
    assert_eq!(m.current, "main/r2-impl");
    assert!(m.stats.items_max > 0 && m.events_round2_end > 0);
    assert!(m.stats.items_max >= m.items_round1_end);
    assert!(m.last_list.contains("<li"), "最後の手でログが空");

    // 決定性: 同じ計画を 2 回流すと同じ値になる。
    let again = measure(&ir);
    assert_eq!(again.stats, m.stats);
    assert_eq!(again.events_round2_end, m.events_round2_end);

    // 基点: 履歴の大きさの測定と同じ操作列であることの裏付け（`ops` の SHA-256）と、教材の内容のハッシュ。
    let plan = plan_a(&ir);
    let mut rs = RecordedSession::new(Session::new(ir.clone()), RecordedSession::first_start(&ir));
    run_plan(&mut rs, &plan);
    let env = rs.envelope_for_save().expect("envelope");
    export_bytes(&env).expect("export");

    let line = serde_json::json!({
        "pack": {
            "id": ir.pack.id,
            "version": ir.pack.version,
            "nodes": ir.nodes.len(),
            "ir_sha256": hex(&ir_bytes),
            "content_hash": env.versions.content_hash,
            "source": SOURCE,
        },
        "ops_sequence": {
            "id": PLAN_ID,
            "ops": env.ops.len(),
            "ops_sha256": hex(&serde_json::to_vec(&env.ops).expect("ops json")),
            "goto_substitutions": goto_count(&plan),
        },
        "log": {
            "items_max": m.stats.items_max,
            "items_round1_end": m.items_round1_end,
            "items_round2_end": m.stats.items_last,
            "events_round2_end": m.events_round2_end,
            "groups_round2_end": m.groups_round2_end,
            "folded_steps": m.stats.folded_steps,
            "limit_marks": m.stats.limit_marks,
            "steps_observed": m.stats.steps_observed,
            "target": LIST_ITEMS_TARGET,
        },
    });
    // 先頭の改行は、`--test-threads=1` で試験名の表示と同じ行に出て行頭の照合が外れるのを防ぐ。
    println!("\nLOG-ROUNDS {line}");
}
