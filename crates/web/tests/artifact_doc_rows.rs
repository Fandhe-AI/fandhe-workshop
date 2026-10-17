//! 文書のブロックの突き合わせの受入と規則の試験（TASK-468 2/2。ART-2・ART-3）。
//!
//! 実データ（`doc-before.md`・`doc-after.md`）を `markdown::parse` でブロックにし、期待値は導出を通さずに
//! パース結果から数える（集合の包含）。`Projection` は経路単位で `~`、文書の行はブロック単位で `-`・`+` になる。
//! 試験では `unwrap`・`expect` を使ってよい。

use std::collections::BTreeMap;
use std::path::PathBuf;

use fandhe_workshop_contract::doc::Block;
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid};
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::projection::Projection;
use fandhe_workshop_core::world::{Element, World};
use fandhe_workshop_pack::markdown::parse;
use fandhe_workshop_web::a11y::ChangeMark;
use fandhe_workshop_web::artifact_doc_rows::{align_blocks, derive_doc_rows, MAX_ALIGN_CELLS};
use fandhe_workshop_web::artifact_rows::{DeriveError, MarkCounts};
use fandhe_workshop_web::artifact_view::{ArtifactViewError, DocRow, Phase, RowKind};
use fandhe_workshop_web::list_policy::LIST_ITEMS_LIMIT;

const PATH: &str = "notes/readme.md";

fn md(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/artifact-view-parts/test-data/sample")
        .join(name);
    std::fs::read_to_string(p).unwrap()
}

fn element(body: &str) -> Element {
    let mut attrs = BTreeMap::new();
    attrs.insert("body".to_owned(), body.to_owned());
    Element {
        fixture: Some("document".to_owned()),
        attrs,
    }
}

fn read_md(el: &Element) -> Result<Vec<Block>, DeriveError> {
    let body = el.attrs.get("body").ok_or(DeriveError::Element)?;
    Ok(parse(body).blocks)
}

fn read_words(el: &Element) -> Result<Vec<String>, DeriveError> {
    let body = el.attrs.get("body").ok_or(DeriveError::Element)?;
    Ok(body.split_whitespace().map(str::to_owned).collect())
}

fn stamp() -> Stamp {
    Stamp {
        generation: Generation(1),
        step: StepUid("s".to_owned()),
    }
}

fn changed(path: &str, before: Option<Element>, after: Option<Element>) -> Event {
    Event::Changed {
        at: stamp(),
        path: path.to_owned(),
        before,
        after,
        cause: Cause::Input,
    }
}

fn world_with(body: Option<&str>) -> World {
    let mut w = World::new();
    if let Some(b) = body {
        w.insert(PATH.to_owned(), element(b));
    }
    w
}

fn words(before: &str, after: &str) -> Vec<DocRow<String>> {
    let w = world_with(Some(before));
    let ev = [changed(PATH, Some(element(before)), Some(element(after)))];
    derive_doc_rows(&w, &ev, PATH, read_words).unwrap()
}

fn marks(rows: &[DocRow<String>]) -> String {
    rows.iter()
        .map(|r| match (r.before(), r.after()) {
            (Some(b), Some(a)) if b == a => format!(" ={b}"),
            (Some(b), None) => format!(" -{b}"),
            (None, Some(a)) => format!(" +{a}"),
            _ => " ~".to_owned(),
        })
        .collect()
}

#[test]
fn 実データの前後の行数と印が数えた値と一致する() {
    let (bm, am) = (md("doc-before.md"), md("doc-after.md"));
    let (b, a) = (parse(&bm).blocks, parse(&am).blocks);
    for (name, blocks) in [("前", &b), ("後", &a)] {
        for (i, x) in blocks.iter().enumerate() {
            assert!(!blocks[..i].contains(x), "{name} に重複したブロックがある");
        }
    }
    let same = b.iter().filter(|x| a.contains(x)).count();
    let (add, rem) = (a.len() - same, b.len() - same);
    assert_eq!((add, rem), (3, 3), "目で見た変更は 3 件");

    let w = world_with(Some(&bm));
    let ev = [changed(PATH, Some(element(&bm)), Some(element(&am)))];
    let rows = derive_doc_rows(&w, &ev, PATH, read_md).unwrap();
    let (cb, ca) = (
        MarkCounts::of(&rows, Phase::Before),
        MarkCounts::of(&rows, Phase::After),
    );
    assert_eq!((cb.rows, cb.add, cb.remove, cb.change), (b.len(), 0, 0, 0));
    assert_eq!(ca.rows, same + add + rem);
    assert_eq!((ca.add, ca.remove, ca.change), (add, rem, 0));
    assert_eq!(same + add, a.len());
    assert_eq!(same + rem, b.len());

    let shown_before: Vec<&Block> = rows
        .iter()
        .filter_map(|r| r.shown(Phase::Before).map(|s| s.content))
        .collect();
    assert_eq!(shown_before, b.iter().collect::<Vec<_>>());
    let shown_after: Vec<&Block> = rows
        .iter()
        .filter_map(|r| r.shown(Phase::After))
        .filter(|s| s.mark != Some(ChangeMark::Remove))
        .map(|s| s.content)
        .collect();
    assert_eq!(shown_after, a.iter().collect::<Vec<_>>());

    // Projection は経路単位の 1 件、文書の行はブロック単位の `-`・`+`。印の有無は一致する。
    let p = Projection::from_events(&ev);
    assert!(p.marks().iter().any(|m| m.path == PATH));
    assert!(ca.add + ca.remove > 0);
}

#[test]
fn 空と同一と全追加と全削除() {
    assert!(align_blocks::<String>("d", &[], &[]).unwrap().is_empty());
    let r = words("a b c", "a b c");
    assert_eq!(marks(&r), " =a =b =c");
    let w = World::new();
    let ev = [changed(PATH, None, Some(element("x y")))];
    let r = derive_doc_rows(&w, &ev, PATH, read_words).unwrap();
    assert_eq!(marks(&r), " +x +y");
    let w = world_with(Some("x y"));
    let ev = [changed(PATH, Some(element("x y")), None)];
    let r = derive_doc_rows(&w, &ev, PATH, read_words).unwrap();
    assert_eq!(marks(&r), " -x -y");
}

#[test]
fn 変更は削除が先の組で波ダッシュは無い() {
    assert_eq!(marks(&words("a b c", "a X c")), " =a -b +X =c");
    assert_eq!(marks(&words("a b c d", "b d e")), " -a =b -c =d +e");
}

#[test]
fn 重複を含む列でも不変条件が成り立つ() {
    let cases = [
        ("a a b a", "a b a a"),
        ("a a a", "a a"),
        ("x a a y", "a a a a"),
        ("a b a b", "b a b a"),
    ];
    for (b, a) in cases {
        let rows = words(b, a);
        let (nb, na) = (b.split(' ').count(), a.split(' ').count());
        let c = MarkCounts::of(&rows, Phase::After);
        let kept = rows.len() - c.add - c.remove;
        assert_eq!(c.change, 0, "{b} -> {a}");
        assert_eq!(kept + c.add, na, "{b} -> {a}");
        assert_eq!(kept + c.remove, nb, "{b} -> {a}");
        assert_eq!(MarkCounts::of(&rows, Phase::Before).rows, nb);
    }
}

#[test]
fn idは位置で一意かつ決定的で行の形が固定() {
    let r = words("a b c", "a X c");
    let ids: Vec<&str> = r.iter().map(|x| x.id().as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "notes/readme.md#0",
            "notes/readme.md#1",
            "notes/readme.md#2",
            "notes/readme.md#3"
        ]
    );
    assert!(r
        .iter()
        .all(|x| x.kind() == RowKind::Block && x.depth() == 0));
    let again = words("a b c", "a X c");
    let ids2: Vec<&str> = again.iter().map(|x| x.id().as_str()).collect();
    assert_eq!(ids, ids2);
}

#[test]
fn 空の経路と読み取りの誤りと上限と他のイベント() {
    assert_eq!(
        align_blocks("", &["a".to_owned()], &[]).unwrap_err(),
        DeriveError::Row(ArtifactViewError::EmptyRowId)
    );
    let w = world_with(Some("a"));
    let bad = |_: &Element| -> Result<Vec<String>, DeriveError> { Err(DeriveError::Element) };
    assert_eq!(
        derive_doc_rows(&w, &[], PATH, bad).unwrap_err(),
        DeriveError::Element
    );

    let big: Vec<u32> = (0..=LIST_ITEMS_LIMIT as u32).collect();
    assert_eq!(
        align_blocks("d", &big, &[]).unwrap_err(),
        DeriveError::TooLarge
    );
    // 両側が上限以内でも、中央の表が大きすぎれば拒否する（先頭・末尾が共通でない列）。
    let side = (MAX_ALIGN_CELLS as f64).sqrt() as u32 + 2;
    let left: Vec<u32> = (0..side).collect();
    let right: Vec<u32> = (side..2 * side).collect();
    assert_eq!(
        align_blocks("d", &left, &right).unwrap_err(),
        DeriveError::TooLarge
    );

    // Changed 以外は無視する。
    let w = world_with(Some("a b"));
    let other = Event::Stopped {
        at: stamp(),
        new_generation: Generation(2),
    };
    let rows = derive_doc_rows(&w, &[other], PATH, read_words).unwrap();
    assert_eq!(marks(&rows), " =a =b");
}
