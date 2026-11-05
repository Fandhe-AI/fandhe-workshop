//! 製品の左エリアの行の組み立てと、ビューの種類ごとの差し替え口が共有する部品（TASK-799・D-100）。
//!
//! 呼び出し元は `engine_parts::ArtifactPart::view`（種類で振り分ける `match` を 1 か所だけ持つ）と、
//! 各 `*_view.rs` の `product_items`（種類ごとの差し替え口）である。7 つの差し替え口は今どれも [`path_rows`]
//! を呼び、画面の出力は振り分けを入れる前と 1 バイトも変わらない。種類ごとの結線（ファイルツリー・文書・差分など）は
//! 後続の子が、自分の種類の `product_items` だけを書き換えて行う。
//!
//! # 変えないもの
//!
//! - `li` の `data-path`（製品の行だけに付ける）、キー `p<経路>`・`i<位置>`
//! - `point_all`・`pinned_paths`・`fold_rows`・`keyed_view_or_notice` の経路
//! - `world_list: hidden` で行 0 件、選択のボタン、変種 (b) 固定
//!
//! # 属性の読み方（仮置き）
//!
//! [`read_cell`] は `title`・`detail`・`state` を `Cell` の欄として読む。確定は `docs/views.md` の未決の事項
//! （記録は #2059）で、後続の子が種類ごとに読み替えてよい。ツリーの子が親の属性を `tree_list_attrs` に替えるときは、
//! `tests/view_select.rs` の親の属性列の期待（`list_attrs` との一致）も意図して直す必要がある。
//!
//! # 信頼境界
//!
//! 経路と属性の値は教材由来で信頼できない。運ぶだけで、属性名・クラス名には使わない。画面へは文字ノードと
//! `render` の既定のエスケープだけを通す（SAFE-11）。全域（panic しない）で、誤りの文面に入力値を含めない。

use std::collections::BTreeSet;

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_contract::doc::Doc;
use fandhe_workshop_contract::view_body::body_doc;
use fandhe_workshop_core::world::{Element, World};
use fandhe_workshop_core::Session;

use crate::a11y::{ChangeMark, TESTID_ATTR};
use crate::artifact_fold::ArtifactRow;
use crate::artifact_mark::{point_all, pointed_item};
use crate::artifact_rows::DeriveError;
use crate::artifact_view::{
    check_unique_ids, list_attrs, Cell, ItemRow, Row, RowId, RowKind, Variant, ViewKind,
};

/// 差し替え口へ渡す狭い文脈。`HostModel` は渡さない（`*_view.rs` から `host` を参照させないため）。
pub(crate) struct ProductCtx<'a> {
    /// 今の世界状態。
    pub world: &'a World,
    /// `engine_parts::artifact_rows` の結果。`world_list: hidden` のときは空。
    pub world_rows: &'a [(String, Option<ChangeMark>)],
    /// 選ばれたビューの種類。
    pub selected: ViewKind,
    /// 選ばれた種類の宣言 ID。宣言が無ければ `None`（全行が表示だけになる）。
    pub declared_id: Option<&'a str>,
    /// 本文の引きに要るセッション。
    pub session: Option<&'a Session>,
    /// 今の Step が指す経路（畳まない）。
    pub pinned_paths: &'a BTreeSet<String>,
}

/// 差し替え口が返すもの。`ArtifactPart::view` が畳み（`fold_rows`）にかけて親の一覧へ入れる。
pub(crate) struct ProductItems {
    /// `(経路, キー, 要素, 固定)` の列。
    pub rows: Vec<ArtifactRow>,
    /// 選ばれたビューの親の一覧の属性。
    pub list_attrs: Vec<(&'static str, &'static str)>,
}

/// 経路をそのまま行のキーにしてよい長さの上限（バイト）。
pub(crate) const ARTIFACT_KEY_PATH_MAX: usize = 256;

/// 本文の区画の `data-testid`（ART-18。`shell-` で始めず、`role`・`aria-*`・`style` を付けない）。
pub(crate) const DOC_BODY_TESTID: &str = "artifact-doc-body";

/// 今の Step の `input`・`confirm` の宣言にある `targets` のうち、ビュー `view` を指す経路を集める。
/// 宣言の形（`ui_action`・`point_at`）に依らず `targets` の配列だけを読む。読めない宣言は何も足さない。
pub(crate) fn current_target_paths(session: &Session, view: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Some(uid) = session.state().run.current.as_ref() else {
        return out;
    };
    let Some(node) = session.ir().nodes.iter().find(|n| n.uid == uid.0) else {
        return out;
    };
    for decl in [&node.step.input, &node.step.confirm].into_iter().flatten() {
        let targets = decl.get("targets").and_then(|t| t.as_array());
        for t in targets.into_iter().flatten() {
            if t.get("view").and_then(|v| v.as_str()) == Some(view) {
                if let Some(path) = t.get("path").and_then(|p| p.as_str()) {
                    out.insert(path.to_owned());
                }
            }
        }
    }
    out
}

/// 成果物の行の子。経路の `span` に、本文の木があれば区画を続ける。
///
/// [`path_rows`] から呼ばれる。木の描画は `doc_view`（リンクのスキーム・外部画像・深さの検査を持つ）に任せ、
/// 生 HTML は使わない。
pub(crate) fn row_children(path: &str, body: Option<&Doc>) -> Vec<Node> {
    let mut kids = vec![el("span", vec![], vec![text(path)])];
    if let Some(doc) = body {
        kids.push(el(
            "div",
            vec![
                (TESTID_ATTR, DOC_BODY_TESTID),
                ("class", "artifact-doc-body"),
            ],
            crate::doc_view::render_doc(doc),
        ));
    }
    kids
}

/// 7 種の差し替え口が今共通に返す、経路の行。世界状態の経路 1 つが `li` 1 つで、文書のビューでは本文の区画も続ける。
pub(crate) fn path_rows(ctx: &ProductCtx<'_>) -> ProductItems {
    let selected = ctx.selected;
    let rows: Vec<ArtifactRow> = ctx
        .world_rows
        .iter()
        .enumerate()
        .map(|(i, (path, mark))| {
            // 仮置きの規則（TASK-358 2/2）: 選ばれたビューの宣言 ID が引ける行はすべて指せる行にする。
            // 規則の確定は TASK-470・TASK-347 の後続と TASK-739 の担当で、受理と判定は core に任せる。
            // `PointTarget::new` が拒否する行（空・上限超過）は表示だけに倒す。
            let target = ctx
                .declared_id
                .and_then(|id| point_all(id)(&RowId::new(path.as_str()).ok()?));
            // 文書ビューの本文（ART-18・EP-13）。`body: shown` の宣言と要素の `fixture` と木がそろうときだけ `Some`。
            // 上限の判断（2026-10-24）: 木の大きさ・深さは IR の読み込みの関門と `doc_view` の深さの上限が、行数は
            // 畳み（`fold_rows`）と 4,096 の安全弁が抑えるので、ここでは追加の上限を置かない。
            let body = (selected == ViewKind::Document)
                .then_some(ctx.declared_id)
                .flatten()
                .zip(ctx.session)
                .zip(ctx.world.get(path))
                .and_then(|((id, s), element)| body_doc(s.ir(), id, element.fixture.as_deref()));
            let node = pointed_item(
                "li",
                Variant::B,
                *mark,
                None,
                vec![("data-path", path.as_str())],
                target.as_ref(),
                row_children(path.as_str(), body),
            );
            // 経路は一意（BTreeMap のキー）なので、挿入で後続の行の同一性がずれないよう経路をキーにする。
            // 教材由来の長い文字列はキーに使わず、位置のキーへ退く（接頭辞が違うので衝突しない）。
            let key = if path.len() <= ARTIFACT_KEY_PATH_MAX {
                format!("p{path}")
            } else {
                format!("i{i}")
            };
            let pinned = target.is_some() && ctx.pinned_paths.contains(path.as_str());
            (path.clone(), key, node, pinned)
        })
        .collect();
    ProductItems {
        rows,
        list_attrs: list_attrs(selected, true),
    }
}

/// 属性 `title` を `Cell::label` に読む。
const ATTR_LABEL: &str = "title";
/// 属性 `detail` を `Cell::detail` に読む。
const ATTR_DETAIL: &str = "detail";
/// 属性 `state` を `Cell::state` に読む。
const ATTR_STATE: &str = "state";

/// 要素を `Cell` として読む（仮置きの規則）。`element` が無ければ経路だけの `Cell`。
pub fn read_cell(path: &str, element: Option<&Element>) -> Cell {
    let attr = |name: &str| {
        element
            .and_then(|e| e.attrs.get(name))
            .cloned()
            .unwrap_or_default()
    };
    let label = element
        .and_then(|e| e.attrs.get(ATTR_LABEL))
        .cloned()
        .unwrap_or_else(|| path.to_owned());
    Cell {
        label,
        detail: attr(ATTR_DETAIL),
        state: attr(ATTR_STATE),
        tags: Vec::new(),
    }
}

/// 経路の `/` の数を深さにする。`u8` で飽和する。
pub fn path_depth(path: &str) -> u8 {
    u8::try_from(path.matches('/').count()).unwrap_or(u8::MAX)
}

/// `artifact_rows` の結果（経路の辞書順、印つき）と世界状態から、前後の組の行を作る。
///
/// 印の出所を `world_rows` の 1 つにするため、`shown(Phase::After).mark` は `world_rows` の印と行ごとに必ず一致する。
/// 製品は `Phase::Before` を出さず、前の中身は印を導くためだけに使う。`kind_of` は経路から行の種類を決める
/// （差し替え口が決める）。空の経路など行の検査に通らないものは `Err` で返し、黙って落とさない。
pub fn item_rows(
    world: &World,
    world_rows: &[(String, Option<ChangeMark>)],
    kind_of: impl Fn(&str) -> RowKind,
) -> Result<Vec<ItemRow>, DeriveError> {
    let mut rows = Vec::with_capacity(world_rows.len());
    for (path, mark) in world_rows {
        let after = read_cell(path, world.get(path));
        let (before, after) = match mark {
            Some(ChangeMark::Add) => (None, Some(after)),
            // 無くなった行の前の中身は経路だけ（世界状態から消えているため読めない）。
            Some(ChangeMark::Remove) => (Some(read_cell(path, None)), None),
            Some(ChangeMark::Change) => {
                // 前の値は世界状態から取り出せないため、後と必ず違う値にして `Change` を導く。
                // 読み方は `tags` を常に空にするので、固定の空文字列 1 件を足せば属性の無い要素でも `!=` になる。
                let mut b = after.clone();
                b.tags.push(String::new());
                (Some(b), Some(after))
            }
            _ => (Some(after.clone()), Some(after)),
        };
        rows.push(Row::new(
            RowId::new(path)?,
            path_depth(path),
            kind_of(path),
            before,
            after,
        )?);
    }
    check_unique_ids(&rows)?;
    Ok(rows)
}
