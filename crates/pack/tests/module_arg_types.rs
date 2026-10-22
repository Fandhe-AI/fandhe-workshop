//! モジュールの引数の型と範囲外の値の拒否（TASK-71 2/3、PACK-51・MOD-17 (c)・D-43）。
//!
//! `params` の隣の `param_types` で宣言した型（`bool`・`int`・`enum`）と範囲に合わない既定値・呼び出しの引数を、
//! `compile` も `validate` も `E-STRUCTURE` で拒否することを、一時ディレクトリの小さな教材で確かめる。
//!
//! - 範囲外の引数は実体を作らず止める（fail-closed）。`validate` と `compile` の判定はそろう
//! - 宣言を書かない引数は従来どおり任意の文字列で通り、IR も変わらない（加算）
//! - 診断に値（引数・既定値・`values`）を写さない
//!
//! 確かめないこと: `condition.param` の `eq` の値が宣言に入るかの検査（限界。`docs/known-gaps.md`）。
//! 試験では `unwrap` を使ってよい。

mod common;

use common::{assert_no_secret, Tmp, SECRET, WORKSHOP};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const END: &str = "  - {id: z, title: Z, final: true}\n";

fn module(params: &str, types: &str) -> String {
    let types = if types.is_empty() {
        String::new()
    } else {
        format!("param_types: {types}\n")
    };
    format!(
        "id: m\ntitle: M\nparams: {params}\n{types}steps:\n  - {{id: a, title: A, next: [{{to: z}}], input: {{kind: choice, options: [{{id: ok, label: OK}}]}}, rules: [{{id: r-a, on: {{option: ok}}, outcome: {{kind: accept}}}}], confirm: {{kind: none, reason: r}}}}\n{END}"
    )
}

/// 章が `m` を引数 `args` で 1 回呼ぶ教材。
fn pack(params: &str, types: &str, args: &str) -> Tmp {
    pack_with_workshop(WORKSHOP, params, types, args)
}

fn pack_with_workshop(workshop: &str, params: &str, types: &str, args: &str) -> Tmp {
    let t = Tmp::new("p51b");
    t.put("workshop.yaml", workshop.as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: u1}}]\nsteps:\n  - {{id: u1, title: U1, use: {{module: m, instance: i, args: {args}}}, final: true}}\n"
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t.put("modules/m/module.yaml", module(params, types).as_bytes());
    t
}

fn count(d: &[Diagnostic], code: DiagCode) -> usize {
    d.iter().filter(|x| x.code() == code).count()
}

/// compile と validate の両方が `E-STRUCTURE` をちょうど 1 件返し、他の診断を返さない。
fn assert_rejected_once(t: &Tmp) {
    let c = compile_dir(&t.0).expect_err("compile must fail");
    assert_eq!(c.len(), 1, "compile: {c:?}");
    assert_eq!(c[0].code(), DiagCode::Structure, "{c:?}");
    assert_no_secret(&c);
    let v = validate_dir(&t.0);
    assert_eq!(v.len(), 1, "validate: {v:?}");
    assert_eq!(v[0].code(), DiagCode::Structure, "{v:?}");
    assert_no_secret(&v);
}

fn assert_clean(t: &Tmp) {
    assert!(compile_dir(&t.0).is_ok(), "{:?}", compile_dir(&t.0).err());
    let v = validate_dir(&t.0);
    assert!(v.is_empty(), "{v:?}");
}

const BOOL: &str = "{setup: {type: bool}}";

#[test]
fn pack51_bool_arg_out_of_range_is_rejected_by_compile() {
    let t = pack("{setup: \"true\"}", BOOL, "{setup: \"flase\"}");
    let d = compile_dir(&t.0).expect_err("out of range");
    assert_eq!(count(&d, DiagCode::Structure), 1, "{d:?}");
    assert_eq!(d.len(), 1, "{d:?}");
}

#[test]
fn pack51_bool_arg_out_of_range_is_rejected_by_validate() {
    let t = pack("{setup: \"true\"}", BOOL, "{setup: \"flase\"}");
    let d = validate_dir(&t.0);
    assert_eq!(count(&d, DiagCode::Structure), 1, "{d:?}");
}

#[test]
fn pack51_int_arg_outside_min_max_is_rejected() {
    let types = "{n: {type: int, min: 1, max: 3}}";
    for bad in ["0", "4", "-1"] {
        let t = pack("{n: \"1\"}", types, &format!("{{n: \"{bad}\"}}"));
        assert_rejected_once(&t);
    }
    for ok in ["1", "2", "3"] {
        let t = pack("{n: \"1\"}", types, &format!("{{n: \"{ok}\"}}"));
        assert_clean(&t);
    }
}

#[test]
fn pack51_int_arg_non_canonical_is_rejected() {
    let types = "{n: {type: int}}";
    for bad in [
        "01",
        "+1",
        "1.0",
        "abc",
        "",
        " 1",
        "-0",
        "9223372036854775808",
    ] {
        let t = pack("{n: \"1\"}", types, &format!("{{n: \"{bad}\"}}"));
        assert_rejected_once(&t);
    }
    for ok in ["0", "-7", "9223372036854775807"] {
        let t = pack("{n: \"1\"}", types, &format!("{{n: \"{ok}\"}}"));
        assert_clean(&t);
    }
}

#[test]
fn pack51_enum_arg_not_in_values_is_rejected() {
    let types = "{mode: {type: enum, values: [fast, slow]}}";
    for bad in ["Fast", "medium", ""] {
        let t = pack("{mode: fast}", types, &format!("{{mode: \"{bad}\"}}"));
        assert_rejected_once(&t);
    }
    for ok in ["fast", "slow"] {
        let t = pack("{mode: fast}", types, &format!("{{mode: \"{ok}\"}}"));
        assert_clean(&t);
    }
}

#[test]
fn pack51_in_range_args_compile_and_ir_matches_untyped() {
    let typed = pack("{setup: \"true\"}", BOOL, "{setup: \"false\"}");
    let untyped = pack("{setup: \"true\"}", "", "{setup: \"false\"}");
    assert_clean(&typed);
    // param_types は IR に載らない。
    assert_eq!(
        compile_dir(&typed.0).unwrap(),
        compile_dir(&untyped.0).unwrap()
    );
}

#[test]
fn pack51_untyped_params_are_not_checked() {
    let t = pack("{setup: \"true\"}", "", "{setup: \"flase\"}");
    assert_clean(&t);
    // 一部だけ宣言した場合、宣言のない引数は従来どおり任意の文字列で通る。
    let t = pack(
        "{setup: \"true\", free: x}",
        BOOL,
        "{setup: \"true\", free: \"anything 123\"}",
    );
    assert_clean(&t);
}

fn uncalled(params: &str, types: &str) -> Tmp {
    let t = Tmp::new("p51u");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: [{id: s1, node: u1}]\nsteps:\n  - {id: u1, title: U1, final: true}\n",
    );
    t.put("modules/m/module.yaml", module(params, types).as_bytes());
    t
}

fn called_twice(params: &str, types: &str) -> Tmp {
    let t = Tmp::new("p51t");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: [{id: s1, node: u1}]\nsteps:\n  - {id: u1, title: U1, use: {module: m, instance: i1}, next: [{to: u2}]}\n  - {id: u2, title: U2, use: {module: m, instance: i2}, final: true}\n",
    );
    t.put("modules/m/module.yaml", module(params, types).as_bytes());
    t
}

#[test]
fn pack51_default_out_of_range_is_rejected_once() {
    let types = "{n: {type: int, min: 1, max: 3}}";
    // 呼び出し 0 回でも 2 回でも、既定値の違反は宣言の側の 1 件である。
    assert_rejected_once(&uncalled("{n: \"9\"}", types));
    assert_rejected_once(&called_twice("{n: \"9\"}", types));
    // 範囲内の既定値は通る。
    assert_clean(&called_twice("{n: \"2\"}", types));
}

#[test]
fn pack51_declaration_errors_are_structure() {
    let cases: [(&str, &str); 8] = [
        ("{n: \"1\"}", "{other: {type: bool}}"),
        ("{m: x}", "{m: {type: enum, values: []}}"),
        ("{m: x}", "{m: {type: enum}}"),
        ("{m: x}", "{m: {type: enum, values: [x, x]}}"),
        ("{b: \"true\"}", "{b: {type: bool, values: [true]}}"),
        ("{m: x}", "{m: {type: enum, values: [x], min: 1}}"),
        ("{n: \"1\"}", "{n: {type: int, min: 3, max: 1}}"),
        ("{n: \"1\"}", "{n: {type: int, values: [\"1\"]}}"),
    ];
    for (params, types) in cases {
        assert_rejected_once(&uncalled(params, types));
    }
}

#[test]
fn pack51_declaration_unknown_type_or_field_is_parse() {
    for types in [
        "{n: {type: float}}",
        "{n: {type: int, step: 2}}",
        "{n: {type: int, min: x}}",
        "{n: {}}",
    ] {
        let t = uncalled("{n: \"1\"}", types);
        let c = compile_dir(&t.0).expect_err("must fail");
        assert_eq!(c.len(), 1, "{types}: {c:?}");
        assert_eq!(c[0].code(), DiagCode::Parse, "{types}: {c:?}");
        let v = validate_dir(&t.0);
        assert_eq!(count(&v, DiagCode::Parse), 1, "{types}: {v:?}");
    }
}

#[test]
fn pack51_nested_arg_is_checked_after_substitution() {
    let make = |x: &str| {
        let t = Tmp::new("p51n");
        t.put("workshop.yaml", WORKSHOP.as_bytes());
        t.put(
            "chapters/c1/chapter.yaml",
            format!(
                "id: c1\ntitle: C\nstarts: [{{id: s1, node: u1}}]\nsteps:\n  - {{id: u1, title: U1, use: {{module: outer, instance: o, args: {{x: \"{x}\"}}}}, final: true}}\n"
            )
            .as_bytes(),
        );
        t.put(
            "modules/outer/module.yaml",
            b"id: outer\ntitle: O\nparams: {x: \"1\"}\nsteps:\n  - {id: o1, title: O1, use: {module: inner, instance: ii, args: {n: \"${x}\"}}, final: true}\n",
        );
        t.put(
            "modules/inner/module.yaml",
            module("{n: \"1\"}", "{n: {type: int, min: 1, max: 3}}")
                .replace("id: m", "id: inner")
                .as_bytes(),
        );
        t
    };
    assert_clean(&make("2"));
    assert_rejected_once(&make("9"));
}

#[test]
fn pack51_format_a_toml_param_types() {
    let toml = |arg: &str| {
        let t = Tmp::new("p51toml");
        t.put(
            "workshop.toml",
            b"[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n",
        );
        t.put(
            "chapters/c1/chapter.toml",
            format!(
                "id = \"c1\"\ntitle = \"C\"\n[[starts]]\nid = \"s1\"\nnode = \"u1\"\n[[steps]]\nid = \"u1\"\ntitle = \"U1\"\nfinal = true\n[steps.use]\nmodule = \"m\"\ninstance = \"i\"\n[steps.use.args]\nn = \"{arg}\"\n"
            )
            .as_bytes(),
        );
        t.put(
            "modules/m/module.toml",
            b"id = \"m\"\ntitle = \"M\"\n[params]\nn = \"1\"\n[param_types.n]\ntype = \"int\"\nmin = 1\nmax = 3\n[[steps]]\nid = \"a\"\ntitle = \"A\"\nfinal = true\n",
        );
        t
    };
    assert_clean(&toml("3"));
    assert_rejected_once(&toml("4"));
}

#[test]
fn pack51_vocab_mismatch_below_1_7() {
    let ws = |vocab: &str| {
        format!("pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: A{vocab}}}\n")
    };
    let old = pack_with_workshop(&ws(", vocab: \"1.6\""), "{setup: \"true\"}", BOOL, "{}");
    let d = validate_dir(&old.0);
    assert_eq!(count(&d, DiagCode::VocabMismatch), 1, "{d:?}");
    let ok = pack_with_workshop(&ws(", vocab: \"1.7\""), "{setup: \"true\"}", BOOL, "{}");
    let d = validate_dir(&ok.0);
    assert_eq!(count(&d, DiagCode::VocabMismatch), 0, "{d:?}");
    // 省略した教材は検査しない。
    let none = pack_with_workshop(&ws(""), "{setup: \"true\"}", BOOL, "{}");
    let d = validate_dir(&none.0);
    assert_eq!(count(&d, DiagCode::VocabMismatch), 0, "{d:?}");
}

#[test]
fn pack51_arg_values_are_not_echoed() {
    // 引数の値。
    let t = pack(
        "{setup: \"true\"}",
        BOOL,
        &format!("{{setup: \"{SECRET}\"}}"),
    );
    assert_rejected_once(&t);
    // 既定値。
    let t = uncalled(&format!("{{n: \"{SECRET}\"}}"), "{n: {type: int}}");
    assert_rejected_once(&t);
    // values の中身（宣言の重複）。
    let t = uncalled(
        &format!("{{m: \"{SECRET}\"}}"),
        &format!("{{m: {{type: enum, values: [\"{SECRET}\", \"{SECRET}\"]}}}}"),
    );
    let d = validate_dir(&t.0);
    assert!(count(&d, DiagCode::Structure) >= 1, "{d:?}");
    assert_no_secret(&d);
    let c = compile_dir(&t.0).expect_err("must fail");
    assert_no_secret(&c);
}
