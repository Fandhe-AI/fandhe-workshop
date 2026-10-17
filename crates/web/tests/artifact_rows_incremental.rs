//! 増分で導いた行と一括で導いた行が一致することの試験（TASK-468 2/2。ART-2・ART-3）。
//!
//! 右のログ・復習（`Projection`）と同じイベント列を、1 件ずつ・2 件ずつ・3 件ずつ・全体 1 回の 4 通りで
//! 渡し、全ての接頭辞で `PhaseWorlds` の結果が `derive_rows`・`derive_doc_rows` と一致することを確かめる。
//! `replay_changes` は絶対値を当てるため、二重処理は結果を変えない。検出できるのは「イベントを飛ばす」誤りである。
//! 試験では `unwrap`・`expect` を使ってよい。

use std::collections::BTreeMap;
use std::path::PathBuf;

use fandhe_workshop_core::addr::{Generation, Stamp, StepUid};
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::projection::Projection;
use fandhe_workshop_core::world::{Element, World};
use fandhe_workshop_web::artifact_doc_rows::derive_doc_rows;
use fandhe_workshop_web::artifact_rows::{derive_rows, DeriveError, PhaseWorlds, RowParts};
use fandhe_workshop_web::artifact_view::{Cell, Row, RowKind, ViewKind};
use serde_json::Value;

const NAMES: [(&str, ViewKind); 6] = [
    ("filetree", ViewKind::FileTree),
    ("diff", ViewKind::Diff),
    ("tasks", ViewKind::TaskTree),
    ("cards", ViewKind::CardList),
    ("checks", ViewKind::CheckList),
    ("appdemo", ViewKind::AppDemo),
];

const DOC: &str = "docs/guide.md";

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

fn stopped() -> Event {
    Event::Stopped {
        at: stamp(),
        new_generation: Generation(2),
    }
}

fn sample_json(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/artifact-view-parts/test-data/sample")
        .join(format!("{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn cell_element(view: ViewKind, row: &Value, c: &Value) -> Element {
    let s = |v: &Value| v.as_str().unwrap().to_owned();
    let tags: Vec<String> = c["tags"].as_array().unwrap().iter().map(s).collect();
    let mut attrs = BTreeMap::new();
    attrs.insert("label".to_owned(), s(&c["label"]));
    attrs.insert("detail".to_owned(), s(&c["detail"]));
    attrs.insert("state".to_owned(), s(&c["state"]));
    attrs.insert("tags".to_owned(), tags.join("\n"));
    attrs.insert("depth".to_owned(), row["depth"].to_string());
    attrs.insert("kind".to_owned(), s(&row["kind"]));
    Element {
        fixture: Some(view.as_str().to_owned()),
        attrs,
    }
}

fn attr<'a>(el: &'a Element, key: &str) -> Result<&'a str, DeriveError> {
    el.attrs
        .get(key)
        .map(String::as_str)
        .ok_or(DeriveError::Element)
}

fn read_items(
    view: ViewKind,
) -> impl Fn(&str, &Element) -> Result<Option<RowParts<Cell>>, DeriveError> {
    move |_, el| {
        if el.fixture.as_deref() != Some(view.as_str()) {
            return Ok(None);
        }
        let tags = attr(el, "tags")?;
        Ok(Some(RowParts {
            depth: attr(el, "depth")?
                .parse()
                .map_err(|_| DeriveError::Element)?,
            kind: RowKind::parse(attr(el, "kind")?).ok_or(DeriveError::Element)?,
            content: Cell {
                label: attr(el, "label")?.to_owned(),
                detail: attr(el, "detail")?.to_owned(),
                state: attr(el, "state")?.to_owned(),
                tags: if tags.is_empty() {
                    Vec::new()
                } else {
                    tags.split('\n').map(str::to_owned).collect()
                },
            },
        }))
    }
}

fn doc_element(body: &str) -> Element {
    let mut attrs = BTreeMap::new();
    attrs.insert("body".to_owned(), body.to_owned());
    Element {
        fixture: Some("document".to_owned()),
        attrs,
    }
}

fn read_blocks(el: &Element) -> Result<Vec<String>, DeriveError> {
    Ok(attr(el, "body")?.lines().map(str::to_owned).collect())
}

/// 6 種の sample を同居させた前の世界状態と、1 つの操作のイベント列。
fn sample_world() -> (World, Vec<Event>) {
    let mut world = World::new();
    let mut events = Vec::new();
    for (name, view) in NAMES {
        let json = sample_json(name);
        for row in json["rows"].as_array().unwrap() {
            let id = row["id"].as_str().unwrap();
            let b = (!row["before"].is_null()).then(|| cell_element(view, row, &row["before"]));
            let a = (!row["after"].is_null()).then(|| cell_element(view, row, &row["after"]));
            if let Some(el) = &b {
                world.insert(id.to_owned(), el.clone());
            }
            if b != a {
                events.push(changed(id, b, a));
            }
        }
    }
    (world, events)
}

/// 同じ経路を何度も変える列と、`Changed` 以外を混ぜた列。文書は 3 回に分けて 1 ブロックずつ変える。
fn churn_world() -> (World, Vec<Event>) {
    let item = |label: &str| {
        let c: Value = serde_json::json!({"label": label, "detail": "", "state": "", "tags": []});
        cell_element(
            ViewKind::CardList,
            &serde_json::json!({"depth": 0, "kind": "item"}),
            &c,
        )
    };
    let mut world = World::new();
    world.insert("c1".to_owned(), item("one"));
    world.insert(DOC.to_owned(), doc_element("a\nb\nc"));
    let events = vec![
        changed("c1", Some(item("one")), Some(item("two"))),
        changed(
            DOC,
            Some(doc_element("a\nb\nc")),
            Some(doc_element("a\nB\nc")),
        ),
        stopped(),
        changed("c1", Some(item("two")), Some(item("one"))),
        changed("c2", None, Some(item("new"))),
        changed("c2", Some(item("new")), None),
        changed("c3", None, Some(item("tmp"))),
        changed(
            DOC,
            Some(doc_element("a\nB\nc")),
            Some(doc_element("a\nB\nC")),
        ),
        changed("c1", Some(item("one")), None),
        changed(
            DOC,
            Some(doc_element("a\nB\nC")),
            Some(doc_element("A\nB\nC")),
        ),
        changed("c1", None, Some(item("again"))),
    ];
    (world, events)
}

fn same_rows<T: PartialEq + std::fmt::Debug>(a: &[Row<T>], b: &[Row<T>]) {
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(b) {
        assert_eq!(x.id(), y.id());
        assert_eq!(x.depth(), y.depth());
        assert_eq!(x.kind(), y.kind());
        assert_eq!(x.before(), y.before());
        assert_eq!(x.after(), y.after());
    }
}

/// `step` 件ずつ渡す。各時点の増分の結果を、その接頭辞の一括の結果と比べる。
fn check_split(world: &World, events: &[Event], step: usize) {
    let mut inc = PhaseWorlds::new(world.clone());
    let mut proj = Projection::new();
    let mut k = 0;
    loop {
        k = (k + step).min(events.len());
        let upto = events.get(..k).unwrap();
        inc.extend(upto);
        proj.extend(upto);
        assert_eq!(inc.consumed(), k);
        assert_eq!(proj.consumed(), inc.consumed());
        assert_eq!(proj, Projection::from_events(upto), "step={step} k={k}");
        for (_, view) in NAMES {
            let got = inc.rows(read_items(view)).unwrap();
            let want = derive_rows(world, upto, read_items(view)).unwrap();
            same_rows(&got, &want);
        }
        let got = inc.doc_rows(DOC, read_blocks).unwrap();
        let want = derive_doc_rows(world, upto, DOC, read_blocks).unwrap();
        same_rows(&got, &want);
        // 短い列を渡しても何も変わらない。
        let before_after = inc.after().clone();
        inc.extend(events.get(..k / 2).unwrap());
        assert_eq!(inc.consumed(), k);
        assert_eq!(inc.after(), &before_after);
        if k == events.len() {
            break;
        }
    }
}

#[test]
fn sampleの六種は全接頭辞と四通りの分け方で一括と一致する() {
    let (world, events) = sample_world();
    assert!(!events.is_empty());
    for step in [1, 2, 3, events.len()] {
        check_split(&world, &events, step);
    }
}

#[test]
fn 同じ経路を何度も変える列と他のイベントを混ぜた列でも一致する() {
    let (world, events) = churn_world();
    for step in [1, 2, 3, events.len()] {
        check_split(&world, &events, step);
    }
}

#[test]
fn 文書は一ブロックずつの変更を重ねても一括と一致する() {
    let (world, events) = churn_world();
    let mut inc = PhaseWorlds::new(world.clone());
    inc.extend(&events);
    let rows = inc.doc_rows(DOC, read_blocks).unwrap();
    // a/b/c が A/B/C に変わった: 3 つの `-` と 3 つの `+`。
    let removed = rows.iter().filter(|r| r.after().is_none()).count();
    let added = rows.iter().filter(|r| r.before().is_none()).count();
    assert_eq!((removed, added), (3, 3));
}
