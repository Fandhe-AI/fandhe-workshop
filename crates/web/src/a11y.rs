//! 画面が守るアクセシビリティの構造の契約（TASK-366、版 2。領域・通知欄・ライブの欄・タブ・変化の印・
//! 操作対象の大きさ・読み上げの更新の単位）。
//!
//! 画面の実装（TASK-517・TASK-450 など）とブラウザ試験（TASK-369〜371・376）は、この契約を入力にする。
//! 呼び出し元は画面の試験で、`view()` が返す [`Node`] の木を [`check_no_undefined`]（部分的な画面）・
//! [`check_complete`]（完成した画面）で照合する。キー処理の実体（JS・glue）は持たず、規則はデータと
//! 純関数（[`tab_key_target`]・[`meets_target_size`]・[`live_response_updates`]）で固定する。
//! native の rlib で試験できるよう、wasm 専用にしない。
//!
//! 出所は spec の `engine-accessibility.md` の A11Y-3・A11Y-5〜8・A11Y-22、`screen-artifact-area.md` の
//! ART-3、試作 PoC-12 の A1〜A3・A7、PoC-1 の F-2 である。
//! ライブの欄の `aria-live`・`aria-atomic` は `view()` に直接書く。後から付けた属性や束縛した属性は更新で
//! 消えるため（F-2）、版 1 では束縛による aria・role・`tabindex` を一切許さない。
//! 値・識別子・規則を変える場合は、先に spec を直し、[`A11Y_CONTRACT_VERSION`] を上げる。黙って変えない。
//!
//! 版 2（TASK-472）は、ART-9・ART-13（ともに spec で確定済み）に従ってツリーの語を足した。`role` の `tree`・
//! `treeitem` と、`aria-level`・`aria-posinset`・`aria-setsize`・`aria-expanded` である。版 1 の規則は変えない。
//!
//! 版 1 が持つ規則は次の 5 つである。
//! 1. 領域の `aria-label`・通知欄・ライブの欄（A11Y-5〜7）
//! 2. tablist と矢印・Home・End のキー（A11Y-5）
//! 3. 変化の印の 3 手段: 記号・文字・`data-mark`（A11Y-22・ART-3。[`ChangeMark`]）
//! 4. 操作対象の最小寸法 24×24 CSS ピクセル（A11Y-3。[`MIN_TARGET_SIZE_CSS_PX`]）
//! 5. 読み上げの更新を数える単位（A11Y-6・A11Y-8。[`live_response_updates`]・[`live_left_should_update`]）
//!
//! 識別子の持ち主の境界:
//! - 本契約が持つ識別子: `live-response`・`live-left`・`view-notice`・`input-notice`・`log-notice`・`save-notice`・`export-notice`・
//!   `log-list`（A11Y-7 が名指しする。TASK-431 は再定義せず参照する）・`pane-tabs`・`pane-tab`、
//!   および id の `pane-tab-ops`・`pane-tab-artifacts`・`pane-ops`・`pane-artifacts`
//! - 変化の印について本契約が持つもの: 属性 `data-mark` とその値、識別子 `mark-glyph`・`mark-text`
//! - 行の種類の名前 `received`・`progress` は TASK-431 が持つ識別子だが、本契約が除外の対象として値を
//!   固定する。TASK-431 が改名するなら本契約の版上げが要る
//! - 値だけを制約するもの: 3 つの領域の `aria-label`。領域の要素の識別子は TASK-511・TASK-431 が持つ
//! - 適合の検査の管轄は、aria・role の属性、tab の `tabindex`、`data-mark`、上記の予約識別子である。
//!   aria・role を持たないそれ以外の `data-testid`（例: `hello-root`）は管轄外とし、他の契約の識別子と
//!   衝突させない。例外として `live-` で始まる識別子は予約空間とし、定義にない値は違反にする
//!
//! 版 2 が足す規則は次の 4 つである（ART-9・ART-13）。
//! 6. `treeitem-attribute`: `role=treeitem` は 3 属性を正の 10 進の整数で持ち、位置は兄弟の数以下
//! 7. `tree-attribute-placement`: ツリーの 4 属性は `role=treeitem` にだけ置く
//! 8. `treeitem-expanded`: `aria-expanded` は `true` だけ。開閉とキー操作は持たない（ART-22。TASK-494）ので、
//!    `false` は項目を隠す宣言になり許さない
//! 9. `treeitem-outside-tree`: `role=treeitem` の祖先に `role=tree` がある
//!
//! 意図して固定しないもの（決める前に固定しない）:
//! - 属性名は HTML に合わせて大文字小文字を区別せずに照合する（`ARIA-LABEL` も `aria-label` と同じ扱い）
//! - tab 以外の要素の静的な `tabindex`（例: `div` の `tabindex="0"`）と、aria・role・`tabindex`・`data-mark`
//!   以外の束縛（`data-bind-class` など）は検査しない
//! - 通知欄 3 つの識別子は試作 PoC-12 の値からの推論である（A11Y-7 は個数 3 だけを確定している）。
//!   TASK-511 が枠の通知欄を 4 つ目として足す場合は、版上げを立てる
//! - `data-mark` の値（`add`・`remove`・`change`、印なしは空文字）は試作の値からの推論である
//!   （ART-3 は記号・文字・`data-mark` の対応だけを確定している）
//! - `aria-describedby`・`aria-hidden`、role の `group`・`list`・`listitem` は別タスクの決定の後に版を
//!   上げて加える。版 1 の閉じた一覧には含めない。記号の要素を読み上げから隠す `aria-hidden` も含まない
//! - 状態表示を「アイコンと文字の両方」で示す規則（A11Y-22 の後半）は木では検査しない。TASK-376・TASK-479
//!   などの範囲である
//! - 操作対象の大きさは木では測らない（[`Node`] にレイアウトが無い）。測定はブラウザ試験（TASK-367）が
//!   行い、判定だけを [`meets_target_size`] が担う
//! - `live-left` の要約の文言（A11Y-15）と、`data-action` の値は固定しない（TASK-343・TASK-431 の範囲）

use core::fmt;
use fandhe_frontend_core::Node;

/// この契約の版。値・識別子・規則を変えたら上げる。
pub const A11Y_CONTRACT_VERSION: u32 = 2;

/// 識別子を載せる属性名。
pub const TESTID_ATTR: &str = "data-testid";

/// 画面の領域。それぞれが `aria-label` を持つ（A11Y-5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    /// 成果物の領域。
    Artifacts,
    /// 案内とログの領域。
    GuideAndLog,
    /// コマンドの入力の領域。
    CommandInput,
}

impl Region {
    /// 全領域。画面の並び順。
    pub const ALL: [Region; 3] = [Region::Artifacts, Region::GuideAndLog, Region::CommandInput];

    /// 領域の `aria-label` の値。
    pub const fn aria_label(self) -> &'static str {
        match self {
            Region::Artifacts => "成果物",
            Region::GuideAndLog => "案内とログ",
            Region::CommandInput => "コマンドの入力",
        }
    }
}

/// ライブの欄。更新を読み上げる欄で、`aria-live`・`aria-atomic` を持つ（A11Y-6）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LiveRegion {
    /// 入力への応答の欄。
    Response,
    /// 残りの手数などの欄。
    Left,
}

impl LiveRegion {
    /// 全ライブの欄。
    pub const ALL: [LiveRegion; 2] = [LiveRegion::Response, LiveRegion::Left];

    /// 識別子（`data-testid`）。
    pub const fn testid(self) -> &'static str {
        match self {
            LiveRegion::Response => "live-response",
            LiveRegion::Left => "live-left",
        }
    }
}

/// ライブの欄が持つ属性。`view()` に直接書く（F-2）。
pub const LIVE_REGION_ATTRS: [(&str, &str); 2] = [("aria-live", "polite"), ("aria-atomic", "true")];

/// 通知欄。`role="status"` を持つ（A11Y-7）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusNotice {
    /// 成果物の表示の通知欄。
    View,
    /// 入力の通知欄。
    Input,
    /// ログの通知欄。
    Log,
}

impl StatusNotice {
    /// 全通知欄。
    pub const ALL: [StatusNotice; 3] = [StatusNotice::View, StatusNotice::Input, StatusNotice::Log];

    /// 識別子（`data-testid`）。
    pub const fn testid(self) -> &'static str {
        match self {
            StatusNotice::View => "view-notice",
            StatusNotice::Input => "input-notice",
            StatusNotice::Log => "log-notice",
        }
    }
}

/// 履歴を保存できなかったことの通知欄の識別子（SAVE-10・TASK-323 1/2）。
///
/// 通知欄（`role="status"`）として扱うが、`StatusNotice::ALL` には入れない。保存の口を持たない画面（右のエリアだけの
/// 描画など）には出ないため、全体の適合では「1 個」でなく「2 個以上でない」だけを求める（任意の通知欄）。
pub const SAVE_NOTICE_TESTID: &str = "save-notice";

/// 履歴の書き出しの案内の通知欄の識別子（SAVE-29・D-57・TASK-340）。`SAVE_NOTICE_TESTID` と同じ扱いの任意の通知欄で、
/// 全体の適合では「2 個以上でない」だけを求める。
pub const EXPORT_NOTICE_TESTID: &str = "export-notice";

/// 古いタブの「保存されていない」通知欄の識別子（SAVE-13・D-56・TASK-325）。`EXPORT_NOTICE_TESTID` と同じ扱いの任意の通知欄。
pub const STALE_NOTICE_TESTID: &str = "stale-notice";

/// ログの一覧の識別子。ライブの欄の外に置き、`aria-live`・`role` を持たせない（A11Y-7）。
pub const LOG_LIST_TESTID: &str = "log-list";

/// tablist の識別子。
pub const TABLIST_TESTID: &str = "pane-tabs";
/// tablist の `aria-label`。
pub const TABLIST_LABEL: &str = "表示する領域";
/// tab の識別子。
pub const TAB_TESTID: &str = "pane-tab";

/// 右エリアのタブ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneTab {
    /// 操作のタブ。
    Ops,
    /// 成果物のタブ。
    Artifacts,
}

impl PaneTab {
    /// 全タブ。並び順がタブの順である。
    pub const ALL: [PaneTab; 2] = [PaneTab::Ops, PaneTab::Artifacts];

    /// 名前（id の部品）。
    pub const fn id(self) -> &'static str {
        match self {
            PaneTab::Ops => "ops",
            PaneTab::Artifacts => "artifacts",
        }
    }

    /// 表示名。
    pub const fn label(self) -> &'static str {
        match self {
            PaneTab::Ops => "操作",
            PaneTab::Artifacts => "成果物",
        }
    }

    /// tab 要素の `id`。
    pub const fn tab_id(self) -> &'static str {
        match self {
            PaneTab::Ops => "pane-tab-ops",
            PaneTab::Artifacts => "pane-tab-artifacts",
        }
    }

    /// tabpanel 要素の `id`。tab の `aria-controls` が指す。
    pub const fn panel_id(self) -> &'static str {
        match self {
            PaneTab::Ops => "pane-ops",
            PaneTab::Artifacts => "pane-artifacts",
        }
    }
}

/// タブを切り替えるキー（A11Y-5）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TabKey {
    /// 左矢印。
    ArrowLeft,
    /// 右矢印。
    ArrowRight,
    /// Home。
    Home,
    /// End。
    End,
}

impl TabKey {
    /// `KeyboardEvent.key` の値から変換する。対象外のキーは `None`。
    pub fn from_key(key: &str) -> Option<TabKey> {
        match key {
            "ArrowLeft" => Some(TabKey::ArrowLeft),
            "ArrowRight" => Some(TabKey::ArrowRight),
            "Home" => Some(TabKey::Home),
            "End" => Some(TabKey::End),
            _ => None,
        }
    }

    /// `KeyboardEvent.key` の値。
    pub const fn as_str(self) -> &'static str {
        match self {
            TabKey::ArrowLeft => "ArrowLeft",
            TabKey::ArrowRight => "ArrowRight",
            TabKey::Home => "Home",
            TabKey::End => "End",
        }
    }
}

/// キー操作の修飾の状態。どれかが真なら、タブの切り替えとして扱わない。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyModifiers {
    /// Alt。
    pub alt: bool,
    /// Ctrl。
    pub ctrl: bool,
    /// Meta。
    pub meta: bool,
    /// 変換中（IME）。
    pub composing: bool,
}

/// キー操作で選ぶタブの添字を返す。選択に合わせてフォーカスも移す（実装側の責務）。
///
/// 右は次へ移り末尾から先頭へ回る。左は前へ移り先頭から末尾へ回る。Home は先頭、End は末尾。
/// 修飾キー・変換中、`len == 0`、`current >= len` では `None` を返す。
pub fn tab_key_target(
    key: TabKey,
    current: usize,
    len: usize,
    mods: KeyModifiers,
) -> Option<usize> {
    if mods.alt || mods.ctrl || mods.meta || mods.composing || len == 0 || current >= len {
        return None;
    }
    Some(match key {
        TabKey::ArrowRight => (current + 1) % len,
        // 加算を避ける（`current + len` は `usize` を超えうる）。
        TabKey::ArrowLeft => {
            if current == 0 {
                len - 1
            } else {
                current - 1
            }
        }
        TabKey::Home => 0,
        TabKey::End => len - 1,
    })
}

/// 変化の印を持たせる要素の属性名（A11Y-22・ART-3）。
pub const MARK_ATTR: &str = "data-mark";
/// 印の記号を出す要素の識別子。`data-mark` を持つ要素の直接の子に 1 つ置く。
pub const MARK_GLYPH_TESTID: &str = "mark-glyph";
/// 印の文字を出す要素の識別子。`data-mark` を持つ要素の直接の子に 1 つ置く。
pub const MARK_TEXT_TESTID: &str = "mark-text";
/// 印のない行の `data-mark` の値。記号・文字も空にする。
pub const MARK_NONE_VALUE: &str = "";

/// 変化の印。色に頼らず、記号・文字・`data-mark` の 3 手段で同じ意味を出す（A11Y-22）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeMark {
    /// 追加された行。
    Add,
    /// 削除された行。
    Remove,
    /// 変更された行。
    Change,
}

impl ChangeMark {
    /// 全種類。
    pub const ALL: [ChangeMark; 3] = [ChangeMark::Add, ChangeMark::Remove, ChangeMark::Change];

    /// `data-mark` の値。
    pub const fn data_value(self) -> &'static str {
        match self {
            ChangeMark::Add => "add",
            ChangeMark::Remove => "remove",
            ChangeMark::Change => "change",
        }
    }

    /// 記号（ART-3）。
    pub const fn glyph(self) -> &'static str {
        match self {
            ChangeMark::Add => "+",
            ChangeMark::Remove => "-",
            ChangeMark::Change => "~",
        }
    }

    /// 文字（ART-3）。
    pub const fn text(self) -> &'static str {
        match self {
            ChangeMark::Add => "【追加】",
            ChangeMark::Remove => "【削除】",
            ChangeMark::Change => "【変更】",
        }
    }
}

/// `data-mark` の値を解釈した結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkValue {
    /// 印のない行（空文字）。
    None,
    /// 印のある行。
    Mark(ChangeMark),
}

impl MarkValue {
    /// `data-mark` の値から変換する。一覧外（大文字小文字違いを含む）は `Option::None`。
    pub fn parse(v: &str) -> Option<MarkValue> {
        if v == MARK_NONE_VALUE {
            return Some(MarkValue::None);
        }
        ChangeMark::ALL
            .iter()
            .find(|m| m.data_value() == v)
            .map(|m| MarkValue::Mark(*m))
    }

    /// 期待する記号。印なしは空。
    pub const fn glyph(self) -> &'static str {
        match self {
            MarkValue::None => "",
            MarkValue::Mark(m) => m.glyph(),
        }
    }

    /// 期待する文字。印なしは空。
    pub const fn text(self) -> &'static str {
        match self {
            MarkValue::None => "",
            MarkValue::Mark(m) => m.text(),
        }
    }
}

/// 操作対象の最小寸法（CSS ピクセル。幅・高さとも。WCAG 2.2 の target-size、A11Y-3）。
pub const MIN_TARGET_SIZE_CSS_PX: u32 = 24;

/// 操作対象の CSS セレクタ。`button`・`select`・`input`（hidden を除く）・指せる行（`data-action` を持つ）。
/// 文中のリンクは WCAG 2.5.8 の例外として対象外である。ブラウザ試験（TASK-367）と、CSS の下限の規則が
/// 同じ集合を指すために持つ。`data-action` の値の命名は TASK-343・TASK-431 の範囲。
pub const TARGET_SIZE_SELECTOR: &str =
    "button, select, input:not([type=\"hidden\"]), [data-action]";

/// 測った寸法が最小寸法を満たすか。幅・高さがともに有限で 24 以上なら真。NaN・無限大・負は偽（fail-closed）。
///
/// 呼び出し元はブラウザ試験で、`getBoundingClientRect` の値を渡す。
pub fn meets_target_size(width_css_px: f64, height_css_px: f64) -> bool {
    let min = f64::from(MIN_TARGET_SIZE_CSS_PX);
    width_css_px.is_finite()
        && height_css_px.is_finite()
        && width_css_px >= min
        && height_css_px >= min
}

/// 木の要素が操作対象（[`TARGET_SIZE_SELECTOR`] の集合）か。`Text`・`RawHtml` は偽。
pub fn is_operation_target(node: &Node) -> bool {
    let Node::Element { tag, attrs, .. } = node else {
        return false;
    };
    if attr(attrs, "data-action").is_some() {
        return true;
    }
    match tag.to_ascii_lowercase().as_str() {
        "button" | "select" => true,
        "input" => !attr(attrs, "type").is_some_and(|t| t.eq_ignore_ascii_case("hidden")),
        _ => false,
    }
}

/// `live-response` の更新に数えない行の種類（A11Y-6）。受領と進捗は応答の要約を作らない。
pub const LIVE_EXCLUDED_LINE_KINDS: [&str; 2] = ["received", "progress"];

/// 行の種類が `live-response` の更新に数えられるか。種類名は大文字小文字を区別する。
pub fn counts_for_live_response(kind: &str) -> bool {
    !LIVE_EXCLUDED_LINE_KINDS.contains(&kind)
}

/// 1 回の応答で増えたログの行の種類の列から、`live-response` の更新回数（0 または 1）を返す。
///
/// 数える行が 1 つ以上あれば、行が複数でも 1 回である（応答ごとにちょうど 1 回、A11Y-6）。除外の行だけ・
/// 空の列では 0 回で、欄を触らない。TASK-370 が観察で使う単位（応答ごとの更新回数）と対応する。
pub fn live_response_updates(kinds: &[&str]) -> usize {
    usize::from(kinds.iter().any(|k| counts_for_live_response(k)))
}

/// Step の中で左の画面が `changed` 件変わったとき、`live-left` を更新すべきか（A11Y-8）。
///
/// 1 件以上なら真。期待は「1 回以上」であり、応答ごとにちょうど 1 回とは限らない（処理中に複数回
/// 更新してよい）。要約の文言（A11Y-15）は固定しない。
pub const fn live_left_should_update(changed: usize) -> bool {
    changed >= 1
}

/// 版 2 で許す aria・role の属性名（閉じた一覧）。版 2 でツリーの 4 つを足した（TASK-472）。
pub const A11Y_ATTRIBUTE_NAMES: [&str; 11] = [
    "role",
    "aria-label",
    "aria-live",
    "aria-atomic",
    "aria-selected",
    "aria-controls",
    "aria-labelledby",
    "aria-level",
    "aria-posinset",
    "aria-setsize",
    "aria-expanded",
];
/// 版 2 で許す `role` の値（閉じた一覧）。版 2 で `tree`・`treeitem` を足した（TASK-472）。
pub const ROLE_VALUES: [&str; 6] = ["status", "tablist", "tab", "tabpanel", "tree", "treeitem"];
/// 版 2 で許す `aria-label` の値（閉じた一覧）。3 つの領域のラベルと tablist のラベル。
pub const ARIA_LABEL_VALUES: [&str; 4] = [
    Region::Artifacts.aria_label(),
    Region::GuideAndLog.aria_label(),
    Region::CommandInput.aria_label(),
    TABLIST_LABEL,
];

/// 適合の違反。位置は根からの子の添字の列（根は空）。文面に利用者の入力本文は含めない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    /// 生 HTML は中身を検査できないため認めない。
    RawHtml {
        /// 位置。
        path: Vec<usize>,
    },
    /// 要素の規則違反。
    Element {
        /// 位置。
        path: Vec<usize>,
        /// 違反した規則の名前。
        rule: &'static str,
        /// 識別子または tag と、属性名など。
        detail: String,
    },
    /// 必須の構造の個数が合わない（`check_complete` のみ）。
    Count {
        /// 数えた対象。
        what: String,
        /// 期待する個数。
        expected: usize,
        /// 見つかった個数。
        found: usize,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Violation::RawHtml { path } => write!(f, "raw html at {path:?} cannot be checked"),
            Violation::Element { path, rule, detail } => {
                write!(f, "{rule} at {path:?}: {detail}")
            }
            Violation::Count {
                what,
                expected,
                found,
            } => write!(f, "expected {expected} of {what}, found {found}"),
        }
    }
}

/// 祖先の文脈。子へ持ち回す。
#[derive(Clone, Copy, Default)]
struct Ctx {
    in_live: bool,
    in_tabpanel: bool,
    in_tablist: bool,
    /// 祖先に `role=tree` がある。
    in_tree: bool,
    /// 直接の親が `data-mark` を持つ。子へ渡すたびに設定し直す（祖先ではなく親だけを表す）。
    parent_is_mark_row: bool,
}

/// `check_complete` が個数を数えるための集計。
#[derive(Default)]
struct Tally {
    labels: Vec<String>,
    live: Vec<String>,
    notice: Vec<String>,
    log_lists: usize,
    tablists: usize,
    tabs: Vec<String>,
    /// role に関係なく、全要素から集めた予約 id（tab・tabpanel の `id`）。
    reserved_ids: Vec<String>,
    selected: usize,
}

/// 属性名は HTML では大文字小文字を区別しないため、`name` との比較は ASCII の大文字小文字を無視する。
fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn walk(view: &Node) -> (Vec<Violation>, Tally) {
    let mut out = Vec::new();
    let mut tally = Tally::default();
    // 再帰しない。深い木でもスタックあふれを起こさないため。子は逆順に積み、文書順に処理する。
    // 位置は親への参照（親の番号と子の添字）の表で持ち、違反を報告するときだけ列に直す。
    // 子ごとに親の列を複製すると、深さ n の鎖で O(n^2) になるため。
    let mut links: Vec<(usize, usize)> = vec![(usize::MAX, 0)];
    let mut stack: Vec<(&Node, usize, Ctx)> = vec![(view, 0, Ctx::default())];
    while let Some((node, id, ctx)) = stack.pop() {
        let (tag, attrs, children) = match node {
            Node::Text(_) => continue,
            Node::RawHtml(_) => {
                out.push(Violation::RawHtml {
                    path: path_of(&links, id),
                });
                continue;
            }
            Node::Element {
                tag,
                attrs,
                children,
            } => (*tag, attrs, children),
        };
        let mut child_ctx = ctx;
        let el = ElementCtx {
            tag,
            attrs,
            children,
            links: &links,
            id,
            ctx,
        };
        check_element(&el, &mut child_ctx, &mut out, &mut tally);
        for (i, c) in children.iter().enumerate().rev() {
            links.push((id, i));
            stack.push((c, links.len() - 1, child_ctx));
        }
    }
    (out, tally)
}

/// 表の `id` の位置を、根からの子の添字の列に直す。
fn path_of(links: &[(usize, usize)], mut id: usize) -> Vec<usize> {
    let mut path = Vec::new();
    while links[id].0 != usize::MAX {
        path.push(links[id].1);
        id = links[id].0;
    }
    path.reverse();
    path
}

/// 1 要素の検査に渡す入力。
struct ElementCtx<'a> {
    tag: &'a str,
    attrs: &'a [(String, String)],
    children: &'a [Node],
    links: &'a [(usize, usize)],
    id: usize,
    ctx: Ctx,
}

fn check_element(
    el: &ElementCtx<'_>,
    child_ctx: &mut Ctx,
    out: &mut Vec<Violation>,
    tally: &mut Tally,
) {
    let attrs = el.attrs;
    let ctx = el.ctx;
    let testid = attr(attrs, TESTID_ATTR);
    let who = testid.unwrap_or(el.tag).to_string();
    let mut bad = |rule: &'static str, detail: String| {
        out.push(Violation::Element {
            path: path_of(el.links, el.id),
            rule,
            detail: format!("{who}: {detail}"),
        });
    };

    for (i, (k, _)) in attrs.iter().enumerate() {
        if attrs[..i].iter().any(|(p, _)| p.eq_ignore_ascii_case(k)) {
            bad("duplicate-attribute", k.clone());
        }
        let lower = k.to_ascii_lowercase();
        if (lower.starts_with("aria-") || lower == "role")
            && !A11Y_ATTRIBUTE_NAMES.contains(&lower.as_str())
        {
            bad("undefined-attribute", k.clone());
        }
    }
    let role = attr(attrs, "role");
    // 予約 id は role に関係なく全要素で検査する（role なしの重複で対応が壊れるのを防ぐ）。
    if let Some(id) = attr(attrs, "id") {
        if let Some(t) = PaneTab::ALL
            .iter()
            .find(|t| t.tab_id() == id || t.panel_id() == id)
        {
            tally.reserved_ids.push(id.to_string());
            let want = if t.tab_id() == id { "tab" } else { "tabpanel" };
            if role != Some(want) {
                bad("reserved-id-role", format!("id {id} requires role {want}"));
            }
        }
    }
    if let Some(r) = role {
        if !ROLE_VALUES.contains(&r) {
            bad("undefined-role", r.to_string());
        }
    }
    let label = attr(attrs, "aria-label");
    if let Some(l) = label {
        if !ARIA_LABEL_VALUES.contains(&l) {
            bad("undefined-aria-label", l.to_string());
        }
        tally.labels.push(l.to_string());
    }
    if let Some(binds) = attr(attrs, "data-bind-attr") {
        for tok in binds.split_whitespace() {
            let name = tok.split(':').next().unwrap_or(tok).to_ascii_lowercase();
            if name.starts_with("aria-") || name == "role" || name == "tabindex" {
                bad("bound-a11y-attribute", tok.to_string());
            }
            // 記号・文字は子の文字で静的に出る。属性だけを束縛すると 3 手段がずれる。
            if name == MARK_ATTR {
                bad("bound-mark", tok.to_string());
            }
        }
    }

    check_mark(el, ctx, testid, &mut bad);
    child_ctx.parent_is_mark_row = attr(attrs, MARK_ATTR).is_some();

    // ライブの欄。
    let is_live = LiveRegion::ALL.iter().any(|l| Some(l.testid()) == testid);
    if is_live {
        tally.live.push(testid.unwrap_or_default().to_string());
        for (k, v) in LIVE_REGION_ATTRS {
            if attr(attrs, k) != Some(v) {
                bad("live-attribute", format!("{k} must be {v}"));
            }
        }
        if role.is_some() {
            bad("live-has-role", "role must be absent".to_string());
        }
        if el.children.iter().any(|c| !matches!(c, Node::Text(_))) {
            bad("live-children", "children must be text only".to_string());
        }
        if ctx.in_tabpanel {
            bad("live-in-tabpanel", "must be outside tabpanel".to_string());
        }
        child_ctx.in_live = true;
    } else {
        if testid.is_some_and(|t| t.starts_with("live-")) {
            bad("undefined-live-id", "reserved prefix".to_string());
        }
        for k in ["aria-live", "aria-atomic"] {
            if attr(attrs, k).is_some() {
                bad("live-attribute-on-non-live", k.to_string());
            }
        }
    }
    if testid == Some(LOG_LIST_TESTID) {
        tally.log_lists += 1;
        if ctx.in_live {
            bad(
                "log-list-in-live",
                "must be outside live region".to_string(),
            );
        }
        if role.is_some() {
            bad("log-list-attribute", "role must be absent".to_string());
        }
    }

    // 通知欄。
    let is_notice = StatusNotice::ALL.iter().any(|n| Some(n.testid()) == testid)
        || testid == Some(SAVE_NOTICE_TESTID)
        || testid == Some(EXPORT_NOTICE_TESTID)
        || testid == Some(STALE_NOTICE_TESTID);
    if is_notice {
        tally.notice.push(testid.unwrap_or_default().to_string());
        if role != Some("status") {
            bad("notice-role", "role must be status".to_string());
        }
    } else if role == Some("status") {
        bad(
            "status-on-non-notice",
            "role=status is reserved".to_string(),
        );
    }

    // tablist・tab・tabpanel。
    if role == Some("tablist") {
        tally.tablists += 1;
        if testid != Some(TABLIST_TESTID) || label != Some(TABLIST_LABEL) {
            bad(
                "tablist-attribute",
                "testid and aria-label required".to_string(),
            );
        }
        child_ctx.in_tablist = true;
    } else if testid == Some(TABLIST_TESTID) {
        bad("tablist-role", "role must be tablist".to_string());
    }
    if role == Some("tab") {
        check_tab(attrs, testid, ctx, tally, &mut bad);
    } else {
        if testid == Some(TAB_TESTID) {
            bad("tab-role", "role must be tab".to_string());
        }
        if attr(attrs, "aria-selected").is_some() || attr(attrs, "aria-controls").is_some() {
            bad(
                "tab-attribute-on-non-tab",
                "aria-selected and aria-controls are for tab".to_string(),
            );
        }
    }
    check_tree(attrs, role, ctx, child_ctx, &mut bad);
    if role == Some("tabpanel") {
        let id = attr(attrs, "id").unwrap_or("");
        match PaneTab::ALL.iter().find(|t| t.panel_id() == id) {
            None => bad("tabpanel-id", "undefined id".to_string()),
            Some(t) => {
                if attr(attrs, "aria-labelledby") != Some(t.tab_id()) {
                    bad(
                        "tabpanel-labelledby",
                        format!("aria-labelledby must be {}", t.tab_id()),
                    );
                }
            }
        }
        child_ctx.in_tabpanel = true;
    } else if attr(attrs, "aria-labelledby").is_some() {
        bad(
            "tab-attribute-on-non-tabpanel",
            "aria-labelledby is for tabpanel".to_string(),
        );
    }
}

/// ツリー（ART-9・ART-13）の属性と配置を照合する。値は違反の文面に載せない。
///
/// `role=treeitem` は 3 属性（正の 10 進の整数。`aria-posinset` は `aria-setsize` 以下）を持ち、祖先に
/// `role=tree` がある。`aria-expanded` は開閉の口（ART-22）が無い版 2 では `true` だけを許す。
/// ツリーの 4 属性は `role=treeitem` にだけ置く。
fn check_tree(
    attrs: &[(String, String)],
    role: Option<&str>,
    ctx: Ctx,
    child_ctx: &mut Ctx,
    bad: &mut impl FnMut(&'static str, String),
) {
    if role == Some("tree") {
        child_ctx.in_tree = true;
    }
    let tree_attrs = [
        "aria-level",
        "aria-posinset",
        "aria-setsize",
        "aria-expanded",
    ];
    if role != Some("treeitem") {
        for k in tree_attrs {
            if attr(attrs, k).is_some() {
                bad("tree-attribute-placement", format!("{k} is for treeitem"));
            }
        }
        return;
    }
    if !ctx.in_tree {
        bad("treeitem-outside-tree", "must be inside tree".to_string());
    }
    let mut nums: [Option<u64>; 3] = [None; 3];
    for (slot, k) in nums.iter_mut().zip(&tree_attrs[..3]) {
        match attr(attrs, k).and_then(positive_decimal) {
            Some(n) => *slot = Some(n),
            None => bad(
                "treeitem-attribute",
                format!("{k} must be a positive integer"),
            ),
        }
    }
    if let [_, Some(pos), Some(size)] = nums {
        if pos > size {
            bad(
                "treeitem-attribute",
                "aria-posinset must not exceed aria-setsize".to_string(),
            );
        }
    }
    if attr(attrs, "aria-expanded").is_some_and(|v| v != "true") {
        bad(
            "treeitem-expanded",
            "aria-expanded must be true (all items are open)".to_string(),
        );
    }
}

/// 先頭が 0 でない ASCII 数字だけの正の 10 進の整数。範囲を超える桁数は `None`。
fn positive_decimal(s: &str) -> Option<u64> {
    if s.is_empty() || s.starts_with('0') || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// 変化の印の 3 手段（A11Y-22）を照合する。値と本文は違反の文面に載せない。
/// 子の探索は直接の子を 1 回なめるだけで、全体で線形のまま。
fn check_mark(
    el: &ElementCtx<'_>,
    ctx: Ctx,
    testid: Option<&str>,
    bad: &mut impl FnMut(&'static str, String),
) {
    if matches!(testid, Some(MARK_GLYPH_TESTID | MARK_TEXT_TESTID)) {
        if !ctx.parent_is_mark_row {
            bad(
                "mark-part-outside-row",
                "must be a direct child of data-mark".to_string(),
            );
        }
        if el.children.iter().any(|c| !matches!(c, Node::Text(_))) {
            bad(
                "mark-part-children",
                "children must be text only".to_string(),
            );
        }
    }
    let Some(value) = attr(el.attrs, MARK_ATTR) else {
        return;
    };
    let Some(expect) = MarkValue::parse(value) else {
        bad("undefined-mark-value", "data-mark".to_string());
        return;
    };
    for (id, want, rule) in [
        (MARK_GLYPH_TESTID, expect.glyph(), "mark-glyph"),
        (MARK_TEXT_TESTID, expect.text(), "mark-text"),
    ] {
        let mut parts = el.children.iter().filter_map(|c| match c {
            Node::Element {
                attrs, children, ..
            } if attr(attrs, TESTID_ATTR) == Some(id) => Some(children),
            _ => None,
        });
        let (first, second) = (parts.next(), parts.next());
        match (first, second) {
            (Some(children), None) => {
                let got: String = children
                    .iter()
                    .filter_map(|c| match c {
                        Node::Text(t) => Some(t.as_str()),
                        _ => None,
                    })
                    .collect();
                if got != want {
                    bad(rule, format!("{id} must match data-mark"));
                }
            }
            _ => bad(rule, format!("exactly one {id} child required")),
        }
    }
}

fn check_tab(
    attrs: &[(String, String)],
    testid: Option<&str>,
    ctx: Ctx,
    tally: &mut Tally,
    bad: &mut impl FnMut(&'static str, String),
) {
    let id = attr(attrs, "id").unwrap_or("");
    tally.tabs.push(id.to_string());
    if testid != Some(TAB_TESTID) {
        bad("tab-testid", "testid must be pane-tab".to_string());
    }
    match PaneTab::ALL.iter().find(|t| t.tab_id() == id) {
        None => bad("tab-id", "undefined id".to_string()),
        Some(t) => {
            if attr(attrs, "aria-controls") != Some(t.panel_id()) {
                bad(
                    "tab-controls",
                    format!("aria-controls must be {}", t.panel_id()),
                );
            }
        }
    }
    let want_tabindex = match attr(attrs, "aria-selected") {
        Some("true") => {
            tally.selected += 1;
            Some("0")
        }
        Some("false") => Some("-1"),
        _ => None,
    };
    match want_tabindex {
        None => bad(
            "tab-selected",
            "aria-selected must be true or false".to_string(),
        ),
        Some(w) if attr(attrs, "tabindex") != Some(w) => {
            bad("tab-tabindex", format!("tabindex must be {w}"));
        }
        Some(_) => {}
    }
    if !ctx.in_tablist {
        bad("tab-outside-tablist", "must be inside tablist".to_string());
    }
}

/// 定義にない識別子・属性が無いことを確かめる。部分的な画面に使う。違反はすべて集めて返す。
///
/// 規則: `RawHtml` の禁止、属性名の重複、aria・role の一覧外、`aria-label` の一覧外、束縛による
/// aria・role・`tabindex` の禁止、ライブの欄・通知欄・`log-list`・tablist・tab・tabpanel の属性と配置。
pub fn check_no_undefined(view: &Node) -> Result<(), Vec<Violation>> {
    let (out, _) = walk(view);
    if out.is_empty() {
        Ok(())
    } else {
        Err(out)
    }
}

/// [`check_no_undefined`] に加え、必須の構造がちょうど所定の数だけあることを確かめる。完成した画面に使う。
pub fn check_complete(view: &Node) -> Result<(), Vec<Violation>> {
    let (mut out, t) = walk(view);
    let mut count = |what: String, expected: usize, found: usize| {
        if expected != found {
            out.push(Violation::Count {
                what,
                expected,
                found,
            });
        }
    };
    let n = |v: &[String], s: &str| v.iter().filter(|x| x.as_str() == s).count();
    for r in Region::ALL {
        count(
            format!("region {}", r.aria_label()),
            1,
            n(&t.labels, r.aria_label()),
        );
    }
    for l in LiveRegion::ALL {
        count(
            format!("live region {}", l.testid()),
            1,
            n(&t.live, l.testid()),
        );
    }
    for s in StatusNotice::ALL {
        count(
            format!("notice {}", s.testid()),
            1,
            n(&t.notice, s.testid()),
        );
    }
    let saves = n(&t.notice, SAVE_NOTICE_TESTID);
    count(format!("notice {SAVE_NOTICE_TESTID}"), saves.min(1), saves);
    let exports = n(&t.notice, EXPORT_NOTICE_TESTID);
    count(
        format!("notice {EXPORT_NOTICE_TESTID}"),
        exports.min(1),
        exports,
    );
    let stales = n(&t.notice, STALE_NOTICE_TESTID);
    count(
        format!("notice {STALE_NOTICE_TESTID}"),
        stales.min(1),
        stales,
    );
    count("log-list".to_string(), 1, t.log_lists);
    count("tablist".to_string(), 1, t.tablists);
    for p in PaneTab::ALL {
        count(
            format!("tab {}", p.tab_id()),
            1,
            n(&t.reserved_ids, p.tab_id()),
        );
        count(
            format!("tabpanel {}", p.panel_id()),
            1,
            n(&t.reserved_ids, p.panel_id()),
        );
    }
    count("selected tab".to_string(), 1, t.selected);
    let in_order = t
        .tabs
        .iter()
        .map(String::as_str)
        .eq(PaneTab::ALL.iter().map(|p| p.tab_id()));
    if !in_order && t.tabs.len() == PaneTab::ALL.len() {
        out.push(Violation::Count {
            what: "tabs in order".to_string(),
            expected: 1,
            found: 0,
        });
    }
    if out.is_empty() {
        Ok(())
    } else {
        Err(out)
    }
}
