//! 語彙の版の表（語彙 v1〜v1.3 の語と名前つき拡張点 EP-1〜EP-12）。
//!
//! 正は `docs/vocabulary/vocabulary.md`（版 2）で、本モジュールは実装側の写しである。
//! `fandhe-pack vocab`（TASK-408）の数え上げと、宣言と使用の一致の検査（TASK-59）が、
//! 語ごとの語彙の版を引くために使う。文書と本表の一致は `tests/vocabulary_terms.rs` が確かめる。
//! 純粋な定数の表で、I/O も panic も持たない。
//!
//! `result`・`rejection` の語は、contract の [`JudgmentKind`]・[`Category`]（TASK-256）への写し
//! （[`result_kind`]・[`rejection_category`]）を持つ。判定結果と拒否分類の型は contract が正で、pack は別に定義しない（TASK-45）。

use fandhe_workshop_contract::judgment::{Category, JudgmentKind};

/// 語彙の版（語ごとの版）。文書の版や IR の形式の版 `ir_format` とは別に管理する。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum VocabVersion {
    /// 語彙 v1（基本の語彙と EP-1〜EP-5）。
    V1,
    /// 語彙 v1.1（EP-6〜EP-8）。
    V1_1,
    /// 語彙 v1.2（EP-9）。
    V1_2,
    /// 語彙 v1.3（EP-10〜EP-12 と `confirm.options[].label`）。
    V1_3,
}

impl VocabVersion {
    /// 文書の表記（`v1`・`v1.1` など）を返す。
    pub const fn as_str(self) -> &'static str {
        match self {
            VocabVersion::V1 => "v1",
            VocabVersion::V1_1 => "v1.1",
            VocabVersion::V1_2 => "v1.2",
            VocabVersion::V1_3 => "v1.3",
        }
    }
}

/// このエンジンが知る語彙の版の上限。
pub const LATEST: VocabVersion = VocabVersion::V1_3;

/// 名前つき拡張点の 1 件。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExtensionPoint {
    /// 拡張点の ID（`EP-1` など）。
    pub id: &'static str,
    /// 拡張点の名前（`input.wait` など）。
    pub name: &'static str,
    /// 種別。名前の先頭の語と一致する。
    pub kind: &'static str,
    /// 拡張点を導入した語彙の版。
    pub version: VocabVersion,
}

/// 語の 1 件。（`kind`, `term`）の組が一意のキーである。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VocabTerm {
    /// 語の分類のキー（`input`・`condition`・`field` など）。拡張点の種別とは別物。
    pub kind: &'static str,
    /// 語。欄は経路（`confirm.options[].label`）、値は `<欄>=<値>` で書く。
    pub term: &'static str,
    /// 語が入った語彙の版。
    pub version: VocabVersion,
    /// 対応する拡張点の ID。番号を持たない語は `None`。
    pub extension_point: Option<&'static str>,
}

/// 名前つき拡張点の公開の一覧（EP 番号順）。
pub const EXTENSION_POINTS: &[ExtensionPoint] = &[
    ExtensionPoint {
        id: "EP-1",
        name: "input.wait",
        kind: "input",
        version: VocabVersion::V1,
    },
    ExtensionPoint {
        id: "EP-2",
        name: "input.cancel",
        kind: "input",
        version: VocabVersion::V1,
    },
    ExtensionPoint {
        id: "EP-3",
        name: "condition.param",
        kind: "condition",
        version: VocabVersion::V1,
    },
    ExtensionPoint {
        id: "EP-4",
        name: "condition.clarifying",
        kind: "condition",
        version: VocabVersion::V1,
    },
    ExtensionPoint {
        id: "EP-5",
        name: "view.card_list",
        kind: "view",
        version: VocabVersion::V1,
    },
    ExtensionPoint {
        id: "EP-6",
        name: "confirm.answer.option_on_wrong",
        kind: "confirm",
        version: VocabVersion::V1_1,
    },
    ExtensionPoint {
        id: "EP-7",
        name: "text.slot",
        kind: "text",
        version: VocabVersion::V1_1,
    },
    ExtensionPoint {
        id: "EP-8",
        name: "guidance.variant",
        kind: "guidance",
        version: VocabVersion::V1_1,
    },
    ExtensionPoint {
        id: "EP-9",
        name: "starts.requires",
        kind: "starts",
        version: VocabVersion::V1_2,
    },
    ExtensionPoint {
        id: "EP-10",
        name: "pack.profile",
        kind: "pack",
        version: VocabVersion::V1_3,
    },
    ExtensionPoint {
        id: "EP-11",
        name: "pack.appearance",
        kind: "pack",
        version: VocabVersion::V1_3,
    },
    ExtensionPoint {
        id: "EP-12",
        name: "pack.left_pane",
        kind: "pack",
        version: VocabVersion::V1_3,
    },
];

/// 語の公開の一覧。文書の 7 節と同じ順で並べる。
pub const TERMS: &[VocabTerm] = &[
    VocabTerm {
        kind: "input",
        term: "command",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "input",
        term: "text",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "input",
        term: "choice",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "input",
        term: "ui_action",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "input",
        term: "approval",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "input",
        term: "wait",
        version: VocabVersion::V1,
        extension_point: Some("EP-1"),
    },
    VocabTerm {
        kind: "input",
        term: "cancel",
        version: VocabVersion::V1,
        extension_point: Some("EP-2"),
    },
    VocabTerm {
        kind: "condition",
        term: "step",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "condition",
        term: "flag",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "condition",
        term: "world_has",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "condition",
        term: "world_attr",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "condition",
        term: "count",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "condition",
        term: "chose",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "condition",
        term: "all",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "condition",
        term: "any",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "condition",
        term: "not",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "condition",
        term: "param",
        version: VocabVersion::V1,
        extension_point: Some("EP-3"),
    },
    VocabTerm {
        kind: "condition",
        term: "clarifying",
        version: VocabVersion::V1,
        extension_point: Some("EP-4"),
    },
    VocabTerm {
        kind: "effect",
        term: "put",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "effect",
        term: "remove",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "effect",
        term: "replace",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "effect",
        term: "set_attr",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "effect",
        term: "set_flag",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "effect",
        term: "clear_flag",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "effect",
        term: "set_flag_global",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "effect",
        term: "processing",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "effect",
        term: "when",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "confirm",
        term: "point_at",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "confirm",
        term: "answer",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "confirm",
        term: "approve",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "confirm",
        term: "none",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "field",
        term: "confirm.reason",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "result",
        term: "accepted",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "result",
        term: "rejected",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "result",
        term: "clarified",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "rejection",
        term: "unknown_command",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "rejection",
        term: "wrong_order",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "rejection",
        term: "missing_arg",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "rejection",
        term: "bad_arg_shape",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "rejection",
        term: "busy",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "rejection",
        term: "not_in_this_step",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "rejection",
        term: "not_supported",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "rejection",
        term: "free_text_unjudged",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "field",
        term: "confirm.on_wrong",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "field",
        term: "view.card_list",
        version: VocabVersion::V1,
        extension_point: Some("EP-5"),
    },
    VocabTerm {
        kind: "module",
        term: "use",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "module",
        term: "module",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "module",
        term: "args",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "module",
        term: "params",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "module",
        term: "starts",
        version: VocabVersion::V1,
        extension_point: None,
    },
    VocabTerm {
        kind: "field",
        term: "confirm.options[].on_wrong",
        version: VocabVersion::V1_1,
        extension_point: Some("EP-6"),
    },
    VocabTerm {
        kind: "slot",
        term: "input_head",
        version: VocabVersion::V1_1,
        extension_point: Some("EP-7"),
    },
    VocabTerm {
        kind: "slot",
        term: "nearest_command",
        version: VocabVersion::V1_1,
        extension_point: Some("EP-7"),
    },
    VocabTerm {
        kind: "slot",
        term: "step_title",
        version: VocabVersion::V1_1,
        extension_point: Some("EP-7"),
    },
    VocabTerm {
        kind: "slot",
        term: "step_do",
        version: VocabVersion::V1_1,
        extension_point: Some("EP-7"),
    },
    VocabTerm {
        kind: "slot",
        term: "remaining",
        version: VocabVersion::V1_1,
        extension_point: Some("EP-7"),
    },
    VocabTerm {
        kind: "slot",
        term: "total",
        version: VocabVersion::V1_1,
        extension_point: Some("EP-7"),
    },
    VocabTerm {
        kind: "field",
        term: "do",
        version: VocabVersion::V1_1,
        extension_point: Some("EP-7"),
    },
    VocabTerm {
        kind: "field",
        term: "pack.guidance_levels",
        version: VocabVersion::V1_1,
        extension_point: Some("EP-8"),
    },
    VocabTerm {
        kind: "field",
        term: "starts[].guidance",
        version: VocabVersion::V1_1,
        extension_point: Some("EP-8"),
    },
    VocabTerm {
        kind: "field",
        term: "body_levels",
        version: VocabVersion::V1_1,
        extension_point: Some("EP-8"),
    },
    VocabTerm {
        kind: "field",
        term: "hints.levels",
        version: VocabVersion::V1_1,
        extension_point: Some("EP-8"),
    },
    VocabTerm {
        kind: "value",
        term: "guidance=A",
        version: VocabVersion::V1_1,
        extension_point: Some("EP-8"),
    },
    VocabTerm {
        kind: "value",
        term: "guidance=B",
        version: VocabVersion::V1_1,
        extension_point: Some("EP-8"),
    },
    VocabTerm {
        kind: "value",
        term: "guidance=C",
        version: VocabVersion::V1_1,
        extension_point: Some("EP-8"),
    },
    VocabTerm {
        kind: "field",
        term: "starts[].requires",
        version: VocabVersion::V1_2,
        extension_point: Some("EP-9"),
    },
    VocabTerm {
        kind: "field",
        term: "starts[].unmet",
        version: VocabVersion::V1_2,
        extension_point: Some("EP-9"),
    },
    VocabTerm {
        kind: "field",
        term: "pack.profile.command_prefix",
        version: VocabVersion::V1_3,
        extension_point: Some("EP-10"),
    },
    VocabTerm {
        kind: "field",
        term: "pack.profile.input_field",
        version: VocabVersion::V1_3,
        extension_point: Some("EP-10"),
    },
    VocabTerm {
        kind: "value",
        term: "pack.profile.input_field=shown",
        version: VocabVersion::V1_3,
        extension_point: Some("EP-10"),
    },
    VocabTerm {
        kind: "value",
        term: "pack.profile.input_field=hidden",
        version: VocabVersion::V1_3,
        extension_point: Some("EP-10"),
    },
    VocabTerm {
        kind: "field",
        term: "pack.appearance.theme",
        version: VocabVersion::V1_3,
        extension_point: Some("EP-11"),
    },
    VocabTerm {
        kind: "value",
        term: "pack.appearance.theme=standard",
        version: VocabVersion::V1_3,
        extension_point: Some("EP-11"),
    },
    VocabTerm {
        kind: "value",
        term: "pack.appearance.theme=mono",
        version: VocabVersion::V1_3,
        extension_point: Some("EP-11"),
    },
    VocabTerm {
        kind: "value",
        term: "pack.appearance.theme=high_contrast",
        version: VocabVersion::V1_3,
        extension_point: Some("EP-11"),
    },
    VocabTerm {
        kind: "value",
        term: "pack.appearance.theme=reading",
        version: VocabVersion::V1_3,
        extension_point: Some("EP-11"),
    },
    VocabTerm {
        kind: "field",
        term: "pack.left_pane.world_list",
        version: VocabVersion::V1_3,
        extension_point: Some("EP-12"),
    },
    VocabTerm {
        kind: "field",
        term: "pack.left_pane.view_tabs",
        version: VocabVersion::V1_3,
        extension_point: Some("EP-12"),
    },
    VocabTerm {
        kind: "value",
        term: "pack.left_pane.world_list=shown",
        version: VocabVersion::V1_3,
        extension_point: Some("EP-12"),
    },
    VocabTerm {
        kind: "value",
        term: "pack.left_pane.world_list=hidden",
        version: VocabVersion::V1_3,
        extension_point: Some("EP-12"),
    },
    VocabTerm {
        kind: "value",
        term: "pack.left_pane.view_tabs=all",
        version: VocabVersion::V1_3,
        extension_point: Some("EP-12"),
    },
    VocabTerm {
        kind: "value",
        term: "pack.left_pane.view_tabs=declared",
        version: VocabVersion::V1_3,
        extension_point: Some("EP-12"),
    },
    VocabTerm {
        kind: "value",
        term: "pack.left_pane.view_tabs=hidden",
        version: VocabVersion::V1_3,
        extension_point: Some("EP-12"),
    },
    VocabTerm {
        kind: "field",
        term: "confirm.options[].label",
        version: VocabVersion::V1_3,
        extension_point: None,
    },
];

/// 表に（`kind`, `term`）の組があるか。
fn has_term(kind: &str, term: &str) -> bool {
    TERMS.iter().any(|t| t.kind == kind && t.term == term)
}

/// 語彙の `result` の語を判定結果の種類に写す。語彙にある 3 語（`accepted`・`rejected`・`clarified`）だけを受ける。
/// `confirmed`・`progressed`・`ignored` はエンジン側の名前で語彙ではないため `None` を返す。
/// 教材テストの期待の読み取り（TASK-45 2/2）が呼ぶ。
pub fn result_kind(term: &str) -> Option<JudgmentKind> {
    if !has_term("result", term) {
        return None;
    }
    JudgmentKind::ALL.into_iter().find(|k| k.as_str() == term)
}

/// 語彙の `rejection` の語を分類に写す。8 語を受け、`free_text_unjudged` は [`Category::is_rejection`] が偽になる。
/// 大文字・空白・未知の語は `None`。
pub fn rejection_category(term: &str) -> Option<Category> {
    if !has_term("rejection", term) {
        return None;
    }
    Category::from_str_opt(term)
}
