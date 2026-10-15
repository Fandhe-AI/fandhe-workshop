//! 案内レベルとヒントの公開 API（TASK-290、契約の版 2）の適合試験。GUIDE-3・GUIDE-17 の型の水準である。
//!
//! 操作とイベントの直列化の形を `tests/data/guidance_1/wire.json` とのバイト一致で固定する（golden の書き換えは
//! 契約の変更で、`CONTRACT_VERSION` の版上げの判断が要る）。振る舞い（ヒントを開く処理）は TASK-294・295・297 が
//! 実装するため、ここでは確かめない。試験は作業ツリーを書き換えない。試験では `unwrap` を使ってよい。

use std::path::Path;

use fandhe_workshop_contract::guidance::{visible_hints, GuidanceLevel};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid};
use fandhe_workshop_core::event::{ControlKind, Event, RefuseReason};
use fandhe_workshop_core::input::{Control, GenerationRule, Op};
use fandhe_workshop_core::session::{Checkpoint, RunState, SessionState, Slice};
use serde_json::{json, Value};

fn stamp() -> Stamp {
    Stamp {
        generation: Generation(3),
        step: StepUid("main/s1".to_string()),
    }
}

fn wire_value() -> Value {
    json!({
        "open_hint_op": Op::Control(Control::OpenHint),
        "set_guidance_op": Op::Control(Control::SetGuidance(GuidanceLevel::C)),
        "hint_opened": Event::HintOpened { at: stamp(), opened: 1, total: 2 },
        "refused_no_hint": Event::Refused {
            at: stamp(),
            op: ControlKind::OpenHint,
            reason: RefuseReason::NoHint,
            rejection: None,
        },
    })
}

#[test]
fn wire_forms_match_golden_bytes() {
    let mut bytes = serde_json::to_vec_pretty(&wire_value()).unwrap();
    bytes.push(b'\n');
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/guidance_1/wire.json");
    let golden = std::fs::read(&p).expect("golden を読めること");
    assert_eq!(
        String::from_utf8_lossy(&bytes),
        String::from_utf8_lossy(&golden)
    );
}

#[test]
fn wire_forms_have_the_documented_shape() {
    let v = wire_value();
    assert_eq!(v["open_hint_op"], json!({"control": "open_hint"}));
    assert_eq!(
        v["set_guidance_op"],
        json!({"control": {"set_guidance": "C"}})
    );
    assert_eq!(v["hint_opened"]["hint_opened"]["opened"], 1);
    assert_eq!(v["hint_opened"]["hint_opened"]["total"], 2);
    assert_eq!(v["refused_no_hint"]["refused"]["op"], "open_hint");
    assert_eq!(v["refused_no_hint"]["refused"]["reason"], "no_hint");
}

#[test]
fn unknown_values_are_rejected() {
    assert!(
        serde_json::from_str::<Op>(r#"{"control":{"set_guidance":"D"}}"#).is_err(),
        "未知の水準"
    );
    assert!(
        serde_json::from_str::<Op>(r#"{"control":{"set_guidance":"c"}}"#).is_err(),
        "小文字の水準"
    );
    let extra =
        r#"{"hint_opened":{"at":{"generation":1,"step":"s"},"opened":1,"total":2,"extra":1}}"#;
    assert!(serde_json::from_str::<Event>(extra).is_err());
    let ok = extra.replace(r#","extra":1"#, "");
    assert!(serde_json::from_str::<Event>(&ok).is_ok());
}

#[test]
fn open_hint_and_set_guidance_keep_the_generation() {
    // GUIDE-17: 世代は変わらない。GUIDE-3: 切り替えも世代を変えない。
    assert_eq!(
        Op::Control(Control::OpenHint).generation_rule(),
        GenerationRule::Keep
    );
    assert_eq!(
        Op::Control(Control::SetGuidance(GuidanceLevel::B)).generation_rule(),
        GenerationRule::Keep
    );
}

#[test]
fn hint_opened_has_no_new_generation_and_no_hint_pairs_with_open_hint() {
    let v = wire_value();
    assert!(v["hint_opened"]["hint_opened"]
        .get("new_generation")
        .is_none());
    let e = Event::Refused {
        at: stamp(),
        op: ControlKind::OpenHint,
        reason: RefuseReason::NoHint,
        rejection: None,
    };
    assert!(matches!(
        e,
        Event::Refused {
            op: ControlKind::OpenHint,
            reason: RefuseReason::NoHint,
            ..
        }
    ));
}

#[test]
fn total_comes_from_visible_hints_of_the_current_level() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../contract/tests/data/guidance_1");
    let ir: Ir =
        serde_json::from_slice(&std::fs::read(dir.join("hints-ir.json")).unwrap()).unwrap();
    let step = &ir.nodes[0].step;
    let level = GuidanceLevel::A;
    let total = u32::try_from(visible_hints(&ir, step, level).len()).unwrap();
    assert_eq!(total, 2);
    let e = Event::HintOpened {
        at: stamp(),
        opened: 1,
        total,
    };
    assert!(matches!(e, Event::HintOpened { total: 2, .. }));
}

#[test]
fn guidance_level_is_one_per_run_and_outside_slices_and_checkpoint_run_is_a_copy() {
    // GUIDE-3: 案内レベルは RunState に 1 つ。Slice には鍵が無い。
    let run = RunState {
        guidance_level: GuidanceLevel::B,
        ..RunState::default()
    };
    let v = serde_json::to_value(&run).unwrap();
    let obj = v.as_object().unwrap();
    assert_eq!(obj.keys().filter(|k| *k == "guidance_level").count(), 1);
    assert_eq!(obj["guidance_level"], "B");
    let slice = serde_json::to_value(Slice::default()).unwrap();
    assert!(!slice
        .as_object()
        .unwrap()
        .keys()
        .any(|k| k.contains("guidance")));
}

#[test]
fn hints_opened_lives_outside_run() {
    // GUIDE-7: 停止・復帰で戻らないよう、run（Checkpoint.run・EntryCopy.run に写される）の外に置く。
    let mut s = SessionState::default();
    s.hints_opened.insert(StepUid("main/s1".to_string()), 1);
    let v = serde_json::to_value(&s).unwrap();
    assert_eq!(v["hints_opened"]["main/s1"], 1);
    assert!(v["run"].get("hints_opened").is_none());
    let cp = serde_json::to_value(Checkpoint {
        run: s.run.clone(),
        entry: None,
        log_len: 0,
        owner: None,
    })
    .unwrap();
    assert!(cp["run"].get("hints_opened").is_none());
}
