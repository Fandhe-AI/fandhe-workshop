//! 製品の左エリアの行の組み立て（`product_rows`）の試験（TASK-799・D-100）。
//!
//! 3 つを確かめる。(1) `item_rows` の印が `artifact_rows` の印と行ごとに一致すること（通し・合成）。
//! (2) 属性の読み方の仮置き（`read_cell`・`path_depth`）。(3) 7 種のビューそれぞれの `ArtifactPart::view` の HTML が、
//! `tests/data/product_view_html/<種類>.html` の期待値と一致すること。
//!
//! 後続の子（種類ごとの結線）は、自分の種類の期待値のファイルだけを意図して書き換える。
//! 期待値を書き出すときは環境変数 `PRODUCT_VIEW_PRINT=1` で標準出力に出した値を手で保存する（試験は作業ツリーを書き換えない）。
//! 試験では `unwrap` を使ってよい。

#[path = "common/ch0_3_journey.rs"]
mod ch0_3_journey;
#[path = "common/ch4_9_journey.rs"]
mod ch4_9_journey;

use std::fs;
use std::path::{Path, PathBuf};

use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::projection::Mark;
use fandhe_workshop_core::world::World;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_web::a11y::ChangeMark;
use fandhe_workshop_web::artifact_view::{Phase, RowKind, ViewKind};
use fandhe_workshop_web::engine_parts::{artifact_rows, standard_registry};
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::product_rows::{item_rows, path_depth, read_cell};

fn ir_of(dir: &Path) -> Ir {
    let bytes = compile_dir(dir).unwrap_or_else(|d| panic!("compile: {} 件の診断", d.len()));
    load_ir(&bytes).expect("load_ir")
}

fn approve(h: &mut EngineHost) {
    h.update(EngineHost::decode_action("decide", "approve").unwrap());
    let mut guard = 0;
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        guard += 1;
        assert!(guard < 50, "進行が終わらない");
    }
}

/// 親の `section`（`part-artifact-view`）の html。
fn artifact_section(h: &EngineHost) -> String {
    let html = render_component_html(h);
    let start = html
        .find("<section data-testid=\"part-artifact-view\"")
        .expect("section の開始");
    let rest = &html[start..];
    let end = rest.find("</section>").expect("section の終了") + "</section>".len();
    rest[..end].to_owned()
}

fn kind_name(k: ViewKind) -> &'static str {
    k.as_str()
}

/// 7 種それぞれの、入った直後・1 回目の承認の後・2 回目の承認の後の html を `\n--- t<n> ---\n` でつないだもの。
fn actual_for(kind: ViewKind) -> String {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/product_view_pack");
    let ir = ir_of(&dir);
    let node = ir.starts[0].node.clone();
    let mut h =
        EngineHost::with_session(Session::entered_at(ir, &node), standard_registry()).unwrap();
    let mut out = String::new();
    for t in 0..3 {
        if t > 0 {
            approve(&mut h);
        }
        // Step の移動で選択は先頭のビューに戻るため、撮る前に毎回選び直す。
        h.update(HostAction::SelectView(kind));
        out.push_str(&format!("--- t{t} ---\n{}\n", artifact_section(&h)));
    }
    out
}

fn expected_path(kind: ViewKind) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/product_view_html")
        .join(format!("{}.html", kind_name(kind)))
}

#[test]
fn seven_kinds_html_matches_expected() {
    let print = std::env::var("PRODUCT_VIEW_PRINT").is_ok();
    for kind in ViewKind::ALL {
        let actual = actual_for(kind);
        if print {
            println!("@@@ {}\n{actual}", kind_name(kind));
            continue;
        }
        let expected = fs::read_to_string(expected_path(kind)).unwrap_or_default();
        assert_eq!(
            actual.trim_end_matches('\n'),
            expected.trim_end_matches('\n'),
            "{} の html が期待値と違う",
            kind_name(kind)
        );
    }
}

fn marks_match(world: &World, marks: &[Mark]) -> (usize, usize, usize) {
    let ar = artifact_rows(world, marks);
    let rows = item_rows(world, &ar, |_| RowKind::Task).unwrap();
    assert_eq!(rows.len(), ar.len());
    let (mut add, mut change, mut none) = (0, 0, 0);
    for (row, (path, mark)) in rows.iter().zip(ar.iter()) {
        assert_eq!(row.id().as_str(), path.as_str());
        assert_eq!(row.shown(Phase::After).unwrap().mark, *mark, "{path}");
        match mark {
            Some(ChangeMark::Add) => add += 1,
            Some(ChangeMark::Change) => change += 1,
            Some(_) => {}
            None => none += 1,
        }
    }
    (add, change, none)
}

fn check_host(h: &EngineHost, total: &mut (usize, usize, usize)) {
    let m = h.model();
    let t = marks_match(m.world(), m.projection().marks());
    total.0 += t.0;
    total.1 += t.1;
    total.2 += t.2;
}

#[test]
fn journey_marks_match_artifact_rows() {
    let mut total = (0, 0, 0);
    let mut h = ch0_3_journey::page();
    check_host(&h, &mut total);
    for (mv, _, _, _) in ch0_3_journey::journey() {
        mv.apply(&mut h);
        ch0_3_journey::settle(&mut h);
        check_host(&h, &mut total);
    }
    let mut h = ch4_9_journey::page();
    check_host(&h, &mut total);
    for (mv, _, _, _) in ch4_9_journey::journey() {
        mv.apply(&mut h);
        ch4_9_journey::settle(&mut h);
        check_host(&h, &mut total);
    }
    assert!(total.0 > 0 && total.2 > 0, "{total:?}");
}

#[test]
fn read_cell_rules() {
    assert_eq!(path_depth("a"), 0);
    assert_eq!(path_depth("a/b/c"), 2);
    assert_eq!(path_depth(&"/".repeat(300)), 255);
    let c = read_cell("a/b", None);
    assert_eq!(c.label, "a/b");
    assert!(c.detail.is_empty() && c.state.is_empty() && c.tags.is_empty());
}
