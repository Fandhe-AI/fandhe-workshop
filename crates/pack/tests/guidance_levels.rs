//! 案内レベル（EP-8。PACK-17）の語の読み込みと、本文・ヒントの出し分けの試験（TASK-49 1/2）。
//!
//! 水準 L で出す本文は C → B → A の順に探す（`guidance::body_for`）。ヒントは契約の `visible_hints` が
//! 選ぶ。`A`・`B`・`C` でない水準の名前は読み込みで `E-PARSE` にし、IR に載せない。
//! TASK-49 2/2 の診断（`body_levels` の鍵 `A` の `E-STRUCTURE`、隠す語の漏れの `E-GUIDANCE-LEAK`、ヒントが出ない水準の
//! `E-HINT-HIDDEN`）の試験も置く。漏れとヒントは `validate` だけの検査で、`compile` は通す。

mod common;

use common::{assert_no_secret, Tmp, SECRET};

use fandhe_workshop_contract::guidance::{visible_hints, GuidanceLevel};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::guidance::body_for;
use fandhe_workshop_pack::source::load;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const WORKSHOP_HEAD: &str = "pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}\n";

fn pack(workshop: &str, chapter: &str) -> Tmp {
    let t = Tmp::new("guidance");
    t.put("workshop.yaml", workshop.as_bytes());
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    for f in ["a.md", "a.b.md", "a.c.md", "x.md"] {
        t.put(&format!("chapters/c1/{f}"), format!("# {f}\n").as_bytes());
    }
    t
}

fn chapter_with(start: &str, step_extra: &str) -> String {
    format!(
        "id: c1\ntitle: C\nstarts: [{start}]\nsteps:\n  - {{id: a, title: A, body: a.md{step_extra}, next: [{{to: b}}]}}\n  - {{id: b, title: B, final: true}}\n"
    )
}

fn ir_of(t: &Tmp) -> Ir {
    let bytes = compile_dir(&t.0).expect("compile");
    serde_json::from_slice(&bytes).expect("ir")
}

fn parse_err(t: &Tmp) -> Vec<Diagnostic> {
    let d = compile_dir(&t.0).expect_err("must reject");
    assert_no_secret(&d);
    assert!(d.iter().all(|x| x.code() == DiagCode::Parse), "{d:?}");
    // validate・load も同じ診断を返す。
    let v = validate_dir(&t.0);
    assert!(v.iter().any(|x| x.code() == DiagCode::Parse), "{v:?}");
    assert!(load(&t.0).is_err());
    d
}

fn one(t: &Tmp, file: &str, item: &str) {
    let d = parse_err(t);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].file(), file, "{d:?}");
    assert_eq!(d[0].item(), item, "{d:?}");
}

fn expected(step_has: &[GuidanceLevel], lvl: GuidanceLevel) -> &'static str {
    for l in lvl.fallback_chain() {
        if *l == GuidanceLevel::A {
            return "a.md";
        }
        if step_has.contains(l) {
            return if *l == GuidanceLevel::B {
                "a.b.md"
            } else {
                "a.c.md"
            };
        }
    }
    "a.md"
}

#[test]
fn body_is_chosen_c_then_b_then_a() {
    use GuidanceLevel::{B, C};
    for (extra, has) in [
        ("", vec![]),
        (", body_levels: {B: a.b.md}", vec![B]),
        (", body_levels: {C: a.c.md}", vec![C]),
        (", body_levels: {B: a.b.md, C: a.c.md}", vec![B, C]),
    ] {
        let t = pack(WORKSHOP_HEAD, &chapter_with("{id: s1, node: a}", extra));
        let ir = ir_of(&t);
        let node = ir.nodes.iter().find(|n| n.step.id == "a").expect("node a");
        for lvl in GuidanceLevel::ALL {
            assert_eq!(
                body_for(&node.step, lvl),
                Some(expected(&has, lvl)),
                "{extra} {lvl}"
            );
        }
    }
}

#[test]
fn hints_are_filtered_by_level_in_declared_order() {
    let ws = format!(
        "{WORKSHOP_HEAD}hints:\n  h1: {{text: t1}}\n  h2: {{text: t2, levels: []}}\n  h3: {{text: t3, levels: [A]}}\n  h4: {{text: t4, levels: [B, C]}}\n"
    );
    let t = pack(
        &ws,
        &chapter_with("{id: s1, node: a}", ", hints: [h4, h3, h2, h1]"),
    );
    let ir = ir_of(&t);
    let node = ir.nodes.iter().find(|n| n.step.id == "a").expect("node a");
    let ids = |l| visible_hints(&ir, &node.step, l);
    assert_eq!(ids(GuidanceLevel::A), ["h3", "h2", "h1"]);
    assert_eq!(ids(GuidanceLevel::B), ["h4", "h2", "h1"]);
    assert_eq!(ids(GuidanceLevel::C), ["h4", "h2", "h1"]);
}

#[test]
fn module_instances_choose_the_same_body() {
    let ws = WORKSHOP_HEAD;
    let module = "id: m\ntitle: M\nparams: {n: x}\nsteps:\n  - {id: ms, title: MS, body: a.md, body_levels: {C: a.c.md}, final: true}\n";
    let chapter = "id: c1\ntitle: C\nstarts: [{id: s1, node: u1}]\nsteps:\n  - {id: u1, title: U1, use: {module: m, instance: i1, args: {n: one}}, next: [{to: u2}]}\n  - {id: u2, title: U2, use: {module: m, instance: i2, args: {n: two}}, final: true}\n";
    let t = pack(ws, chapter);
    t.put("modules/m/module.yaml", module.as_bytes());
    t.put("modules/m/a.md", b"# a\n");
    t.put("modules/m/a.c.md", b"# c\n");
    let ir = ir_of(&t);
    let ms: Vec<_> = ir.nodes.iter().filter(|n| n.step.id == "ms").collect();
    assert_eq!(ms.len(), 2);
    for n in ms {
        assert_eq!(body_for(&n.step, GuidanceLevel::C), Some("a.c.md"));
        assert_eq!(body_for(&n.step, GuidanceLevel::B), Some("a.md"));
    }
}

#[test]
fn every_valid_level_spelling_is_accepted() {
    for g in ["A", "B", "C"] {
        for start in ["", ", guidance: A", ", guidance: B", ", guidance: C"] {
            let ws = format!(
                "pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: {g}, guidance_levels: {{A: {{}}, B: {{hide_command_kinds: [skill, flow], hide_words: ['空白 あり']}}, C: {{hide_command_kinds: [builtin, unsupported]}}}}}}\n"
            );
            let t = pack(
                &ws,
                &chapter_with(&format!("{{id: s1, node: a{start}}}"), ""),
            );
            assert!(load(&t.0).is_ok(), "{g} {start}");
            assert!(validate_dir(&t.0).is_empty(), "{g} {start}");
            ir_of(&t);
        }
    }
}

/// 鍵 `A` は読み込みでは通り、`validate`・`compile` が `E-STRUCTURE` にする（PACK-17。TASK-49 2/2）。
#[test]
fn body_levels_key_a_is_a_structure_error() {
    let t = pack(
        WORKSHOP_HEAD,
        &chapter_with("{id: s1, node: a}", ", body_levels: {A: x.md}"),
    );
    assert!(load(&t.0).is_ok());
    let v = validate_dir(&t.0);
    let c = compile_dir(&t.0).expect_err("compile must reject");
    for d in [&v, &c] {
        assert_no_secret(d);
        let hit: Vec<_> = d
            .iter()
            .filter(|x| x.code() == DiagCode::Structure)
            .collect();
        assert_eq!(hit.len(), 1, "{d:?}");
        assert_eq!(hit[0].file(), "chapters/c1/chapter.yaml");
        assert_eq!(hit[0].item(), "step:a");
    }
}

const LEAK_WS: &str = "pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A, guidance_levels: {B: {hide_command_kinds: [skill], hide_words: ['秘匿語']}, C: {hide_words: ['cword']}}}\ncommands:\n  help: builtin\n  make-item: skill\n";

fn leaks(t: &Tmp) -> Vec<Diagnostic> {
    let d = validate_dir(&t.0);
    assert_no_secret(&d);
    d.into_iter()
        .filter(|x| x.code() == DiagCode::GuidanceLeak)
        .collect()
}

#[test]
fn guidance_leak_in_body_levels_body_is_reported() {
    // (i) body_levels を持つ Step の水準 B・C。
    for (word, body_file, extra) in [
        ("/make-item", "a.b.md", ", body_levels: {B: a.b.md}"),
        ("秘匿語", "a.b.md", ", body_levels: {B: a.b.md}"),
        ("cword", "a.c.md", ", body_levels: {C: a.c.md}"),
    ] {
        let t = pack(LEAK_WS, &chapter_with("{id: s1, node: a}", extra));
        t.put(
            &format!("chapters/c1/{body_file}"),
            format!("# x\n{word} を使う。\n").as_bytes(),
        );
        let d = leaks(&t);
        assert_eq!(d.len(), 1, "{word}: {d:?}");
        assert_eq!(d[0].file(), "chapters/c1/chapter.yaml");
        assert_eq!(d[0].item(), "step:a");
        // 検査は validate だけで、compile は通す。
        assert!(compile_dir(&t.0).is_ok());
    }
}

#[test]
fn guidance_leak_ignores_level_a_and_levels_without_rule() {
    // 水準 B は C の本文を飛ばして a.md を探すため、B で漏れる。水準 C は安全な a.c.md で、漏れない。
    let t = pack(
        LEAK_WS,
        &chapter_with("{id: s1, node: a}", ", body_levels: {C: a.c.md}"),
    );
    t.put("chapters/c1/a.md", "/make-item 秘匿語 cword\n".as_bytes());
    t.put("chapters/c1/a.c.md", "安全な本文\n".as_bytes());
    let d = leaks(&t);
    assert_eq!(d.len(), 1, "{d:?}");
    // 規則の無い水準（B の規則だけの教材の C）では出ない。
    let ws = "pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A, guidance_levels: {B: {hide_words: ['w']}}}\n";
    let t = pack(
        ws,
        &chapter_with("{id: s1, node: a}", ", body_levels: {C: a.c.md}"),
    );
    t.put("chapters/c1/a.c.md", "w\n".as_bytes());
    assert!(leaks(&t).is_empty());
}

#[test]
fn guidance_leak_follows_start_guidance_and_modules() {
    let module = "id: m\ntitle: M\nsteps:\n  - {id: ms, title: MS, body: a.md, final: true}\n";
    let chapter = |g: &str| {
        format!("id: c1\ntitle: C\nstarts: [{{id: s1, node: u1{g}}}]\nsteps:\n  - {{id: u1, title: U1, use: {{module: m, instance: i1}}, final: true}}\n")
    };
    for (g, expect) in [
        (", guidance: B", true),
        (", guidance: C", false), // C の語は cword だけ。本文は /make-item のみ
        (", guidance: A", false),
        ("", false),
    ] {
        let t = pack(LEAK_WS, &chapter(g));
        t.put("modules/m/module.yaml", module.as_bytes());
        t.put("modules/m/a.md", "/make-item\n".as_bytes());
        let d = leaks(&t);
        assert_eq!(
            d.iter().any(|x| x.item() == "step:ms"),
            expect,
            "{g}: {d:?}"
        );
        if expect {
            assert!(d.iter().any(|x| x.file() == "modules/m/module.yaml"));
        }
    }
}

#[test]
fn guidance_leak_in_do_is_reported_but_argument_do_is_not_seen() {
    let t = pack(
        LEAK_WS,
        &chapter_with(
            "{id: s1, node: a, guidance: B}",
            ", do: '/make-item を打つ'",
        ),
    );
    let d = leaks(&t);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].item(), "step:a");
    let t = pack(
        LEAK_WS,
        &chapter_with("{id: s1, node: a, guidance: B}", ", do: '${cmd}'"),
    );
    assert!(leaks(&t).is_empty());
}

#[test]
fn guidance_leak_skips_blank_hide_words_and_does_not_echo_values() {
    let ws = "pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A, guidance_levels: {B: {hide_words: ['', '  ']}}}\n";
    let t = pack(ws, &chapter_with("{id: s1, node: a, guidance: B}", ""));
    assert!(leaks(&t).is_empty());
    let ws = format!("pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: A, guidance_levels: {{B: {{hide_words: ['{SECRET}']}}}}}}\n");
    let t = pack(&ws, &chapter_with("{id: s1, node: a, guidance: B}", ""));
    t.put("chapters/c1/a.md", format!("{SECRET}\n").as_bytes());
    assert_eq!(leaks(&t).len(), 1);
}

fn hidden(t: &Tmp) -> Vec<Diagnostic> {
    let d = validate_dir(&t.0);
    assert_no_secret(&d);
    d.into_iter()
        .filter(|x| x.code() == DiagCode::HintHidden)
        .collect()
}

#[test]
fn hint_hidden_is_reported_per_level() {
    let ws = format!("{WORKSHOP_HEAD}hints:\n  hb: {{text: t, levels: [B, C]}}\n  ha: {{text: t, levels: [A]}}\n  hall: {{text: t}}\n");
    let hs = |ids: &str| chapter_with("{id: s1, node: a}", &format!(", hints: [{ids}]"));
    // B・C だけのヒントしか無い Step では水準 A が空になる。
    let t = pack(&ws, &hs("hb"));
    let d = hidden(&t);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].file(), "chapters/c1/chapter.yaml");
    assert_eq!(d[0].item(), "step:a");
    assert!(compile_dir(&t.0).is_ok());
    // A だけのヒントでは B・C が空になる（2 件）。
    assert_eq!(hidden(&pack(&ws, &hs("ha"))).len(), 2);
    // 全水準で出るヒント、両方を持つ Step は出ない。
    assert!(hidden(&pack(&ws, &hs("hall"))).is_empty());
    assert!(hidden(&pack(&ws, &hs("ha, hb"))).is_empty());
    // ヒントの無い Step・未定義のヒントだけの Step は対象外（後者は E-REF-HINT の担当）。
    assert!(hidden(&pack(&ws, &hs(""))).is_empty());
    let t = pack(&ws, &hs("nope"));
    assert!(hidden(&t).is_empty());
    assert!(validate_dir(&t.0)
        .iter()
        .any(|x| x.code() == DiagCode::RefHint));
}

#[test]
fn hint_hidden_does_not_echo_hint_ids() {
    let ws = format!("{WORKSHOP_HEAD}hints:\n  h-{SECRET}: {{text: t, levels: [B]}}\n");
    let t = pack(
        &ws,
        &chapter_with("{id: s1, node: a}", &format!(", hints: [h-{SECRET}]")),
    );
    assert_eq!(hidden(&t).len(), 2);
}

#[test]
fn pack_guidance_rejects_unknown_levels() {
    for g in ["D", "a", "''"] {
        let ws = format!("pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: {g}}}\n");
        let t = pack(&ws, &chapter_with("{id: s1, node: a}", ""));
        one(&t, "workshop.yaml", "pack:guidance");
    }
}

#[test]
fn pack_guidance_levels_rejects_bad_shapes() {
    for gl in [
        "[]",
        "{D: {}}",
        "{B: {unknown: 1}}",
        "{B: {hide_command_kinds: [shell]}}",
        "{B: {hide_words: [1]}}",
        "{B: 1}",
    ] {
        let ws = format!(
            "pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: A, guidance_levels: {gl}}}\n"
        );
        let t = pack(&ws, &chapter_with("{id: s1, node: a}", ""));
        one(&t, "workshop.yaml", "pack:guidance_levels");
    }
}

#[test]
fn start_guidance_rejects_unknown_levels() {
    for g in ["D", "b", "1", "[B]"] {
        let t = pack(
            WORKSHOP_HEAD,
            &chapter_with(&format!("{{id: s1, node: a, guidance: {g}}}"), ""),
        );
        one(&t, "chapters/c1/chapter.yaml", "scope");
    }
}

#[test]
fn body_levels_rejects_unknown_keys() {
    for k in ["D", "a"] {
        let t = pack(
            WORKSHOP_HEAD,
            &chapter_with(
                "{id: s1, node: a}",
                &format!(", body_levels: {{{k}: x.md}}"),
            ),
        );
        one(&t, "chapters/c1/chapter.yaml", "scope");
    }
}

#[test]
fn hint_levels_rejects_bad_shapes() {
    for lv in ["A", "[D]", "[1]", "[[A]]", "[a]"] {
        let ws = format!("{WORKSHOP_HEAD}hints:\n  h1: {{text: t, levels: {lv}}}\n");
        let t = pack(&ws, &chapter_with("{id: s1, node: a}", ", hints: [h1]"));
        one(&t, "workshop.yaml", "hint:h1");
    }
}

#[test]
fn rejection_messages_do_not_echo_written_values() {
    let ws = format!(
        "pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: {SECRET}, guidance_levels: {{{SECRET}: {{}}}}}}\nhints:\n  h1: {{text: t, levels: [{SECRET}]}}\n"
    );
    let t = pack(
        &ws,
        &chapter_with(
            &format!("{{id: s1, node: a, guidance: {SECRET}}}"),
            &format!(", body_levels: {{{SECRET}: x.md}}"),
        ),
    );
    let d = parse_err(&t);
    assert!(d.len() >= 4, "{d:?}");
}

#[test]
fn toml_format_rejects_the_same_values() {
    let t = Tmp::new("guidance-toml");
    t.put(
        "workshop.toml",
        b"[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"D\"\n",
    );
    t.put(
        "chapters/c1/chapter.toml",
        b"id = \"c1\"\ntitle = \"C\"\n\n[[starts]]\nid = \"s1\"\nnode = \"a\"\nguidance = \"Z\"\n\n[[steps]]\nid = \"a\"\ntitle = \"A\"\nfinal = true\n",
    );
    let d = compile_dir(&t.0).expect_err("reject");
    assert!(d.iter().any(|x| x.item() == "pack:guidance"), "{d:?}");
    assert!(d.iter().any(|x| x.item() == "scope"), "{d:?}");
}
