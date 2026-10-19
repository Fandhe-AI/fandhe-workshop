//! 新しい項目を使わない教材パックの IR が、受け口（拡張点）の追加の前後でバイト単位で同じことの固定
//! （04-behavior の EXT-9・D-52・D-53、TASK-190 の 2/2）。
//!
//! 「追加の前」は、実在する受け口の追加 EP-13（`views.<ID>.body`、語彙 1.4）の親コミット 0fa7146 のエンジンである。
//! `tests/data/golden/*.ir.json` の 5 本は、最後にそのコミットで書かれたもの（以後作り直していない）で、
//! `tests/data/ir_before_ep13/*.ir.json` は同じエンジンで作った凍結物である（作り方は同ディレクトリの README）。
//! 今のエンジンの出力がどちらともバイト一致すれば、受け口の追加で、使わない教材の IR は変わっていない。
//! 受け口ごとの版は持たない（D-53）ことも、IR の版の鍵の集合と、足すと断られることで固定する。
//!
//! 分担: 旧い教材と履歴が開くこと（1/2）は `old_pack_history_open.rs`、版 2 の出力の形は `ir_format.rs`、
//! 欄の注入の拒否は `ir_compat.rs` が受け持つ。
//!
//! 限界: 受け口の追加の実例は EP-13 の 1 件だけで、次の追加は試験できない。golden の出所はコミットの履歴に頼る
//! （試験はそれを見ない）。試験では `unwrap` を使ってよい。失敗の文言には教材名とバイト位置だけを出し、本文を出さない。

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{copy_dir, testdata, Tmp, WORKSHOP};
use fandhe_workshop_contract::ir::{to_canonical_json, Ir};
use fandhe_workshop_core::ir_load::{load_ir, IrLoadError};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::DiagCode;
use serde_json::{json, Value};

const SENTINEL_KEY: &str = "extension_versions";
const SENTINEL_VALUE: &str = "SECRET-VALUE-EXT9";

fn data(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(rel)
}

/// `ir_format.rs` の `packs()` と同じ組。出力の正は `golden/<名前>.ir.json`。
fn golden_packs() -> Vec<(&'static str, PathBuf)> {
    vec![
        (
            "min-3step-pack",
            testdata("engine-event-consistency/test-data/min-3step-pack"),
        ),
        ("ch1-pack", testdata("input-field-ime/test-data/ch1-pack")),
        ("ime-pack", testdata("input-field-ime/test-data/ime-pack")),
        ("min-3step-pack-b", data("min-3step-pack-b")),
        ("fixture-pack", data("fixture-pack")),
    ]
}

/// golden の無い教材。出力の正は `ir_before_ep13/<名前>.ir.json`。
fn other_packs() -> Vec<(&'static str, PathBuf)> {
    [
        "confirm-kinds-pack",
        "confirm-label-pack",
        "fill-pack",
        "input-kinds-pack",
        "judgment-pack",
        "slot-pack",
        "chosen-value-pack",
    ]
    .into_iter()
    .map(|n| (n, data(n)))
    .collect()
}

/// 本文も練習データも持たない最小パック。`ir_before_ep13/README.md` の入力と同じ。
fn no_tree_pack() -> Tmp {
    let t = Tmp::new("ext9-no-tree");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    let chapter = concat!(
        "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n",
        "  - {id: a, title: A, next: [{to: b}]}\n",
        "  - {id: b, title: B, final: true}\n",
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t
}

fn frozen(dir: &str, name: &str) -> Vec<u8> {
    fs::read(data(&format!("{dir}/{name}.ir.json")))
        .unwrap_or_else(|_| panic!("cannot read frozen {dir}/{name}"))
}

/// 最初に違うバイトの位置（長さが違うだけなら短い方の長さ）。
fn first_diff(a: &[u8], b: &[u8]) -> usize {
    (0..a.len().min(b.len()))
        .find(|&i| a[i] != b[i])
        .unwrap_or(a.len().min(b.len()))
}

fn assert_identical(name: &str, got: &[u8], want: &[u8]) {
    assert!(
        got == want,
        "{name}: IR differs from the frozen output at byte {} (got {} bytes, frozen {} bytes)",
        first_diff(got, want),
        got.len(),
        want.len()
    );
}

/// 2 つの JSON の違う位置（JSON Pointer）の一覧。
fn diff_paths(a: &Value, b: &Value, at: &str, out: &mut Vec<String>) {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                let p = format!("{at}/{k}");
                match (x.get(k), y.get(k)) {
                    (Some(l), Some(r)) => diff_paths(l, r, &p, out),
                    _ => out.push(p),
                }
            }
        }
        _ if a == b => {}
        _ => out.push(at.to_string()),
    }
}

#[test]
fn ext9_premise_frozen_set_does_not_use_new_items() {
    let all = golden_packs().into_iter().chain(other_packs());
    for (name, dir) in all {
        let bytes = compile_dir(&dir).unwrap_or_else(|_| panic!("{name} must compile"));
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        for (id, view) in v["views"].as_object().into_iter().flatten() {
            assert!(
                view.get("body").is_none(),
                "{name}: view {id} uses the new item"
            );
        }
        let vocab = v["pack"].get("vocab").and_then(Value::as_str);
        assert!(
            matches!(vocab, None | Some("1.0" | "1.1" | "1.2" | "1.3")),
            "{name}: pack.vocab is not below 1.4"
        );
    }
}

#[test]
fn ext9_golden_packs_are_byte_identical_across_ep13() {
    for (name, dir) in golden_packs() {
        let got = compile_dir(&dir).unwrap_or_else(|_| panic!("{name} must compile"));
        assert_identical(name, &got, &frozen("golden", name));
    }
}

#[test]
fn ext9_other_packs_are_byte_identical_across_ep13() {
    for (name, dir) in other_packs() {
        let got = compile_dir(&dir).unwrap_or_else(|_| panic!("{name} must compile"));
        assert_identical(name, &got, &frozen("ir_before_ep13", name));
    }
}

#[test]
fn ext9_pack_without_trees_stays_v1_and_identical_across_ep13() {
    let t = no_tree_pack();
    let got = compile_dir(&t.0).expect("compiles");
    let ir: Ir = serde_json::from_slice(&got).expect("IR parses");
    assert_eq!((ir.format_version, ir.pack.ir_format), (1, 1));
    assert!(to_canonical_json(&ir).expect("serialize") == got);
    assert_identical(
        "no-tree-pack",
        &got,
        &frozen("ir_before_ep13", "no-tree-pack"),
    );
}

/// 対照: 新しい項目を使うと、違うのはその項目の鍵だけになる。上の一致が空の主張でないことを示す。
#[test]
fn ext9_using_the_new_item_changes_only_its_own_key() {
    let with_item = data("document-body-pack");
    let tmp = Tmp::new("ext9-body-omitted");
    copy_dir(&with_item, &tmp.0);
    let path = tmp.0.join("workshop.yaml");
    let text = fs::read_to_string(&path).unwrap();
    let from = "{kind: document, body: shown}";
    assert!(text.contains(from));
    fs::write(&path, text.replace(from, "{kind: document}")).unwrap();

    let a: Value = serde_json::from_slice(&compile_dir(&with_item).unwrap()).unwrap();
    let b: Value = serde_json::from_slice(&compile_dir(&tmp.0).unwrap()).unwrap();
    let mut paths = Vec::new();
    diff_paths(&a, &b, "", &mut paths);
    assert_eq!(paths, vec!["/views/v-doc/body".to_string()]);
    assert_eq!(a["format_version"], b["format_version"]);
    assert_eq!(a["pack"]["ir_format"], b["pack"]["ir_format"]);
}

/// D-53: 受け口ごとの版は持たない。版を表す鍵は 4 つだけで、受け口ごとの欄は断られる。
#[test]
fn ext9_ir_has_no_per_extension_point_version() {
    let all = golden_packs().into_iter().chain(other_packs());
    for (name, dir) in all {
        let v: Value = serde_json::from_slice(&compile_dir(&dir).unwrap()).unwrap();
        let top = v.as_object().unwrap().keys().map(|k| k.to_string());
        let pack = v["pack"]
            .as_object()
            .unwrap()
            .keys()
            .map(|k| format!("pack.{k}"));
        for k in top.chain(pack) {
            if k.contains("version") || k.contains("format") || k.contains("vocab") {
                assert!(
                    [
                        "format_version",
                        "pack.ir_format",
                        "pack.version",
                        "pack.vocab"
                    ]
                    .contains(&k.as_str()),
                    "{name}: unexpected version-like key {k}"
                );
            }
        }
    }

    // 教材の宣言に受け口ごとの版の欄を足すと、compile が断る。診断は値を写さない。
    let t = no_tree_pack();
    let y = WORKSHOP.replace(
        "guidance: A}",
        &format!("guidance: A, {SENTINEL_KEY}: {{x: {SENTINEL_VALUE}}}}}"),
    );
    t.put("workshop.yaml", y.as_bytes());
    let Err(diags) = compile_dir(&t.0) else {
        panic!("a per-extension-point version field must be refused")
    };
    assert!(diags.iter().any(|d| d.code() == DiagCode::Parse));
    for d in &diags {
        assert!(
            !d.to_string().contains(SENTINEL_VALUE),
            "diagnostic leaks the value"
        );
    }

    // IR の pack に同じ欄を足すと、読み込みが断る。理由は鍵も値も写さない。
    let mut v: Value = serde_json::from_slice(&frozen("golden", "fixture-pack")).unwrap();
    v["pack"][SENTINEL_KEY] = json!({"x": SENTINEL_VALUE});
    let Err(e) = load_ir(&serde_json::to_vec(&v).unwrap()) else {
        panic!("the IR must not accept a per-extension-point version field")
    };
    assert!(matches!(e, IrLoadError::Json(_)));
    let s = e.to_string();
    assert!(
        !s.contains(SENTINEL_VALUE) && !s.contains(SENTINEL_KEY),
        "{s}"
    );
}
