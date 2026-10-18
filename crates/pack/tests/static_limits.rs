//! 静的検査の限界パック（条件の充足可能性・Step の巡回）の試験（PACK-30・D-43、TASK-58 1/2・#422）。
//!
//! D-43 は、`validate` に足す検査を存在しない要素への `remove`・`set_attr` だけに絞り、条件の充足可能性と
//! Step の巡回は検出できない限界として受け入れ、教材テスト（`tests/playthrough.*`）で補うと決めた。
//! 本ファイルは、その限界を「検出しないことが期待」として固定する。検査を足すなら spec 側の決定と
//! 診断コード一覧（TASK-195）の版上げが先で、そのときは本ファイルの期待を同じ変更で改める。
//!
//! - 限界パックは `testdata/pack-defects/limits/<形式>/<ID>` に置く。土台は `base/format-{a,b}` で、
//!   章のファイル 1 つだけを重ねる（`defect_packs.rs` の欠陥パックと同じ作り。ID の空間は `L##` で分ける）
//! - L01 は決して真にならない `when` の先の Step、L02 は `final` に着かない無条件の巡回
//! - 遮蔽の限界は #423 が `L03` 以降として足す余地がある
//! - 教材テスト（CLI `test`）での検出は TASK-404（#648）・TASK-398（#468）の完了前で、本ファイルでは確かめない

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{assert_no_secret, copy_dir, testdata, Tmp};

use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;

/// 限界パックの ID。表（`testdata/pack-defects/README.md`）と一致させる。
const LIMITS: [&str; 2] = ["L01", "L02"];

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

    fn chapter(self) -> &'static str {
        match self {
            Format::A => "chapters/ch-d/chapter.toml",
            Format::B => "chapters/ch-d/chapter.yaml",
        }
    }
}

fn limits_root() -> PathBuf {
    testdata("pack-defects/limits")
}

fn base_dir(f: Format) -> PathBuf {
    testdata("pack-defects/base").join(f.base())
}

fn overlay_dir(id: &str, f: Format) -> PathBuf {
    limits_root().join(f.dir()).join(id)
}

/// 土台を写し、章のファイルを限界パックのもので置き換えたパックを一時ディレクトリに作る。
fn assemble(id: &str, f: Format) -> Tmp {
    let t = Tmp::new("limit");
    copy_dir(&base_dir(f), &t.0);
    let new = fs::read(overlay_dir(id, f).join(f.chapter())).expect("read overlay");
    t.put(f.chapter(), &new);
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

fn ir_of(t: &Tmp) -> serde_json::Value {
    serde_json::from_slice(&compile_dir(&t.0).expect("compiles")).expect("IR is JSON")
}

/// IR の中から、ID が一致する Step を探す（章の入れ子の形に依存しないよう再帰で探す）。
fn find_step<'a>(v: &'a serde_json::Value, id: &str) -> Option<&'a serde_json::Value> {
    match v {
        serde_json::Value::Object(m) => {
            if m.get("id").and_then(|x| x.as_str()) == Some(id) && m.contains_key("next") {
                return Some(v);
            }
            m.values().find_map(|c| find_step(c, id))
        }
        serde_json::Value::Array(a) => a.iter().find_map(|c| find_step(c, id)),
        _ => None,
    }
}

#[test]
fn pack30_limit_dirs_match_table_and_only_replace_base_files() {
    for f in FORMATS {
        let mut dirs: Vec<String> = fs::read_dir(limits_root().join(f.dir()))
            .expect("read limits")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        dirs.sort();
        assert_eq!(
            dirs, LIMITS,
            "{f:?}: limit directories must equal the table"
        );

        for id in LIMITS {
            let mut files = Vec::new();
            files_under(&overlay_dir(id, f), "", &mut files);
            assert_eq!(files, vec![f.chapter()], "{id} {f:?}: one replaced file");
            let new = fs::read(overlay_dir(id, f).join(f.chapter())).expect("read overlay");
            let old = fs::read(base_dir(f).join(f.chapter())).expect("read base");
            assert_ne!(new, old, "{id} {f:?}: overlay must differ from base");
        }
    }
}

#[test]
fn pack30_limits_are_not_reported_by_validate() {
    for f in FORMATS {
        for id in LIMITS {
            let t = assemble(id, f);
            let diags = validate_dir(&t.0);
            assert_no_secret(&diags);
            // D-43 で検査に足さないと決めた範囲。足すなら spec 側の決定と TASK-195 の版上げが先。
            assert!(
                diags.is_empty(),
                "{id} {f:?}: a documented static-check limit must not be reported: {diags:?}"
            );
        }
    }
}

#[test]
fn pack30_limits_compile_and_formats_agree() {
    for id in LIMITS {
        let a = compile_dir(&assemble(id, Format::A).0).expect("compile format A");
        let b = compile_dir(&assemble(id, Format::B).0).expect("compile format B");
        assert_eq!(a, b, "{id}: the two formats must compile to the same IR");
    }
}

#[test]
fn pack30_limit_edges_are_present_in_ir() {
    for f in FORMATS {
        // L01: st-2-5 に、when つきの st-2-6 行きの辺と、無条件の st-2-7 行きの辺がある。
        let ir = ir_of(&assemble("L01", f));
        let next = find_step(&ir, "st-2-5").expect("st-2-5")["next"]
            .as_array()
            .expect("next array")
            .clone();
        let conditional = next
            .iter()
            .any(|e| e["to"] == "main/st-2-6" && e.get("when").is_some_and(|w| !w.is_null()));
        let plain = next
            .iter()
            .any(|e| e["to"] == "main/st-2-7" && e.get("when").is_none_or(|w| w.is_null()));
        assert!(
            conditional,
            "L01 {f:?}: conditional edge to st-2-6: {next:?}"
        );
        assert!(plain, "L01 {f:?}: unconditional edge to st-2-7: {next:?}");

        // L02: st-2-6 の next が st-2-1 だけを指す（巡回）。
        let ir = ir_of(&assemble("L02", f));
        let next = find_step(&ir, "st-2-6").expect("st-2-6")["next"]
            .as_array()
            .expect("next array")
            .clone();
        assert_eq!(next.len(), 1, "L02 {f:?}: one edge: {next:?}");
        assert_eq!(next[0]["to"], "main/st-2-1", "L02 {f:?}: loops back");
    }
}
