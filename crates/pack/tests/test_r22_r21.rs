//! `fandhe-pack test` の R-22・R-21 の集計（TASK-405 1/2、AUTHOR-5）の結合試験。
//!
//! 教材パックの CLI 出力の位置と文言、差し込みの後に「次の行動」が空になる IR の検出（ライブラリの
//! `play_run::run` に壊した IR を渡す）、0 であるべき数が 0 でないときの診断と合否、値を写さないことを確かめる。
//! 確認なしの達成は core の経路では作れないため、`Report` に直接置いて診断の形を確かめる。
//! 入った実体の行は `test_entered_steps.rs`、確かめないこと: 最終行（後続）。

use std::path::{Path, PathBuf};
use std::process::Command;

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_pack::cli_contract::{test_r21_line, test_r22_line};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::play_expect::ResultExpect;
use fandhe_workshop_pack::play_run::{run, CaseResult, Report, Tally};
use fandhe_workshop_pack::play_script::{ErrorInput, PlayOp, Script};
use serde_json::Value;

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

fn example(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples")
        .join(name)
}

fn cli_out(dir: &Path) -> (Option<i32>, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .arg("test")
        .arg(dir)
        .output()
        .expect("run");
    (
        o.status.code(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

#[test]
fn author5_r22_r21_lines_follow_summary() {
    // 手で数えた値。minimal-pack は確認の操作 1 件・`confirm: none` の達成 1 件・拒否の文面 3 件。
    let (code, out, err) = cli_out(&example("minimal-pack"));
    assert_eq!(code, Some(0));
    assert_eq!(err, "");
    let tail = [
        "入力の総数: 6",
        "R-22 受付と達成の分離: 確認の操作で達成になった 1 件、確認を省くと明示した Step の達成 1 件、確認なしで達成になった 0 件",
        "R-21 返した文面 3 件のうち「次の行動」が空 0 件",
        "試験で入った Step の実体: 2/2（入らなかった実体: なし）",
    ]
    .join("\n");
    assert!(out.ends_with(&format!("{tail}\n")), "{out}");
    assert_eq!(out.matches("R-22 ").count(), 1);
    assert_eq!(out.matches("R-21 ").count(), 1);
}

#[test]
fn author5_confirm_operations_are_counted() {
    let (_, out, _) = cli_out(&example("choice-only-pack"));
    assert!(out.contains(&test_r22_line(2, 0, 0)), "{out}");
    assert!(out.contains(&test_r21_line(1, 0)), "{out}");
}

fn fill_ir() -> Ir {
    let bytes = compile_dir(&data("fill-pack")).expect("compile");
    load_ir(&bytes).expect("load_ir")
}

fn script() -> Script {
    Script {
        playthroughs: vec![],
        error_inputs: vec![ErrorInput {
            id: "e-hlep".into(),
            start: "s-a".into(),
            pre: vec![],
            input: PlayOp::Command("/hlep".into()),
            expect: ResultExpect::Clarified,
        }],
        unsupported: vec![],
    }
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
fn author5_empty_next_action_after_fill_is_diagnosed() {
    // 対照: 元の IR では文面が返り、空は 0 件。
    let base = run(&fill_ir(), &script());
    assert_eq!(base.tally().empty_next, 0);
    assert_eq!(base.tally().texts, 1);
    // 次の行動を空白だけにすると、差し込みの後に空になり、core は文面を返さない。
    let broken = with_next_action(fill_ir(), "rj-unknown-command", "  ");
    let mut r = run(&broken, &script());
    r.tests_file = "tests/play.yaml".into();
    assert_eq!(r.tally().empty_next, 1);
    assert_eq!(r.tally().texts, 1);
    assert!(!r.all_passed());
    let shown = format!("{:?}", r.diagnostics());
    assert!(shown.contains("EmptyNextAction"), "{shown}");
    assert!(shown.contains("error_input:e-hlep"), "{shown}");
    assert!(!shown.contains("/hlep"), "{shown}");
}

#[test]
fn author5_unconfirmed_achievement_is_diagnosed() {
    let mut r = Report {
        tests_file: "tests/play.yaml".into(),
        ..Report::default()
    };
    r.playthroughs.push(CaseResult {
        id: "pt".into(),
        inputs: 1,
        failure: None,
        observed: None,
        tally: Tally {
            unconfirmed: 1,
            ..Tally::default()
        },
    });
    assert!(!r.all_passed());
    let d = r.diagnostics();
    assert_eq!(d.len(), 1);
    let s = format!("{:?}", d[0]);
    assert!(
        s.contains("NoConfirm") && s.contains("playthrough:pt"),
        "{s}"
    );
    assert_eq!(
        test_r22_line(0, 0, 1),
        "R-22 受付と達成の分離: 確認の操作で達成になった 0 件、確認を省くと明示した Step の達成 0 件、確認なしで達成になった 1 件"
    );
}
