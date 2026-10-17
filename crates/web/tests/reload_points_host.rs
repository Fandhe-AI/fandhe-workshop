//! 15 地点での再読み込みの一致の試験。host の層（TASK-319 3/3、SAVE-3）。
//!
//! ブラウザの試験（`tools/browser/reload-points.test.mjs`）と同じ 8 Step の教材で、CH-2 に相当する 15 地点
//! （入った直後 8・確認待ち 7）の全部で、保存→新しいページでの起動の前後が世代を除いて一致し、
//! そこから完走した最終の世界状態が再読み込みなしの完走と一致することを、web の保存の経路
//! （ホストの記録・`history_to_save`・保留と書き出し・保存先の偽物からの読み戻し・`boot`）を通して確かめる。
//! 変異の検出先は、画面を持たないこの層である（`tests/mutations/SAVE3-*.mutation`）。
//!
//! 出所は PoC-6 の S-1（地点 15・`busy` は 0）。処理中は各手の後に進行を終えてから次へ進むため、地点に現れない。
//! 処理中の地点は core の 33 地点の試験（`crates/core/tests/reload_points.rs`）が持つ。
//! 教材は `tests/data/reload_points_ch2/ir.json`（core の 16 Step の先頭 8 節点の写し）である。
//! 試験では `unwrap` を使ってよい。

#[path = "common/storage_fakes.rs"]
mod storage_fakes;

use std::path::PathBuf;

use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey, HistoryStore};
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::{artifact_rows, standard_registry};
use fandhe_workshop_web::host::{BootKind, EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::save_pending::PendingSave;
use storage_fakes::{fixture, Fixture};

const LAST: &str = "main/st-8";
/// 1 回の完走に許す手の数の上限（無限ループで試験が止まらないための安全弁）。
const FINISH_LIMIT: usize = 100;

fn ir_bytes() -> Vec<u8> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/reload_points_ch2/ir.json");
    std::fs::read(path).unwrap()
}

/// 開始直後の状態から始まる、起動前のホスト（新しいページを模す）。
fn page() -> EngineHost {
    let ir = load_ir(&ir_bytes()).unwrap();
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../testdata/contract/event-sequence.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let state: SessionState = serde_json::from_value(sample["initial"].clone()).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

fn key(h: &EngineHost) -> HistoryKey {
    h.history_key(DeliveryScope::named("repo")).unwrap()
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

fn state(h: &EngineHost) -> SessionState {
    h.model().session().unwrap().state().clone()
}

fn status_of(s: &SessionState, step: &StepUid) -> StepStatus {
    s.run
        .slices
        .get(&step.root())
        .and_then(|sl| sl.status.get(step))
        .copied()
        .unwrap_or_default()
}

fn done(s: &SessionState) -> bool {
    s.run.completed || status_of(s, &StepUid(LAST.to_owned())) == StepStatus::Achieved
}

/// 手の経路。実際の画面から送れるか、テスト専用の口（raw）か。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    Screen,
    Raw,
}

fn ui(path: &str) -> Input {
    Input::Ui {
        view: "v-files".into(),
        path: path.into(),
    }
}

/// 誘導つきの次の 1 手を打つ。完走していれば `None`、打ったら手の経路を返す。
fn step(h: &mut EngineHost) -> Option<Route> {
    let s = state(h);
    if done(&s) {
        return None;
    }
    if let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        return Some(Route::Screen);
    }
    let current = s.run.current.clone().unwrap();
    let n: usize = current.0.strip_prefix("main/st-").unwrap().parse().unwrap();
    let accepted = status_of(&s, &current) == StepStatus::Accepted;
    let path = format!("notes/n{n}");
    let raw = |h: &mut EngineHost, input: Input| {
        h.update(HostAction::Engine(Op::Input(input)));
        Route::Raw
    };
    let submit = |h: &mut EngineHost, text: &str| {
        h.update(act("draft", text));
        h.update(act("submit", ""));
        Route::Screen
    };
    let route = if accepted {
        match n {
            2 | 7 => {
                h.update(act("choose", "opt-a"));
                Route::Screen
            }
            6 => {
                h.update(act("decide", "approve"));
                Route::Screen
            }
            _ => raw(h, ui(&path)),
        }
    } else {
        match n {
            2 => submit(h, "メモ 2"),
            3 | 8 => {
                h.update(act("decide", "approve"));
                Route::Screen
            }
            4 | 5 => raw(h, Input::Choice("opt-a".into())),
            6 => {
                h.update(act("revise-target", "expect"));
                Route::Screen
            }
            7 => raw(h, ui(&path)),
            _ => submit(h, &format!("/make-note note-{n}")),
        }
    };
    Some(route)
}

/// 処理中が終わるまで進行を打つ（ブラウザと同じく、処理中の地点を作らない）。
fn settle(h: &mut EngineHost) {
    let mut guard = 0;
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        guard += 1;
        assert!(guard < 50, "進行が終わらない");
    }
}

/// 完走するまで誘導つきの手を打つ。
fn finish(h: &mut EngineHost) {
    for _ in 0..FINISH_LIMIT {
        if step(h).is_none() {
            return;
        }
    }
    panic!("did not finish within {FINISH_LIMIT} steps");
}

/// 離れる直前の書き出し（`persist::install_save_on_leave` の native 版）。
fn leave(h: &EngineHost, f: &Fixture) {
    let v = h.history_to_save().expect("保存する値がある");
    let pending = PendingSave::new();
    pending.stage(key(h), v);
    let out = pending.flush(&f.storage);
    assert_eq!((out.written, out.failed), (1, 0));
}

/// 新しいページを開く。保存先から読んで `boot` する（`persist::boot_and_mount` の native 版）。
fn open_page(f: &Fixture) -> (EngineHost, BootKind) {
    let mut h = page();
    let saved = f.storage.read(&key(&h)).unwrap();
    let kind = h.boot(saved.as_deref().map(str::as_bytes));
    (h, kind)
}

fn rows(h: &EngineHost) -> Vec<(String, Option<String>)> {
    artifact_rows(h.model().world(), h.model().projection().marks())
        .into_iter()
        .map(|(p, m)| (p, m.map(|m| format!("{m:?}"))))
        .collect()
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Counts {
    points: usize,
    entered: usize,
    busy: usize,
    awaiting_confirm: usize,
    screen: usize,
    raw: usize,
}

#[test]
fn 十五の地点で再読み込みの前後が世代を除いて一致し完走した世界状態が一致する() {
    let mut base = page();
    finish(&mut base);
    assert!(done(&state(&base)), "baseline must complete");
    let base_world = state(&base).run.world.clone();

    let mut h = page();
    let mut counts = Counts::default();
    loop {
        settle(&mut h);
        let s = state(&h);
        if done(&s) {
            break;
        }
        let k = counts.points;
        counts.points += 1;
        let current = s.run.current.clone().unwrap();
        if s.run.busy.is_some() {
            counts.busy += 1;
        } else if status_of(&s, &current) == StepStatus::Accepted {
            counts.awaiting_confirm += 1;
        } else {
            counts.entered += 1;
        }

        // 地点ごとに違う空でない下書きを入れる（未操作の履歴にしないため。入力欄の値の比較を空同士にしないため）。
        let draft = format!("下書き {k}");
        h.update(act("draft", &draft));
        let before = state(&h);
        let before_rows = rows(&h);
        let before_review = h.model().projection().review().to_vec();
        let before_world = h.model().world().clone();

        let f = fixture(false);
        leave(&h, &f);
        let (mut r, kind) = open_page(&f);
        assert_eq!(kind, BootKind::Restored, "point {k}");
        let after = state(&r);
        assert_eq!(
            after.run.generation.0,
            before.run.generation.0 + 1,
            "point {k}: generation"
        );
        assert!(
            matches!(after.log.last(), Some(Event::Reloaded { .. })),
            "point {k}: log ends with Reloaded"
        );
        let mut normalized = after.clone();
        normalized.run.generation = before.run.generation;
        normalized.log.pop();
        assert_eq!(normalized, before, "point {k}: state");
        assert_eq!(r.draft(), draft, "point {k}: draft");
        assert_eq!(rows(&r), before_rows, "point {k}: rows");
        assert_eq!(
            r.model().projection().review(),
            before_review.as_slice(),
            "point {k}: review"
        );
        assert_eq!(r.model().world(), &before_world, "point {k}: world");

        // 復元した側を完走させ、再読み込みなしの完走と世界状態が一致する。
        finish(&mut r);
        assert!(done(&state(&r)), "point {k}: completes");
        assert_eq!(state(&r).run.world, base_world, "point {k}: final world");

        // 元の側で誘導つきの 1 手を打って、次の地点へ進む。
        match step(&mut h).expect("guided step") {
            Route::Screen => counts.screen += 1,
            Route::Raw => counts.raw += 1,
        }
    }
    println!("OBS {counts:?}");
    assert_eq!(
        counts,
        Counts {
            points: 15,
            entered: 8,
            busy: 0,
            awaiting_confirm: 7,
            screen: 8,
            raw: 7,
        }
    );
}

#[test]
fn データは未実装の語を使わず形が地点の数え方の前提どおり() {
    let ir = load_ir(&ir_bytes()).unwrap();
    assert_eq!(ir.nodes.len(), 8);
    let busy_total: usize = ir.nodes.iter().map(|n| n.step.processing.len()).sum();
    assert_eq!(busy_total, 3, "処理中の進行の合計");
    let finals: Vec<_> = ir
        .nodes
        .iter()
        .filter(|n| n.step.is_final)
        .map(|n| n.uid.as_str())
        .collect();
    assert_eq!(finals, [LAST], "final は最後の Step だけ");
    let awaiting = ir
        .nodes
        .iter()
        .filter(|n| {
            n.step
                .confirm
                .as_ref()
                .and_then(|c| c.get("kind"))
                .and_then(|k| k.as_str())
                != Some("none")
        })
        .count();
    assert_eq!(awaiting, 7, "確認が none でない Step の数");
    for n in &ir.nodes {
        let s = &n.step;
        assert!(s.on_enter.is_empty(), "{}: on_enter", n.uid);
        assert!(s.use_module.is_none(), "{}: use", n.uid);
        assert!(n.exit_to.is_none(), "{}: exit_to", n.uid);
        assert!(
            s.next.iter().all(|t| t.when.is_none()),
            "{}: 条件つきの next",
            n.uid
        );
        assert!(
            s.rules
                .iter()
                .all(|r| r.get("when").is_none_or(|w| w.is_null())),
            "{}: 規則の when",
            n.uid
        );
    }
    assert_ne!(
        ir.pack.id, "min-3step",
        "mount の既定の教材と鍵が衝突しない"
    );
}
