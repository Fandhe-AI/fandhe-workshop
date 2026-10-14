//! 最小の画面（Hello）。マウントの経路が通ることを示すためだけに存在する。
//!
//! `glue::mount_app` が `Runtime::mount` に渡す。教材の画面に置き換えられるまでの足場で、
//! 入力欄の上限（SAFE-21。TASK-158）の受け口だけを持つ。core・contract に型を足さず、この crate の中で閉じる。

// native ビルドでは glue が無く、試験以外から使われない。
#![cfg_attr(
    not(target_arch = "wasm32"),
    allow(
        dead_code,
        reason = "native ビルドでは glue が無く、試験以外から使われない"
    )
)]

use fandhe_frontend_core::{bind_text, el, text, Node};
use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_frontend_wasm_client::{BindingSource, BoundValue};

use crate::input_limit::{clamp_draft, notice_for};

/// 案内欄が束縛される field 名。
const FIELD_NOTICE: &str = "input_notice";

/// 固定の文言を描画するだけの画面。
///
/// 試験の設定（`test-hooks`）では、keydown の属性が届くことを確かめる受信回数を持つ（TASK-346）。
#[derive(Debug, Default)]
pub struct HelloView {
    /// 入力欄の値。上限（コードポイント）で切った後のもの。
    draft: String,
    /// 直近の入力で上限を超えて切り捨てたか。案内文の表示を決める。
    over_limit: bool,
    /// 直近の `update` で変わった field。上流が束縛点の更新に使う。
    dirty: Vec<&'static str>,
    /// keydown の probe が受け取った回数。配信する設定には存在しない。
    #[cfg(feature = "test-hooks")]
    keydown_probe: u32,
}

/// アクション。配信する設定では値を持たず、Hello は操作を受け付けない。
#[derive(Debug)]
pub enum HelloAction {
    /// 入力欄の値の更新（`data-action-input="draft"`。payload は欄の全文）。
    SetDraft(String),
    /// keydown の probe への入力（`data-action-keydown="keydown-probe"`）。試験の設定のみ。
    #[cfg(feature = "test-hooks")]
    KeydownProbe,
}

impl HelloView {
    /// 画面を作る。
    pub fn new() -> Self {
        Self::default()
    }

    /// 入力欄の値。glue が DOM へ書き戻すために読む。
    pub(crate) fn draft(&self) -> &str {
        &self.draft
    }

    /// 直近の入力で切り捨てが起きたか（試験用の口が読む）。
    #[cfg(feature = "test-hooks")]
    pub(crate) fn over_limit(&self) -> bool {
        self.over_limit
    }

    /// keydown の probe が受け取った回数（試験用の口が読む）。
    #[cfg(feature = "test-hooks")]
    pub(crate) fn keydown_probe(&self) -> u32 {
        self.keydown_probe
    }
}

impl Component for HelloView {
    type Action = HelloAction;

    fn update(&mut self, action: Self::Action) {
        self.dirty.clear();
        match action {
            HelloAction::SetDraft(value) => {
                let (clamped, over) = clamp_draft(&value);
                if over != self.over_limit {
                    self.dirty.push(FIELD_NOTICE);
                }
                self.over_limit = over;
                // 値が変わらない入力でも、上限で切った値の書き戻しは glue が毎回確かめる。
                if self.draft != clamped {
                    self.draft = clamped.to_owned();
                }
            }
            #[cfg(feature = "test-hooks")]
            HelloAction::KeydownProbe => {
                self.keydown_probe = self.keydown_probe.saturating_add(1);
            }
        }
    }

    fn view(&self) -> Node {
        #[cfg_attr(
            not(feature = "test-hooks"),
            allow(unused_mut, reason = "test-hooks のときだけ probe を足す")
        )]
        let mut children = vec![el(
            "p",
            vec![("data-testid", "message")],
            vec![text("Hello")],
        )];
        // フォーカスできる要素にするのは、上流の配線が keydown の target から closest で属性を探すため。
        // フォーカスが無いと target は body になり、何も起きない。
        #[cfg(feature = "test-hooks")]
        children.push(el(
            "button",
            vec![
                ("type", "button"),
                ("data-testid", "keydown-probe"),
                ("data-action-keydown", "keydown-probe"),
                ("data-keys", "Escape"),
            ],
            vec![text("probe")],
        ));
        // maxlength は UTF-16 の単位で数えるため使わない。上限は update が掛ける。
        // 欄に aria は付けない（a11y の契約）。案内欄は role=status で読み上げに乗せる。
        children.push(el(
            "textarea",
            vec![
                ("data-testid", "draft-input"),
                ("data-action-input", "draft"),
            ],
            vec![],
        ));
        children.push(bind_text(
            "p",
            vec![("data-testid", "input-notice"), ("role", "status")],
            FIELD_NOTICE,
            notice_for(self.over_limit),
        ));
        el("main", vec![("data-testid", "hello-root")], children)
    }

    fn decode_action(name: &str, payload: &str) -> Option<Self::Action> {
        if name == "draft" {
            return Some(HelloAction::SetDraft(payload.to_owned()));
        }
        #[cfg(feature = "test-hooks")]
        if name == "keydown-probe" {
            return Some(HelloAction::KeydownProbe);
        }
        None
    }
}

impl DirtyTracked for HelloView {
    fn dirty_fields(&self) -> &[&'static str] {
        &self.dirty
    }
}

impl BindingSource for HelloView {
    fn bound_value(&self, field: &str) -> Option<BoundValue> {
        (field == FIELD_NOTICE).then(|| BoundValue::Text(notice_for(self.over_limit).to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fandhe_frontend_wasm_full::render_component_html;

    #[test]
    fn renders_hello_message() {
        let html = render_component_html(&HelloView::new());
        assert!(html.contains("data-testid=\"hello-root\""), "{html}");
        assert!(html.contains("Hello"), "{html}");
    }

    #[cfg(not(feature = "test-hooks"))]
    #[test]
    fn 配信設定の描画には_keydown_の_probe_が入らない() {
        let html = render_component_html(&HelloView::new());
        assert!(!html.contains("data-action-keydown"), "{html}");
        assert!(!html.contains("keydown-probe"), "{html}");
        assert!(HelloView::decode_action("keydown-probe", "").is_none());
    }

    #[cfg(feature = "test-hooks")]
    #[test]
    fn 試験設定の_probe_は_keydown_の属性を持ち_受信回数を数える() {
        let mut view = HelloView::new();
        let html = render_component_html(&view);
        assert!(
            html.contains("data-action-keydown=\"keydown-probe\""),
            "{html}"
        );
        assert!(html.contains("data-keys=\"Escape\""), "{html}");
        let a = HelloView::decode_action("keydown-probe", "").expect("decoded");
        view.update(a);
        assert_eq!(view.keydown_probe(), 1);
    }

    #[test]
    fn 描画に入力欄と案内欄があり_maxlength_は無い() {
        let html = render_component_html(&HelloView::new());
        assert!(html.contains("data-action-input=\"draft\""), "{html}");
        assert!(html.contains("data-testid=\"input-notice\""), "{html}");
        assert!(html.contains("role=\"status\""), "{html}");
        assert!(!html.contains("maxlength"), "{html}");
    }

    #[test]
    fn 超過と解除で案内が出て消え_dirty_は変化のときだけ() {
        use crate::input_limit::notice_for;
        use fandhe_workshop_contract::limits::MAX_INPUT_CODE_POINTS;
        let mut v = HelloView::new();
        let over = "あ".repeat(MAX_INPUT_CODE_POINTS + 1);
        v.update(HelloAction::SetDraft(over));
        assert_eq!(v.draft().chars().count(), MAX_INPUT_CODE_POINTS);
        assert_eq!(v.dirty_fields(), ["input_notice"]);
        assert_eq!(
            v.bound_value("input_notice"),
            Some(BoundValue::Text(notice_for(true).to_owned()))
        );
        // 超過のまま再入力しても、案内は変わらない。
        v.update(HelloAction::SetDraft(
            "あ".repeat(MAX_INPUT_CODE_POINTS + 5),
        ));
        assert!(v.dirty_fields().is_empty());
        // 1 文字消すと案内が消える。
        v.update(HelloAction::SetDraft(
            "あ".repeat(MAX_INPUT_CODE_POINTS - 1),
        ));
        assert_eq!(v.dirty_fields(), ["input_notice"]);
        assert_eq!(
            v.bound_value("input_notice"),
            Some(BoundValue::Text(String::new()))
        );
    }

    #[test]
    fn unknown_action_is_ignored() {
        assert!(HelloView::decode_action("click", "").is_none());
    }
}
