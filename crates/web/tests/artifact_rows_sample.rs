//! sample の 7 種 × 前後で、行の導出の項目数と印の数が期待値と一致する受入の試験（TASK-468 1/2。ART-2・ART-3）。
//!
//! 期待値は導出のコードを通さず、`serde_json::Value` から直接数える。文書だけは JSON が無いため、
//! 手で数えた固定値（前 10・後 13、`+3 -3 ~0`。出所は `doc-before.md`・`doc-after.md` の最上位のブロック）を使う。
//! sample のデータは `tests/` にだけ置き、`src/` には埋め込まない。

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use fandhe_workshop_core::addr::{Generation, Stamp, StepUid};
use fandhe_workshop_core::changed::world_diff;
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::projection::Projection;
use fandhe_workshop_core::world::{Element, World};
use fandhe_workshop_web::a11y::ChangeMark;
use fandhe_workshop_web::artifact_rows::{derive_rows, DeriveError, MarkCounts, RowParts};
use fandhe_workshop_web::artifact_view::{change_mark_of, Cell, Phase, Row, RowKind, ViewKind};
use serde_json::Value;

const NAMES: [(&str, ViewKind); 6] = [
    ("filetree", ViewKind::FileTree),
    ("diff", ViewKind::Diff),
    ("tasks", ViewKind::TaskTree),
    ("cards", ViewKind::CardList),
    ("checks", ViewKind::CheckList),
    ("appdemo", ViewKind::AppDemo),
];

/// 文書のブロックの表（経路, 前, 後）。変更は別の経路に置く（`-` と `+` の突き合わせは 2/2）。
const DOC_BLOCKS: [(&str, Option<&str>, Option<&str>); 13] = [
    ("d01", Some("見出し 1"), Some("見出し 1")),
    ("d02", Some("段落 1"), Some("段落 1")),
    ("d03", Some("見出し 2"), Some("見出し 2")),
    ("d04-old", Some("一覧 1"), None),
    ("d04-new", None, Some("一覧 1 改")),
    ("d05", Some("見出し 3"), Some("見出し 3")),
    ("d06", Some("段落 2"), Some("段落 2")),
    ("d07-old", Some("表 1"), None),
    ("d07-new", None, Some("表 1 改")),
    ("d08", Some("引用 1"), Some("引用 1")),
    ("d09", Some("見出し 4"), Some("見出し 4")),
    ("d10-old", Some("一覧 2"), None),
    ("d10-new", None, Some("一覧 2 改")),
];

fn sample_json(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/artifact-view-parts/test-data/sample")
        .join(format!("{name}.json"));
    let text = std::fs::read_to_string(&path).expect("sample を読めない");
    serde_json::from_str(&text).expect("sample が JSON でない")
}

fn rows_of(v: &Value) -> &Vec<Value> {
    v["rows"].as_array().expect("rows が配列でない")
}

fn cell_element(view: ViewKind, id_kind: (&Value, &Value), c: &Value) -> Element {
    let tags: Vec<&str> = c["tags"]
        .as_array()
        .expect("tags")
        .iter()
        .map(|t| t.as_str().expect("tag"))
        .collect();
    let mut attrs = BTreeMap::new();
    attrs.insert(
        "label".to_owned(),
        c["label"].as_str().expect("label").to_owned(),
    );
    attrs.insert(
        "detail".to_owned(),
        c["detail"].as_str().expect("detail").to_owned(),
    );
    attrs.insert(
        "state".to_owned(),
        c["state"].as_str().expect("state").to_owned(),
    );
    // 試験の中だけの符号化。sample のタグに改行は無い。
    attrs.insert("tags".to_owned(), tags.join("\n"));
    attrs.insert("depth".to_owned(), id_kind.0.to_string());
    attrs.insert(
        "kind".to_owned(),
        id_kind.1.as_str().expect("kind").to_owned(),
    );
    Element {
        fixture: Some(view.as_str().to_owned()),
        attrs,
    }
}

fn doc_element(body: &str) -> Element {
    let mut attrs = BTreeMap::new();
    attrs.insert("body".to_owned(), body.to_owned());
    Element {
        fixture: Some(ViewKind::Document.as_str().to_owned()),
        attrs,
    }
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

/// 7 種を 1 つの世界状態に同居させる。前の世界状態と、1 つの操作のイベント列を返す。
fn world_and_events() -> (World, Vec<Event>) {
    let mut world = World::new();
    let mut events = Vec::new();
    for (name, view) in NAMES {
        let json = sample_json(name);
        for row in rows_of(&json) {
            let id = row["id"].as_str().expect("id");
            let meta = (&row["depth"], &row["kind"]);
            let b = (!row["before"].is_null()).then(|| cell_element(view, meta, &row["before"]));
            let a = (!row["after"].is_null()).then(|| cell_element(view, meta, &row["after"]));
            if let Some(el) = &b {
                world.insert(id.to_owned(), el.clone());
            }
            if b != a {
                events.push(changed(id, b, a));
            }
        }
    }
    for (id, b, a) in DOC_BLOCKS {
        let (b, a) = (b.map(doc_element), a.map(doc_element));
        if let Some(el) = &b {
            world.insert(id.to_owned(), el.clone());
        }
        if b != a {
            events.push(changed(id, b, a));
        }
    }
    (world, events)
}

fn attr<'a>(el: &'a Element, key: &str) -> Result<&'a str, DeriveError> {
    el.attrs
        .get(key)
        .map(String::as_str)
        .ok_or(DeriveError::Element)
}

/// 試験の読み方。`fixture` でビューを選び、sample を写した属性を Cell に戻す。
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

fn read_doc(_: &str, el: &Element) -> Result<Option<RowParts<String>>, DeriveError> {
    if el.fixture.as_deref() != Some(ViewKind::Document.as_str()) {
        return Ok(None);
    }
    Ok(Some(RowParts {
        depth: 0,
        kind: RowKind::Block,
        content: attr(el, "body")?.to_owned(),
    }))
}

/// JSON から直接数えた期待値（前の行数, 後の行数, +, -, ~）。
fn expected_of(json: &Value) -> (usize, usize, usize, usize, usize) {
    let rows = rows_of(json);
    let nb = rows.iter().filter(|r| !r["before"].is_null()).count();
    let na = rows.iter().filter(|r| !r["after"].is_null()).count();
    let add = rows.iter().filter(|r| r["before"].is_null()).count();
    let rem = rows.iter().filter(|r| r["after"].is_null()).count();
    let chg = rows
        .iter()
        .filter(|r| !r["before"].is_null() && !r["after"].is_null() && r["before"] != r["after"])
        .count();
    // 後の状態には削除の行も前の中身で残る。
    (nb, na + rem, add, rem, chg)
}

fn items(view: ViewKind) -> Vec<Row<Cell>> {
    let (world, events) = world_and_events();
    derive_rows(&world, &events, read_items(view)).expect("導出できる")
}

fn doc() -> Vec<Row<String>> {
    let (world, events) = world_and_events();
    derive_rows(&world, &events, read_doc).expect("導出できる")
}

#[test]
fn 六種の前後の項目数と印の数がjsonから数えた期待値と一致する() {
    for (name, view) in NAMES {
        let json = sample_json(name);
        let (nb, na, add, rem, chg) = expected_of(&json);
        let rows = items(view);
        let before = MarkCounts::of(&rows, Phase::Before);
        let after = MarkCounts::of(&rows, Phase::After);
        assert_eq!(before.rows, nb, "{name} 前の項目数");
        assert_eq!(after.rows, na, "{name} 後の項目数");
        assert_eq!(
            (after.add, after.remove, after.change),
            (add, rem, chg),
            "{name} 後の印"
        );
    }
}

#[test]
fn 文書の前後の項目数と印の数が手で数えた値と一致する() {
    let rows = doc();
    let before = MarkCounts::of(&rows, Phase::Before);
    let after = MarkCounts::of(&rows, Phase::After);
    assert_eq!(before.rows, 10);
    assert_eq!(after.rows, 13);
    assert_eq!((after.add, after.remove, after.change), (3, 3, 0));
}

#[test]
fn 手で数えた表とも一致する() {
    // （前, 後の行数, +, -, ~）。後の行数は削除の行を含む。
    let table = [
        ("filetree", ViewKind::FileTree, (10, 11, 1, 1, 2)),
        ("diff", ViewKind::Diff, (7, 9, 2, 1, 3)),
        ("tasks", ViewKind::TaskTree, (5, 6, 1, 1, 3)),
        ("cards", ViewKind::CardList, (3, 4, 1, 1, 2)),
        ("checks", ViewKind::CheckList, (5, 6, 1, 1, 3)),
        ("appdemo", ViewKind::AppDemo, (8, 9, 1, 0, 3)),
    ];
    for (name, view, want) in table {
        let rows = items(view);
        let b = MarkCounts::of(&rows, Phase::Before);
        let a = MarkCounts::of(&rows, Phase::After);
        assert_eq!((b.rows, a.rows, a.add, a.remove, a.change), want, "{name}");
    }
}

#[test]
fn 前の状態の印は七種で零件() {
    let mut all: Vec<MarkCounts> = NAMES
        .iter()
        .map(|&(_, v)| MarkCounts::of(&items(v), Phase::Before))
        .collect();
    all.push(MarkCounts::of(&doc(), Phase::Before));
    assert_eq!(all.len(), 7);
    for c in all {
        assert_eq!((c.add, c.remove, c.change), (0, 0, 0));
    }
}

#[test]
fn 削除の行は後の状態で前の中身と削除の印で残る() {
    for (name, view) in NAMES {
        let json = sample_json(name);
        let rows = items(view);
        for jr in rows_of(&json).iter().filter(|r| r["after"].is_null()) {
            let id = jr["id"].as_str().expect("id");
            let row = rows
                .iter()
                .find(|r| r.id().as_str() == id)
                .expect("行が残る");
            let shown = row.shown(Phase::After).expect("後でも出る");
            assert_eq!(shown.mark, Some(ChangeMark::Remove), "{name} {id}");
            assert_eq!(
                shown.content.label,
                jr["before"]["label"].as_str().expect("label")
            );
        }
    }
    for row in doc().iter().filter(|r| r.id().as_str().ends_with("-old")) {
        let shown = row.shown(Phase::After).expect("後でも出る");
        assert_eq!(shown.mark, Some(ChangeMark::Remove));
        assert_eq!(Some(shown.content), row.before());
    }
}

#[test]
fn 行ごとの印がjsonの行ごとの期待と一致する() {
    for (name, view) in NAMES {
        let json = sample_json(name);
        let rows = items(view);
        for jr in rows_of(&json) {
            let id = jr["id"].as_str().expect("id");
            let want = match (jr["before"].is_null(), jr["after"].is_null()) {
                (true, _) => Some(ChangeMark::Add),
                (_, true) => Some(ChangeMark::Remove),
                _ if jr["before"] != jr["after"] => Some(ChangeMark::Change),
                _ => None,
            };
            let row = rows
                .iter()
                .find(|r| r.id().as_str() == id)
                .expect("行がある");
            assert_eq!(
                row.shown(Phase::After).expect("出る").mark,
                want,
                "{name} {id}"
            );
        }
    }
}

#[test]
fn markkindとの照合と変化の数の一致() {
    let (world, events) = world_and_events();
    let marks = Projection::from_events(&events);
    let mut want: BTreeMap<&str, ChangeMark> = BTreeMap::new();
    for m in marks.marks() {
        want.insert(m.path.as_str(), change_mark_of(m.kind).expect("既知の種類"));
    }
    let mut got: BTreeMap<String, ChangeMark> = BTreeMap::new();
    for (_, view) in NAMES {
        for r in items(view) {
            if let Some(mark) = r.shown(Phase::After).and_then(|s| s.mark) {
                got.insert(r.id().as_str().to_owned(), mark);
            }
        }
    }
    for r in doc() {
        if let Some(mark) = r.shown(Phase::After).and_then(|s| s.mark) {
            got.insert(r.id().as_str().to_owned(), mark);
        }
    }
    let want_owned: BTreeMap<String, ChangeMark> =
        want.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect();
    assert_eq!(got, want_owned);

    let mut after = world.clone();
    fandhe_workshop_core::changed::replay_changes(&mut after, &events);
    assert_eq!(got.len(), world_diff(&world, &after).len());
}

#[test]
fn ビューの外の要素が混ざらず七種で世界状態の経路を覆う() {
    let (world, events) = world_and_events();
    let mut after = world.clone();
    fandhe_workshop_core::changed::replay_changes(&mut after, &events);
    let all: BTreeSet<String> = world.keys().chain(after.keys()).cloned().collect();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut ids: Vec<String> = Vec::new();
    for (_, view) in NAMES {
        ids.extend(items(view).iter().map(|r| r.id().as_str().to_owned()));
    }
    ids.extend(doc().iter().map(|r| r.id().as_str().to_owned()));
    for id in &ids {
        assert!(seen.insert(id.clone()), "ビュー間で重なる: {id}");
    }
    assert_eq!(seen, all);
}
