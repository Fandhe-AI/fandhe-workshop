//! 画面の枠（全体のレイアウト）の区画と、枠が受け取る操作の名前・宛先の形、通知の口の契約（TASK-511 1/2・2/2）。
//!
//! 呼び出し元は、枠を組む画面のタスク（TASK-517・TASK-526・TASK-529〜TASK-533）と画面の試験である。
//! 各担当が識別子や action の名前を別々に決めると統合で食い違うため、先に版つきの型として固定する。
//! native の rlib で試験できるよう、wasm 専用にしない。2/2（通知の種類・値・検査器）は版を上げずに足した。
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
//! - 管轄外: 右のエリアの中（TASK-431）、Step の本文・題・「いまやること」（TASK-525）、通知の文面そのもの（TASK-516・TASK-534・
//!   TASK-545・TASK-549）と通知欄の描画・配置（画面のタスク）。
//! - 通知の口（2/2）: 通知の種類 5 つ（SHELL-16・17・18・20・31）、値 4 つ（種類・理由コード・文面の識別子・
//!   押せる操作）、通知欄の識別子 `shell-notice`（`role="status"` の規則と個数は `a11y` が課す）、適合の検査器
//!   [`check_no_undefined`]・[`check_complete`]。理由コードは版 1 では不透明な文字列で、TASK-310 の列挙には
//!   結ばない（結ぶのは TASK-516）。
//!
//! 意図して固定しないもの: 検討中の項目（再読み込みの告知・次の行動の文面・古いタブ・ブラウザの案内・
//! 早送りの切り替え・履歴の削除）、予約の区画（[`SectionStatus::Reserved`]）の中身と常設の有無、
//! 「次の章へ進む」の action の名前と宛先（TASK-526）、読み込みのファイルの読み取りの仕組み（TASK-531）、
//! 枠の要素への `role`・`aria-*` の付与（`a11y` の閉じた一覧を変えない）。
//!
//! 契約の版は [`SHELL_CONTRACT_VERSION`]。変える場合は先に spec を直して版を上げる。

use crate::a11y::{PaneTab, TESTID_ATTR};
use crate::artifact_view::{ACTION_ATTR, PAYLOAD_ATTR};
use fandhe_frontend_core::Node;
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

/// 通知欄の入れ物の識別子。`role="status"` を持つ（`a11y` の通知欄の規則に従う）。画面全体で 0 か 1 個。
pub const NOTICE_AREA_TESTID: &str = "shell-notice";
/// 通知 1 件の識別子。`role` を持たない。通知欄の中に、種類ごとに 0 か 1 件。
pub const NOTICE_ITEM_TESTID: &str = "shell-notice-item";
/// 通知の文面の部品の識別子。子は文字ノードだけ。通知ごとに 0 か 1 個。
pub const NOTICE_TEXT_TESTID: &str = "shell-notice-text";
/// 通知の種類（[`NoticeKind::as_str`]）を載せる属性名。
pub const NOTICE_KIND_ATTR: &str = "data-notice-kind";
/// 通知の理由コードを載せる属性名。
pub const NOTICE_REASON_ATTR: &str = "data-notice-reason";
/// 通知の文面の識別子（[`MessageId::as_str`]）を載せる属性名。
pub const NOTICE_MESSAGE_ATTR: &str = "data-notice-message";

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
        .chain([
            NEXT_CHAPTER_TESTID,
            NOTICE_AREA_TESTID,
            NOTICE_ITEM_TESTID,
            NOTICE_TEXT_TESTID,
        ])
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

/// 通知の種類。出所は SHELL-16（読み込みの拒否）・SHELL-17（壊れた履歴）・SHELL-18（保存の失敗）・
/// SHELL-20（版の違いを変換して開いた）・SHELL-31（教材を読み込めない）。
///
/// 検討中の項目（次の行動の文面 SHELL-36・SHELL-19・履歴の削除・再読み込みの告知）は種類に含めない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoticeKind {
    /// 保存の失敗（SHELL-18）。保存に成功すると消えるため、閉じる操作を持たない。
    SaveFailed,
    /// 起動時に見つかった壊れた履歴（SHELL-17）。
    CorruptHistory,
    /// 読み込みの拒否（SHELL-16）。
    ImportRefused,
    /// 版の違いを変換して開いた（SHELL-20）。
    VersionConverted,
    /// 教材を読み込めない（SHELL-31）。
    PackUnloadable,
}

/// 理由コードの要否。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasonRule {
    /// 必須。
    Required,
    /// 任意。
    Optional,
}

impl NoticeKind {
    /// 全種類。
    pub const ALL: [NoticeKind; 5] = [
        NoticeKind::SaveFailed,
        NoticeKind::CorruptHistory,
        NoticeKind::ImportRefused,
        NoticeKind::VersionConverted,
        NoticeKind::PackUnloadable,
    ];

    /// 属性と `dismiss-notice` の payload に使う値（`[a-z][a-z0-9-]*`、32 バイト以下）。
    pub const fn as_str(self) -> &'static str {
        match self {
            NoticeKind::SaveFailed => "save-failed",
            NoticeKind::CorruptHistory => "corrupt-history",
            NoticeKind::ImportRefused => "import-refused",
            NoticeKind::VersionConverted => "version-converted",
            NoticeKind::PackUnloadable => "pack-unloadable",
        }
    }

    /// 値から種類を求める。未知は `None`。
    ///
    /// `ShellAction::decode` の `dismiss-notice` は正規形だけを見て種類に結ばない。種類への結びはホストの
    /// 側（ここ）で行う。
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }

    /// 理由コードの要否。
    pub const fn reason_rule(self) -> ReasonRule {
        match self {
            NoticeKind::VersionConverted => ReasonRule::Optional,
            _ => ReasonRule::Required,
        }
    }

    /// 通知の中に置ける操作の閉じた表。`Export`・`Import` の置き場は枠の操作で、通知の中には置かない。
    pub const fn allowed_ops(self) -> &'static [ShellOp] {
        match self {
            NoticeKind::SaveFailed | NoticeKind::PackUnloadable => &[],
            NoticeKind::CorruptHistory => &[ShellOp::DiscardAndStart],
            NoticeKind::ImportRefused | NoticeKind::VersionConverted => &[ShellOp::DismissNotice],
        }
    }

    /// この種類で許す文面の識別子（版 1 は 1 つ）。
    pub const fn messages(self) -> &'static [MessageId] {
        match self {
            NoticeKind::SaveFailed => &[MessageId::SaveFailed],
            NoticeKind::CorruptHistory => &[MessageId::CorruptHistory],
            NoticeKind::ImportRefused => &[MessageId::ImportRefused],
            NoticeKind::VersionConverted => &[MessageId::VersionConverted],
            NoticeKind::PackUnloadable => &[MessageId::PackUnloadable],
        }
    }
}

/// 通知の文面の識別子。文面そのものは固定しない（持ち主は TASK-516・TASK-534・TASK-545・TASK-549）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MessageId {
    /// 保存の失敗。
    SaveFailed,
    /// 壊れた履歴。
    CorruptHistory,
    /// 読み込みの拒否。
    ImportRefused,
    /// 版の違いを変換して開いた。
    VersionConverted,
    /// 教材を読み込めない。
    PackUnloadable,
}

impl MessageId {
    /// 全種類。
    pub const ALL: [MessageId; 5] = [
        MessageId::SaveFailed,
        MessageId::CorruptHistory,
        MessageId::ImportRefused,
        MessageId::VersionConverted,
        MessageId::PackUnloadable,
    ];

    /// 属性に載せる値。
    pub const fn as_str(self) -> &'static str {
        match self {
            MessageId::SaveFailed => "msg-save-failed",
            MessageId::CorruptHistory => "msg-corrupt-history",
            MessageId::ImportRefused => "msg-import-refused",
            MessageId::VersionConverted => "msg-version-converted",
            MessageId::PackUnloadable => "msg-pack-unloadable",
        }
    }

    /// 値から求める。未知は `None`。
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.as_str() == s)
    }
}

/// 理由コードの最大バイト数。
const MAX_REASON_BYTES: usize = 64;

/// 通知の理由コード。版 1 では意味を解釈しない不透明な文字列で、属性に安全な形（ASCII の英字で始まり、
/// 英数字・`_`・`-` が続く 1〜64 バイト）だけを受ける。TASK-310 の列挙に結ぶのは TASK-516。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReasonCode(String);

impl ReasonCode {
    /// 正規形だけを受ける。それ以外は `None`。
    pub fn parse(s: &str) -> Option<ReasonCode> {
        let mut bytes = s.bytes();
        let ok = matches!(bytes.next(), Some(b) if b.is_ascii_alphabetic())
            && s.len() <= MAX_REASON_BYTES
            && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
        ok.then(|| ReasonCode(s.to_owned()))
    }

    /// 値。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// [`Notice::new`] の失敗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeError {
    /// 理由が必須の種類に理由が無い。
    ReasonRequired,
    /// 理由コードが正規形でない。
    ReasonFormat,
    /// その種類で許されない文面の識別子。
    MessageNotAllowed(MessageId),
    /// その種類で許されない操作。
    OpNotAllowed(ShellOp),
    /// 同じ操作が 2 つ以上ある。
    DuplicateOp(ShellOp),
}

/// 通知の値。種類・理由コード・文面の識別子・押せる操作の列の 4 つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    kind: NoticeKind,
    reason: Option<ReasonCode>,
    message: MessageId,
    ops: Vec<ShellOp>,
}

impl Notice {
    /// 規則を満たす通知だけを作る。panic しない。
    pub fn new(
        kind: NoticeKind,
        reason: Option<&str>,
        message: MessageId,
        ops: &[ShellOp],
    ) -> Result<Notice, NoticeError> {
        let reason = match reason {
            Some(r) => Some(ReasonCode::parse(r).ok_or(NoticeError::ReasonFormat)?),
            None if kind.reason_rule() == ReasonRule::Required => {
                return Err(NoticeError::ReasonRequired)
            }
            None => None,
        };
        if !kind.messages().contains(&message) {
            return Err(NoticeError::MessageNotAllowed(message));
        }
        for (i, op) in ops.iter().enumerate() {
            if !kind.allowed_ops().contains(op) {
                return Err(NoticeError::OpNotAllowed(*op));
            }
            if ops[..i].contains(op) {
                return Err(NoticeError::DuplicateOp(*op));
            }
        }
        Ok(Notice {
            kind,
            reason,
            message,
            ops: ops.to_vec(),
        })
    }

    /// 種類。
    pub fn kind(&self) -> NoticeKind {
        self.kind
    }

    /// 理由コード。
    pub fn reason(&self) -> Option<&ReasonCode> {
        self.reason.as_ref()
    }

    /// 文面の識別子。
    pub fn message(&self) -> MessageId {
        self.message
    }

    /// 押せる操作。
    pub fn ops(&self) -> &[ShellOp] {
        &self.ops
    }

    /// 通知 1 件の要素に付ける属性の組（識別子・種類・理由・文面の識別子）。
    pub fn attrs(&self) -> Vec<(&'static str, String)> {
        let mut v = vec![
            (TESTID_ATTR, NOTICE_ITEM_TESTID.to_owned()),
            (NOTICE_KIND_ATTR, self.kind.as_str().to_owned()),
        ];
        if let Some(r) = &self.reason {
            v.push((NOTICE_REASON_ATTR, r.as_str().to_owned()));
        }
        v.push((NOTICE_MESSAGE_ATTR, self.message.as_str().to_owned()));
        v
    }

    /// 操作のボタンに載せる宛先を、`ops` の順に返す。`DismissNotice` の値は自分の種類。
    pub fn actions(&self) -> Vec<ShellAction> {
        self.ops
            .iter()
            .map(|op| match op {
                ShellOp::Export => ShellAction::Export,
                ShellOp::Import => ShellAction::Import(String::new()),
                ShellOp::DiscardAndStart => ShellAction::DiscardAndStart,
                ShellOp::DismissNotice => ShellAction::DismissNotice(self.kind.as_str().to_owned()),
            })
            .collect()
    }
}

/// 適合の違反。位置は根からの子の添字の列（根は空）。文面に文字ノードの中身は含めない。
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
        /// 識別子・属性名・属性値など（文字ノードは含めない）。
        detail: String,
    },
    /// 個数の違反（`check_complete` の fail-closed）。
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
            Violation::Element { path, rule, detail } => write!(f, "{rule} at {path:?}: {detail}"),
            Violation::Count {
                what,
                expected,
                found,
            } => write!(f, "expected {expected} of {what}, found {found}"),
        }
    }
}

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn path_of(links: &[(usize, usize)], mut id: usize) -> Vec<usize> {
    let mut path = Vec::new();
    while id != 0 && id < links.len() {
        let (parent, idx) = links[id];
        path.push(idx);
        id = parent;
    }
    path.reverse();
    path
}

/// 通知 1 件の検査中の状態。
struct Item {
    kind: Option<NoticeKind>,
    ops: Vec<ShellOp>,
    texts: usize,
}

#[derive(Clone, Copy)]
struct Ctx {
    in_area: bool,
    item: Option<usize>,
}

struct Walk {
    out: Vec<Violation>,
    links: Vec<(usize, usize)>,
    items: Vec<Item>,
    counts: Vec<(&'static str, usize)>,
    /// 2 個目以降の（識別子, 要素の id）。
    dups: Vec<(&'static str, usize)>,
}

impl Walk {
    fn bad(&mut self, id: usize, rule: &'static str, detail: impl Into<String>) {
        self.out.push(Violation::Element {
            path: path_of(&self.links, id),
            rule,
            detail: detail.into(),
        });
    }

    fn count(&mut self, testid: &'static str, id: usize) {
        let n = match self.counts.iter_mut().find(|(t, _)| *t == testid) {
            Some((_, n)) => {
                *n += 1;
                *n
            }
            None => {
                self.counts.push((testid, 1));
                1
            }
        };
        if n >= 2 {
            self.dups.push((testid, id));
        }
    }
}

fn check_notice_item(w: &mut Walk, id: usize, attrs: &[(String, String)], ctx: Ctx) -> usize {
    if !ctx.in_area {
        w.bad(id, "notice-item-outside-area", NOTICE_ITEM_TESTID);
    }
    let kind = attr(attrs, NOTICE_KIND_ATTR).and_then(NoticeKind::parse);
    match kind {
        None => w.bad(
            id,
            "undefined-notice-kind",
            attr(attrs, NOTICE_KIND_ATTR).unwrap_or("(missing)"),
        ),
        Some(k) => {
            if w.items.iter().any(|i| i.kind == Some(k)) {
                w.bad(id, "duplicate-notice-kind", k.as_str());
            }
            match attr(attrs, NOTICE_REASON_ATTR) {
                Some(r) if ReasonCode::parse(r).is_none() => {
                    w.bad(id, "reason-format", NOTICE_REASON_ATTR)
                }
                None if k.reason_rule() == ReasonRule::Required => {
                    w.bad(id, "reason-required", k.as_str())
                }
                _ => {}
            }
            let msg = attr(attrs, NOTICE_MESSAGE_ATTR).and_then(MessageId::parse);
            if !msg.is_some_and(|m| k.messages().contains(&m)) {
                w.bad(
                    id,
                    "undefined-message",
                    attr(attrs, NOTICE_MESSAGE_ATTR).unwrap_or("(missing)"),
                );
            }
        }
    }
    w.items.push(Item {
        kind,
        ops: Vec::new(),
        texts: 0,
    });
    w.items.len() - 1
}

fn check_op(w: &mut Walk, id: usize, op: ShellOp, attrs: &[(String, String)], ctx: Ctx) {
    let mut item_kind = None;
    match ctx.item {
        Some(i) => {
            item_kind = w.items[i].kind;
            if w.items[i].ops.contains(&op) {
                w.bad(id, "duplicate-op", op.testid());
            }
            w.items[i].ops.push(op);
            if item_kind.is_some_and(|k| !k.allowed_ops().contains(&op)) {
                w.bad(id, "op-not-allowed", op.testid());
            }
        }
        None => {
            if matches!(op, ShellOp::DismissNotice | ShellOp::DiscardAndStart) {
                w.bad(id, "op-outside-notice", op.testid());
            }
        }
    }
    if attr(attrs, ACTION_ATTR) != Some(op.as_str()) {
        w.bad(id, "op-action", ACTION_ATTR);
        return;
    }
    let payload = attr(attrs, PAYLOAD_ATTR);
    if op == ShellOp::Import {
        if payload.is_some() {
            w.bad(id, "import-payload-in-dom", PAYLOAD_ATTR);
        }
        return;
    }
    let payload = payload.unwrap_or("");
    let mismatch = op == ShellOp::DismissNotice && item_kind.is_some_and(|k| payload != k.as_str());
    if ShellAction::decode(op.as_str(), payload).is_none() || mismatch {
        w.bad(id, "op-action", PAYLOAD_ATTR);
    }
}

fn walk(view: &Node) -> Walk {
    let mut w = Walk {
        out: Vec::new(),
        links: vec![(usize::MAX, 0)],
        items: Vec::new(),
        counts: Vec::new(),
        dups: Vec::new(),
    };
    // 再帰しない。深い木でもスタックあふれを起こさないため。
    let ctx0 = Ctx {
        in_area: false,
        item: None,
    };
    let mut stack: Vec<(&Node, usize, Ctx)> = vec![(view, 0, ctx0)];
    while let Some((node, id, ctx)) = stack.pop() {
        let (attrs, children) = match node {
            Node::Text(_) => continue,
            Node::RawHtml(_) => {
                w.out.push(Violation::RawHtml {
                    path: path_of(&w.links, id),
                });
                continue;
            }
            Node::Element {
                attrs, children, ..
            } => (attrs, children),
        };
        let mut child_ctx = ctx;
        let testid = attr(attrs, TESTID_ATTR);
        if let Some(t) = testid {
            if t.starts_with(SHELL_PREFIX) && !is_defined_testid(t) {
                w.bad(id, "undefined-testid", t);
            }
            if let Some(s) = TopSection::parse(t) {
                w.count(s.testid(), id);
            } else if let Some(m) = PaneMount::parse(t) {
                w.count(m.testid(), id);
            } else if t == NEXT_CHAPTER_TESTID {
                w.count(NEXT_CHAPTER_TESTID, id);
            } else if t == NOTICE_AREA_TESTID {
                w.count(NOTICE_AREA_TESTID, id);
                child_ctx.in_area = true;
            } else if t == NOTICE_ITEM_TESTID {
                child_ctx.item = Some(check_notice_item(&mut w, id, attrs, ctx));
            } else if t == NOTICE_TEXT_TESTID {
                match ctx.item {
                    Some(i) => {
                        w.items[i].texts += 1;
                        if w.items[i].texts == 2 {
                            w.bad(id, "duplicate", NOTICE_TEXT_TESTID);
                        }
                    }
                    None => w.bad(id, "notice-item-outside-area", NOTICE_TEXT_TESTID),
                }
                if !children.iter().all(|c| matches!(c, Node::Text(_))) {
                    w.bad(id, "notice-text-content", NOTICE_TEXT_TESTID);
                }
            } else if let Some(op) = ShellOp::ALL.into_iter().find(|o| o.testid() == t) {
                check_op(&mut w, id, op, attrs, ctx);
            }
        }
        if let Some(name) = attr(attrs, ACTION_ATTR) {
            if let Some(op) = ShellOp::from_name(name) {
                if testid != Some(op.testid()) {
                    w.bad(id, "shell-action-without-testid", name);
                }
            }
        }
        for (i, c) in children.iter().enumerate().rev() {
            w.links.push((id, i));
            stack.push((c, w.links.len() - 1, child_ctx));
        }
    }
    let dups = w.dups.clone();
    for (t, id) in dups {
        w.bad(id, "duplicate", t);
    }
    w
}

fn finish(out: Vec<Violation>) -> Result<(), Vec<Violation>> {
    if out.is_empty() {
        Ok(())
    } else {
        Err(out)
    }
}

/// 部分の木を照合する。定義にない区画・通知の種類・文面の識別子・操作の置き方を違反として全件集める。
/// 生 HTML は中身を検査できないため違反にする。`role="status"` の有無は `a11y` が検査する。
pub fn check_no_undefined(view: &Node) -> Result<(), Vec<Violation>> {
    finish(walk(view).out)
}

/// 完成した画面を照合する。[`check_no_undefined`] に加え、左右の取り付け点がちょうど 1 個ずつあることを求める
/// （対象が無いまま空振りで通るのを防ぐ）。
pub fn check_complete(view: &Node) -> Result<(), Vec<Violation>> {
    let w = walk(view);
    let mut out = w.out;
    for m in PaneMount::ALL {
        let found = w
            .counts
            .iter()
            .find(|(t, _)| *t == m.testid())
            .map_or(0, |(_, n)| *n);
        if found == 0 {
            out.push(Violation::Count {
                what: m.testid().to_owned(),
                expected: 1,
                found,
            });
        }
    }
    finish(out)
}
