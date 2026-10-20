//! glue（wasm の入口）が DOM に対して行ってよい操作の契約（TASK-343 3/3）。
//!
//! 呼び出し元は、契約適合試験 `tests/glue_contract.rs`（`glue.rs` のソースを照合する）と、glue を結線する
//! TASK-347 以降の担当である。値だけを持つため wasm 専用にせず、native でも試験できる。
//!
//! 出所は spec の FEND-20（glue の許される操作）・FEND-21（キー処理の不変条件）と、決定記録 D-42
//! （`docs/spec/01-brainstorm.md`、2026-10-12、TASK-1）である。
//!
//! 状態を変える経路は `Component::update` だけである（FEND-2・FEND-20）。glue は App を共有参照でだけ読む
//! （`glue::with_app`）。`dispatch_action` は Runtime が `update` へ届ける入口で、glue は `set_timeout` のコールバックの中
//! （`glue::deliver`）からだけ呼ぶ（TASK-349。FEND-4）。
//! ツリーの構造を変える操作（要素の生成・挿入・削除、HTML や文字列の一括設定、属性の削除）は
//! [`GlueOp`] に無く、許されない。
//!
//! 現状との差（直さず記録する）:
//! - FEND-20 は要素の取得を mount の時の 2 要素とするが、現在の glue は入力のたびに下書きの欄を引く。
//!   契約は取得の呼び出し箇所の数（[`MAX_ELEMENT_LOOKUPS`] 以下）で表し、時点の見直しは TASK-347 以降で行う。
//! - Enter の送信（TASK-275）のため、取得する要素は root・下書きの欄・送信ボタンの 3 要素になる。FEND-20 の
//!   「mount の時に 2 要素」とは異なる。呼び出し箇所は共有の取得関数 1 つにまとめ、数は 2 のまま保つ。
//! - FEND-4 はタイマーのコールバックから `dispatch_action` を呼ぶ前提のため、`tests/state_paths.rs` の規則を
//!   「`deliver` の 1 箇所だけ・`schedule_action` が `set_timeout` を通して呼ぶ」に見直した（TASK-349）。
//! - root の外の Esc（フォーカスなし・root の外の部品。FEND-8・D-42）のため、keydown の受け手が root に加えて
//!   `document` にもなる。root の中の Esc は届けず、二重配送は作らない（`glue::wire_outside_esc`。TASK-353 1/2）。
//!
//! キー処理は本文に入れない。ここには不変条件と、決定の受け持ちだけを置く。root の中の Esc は上流の属性
//! （`action::ESC_KEYDOWN_ATTRS`）で届く。
//!
//! 変える場合は、先に spec を直し、`parts::PARTS_CONTRACT_VERSION` を上げる。

/// glue が DOM に対して行ってよい操作（FEND-20 の並び）。これ以外の操作は glue に書かない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlueOp {
    /// 要素の取得（root と下書きの欄。root 配下の探索と `Runtime::root()` に限る）。
    ElementLookup,
    /// 属性の書き込み。
    AttributeWrite,
    /// click の呼び出し。
    Click,
    /// タイマー。
    Timer,
    /// keydown の受け取り。
    KeydownReceive,
    /// 変換中の印の読み取りと、変換の終わりの受け取り。
    CompositionMark,
    /// 下書きの欄の値の反映。
    TextareaSync,
}

impl GlueOp {
    /// 全操作。FEND-20 の並び順。
    pub const ALL: [GlueOp; 7] = [
        GlueOp::ElementLookup,
        GlueOp::AttributeWrite,
        GlueOp::Click,
        GlueOp::Timer,
        GlueOp::KeydownReceive,
        GlueOp::CompositionMark,
        GlueOp::TextareaSync,
    ];

    /// 識別と診断用の固定の名前。
    pub const fn name(self) -> &'static str {
        match self {
            GlueOp::ElementLookup => "element-lookup",
            GlueOp::AttributeWrite => "attribute-write",
            GlueOp::Click => "click",
            GlueOp::Timer => "timer",
            GlueOp::KeydownReceive => "keydown-receive",
            GlueOp::CompositionMark => "composition-mark",
            GlueOp::TextareaSync => "textarea-sync",
        }
    }
}

/// 要素の取得の呼び出し箇所の上限（root と下書きの欄の 2 要素）。
pub const MAX_ELEMENT_LOOKUPS: usize = 2;

/// キー処理の受け持ち。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyHandlingOwner {
    /// glue が受け持つ。属性にはせず、上流への要望も出さない（D-42）。
    Glue,
}

/// Enter・Tab・Space の状態依存の `preventDefault` の受け持ち（D-42）。
pub const STATEFUL_PREVENT_DEFAULT_OWNER: KeyHandlingOwner = KeyHandlingOwner::Glue;

/// root の外の Esc の受け持ち（D-42。root の中の Esc は上流の属性が受け持つ）。結線先は `glue::wire_outside_esc`（TASK-353 1/2）。
pub const OUTSIDE_ROOT_ESC_OWNER: KeyHandlingOwner = KeyHandlingOwner::Glue;

/// Tab で `preventDefault` するか（FEND-21）。候補が開いていて、入力欄に文字があるときだけ真である。
///
/// 変換中は前提の外（FEND-10・JUDGE-16 の範囲）で、ここでは扱わない。修飾キー付きの Enter（JUDGE-22）は版 2 で `Nothing`（`preventDefault` なし）に確定しており、ここでは扱わない。
pub const fn tab_prevents_default(candidates_open: bool, input_has_text: bool) -> bool {
    candidates_open && input_has_text
}
