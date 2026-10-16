//! ツリーの位置の計算と、木の属性の組（TASK-472。ART-9・ART-13）。
//!
//! ファイルツリーとタスクツリー（TASK-473・TASK-474）の変種 (b) が、支援技術に木として読ませる属性
//! （`role=treeitem`・`aria-level`・`aria-posinset`・`aria-setsize`・`aria-expanded`）を求めるための共通部品である。
//! 呼び出し元は両ビューの `view()` の行の組み立てで、親の `role=tree` は `artifact_view::tree_view_list` が付ける。
//! 属性は後から付けると更新で消えるため（F-2）、ここで求めた値は `view()` に直接書く。束縛はしない。
//!
//! 出所は spec の ART-9（Must）・ART-13（Should）、試作 PoC-8 の `tree_positions` である。
//! 試作の `HashMap` を添字で引く形は写さず、どの深さの並びでも panic しない全域の関数にした。
//!
//! # 固定するもの
//!
//! - 親は「直前の、より浅い行」、兄弟は「同じ親を持つ行どうし」とする
//! - `aria-level` は祖先の数 + 1 とする。深さが飛ぶ並び（0 の次が 2）でも、親の level + 1 と食い違わない
//! - 子を持つ行（次の行がより深い行）にだけ `aria-expanded="true"` を付ける。全項目を開いた状態で出す
//!   （ART-13）。開閉とキー操作は持たない（ART-22。TASK-494）ので、`false` は契約が禁じる
//! - 前後の切り替え（ART-2）では、その状態で表示される行だけを数える
//!
//! # 信頼境界
//!
//! 属性名は閉じた集合の `&'static str`、値は `usize` を 10 進にした文字列と固定値だけである。
//! 教材由来の文字列（行の識別子・ラベル）は入らない。

use crate::artifact_view::{Phase, Row};

/// ツリーの親要素の `role`。
pub const TREE_ROLE: &str = "tree";
/// ツリーの項目の `role`。
pub const TREEITEM_ROLE: &str = "treeitem";
/// 深さの属性名。
pub const LEVEL_ATTR: &str = "aria-level";
/// 兄弟の中の位置の属性名。
pub const POSINSET_ATTR: &str = "aria-posinset";
/// 兄弟の数の属性名。
pub const SETSIZE_ATTR: &str = "aria-setsize";
/// 開閉の状態の属性名。
pub const EXPANDED_ATTR: &str = "aria-expanded";
/// 開いた状態の値。a11y の版 2 ではこの値だけを許す（ART-13）。
pub const EXPANDED_OPEN: &str = "true";

/// 1 行のツリー上の位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct TreePos {
    /// 祖先の数 + 1。
    pub level: usize,
    /// 兄弟の中での位置（1 始まり）。
    pub posinset: usize,
    /// 兄弟の数。
    pub setsize: usize,
    /// 子を持つか（次の行がより深いか）。
    pub has_children: bool,
}

/// 深さの並び（行の並びと同じ順）から、各行の位置を求める。どの並びでも panic せず、計算量は O(n)。
///
/// 親は直前の、より浅い行である。先頭の行の深さが 0 でなくても level は 1 になる。
pub fn tree_positions(depths: &[u8]) -> Vec<TreePos> {
    let n = depths.len();
    // 親の枠は「親の添字 + 1」（0 は根）。枠ごとに子の数を数える。
    let mut child_count: Vec<usize> = vec![0; n + 1];
    let mut slot_of: Vec<usize> = Vec::with_capacity(n);
    let mut posinset: Vec<usize> = Vec::with_capacity(n);
    let mut level: Vec<usize> = Vec::with_capacity(n);
    // 深さが真に増える行の添字の列。先頭が最も外側の祖先になる。
    let mut stack: Vec<usize> = Vec::new();
    for (i, &d) in depths.iter().enumerate() {
        while let Some(&top) = stack.last() {
            if depths[top] >= d {
                stack.pop();
            } else {
                break;
            }
        }
        let (slot, parent_level) = match stack.last() {
            Some(&p) => (p + 1, level[p]),
            None => (0, 0),
        };
        child_count[slot] += 1;
        slot_of.push(slot);
        posinset.push(child_count[slot]);
        level.push(parent_level + 1);
        stack.push(i);
    }
    (0..n)
        .map(|i| TreePos {
            level: level[i],
            posinset: posinset[i],
            setsize: child_count[slot_of[i]],
            has_children: depths.get(i + 1).is_some_and(|&next| next > depths[i]),
        })
        .collect()
}

/// 1 項目に付ける木の属性。値は数から作った文字列だけを持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct TreeItemAttrs {
    level: String,
    posinset: String,
    setsize: String,
    expanded: bool,
}

impl From<TreePos> for TreeItemAttrs {
    fn from(p: TreePos) -> Self {
        Self {
            level: p.level.to_string(),
            posinset: p.posinset.to_string(),
            setsize: p.setsize.to_string(),
            expanded: p.has_children,
        }
    }
}

impl TreeItemAttrs {
    /// `role`・`aria-level`・`aria-posinset`・`aria-setsize` の順の組。子を持つ行は `aria-expanded="true"` を末尾に足す。
    /// 値は `render` がエスケープする。
    pub fn pairs(&self) -> Vec<(&'static str, &str)> {
        let mut v = vec![
            ("role", TREEITEM_ROLE),
            (LEVEL_ATTR, self.level.as_str()),
            (POSINSET_ATTR, self.posinset.as_str()),
            (SETSIZE_ATTR, self.setsize.as_str()),
        ];
        if self.expanded {
            v.push((EXPANDED_ATTR, EXPANDED_OPEN));
        }
        v
    }
}

/// 行の並びと同じ長さで、その状態の各行の木の属性を返す。その状態で出ない行は `None`。
///
/// 出る行だけを並べ直して数える。後の状態では削除の行も印つきで残るが、前の状態では出ないため、
/// 隠れる行を数えに入れると `aria-setsize` が見た目と食い違う。
pub fn shown_tree_attrs<T: PartialEq>(rows: &[Row<T>], phase: Phase) -> Vec<Option<TreeItemAttrs>> {
    let shown: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.shown(phase).is_some())
        .map(|(i, _)| i)
        .collect();
    let depths: Vec<u8> = shown.iter().map(|&i| rows[i].depth()).collect();
    let mut out: Vec<Option<TreeItemAttrs>> = vec![None; rows.len()];
    for (&i, pos) in shown.iter().zip(tree_positions(&depths)) {
        out[i] = Some(TreeItemAttrs::from(pos));
    }
    out
}
