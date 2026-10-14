//! 画面が守るアクセシビリティの構造の契約（TASK-366、版 1 のうち領域・通知欄・ライブの欄・タブ）。
//!
//! 画面の実装（TASK-517・TASK-450 など）とブラウザ試験（TASK-369〜371・376）は、この契約を入力にする。
//! 呼び出し元は画面の試験で、`view()` が返す [`Node`] の木を [`check_no_undefined`]（部分的な画面）・
//! [`check_complete`]（完成した画面）で照合する。キー処理の実体（JS・glue）は持たず、規則はデータと
//! 純関数（[`tab_key_target`]）で固定する。native の rlib で試験できるよう、wasm 専用にしない。
//!
//! 出所は spec の `engine-accessibility.md` の A11Y-5〜7 と、試作 PoC-12 の A1〜A3・PoC-1 の F-2 である。
//! ライブの欄の `aria-live`・`aria-atomic` は `view()` に直接書く。後から付けた属性や束縛した属性は更新で
//! 消えるため（F-2）、版 1 では束縛による aria・role・`tabindex` を一切許さない。
//! 値・識別子・規則を変える場合は、先に spec を直し、[`A11Y_CONTRACT_VERSION`] を上げる。黙って変えない。
//!
//! 版 1 は 2 段で埋める。本段が (1) 領域の `aria-label`・通知欄・ライブの欄、(2) tablist と矢印キー、
//! 後段（#315）が (3) 変化の印の 3 手段、(4) 操作対象の最小寸法、(5) 読み上げの更新を数える単位を、
//! 版を上げずに加える。
//!
//! 識別子の持ち主の境界:
//! - 本契約が持つ識別子: `live-response`・`live-left`・`view-notice`・`input-notice`・`log-notice`・
//!   `log-list`（A11Y-7 が名指しする。TASK-431 は再定義せず参照する）・`pane-tabs`・`pane-tab`、
//!   および id の `pane-tab-ops`・`pane-tab-artifacts`・`pane-ops`・`pane-artifacts`
//! - 値だけを制約するもの: 3 つの領域の `aria-label`。領域の要素の識別子は TASK-511・TASK-431 が持つ
//! - 適合の検査の管轄は、aria・role の属性、tab の `tabindex`、上記の予約識別子である。aria・role を
//!   持たないそれ以外の `data-testid`（例: `hello-root`）は管轄外とし、他の契約の識別子と衝突させない。
//!   例外として `live-` で始まる識別子は予約空間とし、定義にない値は違反にする
//!
//! 意図して固定しないもの（決める前に固定しない）:
//! - 通知欄 3 つの識別子は試作 PoC-12 の値からの推論である（A11Y-7 は個数 3 だけを確定している）。
//!   TASK-511 が枠の通知欄を 4 つ目として足す場合は、版上げを立てる
//! - `aria-describedby`・`aria-hidden`、role の `group`・`list`・`listitem` は別タスクの決定の後に版を
//!   上げて加える。版 1 の閉じた一覧には含めない
//! - `data-action`・`data-payload` など action の配線は TASK-343・TASK-431 の範囲

use core::fmt;
use fandhe_frontend_core::Node;

/// この契約の版。値・識別子・規則を変えたら上げる。
pub const A11Y_CONTRACT_VERSION: u32 = 1;

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
        TabKey::ArrowLeft => (current + len - 1) % len,
        TabKey::Home => 0,
        TabKey::End => len - 1,
    })
}

/// 版 1 で許す aria・role の属性名（閉じた一覧）。
pub const A11Y_ATTRIBUTE_NAMES: [&str; 7] = [
    "role",
    "aria-label",
    "aria-live",
    "aria-atomic",
    "aria-selected",
    "aria-controls",
    "aria-labelledby",
];
/// 版 1 で許す `role` の値（閉じた一覧）。
pub const ROLE_VALUES: [&str; 4] = ["status", "tablist", "tab", "tabpanel"];
/// 版 1 で許す `aria-label` の値（閉じた一覧）。3 つの領域のラベルと tablist のラベル。
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
}

/// `check_complete` が個数を数えるための集計。
#[derive(Default)]
struct Tally {
    labels: Vec<String>,
    live: Vec<String>,
    notice: Vec<String>,
    tablists: usize,
    tabs: Vec<String>,
    panels: Vec<String>,
    selected: usize,
}

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

fn walk(view: &Node) -> (Vec<Violation>, Tally) {
    let mut out = Vec::new();
    let mut tally = Tally::default();
    // 再帰しない。深い木でもスタックあふれを起こさないため。子は逆順に積み、文書順に処理する。
    let mut stack: Vec<(&Node, Vec<usize>, Ctx)> = vec![(view, Vec::new(), Ctx::default())];
    while let Some((node, path, ctx)) = stack.pop() {
        let (tag, attrs, children) = match node {
            Node::Text(_) => continue,
            Node::RawHtml(_) => {
                out.push(Violation::RawHtml { path });
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
            path: &path,
            ctx,
        };
        check_element(&el, &mut child_ctx, &mut out, &mut tally);
        for (i, c) in children.iter().enumerate().rev() {
            let mut p = path.clone();
            p.push(i);
            stack.push((c, p, child_ctx));
        }
    }
    (out, tally)
}

/// 1 要素の検査に渡す入力。
struct ElementCtx<'a> {
    tag: &'a str,
    attrs: &'a [(String, String)],
    children: &'a [Node],
    path: &'a [usize],
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
            path: el.path.to_vec(),
            rule,
            detail: format!("{who}: {detail}"),
        });
    };

    for (i, (k, _)) in attrs.iter().enumerate() {
        if attrs[..i].iter().any(|(p, _)| p == k) {
            bad("duplicate-attribute", k.clone());
        }
        if (k.starts_with("aria-") || k == "role") && !A11Y_ATTRIBUTE_NAMES.contains(&k.as_str()) {
            bad("undefined-attribute", k.clone());
        }
    }
    let role = attr(attrs, "role");
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
            let name = tok.split(':').next().unwrap_or(tok);
            if name.starts_with("aria-") || name == "role" || name == "tabindex" {
                bad("bound-a11y-attribute", tok.to_string());
            }
        }
    }

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
    let is_notice = StatusNotice::ALL.iter().any(|n| Some(n.testid()) == testid);
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
    if role == Some("tabpanel") {
        let id = attr(attrs, "id").unwrap_or("");
        tally.panels.push(id.to_string());
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
    count("tablist".to_string(), 1, t.tablists);
    for p in PaneTab::ALL {
        count(format!("tab {}", p.tab_id()), 1, n(&t.tabs, p.tab_id()));
        count(
            format!("tabpanel {}", p.panel_id()),
            1,
            n(&t.panels, p.panel_id()),
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
