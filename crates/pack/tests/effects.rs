//! 状態変化の語彙（要素・属性・フラグ）の「読み込み → IR → 適用」の結合試験（TASK-43 1/2、PACK-9）。
//!
//! 読み込み時の検査は pack、適用は core が持つ（置き場の理由は `docs/architecture.md` の第 6 節）。
//! 試験名の `pack9_` で 04 の ID を追える。

mod common;

use common::{assert_no_secret, Tmp, SECRET, WORKSHOP};

use fandhe_workshop_contract::effect::EffectItem;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Root, Stamp, StepUid};
use fandhe_workshop_core::effect::apply_effect;
use fandhe_workshop_core::event::Cause;
use fandhe_workshop_core::session::RunState;
use fandhe_workshop_core::world::Element;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const WS: &str = "fixtures:\n  f1: {path: fixtures/x.txt, kind: document}\n";

/// 章の 1 つ目の Step（`a`）の欄と開始点の `effects` を差し込んだパックを作る。
fn pack(step_extra: &str, start_effects: &str) -> Tmp {
    let t = Tmp::new("effects");
    t.put("workshop.yaml", format!("{WORKSHOP}{WS}").as_bytes());
    t.put("fixtures/x.txt", b"x");
    t.put(
        "chapters/c1/chapter.yaml",
        format!(
            "id: c1\ntitle: C\nstarts: [{{id: s1, node: a, effects: {start_effects}}}]\nsteps:\n  - {{id: a, title: A, next: [{{to: b}}], input: {{kind: choice, options: [{{id: ok, label: OK}}]}}, rules: [{{id: r-a, on: {{option: ok}}, outcome: {{kind: accept}}}}], confirm: {{kind: none, reason: r}}{step_extra}}}\n  - {{id: b, title: B, final: true}}\n"
        )
        .as_bytes(),
    );
    t
}

fn diags(t: &Tmp) -> Vec<Diagnostic> {
    match compile_dir(&t.0) {
        Ok(_) => panic!("expected diagnostics"),
        Err(d) => d,
    }
}

fn one(t: &Tmp, code: DiagCode) -> Diagnostic {
    let mut d = diags(t);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code(), code, "{d:?}");
    assert_no_secret(&d);
    d.remove(0)
}

const SEVEN: &str = concat!(
    ", on_enter: [{put: {path: panel, fixture: f1, attrs: {k: v}}}, {set_attr: {path: panel, attr: k, value: w}},",
    " {set_flag: f}, {clear_flag: f}, {set_flag_global: g}],",
    " on_achieved: [{replace: {path: panel, attrs: {z: y}}}, {remove: panel}]"
);

fn ir_of(t: &Tmp) -> Ir {
    let bytes = compile_dir(&t.0).expect("compile");
    serde_json::from_slice(&bytes).expect("ir")
}

#[test]
fn pack9_seven_words_compile_and_stay_values_in_the_ir() {
    let t = pack(SEVEN, "[{put: {path: seed}}]");
    let ir = ir_of(&t);
    let a = ir.nodes.iter().find(|n| n.uid == "main/a").expect("node a");
    assert_eq!(a.step.on_enter.len(), 5);
    assert_eq!(a.step.on_enter[0]["put"]["path"], "panel");
    assert_eq!(a.step.on_achieved.len(), 2);
    assert_eq!(a.step.on_achieved[1]["remove"], "panel");
    assert_eq!(ir.starts[0].effects.len(), 1);
}

#[test]
fn pack9_load_to_ir_to_apply() {
    let t = pack(SEVEN, "[{put: {path: seed, attrs: {s: t}}}]");
    let ir = ir_of(&t);
    let at = Stamp {
        generation: Generation::INITIAL,
        step: StepUid("main/a".into()),
    };
    let root = Root("main".into());
    let mut run = RunState::default();
    let apply_all = |run: &mut RunState, list: &[serde_json::Value]| {
        for v in list {
            let item = EffectItem::from_value(v).expect("parse");
            apply_effect(run, &root, &item.effect, &at, Cause::Start);
        }
    };
    apply_all(&mut run, &ir.starts[0].effects);
    assert_eq!(run.world["seed"].attrs["s"], "t");
    let a = ir.nodes.iter().find(|n| n.uid == "main/a").expect("node a");
    apply_all(&mut run, &a.step.on_enter);
    assert_eq!(
        run.world["panel"],
        Element {
            fixture: Some("f1".into()),
            attrs: [("k".to_string(), "w".to_string())].into()
        }
    );
    assert!(run.slices[&root].flags.is_empty());
    assert!(run.global_flags.contains("g"));
    apply_all(&mut run, &a.step.on_achieved);
    assert!(!run.world.contains_key("panel"));
    assert!(run.world.contains_key("seed"));
}

#[test]
fn pack9_unknown_field_is_e_parse() {
    let t = pack(
        &format!(", on_enter: [{{put: {{path: a}}, script: {SECRET}}}]"),
        "[]",
    );
    let d = one(&t, DiagCode::Parse);
    assert!(d.message().contains("unknown field `script`"), "{d}");
    assert!(d.message().starts_with("on_enter[0]"), "{d}");
    assert_eq!(d.item(), "step:a");
}

#[test]
fn pack9_word_count_is_e_structure() {
    for extra in [
        ", on_enter: [{}]",
        ", on_enter: [{put: {path: a}, remove: a}]",
    ] {
        one(&pack(extra, "[]"), DiagCode::Structure);
    }
}

#[test]
fn pack9_undeclared_fixture_is_e_ref_fixture() {
    let t = pack(", on_enter: [{put: {path: a, fixture: nope}}]", "[]");
    one(&t, DiagCode::RefFixture);
    let t = pack("", "[{replace: {path: a, fixture: nope}}]");
    let d = one(&t, DiagCode::RefFixture);
    assert_eq!(d.item(), "start:s1");
}

#[test]
fn pack9_empty_names_are_e_structure() {
    one(
        &pack(", on_enter: [{put: {path: ''}}]", "[]"),
        DiagCode::Structure,
    );
    one(&pack("", "[{set_flag: ''}]"), DiagCode::Structure);
}

#[test]
fn pack9_unquoted_number_is_e_parse_without_echoing_values() {
    let t = pack(
        ", on_enter: [{set_attr: {path: a, attr: k, value: 1}}]",
        "[]",
    );
    let d = one(&t, DiagCode::Parse);
    assert!(!d.message().contains("value: 1"), "{d}");
}

#[test]
fn pack9_format_a_accepts_and_rejects() {
    let toml = |effects: &str| {
        let t = Tmp::new("effects-toml");
        t.put(
            "workshop.toml",
            b"[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n",
        );
        t.put(
            "chapters/c1/chapter.toml",
            format!(
                "id = \"c1\"\ntitle = \"C\"\n[[starts]]\nid = \"s1\"\nnode = \"a\"\neffects = {effects}\n[[steps]]\nid = \"a\"\ntitle = \"A\"\nfinal = true\n"
            )
            .as_bytes(),
        );
        t
    };
    assert!(compile_dir(&toml("[{set_flag = \"f\"}]").0).is_ok());
    let t = toml(&format!("[{{set_flag = \"f\", script = \"{SECRET}\"}}]"));
    one(&t, DiagCode::Parse);
}

// ---- 進行（processing）と when（TASK-43 2/2） ----

use fandhe_workshop_contract::effect::ProcessingTick;
use fandhe_workshop_core::effect::{apply_tick, ConditionEval};

/// 9 語（7 語と `processing`・`when`）をすべて使う Step の欄。
const NINE: &str = concat!(
    ", on_enter: [{put: {path: panel, fixture: f1, attrs: {k: v}}}, {set_attr: {path: panel, attr: k, value: w}},",
    " {set_flag: f}, {clear_flag: f}, {set_flag_global: g}],",
    " on_achieved: [{replace: {path: panel, attrs: {z: y}}}, {remove: panel}],",
    " processing: [{name: scan, effects: [{set_flag: ready}, {put: {path: out, attrs: {s: done}}, when: {flag: ready}}]},",
    " {name: idle}]"
);

/// 試験専用の評価器。`{flag: 名前}` を `main` の切り身で見るだけ。
struct FlagStub;
impl ConditionEval for FlagStub {
    fn eval(&self, when: &serde_json::Value, run: &RunState, root: &Root) -> Option<bool> {
        let name = when.get("flag")?.as_str()?;
        Some(run.slices.get(root).is_some_and(|s| s.flags.contains(name)))
    }
}

#[test]
fn pack9_nine_words_compile_and_apply_through_ir() {
    let t = pack(NINE, "[]");
    let ir = ir_of(&t);
    let a = ir.nodes.iter().find(|n| n.uid == "main/a").expect("node a");
    assert_eq!(a.step.processing.len(), 2);
    assert_eq!(a.step.processing[0]["name"], "scan");
    assert_eq!(a.step.processing[0]["effects"][1]["when"]["flag"], "ready");
    let at = Stamp {
        generation: Generation::INITIAL,
        step: StepUid("main/a".into()),
    };
    let root = Root("main".into());
    let mut run = RunState::default();
    let total = a.step.processing.len() as u32;
    for (i, v) in a.step.processing.iter().enumerate() {
        let tick = ProcessingTick::from_value(v).expect("parse");
        let ev = apply_tick(
            &mut run,
            &root,
            &tick,
            i as u32,
            total,
            &at,
            Cause::Input,
            &FlagStub,
        );
        assert!(
            matches!(&ev[0], fandhe_workshop_core::event::Event::Progress { index, .. } if *index == i as u32)
        );
    }
    assert!(run.slices[&root].flags.contains("ready"));
    assert_eq!(run.world["out"].attrs["s"], "done");
}

#[test]
fn pack9_processing_diagnostics() {
    let d = one(
        &pack(", processing: [{name: n, script: x}]", "[]"),
        DiagCode::Parse,
    );
    assert!(d.message().starts_with("processing[0]"), "{d}");
    assert!(d.message().contains("unknown field `script`"), "{d}");
    assert_eq!(d.item(), "step:a");

    let d = one(
        &pack(
            &format!(", processing: [{{name: n, effects: [{{set_flag: f}}, {{put: {{path: a}}, script: {SECRET}}}]}}]"),
            "[]",
        ),
        DiagCode::Parse,
    );
    assert!(d.message().starts_with("processing[0].effects[1]"), "{d}");

    one(
        &pack(", processing: [{name: ''}]", "[]"),
        DiagCode::Structure,
    );
    one(
        &pack(
            ", processing: [{name: n, effects: [{put: {path: a, fixture: nope}}]}]",
            "[]",
        ),
        DiagCode::RefFixture,
    );
    for w in ["1", "x", "[a]"] {
        one(
            &pack(
                &format!(", processing: [{{name: n, effects: [{{set_flag: f, when: {w}}}]}}]"),
                "[]",
            ),
            DiagCode::Parse,
        );
    }
    one(&pack(", processing: [1]", "[]"), DiagCode::Parse);
}

#[test]
fn pack9_format_a_processing_accepts_and_rejects() {
    let toml = |tick: &str| {
        let t = Tmp::new("processing-toml");
        t.put(
            "workshop.toml",
            b"[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n",
        );
        t.put(
            "chapters/c1/chapter.toml",
            format!(
                "id = \"c1\"\ntitle = \"C\"\n[[starts]]\nid = \"s1\"\nnode = \"a\"\n[[steps]]\nid = \"a\"\ntitle = \"A\"\nfinal = true\nprocessing = [{tick}]\n"
            )
            .as_bytes(),
        );
        t
    };
    assert!(compile_dir(
        &toml("{name = \"n\", effects = [{set_flag = \"f\", when = {flag = \"f\"}}]}").0
    )
    .is_ok());
    one(
        &toml(&format!("{{name = \"n\", script = \"{SECRET}\"}}")),
        DiagCode::Parse,
    );
}

// ---- 進行の間隔は宣言できない（PACK-44、D-43） ----
// 間隔はエンジンのビルド時の定数で（D-47）、教材は変えられない。欄を足さない決定を、未知の欄の拒否として固定する。
// 回避は進行の件数を増やすことである（docs/pack-authoring.md の「処理中の進行」）。

#[test]
fn pack44_interval_on_tick_is_e_parse() {
    for key in ["interval", "interval_ms", "tick_ms"] {
        let d = one(
            &pack(
                &format!(", processing: [{{name: n, {key}: {SECRET}}}]"),
                "[]",
            ),
            DiagCode::Parse,
        );
        assert!(d.message().starts_with("processing[0]"), "{d}");
        assert!(
            d.message().contains(&format!("unknown field `{key}`")),
            "{d}"
        );
    }
}

#[test]
fn pack44_interval_on_step_is_e_parse() {
    let d = one(
        &pack(
            &format!(", interval: {SECRET}, processing: [{{name: n}}]"),
            "[]",
        ),
        DiagCode::Parse,
    );
    assert!(d.message().contains("unknown field `interval`"), "{d}");
}

#[test]
fn pack44_more_ticks_is_the_workaround() {
    let ticks: Vec<String> = (0..20).map(|i| format!("{{name: t{i}}}")).collect();
    let t = pack(&format!(", processing: [{}]", ticks.join(", ")), "[]");
    let ir = ir_of(&t);
    let a = ir.nodes.iter().find(|n| n.uid == "main/a").expect("node a");
    assert_eq!(a.step.processing.len(), 20);
    for (i, v) in a.step.processing.iter().enumerate() {
        assert_eq!(v["name"], format!("t{i}"));
        let obj = v.as_object().expect("object");
        assert!(obj.keys().all(|k| k == "name" || k == "effects"), "{v}");
    }
}

#[test]
fn pack44_format_a_interval_is_e_parse() {
    let t = Tmp::new("interval-toml");
    t.put(
        "workshop.toml",
        b"[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n",
    );
    t.put(
        "chapters/c1/chapter.toml",
        format!(
            "id = \"c1\"\ntitle = \"C\"\n[[starts]]\nid = \"s1\"\nnode = \"a\"\n[[steps]]\nid = \"a\"\ntitle = \"A\"\nfinal = true\nprocessing = [{{name = \"n\", interval = \"{SECRET}\"}}]\n"
        )
        .as_bytes(),
    );
    one(&t, DiagCode::Parse);
}

/// コードを書ける欄を作らない: 効果・進行・when の全位置に差し込んだコード欄はすべて拒否される。
#[test]
fn pack9_no_code_field_is_accepted_anywhere() {
    for key in ["script", "code", "eval", "exec", "expr", "js"] {
        let val = format!("{key}: {SECRET}");
        let cases = [
            (
                format!(", on_enter: [{{set_flag: f, {val}}}]"),
                "[]".to_string(),
            ),
            (
                format!(", on_achieved: [{{set_flag: f, {val}}}]"),
                "[]".to_string(),
            ),
            ("".to_string(), format!("[{{set_flag: f, {val}}}]")),
            (
                format!(", processing: [{{name: n, {val}}}]"),
                "[]".to_string(),
            ),
            (
                format!(", processing: [{{name: n, effects: [{{set_flag: f, {val}}}]}}]"),
                "[]".to_string(),
            ),
        ];
        for (extra, starts) in cases {
            let d = diags(&pack(&extra, &starts));
            assert!(
                d.iter().all(|x| x.code() == DiagCode::Parse) && !d.is_empty(),
                "{key}: {d:?}"
            );
            assert_no_secret(&d);
        }
    }
}

/// 9 語を使ったパックの IR に、コードを意味する鍵が 0 件であること。
#[test]
fn pack9_compiled_ir_has_no_code_keys() {
    fn has_code(v: &serde_json::Value) -> bool {
        const KEYS: [&str; 6] = ["script", "code", "eval", "exec", "expr", "js"];
        match v {
            serde_json::Value::Object(m) => m
                .iter()
                .any(|(k, v)| KEYS.contains(&k.as_str()) || has_code(v)),
            serde_json::Value::Array(a) => a.iter().any(has_code),
            _ => false,
        }
    }
    let t = pack(NINE, "[{put: {path: seed}}]");
    let bytes = compile_dir(&t.0).expect("compile");
    let v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert!(!has_code(&v));
}

// ---- 教材内に動きの設定は置かない（A11Y-12、D-82） ----
// 動きの抑制は OS の `prefers-reduced-motion` を読み込み時に 1 回だけ読む方式で、教材の側に口は無い。欄を足さない決定を、
// 未知の欄の拒否として固定する。`reduce` で処理中の停止（LO-3）を体験できない分は、教材の文面で補う（INTRO-20・TASK-607。
// docs/pack-authoring.md の「処理中の進行」）。

const MOTION_KEYS: [&str; 5] = [
    "motion",
    "reduced_motion",
    "prefers_reduced_motion",
    "animation",
    "progress_display",
];

fn assert_unknown_field(t: &Tmp, key: &str, place: &str) {
    let d = one(t, DiagCode::Parse);
    assert!(
        d.message().contains(&format!("unknown field `{key}`")),
        "{place}: {d}"
    );
}

#[test]
fn a11y12_motion_keys_are_e_parse_everywhere() {
    for key in MOTION_KEYS {
        // workshop.yaml の最上位
        let t = pack("", "[]");
        t.put(
            "workshop.yaml",
            format!("{WORKSHOP}{WS}{key}: {SECRET}\n").as_bytes(),
        );
        assert_unknown_field(&t, key, "top");
        // Step の直下
        assert_unknown_field(&pack(&format!(", {key}: {SECRET}"), "[]"), key, "step");
        // processing[] の要素
        assert_unknown_field(
            &pack(
                &format!(", processing: [{{name: n, {key}: {SECRET}}}]"),
                "[]",
            ),
            key,
            "processing",
        );
        // pack の直下・pack.profile・pack.appearance
        for place in ["pack", "profile", "appearance"] {
            let t = pack("", "[]");
            let head = "id: p, title: t, version: '0', ir_format: 1, guidance: A, vocab: '1.5'";
            let w = if place == "pack" {
                format!("pack: {{{head}, {key}: {SECRET}}}\n{WS}")
            } else {
                format!("pack: {{{head}, {place}: {{{key}: {SECRET}}}}}\n{WS}")
            };
            t.put("workshop.yaml", w.as_bytes());
            let d = diags(&t);
            assert!(!d.is_empty(), "{place}/{key}: 拒否されない");
            assert_no_secret(&d);
        }
    }
}

#[test]
fn a11y12_theme_value_for_motion_is_refused() {
    for v in ["reduced_motion", "reduce", "no_motion"] {
        let t = pack("", "[]");
        t.put(
            "workshop.yaml",
            format!("pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: A, vocab: '1.5', appearance: {{theme: {v}}}}}\n{WS}").as_bytes(),
        );
        let d = diags(&t);
        assert!(!d.is_empty(), "{v}: 拒否されない");
        assert_no_secret(&d);
    }
}

#[test]
fn a11y12_format_a_motion_is_e_parse() {
    for key in MOTION_KEYS {
        let pack_toml = |extra: &str| {
            let t = Tmp::new("motion-toml");
            t.put(
                "workshop.toml",
                format!("[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n{extra}\n").as_bytes(),
            );
            t
        };
        let chapter = |step_extra: &str| {
            format!("id = \"c1\"\ntitle = \"C\"\n[[starts]]\nid = \"s1\"\nnode = \"a\"\n[[steps]]\nid = \"a\"\ntitle = \"A\"\nfinal = true\n{step_extra}\n")
        };
        let t = pack_toml(&format!("{key} = \"{SECRET}\""));
        t.put("chapters/c1/chapter.toml", chapter("").as_bytes());
        assert_unknown_field(&t, key, "toml pack");
        let t = pack_toml("");
        t.put(
            "chapters/c1/chapter.toml",
            chapter(&format!("{key} = \"{SECRET}\"")).as_bytes(),
        );
        assert_unknown_field(&t, key, "toml step");
    }
}
