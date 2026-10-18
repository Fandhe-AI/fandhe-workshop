//! `pack.left_pane`（左エリアの設定）と `E-LEFT-*` の 3 種の結合試験（TASK-53 2/2、PACK-22）。

mod common;

use common::{Tmp, SECRET};
use fandhe_workshop_contract::left_pane::{read_left_pane, ViewTabs, WorldList};
use fandhe_workshop_pack::compile::compile;
use fandhe_workshop_pack::source;
use fandhe_workshop_pack::validate::validate;

const CHAPTER_FILE: &str = "chapters/c1/chapter.yaml";

/// ビューは `v1`・`v3` がファイルツリー、`v2` が文書。`left_pane` は YAML のフロー形式の文字列（空なら省略）。
fn workshop(left_pane: &str) -> String {
    let lp = if left_pane.is_empty() {
        String::new()
    } else {
        format!(", left_pane: {left_pane}")
    };
    format!(
        "pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: A{lp}}}\nviews:\n  v1: {{kind: file_tree}}\n  v2: {{kind: document}}\n  v3: {{kind: file_tree}}\n"
    )
}

/// 章は Step `a`（`a_extra` を足す）から最後の Step `b` へ進む。
fn pack(left_pane: &str, a_extra: &str) -> Tmp {
    let t = Tmp::new("left-pane");
    t.put("workshop.yaml", workshop(left_pane).as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: a}}]\nsteps:\n  - {{id: a, title: A, body: a.md, next: [{{to: b}}], {a_extra}}}\n  - {{id: b, title: B, final: true}}\n"
    );
    t.put(CHAPTER_FILE, chapter.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    t
}

fn compile_diags(t: &Tmp) -> Vec<String> {
    let lp = source::load(&t.0).expect("load");
    match compile(&lp) {
        Ok(_) => vec![],
        Err(d) => d.iter().map(|x| x.to_string()).collect(),
    }
}

fn validate_diags(t: &Tmp) -> Vec<String> {
    let lp = source::load(&t.0).expect("load");
    validate(&lp).iter().map(|x| x.to_string()).collect()
}

/// `E-LEFT-*` の行だけ。`compile` と `validate` が同じ行を返すことも確かめる。
fn left(t: &Tmp) -> Vec<String> {
    let pick = |v: Vec<String>| -> Vec<String> {
        v.into_iter().filter(|l| l.contains("E-LEFT-")).collect()
    };
    let c = pick(compile_diags(t));
    assert_eq!(pick(validate_diags(t)), c, "compile and validate agree");
    c
}

fn prefix(code: &str) -> String {
    format!("{CHAPTER_FILE}: step:a: {code}")
}

const ENTER_X: &str = "on_enter: [{put: {path: f/x, attrs: {k: v}}}]";
const CONFIRM: &str = "confirm: {kind: none, reason: r}";

fn ui_input(view: &str, path: &str) -> String {
    format!("input: {{kind: ui_action, targets: [{{view: {view}, path: {path}}}]}}, {CONFIRM}")
}

#[test]
fn default_settings_report_nothing() {
    // 2 つのビュー・宣言外の宛先・置かれない経路があっても、既定では検査しない。
    let extra = format!("views: [v1, v2], {}", ui_input("v3", "nowhere"));
    for lp in ["", "{}", "{world_list: shown, view_tabs: all}"] {
        let t = pack(lp, &extra);
        assert!(left(&t).is_empty(), "{lp}");
    }
}

#[test]
fn bad_left_pane_is_one_parse_error_without_values() {
    for bad in [
        "{view_tabs: x}",
        "{view_tabs: Hidden}",
        "{view_tabs: 5}",
        "{view_tabs: null}",
        "{world_list: all}",
        "{world_list: [1]}",
        "[1]",
        "{tabs: [a]}",
        "{view_tabs: hidden, style: x}",
    ] {
        let t = pack(bad, "views: [v1, v2]");
        let c = compile_diags(&t);
        assert_eq!(c.len(), 1, "{bad}: {c:?}");
        assert!(
            c[0].starts_with("workshop.yaml: pack:left_pane: E-PARSE"),
            "{}",
            c[0]
        );
        assert!(!c[0].contains("E-LEFT-"), "no E-LEFT on parse error");
        assert_eq!(validate_diags(&t), c, "validate agrees: {bad}");
    }
}

#[test]
fn valid_values_compile_and_read_back() {
    for w in WorldList::ALL {
        for tabs in ViewTabs::ALL {
            let t = pack(
                &format!(
                    "{{world_list: {}, view_tabs: {}}}",
                    w.as_str(),
                    tabs.as_str()
                ),
                "",
            );
            let lp = source::load(&t.0).expect("load");
            let ir = compile(&lp).expect("compile");
            let v = ir.pack.left_pane.as_ref().expect("left_pane");
            let read = read_left_pane(Some(v)).expect("read");
            assert_eq!((read.world_list, read.view_tabs), (w, tabs));
        }
    }
}

#[test]
fn multi_view_only_for_hidden_with_two_or_more() {
    let t = pack("{view_tabs: hidden}", "views: [v1, v2]");
    let d = left(&t);
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(d[0].starts_with(&prefix("E-LEFT-MULTI-VIEW")), "{}", d[0]);

    assert!(left(&pack("{view_tabs: hidden}", "views: [v1]")).is_empty());
    assert!(left(&pack("{view_tabs: hidden}", "")).is_empty());
    assert!(left(&pack("{view_tabs: declared}", "views: [v1, v2]")).is_empty());
    assert!(left(&pack(
        "{view_tabs: all, world_list: hidden}",
        "views: [v1, v2]"
    ))
    .is_empty());
}

#[test]
fn target_view_in_each_location() {
    let rule = format!(
        "rules: [{{on: {{ui: {{view: v2, path: f/x}}}}, outcome: {{kind: accept}}}}], {ENTER_X}"
    );
    let cases = [
        (
            format!("views: [v1], {ENTER_X}, {}", ui_input("v2", "f/x")),
            "input",
        ),
        (
            format!(
                "views: [v1], {ENTER_X}, input: {{kind: approval}}, confirm: {{kind: point_at, targets: [{{view: v2, path: f/x}}]}}"
            ),
            "confirm",
        ),
        (format!("views: [v1], {rule}, {CONFIRM}"), "rule"),
    ];
    for (extra, which) in cases {
        let t = pack("{view_tabs: declared}", &extra);
        let d = left(&t);
        assert_eq!(d.len(), 1, "{which}: {d:?}");
        assert!(
            d[0].starts_with(&prefix("E-LEFT-TARGET-VIEW")),
            "{which}: {}",
            d[0]
        );
        assert!(
            left(&pack("{view_tabs: all, world_list: hidden}", &extra)).is_empty(),
            "{which}: all shows every view"
        );
    }
}

#[test]
fn target_view_declared_keeps_first_view_per_kind() {
    // v3 は v1 と同じ種類の 2 つ目なので画面に出ない。v2 は別の種類なので出る。
    let ok = format!("views: [v1, v2], {ENTER_X}, {}", ui_input("v2", "f/x"));
    assert!(left(&pack("{view_tabs: declared}", &ok)).is_empty());
    let same_kind = format!("views: [v1, v3], {ENTER_X}, {}", ui_input("v3", "f/x"));
    let d = left(&pack("{view_tabs: declared}", &same_kind));
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(d[0].starts_with(&prefix("E-LEFT-TARGET-VIEW")), "{}", d[0]);
}

#[test]
fn target_view_hidden_keeps_only_the_first_view() {
    let extra = format!("views: [v1, v2], {ENTER_X}, {}", ui_input("v2", "f/x"));
    let d = left(&pack("{view_tabs: hidden}", &extra));
    assert_eq!(d.len(), 2, "{d:?}");
    assert!(d[0].starts_with(&prefix("E-LEFT-MULTI-VIEW")), "{}", d[0]);
    assert!(d[1].starts_with(&prefix("E-LEFT-TARGET-VIEW")), "{}", d[1]);
}

#[test]
fn undeclared_step_shows_first_view_by_id() {
    // 宣言の無い Step は workshop.views の辞書順で最初（v1）だけが出る（試作の仮置き）。
    let to_v1 = format!("{ENTER_X}, {}", ui_input("v1", "f/x"));
    assert!(left(&pack("{view_tabs: declared}", &to_v1)).is_empty());
    let to_v2 = format!("{ENTER_X}, {}", ui_input("v2", "f/x"));
    let d = left(&pack("{view_tabs: declared}", &to_v2));
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(d[0].starts_with(&prefix("E-LEFT-TARGET-VIEW")), "{}", d[0]);
}

#[test]
fn target_row_is_checked_even_with_world_list_hidden_only() {
    let extra = format!("views: [v1], {}", ui_input("v1", "nowhere/y"));
    let t = pack("{world_list: hidden}", &extra);
    let d = left(&t);
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(d[0].starts_with(&prefix("E-LEFT-TARGET-ROW")), "{}", d[0]);
    // 別の Step が置いていれば、現れるとみなす。
    let placed = format!("views: [v1], {ENTER_X}, {}", ui_input("v1", "f/x"));
    assert!(left(&pack("{world_list: hidden}", &placed)).is_empty());
}

#[test]
fn target_row_counts_every_place_that_puts_a_path() {
    let places = [
        "on_achieved: [{put: {path: f/x, attrs: {k: v}}}]",
        "on_enter: [{replace: {path: f/x, attrs: {k: v}}}]",
        "processing: [{name: p, effects: [{put: {path: f/x, attrs: {k: v}}}]}]",
        "rules: [{on: {command: c}, outcome: {kind: accept, effects: [{put: {path: f/x, attrs: {k: v}}}]}}]",
    ];
    for place in places {
        let extra = format!("views: [v1], {place}, {}", ui_input("v1", "f/x"));
        let t = pack("{world_list: hidden}", &extra);
        let mut w = workshop("{world_list: hidden}");
        w.push_str("commands:\n  c: {name: c}\n");
        t.put("workshop.yaml", w.as_bytes());
        let d = left(&t);
        assert!(d.is_empty(), "{place}: {d:?}");
    }
}

#[test]
fn target_row_sees_starts_effects() {
    let t = Tmp::new("left-pane-starts");
    t.put("workshop.yaml", workshop("{world_list: hidden}").as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts:\n  - {{id: s1, node: a, effects: [{{put: {{path: f/x, attrs: {{k: v}}}}}}]}}\nsteps:\n  - {{id: a, title: A, body: a.md, next: [{{to: b}}], views: [v1], {}}}\n  - {{id: b, title: B, final: true}}\n",
        ui_input("v1", "f/x")
    );
    t.put(CHAPTER_FILE, chapter.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    assert!(left(&t).is_empty());
}

#[test]
fn parameterised_targets_and_puts() {
    // `${` を含む宛先は判定しない。
    let skipped = format!("views: [v1], {}", ui_input("v1", "'f/${name}'"));
    assert!(left(&pack("{world_list: hidden}", &skipped)).is_empty());
    // `put` 側の `${...}` は空でない任意の文字列に当たる。
    let wild = "on_enter: [{put: {path: 'f/${name}/x', attrs: {k: v}}}]";
    let hit = format!("views: [v1], {wild}, {}", ui_input("v1", "f/abc/x"));
    assert!(left(&pack("{world_list: hidden}", &hit)).is_empty());
    let miss = format!("views: [v1], {wild}, {}", ui_input("v1", "f/abc/y"));
    let d = left(&pack("{world_list: hidden}", &miss));
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(d[0].contains("E-LEFT-TARGET-ROW"), "{}", d[0]);
}

#[test]
fn unknown_target_view_is_left_to_ref_view() {
    let extra = format!("views: [v1], {}", ui_input("nope", "z"));
    assert!(left(&pack("{view_tabs: declared}", &extra)).is_empty());
}

#[test]
fn module_steps_are_reported_in_the_module_file() {
    let t = pack("{view_tabs: hidden}", "");
    let module = format!(
        "id: m1\ntitle: M\nsteps:\n  - {{id: ma, title: MA, views: [v1, v2], {CONFIRM}}}\n"
    );
    t.put("modules/m1/module.yaml", module.as_bytes());
    let d = left(&t);
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(
        d[0].starts_with("modules/m1/module.yaml: step:ma: E-LEFT-MULTI-VIEW"),
        "{}",
        d[0]
    );
}

#[test]
fn diagnostics_do_not_leak_values() {
    // 経路は `/` を含むので `safe_ident` を通らず、値は写らない。
    let secret_path = format!("views: [v1], {}", ui_input("v1", &format!("x/{SECRET}")));
    let d = left(&pack("{world_list: hidden}", &secret_path));
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(!d[0].contains(SECRET), "{}", d[0]);
    assert!(d[0].ends_with("target path (id not shown) is not placed by any put or replace"));
}

#[test]
fn toml_form_reports_the_same_codes() {
    let t = Tmp::new("left-pane-toml");
    t.put(
        "workshop.toml",
        b"[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n\n[pack.left_pane]\nview_tabs = \"hidden\"\n\n[views.v1]\nkind = \"file_tree\"\n\n[views.v2]\nkind = \"document\"\n",
    );
    let chapter = concat!(
        "id = \"c1\"\ntitle = \"C\"\nstarts = [{ id = \"s1\", node = \"a\" }]\n",
        "[[steps]]\nid = \"a\"\ntitle = \"A\"\nbody = \"a.md\"\nviews = [\"v1\", \"v2\"]\nnext = [{ to = \"b\" }]\n",
        "[[steps]]\nid = \"b\"\ntitle = \"B\"\nfinal = true\n",
    );
    t.put("chapters/c1/chapter.toml", chapter.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    let d = left(&t);
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(
        d[0].starts_with("chapters/c1/chapter.toml: step:a: E-LEFT-MULTI-VIEW"),
        "{}",
        d[0]
    );
}

#[test]
fn cli_reports_all_three_codes_and_accepts_the_fixed_pack() {
    let bin = || std::process::Command::new(env!("CARGO_BIN_EXE_fandhe-pack"));
    let bad = pack(
        "{view_tabs: hidden}",
        &format!("views: [v1, v2], {}", ui_input("v2", "nowhere")),
    );
    let o = bin().arg("compile").arg(&bad.0).output().unwrap();
    assert_ne!(o.status.code(), Some(0));
    let out = String::from_utf8_lossy(&o.stdout);
    for code in [
        "E-LEFT-MULTI-VIEW",
        "E-LEFT-TARGET-VIEW",
        "E-LEFT-TARGET-ROW",
    ] {
        assert!(out.contains(code), "{code} in {out}");
    }
    assert!(out.contains("step:a"), "{out}");

    let good = pack(
        "{view_tabs: hidden}",
        &format!("views: [v1], {ENTER_X}, {}", ui_input("v1", "f/x")),
    );
    let o = bin().arg("compile").arg(&good.0).output().unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );
}

/// 章の Step `u` が、モジュール `m1`（Step `ma`、`ma_extra` を足す）を引数 `v` つきで呼ぶ。
fn module_pack(left_pane: &str, arg: &str, ma_extra: &str) -> Tmp {
    let t = Tmp::new("left-pane-mod");
    t.put("workshop.yaml", workshop(left_pane).as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: u}}]\nsteps:\n  - {{id: u, title: U, use: {{module: m1, instance: i1, args: {{v: {arg}}}}}, next: [{{to: b}}]}}\n  - {{id: b, title: B, final: true}}\n"
    );
    t.put(CHAPTER_FILE, chapter.as_bytes());
    let module = format!(
        "id: m1\ntitle: M\nparams: {{v: v1}}\nsteps:\n  - {{id: ma, title: MA, final: true, {ma_extra}}}\n"
    );
    t.put("modules/m1/module.yaml", module.as_bytes());
    t
}

#[test]
fn argument_driven_views_are_checked_after_expansion() {
    // 引数で宣言したビュー v2 と、固定の宛先 v2 は同じ。展開の前に v1 へ倒して誤拒否しない。
    let ok = "views: ['${v}'], input: {kind: ui_action, targets: [{view: v2, path: f/x}]}, confirm: {kind: none, reason: r}, on_enter: [{put: {path: f/x, attrs: {k: v}}}]";
    let t = module_pack("{view_tabs: declared}", "v2", ok);
    assert!(left(&t).is_empty(), "{:?}", left(&t));
    // 引数が別のビュー v1 なら、宛先 v2 は画面に出ない。
    let t = module_pack("{view_tabs: declared}", "v1", ok);
    let d = left(&t);
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(
        d[0].starts_with("modules/m1/module.yaml: step:ma: E-LEFT-TARGET-VIEW"),
        "{}",
        d[0]
    );
}

#[test]
fn argument_driven_targets_are_checked_after_expansion() {
    let body = "views: [v1], input: {kind: ui_action, targets: [{view: '${v}', path: f/x}]}, confirm: {kind: none, reason: r}, on_enter: [{put: {path: f/x, attrs: {k: v}}}]";
    // 画面内のビューを引数で指定すれば報告しない。
    let t = module_pack("{view_tabs: declared}", "v1", body);
    assert!(left(&t).is_empty(), "{:?}", left(&t));
    // 画面外のビューを引数で指定すると報告する。
    let t = module_pack("{view_tabs: declared}", "v2", body);
    let d = left(&t);
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(d[0].contains("E-LEFT-TARGET-VIEW"), "{}", d[0]);
    // 経路を引数で指定し、どこにも置かれなければ報告する。
    let row = "views: [v1], input: {kind: ui_action, targets: [{view: v1, path: 'f/${v}'}]}, confirm: {kind: none, reason: r}, on_enter: [{put: {path: f/other, attrs: {k: v}}}]";
    let t = module_pack("{world_list: hidden}", "zz", row);
    let d = left(&t);
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(d[0].contains("E-LEFT-TARGET-ROW"), "{}", d[0]);
}
