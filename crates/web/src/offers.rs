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
//! 入力方式 `choice` の入力待ち（TASK-450 1/3）: 宣言の選択肢を同じ口（`choice-btn`）で出す。出す条件は
//! 「現在の Step がある・処理中でない・`Entered`・宣言が読めて `check()` に通る」で、選択肢は core の
//! `select_judge::choice_options` だけから得る（web は宣言の JSON を読まない）。欠けたら空にする（fail-closed）。
//! 入力欄・送信は Step の入力方式では出し入れしない（教材全体の `input_field` の設定が決め、描くのは `hello::HelloView`、設定を渡すのは `host`。TASK-450 2/3）。
//!
//! 範囲の境界: 入れ物の識別子と配置は `terminal_area` の版上げ・TASK-517 が決める
//! ため、親の `ul` には `data-testid` を付けない。
//!
//! 承認のボタン（TASK-447 1/2。TERM-27）: 同じ部品に「はい」「いいえ」「直す」と、規則に書かれた対象ごとの
//! 「直す（対象: …）」を足す。出すのは次の 2 つの状態だけである。
//!
//! | 状態 | 宣言 | 出すボタン |
//! |---|---|---|
//! | 入力待ち（`Entered`） | 入力方式 `approval` | 3 つ ＋ 対象つきの「直す」 |
//! | 確認待ち（`Accepted`） | 確認が `approve`、または `confirm` が無い | 3 つ（対象つきは出さない） |
//!
//! 対象の一覧は core の `select_judge::revise_targets` だけから得る（web は規則の JSON を読まない）。現在の Step が無い・
//! 処理中・宣言が読めない・一覧が取れないときは、1 つも出さない（fail-closed）。クリックは `data-action="decide"`
//! （payload は `approve`・`reject`・`revise`）と `data-action="revise-target"`（payload は対象）で、ホストの
//! `decode_action` が `Input::Approval` に変える。対象の照合・拒否の判定は core で、ホストは照合しない。
//! ログの文言の確認と「はい」で先へ進む流れの結線は TASK-447 2/2 が持つ。
//!
//! 選択肢の ID と `label` は教材由来の信頼できない文字列である。表示名は文字ノード、ID は `data-choice` の属性値に
//! だけ入れ、`render` の既定のエスケープに通す（SAFE-11）。生 HTML の経路は使わない。

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_contract::confirm_spec::{answer_spec_of, ConfirmKind};
use fandhe_workshop_contract::input_spec::InputKind;
use fandhe_workshop_core::input::Decision;
use fandhe_workshop_core::select_judge::{choice_options, revise_targets};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::Session;

use crate::artifact_view::{ACTION_ATTR, PAYLOAD_ATTR};
use crate::host::HostModel;
use crate::list_policy::keyed_view_or_notice;
use crate::parts::{DisplayPart, FieldKind, PartField, PartKind};
use crate::terminal_area::{
    decision_value, decision_word, InputPort, CHOICE_ATTR, DECISION_ATTR, REVISE_TARGET_ATTR,
};

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
        .get(&fandhe_workshop_core::instance::root_of(uid))
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

/// 入力方式 `choice` の Step が入力待ちのときの選択肢。条件を満たさなければ空（fail-closed）。
pub fn input_choice_offers(session: &Session) -> Vec<Offer> {
    let state = session.state();
    let Some(uid) = state.run.current.as_ref() else {
        return Vec::new();
    };
    if state.run.busy.is_some() {
        return Vec::new();
    }
    let entered = state
        .run
        .slices
        .get(&fandhe_workshop_core::instance::root_of(uid))
        .and_then(|s| s.status.get(uid))
        .is_some_and(|st| *st == StepStatus::Entered);
    if !entered {
        return Vec::new();
    }
    let Some(node) = session.ir().nodes.iter().find(|n| n.uid == uid.0) else {
        return Vec::new();
    };
    match choice_options(&node.step) {
        Ok(opts) => opts
            .into_iter()
            .map(|o| Offer {
                id: o.id,
                label: o.label,
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// 選択肢のクリックの action 名。`"choice"` は `data-input` の値と紛れるため使わない。
/// 受け手は `host::EngineHost::decode_action` で、`RuntimeAction`（版 1 の契約）には入れない。
pub const ACTION_CHOOSE: &str = "choose";

/// 承認のボタン 1 つ。対象つきの「直す」は `target` を持つ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalOffer {
    /// 承認の操作。
    pub decision: Decision,
    /// 「直す」の対象（教材由来の文字列）。対象なしは `None`。
    pub target: Option<String>,
    /// 入力の段のボタンなら `true`、確認の段なら `false`（keyed list のキーを分ける）。
    pub input_stage: bool,
}

/// 承認のボタンの導出。条件を満たさなければ空（fail-closed）。
pub fn approval_offers(session: &Session) -> Vec<ApprovalOffer> {
    let state = session.state();
    let Some(uid) = state.run.current.as_ref() else {
        return Vec::new();
    };
    if state.run.busy.is_some() {
        return Vec::new();
    }
    let Some(status) = state
        .run
        .slices
        .get(&fandhe_workshop_core::instance::root_of(uid))
        .and_then(|s| s.status.get(uid))
    else {
        return Vec::new();
    };
    let Some(node) = session.ir().nodes.iter().find(|n| n.uid == uid.0) else {
        return Vec::new();
    };
    let step = &node.step;
    let (input_stage, targets) = match status {
        StepStatus::Entered => {
            let is_approval = step
                .input
                .as_ref()
                .and_then(|v| v.get("kind"))
                .and_then(|k| k.as_str())
                .and_then(InputKind::from_str_opt)
                == Some(InputKind::Approval);
            if !is_approval {
                return Vec::new();
            }
            match revise_targets(step) {
                Ok(t) => (true, t),
                Err(_) => return Vec::new(),
            }
        }
        StepStatus::Accepted => {
            // `confirm` が無い Step は core が approve として扱う（`confirm_spec::achievement(None)`）。
            let approve = match step.confirm.as_ref() {
                None => true,
                Some(c) => {
                    c.get("kind")
                        .and_then(|k| k.as_str())
                        .and_then(ConfirmKind::from_str_opt)
                        == Some(ConfirmKind::Approve)
                }
            };
            if !approve {
                return Vec::new();
            }
            (false, Vec::new())
        }
        _ => return Vec::new(),
    };
    let mut out: Vec<ApprovalOffer> = [Decision::Approve, Decision::Reject, Decision::Revise]
        .into_iter()
        .map(|decision| ApprovalOffer {
            decision,
            target: None,
            input_stage,
        })
        .collect();
    // 空・空白だけの対象はボタンにしない（`terminal_area::check_approval` の違反になるため）。
    out.extend(
        targets
            .into_iter()
            .filter(|t| !t.trim().is_empty())
            .map(|t| ApprovalOffer {
                decision: Decision::Revise,
                target: Some(t),
                input_stage,
            }),
    );
    out
}

/// 承認のクリックの action 名。`"approval"` は `data-input` の値と紛れるため使わない。payload は `approve`・`reject`・`revise`。
pub const ACTION_DECIDE: &str = "decide";

/// 対象つきの「直す」のクリックの action 名。payload は対象の文字列で、分解しない。
pub const ACTION_REVISE_TARGET: &str = "revise-target";

/// 承認のボタン 1 つ。見える文字は語（対象なし）か `直す（対象: …）`、対象は属性値にだけ載せる。
pub fn approval_button(decision: Decision, target: Option<&str>) -> Node {
    let word = decision_word(decision);
    match target {
        None => el(
            "button",
            vec![
                ("type", "button"),
                ("data-testid", InputPort::Approval.testid()),
                (DECISION_ATTR, decision_value(decision)),
                (ACTION_ATTR, ACTION_DECIDE),
                (PAYLOAD_ATTR, decision_value(decision)),
            ],
            vec![text(word)],
        ),
        Some(t) => el(
            "button",
            vec![
                ("type", "button"),
                ("data-testid", InputPort::Approval.testid()),
                (DECISION_ATTR, decision_value(Decision::Revise)),
                (REVISE_TARGET_ATTR, t),
                (ACTION_ATTR, ACTION_REVISE_TARGET),
                (PAYLOAD_ATTR, t),
            ],
            vec![text(format!("{word}（対象: {t}）"))],
        ),
    }
}

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
        let mut items: Vec<(String, Node)> = offers
            .iter()
            .enumerate()
            .map(|(i, o)| {
                (
                    format!("s{step}#{i}"),
                    el("li", vec![], vec![choice_button(&o.id, &o.label)]),
                )
            })
            .collect();
        // 入力待ちの選択肢。キーの接頭辞 `e` は、確認の段（接頭辞なし）・承認の `i`・`c` と衝突させない。
        let input_choices = model.session().map(input_choice_offers).unwrap_or_default();
        items.extend(input_choices.iter().enumerate().map(|(i, o)| {
            (
                format!("s{step}#e{i}"),
                el("li", vec![], vec![choice_button(&o.id, &o.label)]),
            )
        }));
        let approvals = model.session().map(approval_offers).unwrap_or_default();
        items.extend(approvals.iter().enumerate().map(|(i, o)| {
            let stage = if o.input_stage { "i" } else { "c" };
            (
                format!("s{step}#{stage}{i}"),
                el(
                    "li",
                    vec![],
                    vec![approval_button(o.decision, o.target.as_deref())],
                ),
            )
        }));
        let attrs = if items.is_empty() {
            vec![("hidden", "")]
        } else {
            vec![]
        };
        keyed_view_or_notice("ul", attrs, &F_OFFERS[0], items)
    }
}
