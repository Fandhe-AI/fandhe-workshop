//! 旧新 IR の読み込みの互換（04-behavior の VER-13、TASK-181 の 1/2）。
//!
//! 新しい欄を足した IR を、旧いエンジンの配信物と新しい教材が混ざる場面で開いたときの期待を、決定に合わせて固定する。
//! 期待の正は D-51（新しい欄を足すときは `ir_format` を上げる）・D-52（変換器は使った項目に足りる最小の
//! `ir_format` を出す）・D-59（エンジンは `ir_format` 1 と 2 を読む）である。試作では `Ir`・`Node` が未知の欄を
//! 許したため「本文の木だけを足した IR は旧いエンジンでも読める」が成り立ったが、製品では木を使えば
//! `ir_format` が 2 になり、版 1 だけを読む旧いエンジンは版の理由で断る。
//!
//! 旧いエンジンの実物（過去の版のビルド）はこの repo に無い。そこで試験の中に「版 1 だけを読み、確認の選択肢の
//! `label`（v1.3）を知らない旧いエンジン」の模型（`old_engine`）を置く。模型は、版の先読み・`load_ir`（関門・型・
//! 参照の整合は製品と共有する）・凍結した旧い型での `answer` の宣言の読みの順で動く。模型であり実物ではない。
//! 旧いエンジンで実際に開く確認と、表示・保存済みの履歴の不変は TASK-181 の 2/2（#456）が受け持つ。
//!
//! 限界: 製品の `load_ir` は `confirm` を読み込み時に型付けしない。`confirm` の中の未知の欄を製品が断るのは
//! 確認の判定時である。読み込み時の検査を足すのは振る舞いの変更で、本件の範囲外とする
//! （`ver13_future_field_in_answer_option_is_refused_at_judging_not_loading`）。
//!
//! 版 1 の golden の読みとバイト一致は `ir_compat.rs`（`old_to_new_compile_goldens_roundtrip_byte_identical`）、
//! 版 1 と名乗る木つきの IR の拒否は同じ `v1_declared_ir_with_each_v2_field_is_rejected_without_echo`、
//! 型の水準は `crates/contract/tests/ir_compat.rs`・`ir_v1_handling.rs` が受け持つ。ここでは重ねない。
//! golden は読むだけで書き換えない。試験では `unwrap` を使ってよい。

mod common;

use std::fs;
use std::path::Path;

use common::{Tmp, WORKSHOP};
use fandhe_workshop_contract::confirm_spec::answer_spec_of;
use fandhe_workshop_contract::ir::{to_canonical_json, Ir};
use fandhe_workshop_core::ir_load::{load_ir, IrLoadError};
use fandhe_workshop_pack::compile::compile_dir;
use serde_json::{json, Value};

const SENTINEL_LABEL: &str = "SECRET-LABEL";
const SENTINEL_KEY: &str = "future_v3_x";
const V1_GOLDENS: [&str; 4] = ["min-3step-pack", "ch1-pack", "ime-pack", "min-3step-pack-b"];
const V2_GOLDENS: [&str; 5] = [
    "min-3step-pack",
    "ch1-pack",
    "ime-pack",
    "min-3step-pack-b",
    "fixture-pack",
];

/// 版 1 だけを読み、確認の選択肢の `label` を知らない旧いエンジンの模型。
mod old_engine {
    use super::*;
    use serde::Deserialize;
    use std::fmt;

    /// 凍結した旧い `answer` の選択肢（v1.2 まで。`label` を持たない）。
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code, reason = "読めるかどうかだけを見る凍結した型")]
    struct AnswerOptionV12 {
        id: String,
        correct: bool,
        #[serde(default)]
        on_wrong: Option<String>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    #[allow(dead_code, reason = "読めるかどうかだけを見る凍結した型")]
    struct AnswerSpecV12 {
        options: Vec<AnswerOptionV12>,
        on_wrong: String,
    }

    /// 旧いエンジンが断る理由の分類。入力値は含めない。
    #[derive(Debug)]
    pub enum OldEngineError {
        /// 版の先読みで断る（`format_version`・`pack.ir_format` のどちらかが 1 以外）。
        Version {
            format_version: u64,
            pack_ir_format: u64,
        },
        /// 製品の読み込み口が断る。
        Load(IrLoadError),
        /// 凍結した旧い型で `answer` の宣言を読めない（未知の欄）。
        Type,
    }

    impl fmt::Display for OldEngineError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                OldEngineError::Version {
                    format_version,
                    pack_ir_format,
                } => write!(
                    f,
                    "unsupported ir format (format_version={format_version}, pack.ir_format={pack_ir_format})"
                ),
                OldEngineError::Load(e) => write!(f, "load error: {e}"),
                OldEngineError::Type => f.write_str("answer declaration has an unknown field"),
            }
        }
    }

    pub fn load(bytes: &[u8]) -> Result<Ir, OldEngineError> {
        if let Ok(v) = serde_json::from_slice::<Value>(bytes) {
            let fv = v.get("format_version").and_then(Value::as_u64);
            let pf = v.pointer("/pack/ir_format").and_then(Value::as_u64);
            if let (Some(fv), Some(pf)) = (fv, pf) {
                if fv != 1 || pf != 1 {
                    return Err(OldEngineError::Version {
                        format_version: fv,
                        pack_ir_format: pf,
                    });
                }
            }
        }
        let ir = load_ir(bytes).map_err(OldEngineError::Load)?;
        for n in &ir.nodes {
            let Some(confirm) = &n.step.confirm else {
                continue;
            };
            if confirm.get("kind").and_then(Value::as_str) != Some("answer") {
                continue;
            }
            let mut map = confirm.as_object().unwrap().clone();
            map.remove("kind");
            serde_json::from_value::<AnswerSpecV12>(Value::Object(map))
                .map_err(|_| OldEngineError::Type)?;
        }
        Ok(ir)
    }
}

use old_engine::OldEngineError;

fn golden_in(dir: &str, name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(dir)
        .join(format!("{name}.ir.json"));
    fs::read(&p).unwrap_or_else(|_| panic!("cannot read golden {dir}/{name}"))
}

fn bytes(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap()
}

/// `answer` の確認を持つ Step の位置。
fn answer_nodes(v: &Value) -> Vec<usize> {
    v["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, n)| n["step"]["confirm"]["kind"] == "answer")
        .map(|(i, _)| i)
        .collect()
}

/// 版 1 の `ch1-pack` の位置 `node` の `answer` の先頭の選択肢に `label` を足した IR。
fn ch1_with_label(node: usize) -> Value {
    let mut v: Value = serde_json::from_slice(&golden_in("ir_format_1", "ch1-pack")).unwrap();
    v["nodes"][node]["step"]["confirm"]["options"][0]["label"] = json!(SENTINEL_LABEL);
    v
}

fn assert_no_echo(s: &str) {
    assert!(!s.is_empty());
    assert!(
        !s.contains(SENTINEL_LABEL) && !s.contains(SENTINEL_KEY),
        "{s}"
    );
}

/// 対照: 版 1 の golden は模型が読める。模型が正しい理由で断っていることの前提になる。
/// 入力方式 `choice` の `options[].label`（v1 の語）は確認の `label` ではなく、断らない。
#[test]
fn ver13_old_engine_model_reads_v1_goldens() {
    for name in V1_GOLDENS {
        let g = golden_in("ir_format_1", name);
        let ir = old_engine::load(&g).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(ir.format_version, 1, "{name}");
    }
}

/// 場合 1: 旧い IR（木も確認の `label` も無い）を新しいエンジンが、変換せずそのまま読む。
#[test]
fn ver13_old_ir_answers_are_read_by_new_engine_without_label() {
    let g = golden_in("ir_format_1", "ch1-pack");
    let ir = load_ir(&g).unwrap();
    assert_eq!((ir.format_version, ir.pack.ir_format), (1, 1));
    let v: Value = serde_json::from_slice(&g).unwrap();
    let nodes = answer_nodes(&v);
    assert_eq!(nodes.len(), 2);
    for i in nodes {
        let spec = answer_spec_of(ir.nodes[i].step.confirm.as_ref().unwrap()).expect("readable");
        spec.check().unwrap();
        for o in &spec.options {
            assert!(o.label.is_none());
            assert_eq!(spec.display_name(&o.id), Some(o.id.as_str()));
        }
    }
}

/// 場合 2 の前提: 確認の `label` は v1.3 の加算で、`ir_format` は 1 のまま。新しいエンジンは読める（PACK-24）。
#[test]
fn ver13_label_stays_v1_and_new_engine_reads_it() {
    let base: Value = serde_json::from_slice(&golden_in("ir_format_1", "ch1-pack")).unwrap();
    for i in answer_nodes(&base) {
        let b = bytes(&ch1_with_label(i));
        let ir = load_ir(&b).unwrap();
        assert_eq!(ir.required_ir_format(), 1);
        assert_eq!((ir.format_version, ir.pack.ir_format), (1, 1));
        let spec = answer_spec_of(ir.nodes[i].step.confirm.as_ref().unwrap()).unwrap();
        assert_eq!(spec.options[0].label.as_deref(), Some(SENTINEL_LABEL));
    }
}

/// 場合 2: `label` を持つ IR を旧いエンジンは断る。版ではなく型で断り、`label` の値を理由に含めない。
#[test]
fn ver13_label_ir_is_refused_by_old_engine_without_echo() {
    let base: Value = serde_json::from_slice(&golden_in("ir_format_1", "ch1-pack")).unwrap();
    for i in answer_nodes(&base) {
        let Err(e) = old_engine::load(&bytes(&ch1_with_label(i))) else {
            panic!("node {i}: an old engine must refuse a confirm label")
        };
        assert!(matches!(e, OldEngineError::Type), "node {i}: {e}");
        assert_no_echo(&e.to_string());
    }
}

/// 場合 3a: 本文の木を使う教材は `ir_format` 2 になり（D-51・D-52）、新しいエンジンは読み、
/// 旧いエンジンは版の理由で断る（型の不一致にしない）。
#[test]
fn ver13_tree_ir_requires_v2_and_old_engine_refuses_by_version() {
    let mut inputs: Vec<(String, Vec<u8>)> = V2_GOLDENS
        .iter()
        .map(|n| (format!("golden/{n}"), golden_in("golden", n)))
        .collect();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/document-body-pack");
    inputs.push((
        "compiled/document-body-pack".into(),
        compile_dir(&dir).unwrap(),
    ));
    for (name, b) in inputs {
        let ir = load_ir(&b).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(ir.required_ir_format(), 2, "{name}");
        assert_eq!((ir.format_version, ir.pack.ir_format), (2, 2), "{name}");
        let Err(e) = old_engine::load(&b) else {
            panic!("{name}: an old engine must refuse a v2 IR")
        };
        assert!(
            matches!(
                e,
                OldEngineError::Version {
                    format_version: 2,
                    pack_ir_format: 2
                }
            ),
            "{name}: {e}"
        );
    }
}

/// 場合 3b: 木を使わない教材は最小の `ir_format`（1）のままで、旧いエンジンが読める（D-52）。
#[test]
fn ver13_pack_without_trees_stays_v1_and_old_engine_reads_it() {
    let t = Tmp::new("ver13-v1");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    let chapter = concat!(
        "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n",
        "  - {id: a, title: A, next: [{to: b}]}\n",
        "  - {id: b, title: B, final: true}\n",
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    let b = compile_dir(&t.0).unwrap();
    let ir = old_engine::load(&b).unwrap();
    assert_eq!((ir.format_version, ir.pack.ir_format), (1, 1));
    assert_eq!(to_canonical_json(&ir).unwrap(), b);
}

/// 決定の仕組み: 今後の欄は `ir_format` を上げる。`Value` で通す位置（`confirm`）の欄は型では断れないが、
/// 版が上がっていれば版の先読みが先に断る。
#[test]
fn ver13_future_field_inside_confirm_is_refused_by_version_gate() {
    let base: Value = serde_json::from_slice(&golden_in("ir_format_1", "ch1-pack")).unwrap();
    for i in answer_nodes(&base) {
        let mut v = base.clone();
        v["nodes"][i]["step"]["confirm"][SENTINEL_KEY] = json!(SENTINEL_LABEL);
        v["nodes"][i]["step"]["confirm"]["options"][0][SENTINEL_KEY] = json!(SENTINEL_LABEL);
        let mut a = v.clone();
        a["format_version"] = json!(3);
        let e = load_ir(&bytes(&a)).unwrap_err();
        assert_eq!(e, IrLoadError::FormatVersion(3));
        assert_no_echo(&e.to_string());
        let mut b = v;
        b["pack"]["ir_format"] = json!(3);
        let e = load_ir(&bytes(&b)).unwrap_err();
        assert_eq!(e, IrLoadError::PackIrFormat(3));
        assert_no_echo(&e.to_string());
    }
}

/// 限界の記録: 版 1 のまま `answer` の選択肢に未知の欄を足すと、`load_ir` は読み込み時に断らない（`Value` で通る）。
/// 型で読めないため確認の宣言は読めず、判定時に不正な宣言として扱われる。これは現状の記録であり、
/// 望ましい挙動の主張ではない。D-51 により今後の欄は版を上げるので、実運用では版の先読みが先に断る。
#[test]
fn ver13_future_field_in_answer_option_is_refused_at_judging_not_loading() {
    let base: Value = serde_json::from_slice(&golden_in("ir_format_1", "ch1-pack")).unwrap();
    for i in answer_nodes(&base) {
        let mut v = base.clone();
        v["nodes"][i]["step"]["confirm"]["options"][0][SENTINEL_KEY] = json!(SENTINEL_LABEL);
        let ir = load_ir(&bytes(&v)).expect("read at load time");
        assert!(answer_spec_of(ir.nodes[i].step.confirm.as_ref().unwrap()).is_none());
    }
}
