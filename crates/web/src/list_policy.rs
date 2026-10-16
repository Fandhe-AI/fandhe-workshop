//! keyed list の親要素の属性の同期と、1 リストの項目数の方針の契約（TASK-343 3/3）。
//!
//! 呼び出し元は、keyed list を持つ表示部品（ログ・復習・選択肢・チェックポイント・候補の一覧。TASK-434 など）で、
//! 上流の `keyed_list` を直接呼ばずに [`keyed_view`] を通す。native の rlib で試験できるよう、wasm 専用にしない。
//!
//! 出所は spec の FEND-15（keyed list の差分更新）・FEND-22（1 リストの項目数）と、決定記録 D-46（TASK-344）である。
//!
//! 親要素の属性の同期（FEND-15）: keyed list の親要素の属性は、field が dirty になるたびに `view()` が返す
//! 属性の列へ同期される。JS で後付けした属性（`aria-live`・class など）は、次の同期で消える。
//! そのため親の属性は必ず [`keyed_view`] の `attrs`（つまり `view()`）に書く。アクセシビリティ契約の
//! 「ライブの欄の属性は `view()` に直接書く」と同じ規則である。
//!
//! 項目数の方針（FEND-22・D-46）: 1 リストの項目は約 [`LIST_ITEMS_TARGET`] 件までを目安とし、超える場合は
//! 部品の側でリストを分割する。上流の [`LIST_ITEMS_LIMIT`] 件は安全弁で、目安ではない。目安の値は基準端末 A の
//! 測定に基づく暫定値（FEND-22 は検討中）である。超えた場合の分割・畳み方の細目は、実装タスク
//! （ART-14・TERM-35・EVT-26 の担当）が試作して決めるため、本契約では設計しない。
//!
//! 上限を超えた場合は `Err` をそのまま返し、空の要素への代替や切り詰めで黙って続けない（空の要素にすると
//! 差分更新ではなく全再描画になる）。
//!
//! 変える場合は、先に spec を直し、`parts::PARTS_CONTRACT_VERSION` を上げる。

use core::fmt;

use fandhe_frontend_core::keyed::{keyed_list, KeyedListError, MAX_KEYED_LIST_ITEMS};
use fandhe_frontend_core::Node;

use crate::parts::{FieldKind, PartField};

/// 1 リストの項目数の目安（D-46）。超えたらリストを分割する。FEND-22 は検討中で、基準端末 A の値である。
pub const LIST_ITEMS_TARGET: usize = 1_000;

/// 1 リストの項目数の安全弁。上流の上限を参照する。目安ではなく、超えると `keyed_list` が拒否する。
pub const LIST_ITEMS_LIMIT: usize = MAX_KEYED_LIST_ITEMS;

/// 項目数の区分。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListSize {
    /// 目安以内。
    Within,
    /// 目安を超えているが安全弁以内。部品の側で分割する。
    OverTarget,
    /// 安全弁を超えている。`keyed_view` は拒否する。
    OverLimit,
}

/// 項目数 `n` の区分。
pub const fn classify_list_len(n: usize) -> ListSize {
    if n <= LIST_ITEMS_TARGET {
        ListSize::Within
    } else if n <= LIST_ITEMS_LIMIT {
        ListSize::OverTarget
    } else {
        ListSize::OverLimit
    }
}

/// [`keyed_view`] の拒否の理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListError {
    /// field の種別が keyed list ではない。
    NotKeyedField,
    /// 上流の `keyed_list` が拒否した（項目数・キー・予約属性など）。
    Upstream(KeyedListError),
}

impl fmt::Display for ListError {
    /// 固定文だけで、キーや field 名の値を埋め込まない（上流の文言も値を含まない）。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ListError::NotKeyedField => f.write_str("field is not a keyed list"),
            ListError::Upstream(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for ListError {}

/// keyed list の親要素を作る。`attrs` は親要素の属性で、`view()` に書く規則に従う。
///
/// # Errors
///
/// `field` が keyed list でない場合は [`ListError::NotKeyedField`]、上流が拒否した場合（項目数が
/// [`LIST_ITEMS_LIMIT`] を超える・キーが空か重複・予約属性の混入など）は [`ListError::Upstream`] を返す。
pub fn keyed_view(
    tag: &'static str,
    attrs: Vec<(&str, &str)>,
    field: &PartField,
    items: Vec<(String, Node)>,
) -> Result<Node, ListError> {
    if field.kind != FieldKind::KeyedList {
        return Err(ListError::NotKeyedField);
    }
    keyed_list(tag, attrs, field.name, items).map_err(ListError::Upstream)
}
