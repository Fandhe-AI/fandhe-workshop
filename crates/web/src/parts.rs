//! 表示部品を `fandhe-frontend` に載せる口の契約（TASK-343 1/3）。ホストの型が実装する 3 つのトレイトと、
//! 表示部品（ログ・左の成果物のビュー・復習・選択肢・チェックポイント・候補の一覧）を登録する口を固定する。
//!
//! 呼び出し元は、ホストの Component を作る TASK-347 と、部品を実装する TASK-434 などの担当である。
//! `Runtime::mount` は 1 つの Component だけを受け取るため、部品は個別に mount されず、ホストが
//! [`PartRegistry`] を持って `view`・`dirty_fields`・`bound_value` を振り分ける（合成はホストの責務）。
//! native の rlib で試験できるよう、wasm 専用にしない。
//!
//! 出所は spec の FEND-2（表示層は進行管理を持たない）・FEND-3（dirty field は束縛点か keyed list に対応する）・
//! FEND-15・FEND-20（状態を変える経路はホストの `update` だけ）と、upstream-api の要約である。
//! 部品のトレイトは `&self` だけを受け取り、状態を変える口を持たない。登録の不正は拒否し、登録しない。
//!
//! 3 分割の境界: 本モジュールは口の型・登録・振り分けまでを持つ。action の名前と宛先（`action`）・操作
//! プロファイルと設定の渡し方（`profile`）は 2/3 で足した。glue の許される操作（`glue_ops`）・keyed list の
//! 親の属性と項目数の方針（`list_policy`）・契約の版番号 [`PARTS_CONTRACT_VERSION`] は 3/3 で足し、
//! TASK-343 の契約は版 1 で確定した。TASK-283 で版 2 に上げ（spec の FEND-20・D-65）、上限を超える貼り付けの切り捨て（`beforeinput`・選択の位置の読み取り・キャレット・合成の `input`）を欄の値の反映の範囲に足した。
//!
//! 意図して固定しないもの（持ち主が別にあるため）:
//! - 行の型、`data-testid`、束縛点の名前（TASK-431・TASK-467・TASK-511 が持つ。部品は自分の field 名を宣言するだけ）
//! - 部品の画面上の配置（ホストが決める。登録の口は種類ごとの [`Node`] を返すだけ）
//! - `data-action` の値と action の復号
//!
//! 変える場合は、先に spec を直す。

use core::fmt;
use std::collections::HashSet;

use fandhe_frontend_core::Node;

use crate::profile::DisplaySettings;

/// 表示部品の契約の版。対象は `parts`・`action`・`profile`・`glue_ops`・`list_policy` の 5 モジュールである。
///
/// 口の型・名前・payload の書式・許される操作の一覧・項目数の方針・不変条件を変えたら上げる。先に spec を直す。
/// アクセシビリティの構造の契約の版（`a11y::A11Y_CONTRACT_VERSION`）とは別に管理する。
pub const PARTS_CONTRACT_VERSION: u32 = 3;

pub use fandhe_frontend_interactive::{Component, DirtyTracked};
pub use fandhe_frontend_wasm_client::{BindingSource, BoundValue};

/// `Runtime::mount` が求める境界（wasm32 専用）を、native でも名前で参照するための束ね。
pub trait Mountable: Component + DirtyTracked + BindingSource + 'static {}

impl<C: Component + DirtyTracked + BindingSource + 'static> Mountable for C {}

/// 型 `C` が mount できる条件を満たすことを、コンパイル時に確かめる。
pub fn assert_mountable<C: Mountable>() {}

/// 表示部品の種類。1 種類につき 1 つだけ登録できる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartKind {
    /// ログ。
    Log,
    /// 左の成果物のビュー。
    ArtifactView,
    /// 復習。
    Review,
    /// 選択肢。
    Offers,
    /// チェックポイント。
    Checkpoints,
    /// 候補の一覧。
    Candidates,
}

impl PartKind {
    /// 全種類。登録の標準の並び順。
    pub const ALL: [PartKind; 6] = [
        PartKind::Log,
        PartKind::ArtifactView,
        PartKind::Review,
        PartKind::Offers,
        PartKind::Checkpoints,
        PartKind::Candidates,
    ];

    /// 識別と診断用の固定の名前。DOM の識別子には使わない（識別子の持ち主は別の契約）。
    pub const fn name(self) -> &'static str {
        match self {
            PartKind::Log => "log",
            PartKind::ArtifactView => "artifact-view",
            PartKind::Review => "review",
            PartKind::Offers => "offers",
            PartKind::Checkpoints => "checkpoints",
            PartKind::Candidates => "candidates",
        }
    }
}

/// dirty field の種別。keyed list の field を選び出すための区別である。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldKind {
    /// 束縛点（`BindingSource::bound_value` が値を答える）。
    Bound,
    /// keyed list（差分更新の対象。値は束縛では答えない）。
    KeyedList,
}

/// 部品が宣言する dirty field。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartField {
    /// field 名。`[a-z][a-z0-9_]*` で 64 バイト以内。
    pub name: &'static str,
    /// field の種別。
    pub kind: FieldKind,
}

/// field 名の最大のバイト数。
pub const MAX_FIELD_NAME_BYTES: usize = 64;

/// 表示部品。`M` はホストが持つ表示用の値の型で、部品は読むだけにする。
///
/// 状態を変える口は持たない。状態を変える経路は、ホストの `Component::update` だけである（FEND-2・FEND-20）。
pub trait DisplayPart<M: ?Sized> {
    /// 部品の種類。
    fn kind(&self) -> PartKind;

    /// この部品が持つ dirty field の宣言。
    fn fields(&self) -> &'static [PartField];

    /// 部品の描画。
    fn view(&self, model: &M) -> Node;

    /// 束縛点 `field` の現在の値。既定は答えない。
    fn bound_value(&self, _field: &str, _model: &M) -> Option<BoundValue> {
        None
    }
}

/// 登録の拒否の理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegisterError {
    /// 同じ種類が登録済み。
    DuplicateKind(PartKind),
    /// field 名が規則に合わない（空・文字種・長さ）。
    InvalidFieldName(&'static str),
    /// field 名が重複している（部品の中・部品の間）。
    DuplicateField(&'static str),
}

impl fmt::Display for RegisterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegisterError::DuplicateKind(k) => write!(f, "duplicate part kind: {}", k.name()),
            RegisterError::InvalidFieldName(n) => write!(f, "invalid field name: {n:?}"),
            RegisterError::DuplicateField(n) => write!(f, "duplicate field name: {n}"),
        }
    }
}

impl std::error::Error for RegisterError {}

/// 未登録の field。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownField(pub &'static str);

impl fmt::Display for UnknownField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown dirty field: {}", self.0)
    }
}

impl std::error::Error for UnknownField {}

fn valid_field_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    name.len() <= MAX_FIELD_NAME_BYTES
        && bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// 表示部品の登録簿。ホストが持ち、`view`・`dirty_fields`・`bound_value` をここへ振り分ける。
pub struct PartRegistry<M: ?Sized> {
    parts: Vec<Box<dyn DisplayPart<M>>>,
}

impl<M: ?Sized> Default for PartRegistry<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: ?Sized> PartRegistry<M> {
    /// 空の登録簿を作る。
    pub fn new() -> Self {
        Self { parts: Vec::new() }
    }

    /// 登録した部品の数。
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// 登録が空か。
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// 部品を登録する。種類・field 名の不正や重複は拒否し、登録しない。
    pub fn register(&mut self, part: Box<dyn DisplayPart<M>>) -> Result<(), RegisterError> {
        let kind = part.kind();
        if self.parts.iter().any(|p| p.kind() == kind) {
            return Err(RegisterError::DuplicateKind(kind));
        }
        let mut seen: HashSet<&'static str> = HashSet::new();
        for field in part.fields() {
            if !valid_field_name(field.name) {
                return Err(RegisterError::InvalidFieldName(field.name));
            }
            if !seen.insert(field.name) || self.owner(field.name).is_some() {
                return Err(RegisterError::DuplicateField(field.name));
            }
        }
        self.parts.push(part);
        Ok(())
    }

    /// 種類ごとの描画。未登録なら `None`。
    pub fn view(&self, kind: PartKind, model: &M) -> Option<Node> {
        self.parts
            .iter()
            .find(|p| p.kind() == kind)
            .map(|p| p.view(model))
    }

    /// 登録順の描画。
    pub fn views(&self, model: &M) -> Vec<(PartKind, Node)> {
        self.parts
            .iter()
            .map(|p| (p.kind(), p.view(model)))
            .collect()
    }

    /// 登録順の描画のうち、設定が出すとする種類だけ。入力欄なしのときは候補の一覧を描かない。
    pub fn visible_views(&self, settings: &DisplaySettings, model: &M) -> Vec<(PartKind, Node)> {
        self.parts
            .iter()
            .filter(|p| settings.shows_part(p.kind()))
            .map(|p| (p.kind(), p.view(model)))
            .collect()
    }

    /// 登録済みの全 field。
    pub fn fields(&self) -> impl Iterator<Item = &PartField> {
        self.parts.iter().flat_map(|p| p.fields().iter())
    }

    /// field の持ち主の種類。未登録なら `None`。
    pub fn owner(&self, field: &str) -> Option<PartKind> {
        self.parts
            .iter()
            .find(|p| p.fields().iter().any(|f| f.name == field))
            .map(|p| p.kind())
    }

    /// 束縛点の値を持ち主の部品へ振り分ける。持ち主が無い field と keyed list の field は `None`。
    pub fn bound_value(&self, field: &str, model: &M) -> Option<BoundValue> {
        let part = self
            .parts
            .iter()
            .find(|p| p.fields().iter().any(|f| f.name == field))?;
        let declared = part.fields().iter().find(|f| f.name == field)?;
        match declared.kind {
            FieldKind::Bound => part.bound_value(field, model),
            FieldKind::KeyedList => None,
        }
    }

    /// dirty field がすべて登録済みであることを確かめる（FEND-3）。未登録が 1 つでもあれば `Err`。
    pub fn check_dirty(&self, dirty: &[&'static str]) -> Result<(), UnknownField> {
        match dirty.iter().find(|d| self.owner(d).is_none()) {
            Some(name) => Err(UnknownField(name)),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fandhe_frontend_core::text;

    struct P(PartKind, &'static [PartField]);

    impl DisplayPart<()> for P {
        fn kind(&self) -> PartKind {
            self.0
        }
        fn fields(&self) -> &'static [PartField] {
            self.1
        }
        fn view(&self, _: &()) -> Node {
            text("x")
        }
    }

    const BAD: &[PartField] = &[PartField {
        name: "Bad",
        kind: FieldKind::Bound,
    }];

    #[test]
    fn 不正な_field_名の部品は登録されない() {
        let mut r = PartRegistry::<()>::new();
        assert_eq!(
            r.register(Box::new(P(PartKind::Log, BAD))),
            Err(RegisterError::InvalidFieldName("Bad"))
        );
        assert!(r.is_empty());
    }

    #[test]
    fn 名前の規則() {
        assert!(valid_field_name("a"));
        assert!(valid_field_name("a_1"));
        assert!(!valid_field_name(""));
        assert!(!valid_field_name("1a"));
        assert!(!valid_field_name("a-b"));
        assert!(!valid_field_name(&"a".repeat(65)));
    }
}
