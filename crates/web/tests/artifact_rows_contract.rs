//! 行の導出（`artifact_rows`）の規則の試験（TASK-468 1/2）。境界・誤り・指せる行・決定性を固定する。

use std::collections::BTreeMap;

use fandhe_workshop_core::addr::{Generation, Stamp, StepUid};
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::input::Input;
use fandhe_workshop_core::projection::{MarkKind, Projection};
use fandhe_workshop_core::world::{Element, World};
use fandhe_workshop_web::a11y::ChangeMark;
use fandhe_workshop_web::artifact_rows::{derive_rows, DeriveError, RowParts};
use fandhe_workshop_web::artifact_view::{ArtifactViewError, Phase, PointTarget, Row, RowKind};

fn el(fixture: &str, label: &str) -> Element {
    let mut attrs = BTreeMap::new();
    attrs.insert("label".to_owned(), label.to_owned());
    Element {
        fixture: Some(fixture.to_owned()),
        attrs,
    }
}

fn ev(path: &str, before: Option<Element>, after: Option<Element>) -> Event {
    Event::Changed {
        at: Stamp {
            generation: Generation(1),
            step: StepUid("s".to_owned()),
        },
        path: path.to_owned(),
        before,
        after,
        cause: Cause::Input,
    }
}

fn world(entries: &[(&str, Element)]) -> World {
    entries
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect()
}

/// `view` の fixture の要素だけを `label` で読む。
fn reader(
    view: &'static str,
) -> impl Fn(&str, &Element) -> Result<Option<RowParts<String>>, DeriveError> {
    move |_, e| {
        if e.fixture.as_deref() != Some(view) {
            return Ok(None);
        }
        let label = e.attrs.get("label").ok_or(DeriveError::Element)?;
        Ok(Some(RowParts {
            depth: 0,
            kind: RowKind::Item,
            content: label.clone(),
        }))
    }
}

fn mark_of(rows: &[Row<String>], id: &str) -> Option<Option<ChangeMark>> {
    rows.iter()
        .find(|r| r.id().as_str() == id)
        .and_then(|r| r.shown(Phase::After))
        .map(|s| s.mark)
}

#[test]
fn ビューの外の要素は行にならない() {
    let w = world(&[("a", el("w", "x"))]);
    let rows = derive_rows(&w, &[], reader("v")).unwrap();
    assert!(rows.is_empty());
}

#[test]
fn 値を戻した経路は印なし() {
    let w = world(&[("a", el("v", "x"))]);
    let events = [
        ev("a", Some(el("v", "x")), Some(el("v", "y"))),
        ev("a", Some(el("v", "y")), Some(el("v", "x"))),
    ];
    let rows = derive_rows(&w, &events, reader("v")).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(mark_of(&rows, "a"), Some(None));
    // Projection は AttrChanged を出す（導出は前後の差を正とする境界）。
    let marks = Projection::from_events(&events);
    assert!(marks
        .marks()
        .iter()
        .any(|m| m.kind == MarkKind::AttrChanged));
}

#[test]
fn 置いてから消した経路は行にならない() {
    let w = World::new();
    let events = [
        ev("a", None, Some(el("v", "x"))),
        ev("a", Some(el("v", "x")), None),
    ];
    let rows = derive_rows(&w, &events, reader("v")).unwrap();
    assert!(rows.is_empty());
}

#[test]
fn 読まない属性だけの変化は印なし() {
    let mut e1 = el("v", "x");
    e1.attrs.insert("other".to_owned(), "1".to_owned());
    let mut e2 = e1.clone();
    e2.attrs.insert("other".to_owned(), "2".to_owned());
    let w = world(&[("a", e1.clone())]);
    let rows = derive_rows(&w, &[ev("a", Some(e1), Some(e2))], reader("v")).unwrap();
    assert_eq!(mark_of(&rows, "a"), Some(None));
}

#[test]
fn ビューの外へ出た要素は削除で入った要素は追加() {
    let w = world(&[("a", el("v", "x"))]);
    let events = [ev("a", Some(el("v", "x")), Some(el("w", "x")))];
    let in_v = derive_rows(&w, &events, reader("v")).unwrap();
    assert_eq!(mark_of(&in_v, "a"), Some(Some(ChangeMark::Remove)));
    let in_w = derive_rows(&w, &events, reader("w")).unwrap();
    assert_eq!(mark_of(&in_w, "a"), Some(Some(ChangeMark::Add)));
}

#[test]
fn 読み方の誤りはerrで行を落とさない() {
    let mut bad = el("v", "x");
    bad.attrs.clear();
    let w = world(&[("a", el("v", "x")), ("b", bad)]);
    assert_eq!(
        derive_rows(&w, &[], reader("v")).unwrap_err(),
        DeriveError::Element
    );
}

#[test]
fn 空の経路はerr() {
    let w = world(&[("", el("v", "x"))]);
    assert_eq!(
        derive_rows(&w, &[], reader("v")).unwrap_err(),
        DeriveError::Row(ArtifactViewError::EmptyRowId)
    );
}

#[test]
fn 導出した全行が指せて経路が一致する() {
    let w = world(&[("a", el("v", "x")), ("b", el("v", "y"))]);
    let events = [
        ev("b", Some(el("v", "y")), None),
        ev("c", None, Some(el("v", "z"))),
    ];
    let rows = derive_rows(&w, &events, reader("v")).unwrap();
    assert_eq!(rows.len(), 3);
    for r in &rows {
        let t = PointTarget::new("view-1", r.id().clone()).unwrap();
        match t.to_input() {
            Input::Ui { view, path } => {
                assert_eq!(view, "view-1");
                assert_eq!(path, r.id().as_str());
                assert!(w.contains_key(&path) || path == "c");
            }
            _ => panic!("Ui でない"),
        }
    }
    assert_eq!(mark_of(&rows, "b"), Some(Some(ChangeMark::Remove)));
}

#[test]
fn changed以外のイベントは無視する() {
    let w = world(&[("a", el("v", "x"))]);
    let base = [ev("a", Some(el("v", "x")), Some(el("v", "y")))];
    let want = derive_rows(&w, &base, reader("v")).unwrap();
    let mixed = [
        Event::Warning {
            at: Stamp {
                generation: Generation(1),
                step: StepUid("s".to_owned()),
            },
            message: "w".to_owned(),
        },
        base[0].clone(),
    ];
    assert_eq!(derive_rows(&w, &mixed, reader("v")).unwrap(), want);
}

#[test]
fn 同じ入力から同じ結果() {
    let w = world(&[("a", el("v", "x")), ("b", el("v", "y"))]);
    let events = [ev("a", Some(el("v", "x")), Some(el("v", "z")))];
    assert_eq!(
        derive_rows(&w, &events, reader("v")).unwrap(),
        derive_rows(&w, &events, reader("v")).unwrap()
    );
}
