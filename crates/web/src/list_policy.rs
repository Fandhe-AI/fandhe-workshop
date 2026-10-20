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
//! （ART-14・TERM-35・EVT-26 の担当）が試作して決める。ログと復習の畳みは [`plan_fold`] が決めた。
//!
//! 畳み方（TASK-360 1/2。D-46 の案 3）: 表示部品は行を Step などの連続する「グループ」に分け、[`plan_fold`] に
//! グループの長さの列を渡して、1 リストが [`LIST_ITEMS_TARGET`] 項目以内になるよう古い側から畳む。順序は
//! (1) 最新を除く 2 行以上のグループを Step の要約 1 行にする、(2) まだ超えるなら最古のグループから順に 1 つの
//! 集約行へまとめる、(3) それでも超えるなら最新のグループの先頭を 1 行に畳む（最新の行は残る）。計画はグループの
//! 長さの列と目安だけで決まる純粋関数で、更新の履歴を持たない（増分と一括の導出が一致する。EVT-1）。
//! 章の単位のグループは作らず、集約行で代える。要約行は表示だけで、開く操作は未実装である
//! （`docs/known-gaps.md`）。左の成果物の 7 本の list への適用は TASK-484 が持つ。
//!
//! [`keyed_view`] は上限を超えると `Err` をそのまま返す。呼び出し側の復帰は [`keyed_view_or_notice`] に
//! 一本化し、上限を超えた場合は先頭を残して打ち切り、状態の行（「上限超過: 先頭 … 件だけ表示（全 … 件）」。
//! ART-14・TASK-360 2/2）で知らせる。黙って切り詰めない。件数以外の `Err` のときも親要素は `data-bind-list` を保った keyed list
//! （データの行は 0 件）で描く。`data-bind-list` を持たない空の要素に落とすと、上流は dirty な field を束縛点にも
//! keyed list にも解決できず、構造のフォールバックで `app-root` の子を丸ごと置き換える（全再描画。
//! FEND-15・PoC-8。TASK-359 2/2）ためである。
//!
//! 変える場合は、先に spec を直し、`parts::PARTS_CONTRACT_VERSION` を上げる。

use core::fmt;

use fandhe_frontend_core::keyed::{keyed_list, KeyedListError, MAX_KEYED_LIST_ITEMS};
use fandhe_frontend_core::{el, text, Node};

use crate::log_lines::group_digits;
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
    /// 安全弁を超えている。`keyed_view` は拒否し、`keyed_view_or_notice` は先頭を残して打ち切る。
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

/// 打ち切りの印の属性名。値は全件数の 10 進だけ（区切りなし）。親と状態の行の両方に付く。
pub const LIST_LIMIT_ATTR: &str = "data-list-limit";

/// 状態の行のキー。データ行のキーの接頭辞や通知のキーと重ならない。
const LIMIT_KEY: &str = "list-limit";

/// 打ち切ったときに残すデータ行の数。状態の行 1 件の余地を残し、合計を [`LIST_ITEMS_LIMIT`] 項目に収める。
const LIMIT_SHOWN: usize = LIST_ITEMS_LIMIT - 1;

/// 打ち切りの状態の行を作る。文言は固定で、数は 3 桁区切りの 10 進だけを入れる（教材・受講者の値を連結しない。
/// SAFE-11）。`data-event` は付けない（データ行の数え上げと区別するため）。
pub fn limit_node(shown: usize, total: usize) -> Node {
    let n = |v: usize| group_digits(u32::try_from(v).unwrap_or(u32::MAX));
    let total_attr = total.to_string();
    el(
        "li",
        vec![(LIST_LIMIT_ATTR, total_attr.as_str())],
        vec![text(format!(
            "上限超過: 先頭 {} 件だけ表示（全 {} 件）",
            n(shown),
            n(total)
        ))],
    )
}

/// 項目が [`LIST_ITEMS_LIMIT`] を超えるときは先頭を残して打ち切り、[`keyed_view`] の結果を返す。それ以外の
/// `Err` のときは `data-bind-list` を保った「データ行 0 件の keyed list」に落とす。
///
/// 呼び出し元は keyed list を持つ表示部品（ログ・成果物・復習・入力欄の送信済み一覧）。安全弁（D-46・ART-14・
/// TASK-360 2/2）として、上限を超えたら先頭の `LIMIT_SHOWN` 件と状態の行 1 件（[`limit_node`]）を出し、
/// 親と状態の行に [`LIST_LIMIT_ATTR`] を付ける。`Err` にならず、黙って切り詰めることもない。
///
/// 件数以外の `Err`（キーの重複など）は、親に [`LIST_ERROR_ATTR`] の印と固定の通知の行を 1 件だけ出す。
/// 親が keyed list のまま残るため、上流は差分更新として扱い、全再描画にならない（モジュールの doc）。
///
/// 通知の行と印は固定の文字列だけで、教材の値を含めない（SAFE-11）。`keyed_view` さえ拒否する場合
/// （`NotKeyedField`・予約属性の混入。どちらも固定値の誤り）は、最後の手段として空の要素を返す。この経路だけは
/// 全再描画になりうるが、panic はしない。
pub fn keyed_view_or_notice(
    tag: &'static str,
    attrs: Vec<(&str, &str)>,
    field: &PartField,
    mut items: Vec<(String, Node)>,
) -> Node {
    if items.len() > LIST_ITEMS_LIMIT {
        let total = items.len();
        let total_attr = total.to_string();
        items.truncate(LIMIT_SHOWN);
        items.push((LIMIT_KEY.to_owned(), limit_node(LIMIT_SHOWN, total)));
        let mut limited = attrs.clone();
        limited.push((LIST_LIMIT_ATTR, total_attr.as_str()));
        if let Ok(node) = keyed_view(tag, limited, field, items) {
            return node;
        }
    } else if let Ok(node) = keyed_view(tag, attrs.clone(), field, items) {
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

/// 畳みの計画の 1 項目。古い順に並び、出す行の総数は [`plan_fold`] の `target` 以下になる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldEntry {
    /// `group` の行のうち、先頭から `skip` 行を除いた残りをそのまま出す。
    Rows {
        /// グループの位置（古い順）。
        group: usize,
        /// 先頭から除く行数（畳まれた分は別の項目が受け持つ）。
        skip: usize,
    },
    /// `group` の全行を Step の要約 1 行にする。
    Summary {
        /// グループの位置。
        group: usize,
        /// 畳んだ行数。
        rows: usize,
    },
    /// 最古の `groups` 個のグループ（計 `rows` 行）を集約の 1 行にする。
    Older {
        /// 畳んだグループの数。
        groups: usize,
        /// 畳んだ行数。
        rows: usize,
    },
    /// 最新のグループの先頭 `rows` 行を 1 行にする（直後に同じグループの `Rows` が続く）。
    Head {
        /// グループの位置。
        group: usize,
        /// 畳んだ行数。
        rows: usize,
    },
}

/// 畳みの計画を立てる。`group_lens` は古い順のグループの長さ、`target` は 1 リストの項目数の目安
/// （通常は [`LIST_ITEMS_TARGET`]）。呼び出し元は `engine_parts` のログ（Step ごと）と復習（1 項目 1 グループ）。
///
/// 総数が `target` 以下なら畳まない。`target` が 3 未満のときは 3 として扱う。結果の項目が出す行の数
/// （`Rows` の残りと要約・集約・先頭の各 1 行の合計）は、常に `target` 以下になる。
/// 計算は飽和演算で、panic しない。
pub fn plan_fold(group_lens: &[usize], target: usize) -> Vec<FoldEntry> {
    let target = target.max(3);
    let n = group_lens.len();
    let mut total: usize = group_lens.iter().fold(0, |a, l| a.saturating_add(*l));
    if total <= target || n == 0 {
        return (0..n)
            .map(|group| FoldEntry::Rows { group, skip: 0 })
            .collect();
    }
    let latest = n - 1;
    // (1) 最新を除く 2 行以上のグループを、古い順に、収まるまで要約にする。
    let mut summarized_upto = 0;
    for (g, len) in group_lens.iter().enumerate().take(latest) {
        if total <= target {
            break;
        }
        if *len >= 2 {
            total = total.saturating_sub(len - 1);
        }
        summarized_upto = g + 1;
    }
    // (2) まだ超えるなら、最古の k 個（最新を除く）を 1 行へ集約する。k 個で k - 1 行減る。
    let mut aggregated = 0;
    if total > target && latest >= 2 {
        aggregated = (total - target + 1).min(latest);
        total = total.saturating_sub(aggregated - 1);
        summarized_upto = summarized_upto.max(aggregated);
    }
    // (3) それでも超えるなら、最新のグループの先頭 h 行を 1 行へ畳む。h 行で h - 1 行減る。
    let latest_len = group_lens[latest];
    let head = if total > target && latest_len >= 2 {
        (total - target + 1).min(latest_len - 1)
    } else {
        0
    };
    let mut out = Vec::with_capacity(n + 2);
    if aggregated > 0 {
        let rows = group_lens[..aggregated]
            .iter()
            .fold(0usize, |a, l| a.saturating_add(*l));
        out.push(FoldEntry::Older {
            groups: aggregated,
            rows,
        });
    }
    for (group, len) in group_lens.iter().enumerate().skip(aggregated) {
        if group == latest && head > 0 {
            out.push(FoldEntry::Head { group, rows: head });
            out.push(FoldEntry::Rows { group, skip: head });
        } else if group < summarized_upto && *len >= 2 {
            out.push(FoldEntry::Summary { group, rows: *len });
        } else {
            out.push(FoldEntry::Rows { group, skip: 0 });
        }
    }
    out
}

/// 要約行の種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldKind {
    /// 1 つの Step の行を畳んだ要約。
    Step,
    /// 古いグループをまとめた集約。
    Older,
    /// 最新の Step の先頭を畳んだ行。
    Head,
}

impl FoldKind {
    /// `data-fold` の値。
    pub const fn as_str(self) -> &'static str {
        match self {
            FoldKind::Step => "step",
            FoldKind::Older => "older",
            FoldKind::Head => "head",
        }
    }
}

/// 畳みの印の属性名（値は [`FoldKind::as_str`]）。
pub const FOLD_ATTR: &str = "data-fold";
/// 畳んだ行数の属性名。値は 10 進の数だけ。
pub const FOLD_ROWS_ATTR: &str = "data-fold-rows";

/// 要約行の `li` を作る。文言は固定で、数は 3 桁区切りの 10 進だけを入れる（教材・受講者の値を連結しない。SAFE-11）。
/// `step` は検証済みの `StepUid` の文字列で、属性値として既定のエスケープに通る（[`FoldKind::Step`] のときだけ付く）。
/// `review` が真なら復習の項目の文言にする。`data-event` は付けない（データ行の数え上げと区別するため）。
pub fn fold_node(
    kind: FoldKind,
    groups: usize,
    rows: usize,
    step: Option<&str>,
    review: bool,
) -> Node {
    let n = |v: usize| group_digits(u32::try_from(v).unwrap_or(u32::MAX));
    let rows_text = n(rows);
    let word = match (kind, review) {
        (FoldKind::Step, _) => format!("前の Step の {rows_text} 行を畳みました"),
        (FoldKind::Older, false) => {
            format!(
                "それ以前の {} 個の Step の {rows_text} 行を畳みました",
                n(groups)
            )
        }
        (FoldKind::Older, true) => {
            format!("それ以前の {} 件の復習の項目を畳みました", n(groups))
        }
        (FoldKind::Head, _) => format!("この Step の前の {rows_text} 行を畳みました"),
    };
    let rows_attr = rows.to_string();
    let mut attrs = vec![
        (FOLD_ATTR, kind.as_str()),
        (FOLD_ROWS_ATTR, rows_attr.as_str()),
    ];
    if let (FoldKind::Step, Some(s)) = (kind, step) {
        attrs.push(("data-step", s));
    }
    el("li", attrs, vec![text(word)])
}
