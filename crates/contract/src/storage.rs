//! 保存先の口と履歴の鍵の生成規則（TASK-310 1/3、版 1）。
//!
//! 履歴は、受講者のブラウザの保存領域に置く。保存領域は localStorage を既定とし、上限に近づいたら
//! IndexedDB に切り替える（D-52）。この module は、両者を同じ口 [`HistoryStore`] で扱うための型と、
//! 口に渡す鍵 [`HistoryKey`] の生成規則を置く。呼び出し元は、web の保存先の実装（TASK-317）、鍵の実装と
//! 自動ブラウザ試験（TASK-326）、教材ごとの履歴の一覧と削除（SAVE-32）である。contract は core にも web にも
//! 依存せず、native で試験できる。
//!
//! 決定との対応（ID と要旨だけを書く）:
//!
//! | 決定・振る舞い | 型・定数での表れ |
//! |---|---|
//! | D-52: 既定は localStorage、上限に近づいたら IndexedDB | [`StorageKind`]・[`DEFAULT_STORAGE_KIND`]。切り替えの閾値は TASK-317 |
//! | 口の操作は書く・読む・消す・鍵の一覧 | [`HistoryStore`] の `write`・`read`・`remove`・`keys` |
//! | SAVE-14: 鍵は教材の版・内容に依らず、教材 ID と最初の開始点の ID で決まる | [`HistoryKey::from_ir`] は `pack.id` と `starts[0].id` だけを読む |
//! | SAVE-15: IR が読めないときは `unreadable/<IR の名前>` | [`HistoryKey::unreadable`] |
//! | D-56: 配信先のパスの先頭 1 区間を含め、全体は含めない | [`DeliveryScope`]（[`DeliveryScope::from_pathname`]） |
//! | SAVE-12: 古いタブは保存しない。合流の規則は足さない | 鍵に起動番号を含めない。口に合流の操作は無い。起動番号の照合は web の `boot_guard` が持つ |
//!
//! 鍵の形（版 1）。区間は常に 5 つで、区間の中に `/` は現れない。
//!
//! ```text
//! fandhe-workshop/history/<配信の区間>/<教材の ID>/<最初の開始点の ID>
//! fandhe-workshop/history/<配信の区間>/unreadable/<IR の名前>
//! ```
//!
//! 仕様が決めていない点を、次のとおり決めた（推論）。
//!
//! - 配信の区間の位置と根の印: D-56 は区間を含めることだけを決めている。接頭辞の直後に置き、
//!   区間が無い（根に置いた）ときは予約の印 `~` を書く。区間の数を固定し、接頭辞での一覧・削除が別の配信先の
//!   鍵に届かないようにするため。`/repo/v2/` のように版つきの URL でも先頭の 1 区間だけを見るので、履歴は分かれない。
//! - 区間の符号化: 教材の ID・開始点の ID・IR の名前・pathname は未信頼の文字列で、`/` や `%` や非 ASCII を含みうる。
//!   各区間は `[A-Za-z0-9._-]` 以外を UTF-8 のバイトごとに `%XX`（大文字の 16 進）へ符号化し、鍵の組み立てを単射にする。
//!   教材の ID がちょうど `unreadable` のときは `%75nreadable` と書き、読めない IR の鍵と衝突させない。
//!   普通の ID（`bbs-tutorial`・`s-0-1`）は試作の文字列のまま出る。
//! - 口は同期にした。contract は非同期の実行基盤を持たず、localStorage への書き込みは `pagehide` で同期に
//!   行う必要がある（TASK-317）。IndexedDB の非同期を埋める仕組み（手元の写し・後からの書き込み）は web の責務である。
//!
//! 限界: [`DeliveryScope::from_pathname`] は pathname を復号せず、末尾の `/` が無い `/repo` は根として扱う。
//! 配信先が `/repo/` へ転送する前提である。
//!
//! 本契約が定義しないもの:
//!
//! - 起動番号の `fandhe-workshop/meta/…` の鍵（SAVE-12）。web の `boot_guard` が持ち、版 1 の契約には含めない（接頭辞が違うため [`HistoryKey::parse`] は拒み、一覧・削除の対象に入らない）
//! - 秘密情報の警告の `sessionStorage` の鍵。必要なら版上げで足す
//! - localStorage に書ける最大の判定と、IndexedDB の失敗時の挙動（TASK-317）
//! - 判定の順序（TASK-310 3/3）と読み込みの拒否理由コード（TASK-310 2/3）は [`crate::load_reason`] にある
//!   （順序の正は [`crate::load_reason::HISTORY_LOAD_ORDER`]）。どちらも同じ版 1 の範囲で、
//!   新しい版の定数を作らず [`STORAGE_CONTRACT_VERSION`] を参照する
//! - 履歴の値の加工。値は口が触らず、HTML へ出すときのエスケープは web の責務である
//! - SAVE-32 の削除の接頭辞は、試作の鍵（`…/history/<教材の ID>/`）の文面から配信の区間の分だけずれる。
//!   本契約では [`pack_prefix`]・[`HistoryKey::belongs_to`] が正である
//!
//! 版の定数 [`STORAGE_CONTRACT_VERSION`] は、鍵の形・口の操作・[`StorageError`] の variant・
//! [`crate::load_reason::LoadReason`] の variant と文字列・判定の順序（[`crate::load_reason::HISTORY_LOAD_ORDER`]・
//! [`crate::load_reason::HistoryLoadStage`]・[`crate::load_reason::LoadReason::stage`]）を変えるときに上げる。

use crate::ir::Ir;
use std::fmt;

/// 保存先の契約の版。鍵の形・口の操作・[`StorageError`] の variant・[`crate::load_reason::LoadReason`] の
/// variant と文字列・判定の順序（[`crate::load_reason::HISTORY_LOAD_ORDER`]）を変えるときに上げる。
pub const STORAGE_CONTRACT_VERSION: u32 = 1;

/// 履歴の鍵の接頭辞。履歴の鍵はすべてこれと `/` で始まる。
pub const HISTORY_KEY_PREFIX: &str = "fandhe-workshop/history";

/// 配信先の区間が無い（根に置いた）ことを表す予約の印。区間としての `~` は `%7E` に符号化されるため衝突しない。
const ROOT_MARK: &str = "~";

/// 読めない IR の鍵に使う予約の区間。教材の ID の位置では `%75nreadable` と符号化される。
const UNREADABLE_SEGMENT: &str = "unreadable";

/// 保存先の種類（D-52）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageKind {
    /// localStorage。既定の保存先。
    LocalStorage,
    /// IndexedDB。localStorage の上限に近づいたときの保存先。
    IndexedDb,
}

/// 既定の保存先（D-52）。
pub const DEFAULT_STORAGE_KIND: StorageKind = StorageKind::LocalStorage;

/// 口の操作の失敗。値・鍵・ブラウザの例外の文は持たない（履歴は受講者のデータで、下書きを含むため）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageError {
    /// 保存領域の上限を超えた。呼び出し側が IndexedDB への切り替えを判断する材料になる。
    QuotaExceeded,
    /// 保存領域が無効・遮断されている（プライベートモード等）、または開けなかった。
    Unavailable,
    /// その他の失敗。
    Failed,
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            StorageError::QuotaExceeded => "storage quota exceeded",
            StorageError::Unavailable => "storage unavailable",
            StorageError::Failed => "storage operation failed",
        };
        f.write_str(s)
    }
}

impl std::error::Error for StorageError {}

/// `%XX` に符号化せずそのまま出してよいバイトか。
fn is_plain(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')
}

/// 区間を符号化する。`[A-Za-z0-9._-]` 以外を UTF-8 のバイトごとに `%XX`（大文字）へ書く。
fn encode_segment(s: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if is_plain(b) {
            out.push(char::from(b));
        } else {
            out.push('%');
            out.push(char::from(HEX[usize::from(b >> 4)]));
            out.push(char::from(HEX[usize::from(b & 0x0f)]));
        }
    }
    out
}

fn hex_value(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// [`encode_segment`] の逆。大文字の 16 進だけを受け、不完全な `%` と UTF-8 でない結果は `None`。
/// 符号化しなくてよい文字の符号化の拒否は、呼び出し側の再書き出しとの照合で行う。
fn decode_segment(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hi = hex_value(*bytes.get(i + 1)?)?;
            let lo = hex_value(*bytes.get(i + 2)?)?;
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// 教材の ID の位置の符号化。予約の区間 `unreadable` と衝突しないよう、先頭の 1 文字を符号化する。
fn encode_pack_id(s: &str) -> String {
    if s == UNREADABLE_SEGMENT {
        "%75nreadable".to_string()
    } else {
        encode_segment(s)
    }
}

/// 配信先の区間（D-56）。配信先のパスの先頭 1 区間（Pages ではリポジトリ名）で、パスの全体は含めない。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeliveryScope {
    segment: Option<String>,
}

impl DeliveryScope {
    /// 区間が無い（根に置いた）配信先。
    pub fn root() -> Self {
        DeliveryScope { segment: None }
    }

    /// 区間を指定する。空文字列は根として扱う。
    pub fn named(segment: &str) -> Self {
        if segment.is_empty() {
            Self::root()
        } else {
            DeliveryScope {
                segment: Some(segment.to_string()),
            }
        }
    }

    /// URL の pathname から作る。最後の `/` までのディレクトリ部分の、最初の空でない区間を取る。
    ///
    /// `/repo/`・`/repo/index.html`・`/repo/v2/` は `repo`。`/`・`/index.html`・`//`・空文字列・`/repo` は根。
    /// 復号はしない（同じ入力から同じ鍵ができれば足りる）。
    pub fn from_pathname(pathname: &str) -> Self {
        let Some(last) = pathname.rfind('/') else {
            return Self::root();
        };
        let dir = &pathname[..last];
        match dir.split('/').find(|s| !s.is_empty()) {
            Some(seg) => Self::named(seg),
            None => Self::root(),
        }
    }

    /// 区間。根なら `None`。
    pub fn segment(&self) -> Option<&str> {
        self.segment.as_deref()
    }

    fn render(&self) -> String {
        match &self.segment {
            Some(s) => encode_segment(s),
            None => ROOT_MARK.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum KeyKind {
    Pack { pack_id: String, start_id: String },
    Unreadable { ir_name: String },
}

/// 履歴の鍵。欄は閉じており、構築子を通した値だけが存在する。口は任意の文字列の鍵を受けない。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HistoryKey {
    scope: DeliveryScope,
    kind: KeyKind,
}

impl HistoryKey {
    /// 教材の ID と最初の開始点の ID から作る（SAVE-14）。
    pub fn for_pack(scope: DeliveryScope, pack_id: &str, start_id: &str) -> Self {
        HistoryKey {
            scope,
            kind: KeyKind::Pack {
                pack_id: pack_id.to_string(),
                start_id: start_id.to_string(),
            },
        }
    }

    /// IR が読めないときの鍵（SAVE-15）。`ir_name` は呼び出し側が渡す不透明な名前で、符号化して使う。
    pub fn unreadable(scope: DeliveryScope, ir_name: &str) -> Self {
        HistoryKey {
            scope,
            kind: KeyKind::Unreadable {
                ir_name: ir_name.to_string(),
            },
        }
    }

    /// 読めた IR から作る。`pack.id` と `starts` の先頭の `id` だけを読み、版・題名・内容は読まない。
    /// `starts` が空なら `None` を返す。呼び出し側は [`HistoryKey::unreadable`] へ倒す。
    pub fn from_ir(scope: DeliveryScope, ir: &Ir) -> Option<Self> {
        let start = ir.starts.first()?;
        Some(Self::for_pack(scope, &ir.pack.id, &start.id))
    }

    /// 配信先の区間。
    pub fn scope(&self) -> &DeliveryScope {
        &self.scope
    }

    /// この鍵が、指定した配信先の指定した教材の履歴か。SAVE-32 の削除の対象の判定に使う。
    pub fn belongs_to(&self, scope: &DeliveryScope, pack_id: &str) -> bool {
        &self.scope == scope
            && matches!(&self.kind, KeyKind::Pack { pack_id: p, .. } if p == pack_id)
    }

    /// 保存領域に書く文字列にする。
    pub fn render(&self) -> String {
        let scope = self.scope.render();
        match &self.kind {
            KeyKind::Pack { pack_id, start_id } => format!(
                "{HISTORY_KEY_PREFIX}/{scope}/{}/{}",
                encode_pack_id(pack_id),
                encode_segment(start_id)
            ),
            KeyKind::Unreadable { ir_name } => format!(
                "{HISTORY_KEY_PREFIX}/{scope}/{UNREADABLE_SEGMENT}/{}",
                encode_segment(ir_name)
            ),
        }
    }

    /// [`HistoryKey::render`] の逆。正規の文字列だけを受け、それ以外（小文字の 16 進・不要な符号化・
    /// 区間の数の違い・接頭辞の違い）は `None`。同じオリジンのほかの鍵を履歴の鍵と見なさないために厳格にする。
    pub fn parse(s: &str) -> Option<Self> {
        let rest = s.strip_prefix(HISTORY_KEY_PREFIX)?.strip_prefix('/')?;
        let mut parts = rest.split('/');
        let (scope, second, third) = (parts.next()?, parts.next()?, parts.next()?);
        if parts.next().is_some() {
            return None;
        }
        let scope = if scope == ROOT_MARK {
            DeliveryScope::root()
        } else {
            DeliveryScope::named(&decode_segment(scope)?)
        };
        let key = if second == UNREADABLE_SEGMENT {
            Self::unreadable(scope, &decode_segment(third)?)
        } else {
            Self::for_pack(scope, &decode_segment(second)?, &decode_segment(third)?)
        };
        (key.render() == s).then_some(key)
    }
}

impl fmt::Display for HistoryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

/// 指定した配信先・教材の履歴の鍵がすべて前方一致する接頭辞（末尾の `/` を含む）。
/// 区間の数が固定で区間の中に `/` が無いため、別の配信先・別の教材の鍵には一致しない。
pub fn pack_prefix(scope: &DeliveryScope, pack_id: &str) -> String {
    format!(
        "{HISTORY_KEY_PREFIX}/{}/{}/",
        scope.render(),
        encode_pack_id(pack_id)
    )
}

/// 保存先の口。localStorage と IndexedDB を同じ操作で切り替える。
///
/// 受け手は `&self` で、状態はブラウザの保存領域の側にある。object safe で、実行時の切り替えは
/// `Box<dyn HistoryStore>` で行える。非同期の保存領域を同期の口に載せる仕組みは実装側の責務である。
pub trait HistoryStore {
    /// この口の保存先の種類。
    fn kind(&self) -> StorageKind;

    /// 値を書く。同じ鍵があれば置き換える。
    fn write(&self, key: &HistoryKey, value: &str) -> Result<(), StorageError>;

    /// 値を読む。鍵が無ければ `Ok(None)`。
    fn read(&self, key: &HistoryKey) -> Result<Option<String>, StorageError>;

    /// 値を消す。鍵が無くても `Ok`。
    fn remove(&self, key: &HistoryKey) -> Result<(), StorageError>;

    /// 履歴の鍵の一覧。[`HistoryKey::parse`] できる鍵だけを返し、同じオリジンのほかの鍵は含めない。順序は規定しない。
    fn keys(&self) -> Result<Vec<HistoryKey>, StorageError>;
}

impl<P: HistoryStore + ?Sized> HistoryStore for &P {
    fn kind(&self) -> StorageKind {
        (**self).kind()
    }
    fn write(&self, key: &HistoryKey, value: &str) -> Result<(), StorageError> {
        (**self).write(key, value)
    }
    fn read(&self, key: &HistoryKey) -> Result<Option<String>, StorageError> {
        (**self).read(key)
    }
    fn remove(&self, key: &HistoryKey) -> Result<(), StorageError> {
        (**self).remove(key)
    }
    fn keys(&self) -> Result<Vec<HistoryKey>, StorageError> {
        (**self).keys()
    }
}

impl<P: HistoryStore + ?Sized> HistoryStore for Box<P> {
    fn kind(&self) -> StorageKind {
        (**self).kind()
    }
    fn write(&self, key: &HistoryKey, value: &str) -> Result<(), StorageError> {
        (**self).write(key, value)
    }
    fn read(&self, key: &HistoryKey) -> Result<Option<String>, StorageError> {
        (**self).read(key)
    }
    fn remove(&self, key: &HistoryKey) -> Result<(), StorageError> {
        (**self).remove(key)
    }
    fn keys(&self) -> Result<Vec<HistoryKey>, StorageError> {
        (**self).keys()
    }
}
