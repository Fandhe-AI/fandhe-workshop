//! glue（`src/glue.rs`）のコード行の上限を固定する試験（FEND-20・TASK-362 2/2）。
//!
//! FEND-20 は glue の行数を記録し、増えたら理由を求める。ここでは上限を試験の定数に置き、超えたら失敗にする。
//! 上限を上げる PR は、定数と `docs/testing.md` 5.166 の表を同時に更新し、PR 本文に理由を書く。
//! 数え方は `docs/testing.md` 5.92 と同じ（空行と `//` で始まる行を除いた行数）で、系列を比べられる。
//! 読み込みは `CARGO_MANIFEST_DIR` からの相対で、変異注入の一時コピーでもコピー側が読まれる。

mod common;

use common::glue_source;

/// glue.rs のコード行の上限（2026-10-22 の実測は 562 行。557 から上げた理由は TASK-112 2/2 の同梱の IR の受信上限の公開 `ir_read_limit`。531 から上げた理由は TASK-112 2/2 の `mount_app` の引数 `ir`（同梱の IR のバイト列。署名が rustfmt で複数行になる分）、TASK-531 1/2 の履歴の読み込みの配送 `import_history`・読み取りの上限 `history_read_limit`・書き出しの URL の口（`blob:`）の結線、527 から上げた理由は TASK-335 の試験用の口 `export_live`（表示だけで開いた履歴の書き出しの読み取り。`test-hooks` のときだけ）、509 から上げた理由は TASK-517 2/2 のキーで選んだタブへのフォーカス移動（`on_pane_focus_request`。`a11y::tab_key_target` の契約）、491 から上げた理由は TASK-284 2/2 の拒否の後の下書きの全選択の予約（`on_draft_select_request`・`sync_draft` の `select`。D-67）、401 から上げた理由は TASK-318 の遅延保存のタイマーと、その結果・予約の無効化・保存世代の配送、449 から上げた理由は TASK-164 3/3 の保存直前の判定 `secret_gate`（確認済みの除外。借りられなければ厳しい側）、452 から上げた理由は TASK-283 の `beforeinput`（上限を超える貼り付けの切り捨て。`wire_paste_clip`）。`docs/testing.md` 5.166・5.170・5.183・5.189・5.200）。
///
/// spec（FEND-7・FEND-20）の「349 行」は決定時点の PoC の追試の値で、数え方と範囲が今と違う
/// （`docs/testing.md` 5.92）。今のソースが通らなくなるため目標にしない。今の実測値を上限とし、増えたときだけ理由を求める。
const MAX_GLUE_CODE_LINES: usize = 562;

/// 空行と、前後の空白を除いて `//` で始まる行を除いた行数。`/* */` は glue に 0 件（数え方は 5.92 と同じ）。
fn code_lines(src: &str) -> usize {
    src.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("//"))
        .count()
}

/// 上限を超えたときの失敗の文言。上げ方を示す。
fn over_limit_message(n: usize) -> String {
    let rule = "FEND-20 は増加に理由を求める。PR に理由を書き、MAX_GLUE_CODE_LINES と docs/testing.md 5.166 の表を同時に更新する";
    format!("glue.rs のコード行が {n} 行（上限 {MAX_GLUE_CODE_LINES}）。{rule}")
}

fn check(src: &str) -> Result<usize, String> {
    let n = code_lines(src);
    if n > MAX_GLUE_CODE_LINES {
        Err(over_limit_message(n))
    } else {
        Ok(n)
    }
}

#[test]
fn glue_のコード行は上限以下() {
    let n = check(&glue_source()).unwrap_or_else(|m| panic!("{m}"));
    // 数え方が空振りしていないことの確認（0 行だと上限の照合が素通りする）。
    assert!(n > 100, "{n}");
}

#[test]
fn コード行の数え方は空行とコメントの行を除く() {
    let src = "fn a() {\n\n    // c\n    let x = 1; // 行末のコメントは数える\n}\n   \n/// doc\n";
    assert_eq!(code_lines(src), 3);
}

#[test]
fn 負例_上限ちょうどは通り_上限を_1_行超えると失敗する() {
    let at_limit = "let x = 1;\n".repeat(MAX_GLUE_CODE_LINES);
    assert_eq!(check(&at_limit), Ok(MAX_GLUE_CODE_LINES));
    let over = format!("{at_limit}let y = 2;\n");
    let err = check(&over).expect_err("上限超過は失敗のはず");
    assert!(err.contains("docs/testing.md"), "{err}");
}

#[test]
fn 負例_コメントと空行を足しても行数は増えない() {
    let base = "let x = 1;\n".repeat(MAX_GLUE_CODE_LINES);
    let padded = format!("{base}\n// 追記\n\n");
    assert_eq!(check(&padded), Ok(MAX_GLUE_CODE_LINES));
}
