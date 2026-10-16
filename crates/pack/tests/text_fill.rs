//! 拒否・解説への値の差し込み（実行時の充填）と R-21 の確認の受け入れ試験（JUDGE-8、TASK-274）。
//!
//! 教材パックを `compile_dir` → `load_ir` で IR にし、core の `judge_input` と `text_fill::filled_text` に通す。
//! 文面の確認（`text_checks`）は、試作の追加の検証（19 件）と PoC-13 の準備（38 件）と同じ規模で、架空の教材
//! `tests/data/fill-pack` に 38 件（群 A が 19 件、群 B が 19 件）置く。件数は固定し、データが減っても素通りしない。
//! 各件の期待は、結果の種類と分類・文面の ID・差し込みの後の文面と次の行動である。
//!
//! 開始点の状態は、開始点の Step を `Entered`（または `accepted` の指定で `Accepted`）にして直接作る。開始点の効果は
//! 当てない（`Control::Begin` は未実装。TASK-217）。処理中は `busy` の指定で作る。`pre` を持つ誤入力は
//! `Session::apply` で流してから判定する。
//!
//! 確かめないこと: `fandhe-pack test` の結線と出力（TASK-404）、画面の表示（TASK-447・TASK-445）。

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::{copy_dir, testdata, Tmp};
use fandhe_workshop_contract::ir::{Ir, Node};
use fandhe_workshop_contract::judgment::Judgment;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::effect::NoConditions;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::judge::{judge_input, JudgeError};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::{Busy, RunState, Session, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::text_fill::{filled_text, FilledText};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::play_expect::{parse_expect, ResultExpect};
use fandhe_workshop_pack::source;
use fandhe_workshop_pack::validate::validate_dir;
use serde_json::Value;

fn fill_pack_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/fill-pack")
}

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

fn compile(dir: &Path) -> Ir {
    let bytes = compile_dir(dir).unwrap_or_else(|d| panic!("compile {dir:?}: {d:?}"));
    load_ir(&bytes).expect("load_ir")
}

/// 1 件の確認。`expect_*` が `None` のものは、その欄を比べない。
struct Check {
    id: String,
    start: String,
    status: StepStatus,
    busy: Option<Busy>,
    pre: Vec<Input>,
    input: Input,
    expect: ResultExpect,
    rejection: Option<String>,
    message: Option<String>,
    next_action: Option<String>,
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

/// 入力の指定を core の `Input` に写す。未知の形は黙って飛ばさず失敗にする。
fn input_of(label: &str, v: &Value) -> Input {
    let obj = v
        .as_object()
        .unwrap_or_else(|| panic!("{label}: input is not a mapping"));
    let mut found: Option<Input> = None;
    for (k, val) in obj {
        let next = match k.as_str() {
            "expect" | "expect_rejection" | "expect_message" | "expect_next_action" => continue,
            "cmd" => Input::Command(val.as_str().expect("cmd").to_string()),
            "text" => Input::Text(val.as_str().expect("text").to_string()),
            "choice" => Input::Choice(val.as_str().expect("choice").to_string()),
            "ui" => {
                let s = val.as_str().expect("ui");
                let (view, path) = s.split_once(' ').expect("ui: <view> <path>");
                Input::Ui {
                    view: view.to_string(),
                    path: path.to_string(),
                }
            }
            "approval" => Input::Approval {
                decision: match val.as_str().expect("approval") {
                    "approve" => Decision::Approve,
                    "reject" => Decision::Reject,
                    "revise" => Decision::Revise,
                    other => panic!("{label}: unknown decision {other}"),
                },
                target: str_of(v, "target").map(str::to_string),
            },
            "target" => continue,
            "cancel" => Input::Cancel,
            other => panic!("{label}: unknown input key {other}"),
        };
        assert!(found.replace(next).is_none(), "{label}: two input forms");
    }
    found.unwrap_or_else(|| panic!("{label}: no input form"))
}

fn checks_of(dir: &Path, with_error_inputs: bool) -> Vec<Check> {
    let lp = source::load(dir).expect("load");
    let tests = lp.tests.as_ref().expect("tests");
    let mut out = Vec::new();
    let mut lists: Vec<&Vec<Value>> = vec![&tests.text_checks];
    if with_error_inputs {
        lists.push(&tests.error_inputs);
    }
    for list in lists {
        for v in list {
            let id = str_of(v, "id").expect("id").to_string();
            let input_v = &v["input"];
            let input = input_of(&id, input_v);
            let expect = parse_expect(str_of(input_v, "expect").unwrap_or("accepted"))
                .unwrap_or_else(|e| panic!("{id}: {e}"));
            let status = match str_of(v, "status") {
                None | Some("entered") => StepStatus::Entered,
                Some("accepted") => StepStatus::Accepted,
                Some(other) => panic!("{id}: unknown status {other}"),
            };
            let busy = v.get("busy").map(|b| Busy {
                done: u32::try_from(b["done"].as_u64().expect("done")).expect("done"),
                total: u32::try_from(b["total"].as_u64().expect("total")).expect("total"),
            });
            let pre = v
                .get("pre")
                .and_then(Value::as_array)
                .map(|a| a.iter().map(|p| input_of(&id, p)).collect())
                .unwrap_or_default();
            out.push(Check {
                id,
                start: str_of(v, "start").expect("start").to_string(),
                status,
                busy,
                pre,
                input,
                expect,
                rejection: str_of(input_v, "expect_rejection").map(str::to_string),
                message: str_of(input_v, "expect_message").map(str::to_string),
                next_action: str_of(input_v, "expect_next_action").map(str::to_string),
            });
        }
    }
    out
}

fn node_of<'a>(ir: &'a Ir, start_id: &str) -> &'a Node {
    let start = ir
        .starts
        .iter()
        .find(|s| s.id == start_id)
        .unwrap_or_else(|| panic!("start {start_id} not found"));
    ir.nodes
        .iter()
        .find(|n| n.uid == start.node)
        .unwrap_or_else(|| panic!("node {} not found", start.node))
}

/// 開始点の Step に入った状態。効果は当てない。
fn state_of(ir: &Ir, c: &Check) -> SessionState {
    let uid = StepUid(node_of(ir, &c.start).uid.clone());
    let mut slice = Slice::default();
    slice.status.insert(uid.clone(), c.status);
    let mut slices = BTreeMap::new();
    slices.insert(uid.root(), slice);
    let run = RunState {
        current: Some(uid),
        busy: c.busy.clone(),
        slices,
        ..RunState::default()
    };
    let mut session = Session::from_state(
        ir.clone(),
        SessionState {
            run,
            ..SessionState::default()
        },
    );
    for p in &c.pre {
        session.apply(Op::Input(p.clone()));
    }
    session.into_state()
}

/// 1 件の結果。判定できない宣言は `Err`。
struct Judged {
    judgment: Judgment,
    filled: Option<FilledText>,
}

fn judge(ir: &Ir, c: &Check) -> Result<Judged, String> {
    let state = state_of(ir, c);
    let outcome = judge_input(ir, &state.run, &c.input, &NoConditions)
        .map_err(|e: JudgeError| format!("{}: judge_input: {e}", c.id))?;
    let filled = filled_text(ir, &state.run, &c.input, &outcome.judgment)
        .map_err(|e| format!("{}: filled_text: {e}", c.id))?;
    Ok(Judged {
        judgment: outcome.judgment,
        filled,
    })
}

/// 判定の結果が指す文面の ID。
fn text_id(j: &Judgment) -> Option<&str> {
    match j {
        Judgment::Rejected(r) => Some(&r.text),
        Judgment::Clarified(c) => Some(&c.text),
        Judgment::Accepted(Some(n)) => Some(&n.text),
        _ => None,
    }
}

fn next_action_of(j: &Judgment) -> Option<&str> {
    match j {
        Judgment::Rejected(r) => Some(r.next_action.as_str()),
        Judgment::Clarified(c) => Some(c.next_action.as_str()),
        Judgment::Accepted(Some(n)) => Some(n.next_action.as_str()),
        _ => None,
    }
}

/// 期待との差を文字列で返す。一致なら `None`。
fn mismatch(ir: &Ir, c: &Check) -> Option<String> {
    let got = match judge(ir, c) {
        Ok(g) => g,
        Err(e) => return Some(e),
    };
    if !c.expect.matches(&got.judgment) {
        return Some(format!("{}: result {:?}", c.id, got.judgment));
    }
    if let Some(want) = &c.rejection {
        if text_id(&got.judgment) != Some(want.as_str()) {
            return Some(format!("{}: text id {:?}", c.id, text_id(&got.judgment)));
        }
    }
    let filled = got.filled.as_ref()?;
    if let Some(want) = &c.message {
        if &filled.message != want {
            return Some(format!("{}: message {:?}", c.id, filled.message));
        }
    }
    if let Some(want) = &c.next_action {
        if filled.next_action.as_str() != want {
            return Some(format!("{}: next_action {:?}", c.id, filled.next_action));
        }
    }
    None
}

fn fill_checks() -> (Ir, Vec<Check>) {
    let dir = fill_pack_dir();
    let checks = checks_of(&dir, false);
    (compile(&dir), checks)
}

#[test]
fn judge8_fill_pack_has_no_diagnostics() {
    assert_eq!(validate_dir(&fill_pack_dir()), vec![]);
}

#[test]
fn judge8_text_checks_group_a_19() {
    let (ir, checks) = fill_checks();
    let group: Vec<_> = checks.iter().filter(|c| c.id.starts_with("a-")).collect();
    assert_eq!(group.len(), 19);
    let bad: Vec<_> = group.iter().filter_map(|c| mismatch(&ir, c)).collect();
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn judge8_text_checks_all_38() {
    let (ir, checks) = fill_checks();
    assert_eq!(checks.len(), 38);
    let bad: Vec<_> = checks.iter().filter_map(|c| mismatch(&ir, c)).collect();
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn judge8_every_check_states_its_text_and_message() {
    let (_, checks) = fill_checks();
    for c in &checks {
        assert!(
            c.rejection.is_some() && c.message.is_some() && c.next_action.is_some(),
            "{} lacks an expectation",
            c.id
        );
    }
}

#[test]
fn judge8_judged_next_action_equals_filled() {
    let (ir, checks) = fill_checks();
    for c in &checks {
        let got = judge(&ir, c).expect("judge");
        let filled = got.filled.as_ref().expect("filled");
        assert_eq!(
            Some(filled.next_action.as_str()),
            next_action_of(&got.judgment),
            "{}",
            c.id
        );
        assert_eq!(
            Some(filled.text.as_str()),
            text_id(&got.judgment),
            "{}",
            c.id
        );
    }
}

/// 差し込みの後の値で、次の行動が空の拒否・問い返し・注記を数える。返す値は（文面を返した件数、空の件数）。
fn count_empty(label: &str, ir: &Ir, checks: &[Check]) -> (usize, usize) {
    let (mut n, mut empty) = (0, 0);
    for c in checks {
        let got = judge(ir, c).unwrap_or_else(|e| panic!("{label}: {e}"));
        let Some(filled) = got.filled else { continue };
        n += 1;
        if filled.next_action.as_str().trim().is_empty() {
            empty += 1;
        }
    }
    (n, empty)
}

#[test]
fn judge8_r21_no_empty_next_action_after_fill() {
    let mut total = 0;
    let mut cases: Vec<(&str, PathBuf, bool)> = vec![
        ("fill-pack", data("fill-pack"), false),
        ("slot-pack", data("slot-pack"), true),
        ("judgment-pack", data("judgment-pack"), true),
    ];
    cases.push((
        "module-reuse-rounds",
        testdata("module-reuse-rounds/test-data/pack"),
        true,
    ));
    for (label, dir, with_errors) in cases {
        let checks = checks_of(&dir, with_errors);
        let ir = compile(&dir);
        let (n, empty) = count_empty(label, &ir, &checks);
        eprintln!("R-21 {label}: {n} returned, {empty} empty");
        assert_eq!(empty, 0, "{label}");
        assert!(n > 0, "{label}: nothing was returned");
        total += n;
    }
    eprintln!("R-21 total: {total} returned, 0 empty");
}

#[test]
fn judge8_input_head_is_not_rescanned() {
    let (ir, checks) = fill_checks();
    let c = checks.iter().find(|c| c.id == "b-14").expect("b-14");
    let got = judge(&ir, c).expect("judge");
    let filled = got.filled.expect("filled");
    assert!(filled.message.contains("{{step_title}}"));
    assert!(!filled.message.contains("ヘルプを開く"));
}

/// 接頭辞を変えた写しを作る。
fn prefixed(prefix: &str) -> Tmp {
    let t = Tmp::new("fill-prefix");
    copy_dir(&fill_pack_dir(), &t.0);
    let p = t.0.join("workshop.yaml");
    let text = std::fs::read_to_string(&p).expect("read");
    let text = text.replace(
        "  guidance: \"A\"\nviews:",
        &format!("  guidance: \"A\"\n  profile: {{command_prefix: \"{prefix}\"}}\nviews:"),
    );
    assert!(text.contains("profile"), "profile was not inserted");
    std::fs::write(&p, text).expect("write");
    t
}

#[test]
fn judge8_non_default_prefix() {
    let t = prefixed("!");
    assert_eq!(validate_dir(&t.0), vec![]);
    let ir = compile(&t.0);
    let mk = |line: &str| Check {
        id: "prefix".into(),
        start: "s-a".into(),
        status: StepStatus::Entered,
        busy: None,
        pre: vec![],
        input: Input::Command(line.into()),
        expect: ResultExpect::Rejected(
            fandhe_workshop_contract::judgment::RejectCategory::UnknownCommand,
        ),
        rejection: None,
        message: None,
        next_action: None,
    };
    let got = judge(&ir, &mk("!hlep")).expect("judge");
    let filled = got.filled.expect("filled");
    assert_eq!(filled.message, "「!hlep」は、この教材の一覧にありません。");
    assert_eq!(
        filled.next_action.as_str(),
        "「!help」に直して、もう一度送ります。"
    );
}

/// 文面を書き換えた IR で、`Judged` が出ず `Warning` だけになることを確かめる。
fn session_events(ir: Ir, line: &str) -> Vec<Event> {
    let c = Check {
        id: "x".into(),
        start: "s-a".into(),
        status: StepStatus::Entered,
        busy: None,
        pre: vec![],
        input: Input::Command(line.into()),
        expect: ResultExpect::Clarified,
        rejection: None,
        message: None,
        next_action: None,
    };
    let state = state_of(&ir, &c);
    let mut session = Session::from_state(ir, state);
    session
        .apply(Op::Input(Input::Command(line.into())))
        .to_vec()
}

fn with_next_action(mut ir: Ir, id: &str, text: &str) -> Ir {
    ir.rejections
        .get_mut(id)
        .expect("rejection")
        .as_object_mut()
        .expect("object")
        .insert("next_action".into(), Value::String(text.into()));
    ir
}

#[test]
fn judge8_slot_error_fails_closed() {
    let ir = compile(&fill_pack_dir());
    let c = Check {
        id: "x".into(),
        start: "s-a".into(),
        status: StepStatus::Entered,
        busy: None,
        pre: vec![],
        input: Input::Command("/hlep".into()),
        expect: ResultExpect::Clarified,
        rejection: None,
        message: None,
        next_action: None,
    };
    let state = state_of(&ir, &c);
    // 近い候補が無い入力（`/hxxxp`）では、枠だけの次の行動は差し込みの後に空になる。
    for (bad, line, want) in [
        ("{{nope}}", "/hlep", JudgeError::InvalidTextSlot),
        ("{{nearest_command", "/hlep", JudgeError::InvalidTextSlot),
        ("{{nearest_command}}", "/hxxxp", JudgeError::EmptyNextAction),
        ("  ", "/hlep", JudgeError::EmptyNextAction),
    ] {
        let broken = with_next_action(ir.clone(), "rj-unknown-command", bad);
        let input = Input::Command(line.into());
        let err =
            judge_input(&broken, &state.run, &input, &NoConditions).expect_err("must fail closed");
        assert_eq!(err, want, "{bad}");
        let events = session_events(broken, line);
        assert!(
            events.iter().all(|e| !matches!(e, Event::Judged { .. })),
            "{bad}: Judged was emitted"
        );
        assert!(
            events.iter().any(|e| matches!(e, Event::Warning { .. })),
            "{bad}: no Warning"
        );
        let shown = format!("{events:?}");
        assert!(!shown.contains("nope"), "{bad}: Warning echoes the text");
    }
}

/// `do` の無い Step（`s-b`）では、`step_do` は値なしになり、題名で代えない（静的な数え上げと同じ前提）。
#[test]
fn judge8_step_do_is_not_replaced_by_title() {
    let ir = with_next_action(
        compile(&fill_pack_dir()),
        "rj-wrong-order-title",
        "{{step_do|やることなし}}",
    );
    let c = Check {
        id: "x".into(),
        start: "s-b".into(),
        status: StepStatus::Entered,
        busy: None,
        pre: vec![],
        input: Input::Command("/publish-run".into()),
        expect: ResultExpect::Clarified,
        rejection: None,
        message: None,
        next_action: None,
    };
    let got = judge(&ir, &c).expect("judge");
    assert_eq!(next_action_of(&got.judgment), Some("やることなし"));
}

#[test]
fn judge8_session_emits_filled_next_action() {
    let ir = compile(&fill_pack_dir());
    let events = session_events(ir, "/hlep");
    let judged: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Judged { judgment, .. } => Some(judgment),
            _ => None,
        })
        .collect();
    assert_eq!(judged.len(), 1, "{events:?}");
    assert_eq!(
        next_action_of(judged[0]),
        Some("「/help」に直して、もう一度送ります。")
    );
}

#[cfg(test)]
mod property {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// どんな 1 行のコマンドでも、拒否・問い返しの次の行動は空でなく、入力の `{{` は展開されない。
        #[test]
        fn any_command_line_keeps_next_action_non_empty(
            line in "/[a-z{}| ]{0,24}",
            start in prop::sample::select(vec!["s-a", "s-b", "s-c", "s-d", "s-e", "s-f"]),
        ) {
            static IR: std::sync::OnceLock<Ir> = std::sync::OnceLock::new();
            let ir = IR.get_or_init(|| compile(&fill_pack_dir()));
            let c = Check {
                id: "p".into(),
                start: start.into(),
                status: StepStatus::Entered,
                busy: None,
                pre: vec![],
                input: Input::Command(line.clone()),
                expect: ResultExpect::Clarified,
                rejection: None,
                message: None,
                next_action: None,
            };
            if let Ok(got) = judge(ir, &c) {
                if let Some(next) = next_action_of(&got.judgment) {
                    prop_assert!(!next.trim().is_empty());
                }
            }
        }
    }
}
