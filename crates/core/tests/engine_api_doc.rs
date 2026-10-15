//! 公開 API の契約の文書（`docs/engine-api.md`）と型の一致の検査（TASK-204 3/3）。
//!
//! 正は型と rustdoc で、文書は一覧である。文書だけが古くなるのを防ぐため、冒頭の契約の版と
//! 世代の変わり方の表（EVT-6）を、文書を読んで型と照合する。型の一覧など他の記述は確かめない。
//! 呼び出し元は `cargo test`（`make test`）のみである。

use std::fs;
use std::path::Path;

use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::input::{CheckpointId, Control, GenerationRule, Input, Op};
use fandhe_workshop_core::CONTRACT_VERSION;

fn doc() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/engine-api.md");
    fs::read_to_string(&path).expect("契約の文書を読めること")
}

fn cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|c| c.trim().trim_matches('`').trim().to_string())
        .collect()
}

fn uid() -> StepUid {
    StepUid("main/a".to_string())
}

/// 文書の第 1 列の記法に対応する代表の操作。
fn representative(key: &str) -> Op {
    match key {
        "Op::Input(_)" => Op::Input(Input::Command("x".into())),
        "Op::Input(Input::Cancel)" => Op::Input(Input::Cancel),
        "Op::Tick(_)" => Op::Tick(TickAddr {
            generation: Generation::INITIAL,
            step: uid(),
            index: 0,
        }),
        "Op::Control(Control::Stop)" => Op::Control(Control::Stop),
        "Op::Control(Control::GoTo(_))" => Op::Control(Control::GoTo(uid())),
        "Op::Control(Control::Save)" => Op::Control(Control::Save),
        "Op::Control(Control::Restore(_))" => Op::Control(Control::Restore(CheckpointId(1))),
        "Op::Control(Control::Begin(_))" => Op::Control(Control::Begin("s".into())),
        "Op::Control(Control::OpenHint)" => Op::Control(Control::OpenHint),
        "Op::Control(Control::SetGuidance(_))" => {
            Op::Control(Control::SetGuidance(GuidanceLevel::A))
        }
        other => panic!("文書の表に未知の操作の記法がある: {other}"),
    }
}

fn rule(name: &str) -> GenerationRule {
    match name {
        "Keep" => GenerationRule::Keep,
        "IncrementIfProcessing" => GenerationRule::IncrementIfProcessing,
        "IncrementOnSuccess" => GenerationRule::IncrementOnSuccess,
        other => panic!("文書の表に未知の規則がある: {other}"),
    }
}

#[test]
fn doc_contract_version_matches_constant() {
    let d = doc();
    let line = d
        .lines()
        .find(|l| l.starts_with("対象の契約の版（CONTRACT_VERSION）:"))
        .expect("冒頭の囲みに契約の版の行があること");
    let n: u32 = line
        .split_once(':')
        .expect("コロンがあること")
        .1
        .trim()
        .parse()
        .expect("契約の版が整数であること");
    assert_eq!(
        n, CONTRACT_VERSION,
        "文書の契約の版と CONTRACT_VERSION が違う"
    );
}

#[test]
fn doc_generation_table_matches_generation_rule() {
    let d = doc();
    let rows: Vec<Vec<String>> = d
        .lines()
        .filter(|l| l.starts_with("| `Op::"))
        .map(cells)
        .collect();
    // EVT-6 の 6 項目に当たる 7 行（Input と Cancel を分けて書く）と、範囲外の Begin・OpenHint・SetGuidance の 3 行。
    assert_eq!(rows.len(), 10, "世代の表の行数が違う");
    for row in &rows {
        let op = representative(&row[0]);
        assert_eq!(op.generation_rule(), rule(&row[1]), "{}", row[0]);
    }
    let mut outside: Vec<&str> = rows
        .iter()
        .filter(|r| r[2].starts_with("範囲外"))
        .map(|r| r[0].as_str())
        .collect();
    outside.sort_unstable();
    assert_eq!(
        outside,
        [
            "Op::Control(Control::Begin(_))",
            "Op::Control(Control::OpenHint)",
            "Op::Control(Control::SetGuidance(_))",
        ],
        "EVT-6 の外の行は Begin・OpenHint・SetGuidance の 3 行だけである"
    );
}
