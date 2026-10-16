//! 左の成果物ビューの契約適合試験（TASK-467 1/2。ART-2・ART-3・ART-5・ART-11）。
//!
//! 型・綴り・印の対応が契約どおりであることを native で確かめる。印の記号・文字・`data-mark` の値は
//! `a11y` を参照して比べ、文字列を写さない。

use fandhe_workshop_core::projection::MarkKind;
use fandhe_workshop_web::a11y::{ChangeMark, MarkValue};
use fandhe_workshop_web::artifact_view::{
    change_mark_of, check_unique_ids, declared_kinds, mark_value, ArtifactViewError, Cell,
    DeclaredView, ItemRow, Phase, Row, RowId, RowKind, ViewKind, ARTIFACT_VIEW_CONTRACT_VERSION,
};

fn rid(s: &str) -> RowId {
    RowId::new(s).unwrap()
}

fn row(id: &str, before: Option<u32>, after: Option<u32>) -> Row<u32> {
    Row::new(rid(id), 0, RowKind::Item, before, after).unwrap()
}

fn cell(label: &str) -> Cell {
    Cell {
        label: label.to_owned(),
        ..Cell::default()
    }
}

#[test]
fn 契約の版は_1() {
    assert_eq!(ARTIFACT_VIEW_CONTRACT_VERSION, 1);
}

#[test]
fn view_kind_の綴りの表() {
    let table = [
        (ViewKind::FileTree, "file_tree"),
        (ViewKind::Document, "document"),
        (ViewKind::Diff, "diff"),
        (ViewKind::TaskTree, "task_tree"),
        (ViewKind::CardList, "card_list"),
        (ViewKind::CheckList, "check_list"),
        (ViewKind::AppDemo, "app_demo"),
    ];
    assert_eq!(ViewKind::ALL.len(), table.len());
    for (k, s) in table {
        assert_eq!(k.as_str(), s);
        assert_eq!(ViewKind::parse(s), Some(k));
    }
}

#[test]
fn view_kind_の全種類は重ならず往復する() {
    for (i, a) in ViewKind::ALL.iter().enumerate() {
        assert_eq!(ViewKind::parse(a.as_str()), Some(*a));
        for b in &ViewKind::ALL[i + 1..] {
            assert_ne!(a, b);
            assert_ne!(a.as_str(), b.as_str());
        }
    }
}

#[test]
fn view_kind_の一覧外は_none() {
    for s in ["File_Tree", "", "files", "FILE_TREE", " diff", "diff "] {
        assert_eq!(ViewKind::parse(s), None, "{s:?}");
    }
}

#[test]
fn row_kind_は往復し一覧外は_none() {
    for (i, a) in RowKind::ALL.iter().enumerate() {
        assert_eq!(RowKind::parse(a.as_str()), Some(*a));
        for b in &RowKind::ALL[i + 1..] {
            assert_ne!(a.as_str(), b.as_str());
        }
    }
    assert_eq!(RowKind::parse("block"), Some(RowKind::Block));
    for s in ["", "Dir", "dirs", "unknown"] {
        assert_eq!(RowKind::parse(s), None, "{s:?}");
    }
}

#[test]
fn declared_kinds_は宣言の順を保ち同じ種類の最初だけ残す() {
    let views = [
        DeclaredView {
            id: "b".into(),
            kind: ViewKind::Diff,
        },
        DeclaredView {
            id: "a".into(),
            kind: ViewKind::FileTree,
        },
        DeclaredView {
            id: "c".into(),
            kind: ViewKind::Diff,
        },
    ];
    assert_eq!(
        declared_kinds(&views),
        vec![(ViewKind::Diff, "b"), (ViewKind::FileTree, "a")]
    );
    assert!(declared_kinds(&[]).is_empty());
}

#[test]
fn row_の不変条件() {
    assert_eq!(RowId::new("").unwrap_err(), ArtifactViewError::EmptyRowId);
    let r: Result<Row<u32>, _> = Row::new(rid("x"), 0, RowKind::Item, None, None);
    assert_eq!(r.unwrap_err(), ArtifactViewError::EmptyRow);
    let ok = row("x", None, Some(1));
    assert_eq!(ok.id().as_str(), "x");
    assert_eq!(ok.depth(), 0);
    assert_eq!(ok.kind(), RowKind::Item);
    assert_eq!(ok.before(), None);
    assert_eq!(ok.after(), Some(&1));
}

#[test]
fn 誤りの文面は固定で入力値を含まない() {
    assert_eq!(ArtifactViewError::EmptyRowId.to_string(), "empty row id");
    assert_eq!(
        ArtifactViewError::EmptyRow.to_string(),
        "row has neither before nor after"
    );
    assert_eq!(
        ArtifactViewError::DuplicateRowId.to_string(),
        "duplicate row id"
    );
}

#[test]
fn check_unique_ids_は重複を検出する() {
    let ok = [row("a", Some(1), None), row("b", None, Some(1))];
    assert_eq!(check_unique_ids(&ok), Ok(()));
    let dup = [row("a", Some(1), None), row("a", None, Some(1))];
    assert_eq!(
        check_unique_ids(&dup),
        Err(ArtifactViewError::DuplicateRowId)
    );
}

#[test]
fn change_mark_of_の対応表() {
    assert_eq!(change_mark_of(MarkKind::Put), Some(ChangeMark::Add));
    assert_eq!(change_mark_of(MarkKind::Removed), Some(ChangeMark::Remove));
    assert_eq!(
        change_mark_of(MarkKind::AttrChanged),
        Some(ChangeMark::Change)
    );
}

#[test]
fn shown_前の状態の印は全行で_none() {
    let rows = [
        row("a", Some(1), Some(2)),
        row("b", Some(1), None),
        row("c", None, Some(1)),
        row("d", Some(1), Some(1)),
    ];
    let shown: Vec<_> = rows.iter().filter_map(|r| r.shown(Phase::Before)).collect();
    // 前に無い行（追加）は前の状態に出さない。
    assert_eq!(shown.len(), 3);
    assert!(shown.iter().all(|s| s.mark.is_none()));
}

#[test]
fn shown_後の状態の追加_削除_変更_不変() {
    let r_add = row("a", None, Some(2));
    let add = r_add.shown(Phase::After).unwrap();
    assert_eq!((*add.content, add.mark), (2, Some(ChangeMark::Add)));

    let r_rm = row("b", Some(1), None);
    let rm = r_rm.shown(Phase::After).unwrap();
    // 削除の行は前の中身で後にも残る。
    assert_eq!((*rm.content, rm.mark), (1, Some(ChangeMark::Remove)));

    let r_ch = row("c", Some(1), Some(2));
    let ch = r_ch.shown(Phase::After).unwrap();
    assert_eq!((*ch.content, ch.mark), (2, Some(ChangeMark::Change)));

    let r_same = row("d", Some(3), Some(3));
    let same = r_same.shown(Phase::After).unwrap();
    assert_eq!((*same.content, same.mark), (3, None));
}

#[test]
fn 後の状態の印の数は前後の組から数えた変化の数と一致する() {
    let pairs: [(Option<u32>, Option<u32>); 10] = [
        (Some(1), Some(1)),
        (Some(1), Some(2)),
        (None, Some(1)),
        (Some(1), None),
        (None, Some(2)),
        (Some(5), Some(5)),
        (Some(5), None),
        (Some(7), Some(8)),
        (Some(9), Some(9)),
        (Some(4), Some(4)),
    ];
    let rows: Vec<Row<u32>> = pairs
        .iter()
        .enumerate()
        .map(|(i, (b, a))| row(&format!("r{i}"), *b, *a))
        .collect();
    let count = |m: ChangeMark| {
        rows.iter()
            .filter_map(|r| r.shown(Phase::After))
            .filter(|s| s.mark == Some(m))
            .count()
    };
    let expect_add = pairs
        .iter()
        .filter(|(b, a)| b.is_none() && a.is_some())
        .count();
    let expect_rm = pairs
        .iter()
        .filter(|(b, a)| b.is_some() && a.is_none())
        .count();
    let expect_ch = pairs
        .iter()
        .filter(|(b, a)| matches!((b, a), (Some(x), Some(y)) if x != y))
        .count();
    assert_eq!(count(ChangeMark::Add), expect_add);
    assert_eq!(count(ChangeMark::Remove), expect_rm);
    assert_eq!(count(ChangeMark::Change), expect_ch);
    assert_eq!((expect_add, expect_rm, expect_ch), (2, 2, 2));
}

#[test]
fn mark_value_は_a11y_の_3_手段と一致する() {
    assert_eq!(mark_value(None), MarkValue::None);
    for m in ChangeMark::ALL {
        let v = mark_value(Some(m));
        assert_eq!(v, MarkValue::Mark(m));
        assert_eq!(v.glyph(), m.glyph());
        assert_eq!(v.text(), m.text());
        assert_eq!(MarkValue::parse(m.data_value()), Some(v));
    }
}

#[test]
fn 文書のブロックの代わりの型でも使える() {
    #[derive(PartialEq, Debug)]
    struct Block(&'static str);
    let r: Row<Block> = Row::new(
        rid("b1"),
        1,
        RowKind::Block,
        Some(Block("x")),
        Some(Block("y")),
    )
    .unwrap();
    assert_eq!(
        r.shown(Phase::After).map(|s| s.mark),
        Some(Some(ChangeMark::Change))
    );
}

#[test]
fn item_row_は_cell_を運ぶ() {
    let r: ItemRow =
        Row::new(rid("f"), 2, RowKind::File, Some(cell("a")), Some(cell("b"))).unwrap();
    assert_eq!(r.depth(), 2);
    assert_eq!(
        r.shown(Phase::After).map(|s| s.mark),
        Some(Some(ChangeMark::Change))
    );
}
