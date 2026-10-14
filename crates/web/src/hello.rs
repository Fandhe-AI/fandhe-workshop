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
#[derive(Debug, Default)]
pub struct HelloView;

/// 値を持たないアクション。Hello は操作を受け付けない。
#[derive(Debug)]
pub enum HelloAction {}

impl HelloView {
    /// 画面を作る。
    pub fn new() -> Self {
        Self
    }
}

impl Component for HelloView {
    type Action = HelloAction;

    fn update(&mut self, action: Self::Action) {
        match action {}
    }

    fn view(&self) -> Node {
        el(
            "main",
            vec![("data-testid", "hello-root")],
            vec![el(
                "p",
                vec![("data-testid", "message")],
                vec![text("Hello")],
            )],
        )
    }

    fn decode_action(_name: &str, _payload: &str) -> Option<Self::Action> {
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
