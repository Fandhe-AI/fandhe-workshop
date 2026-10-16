//! 最小の画面（Hello）。マウントの経路が通ることを示すためだけに存在する。
//!
//! `glue::mount_app` が `Runtime::mount` に渡す。教材の画面に置き換えられるまでの足場で、
//! 入力欄の上限（SAFE-21。TASK-158）の受け口と、送信と送信済みの一覧（受講者の文字列の非解釈の表示。
//! SAFE-11。TASK-150）の足場だけを持つ。core・contract に型を足さず、この crate の中で閉じる。

// native ビルドでは glue が無く、試験以外から使われない。
#![cfg_attr(
    not(target_arch = "wasm32"),
    allow(
        dead_code,
        reason = "native ビルドでは glue が無く、試験以外から使われない"
    )
)]

use std::collections::VecDeque;

use fandhe_frontend_core::keyed::keyed_list;
use fandhe_frontend_core::{bind_text, el, text, Node};
use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_frontend_wasm_client::{BindingSource, BoundValue};

use crate::a11y::LOG_LIST_TESTID;
use crate::input_limit::{clamp_draft, notice_for};
use crate::learner_text::learner_line;

/// 案内欄が束縛される field 名。
const FIELD_NOTICE: &str = "input_notice";

/// 送信済みの一覧が束縛される field 名（`keyed_list` の差分更新の対象）。
const FIELD_LOG: &str = "learner_log";

/// 送信済みの一覧に残す最大の件数。超えたら古い順に捨てる。
/// `keyed_list` の上限（4,096 件）に届かない値にして、`Err` を通常の経路にしない。
const MAX_LEARNER_ENTRIES: usize = 256;

/// 固定の文言を描画するだけの画面。
///
/// 試験の設定（`test-hooks`）では、keydown の属性が届くことを確かめる受信回数を持つ（TASK-346）。
#[derive(Debug, Default)]
pub struct HelloView {
    /// 入力欄の値。上限（コードポイント）で切った後のもの。
    draft: String,
    /// 直近の入力で上限を超えて切り捨てたか。案内文の表示を決める。
    over_limit: bool,
    /// 送信済みの本文と、単調に増える通し番号（一覧の項目のキーになる）。
    entries: VecDeque<(u64, String)>,
    /// 次に振る通し番号。
    next_seq: u64,
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
    /// 送信（`data-action="submit"`）。本文は状態の下書きから取り、DOM は読まない。
    Submit,
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
            HelloAction::Submit => {
                if self.draft.is_empty() {
                    return;
                }
                self.entries.push_back((self.next_seq, self.draft.clone()));
                self.next_seq = self.next_seq.saturating_add(1);
                while self.entries.len() > MAX_LEARNER_ENTRIES {
                    self.entries.pop_front();
                }
                self.dirty.push(FIELD_LOG);
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
            // 再描画で欄が作り直されても、状態の下書きを初期内容として復元する。
            vec![text(&self.draft)],
        ));
        children.push(bind_text(
            "p",
            vec![("data-testid", "input-notice"), ("role", "status")],
            FIELD_NOTICE,
            notice_for(self.over_limit),
        ));
        children.push(el(
            "button",
            vec![
                ("type", "button"),
                ("data-testid", "submit-input"),
                ("data-action", "submit"),
            ],
            vec![text("送信")],
        ));
        // 件数が 0 でも常に keyed_list で描く（最初の送信から差分の経路に乗せるため）。
        // Err は空の一覧に倒す（expect は wasm を太らせ、panic は画面を壊す）。
        let items: Vec<(String, Node)> = self
            .entries
            .iter()
            .map(|(seq, body)| (format!("e{seq}"), learner_line(body)))
            .collect();
        let attrs = vec![("data-testid", LOG_LIST_TESTID)];
        children.push(match keyed_list("ol", attrs.clone(), FIELD_LOG, items) {
            Ok(list) => list,
            Err(_) => el("ol", attrs, vec![]),
        });
        el("main", vec![("data-testid", "hello-root")], children)
    }

    fn decode_action(name: &str, payload: &str) -> Option<Self::Action> {
        if name == "draft" {
            return Some(HelloAction::SetDraft(payload.to_owned()));
        }
        if name == "submit" {
            return Some(HelloAction::Submit);
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
    fn 入力欄は下書きを初期内容として描画する() {
        let mut v = HelloView::new();
        v.update(HelloAction::SetDraft("下書き<b>".to_owned()));
        let html = render_component_html(&v);
        assert!(html.contains("下書き&lt;b&gt;</textarea>"), "{html}");
    }

    #[test]
    fn 下書きが空の送信は一覧も_dirty_も変えない() {
        let mut v = HelloView::new();
        v.update(HelloAction::Submit);
        assert!(v.entries.is_empty());
        assert!(v.dirty_fields().is_empty());
    }

    #[test]
    fn 送信で一覧が増え_dirty_が立ち_下書きは残る() {
        let mut v = HelloView::new();
        v.update(HelloAction::SetDraft("a".to_owned()));
        v.update(HelloAction::Submit);
        assert_eq!(v.entries.len(), 1);
        assert_eq!(v.dirty_fields(), [FIELD_LOG]);
        assert_eq!(v.draft(), "a");
    }

    #[test]
    fn 複数行の下書きは改行を保って一覧に積まれる() {
        // JUDGE-13・S-29: 改行は trim も分割もせず、文字ノードの `\n` として残る。
        let body = "一行目のメモ\n二行目のメモ";
        let mut v = HelloView::new();
        v.update(HelloAction::SetDraft(body.to_owned()));
        v.update(HelloAction::Submit);
        assert_eq!(v.entries.back().map(|e| e.1.as_str()), Some(body));
        let html = render_component_html(&v);
        assert!(html.contains(body), "{html}");
    }

    #[test]
    fn 貼り付けた複数行の下書きは改行を保ち_貼り付けだけでは一覧が増えない() {
        // JUDGE-25: 貼り付けは input 経由の SetDraft で入る。値は加工されず、送信は Submit だけが起こす。
        for body in ["a\nb\nc\n", "x\ty\n\tz", "/help\n/list\n/quit"] {
            let mut v = HelloView::new();
            v.update(HelloAction::SetDraft(body.to_owned()));
            assert!(v.entries.is_empty(), "{body:?}");
            v.update(HelloAction::Submit);
            assert_eq!(v.entries.len(), 1);
            assert_eq!(v.entries.back().map(|e| e.1.as_str()), Some(body));
        }
    }

    #[test]
    fn 一覧は上限を超えると古い順に捨て_キーは重複しない() {
        let mut v = HelloView::new();
        v.update(HelloAction::SetDraft("a".to_owned()));
        for _ in 0..=MAX_LEARNER_ENTRIES {
            v.update(HelloAction::Submit);
        }
        assert_eq!(v.entries.len(), MAX_LEARNER_ENTRIES);
        assert_eq!(v.entries.front().map(|e| e.0), Some(1));
        let html = render_component_html(&v);
        assert!(!html.contains("data-key=\"e0\""), "{html}");
        assert!(html.contains("data-key=\"e256\""), "{html}");
    }

    #[test]
    fn 記法を含む本文は記法の文字のまま描かれ_要素にならない() {
        let mut v = HelloView::new();
        let body = "# 見出し\n[x](https://example.invalid/a) <b>太字</b> `code` **強調**";
        v.update(HelloAction::SetDraft(body.to_owned()));
        v.update(HelloAction::Submit);
        let html = render_component_html(&v);
        assert!(html.contains("# 見出し"), "{html}");
        assert!(html.contains("[x](https://example.invalid/a)"), "{html}");
        assert!(html.contains("&lt;b&gt;太字&lt;/b&gt;"), "{html}");
        for tag in ["<a ", "<img", "<h1", "<code", "<strong", "<b>"] {
            assert!(!html.contains(tag), "{tag}: {html}");
        }
    }

    #[test]
    fn 一覧に項目があっても_a11y_の未定義は無い() {
        let mut v = HelloView::new();
        v.update(HelloAction::SetDraft("a".to_owned()));
        v.update(HelloAction::Submit);
        assert_eq!(crate::a11y::check_no_undefined(&v.view()), Ok(()));
    }

    #[test]
    fn submit_は_payload_を無視して_submit_になる() {
        assert!(matches!(
            HelloView::decode_action("submit", "任意"),
            Some(HelloAction::Submit)
        ));
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
