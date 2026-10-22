//! 隠す語の検査（`E-GUIDANCE-LEAK`）の範囲の試験（PACK-52・GUIDE-5・D-43。TASK-71 3/3）。
//!
//! 決定 D-43（2026-10-12）に従い、範囲に足すのは (a) モジュールの実体の引数で渡す `do`・`do_levels` の
//! 置換後の文字列だけである。(b) ヒントの段 2・3 と (c) 開始点の `guidance` も `body_levels` も持たない章の本文は
//! 作成者の規約として検査の外に置き、検出しないことを観察の試験（`pack52_b_`・`pack52_c_`）で固定する。
//! 検査は `validate` だけで、`compile` の結果は変わらない。

mod common;

use common::{assert_no_secret, Tmp, SECRET};

use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const WS: &str = "pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A, guidance_levels: {B: {hide_command_kinds: [skill], hide_words: ['秘匿語']}, C: {hide_words: ['cword']}}}\ncommands:\n  help: builtin\n  make-item: skill\n";

/// 章 c1 が実体 i1 でモジュール m を呼ぶ。`start_extra` は開始点の追加欄（例: `, guidance: B`）、`args` は `use.args`。
fn chapter(start_extra: &str, args: &str) -> String {
    format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: u1{start_extra}}}]\nsteps:\n  - {{id: u1, title: U1, use: {{module: m, instance: i1{args}}}, final: true}}\n"
    )
}

fn pack(ws: &str, chapter: &str, module: &str) -> Tmp {
    let t = Tmp::new("leak-scope");
    t.put("workshop.yaml", ws.as_bytes());
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t.put("chapters/c1/a.md", "# a\n".as_bytes());
    t.put("modules/m/module.yaml", module.as_bytes());
    t.put("modules/m/a.md", "# a\n".as_bytes());
    t
}

fn leaks(t: &Tmp) -> Vec<Diagnostic> {
    let d = validate_dir(&t.0);
    assert_no_secret(&d);
    d.into_iter()
        .filter(|x| x.code() == DiagCode::GuidanceLeak)
        .collect()
}

/// モジュールの Step `ms` が `do` に引数 `d` を差し込む。`d` の既定値は安全な文字列。
const MOD_DO: &str = "id: m\ntitle: M\nparams: {d: safe}\nsteps:\n  - {id: ms, title: MS, body: a.md, do: '${d}', final: true}\n";

#[test]
fn pack52_a_argument_do_leak_is_reported() {
    let t = pack(
        WS,
        &chapter(", guidance: B", ", args: {d: '/make-item'}"),
        MOD_DO,
    );
    let d = leaks(&t);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].file(), "modules/m/module.yaml");
    assert_eq!(d[0].item(), "step:ms");
    // 検査は validate だけで、compile は通す。
    assert!(compile_dir(&t.0).is_ok());
}

#[test]
fn pack52_a_default_param_leak_is_reported() {
    let module = "id: m\ntitle: M\nparams: {d: '/make-item を打つ'}\nsteps:\n  - {id: ms, title: MS, body: a.md, do: '${d}', final: true}\n";
    let t = pack(WS, &chapter(", guidance: B", ""), module);
    let d = leaks(&t);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].item(), "step:ms");
}

#[test]
fn pack52_a_do_levels_argument_is_checked_at_that_level() {
    // B の値が漏れれば 1 件。do（A）の値は B・C で別の文なので読まれない。
    let module = "id: m\ntitle: M\nparams: {d: safe}\nsteps:\n  - {id: ms, title: MS, body: a.md, do: '/make-item を打つ', do_levels: {B: '${d}', C: '安全な文'}, final: true}\n";
    let t = pack(WS, &chapter("", ", args: {d: '/make-item'}"), module);
    let d = leaks(&t);
    assert_eq!(d.len(), 1, "{d:?}");
    // B の値が安全なら 0 件（do の skill 名は B では読まれない）。
    let t = pack(WS, &chapter("", ", args: {d: '安全'}"), module);
    assert!(leaks(&t).is_empty());
}

#[test]
fn pack52_a_nested_argument_is_substituted() {
    let outer = "id: m\ntitle: M\nparams: {o: safe}\nsteps:\n  - {id: mu, title: MU, use: {module: n, instance: j1, args: {d: '${o}'}}, final: true}\n";
    let inner = "id: n\ntitle: N\nparams: {d: safe}\nsteps:\n  - {id: ns, title: NS, body: a.md, do: '${d}', final: true}\n";
    let t = pack(
        WS,
        &chapter(", guidance: B", ", args: {o: '/make-item'}"),
        outer,
    );
    t.put("modules/n/module.yaml", inner.as_bytes());
    t.put("modules/n/a.md", "# a\n".as_bytes());
    let d = leaks(&t);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].file(), "modules/n/module.yaml");
    assert_eq!(d[0].item(), "step:ns");
}

#[test]
fn pack52_a_clean_argument_and_level_a_have_no_leak() {
    // 引数の値が安全。
    let t = pack(WS, &chapter(", guidance: B", ", args: {d: 安全}"), MOD_DO);
    assert!(leaks(&t).is_empty());
    // 開始点が A・無しの章から呼ぶと、漏れる値でも対象外。
    for g in [", guidance: A", ""] {
        let t = pack(WS, &chapter(g, ", args: {d: '/make-item'}"), MOD_DO);
        assert!(leaks(&t).is_empty(), "{g}");
    }
    // 規則の無い水準（C の語は cword だけ）では skill 名は漏れない。
    let t = pack(
        WS,
        &chapter(", guidance: C", ", args: {d: '/make-item'}"),
        MOD_DO,
    );
    assert!(leaks(&t).is_empty());
    let t = pack(WS, &chapter(", guidance: C", ", args: {d: cword}"), MOD_DO);
    assert_eq!(leaks(&t).len(), 1);
}

#[test]
fn pack52_a_not_double_reported() {
    // 定義の段の字面にすでに語がある do は、既存の検査の 1 件だけ。
    let module = "id: m\ntitle: M\nparams: {d: safe}\nsteps:\n  - {id: ms, title: MS, body: a.md, do: '/make-item ${d}', final: true}\n";
    let t = pack(
        WS,
        &chapter(", guidance: B", ", args: {d: '/make-item'}"),
        module,
    );
    assert_eq!(leaks(&t).len(), 1);
    // 同じモジュールを 2 実体（同じ漏れる値）で呼んでも 1 件にまとめる。
    let two = "id: c1\ntitle: C\nstarts: [{id: s1, node: u1, guidance: B}]\nsteps:\n  - {id: u1, title: U1, use: {module: m, instance: i1, args: {d: '/make-item'}}, next: [{to: u2}]}\n  - {id: u2, title: U2, use: {module: m, instance: i2, args: {d: '/make-item'}}, final: true}\n";
    let t = pack(WS, two, MOD_DO);
    assert_eq!(leaks(&t).len(), 1, "{:?}", leaks(&t));
}

#[test]
fn pack52_b_hint_tiers_are_not_checked() {
    // D-43: ヒントの段 2・3 は作成者の規約で、検出しない。
    let ws = format!("{WS}hints:\n  h1: {{text: '/make-item を打つ', levels: [B, C]}}\n");
    let ch = "id: c1\ntitle: C\nstarts: [{id: s1, node: a, guidance: B}]\nsteps:\n  - {id: a, title: A, body: a.md, hints: [h1], final: true}\n";
    let t = pack(&ws, ch, MOD_DO);
    assert!(leaks(&t).is_empty());
}

#[test]
fn pack52_c_undeclared_chapter_body_has_no_false_positive() {
    // 開始点の guidance も body_levels も持たない章の本文と do は、隠す語があっても 0 件（誤検出 0 件）。
    let ch = "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n  - {id: a, title: A, body: a.md, do: '/make-item を打つ', final: true}\n";
    let t = pack(WS, ch, MOD_DO);
    t.put("chapters/c1/a.md", "/make-item 秘匿語\n".as_bytes());
    assert!(leaks(&t).is_empty());
    // 正の対照: 開始点を guidance: B にすると、同じ本文・do が検出される（対象は宣言に結びつく）。
    let ch = ch.replace("node: a}", "node: a, guidance: B}");
    let t = pack(WS, &ch, MOD_DO);
    t.put("chapters/c1/a.md", "/make-item 秘匿語\n".as_bytes());
    assert_eq!(leaks(&t).len(), 2);
}

#[test]
fn pack52_values_are_not_echoed() {
    let ws = format!("pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: A, guidance_levels: {{B: {{hide_words: ['{SECRET}']}}}}}}\n");
    let module = format!("id: m\ntitle: M\nparams: {{d: '{SECRET}'}}\nsteps:\n  - {{id: ms, title: MS, body: a.md, do: '${{d}}', final: true}}\n");
    let t = pack(
        &ws,
        &chapter(", guidance: B", &format!(", args: {{d: '{SECRET}'}}")),
        &module,
    );
    assert_eq!(leaks(&t).len(), 1);
    let t = pack(&ws, &chapter(", guidance: B", ""), &module);
    assert_eq!(leaks(&t).len(), 1);
}

#[test]
fn pack52_rounds_testdata_reports_known_true_leaks() {
    // R-5 が記録した真の漏れ。issue-to-merge・idea-cycle の do は `body_levels` を宣言した Step で、
    // ch-round1 は引数を渡さず既定値（skill 名を含む `do_*`）を使う。このため水準 B・C で skill 名が出る。
    // ch-round2 は skill 名を含まない値を渡しており、漏れの元ではない。
    let dir = common::testdata("module-reuse-rounds/test-data/pack");
    let d: Vec<Diagnostic> = validate_dir(&dir)
        .into_iter()
        .filter(|x| x.code() == DiagCode::GuidanceLeak)
        .collect();
    assert_no_secret(&d);
    let mut items: Vec<(String, String)> = d
        .iter()
        .map(|x| (x.file().to_string(), x.item().to_string()))
        .collect();
    items.sort();
    let want: Vec<(String, String)> = [
        ("idea-cycle", "st-2-1"),
        ("issue-to-merge", "st-4-1"),
        ("issue-to-merge", "st-5-1"),
        ("issue-to-merge", "st-6-1"),
        ("issue-to-merge", "st-7-1"),
    ]
    .iter()
    // 水準 B・C の 2 件ずつ。
    .flat_map(|(m, st)| {
        let one = (format!("modules/{m}/module.yaml"), format!("step:{st}"));
        [one.clone(), one]
    })
    .collect();
    assert_eq!(items, want, "{d:?}");
}
