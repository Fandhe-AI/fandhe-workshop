//! 画面の枠（全体のレイアウト）の区画と、枠が受け取る操作の名前・宛先の形の契約（TASK-511 1/2）。
//!
//! 呼び出し元は、枠を組む画面のタスク（TASK-517・TASK-526・TASK-529〜TASK-533）と画面の試験である。
//! 各担当が識別子や action の名前を別々に決めると統合で食い違うため、先に版つきの型として固定する。
//! native の rlib で試験できるよう、wasm 専用にしない。2/2（通知の種類・検査器）は版を上げずに足す。
//!
//! 出所は spec の SHELL-1（2 領域とタブ）・SHELL-10（次の章へ進む）・SHELL-13（チェックポイント）・
//! SHELL-14（ヒント。D-55）・SHELL-15（書き出しと読み込み）・SHELL-17（履歴を捨てて始める）と、PoC-12 の
//! 観察である。試作の `left-pane`・`right-pane`・`hint-btn` などは本契約の `shell-left`・`shell-right`・
//! `shell-hint` に対応する。試作の `step-title`・`progress` は Step の本文（TASK-525）と右のエリアの持ち分で、
//! 枠の識別子にしない。
//!
//! 識別子の持ち主の境界:
//! - 本契約が持つもの: `shell-` で始まる識別子すべてと、[`ShellOp`] の action の名前。
//! - 参照するだけのもの: `a11y` の `tablist`・tab・tabpanel の id（`a11y::TABLIST_TESTID`・`TAB_TESTID`・
//!   `TABLIST_LABEL`・`PaneTab`）と領域の `aria-label`・通知欄・ライブの欄・印の識別子、`terminal_area` の
//!   識別子。これらは再定義しない。[`PaneMount::pane_tab`] で参照として結ぶ。
//! - 管轄外: 右のエリアの中（TASK-431）、Step の本文・題・「いまやること」（TASK-525）、通知の種類と値（2/2）。
//!
//! 意図して固定しないもの: 検討中の項目（再読み込みの告知・次の行動の文面・古いタブ・ブラウザの案内・
//! 早送りの切り替え・履歴の削除）、予約の区画（[`SectionStatus::Reserved`]）の中身と常設の有無、
//! 「次の章へ進む」の action の名前と宛先（TASK-526）、読み込みのファイルの読み取りの仕組み（TASK-531）、
//! 枠の要素への `role`・`aria-*` の付与（`a11y` の閉じた一覧を変えない）。
//!
//! 契約の版は [`SHELL_CONTRACT_VERSION`]。変える場合は先に spec を直して版を上げる。

use crate::a11y::PaneTab;
use std::fmt;

/// 本契約の版。`parts`・`a11y`・`terminal_area`・`artifact_view` の版とは別に管理する。
pub const SHELL_CONTRACT_VERSION: u32 = 1;

/// 本契約の識別子の予約の接頭辞。`a11y` の `live-`、`terminal_area` の `line-` と同じ考え方で、
/// 他の契約との衝突を機械的に防ぐ。
pub const SHELL_PREFIX: &str = "shell-";

/// 「次の章へ進む」の表示の文字の接頭辞（SHELL-10。`次の章へ進む: <Step の題>`）。
pub const NEXT_CHAPTER_LABEL_PREFIX: &str = "次の章へ進む: ";

/// 「次の章へ進む」の識別子。画面全体で 0 か 1 個（SHELL-10・SHELL-35）。
pub const NEXT_CHAPTER_TESTID: &str = "shell-next-chapter";

/// 「次の章へ進む」の表示の文字を作る。
///
/// 題は教材由来の文字列で、呼び出し側が文字ノードとして描く（生 HTML にしない）。
pub fn next_chapter_label(title: &str) -> String {
    format!("{NEXT_CHAPTER_LABEL_PREFIX}{title}")
}

/// 区画の個数の規則。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    /// 完成した画面でちょうど 1 個。
    ExactlyOne,
    /// 0 か 1 個。
    AtMostOne,
}

/// 区画の確定の度合い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionStatus {
    /// 版 1 で確定した区画。
    Fixed,
    /// 名前と識別子だけを予約した区画。中身の形と常設の有無は固定しない（検討中。決定の後に版 2 が足す）。
    Reserved,
}

/// 上部の区画。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TopSection {
    /// 上部の入れ物。
    Top,
    /// ヒント（SHELL-14・D-55）。
    Hint,
    /// チェックポイント（SHELL-13）。
    Checkpoint,
    /// 教材名（SHELL-8。検討中のため予約）。
    Title,
    /// 章と Step の位置（SHELL-8。検討中のため予約）。
    Position,
    /// 「模擬」の表示（SHELL-6。検討中のため予約）。
    Mock,
}

impl TopSection {
    /// 全種類。
    pub const ALL: [TopSection; 6] = [
        TopSection::Top,
        TopSection::Hint,
        TopSection::Checkpoint,
        TopSection::Title,
        TopSection::Position,
        TopSection::Mock,
    ];

    /// `data-testid` の値。
    pub const fn testid(self) -> &'static str {
        match self {
            TopSection::Top => "shell-top",
            TopSection::Hint => "shell-hint",
            TopSection::Checkpoint => "shell-checkpoint",
            TopSection::Title => "shell-title",
            TopSection::Position => "shell-position",
            TopSection::Mock => "shell-mock",
        }
    }

    /// 識別子から区画を求める。未知は `None`。
    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.testid() == id)
    }

    /// 確定か予約か。
    pub const fn status(self) -> SectionStatus {
        match self {
            TopSection::Top | TopSection::Hint | TopSection::Checkpoint => SectionStatus::Fixed,
            TopSection::Title | TopSection::Position | TopSection::Mock => SectionStatus::Reserved,
        }
    }

    /// 個数の規則。予約の区画は必須にしない。
    pub const fn cardinality(self) -> Cardinality {
        Cardinality::AtMostOne
    }
}

/// 左右の領域の取り付け点。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneMount {
    /// 左（成果物）。
    Left,
    /// 右（操作）。
    Right,
}

impl PaneMount {
    /// 全種類。
    pub const ALL: [PaneMount; 2] = [PaneMount::Left, PaneMount::Right];

    /// `data-testid` の値。
    pub const fn testid(self) -> &'static str {
        match self {
            PaneMount::Left => "shell-left",
            PaneMount::Right => "shell-right",
        }
    }

    /// 識別子から取り付け点を求める。未知は `None`。
    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.testid() == id)
    }

    /// 結ぶ `a11y` のタブ。tab・tabpanel の id と `aria-label` は `a11y` が持ち、ここでは再定義しない。
    pub const fn pane_tab(self) -> PaneTab {
        match self {
            PaneMount::Left => PaneTab::Artifacts,
            PaneMount::Right => PaneTab::Ops,
        }
    }

    /// 個数の規則。完成した画面でちょうど 1 個（SHELL-1）。
    pub const fn cardinality(self) -> Cardinality {
        Cardinality::ExactlyOne
    }
}

/// 本契約の識別子の全件（区画・取り付け点・次の章へ進む・操作の部品）。
pub fn shell_testids() -> impl Iterator<Item = &'static str> {
    TopSection::ALL
        .into_iter()
        .map(TopSection::testid)
        .chain(PaneMount::ALL.into_iter().map(PaneMount::testid))
        .chain(std::iter::once(NEXT_CHAPTER_TESTID))
        .chain(ShellOp::ALL.into_iter().map(ShellOp::testid))
}

/// 本契約が定義した識別子なら真。
pub fn is_defined_testid(id: &str) -> bool {
    shell_testids().any(|t| t == id)
}

/// 書き出しの action の名前（SHELL-15）。
pub const ACTION_EXPORT_HISTORY: &str = "export-history";
/// 読み込みの action の名前（SHELL-15・SHELL-16）。
pub const ACTION_IMPORT_HISTORY: &str = "import-history";
/// 履歴を捨てて始める action の名前（SHELL-17）。
pub const ACTION_DISCARD_AND_START: &str = "discard-and-start";
/// 通知を閉じる action の名前。
pub const ACTION_DISMISS_NOTICE: &str = "dismiss-notice";

/// `dismiss-notice` の payload の最大バイト数。
const MAX_NOTICE_KIND_BYTES: usize = 32;

/// 枠が受け取る操作の名前。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShellOp {
    /// 書き出し。
    Export,
    /// 読み込み。
    Import,
    /// 履歴を捨てて始める。
    DiscardAndStart,
    /// 通知を閉じる。
    DismissNotice,
}

impl ShellOp {
    /// 全種類。
    pub const ALL: [ShellOp; 4] = [
        ShellOp::Export,
        ShellOp::Import,
        ShellOp::DiscardAndStart,
        ShellOp::DismissNotice,
    ];

    /// `dispatch_action` に渡す名前。
    pub const fn as_str(self) -> &'static str {
        match self {
            ShellOp::Export => ACTION_EXPORT_HISTORY,
            ShellOp::Import => ACTION_IMPORT_HISTORY,
            ShellOp::DiscardAndStart => ACTION_DISCARD_AND_START,
            ShellOp::DismissNotice => ACTION_DISMISS_NOTICE,
        }
    }

    /// 名前から種類を求める。未知の名前は `None`。
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|o| o.as_str() == name)
    }

    /// 操作の部品の `data-testid`。置き場（通知の中など）は 2/2 と画面のタスクが持つ。
    pub const fn testid(self) -> &'static str {
        match self {
            ShellOp::Export => "shell-export",
            ShellOp::Import => "shell-import",
            ShellOp::DiscardAndStart => "shell-discard-start",
            ShellOp::DismissNotice => "shell-dismiss",
        }
    }
}

/// 枠が受け取る操作と宛先。
#[derive(Clone, PartialEq, Eq)]
pub enum ShellAction {
    /// 書き出し。
    Export,
    /// 読み込み。本文はファイルの文字列そのもので、未検査の値である。
    Import(String),
    /// 履歴を捨てて始める。
    DiscardAndStart,
    /// 通知を閉じる。値は通知の種類の識別子（`[a-z][a-z0-9-]*`、32 バイト以下）。
    DismissNotice(String),
}

/// 読み込みの本文には秘密情報らしい入力が入りうるため、本文は出さずバイト数だけを出す（SAFE-27）。
impl fmt::Debug for ShellAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShellAction::Export => f.write_str("Export"),
            ShellAction::Import(body) => write!(f, "Import({} bytes)", body.len()),
            ShellAction::DiscardAndStart => f.write_str("DiscardAndStart"),
            ShellAction::DismissNotice(kind) => write!(f, "DismissNotice({kind:?})"),
        }
    }
}

impl ShellAction {
    /// ホストの `Component::decode_action` から呼ぶ復号。
    ///
    /// payload は信頼できない入力として扱い、正規形だけを受理する。未知の名前・不正な payload は `None` で、
    /// 状態も DOM も変えない。panic しない。
    ///
    /// payload の書式:
    /// - `export-history`・`discard-and-start`: 空文字列だけ。
    /// - `import-history`: ファイルの本文そのもの（空を含めて受ける）。空を `None` にすると受講者に理由が
    ///   出ないまま何も起きないため、大きさ・JSON・版の断りは `open_history` に任せる。
    /// - `dismiss-notice`: `[a-z][a-z0-9-]*` の 32 バイト以下。
    pub fn decode(name: &str, payload: &str) -> Option<ShellAction> {
        match ShellOp::from_name(name)? {
            ShellOp::Export => payload.is_empty().then_some(ShellAction::Export),
            ShellOp::Import => Some(ShellAction::Import(payload.to_owned())),
            ShellOp::DiscardAndStart => payload.is_empty().then_some(ShellAction::DiscardAndStart),
            ShellOp::DismissNotice => {
                is_notice_kind(payload).then(|| ShellAction::DismissNotice(payload.to_owned()))
            }
        }
    }

    /// `dispatch_action` に渡す（名前, payload）の組。符号化の正はここだけに置く。
    pub fn encode(&self) -> (&'static str, String) {
        match self {
            ShellAction::Export => (ACTION_EXPORT_HISTORY, String::new()),
            ShellAction::Import(body) => (ACTION_IMPORT_HISTORY, body.clone()),
            ShellAction::DiscardAndStart => (ACTION_DISCARD_AND_START, String::new()),
            ShellAction::DismissNotice(kind) => (ACTION_DISMISS_NOTICE, kind.clone()),
        }
    }
}

/// 通知の種類の識別子の正規形（小文字で始まり、小文字・数字・ハイフン。32 バイト以下）なら真。
fn is_notice_kind(s: &str) -> bool {
    let mut bytes = s.bytes();
    matches!(bytes.next(), Some(b) if b.is_ascii_lowercase())
        && s.len() <= MAX_NOTICE_KIND_BYTES
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
