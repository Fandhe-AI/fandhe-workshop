//! 履歴の書き出し（`history_export`、TASK-330 1/2、SAVE-16・SAVE-17）の試験。
//!
//! 書き出したバイト列が読み込みの段 1〜5 を通って元の包みに戻ること、同じ包みから同じバイト列が出ること、
//! `ops` と写しの両方を持つ compact な 1 行であること、読み込みの上限を超えても書き出しは拒否せず印で知らせることを確かめる。
//! 大きさの測定（`record_export_sizes`）は `--nocapture` で `OBS` 行に出す。記録の様式は `docs/testing.md` の 5.66。
//! 履歴は受講者のデータのため、失敗の文には長さだけを出し、中身を出さない。

mod common;

use common::history_sample::{load, read_bytes, SAMPLES};
use fandhe_workshop_contract::limits::{MAX_HISTORY_BYTES, MAX_HISTORY_OPS};
use fandhe_workshop_core::history_envelope::SessionEnvelope;
use fandhe_workshop_core::history_export::{export_bytes, export_report};
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};

fn reload(bytes: &[u8]) -> SessionEnvelope {
    let value = parse_history_json(bytes).expect("exported bytes must pass stages 1-3");
    parse_history_envelope(value).expect("exported bytes must pass stages 4-5")
}

#[test]
fn export_roundtrips_through_the_load_gate() {
    for name in SAMPLES {
        let original = load(name);
        let bytes = export_bytes(&original).expect("sample must export");
        assert_eq!(
            reload(&bytes),
            original,
            "sample {name} changed by the roundtrip"
        );
    }
}

#[test]
fn export_is_byte_deterministic() {
    for name in SAMPLES {
        let original = load(name);
        let first = export_bytes(&original).expect("export");
        let second = export_bytes(&original).expect("export");
        assert_eq!(first, second, "sample {name}: two exports differ");
        let again = export_bytes(&reload(&first)).expect("export");
        assert_eq!(
            first, again,
            "sample {name}: export after the roundtrip differs"
        );
    }
}

#[test]
fn export_has_ops_and_snapshot() {
    for name in SAMPLES {
        let bytes = export_bytes(&load(name)).expect("export");
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        let keys: Vec<&str> = value
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        // serde_json の Map は並べ替え（preserve_order なし）なので、有無と型だけを見る。
        for key in [
            "format",
            "history_format",
            "pack_id",
            "versions",
            "start",
            "ops",
            "snapshot",
        ] {
            assert!(
                keys.contains(&key),
                "sample {name}: missing top-level key {key}"
            );
        }
        assert_eq!(keys.len(), 7, "sample {name}: unexpected top-level keys");
        assert!(value["ops"].is_array());
        assert!(value["snapshot"].is_object());
        // 宣言順（契約の固定の履歴と同じ並び）は、生のバイト列の先頭で見る。
        let text = std::str::from_utf8(&bytes).expect("utf8");
        let positions: Vec<usize> = [
            "format",
            "history_format",
            "pack_id",
            "versions",
            "start",
            "ops",
            "snapshot",
        ]
        .iter()
        .map(|k| text.find(&format!("\"{k}\":")).expect("key position"))
        .collect();
        assert!(
            positions.windows(2).all(|w| w[0] < w[1]),
            "sample {name}: top-level keys are not in declaration order"
        );
    }
}

#[test]
fn export_is_compact_single_line() {
    for name in SAMPLES {
        let bytes = export_bytes(&load(name)).expect("export");
        assert!(
            !bytes.contains(&b'\n'),
            "sample {name}: export contains a newline"
        );
        assert!(
            !bytes.windows(2).any(|w| w == b": "),
            "sample {name}: export is not compact"
        );
    }
}

#[test]
fn export_keeps_log_order_and_positions() {
    for name in SAMPLES {
        let original = load(name);
        let back = reload(&export_bytes(&original).expect("export"));
        assert_eq!(
            back.snapshot.log, original.snapshot.log,
            "sample {name}: log order changed"
        );
        assert_eq!(
            back.snapshot.checkpoints, original.snapshot.checkpoints,
            "sample {name}: checkpoint positions changed"
        );
        assert_eq!(back.ops, original.ops, "sample {name}: ops changed");
    }
}

#[test]
fn report_flags_over_limit_without_refusing() {
    let env = load("two-rounds");
    let bytes = export_bytes(&env).expect("export");
    let report = export_report(&bytes, &env);
    assert_eq!(report.bytes, bytes.len());
    assert_eq!(report.ops, env.ops.len());
    assert!(!report.exceeds_size_limit && !report.exceeds_ops_limit);

    let limit = usize::try_from(MAX_HISTORY_BYTES).expect("limit fits usize");
    let at = vec![b' '; limit];
    let over = vec![b' '; limit + 1];
    assert!(
        !export_report(&at, &env).exceeds_size_limit,
        "exactly at the limit is within"
    );
    assert!(
        export_report(&over, &env).exceeds_size_limit,
        "one byte over is flagged"
    );

    // 件数の上限を超える包みでも、書き出し自体は Err にならない。
    let mut many = env.clone();
    let first = many.ops.first().expect("sample has ops").clone();
    many.ops = vec![first; MAX_HISTORY_OPS + 1];
    let many_bytes = export_bytes(&many).expect("over-limit history must still export");
    let many_report = export_report(&many_bytes, &many);
    assert!(many_report.exceeds_ops_limit);
    assert_eq!(many_report.ops, MAX_HISTORY_OPS + 1);
}

#[test]
fn record_export_sizes() {
    // U-5・SAVE-26 の保存先の上限（5MB）と、読み込みの上限（10MB）。
    const STORAGE_BUDGET: usize = 5 * 1024 * 1024;
    let load_limit = usize::try_from(MAX_HISTORY_BYTES).expect("limit fits usize");
    for name in SAMPLES {
        let env = load(name);
        let total = export_bytes(&env).expect("export").len();
        let ops = serde_json::to_vec(&env.ops).expect("ops").len();
        let snapshot = serde_json::to_vec(&env.snapshot).expect("snapshot").len();
        println!(
            "OBS export_size sample={name} total={total} ops={ops} snapshot={snapshot} ops_count={}",
            env.ops.len()
        );
        assert!(
            total <= STORAGE_BUDGET,
            "sample {name}: {total} bytes exceed 5MB"
        );
        assert!(
            total <= load_limit,
            "sample {name}: {total} bytes exceed the load limit"
        );
        assert!(
            ops > 0 && snapshot > 0 || name == "empty",
            "sample {name}: a section is empty"
        );
    }
    // 参考: PoC-6 の固定の履歴（2 周分。試作の教材・PoC 時点の写しの形）のファイルの大きさ。今の型では読めないため比べるだけ。
    let reference = std::fs::metadata(
        common::contract_sample::testdata_dir()
            .join("save-restore-versions/test-data-addendum/histories/h-full.json"),
    );
    if let Ok(meta) = reference {
        println!("OBS export_size reference=poc6-h-full total={}", meta.len());
    }
    // 同じ入力を使うので、読み込み側の見本の大きさとの関係も記録しておく。
    let two_rounds_file = read_bytes("two-rounds").len();
    println!("OBS export_size sample_file=two-rounds pretty_total={two_rounds_file}");
}
