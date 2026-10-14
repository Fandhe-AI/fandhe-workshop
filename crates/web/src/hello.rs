//! 最小の画面（Hello）。マウントの経路が通ることを示すためだけに存在する。
//!
//! `glue::mount_app` が `Runtime::mount` に渡す。状態もアクションも持たず、教材の画面に
//! 置き換えられるまでの足場である。core・contract に型を足さず、この crate の中で閉じる。

// native ビルドでは glue が無く、試験以外から使われない。
#![cfg_attr(
    not(target_arch = "wasm32"),
    allow(
        dead_code,
        reason = "native ビルドでは glue が無く、試験以外から使われない"
    )
)]

use fandhe_frontend_core::{el, text, Node};
use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_frontend_wasm_client::{BindingSource, BoundValue};

/// 固定の文言を描画するだけの画面。
///
/// 試験の設定（`test-hooks`）では、keydown の属性が届くことを確かめる受信回数を持つ（TASK-346）。
#[derive(Debug, Default)]
pub struct HelloView {
    /// keydown の probe が受け取った回数。配信する設定には存在しない。
    #[cfg(feature = "test-hooks")]
    keydown_probe: u32,
}

/// アクション。配信する設定では値を持たず、Hello は操作を受け付けない。
#[derive(Debug)]
pub enum HelloAction {
    /// keydown の probe への入力（`data-action-keydown="keydown-probe"`）。試験の設定のみ。
    #[cfg(feature = "test-hooks")]
    KeydownProbe,
}

impl HelloView {
    /// 画面を作る。
    pub fn new() -> Self {
        Self::default()
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
        match action {
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
        el("main", vec![("data-testid", "hello-root")], children)
    }

    fn decode_action(_name: &str, _payload: &str) -> Option<Self::Action> {
        #[cfg(feature = "test-hooks")]
        if _name == "keydown-probe" {
            return Some(HelloAction::KeydownProbe);
        }
        None
    }
}

impl DirtyTracked for HelloView {
    fn dirty_fields(&self) -> &[&'static str] {
        &[]
    }
}

impl BindingSource for HelloView {
    fn bound_value(&self, _field: &str) -> Option<BoundValue> {
        None
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
    fn unknown_action_is_ignored() {
        assert!(HelloView::decode_action("click", "").is_none());
    }

    #[test]
    fn view_has_no_undefined_a11y_items() {
        // 教材の画面に置き換わったら check_complete へ切り替える（TASK-517・TASK-450）。
        let view = HelloView::new().view();
        assert_eq!(crate::a11y::check_no_undefined(&view), Ok(()));
    }
}
