//! `fandhe-pack test` の入った Step の実体の行（TASK-405 2/2、AUTHOR-6）の結合試験。
//!
//! 入った数と入らなかった実体の表示、失敗にしないこと（終了コード 0・標準エラー出力が空）、開始の節点と 2 つ目以降の
//! 開始点の切り替えで入った Step を数えること、教材データ由来の名前を写せる形のときだけ写すことを確かめる。
//! 確かめないこと: 掲示板教材の基準値（34/40）との一致（教材が本 repo に無い。TASK-84）、最終行（後続）。

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{copy_dir, Tmp};

use fandhe_workshop_pack::cli_contract::test_entered_line;
use fandhe_workshop_pack::play_run::{run_pack, Report, TestOutcome};

const NONE_MISSED: &str = "試験で入った Step の実体: 3/3（入らなかった実体: なし）";
const ST3_MISSED: &str = "試験で入った Step の実体: 2/3（入らなかった実体: main/st-3）";

fn min_pack() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/min-3step-pack-b")
}

fn cli(dir: &Path) -> (Option<i32>, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .arg("test")
        .arg(dir)
        .output()
        .expect("run fandhe-pack");
    (
        o.status.code(),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn ran(dir: &Path) -> Report {
    match run_pack(dir) {
        TestOutcome::Ran(r) => r,
        other => panic!("expected Ran: {other:?}"),
    }
}

/// st-1 から st-2 まで進める模範解答の入力列（`test_command.rs` の `min_with_ticks` と同じ）。
fn to_st2(end: &str) -> Vec<String> {
    [
        "playthroughs:",
        "  - id: pt-min",
        &format!("    end: {end}"),
        "    segments:",
        "      - start: s-1",
        "        steps:",
        "          - {cmd: \"/make-note first-note\", expect: accepted}",
        "          - {tick: true, expect: progress}",
        "          - {tick: true, expect: progress}",
        "          - {tick: true, expect: progress}",
        "          - {tick: true, expect: progress}",
        "          - {tick: true, expect: progress}",
        "          - {ui: \"v-files notes/draft\", expect: achieved}",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// 作業ツリーを変えないため一時ディレクトリに写し、開始点 `s-3`（st-3）を足して教材テストを差し替える。
fn pack_with(tag: &str, tests: &[String]) -> Tmp {
    let t = Tmp::new(tag);
    copy_dir(&min_pack(), &t.0);
    let ch = "chapters/ch-min/chapter.yaml";
    let src = std::fs::read_to_string(t.0.join(ch)).expect("chapter");
    let with_start = src.replacen(
        "    node: \"st-1\"\n",
        "    node: \"st-1\"\n  - id: \"s-3\"\n    node: \"st-3\"\n",
        1,
    );
    assert_ne!(src, with_start, "開始点を足せること");
    t.put(ch, with_start.as_bytes());
    t.put("tests/playthrough.yaml", tests.join("\n").as_bytes());
    t
}

#[test]
fn author6_missed_steps_are_shown_and_not_a_failure() {
    let t = pack_with("t405b-missed", &to_st2("main/st-2"));
    let (code, out, err) = cli(&t.0);
    assert_eq!(code, Some(0), "{out}");
    assert_eq!(err, "");
    assert!(out.ends_with(&format!("{ST3_MISSED}\n")), "{out}");
    assert_eq!(out.matches("試験で入った Step の実体").count(), 1);
    assert!(ran(&t.0).all_passed());
}

#[test]
fn author6_start_step_is_counted_as_entered() {
    // 誤入力の開始点 s-3 だけが st-3 に入る。`Session::entered_at` は入場のイベントを出さないため、
    // 開始の節点を数えないと st-3 は入らなかったままになる。
    let mut tests = to_st2("main/st-2");
    tests.extend(
        [
            "error_inputs:",
            "  - id: e-at-s3",
            "    start: s-3",
            "    input: {cmd: \"/nothing\", expect: \"rejected:unknown_command\"}",
        ]
        .map(String::from),
    );
    let t = pack_with("t405b-start", &tests);
    let rep = ran(&t.0);
    assert!(rep.all_passed(), "{rep:?}");
    assert!(rep.entered.contains("main/st-3"), "{:?}", rep.entered);
    let (_, out, _) = cli(&t.0);
    assert!(out.contains(NONE_MISSED), "{out}");
}

#[test]
fn author6_begin_in_a_later_segment_is_counted() {
    // 2 つ目の区間は `Control::Begin` で s-3 へ重ねる。この経路は `step_once` を通らない。
    let mut tests = to_st2("main/st-3");
    tests.extend(["      - start: s-3", "        steps: []"].map(String::from));
    let t = pack_with("t405b-begin", &tests);
    let rep = ran(&t.0);
    assert!(rep.entered.contains("main/st-3"), "{:?}", rep.entered);
    let (n, m, missed) = rep.entered_steps();
    assert_eq!((n, m, missed.len()), (3, 3, 0));
}

#[test]
fn author6_missed_list_follows_ir_order_with_positions() {
    let r = Report {
        steps: vec!["main/st-1".into(), "main/st 2".into(), "mod@i1/u-b".into()],
        entered: ["main/st-1".to_string()].into_iter().collect(),
        ..Report::default()
    };
    let (n, m, missed) = r.entered_steps();
    assert_eq!((n, m), (1, 3));
    assert_eq!(missed, vec![(2, "main/st 2"), (3, "mod@i1/u-b")]);
}

#[test]
fn author6_line_has_no_newline_for_replaced_names() {
    let line = test_entered_line(1, 3, &["#2".to_string(), "main/st-3".to_string()]);
    assert_eq!(
        line,
        "試験で入った Step の実体: 1/3（入らなかった実体: #2, main/st-3）"
    );
    assert!(!line.contains('\n'));
}
