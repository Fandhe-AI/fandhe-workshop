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
//! 上流（`fandhe-frontend-wasm-client` 0.6.1）の挙動: `keyed_dom.rs` の keyed list の適用は、そのたびに親の
//! 属性を同期し（`sync_parent_attrs`・`sync_attrs`）、`view()` の列に無い属性を予約属性 `data-bind-list` を
//! 除いてすべて消す。同期はその field が dirty になった更新でだけ起きる。また、宣言済みの属性の値を JS が
//! 書き換えた場合、キャッシュと新しい値が同じなら、その回の `set_attribute` は省かれ、値が戻るのは次の適用になる。
//!
//! [`keyed_view`] は上流の `keyed_list` を呼べる唯一の入口である。`tests/list_parent_attrs.rs` が、
//! `src/` で `keyed_list` を呼ぶのがこのファイルだけであることを検査する。
//!
//! 項目数の方針（FEND-22・D-46）: 1 リストの項目は約 [`LIST_ITEMS_TARGET`] 件までを目安とし、超える場合は
//! 部品の側でリストを分割する。上流の [`LIST_ITEMS_LIMIT`] 件は安全弁で、目安ではない。目安の値は基準端末 A の
//! 測定に基づく暫定値（FEND-22 は検討中）である。超えた場合の分割・畳み方の細目は、実装タスク
//! （ART-14・TERM-35・EVT-26 の担当）が試作して決めるため、本契約では設計しない。
//!
//! 上限を超えた場合は [`keyed_view`] が `Err` をそのまま返し、切り詰めで黙って続けない。呼び出し側の復帰は
//! [`keyed_view_or_notice`] に一本化する。`Err` のときも親要素は `data-bind-list` を保った keyed list
//! （データの行は 0 件）で描く。`data-bind-list` を持たない空の要素に落とすと、上流は dirty な field を束縛点にも
//! keyed list にも解決できず、構造のフォールバックで `app-root` の子を丸ごと置き換える（全再描画。
//! FEND-15・PoC-8。TASK-359 2/2）ためである。
//!
//! 変える場合は、先に spec を直し、`parts::PARTS_CONTRACT_VERSION` を上げる。

use core::fmt;

use fandhe_frontend_core::keyed::{keyed_list, KeyedListError, MAX_KEYED_LIST_ITEMS};
use fandhe_frontend_core::{el, text, Node};

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

/// 一覧を描けなかったときの印の属性名。値は固定の `"limit"` だけで、教材の値を入れない。
pub const LIST_ERROR_ATTR: &str = "data-list-error";

/// 通知の行のキー。データ行のキーの接頭辞（`r`・`p`・`i`・`v`・`e`）と重ならない。
const NOTICE_KEY: &str = "list-error";

/// [`keyed_view`] の結果を返し、`Err` のときは `data-bind-list` を保った「データ行 0 件の keyed list」に落とす。
///
/// 呼び出し元は keyed list を持つ表示部品（ログ・成果物・復習・入力欄の送信済み一覧）。`Err` を握りつぶすのでは
/// なく、親に [`LIST_ERROR_ATTR`] の印と固定の通知の行を 1 件だけ出し、利用者にも試験にも分かる形にする。
/// 親が keyed list のまま残るため、上流は差分更新として扱い、全再描画にならない（モジュールの doc）。
///
/// 通知の行と印は固定の文字列だけで、教材の値を含めない（SAFE-11）。`keyed_view` さえ拒否する場合
/// （`NotKeyedField`・予約属性の混入。どちらも固定値の誤り）は、最後の手段として空の要素を返す。この経路だけは
/// 全再描画になりうるが、panic はしない。
pub fn keyed_view_or_notice(
    tag: &'static str,
    attrs: Vec<(&str, &str)>,
    field: &PartField,
    items: Vec<(String, Node)>,
) -> Node {
    if let Ok(node) = keyed_view(tag, attrs.clone(), field, items) {
        return node;
    }
    let mut marked = attrs;
    marked.push((LIST_ERROR_ATTR, "limit"));
    let notice = vec![(
        NOTICE_KEY.to_owned(),
        el(
            "li",
            vec![(LIST_ERROR_ATTR, "limit")],
            vec![text("一覧を表示できません")],
        ),
    )];
    keyed_view(tag, marked.clone(), field, notice).unwrap_or_else(|_| el(tag, marked, vec![]))
}
