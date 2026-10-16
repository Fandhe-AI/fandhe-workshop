//! `answer` の選択肢のボタン（TASK-445 1/2。TERM-24）。
//!
//! 呼び出し元は `engine_parts::standard_registry` で、右の操作エリアに「確認が `answer` の Step が確認待ちのとき」
//! だけ選択肢を表示名のボタンとして描く。見える文字は contract の `AnswerOption::display_name`（`label` が
//! 空白以外ならその値、無ければ ID）で、`label` のある選択肢では ID を文字として出さない。
//!
//! 表示する条件は core の `accept::phase` の読み方に合わせる（確認待ち = `StepStatus::Accepted` かつ処理中でない）。
//! 受付の前や処理中に答えを先に見せないため、どれか 1 つでも欠けたら空の一覧にする（fail-closed）。
//! 宣言が読めない・`check()` に通らない場合も同じで、警告の文面は出さない。
//!
//! クリックの結線（TASK-445 2/2）: ボタンは `data-action="choose"`・`data-payload=<ID>` を持ち、ホストの
//! `decode_action` が `Op::Input(Input::Choice(ID))` に変えて core へ渡す。誤答の拒否・選び直し・達成は core の判定
//! （`judge.rs`）の結果で、ホストは判定も「いま出ている選択肢か」の照合もしない。`choice_button` は入力方式 `choice`
//! の段階（TASK-450）でも使い回す前提で、同じ `Input::Choice` を確認の段階と入力の段階のどちらで扱うかは core が
//! Step の状態で振り分ける。ホストに確認の段階だけの照合を置くと、その再利用が黙って壊れる。偽造・古い ID は core が
//! 拒否か無視に倒す。
//!
//! 範囲の境界: 入力方式 `choice` の Step の口は TASK-450 が持つ。入れ物の識別子と配置は `terminal_area` の版上げ・TASK-517 が決める
//! ため、親の `ul` には `data-testid` を付けない。
//!
//! 選択肢の ID と `label` は教材由来の信頼できない文字列である。表示名は文字ノード、ID は `data-choice` の属性値に
//! だけ入れ、`render` の既定のエスケープに通す（SAFE-11）。生 HTML の経路は使わない。

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_contract::confirm_spec::answer_spec_of;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::Session;

use crate::artifact_view::{ACTION_ATTR, PAYLOAD_ATTR};
use crate::host::HostModel;
use crate::list_policy::keyed_view_or_notice;
use crate::parts::{DisplayPart, FieldKind, PartField, PartKind};
use crate::terminal_area::{InputPort, CHOICE_ATTR};

/// 選択肢の一覧が束縛される field 名（`keyed_list` の差分更新の対象）。
pub const FIELD_OFFERS: &str = "offer_choices";

const F_OFFERS: &[PartField] = &[PartField {
    name: FIELD_OFFERS,
    kind: FieldKind::KeyedList,
}];

/// 表示する選択肢 1 つ。ID は入力の宛先、`label` は見える文字（表示名）である。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// 選択肢の ID。
    pub id: String,
    /// 見える文字（`display_name` の値）。
    pub label: String,
}

/// 現在の Step が確認待ちで、確認が `answer` のときの選択肢。条件を満たさなければ空。
pub fn answer_offers(session: &Session) -> Vec<Offer> {
    let state = session.state();
    let Some(uid) = state.run.current.as_ref() else {
        return Vec::new();
    };
    if state.run.busy.is_some() {
        return Vec::new();
    }
    let awaiting = state
        .run
        .slices
        .get(&uid.root())
        .and_then(|s| s.status.get(uid))
        .is_some_and(|st| *st == StepStatus::Accepted);
    if !awaiting {
        return Vec::new();
    }
    let Some(node) = session.ir().nodes.iter().find(|n| n.uid == uid.0) else {
        return Vec::new();
    };
    let Some(spec) = node.step.confirm.as_ref().and_then(answer_spec_of) else {
        return Vec::new();
    };
    if spec.check().is_err() {
        return Vec::new();
    }
    spec.options
        .iter()
        .map(|o| Offer {
            id: o.id.clone(),
            label: o.display_name().to_owned(),
        })
        .collect()
}

/// 選択肢のクリックの action 名。`"choice"` は `data-input` の値と紛れるため使わない。
/// 受け手は `host::EngineHost::decode_action` で、`RuntimeAction`（版 1 の契約）には入れない。
pub const ACTION_CHOOSE: &str = "choose";

/// 選択肢のボタン 1 つ。文字は表示名だけ、ID は `data-choice`・`data-payload` の属性値にだけ載せる。
/// IR に依存しない形にして、入力方式 `choice` の口（TASK-450）も使い回せるようにする。
pub fn choice_button(id: &str, label: &str) -> Node {
    el(
        "button",
        vec![
            ("type", "button"),
            ("data-testid", InputPort::Choice.testid()),
            (CHOICE_ATTR, id),
            (ACTION_ATTR, ACTION_CHOOSE),
            (PAYLOAD_ATTR, id),
        ],
        vec![text(label)],
    )
}

/// 選択肢の部品。
pub(crate) struct OffersPart;

impl DisplayPart<HostModel> for OffersPart {
    fn kind(&self) -> PartKind {
        PartKind::Offers
    }
    fn fields(&self) -> &'static [PartField] {
        F_OFFERS
    }
    fn view(&self, model: &HostModel) -> Node {
        let offers = model.session().map(answer_offers).unwrap_or_default();
        let step = model
            .session()
            .and_then(|s| s.state().run.current.as_ref())
            .map_or("", |u| u.0.as_str());
        let items: Vec<(String, Node)> = offers
            .iter()
            .enumerate()
            .map(|(i, o)| {
                (
                    format!("s{step}#{i}"),
                    el("li", vec![], vec![choice_button(&o.id, &o.label)]),
                )
            })
            .collect();
        let attrs = if items.is_empty() {
            vec![("hidden", "")]
        } else {
            vec![]
        };
        keyed_view_or_notice("ul", attrs, &F_OFFERS[0], items)
    }
}
