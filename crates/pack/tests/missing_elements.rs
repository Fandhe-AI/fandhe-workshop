//! 存在しない要素への `remove`・`set_attr` の検査（`E-REF-ELEMENT`）の試験（PACK-30・D-43、TASK-58 2/2・#423）。
//!
//! D-43 が `validate` に足すと決めた唯一の検査である。実行時は `Event::Warning` になり教材テストが失敗する
//! （H-6）欠陥を、「教材のどこの `put`・`replace` も置かない経路」として静的に報告する。判定は必要条件の
//! 近似で、順序に依る不在は検出しない（`element_check` のモジュール文書）。
//!
//! - 欠陥パックは `testdata/pack-defects/elements/<形式>/<ID>` に置く。土台は `base/format-{a,b}` で、
//!   差し替えるファイルだけを重ねる（`defect_packs.rs` と同じ作り。ID の空間は `M##` で分ける）
//! - M01 は `on_enter` の `remove`、M02 は `on_achieved` の `set_attr`、M03 はモジュールの引数に依る経路
//!   （展開の後でだけ決まる）
//! - 対照として、置かれる経路に変えたパックで診断が 0 件になることを確かめる
//! - 検出の判定は `defect_packs.rs` と同じで、コード・ファイル・項目がすべて一致する診断が 1 件以上あること

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{assert_no_secret, copy_dir, testdata, Tmp};

use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

#[derive(Clone, Copy, Debug)]
enum Format {
    A,
    B,
}

const FORMATS: [Format; 2] = [Format::A, Format::B];

impl Format {
    fn dir(self) -> &'static str {
        match self {
            Format::A => "a",
            Format::B => "b",
        }
    }

    fn base(self) -> &'static str {
        match self {
            Format::A => "format-a",
            Format::B => "format-b",
        }
    }

    fn ext(self) -> &'static str {
        match self {
            Format::A => "toml",
            Format::B => "yaml",
        }
    }
}

/// 期待の表の 1 行。`file` は拡張子を除いたパックの根からの相対パス。
struct Defect {
    id: &'static str,
    file: &'static str,
    item: &'static str,
}

const CHAPTER: &str = "chapters/ch-d/chapter";
const INNER: &str = "modules/mod-inner/module";
const DEFECTS: [Defect; 3] = [
    Defect {
        id: "M01",
        file: CHAPTER,
        item: "step:st-2-4",
    },
    Defect {
        id: "M02",
        file: CHAPTER,
        item: "step:st-2-5",
    },
    Defect {
        id: "M03",
        file: INNER,
        item: "step:u-a",
    },
];

fn elements_root() -> PathBuf {
    testdata("pack-defects/elements")
}

fn base_dir(f: Format) -> PathBuf {
    testdata("pack-defects/base").join(f.base())
}

fn overlay_dir(d: &Defect, f: Format) -> PathBuf {
    elements_root().join(f.dir()).join(d.id)
}

fn rel_file(d: &Defect, f: Format) -> String {
    format!("{}.{}", d.file, f.ext())
}

fn assemble(d: &Defect, f: Format) -> Tmp {
    let t = Tmp::new(&format!("element-{}-{}", d.id, f.dir()));
    copy_dir(&base_dir(f), &t.0);
    copy_dir(&overlay_dir(d, f), &t.0);
    t
}

fn files_under(dir: &Path, prefix: &str, out: &mut Vec<String>) {
    for e in fs::read_dir(dir).expect("read_dir").flatten() {
        let name = format!("{prefix}{}", e.file_name().to_string_lossy());
        if e.path().is_dir() {
            files_under(&e.path(), &format!("{name}/"), out);
        } else {
            out.push(name);
        }
    }
    out.sort();
}

fn element_diags(diags: &[Diagnostic]) -> Vec<&Diagnostic> {
    diags
        .iter()
        .filter(|d| d.code() == DiagCode::RefElement)
        .collect()
}

/// 土台のファイルを読み、`from` を `to` に 1 回だけ置き換えて、パックの中の同じ場所へ書く。
fn rewrite(t: &Tmp, rel: &str, from: &str, to: &str) {
    let text = fs::read_to_string(t.0.join(rel)).expect("read file");
    assert_eq!(
        text.matches(from).count(),
        1,
        "{rel}: `{from}` must occur once"
    );
    t.put(rel, text.replacen(from, to, 1).as_bytes());
}

#[test]
fn pack30_element_dirs_match_table_and_only_replace_base_files() {
    for f in FORMATS {
        let mut dirs: Vec<String> = fs::read_dir(elements_root().join(f.dir()))
            .expect("read elements")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        dirs.sort();
        let want: Vec<&str> = DEFECTS.iter().map(|d| d.id).collect();
        assert_eq!(
            dirs, want,
            "{f:?}: element directories must equal the table"
        );

        for d in &DEFECTS {
            let mut files = Vec::new();
            files_under(&overlay_dir(d, f), "", &mut files);
            assert_eq!(
                files,
                vec![rel_file(d, f)],
                "{} {f:?}: one replaced file",
                d.id
            );
            let new = fs::read(overlay_dir(d, f).join(&files[0])).expect("read overlay");
            let old = fs::read(base_dir(f).join(&files[0])).expect("base has the same path");
            assert_ne!(new, old, "{} {f:?}: overlay must differ from base", d.id);
        }
    }
}

#[test]
fn pack30_missing_element_defects_reported_at_expected_location() {
    for f in FORMATS {
        for d in &DEFECTS {
            let diags = validate_dir(&assemble(d, f).0);
            assert_no_secret(&diags);
            let hit = element_diags(&diags)
                .iter()
                .any(|x| x.file() == rel_file(d, f) && x.item() == d.item);
            assert!(
                hit,
                "{} {f:?}: want E-REF-ELEMENT {} {}; got {diags:?}",
                d.id,
                rel_file(d, f),
                d.item
            );
            // 展開の前と後で同じ内容を重ねて報告しない（M03 は展開の後でだけ決まる）。
            assert_eq!(
                element_diags(&diags).len(),
                1,
                "{} {f:?}: exactly one E-REF-ELEMENT: {diags:?}",
                d.id
            );
        }
    }
}

#[test]
fn pack30_element_defects_compile_and_formats_agree() {
    // `compile` はこの検査を行わない（D-43 は `validate` を指定）。IR は両形式で一致する。
    for d in &DEFECTS {
        let a = compile_dir(&assemble(d, Format::A).0).expect("compile format A");
        let b = compile_dir(&assemble(d, Format::B).0).expect("compile format B");
        assert_eq!(
            a, b,
            "{}: the two formats must compile to the same IR",
            d.id
        );
    }
}

#[test]
fn pack30_element_defect_effects_are_in_ir() {
    for f in FORMATS {
        let ir = |id: &str| -> String {
            let d = DEFECTS.iter().find(|d| d.id == id).expect("defect");
            String::from_utf8(compile_dir(&assemble(d, f).0).expect("compiles")).expect("utf8")
        };
        assert!(ir("M01").contains(r#""remove": "lost-item""#), "M01 {f:?}");
        let m2 = ir("M02");
        assert!(
            m2.contains(r#""set_attr""#) && m2.contains("lost-item"),
            "M02 {f:?}"
        );
        assert!(ir("M03").contains(r#""remove": "lost-item""#), "M03 {f:?}");
    }
}

#[test]
fn pack30_control_placed_paths_are_not_reported() {
    for f in FORMATS {
        // M03 の引数の既定値を、土台が置く経路 `items/x` にする。
        let t = assemble(&DEFECTS[2], f);
        let (from, to) = match f {
            Format::A => ("target = \"lost-item\"", "target = \"items/x\""),
            Format::B => ("target: lost-item", "target: items/x"),
        };
        rewrite(&t, &rel_file(&DEFECTS[2], f), from, to);
        let diags = validate_dir(&t.0);
        assert!(
            element_diags(&diags).is_empty(),
            "M03 control {f:?}: {diags:?}"
        );
        assert!(diags.is_empty(), "M03 control {f:?}: {diags:?}");

        // M01 の `remove` を、土台が置く経路にする。
        let t = assemble(&DEFECTS[0], f);
        let (from, to) = match f {
            Format::A => ("remove = \"lost-item\"", "remove = \"items/x\""),
            Format::B => ("remove: lost-item", "remove: items/x"),
        };
        rewrite(&t, &rel_file(&DEFECTS[0], f), from, to);
        assert!(validate_dir(&t.0).is_empty(), "M01 control {f:?}");
    }
}

#[test]
fn pack30_replace_and_templated_put_place_the_path() {
    for f in FORMATS {
        // `replace` だけが置く経路への `set_attr` と、`${}` を含む `put` が置く経路への `remove` は報告しない。
        let t = assemble(&DEFECTS[1], f);
        let (from, to) = match f {
            Format::A => (
                "on_achieved = [{ set_attr = { path = \"lost-item\"",
                "on_enter = [{ replace = { path = \"lost-item\" } }]\non_achieved = [{ set_attr = { path = \"lost-item\"",
            ),
            Format::B => (
                "on_achieved: [{ set_attr: { path: lost-item",
                "on_enter: [{ replace: { path: lost-item } }]\n    on_achieved: [{ set_attr: { path: lost-item",
            ),
        };
        rewrite(&t, &rel_file(&DEFECTS[1], f), from, to);
        let diags = validate_dir(&t.0);
        assert!(element_diags(&diags).is_empty(), "replace {f:?}: {diags:?}");

        let t = assemble(&DEFECTS[0], f);
        let (from, to) = match f {
            Format::A => (
                "on_enter = [{ remove = \"lost-item\" }]",
                "on_enter = [{ remove = \"lost-item\" }, { put = { path = \"lost-${n}\" } }]",
            ),
            Format::B => (
                "on_enter: [{ remove: lost-item }]",
                "on_enter: [{ remove: lost-item }, { put: { path: \"lost-${n}\" } }]",
            ),
        };
        rewrite(&t, &rel_file(&DEFECTS[0], f), from, to);
        let diags = validate_dir(&t.0);
        assert!(
            element_diags(&diags).is_empty(),
            "templated put {f:?}: {diags:?}"
        );
    }
}

#[test]
fn pack30_start_effects_are_checked_and_put_by_start_places() {
    let t = Tmp::new("element-start");
    copy_dir(&base_dir(Format::A), &t.0);
    let rel = "chapters/ch-d/chapter.toml";
    rewrite(
        &t,
        rel,
        "id = \"s-1\"\nnode = \"st-2-1\"",
        "id = \"s-1\"\nnode = \"st-2-1\"\neffects = [{ set_attr = { path = \"lost-item\", attr = \"state\", value = \"v\" } }]",
    );
    let diags = validate_dir(&t.0);
    let hits = element_diags(&diags);
    assert_eq!(hits.len(), 1, "{diags:?}");
    assert_eq!(hits[0].item(), "start:s-1");
    assert_eq!(hits[0].file(), rel);

    // 開始点の `put` が置く経路なら報告しない。
    rewrite(
        &t,
        rel,
        "id = \"s-7\"\nnode = \"st-2-7\"",
        "id = \"s-7\"\nnode = \"st-2-7\"\neffects = [{ put = { path = \"lost-item\" } }]",
    );
    assert!(element_diags(&validate_dir(&t.0)).is_empty());
}

#[test]
fn pack30_unsafe_path_and_values_are_not_echoed() {
    let t = Tmp::new("element-unsafe");
    copy_dir(&base_dir(Format::A), &t.0);
    rewrite(
        &t,
        "chapters/ch-d/chapter.toml",
        "title = \"次へ進む準備をする\"",
        "title = \"次へ進む準備をする\"\non_enter = [{ set_attr = { path = \"bad path SECRET-VALUE\", attr = \"state\", value = \"SECRET-VALUE\" } }]",
    );
    let diags = validate_dir(&t.0);
    let hits = element_diags(&diags);
    assert_eq!(hits.len(), 1, "{diags:?}");
    assert!(hits[0].message().contains("(id not shown)"));
    for d in &diags {
        assert!(!d.to_string().contains("SECRET-VALUE"), "{d}");
    }
}

#[test]
fn pack30_module_start_with_param_path_is_checked_after_expansion() {
    // モジュールの `starts[].effects` の `remove: "${target}"` も、展開の後に定義元の開始点として報告する。
    let t = assemble(&DEFECTS[2], Format::A);
    let rel = "modules/mod-inner/module.toml";
    rewrite(
        &t,
        rel,
        "[params]\ntarget = \"lost-item\"\n",
        "[params]\ntarget = \"lost-item\"\n\n[[starts]]\nid = \"us-1\"\nnode = \"u-b\"\neffects = [{ remove = \"${target}\" }]\n",
    );
    let diags = validate_dir(&t.0);
    let hit = element_diags(&diags)
        .iter()
        .any(|x| x.file() == rel && x.item() == "start:us-1");
    assert!(hit, "{diags:?}");
}

#[test]
fn pack30_chapter_templated_path_is_checked_without_substitution() {
    // 章は引数を置換されない。`${x}` を含む経路でも、どこにも置かれなければ報告する。
    let t = Tmp::new("element-chapter-templated");
    copy_dir(&base_dir(Format::A), &t.0);
    let rel = "chapters/ch-d/chapter.toml";
    rewrite(
        &t,
        rel,
        "title = \"次へ進む準備をする\"",
        "title = \"次へ進む準備をする\"\non_enter = [{ remove = \"lost-${x}\" }]",
    );
    let diags = validate_dir(&t.0);
    let hits = element_diags(&diags);
    assert_eq!(hits.len(), 1, "{diags:?}");
    assert_eq!(hits[0].file(), rel);
}
