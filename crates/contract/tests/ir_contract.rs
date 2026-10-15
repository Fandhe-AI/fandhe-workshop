//! IR の型（`ir_format` 1・2）の適合試験（TASK-41・TASK-60、04-behavior の PACK-24）。
//!
//! 版の値、バイト列の形、任意の欄の既定値は書き出さない規則、版 1 の構造の欄は空でも書き出す規則、
//! 未知の欄の拒否を、golden（`tests/data/ir_format_1/`）とのバイト一致で固定する。
//! golden の書き換えは契約の変更であり、`ir_format` の版上げの判断が要る（spec を先に直す）。
//! 試験は作業ツリーを書き換えない。golden のパスは `CARGO_MANIFEST_DIR` から固定で組み立てる。
//! 版 2 は `tests/data/ir_format_2/` の golden で、木の欄を使うときだけ版が 2 になることを固定する。
//! 新 → 旧の拒否と旧 → 新の読み込みの適合試験は `tests/ir_compat.rs`（型の水準）と
//! `crates/pack/tests/ir_compat.rs`（`load_ir` の水準）が扱う。
//! 試験では `unwrap` を使ってよい（失敗はそのまま試験の失敗になる）。

use std::collections::BTreeMap;
use std::path::Path;

use fandhe_workshop_contract::doc::{Block, Doc, Inline, TREE_VERSION};
use fandhe_workshop_contract::ir::*;
use serde_json::{json, Value};

fn golden(name: &str) -> Vec<u8> {
    golden_in("ir_format_1", name)
}

fn golden_in(dir: &str, name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(dir)
        .join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("cannot read golden {name}"))
}

fn pack() -> PackInfo {
    PackInfo {
        id: "p".into(),
        title: "題名".into(),
        version: "0.1.0".into(),
        ir_format: IR_FORMAT_V1,
        guidance: "A".into(),
        vocab: None,
        guidance_levels: None,
        profile: None,
        appearance: None,
        left_pane: None,
    }
}

fn step(id: &str) -> Step {
    serde_json::from_value(json!({"id": id, "title": "t"})).unwrap()
}

fn minimal_ir() -> Ir {
    Ir {
        format_version: IR_FORMAT_V1,
        pack: pack(),
        refs: vec![],
        commands: BTreeMap::new(),
        views: BTreeMap::new(),
        fixtures: BTreeMap::new(),
        hints: BTreeMap::new(),
        rejections: BTreeMap::new(),
        clarifications: BTreeMap::new(),
        defaults: BTreeMap::new(),
        instances: vec![],
        nodes: vec![],
        starts: vec![],
        fixture_docs: BTreeMap::new(),
    }
}

fn text(t: &str) -> Inline {
    Inline::Text { text: t.into() }
}

/// 版 2 の全種別の木。
fn sample_doc() -> Doc {
    Doc {
        version: TREE_VERSION,
        blocks: vec![
            Block::Heading {
                level: 1,
                children: vec![text("見出し")],
            },
            Block::Paragraph {
                children: vec![
                    text("本文"),
                    Inline::Code { text: "c".into() },
                    Inline::Emph {
                        children: vec![text("e")],
                    },
                    Inline::Strong {
                        children: vec![text("s")],
                    },
                    Inline::Link {
                        href: "https://example.com/".into(),
                        children: vec![text("l")],
                    },
                    Inline::Image {
                        src: "a.png".into(),
                        alt: "a".into(),
                    },
                    Inline::Break,
                ],
            },
            Block::Quote {
                children: vec![Block::Rule],
            },
            Block::List {
                ordered: true,
                items: vec![vec![Block::Paragraph {
                    children: vec![text("i")],
                }]],
            },
            Block::Code {
                lang: Some("rust".into()),
                text: "fn main() {}".into(),
            },
            Block::Code {
                lang: None,
                text: "plain".into(),
            },
            Block::Table {
                head: vec![vec![text("h")]],
                rows: vec![vec![vec![text("r")]]],
            },
            Block::Rule,
        ],
    }
}

/// 版 1 の `full_ir()` に、版 2 の欄をすべて足したもの。
fn full_ir_v2() -> Ir {
    let mut ir = full_ir();
    ir.format_version = IR_FORMAT_V2;
    ir.pack.ir_format = IR_FORMAT_V2;
    ir.nodes[0].body_doc = Some(sample_doc());
    ir.nodes[0].body_levels_doc.insert("B".into(), sample_doc());
    ir.fixture_docs.insert("f".into(), sample_doc());
    ir
}

fn full_ir() -> Ir {
    let mut s = step("a");
    s.body = Some("a.md".into());
    s.body_levels.insert("B".into(), "b.md".into());
    s.do_ = Some("やること".into());
    s.lo = vec!["lo-1".into()];
    s.is_final = true;
    s.views = vec!["v".into()];
    s.fixtures = vec!["f".into()];
    s.hints = vec!["h".into()];
    s.on_enter = vec![json!({"op": "x"})];
    s.processing = vec![json!({"op": "y"})];
    s.input = Some(json!({"kind": "text"}));
    s.rules = vec![json!({"when": "z"})];
    s.on_reject.insert("busy".into(), "rj".into());
    s.confirm = Some(json!({"kind": "yes_no"}));
    s.on_achieved = vec![json!({"op": "w"})];
    s.next = vec![Transition {
        to: "main/b".into(),
        when: Some(json!({"all": []})),
    }];
    s.use_module = Some(json!({"module": "m"}));

    let mut p = pack();
    p.vocab = Some(json!({"v": 1}));
    p.guidance_levels = Some(json!({"A": {}}));
    p.profile = Some(json!({"ime": true}));
    p.appearance = Some(json!({"theme": "dark"}));
    p.left_pane = Some(json!({"tabs": []}));

    let mut ir = minimal_ir();
    ir.pack = p;
    ir.refs = vec![json!({"name": "r"})];
    ir.commands.insert("help".into(), json!("builtin"));
    ir.views.insert("v".into(), json!({"kind": "file_tree"}));
    ir.fixtures.insert(
        "f".into(),
        FixtureDef {
            path: "fixtures/f.txt".into(),
            kind: "text".into(),
        },
    );
    ir.hints.insert("h".into(), json!({"level": 1}));
    ir.rejections.insert("rj".into(), json!({"message": "m"}));
    ir.clarifications.insert("c".into(), json!({"text": "t"}));
    ir.defaults.insert("busy".into(), "rj".into());
    ir.instances = vec![Instance {
        id: "main".into(),
        module: Some("m".into()),
        args: BTreeMap::from([("k".to_string(), "v".to_string())]),
        caller: Some("main/a".into()),
    }];
    ir.nodes = vec![Node {
        uid: "main/a".into(),
        scope: "main".into(),
        unit: "c1".into(),
        step: s,
        enter: Some("m/x".into()),
        exit_to: Some("main/b".into()),
        body_doc: None,
        body_levels_doc: BTreeMap::new(),
    }];
    ir.starts = vec![Start {
        id: "s1".into(),
        node: "main/a".into(),
        effects: vec![json!({"op": "e"})],
        guidance: Some(json!("B")),
        requires: Some(json!({"k": 1})),
        unmet: Some(json!("rj")),
    }];
    ir
}

#[test]
fn format_versions_are_one_and_two() {
    assert_eq!(IR_FORMAT_V1, 1);
    assert_eq!(IR_FORMAT_V2, 2);
    assert_eq!(SUPPORTED_IR_FORMATS, [1, 2]);
    assert!(is_supported_ir_format(1) && is_supported_ir_format(2));
    assert!(!is_supported_ir_format(0) && !is_supported_ir_format(3));
}

#[test]
fn required_ir_format_is_the_smallest_that_fits() {
    assert_eq!(minimal_ir().required_ir_format(), IR_FORMAT_V1);
    assert_eq!(full_ir().required_ir_format(), IR_FORMAT_V1);
    assert_eq!(full_ir_v2().required_ir_format(), IR_FORMAT_V2);
    let mut only_fixture = minimal_ir();
    only_fixture.fixture_docs.insert("f".into(), sample_doc());
    assert_eq!(only_fixture.required_ir_format(), IR_FORMAT_V2);
    let mut only_levels = full_ir();
    only_levels.nodes[0]
        .body_levels_doc
        .insert("B".into(), sample_doc());
    assert_eq!(only_levels.required_ir_format(), IR_FORMAT_V2);
}

#[test]
fn v2_golden_bytes_match_and_roundtrip() {
    let ir = full_ir_v2();
    let want = golden_in("ir_format_2", "full.json");
    assert!(to_canonical_json(&ir).unwrap() == want, "bytes differ");
    assert!(want.ends_with(b"}\n") && !want.ends_with(b"\n\n"));
    assert!(!String::from_utf8(want.clone()).unwrap().contains("null"));
    let read: Ir = serde_json::from_slice(&want).unwrap();
    assert_eq!(read, ir);
}

#[test]
fn v1_shape_is_unchanged_when_v2_fields_are_empty() {
    // 版 2 の欄を除き、版を 1 に戻すと版 1 の golden と同じバイト列になる。
    let mut ir = full_ir_v2();
    ir.format_version = IR_FORMAT_V1;
    ir.pack.ir_format = IR_FORMAT_V1;
    ir.nodes[0].body_doc = None;
    ir.nodes[0].body_levels_doc.clear();
    ir.fixture_docs.clear();
    assert!(to_canonical_json(&ir).unwrap() == golden("full.json"));
}

#[test]
fn golden_bytes_match() {
    for (name, ir) in [("minimal.json", minimal_ir()), ("full.json", full_ir())] {
        let got = to_canonical_json(&ir).unwrap();
        let want = golden(name);
        assert!(got == want, "{name}: bytes differ from golden");
        assert!(want.ends_with(b"}\n") && !want.ends_with(b"\n\n"), "{name}");
    }
}

#[test]
fn golden_roundtrips() {
    for (name, ir) in [("minimal.json", minimal_ir()), ("full.json", full_ir())] {
        let want = golden(name);
        let read: Ir = serde_json::from_slice(&want).unwrap();
        assert_eq!(read, ir, "{name}");
        assert!(to_canonical_json(&read).unwrap() == want, "{name}");
    }
}

#[test]
fn always_emitted_fields_appear_when_empty() {
    let v: Value = serde_json::from_slice(&to_canonical_json(&minimal_ir()).unwrap()).unwrap();
    for k in [
        "refs",
        "commands",
        "views",
        "fixtures",
        "hints",
        "rejections",
        "clarifications",
        "defaults",
        "instances",
        "nodes",
        "starts",
    ] {
        assert!(v.get(k).is_some(), "missing top-level field {k}");
    }
    let start = Start {
        id: "s".into(),
        node: "n".into(),
        effects: vec![],
        guidance: None,
        requires: None,
        unmet: None,
    };
    let v = serde_json::to_value(&start).unwrap();
    assert_eq!(v, json!({"id": "s", "node": "n", "effects": []}));
    let inst = Instance {
        id: "i".into(),
        module: None,
        args: BTreeMap::new(),
        caller: None,
    };
    let v = serde_json::to_value(&inst).unwrap();
    assert_eq!(v, json!({"id": "i", "args": {}}));
}

/// 任意の欄を省略した JSON と、既定値を明示した JSON は、同じ値に読め、同じバイト列に書ける。
fn assert_default_equiv<T>(omitted: Value, explicit: Value)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let a: T = serde_json::from_value(omitted.clone()).unwrap();
    let b: T = serde_json::from_value(explicit).unwrap();
    assert_eq!(a, b);
    let (ja, jb) = (
        serde_json::to_vec_pretty(&a).unwrap(),
        serde_json::to_vec_pretty(&b).unwrap(),
    );
    assert!(ja == jb, "bytes differ for {}", std::any::type_name::<T>());
    // 省略した形に、既定値の欄が現れない。
    assert_eq!(serde_json::from_slice::<Value>(&ja).unwrap(), omitted);
}

#[test]
fn explicit_defaults_equal_omitted() {
    assert_default_equiv::<Step>(
        json!({"id": "a", "title": "t"}),
        json!({"id": "a", "title": "t", "body": null, "body_levels": {}, "do": null,
            "lo": [], "final": false, "views": [], "fixtures": [], "hints": [],
            "on_enter": [], "processing": [], "input": null, "rules": [], "on_reject": {},
            "confirm": null, "on_achieved": [], "next": [], "use": null}),
    );
    assert_default_equiv::<Transition>(json!({"to": "b"}), json!({"to": "b", "when": null}));
    assert_default_equiv::<PackInfo>(
        json!({"id": "p", "title": "t", "version": "0", "ir_format": 1, "guidance": "A"}),
        json!({"id": "p", "title": "t", "version": "0", "ir_format": 1, "guidance": "A",
            "vocab": null, "guidance_levels": null, "profile": null, "appearance": null,
            "left_pane": null}),
    );
    assert_default_equiv::<Node>(
        json!({"uid": "main/a", "scope": "main", "unit": "u", "step": {"id": "a", "title": "t"}}),
        json!({"uid": "main/a", "scope": "main", "unit": "u", "step": {"id": "a", "title": "t"},
            "enter": null, "exit_to": null, "body_doc": null, "body_levels_doc": {}}),
    );
    assert_default_equiv::<Start>(
        json!({"id": "s", "node": "n", "effects": []}),
        json!({"id": "s", "node": "n", "effects": [], "guidance": null, "requires": null,
            "unmet": null}),
    );
    assert_default_equiv::<Instance>(
        json!({"id": "i", "args": {}}),
        json!({"id": "i", "args": {}, "module": null, "caller": null}),
    );
}

#[test]
fn unknown_fields_are_rejected_at_every_level() {
    let cases: [(&str, Value); 6] = [
        (
            "Ir",
            json!({"format_version": 1, "pack": {"id": "p", "title": "t", "version": "0", "ir_format": 1, "guidance": "A"}, "nodes": [], "starts": [], "future": 1}),
        ),
        (
            "PackInfo",
            json!({"id": "p", "title": "t", "version": "0", "ir_format": 1, "guidance": "A", "future": 1}),
        ),
        ("Step", json!({"id": "a", "title": "t", "future": 1})),
        ("Transition", json!({"to": "b", "future": 1})),
        ("Start", json!({"id": "s", "node": "n", "future": 1})),
        (
            "Node",
            json!({"uid": "u", "scope": "s", "unit": "c", "step": {"id": "a", "title": "t"}, "future": 1}),
        ),
    ];
    for (name, v) in cases {
        let r = match name {
            "Ir" => serde_json::from_value::<Ir>(v).map(|_| ()),
            "PackInfo" => serde_json::from_value::<PackInfo>(v).map(|_| ()),
            "Step" => serde_json::from_value::<Step>(v).map(|_| ()),
            "Transition" => serde_json::from_value::<Transition>(v).map(|_| ()),
            "Start" => serde_json::from_value::<Start>(v).map(|_| ()),
            _ => serde_json::from_value::<Node>(v).map(|_| ()),
        };
        assert!(r.is_err(), "{name} must reject unknown fields");
    }
    let inst = json!({"id": "i", "future": 1});
    assert!(serde_json::from_value::<Instance>(inst).is_err());
    let fx = json!({"path": "p", "kind": "k", "future": 1});
    assert!(serde_json::from_value::<FixtureDef>(fx).is_err());
}

#[test]
fn v2_fields_reject_unknown_content() {
    let node = |extra: Value| {
        let mut v =
            json!({"uid": "u", "scope": "s", "unit": "c", "step": {"id": "a", "title": "t"}});
        v.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        serde_json::from_value::<Node>(v)
    };
    let ok = json!({"body_doc": {"version": 1, "blocks": [{"t": "rule"}]}});
    assert!(node(ok).is_ok());
    let bad_kind = json!({"body_doc": {"version": 1, "blocks": [{"t": "script", "text": "x"}]}});
    assert!(node(bad_kind).is_err());
    let bad_field = json!({"body_levels_doc": {"B": {"version": 1, "blocks": [], "x": 1}}});
    assert!(node(bad_field).is_err());
}
