//! `pack.appearance`（外観）の診断の結合試験（TASK-53 1/2、PACK-21）。

mod common;

use common::{Tmp, CHAPTER};
use fandhe_workshop_contract::appearance::{read_appearance, Theme};
use fandhe_workshop_pack::compile::compile;
use fandhe_workshop_pack::source;
use fandhe_workshop_pack::validate::validate;
use fandhe_workshop_pack::vocab;

/// `appearance` は YAML のフロー形式の文字列で渡す。
fn pack_with(appearance: &str) -> Tmp {
    let t = Tmp::new("appearance");
    let w = format!(
        "pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: A, appearance: {appearance}}}\n"
    );
    t.put("workshop.yaml", w.as_bytes());
    t.put("chapters/c1/chapter.yaml", CHAPTER.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    t
}

fn diags(t: &Tmp) -> Vec<String> {
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

const BAD: &[&str] = &[
    "{theme: dark}",
    "{theme: Standard}",
    "{theme: 5}",
    "{theme: null}",
    "[1]",
    "{theme: standard, style: x}",
    "{css: x}",
    "{color: red}",
    "{font: serif}",
];

#[test]
fn four_values_compile_and_read_back() {
    for t in Theme::ALL {
        let p = pack_with(&format!("{{theme: {}}}", t.as_str()));
        let lp = source::load(&p.0).expect("load");
        let ir = compile(&lp).expect("compile");
        let v = ir.pack.appearance.as_ref().expect("appearance");
        assert_eq!(v["theme"], t.as_str());
        assert_eq!(read_appearance(Some(v)), Ok(t));
    }
}

#[test]
fn absent_appearance_stays_absent() {
    let t = common::minimal();
    let lp = source::load(&t.0).expect("load");
    let ir = compile(&lp).expect("compile");
    assert!(ir.pack.appearance.is_none());
}

#[test]
fn bad_appearance_is_one_parse_error_without_values() {
    for p in BAD {
        let t = pack_with(p);
        let d = diags(&t);
        assert_eq!(d.len(), 1, "{p}: {d:?}");
        assert!(
            d[0].starts_with("workshop.yaml: pack:appearance: E-PARSE"),
            "{}",
            d[0]
        );
        assert!(!d[0].contains("dark") && !d[0].contains("red"), "{}", d[0]);
        assert_eq!(validate_diags(&t), d, "validate agrees: {p}");
    }
}

#[test]
fn toml_form() {
    let mk = |theme: &str| {
        let t = Tmp::new("appearance-toml");
        let w = format!(
            "[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n\n[pack.appearance]\ntheme = \"{theme}\"\n"
        );
        t.put("workshop.toml", w.as_bytes());
        let chapter = concat!(
            "id = \"c1\"\ntitle = \"C\"\nstarts = [{ id = \"s1\", node = \"a\" }]\n",
            "[[steps]]\nid = \"a\"\ntitle = \"A\"\nbody = \"a.md\"\nnext = [{ to = \"b\" }]\ninput = { kind = \"choice\", options = [{ id = \"ok\", label = \"OK\" }] }\nrules = [{ id = \"r-a\", on = { option = \"ok\" }, outcome = { kind = \"accept\" } }]\nconfirm = { kind = \"none\", reason = \"r\" }\n",
            "[[steps]]\nid = \"b\"\ntitle = \"B\"\nfinal = true\n",
        );
        t.put("chapters/c1/chapter.toml", chapter.as_bytes());
        t.put("chapters/c1/a.md", b"# a\n");
        t
    };
    assert!(diags(&mk("reading")).is_empty());
    let d = diags(&mk("dark"));
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(d[0].contains("pack:appearance: E-PARSE"), "{}", d[0]);
}

#[test]
fn style_field_on_pack_is_rejected() {
    let t = Tmp::new("appearance-style");
    t.put(
        "workshop.yaml",
        b"pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A, style: x}\n",
    );
    t.put("chapters/c1/chapter.yaml", CHAPTER.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    let d: Vec<String> = match source::load(&t.0) {
        Err(d) => d.iter().map(|x| x.to_string()).collect(),
        Ok(lp) => match compile(&lp) {
            Ok(_) => vec![],
            Err(d) => d.iter().map(|x| x.to_string()).collect(),
        },
    };
    assert!(d.iter().any(|x| x.contains("E-PARSE")), "{d:?}");
}

#[test]
fn vocabulary_has_no_style_field() {
    let fields: Vec<&str> = vocab::TERMS
        .iter()
        .map(|t| t.term)
        .filter(|t| t.starts_with("pack.appearance.") && !t.contains('='))
        .collect();
    assert_eq!(fields, ["pack.appearance.theme"]);
    for t in vocab::TERMS {
        let w = t.term.to_ascii_lowercase();
        assert!(
            !(w.contains("style")
                || w.contains("css")
                || w.contains("color")
                || w.contains("font")),
            "{}",
            t.term
        );
    }
}

#[test]
fn cli_reports_and_accepts() {
    let bin = || std::process::Command::new(env!("CARGO_BIN_EXE_fandhe-pack"));
    let bad = pack_with("{theme: dark}");
    let o = bin().arg("compile").arg(&bad.0).output().unwrap();
    assert_ne!(o.status.code(), Some(0));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("E-PARSE") && out.contains("pack:appearance"),
        "{out}"
    );
    let good = pack_with("{theme: reading}");
    let o = bin().arg("compile").arg(&good.0).output().unwrap();
    assert_eq!(o.status.code(), Some(0));
}
