//! 教材の通し（`tests/playthrough.yaml` の 1 本）をブラウザ試験の入力の JSON にする（TASK-92 2/2。BUILD-17）。
//!
//! 呼び出し元は `scripts/check-keyboard-playthrough.sh`（`--ignored` の `emit_playthrough_for_browser`）と、
//! `scripts/test-browser.sh`。出力は `tools/browser/keyboard-playthrough.mjs` が読む。YAML を JS で読むと npm の依存が
//! 増えるため、既存の厳格な読み込み器（`source::load`・`play_script::read`）を通した値を JSON にして渡す。
//! 出力のファイル名は固定で、教材の文字列からパスを作らない。通しの ID が 0 件・2 件以上一致するときは止める（fail-closed）。
//! 試験では `unwrap` を使ってよい。

use std::path::{Path, PathBuf};

use fandhe_workshop_pack::play_script;
use fandhe_workshop_pack::source;
use serde_json::{json, Value};

/// 教材を読み、教材テストを検証した上で、ID が一致する通し 1 本を `playthrough.json` 用の値にする。
fn build(pack: &Path, id: &str) -> Value {
    let loaded =
        source::load(pack).unwrap_or_else(|d| panic!("load failed: {} diagnostics", d.len()));
    if let Err(d) = play_script::read(&loaded) {
        panic!("playthrough tests are invalid: {} diagnostics", d.len());
    }
    let tests = loaded.tests.as_ref().expect("tests");
    let hits: Vec<&Value> = tests
        .playthroughs
        .iter()
        .filter(|p| p.get("id").and_then(Value::as_str) == Some(id))
        .collect();
    assert_eq!(hits.len(), 1, "playthrough id must match exactly one");
    let p = hits[0];
    json!({
        "id": p["id"],
        "end": p.get("end").cloned().unwrap_or(Value::Null),
        "segments": p.get("segments").cloned().unwrap_or(Value::Null),
        "asserts": p.get("asserts").cloned().unwrap_or(Value::Null),
    })
}

fn ch0_3_pack() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/tests/data/ch0_3_pack")
}

/// 自 repo の試験用教材の通しが、区間 4 つ・開始点の並び・操作の鍵のまま出ること。
#[test]
fn builds_the_playthrough_of_the_test_pack() {
    let v = build(&ch0_3_pack(), "pt-ch0-3-keys");
    assert_eq!(v["end"], "complete");
    let segs = v["segments"].as_array().unwrap();
    let starts: Vec<&str> = segs.iter().map(|s| s["start"].as_str().unwrap()).collect();
    assert_eq!(starts, ["s-0-1", "s-1-1", "s-2-1", "s-3-1"]);
    let ops: usize = segs
        .iter()
        .map(|s| s["steps"].as_array().unwrap().len())
        .sum();
    assert_eq!(ops, 53);
    assert!(v["asserts"].is_null());
}

#[test]
#[should_panic(expected = "exactly one")]
fn unknown_id_is_refused() {
    build(&ch0_3_pack(), "no-such-playthrough");
}

/// 書き出し。`scripts/` だけが `--ignored` で呼ぶ。入力は環境変数 3 つ（未設定は panic）。
#[test]
#[ignore = "ブラウザ試験の入力を書き出す。scripts/ だけが --ignored で呼ぶ"]
fn emit_playthrough_for_browser() {
    let pack = PathBuf::from(
        std::env::var("FANDHE_PLAYTHROUGH_PACK").expect("FANDHE_PLAYTHROUGH_PACK must be set"),
    );
    let id = std::env::var("FANDHE_PLAYTHROUGH_ID").expect("FANDHE_PLAYTHROUGH_ID must be set");
    let out = PathBuf::from(
        std::env::var("FANDHE_PLAYTHROUGH_OUT").expect("FANDHE_PLAYTHROUGH_OUT must be set"),
    );
    std::fs::create_dir_all(&out).unwrap();
    let v = build(&pack, &id);
    std::fs::write(
        out.join("playthrough.json"),
        serde_json::to_vec(&v).unwrap(),
    )
    .unwrap();
}
