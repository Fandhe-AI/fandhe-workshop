//! 章の完了後の「次の章へ進む」の試験（SHELL-10。TASK-526 1/2）。native で、受講者の画面と同じ既定の feature（`test-hooks` なし）で流れる。
//!
//! 試験用の教材 `tests/data/ch0_3/ir.json` は章の境目を無条件の `next` でつなぐため、試験の中で変種を作る。
//! 各章の最後の Step を `final` にし、開始点を並べ替えて `#` つきの開始点を混ぜる。
//! 2/2 では押下の結線（今の提示と完全一致したときだけ次の章の入口へ `Control::GoTo` で移る）と、CH-0 から CH-3 の通し、
//! 記録（`ops`）からの復元を固定する。`Control::Begin` でないこと・開始点の `effects` を適用しないことは `next_chapter` のモジュールの doc に書いた限界である。
//! 変種の作り方はブラウザ用の教材（`test_fixture::ch03_chapters_ir_bytes`）と同じで、2 か所を揃えて保つ。
//! 試験では `unwrap` を使ってよい。失敗の文面には教材由来の値を出さない。

use std::path::PathBuf;

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::DirtyTracked;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::next_chapter::{next_chapter, ACTION_NEXT_CHAPTER, NEXT_CHAPTER_FIELD};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::shell::{check_complete, NEXT_CHAPTER_TESTID};
use serde_json::{json, Value};

fn manifest(p: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(p)
}

/// 章の最後を `final` にし、開始点を差し替えた変種の IR（JSON）。
fn variant_json(starts: Value) -> Value {
    let mut d: Value =
        serde_json::from_slice(&std::fs::read(manifest("tests/data/ch0_3/ir.json")).unwrap())
            .unwrap();
    for n in d["nodes"].as_array_mut().unwrap() {
        let uid = n["uid"].as_str().unwrap().to_owned();
        if ["main/st-0-2c", "main/st-1-6", "main/st-2-8"].contains(&uid.as_str()) {
            n["step"]["final"] = json!(true);
            n["step"]["next"] = json!([]);
        }
    }
    d["starts"] = starts;
    d
}

fn all_starts() -> Value {
    json!([
        {"id": "s-0-1", "node": "main/st-0-1", "effects": []},
        {"id": "s-1-4", "node": "main/st-1-4", "effects": []},
        {"id": "s-1-1", "node": "main/st-1-1", "effects": []},
        {"id": "m#x", "node": "main/st-2-1", "effects": []},
        {"id": "s-2-1", "node": "main/st-2-1", "effects": []},
        {"id": "s-3-1", "node": "main/st-3-1", "effects": []}
    ])
}

fn ir_of(d: &Value) -> Ir {
    load_ir(&serde_json::to_vec(d).unwrap()).unwrap()
}

fn initial(first: &str) -> SessionState {
    let sample: Value = serde_json::from_slice(
        &std::fs::read(manifest("../../testdata/contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let text = serde_json::to_string(&sample["initial"])
        .unwrap()
        .replace("\"main/st-1\"", &format!("\"{first}\""));
    serde_json::from_str(&text).unwrap()
}

fn host_of(ir: Ir, first: &str) -> EngineHost {
    EngineHost::with_session(Session::from_state(ir, initial(first)), standard_registry()).unwrap()
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

/// `uid` を現在の Step にして、状態を `status`（処理中でない）にした状態。
fn at(uid: &str, status: StepStatus) -> SessionState {
    let mut s = initial(uid);
    let uid = StepUid(uid.to_owned());
    s.run.busy = None;
    s.run
        .slices
        .entry(uid.root())
        .or_default()
        .status
        .insert(uid.clone(), status);
    s.run.current = Some(uid);
    s
}

fn offer(ir: &Ir, uid: &str, status: StepStatus) -> Option<(String, String)> {
    next_chapter(ir, &at(uid, status)).map(|o| (o.start_id, o.title))
}

#[test]
fn 各章の最後の達成で次の章の入口を選び_最後の章では出さない() {
    let ir = ir_of(&variant_json(all_starts()));
    let a = StepStatus::Achieved;
    let ch0 = offer(&ir, "main/st-0-2c", a).expect("CH-0 の後");
    assert_eq!(ch0.0, "s-1-1", "先頭の節点に一致する開始点を選ぶ");
    let ch1 = offer(&ir, "main/st-1-6", a).expect("CH-1 の後");
    assert_eq!(ch1.0, "s-2-1", "# つきの開始点は選ばない");
    let ch2 = offer(&ir, "main/st-2-8", a).expect("CH-2 の後");
    assert_eq!(ch2.0, "s-3-1");
    assert!(!ch0.1.is_empty() && !ch1.1.is_empty() && !ch2.1.is_empty());
    assert_eq!(offer(&ir, "main/st-3-2", a), None, "最後の章の後は出さない");
}

#[test]
fn 達成の前と最後でない_step_では出さない() {
    let ir = ir_of(&variant_json(all_starts()));
    for st in [
        StepStatus::NotStarted,
        StepStatus::Entered,
        StepStatus::Accepted,
        StepStatus::Skipped,
    ] {
        assert_eq!(offer(&ir, "main/st-0-2c", st), None, "達成の前は出さない");
    }
    assert_eq!(
        offer(&ir, "main/st-0-2b", StepStatus::Achieved),
        None,
        "最後でない Step"
    );
}

#[test]
fn 次の章の開始点が_ir_に無ければ出さない() {
    let starts = json!([
        {"id": "s-0-1", "node": "main/st-0-1", "effects": []},
        {"id": "s-2-1", "node": "main/st-2-1", "effects": []}
    ]);
    let ir = ir_of(&variant_json(starts));
    let a = StepStatus::Achieved;
    assert_eq!(
        offer(&ir, "main/st-0-2c", a),
        None,
        "D-67: 範囲外の章へは出さない"
    );
    assert!(offer(&ir, "main/st-1-6", a).is_some());
}

#[test]
fn 先頭の節点に一致する開始点が無ければ途中の節点を指す開始点へ代えず出さない() {
    let starts = json!([
        {"id": "s-0-1", "node": "main/st-0-1", "effects": []},
        {"id": "s-1-4", "node": "main/st-1-4", "effects": []}
    ]);
    let ir = ir_of(&variant_json(starts));
    assert_eq!(offer(&ir, "main/st-0-2c", StepStatus::Achieved), None);
}

fn count_next(v: &Node) -> usize {
    format!("{v:?}").matches(NEXT_CHAPTER_TESTID).count()
}

/// 「移動」ボタンも、進行を直接送る口も無いこと（feature `test-hooks` なしの画面）。
fn assert_no_goto(v: &Node) {
    let s = format!("{v:?}");
    assert!(!s.contains("移動"), "移動ボタン");
    // 試験の設定（`test-hooks`）では見えない `raw-trigger` が 1 件ある。受講者の画面（既定）にだけ無いことを求める。
    #[cfg(not(feature = "test-hooks"))]
    assert!(!s.contains("raw-trigger"), "raw-trigger");
}

#[test]
fn ホストは_ch0_の最後の達成でちょうど_1_個出し_途中では出さない() {
    let ir = ir_of(&variant_json(all_starts()));
    let mut h = host_of(ir, "main/st-0-1");
    let steps = [
        "choose:opt-left",
        "choose:opt-top",
        "choose:opt-right",
        "point:v-cards:cards/notice",
        "choose:opt-read-a",
        "choose:ans-a-ok",
        "choose:opt-read-b",
        "choose:ans-b-ok",
        "choose:opt-read-c",
        "choose:ans-c-ok",
    ];
    let mut last_dirty_has = false;
    for (i, m) in steps.iter().enumerate() {
        let parts: Vec<&str> = m.split(':').collect();
        if parts[0] == "choose" {
            h.update(act("choose", parts[1]));
        } else {
            h.update(HostAction::Engine(Op::Input(Input::Ui {
                view: parts[1].into(),
                path: parts[2].into(),
            })));
        }
        while let Some(addr) = h.next_tick() {
            h.update(HostAction::Engine(Op::Tick(addr)));
        }
        let v = h.view();
        assert!(check_complete(&v).is_ok(), "手 {i}: 枠の適合");
        assert!(
            fandhe_workshop_web::a11y::check_no_undefined(&v).is_ok(),
            "手 {i}"
        );
        assert_no_goto(&v);
        if i + 1 < steps.len() {
            assert_eq!(count_next(&v), 0, "手 {i}: 最後より前は出さない");
        } else {
            assert_eq!(count_next(&v), 1, "最後の達成の後はちょうど 1 個");
            let s = format!("{v:?}");
            assert!(s.contains("次の章へ進む: "), "表示の文字");
            assert!(s.contains("s-1-1"), "payload は開始点の ID");
            last_dirty_has = h.dirty_fields().contains(&NEXT_CHAPTER_FIELD.name);
        }
    }
    assert!(last_dirty_has, "出たとき next_chapter が dirty");
}

#[test]
fn 初期の画面は親の_list_だけを描き移動ボタンを持たない() {
    let ir = ir_of(&variant_json(all_starts()));
    let h = host_of(ir, "main/st-0-1");
    let v = h.view();
    assert_eq!(count_next(&v), 0);
    assert_no_goto(&v);
    assert!(
        format!("{v:?}").contains("next_chapter"),
        "親の list は常に描く"
    );
}

fn ready_ch0() -> EngineHost {
    let ir = ir_of(&variant_json(all_starts()));
    EngineHost::with_session(
        Session::from_state(ir, at("main/st-0-2c", StepStatus::Achieved)),
        standard_registry(),
    )
    .unwrap()
}

fn cur(h: &EngineHost) -> (String, StepStatus) {
    let s = h.model().session().unwrap().state();
    let uid = s.run.current.clone().unwrap();
    let st = s.run.slices[&uid.root()].status[&uid];
    (uid.0, st)
}

#[test]
fn 一致した押下は次の章の最初の_step_へ移り提示が消える() {
    let mut h = ready_ch0();
    let gen_before = h.model().session().unwrap().state().run.generation;
    let _ = h.dirty_fields();
    h.update(act(ACTION_NEXT_CHAPTER, "s-1-1"));
    assert_eq!(cur(&h), ("main/st-1-1".to_owned(), StepStatus::Entered));
    let s = h.model().session().unwrap().state();
    assert_eq!(
        s.run.generation,
        fandhe_workshop_core::addr::Generation(gen_before.0 + 1),
        "世代が +1"
    );
    assert!(s.run.busy.is_none());
    assert!(h.dirty_fields().contains(&NEXT_CHAPTER_FIELD.name));
    let v = h.view();
    assert_eq!(count_next(&v), 0);
    assert!(check_complete(&v).is_ok());
    assert!(fandhe_workshop_web::a11y::check_no_undefined(&v).is_ok());
    assert_no_goto(&v);
}

#[test]
fn 一致しない押下は状態もログも変えない() {
    for payload in [
        "s-2-1",
        "s-0-1",
        "m#x",
        "main/st-1-1",
        " s-1-1",
        "s-1-1 ",
        "S-1-1",
        "unknown",
    ] {
        let mut h = ready_ch0();
        let before = h.model().session().unwrap().state().clone();
        h.update(act(ACTION_NEXT_CHAPTER, payload));
        assert_eq!(before, *h.model().session().unwrap().state(), "不一致");
    }
    assert!(
        EngineHost::decode_action(ACTION_NEXT_CHAPTER, "").is_none(),
        "空は復号しない"
    );
}

#[test]
fn 提示が無いときは正しい_id_でも何もしない() {
    let ir = ir_of(&variant_json(all_starts()));
    for (uid, st) in [
        ("main/st-0-2b", StepStatus::Achieved),
        ("main/st-0-2c", StepStatus::Accepted),
        ("main/st-3-2", StepStatus::Achieved),
    ] {
        let mut h = EngineHost::with_session(
            Session::from_state(ir.clone(), at(uid, st)),
            standard_registry(),
        )
        .unwrap();
        let before = h.model().session().unwrap().state().clone();
        h.update(act(ACTION_NEXT_CHAPTER, "s-1-1"));
        assert_eq!(before, *h.model().session().unwrap().state());
    }
}

#[test]
fn 表示だけの間は押下を捨てる() {
    use fandhe_workshop_core::history_envelope::build_envelope;
    use fandhe_workshop_core::history_export::export_bytes;
    use fandhe_workshop_web::host::ImportOutcome;

    let d = variant_json(all_starts());
    let mut h = ready_ch0();
    let mut d2 = d.clone();
    d2["pack"]["version"] = json!("9.0.0");
    let env = build_envelope(&ir_of(&d2), "s-0-1", vec![], initial("main/st-0-1")).unwrap();
    let outcome = h.open_history(&export_bytes(&env).unwrap());
    assert!(matches!(outcome, ImportOutcome::ViewOnly(_)));
    let before = h.model().session().unwrap().state().clone();
    h.update(act(ACTION_NEXT_CHAPTER, "s-1-1"));
    assert_eq!(before, *h.model().session().unwrap().state());
}

/// 画面から送れる手（選択肢・左の行・承認・下書きの送信）。
#[derive(Clone, Copy)]
enum M {
    C(&'static str),
    P(&'static str, &'static str),
    D(&'static str),
    R(&'static str),
    S(&'static str),
    E,
}

fn play(h: &mut EngineHost, m: M) {
    match m {
        M::C(id) => h.update(act("choose", id)),
        M::D(d) => h.update(act("decide", d)),
        M::R(t) => h.update(act("revise-target", t)),
        M::P(v, p) => h.update(HostAction::Engine(Op::Input(Input::Ui {
            view: v.into(),
            path: p.into(),
        }))),
        M::E => h.update(HostAction::Engine(Op::Control(Control::Stop))),
        M::S(t) => {
            h.update(act("draft", t));
            h.update(act("submit", ""));
        }
    }
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
    }
}

fn chapter_moves() -> [Vec<M>; 4] {
    use M::*;
    let (v, f) = ("v-cards", "v-files");
    [
        vec![
            C("opt-left"),
            C("opt-top"),
            C("opt-right"),
            P(v, "cards/notice"),
            C("opt-read-a"),
            C("ans-a-ok"),
            C("opt-read-b"),
            C("ans-b-ok"),
            C("opt-read-c"),
            C("ans-c-ok"),
        ],
        vec![
            S("/help"),
            P(v, "cards/help-1"),
            S("/help"),
            S("1 行目のメモ\n2 行目のメモ"),
            S("/new-idea"),
            S("/advance-phase"),
            E,
            C("asked-back"),
            S("/make-card sample"),
            P(v, "cards/sample"),
            C("opt-right"),
            C("ans-restored"),
        ],
        vec![
            S("/new-idea ideas"),
            P(f, "docs/readme"),
            C("opt-view"),
            C("ans-readme"),
            D("approve"),
            P(f, "docs/plan"),
            S("状況を書いた自然文"),
            S("/run-check"),
            P(f, "checks/result"),
            D("revise"),
            R("t-expect"),
            D("approve"),
            P(f, "tasks/t-1"),
            C("ans-t1"),
            C("opt-go"),
        ],
        vec![
            C("opt-place-ok"),
            P(f, "work/place"),
            D("approve"),
            D("approve"),
        ],
    ]
}

#[test]
fn ch0_から_ch3_まで_次の章へ進む_だけで章をまたいで通る() {
    let ir = ir_of(&variant_json(all_starts()));
    let mut h = host_of(ir, "main/st-0-1");
    let firsts = ["main/st-1-1", "main/st-2-1", "main/st-3-1"];
    let ids = ["s-1-1", "s-2-1", "s-3-1"];
    let chapters = chapter_moves();
    let mut generations = Vec::new();
    for (ci, moves) in chapters.iter().enumerate() {
        let n = moves.len();
        for (i, m) in moves.iter().enumerate() {
            play(&mut h, *m);
            let v = h.view();
            assert_no_goto(&v);
            if i + 1 < n {
                assert_eq!(count_next(&v), 0, "章 {ci} 手 {i}: 途中は出さない");
            }
        }
        let v = h.view();
        if ci < 3 {
            assert_eq!(count_next(&v), 1, "章 {ci} の完了後はちょうど 1 個");
            h.update(act(ACTION_NEXT_CHAPTER, ids[ci]));
            assert_eq!(cur(&h), (firsts[ci].to_owned(), StepStatus::Entered));
            assert_eq!(count_next(&h.view()), 0);
            generations.push(h.model().session().unwrap().state().run.generation);
        } else {
            assert_eq!(count_next(&v), 0, "最後の章の後は出さない");
            assert_eq!(cur(&h), ("main/st-3-2".to_owned(), StepStatus::Achieved));
        }
    }
    assert!(generations.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn 越えた移動は_ops_に残り_同じ履歴から復元できる() {
    let ir = ir_of(&variant_json(all_starts()));
    let mut h = host_of(ir.clone(), "main/st-0-1");
    for m in chapter_moves()[0].iter() {
        play(&mut h, *m);
    }
    h.update(act(ACTION_NEXT_CHAPTER, "s-1-1"));
    let text = h.history_to_save().expect("保存する履歴がある");
    let v: Value = serde_json::from_str(&text).unwrap();
    let ops = find_ops(&v).expect("ops");
    let n = ops
        .iter()
        .filter(|o| o.to_string().contains("go_to") && o.to_string().contains("main/st-1-1"))
        .count();
    assert_eq!(n, 1, "移動先が main/st-1-1 の go_to が 1 件");
    // 同じ履歴を新しいホストで開くと、移動の後の位置が再現する。
    let mut fresh = host_of(ir, "main/st-0-1");
    let outcome = fresh.open_history(text.as_bytes());
    assert!(
        !matches!(
            outcome,
            fandhe_workshop_web::host::ImportOutcome::ViewOnly(_)
        ),
        "開ける"
    );
    assert_eq!(cur(&fresh).0, "main/st-1-1");
    assert_eq!(count_next(&fresh.view()), 0);
}

fn find_ops(v: &Value) -> Option<&Vec<Value>> {
    match v {
        Value::Object(m) => {
            if let Some(Value::Array(a)) = m.get("ops") {
                return Some(a);
            }
            m.values().find_map(find_ops)
        }
        _ => None,
    }
}

#[test]
fn 出ている状態で履歴が表示だけになると_next_chapter_が_dirty_になり項目が消える() {
    use fandhe_workshop_core::history_envelope::build_envelope;
    use fandhe_workshop_core::history_export::export_bytes;
    use fandhe_workshop_web::host::ImportOutcome;

    let d = variant_json(all_starts());
    let ir = ir_of(&d);
    let mut h = EngineHost::with_session(
        Session::from_state(ir, at("main/st-0-2c", StepStatus::Achieved)),
        standard_registry(),
    )
    .unwrap();
    assert_eq!(count_next(&h.view()), 1, "前提: 項目が出ている");
    // 主版の違う教材で書いた履歴は、表示だけで開く。
    let mut d2 = d.clone();
    d2["pack"]["version"] = json!("9.0.0");
    let ir2 = ir_of(&d2);
    let env = build_envelope(&ir2, "s-0-1", vec![], initial("main/st-0-1")).unwrap();
    let bytes = export_bytes(&env).unwrap();
    // `update` を経由しない公開 API で切り替える前に、前の dirty を使い切る。
    let _ = h.dirty_fields();
    let outcome = h.open_history(&bytes);
    assert!(
        matches!(outcome, ImportOutcome::ViewOnly(_)),
        "前提: 表示だけ"
    );
    assert!(
        h.dirty_fields().contains(&NEXT_CHAPTER_FIELD.name),
        "FEND-3: 消える項目を dirty で通知する"
    );
    assert_eq!(count_next(&h.view()), 0, "表示だけの間は出さない");
}
