//! 3 つの表示（右のログ・左の成果物・復習）と選択肢・承認のボタンの部品（TASK-347 2/2・TASK-445・TASK-447。FEND-2）。
//!
//! 呼び出し元は `host::EngineHost` で、[`standard_registry`] の登録簿を持って `view`・`bound_value` を振り分ける。
//! 部品はどれも `&HostModel` を読むだけで、状態を変える口を持たない。3 つの表示の内容は同じイベント列
//! （`Session::log()`）から core が導いた値（`Projection`・世界状態）だけで決まるため、3 つの表示は
//! 同じ列の別の見え方になる。選択肢の部品（`offers`）は `Session` の現在の Step と IR を読む（`tests/host_contract.rs` が増分と一括の一致を、
//! `tools/browser/three-views.test.mjs` が DOM での一致を確かめるのは 3 つの表示）。
//!
//! 仮のもの（持ち主が別にあるため、ここで確定しない）:
//! - `data-testid`（`part-log`・`part-artifact-view`・`part-review`）・束縛点の名前は、
//!   TASK-431・TASK-467・TASK-511 が決める。ログの行の型は TASK-431 に準拠済み（`log_lines`。TASK-434）で、
//!   識別子 `part-log*` だけが仮のまま残る（`log-list` の引き継ぎは TASK-517）。
//! - 親の一覧 `ul` の `data-testid`（`part-log-list`・`part-review-list`）も同じ持ち主の
//!   仮のものである。左の成果物の親は、ビューごとに `view-list`（`artifact_view::list_attrs`）で確定済みである。親要素の属性は、上流が dirty な更新のたびに `view()` の属性列へ同期し、無い属性を消す
//!   （FEND-15）ため、`view()` に書く。JS で後付けしない。
//! - 状態表示 `run-status` は `LogPart` の中の仮の位置に置く（選択肢の部品は `Offers` として別に登録した）。
//!   配置は TASK-517・TASK-438 が決める。
//! - 受講者の入力文字列は、ログの行（`log_lines`）が `learner_text::learner_part` の文字ノードで描く。
//!   入力欄の送信済み一覧は `learner_text::learner_line` で描く。
//! - 1 リストが 4,096 件を超えるログの分割は EVT-26（TASK-225）の担当である。超えたら
//!   親の `data-bind-list` を保ったまま `data-list-error="limit"` と固定の通知の行を出し、全再描画を避ける
//!   （`list_policy::keyed_view_or_notice`）。
//!
//! 経路・Step の ID は教材由来の信頼できない文字列である。属性値・文字ノードとして `render` の既定の
//! エスケープに通し、生 HTML の経路は使わない（SAFE-11）。

use std::collections::BTreeMap;

use fandhe_frontend_core::{bind_attr_token, bind_text, el, text, Node};
use fandhe_frontend_wasm_client::BoundValue;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::projection::{Mark, MarkKind};
use fandhe_workshop_core::world::World;
use fandhe_workshop_core::Session;

use crate::a11y::{ChangeMark, MARK_ATTR, MARK_GLYPH_TESTID, MARK_TEXT_TESTID, TESTID_ATTR};
use crate::artifact_view::{
    declared_kinds, list_attrs, view_list, ViewKind, ACTION_ATTR, PAYLOAD_ATTR, VIEW_LIST_FIELDS,
};
use crate::host::HostModel;
use crate::list_policy::keyed_view_or_notice;
pub use crate::list_policy::LIST_ERROR_ATTR;
use crate::log_lines::{line_from_event, line_node};
use crate::offers::OffersPart;
use crate::parts::{DisplayPart, FieldKind, PartField, PartKind, PartRegistry};
use crate::run_status::run_status;
use crate::terminal_area::{RUN_STATUS_TESTID, STATE_ATTR};
use crate::view_select::{
    declared_views, view_label, ACTION_SELECT_VIEW, SELECTED_ATTR, VIEW_SELECT_BUTTON_TESTID,
    VIEW_SELECT_FIELD, VIEW_SELECT_TESTID,
};

/// 親の一覧の属性（FEND-15）。`keyed_view` へ渡す `view()` の属性列で、固定値だけにする。
const LOG_LIST_ATTRS: [(&str, &str); 1] = [("data-testid", "part-log-list")];
const REVIEW_LIST_ATTRS: [(&str, &str); 1] = [("data-testid", "part-review-list")];

const F_LOG: &[PartField] = &[
    PartField {
        name: "engine_log_rows",
        kind: FieldKind::KeyedList,
    },
    PartField {
        name: "engine_log_consumed",
        kind: FieldKind::Bound,
    },
    PartField {
        name: "run_status_word",
        kind: FieldKind::Bound,
    },
    PartField {
        name: "run_status_state",
        kind: FieldKind::Bound,
    },
];
/// 左の成果物の束縛点。ビューごとの keyed list 7 本・選択のボタンの list・世界状態の読み終えた長さの 9 件。
const F_ARTIFACT: &[PartField] = &[
    VIEW_LIST_FIELDS[0],
    VIEW_LIST_FIELDS[1],
    VIEW_LIST_FIELDS[2],
    VIEW_LIST_FIELDS[3],
    VIEW_LIST_FIELDS[4],
    VIEW_LIST_FIELDS[5],
    VIEW_LIST_FIELDS[6],
    VIEW_SELECT_FIELD,
    PartField {
        name: "artifact_consumed",
        kind: FieldKind::Bound,
    },
];
/// `F_ARTIFACT` の中の `artifact_consumed` の位置。
const F_ARTIFACT_CONSUMED: usize = 8;
const F_REVIEW: &[PartField] = &[
    PartField {
        name: "review_items",
        kind: FieldKind::KeyedList,
    },
    PartField {
        name: "review_consumed",
        kind: FieldKind::Bound,
    },
];

/// 4 部品（ログ・成果物・復習・選択肢）を登録した標準の登録簿。field は固定で、入力欄の field（`HELLO_FIELDS`）とは衝突しない。
///
/// 登録の拒否は起きない構成だが、`expect` を避けるため、拒否されたら登録済みの分だけを返す（安全側。
/// 試験が 4 件の登録を固定する）。
pub fn standard_registry() -> PartRegistry<HostModel> {
    let mut r = PartRegistry::new();
    let parts: [Box<dyn DisplayPart<HostModel>>; 4] = [
        Box::new(LogPart),
        Box::new(ArtifactPart),
        Box::new(ReviewPart),
        Box::new(OffersPart),
    ];
    for p in parts {
        if r.register(p).is_err() {
            break;
        }
    }
    r
}

/// イベントの種類名。表示と試験の照合に使う固定の名前で、教材の値は含まない。
/// `Event` は `non_exhaustive` のため、足された種類は `other` に倒す。
pub fn event_kind_name(event: &Event) -> &'static str {
    match event {
        Event::Entered { .. } => "entered",
        Event::ModuleEntered { .. } => "module_entered",
        Event::ModuleExited { .. } => "module_exited",
        Event::InputReceived { .. } => "input_received",
        Event::Progress { .. } => "progress",
        Event::Judged { .. } => "judged",
        Event::Accepted { .. } => "accepted",
        Event::Achieved { .. } => "achieved",
        Event::Skipped { .. } => "skipped",
        Event::Transitioned { .. } => "transitioned",
        Event::ChapterCompleted { .. } => "chapter_completed",
        Event::Flag { .. } => "flag",
        Event::Changed { .. } => "changed",
        Event::Stopped { .. } => "stopped",
        Event::Moved { .. } => "moved",
        Event::CheckpointSaved { .. } => "checkpoint_saved",
        Event::Restored { .. } => "restored",
        Event::Begun { .. } => "begun",
        Event::HintOpened { .. } => "hint_opened",
        Event::Reloaded { .. } => "reloaded",
        Event::Refused { .. } => "refused",
        Event::Ignored { .. } => "ignored",
        Event::Warning { .. } => "warning",
        _ => "other",
    }
}

/// 変化の印の種類を、見た目の印（A11Y-22）へ写す。未知の種類は印なし。
pub fn change_mark(kind: MarkKind) -> Option<ChangeMark> {
    match kind {
        MarkKind::Put => Some(ChangeMark::Add),
        MarkKind::Removed => Some(ChangeMark::Remove),
        MarkKind::AttrChanged => Some(ChangeMark::Change),
        _ => None,
    }
}

/// 左の成果物の行（経路の辞書順）。世界状態にある経路と、削除の印が付いた経路を合わせる。
/// 印は世界状態にある経路か削除のものだけを付ける。
pub fn artifact_rows(world: &World, marks: &[Mark]) -> Vec<(String, Option<ChangeMark>)> {
    let mut rows: BTreeMap<&str, Option<ChangeMark>> =
        world.keys().map(|p| (p.as_str(), None)).collect();
    for m in marks {
        let mark = change_mark(m.kind);
        if world.contains_key(&m.path) || m.kind == MarkKind::Removed {
            rows.insert(m.path.as_str(), mark);
        }
    }
    rows.into_iter().map(|(p, m)| (p.to_owned(), m)).collect()
}

fn consumed_text(model: &HostModel) -> String {
    model.projection().consumed().to_string()
}

fn consumed_bound(field: &str, own: &str, model: &HostModel) -> Option<BoundValue> {
    (field == own).then(|| BoundValue::Text(consumed_text(model)))
}

/// 行の `li` に、試験が元のイベントへ辿るための属性（イベントの位置・Step）を足す。
/// 値は 10 進の数字と検証済みの `StepUid` だけで、`terminal_area` は必須の属性しか見ないため違反にならない。
fn with_trace(node: Node, event: u32, step: &str) -> Node {
    match node {
        Node::Element {
            tag,
            mut attrs,
            children,
        } => {
            attrs.push(("data-event".to_owned(), event.to_string()));
            attrs.push(("data-step".to_owned(), step.to_owned()));
            Node::Element {
                tag,
                attrs,
                children,
            }
        }
        other => other,
    }
}

/// 状態表示（`run-status`。TASK-436）。語は文字ノード 1 つ、`data-state` は束縛点で更新する。
///
/// 値は `RunStatus` の固定の語だけで、教材や受講者の文字列を含まない。`data-state` を `aria-*` の束縛へ
/// 替えない（`a11y` の `bound-a11y-attribute` が禁じる。ライブの属性は TASK-366 の持ち分）。
/// 置き場所は仮で、画面の配置（TASK-517・TASK-438）が決めたら移す。
fn run_status_node(model: &HostModel) -> Node {
    let status = run_status(model.session());
    let token = bind_attr_token(STATE_ATTR, F_LOG[3].name);
    bind_text(
        "span",
        vec![
            ("data-testid", RUN_STATUS_TESTID),
            (STATE_ATTR, status.data_state()),
            ("data-bind-attr", token.as_str()),
        ],
        F_LOG[2].name,
        status.word(),
    )
}

/// 右のログ。行にできるイベント 1 件が 1 行で、キーはイベントの位置（追記だけで安定する）。
/// 行の構造は `log_lines`（TASK-431 準拠）が持ち、行にしないイベントは飛ばす。
struct LogPart;

impl DisplayPart<HostModel> for LogPart {
    fn kind(&self) -> PartKind {
        PartKind::Log
    }
    fn fields(&self) -> &'static [PartField] {
        F_LOG
    }
    fn view(&self, model: &HostModel) -> Node {
        let events = model.session().map(|s| s.log()).unwrap_or(&[]);
        let items: Vec<(String, Node)> = model
            .projection()
            .log()
            .iter()
            .filter_map(|row| {
                let line = line_from_event(
                    events.get(row.event as usize)?,
                    model.judged_text(row.event),
                )?;
                let node = line_node(row.at.generation.0, &line)?;
                Some((
                    format!("r{}", row.event),
                    with_trace(node, row.event, row.at.step.0.as_str()),
                ))
            })
            .collect();
        let list = keyed_view_or_notice("ul", LOG_LIST_ATTRS.to_vec(), &F_LOG[0], items);
        el(
            "section",
            vec![("data-testid", "part-log")],
            vec![
                run_status_node(model),
                list,
                bind_text(
                    "span",
                    vec![("data-testid", "part-log-consumed")],
                    F_LOG[1].name,
                    consumed_text(model),
                ),
            ],
        )
    }
    fn bound_value(&self, field: &str, model: &HostModel) -> Option<BoundValue> {
        let status = run_status(model.session());
        match field {
            f if f == F_LOG[2].name => Some(BoundValue::Text(status.word().to_owned())),
            f if f == F_LOG[3].name => Some(BoundValue::Text(status.data_state().to_owned())),
            _ => consumed_bound(field, F_LOG[1].name, model),
        }
    }
}

/// 経路をそのまま行のキーにしてよい長さの上限（バイト）。
const ARTIFACT_KEY_PATH_MAX: usize = 256;

/// 左の成果物。世界状態の経路と、変化の印を並べる。
struct ArtifactPart;

impl DisplayPart<HostModel> for ArtifactPart {
    fn kind(&self) -> PartKind {
        PartKind::ArtifactView
    }
    fn fields(&self) -> &'static [PartField] {
        F_ARTIFACT
    }
    fn view(&self, model: &HostModel) -> Node {
        let selected = model.selected_view();
        let items: Vec<(String, Node)> = artifact_rows(model.world(), model.projection().marks())
            .into_iter()
            .enumerate()
            .map(|(i, (path, mark))| {
                let value = mark.map_or("", ChangeMark::data_value);
                let node = el(
                    "li",
                    vec![("data-path", path.as_str()), (MARK_ATTR, value)],
                    vec![
                        el(
                            "span",
                            vec![("data-testid", MARK_GLYPH_TESTID)],
                            vec![text(mark.map_or("", ChangeMark::glyph))],
                        ),
                        el(
                            "span",
                            vec![("data-testid", MARK_TEXT_TESTID)],
                            vec![text(mark.map_or("", ChangeMark::text))],
                        ),
                        el("span", vec![], vec![text(path.as_str())]),
                    ],
                );
                // 経路は一意（BTreeMap のキー）なので、挿入で後続の行の同一性がずれないよう経路をキーにする。
                // 教材由来の長い文字列はキーに使わず、位置のキーへ退く（接頭辞が違うので衝突しない）。
                let key = if path.len() <= ARTIFACT_KEY_PATH_MAX {
                    format!("p{path}")
                } else {
                    format!("i{i}")
                };
                (key, node)
            })
            .collect();
        // 選択のボタン。宣言が 2 種以上のときだけ出す（ART-12）。静的な子にせず keyed list に置くのは、
        // 選択だけの更新で dirty な field を束縛点に解決するため（上流は解決できないと全再描画にする）。
        let buttons = model
            .session()
            .map_or_else(Vec::new, |s| select_buttons(s, selected));
        let select_attrs = if buttons.is_empty() {
            vec![(TESTID_ATTR, VIEW_SELECT_TESTID), ("hidden", "")]
        } else {
            vec![(TESTID_ATTR, VIEW_SELECT_TESTID)]
        };
        let select_list = keyed_view_or_notice("ul", select_attrs, &VIEW_SELECT_FIELD, buttons);
        // 行は選ばれたビューの list だけに入れ、他の 6 本は空で hidden にする。
        let mut children = vec![select_list];
        let mut rows = Some(items);
        for kind in ViewKind::ALL {
            let is_selected = kind == selected;
            let items = if is_selected {
                rows.take().unwrap_or_default()
            } else {
                Vec::new()
            };
            // 上限超過などの `Err` も同じ属性で「行 0 件の keyed list」に倒す（`data-bind-list` を保つ）。
            let list = view_list("ul", kind, is_selected, items).unwrap_or_else(|_| {
                keyed_view_or_notice(
                    "ul",
                    list_attrs(kind, is_selected),
                    &kind.list_part_field(),
                    Vec::new(),
                )
            });
            children.push(list);
        }
        children.push(bind_text(
            "span",
            vec![("data-testid", "part-artifact-consumed")],
            F_ARTIFACT[F_ARTIFACT_CONSUMED].name,
            model.world_consumed().to_string(),
        ));
        el(
            "section",
            vec![("data-testid", "part-artifact-view")],
            children,
        )
    }
    fn bound_value(&self, field: &str, model: &HostModel) -> Option<BoundValue> {
        // 世界状態の導出が読み終えたログの長さ。ログ・復習の値（Projection の consumed）と一致する。
        (field == F_ARTIFACT[F_ARTIFACT_CONSUMED].name)
            .then(|| BoundValue::Text(model.world_consumed().to_string()))
    }
}

/// 選択のボタンの項目。宣言が 2 種以上のときだけ返す。文字・属性値は閉じた集合の固定値だけ（SAFE-11）。
fn select_buttons(session: &Session, selected: ViewKind) -> Vec<(String, Node)> {
    let views = declared_views(session);
    let declared = declared_kinds(&views);
    if declared.len() < 2 {
        return Vec::new();
    }
    declared
        .iter()
        .map(|(kind, _)| {
            let flag = if *kind == selected { "true" } else { "false" };
            let button = el(
                "button",
                vec![
                    ("type", "button"),
                    (TESTID_ATTR, VIEW_SELECT_BUTTON_TESTID),
                    (ACTION_ATTR, ACTION_SELECT_VIEW),
                    (PAYLOAD_ATTR, kind.as_str()),
                    (SELECTED_ATTR, flag),
                ],
                vec![text(view_label(*kind))],
            );
            (
                format!("t{}", kind.as_str()),
                el("li", vec![], vec![button]),
            )
        })
        .collect()
}

/// 復習。Step 単位の項目で、達成・省略と変化の数・経路を出す。
struct ReviewPart;

impl DisplayPart<HostModel> for ReviewPart {
    fn kind(&self) -> PartKind {
        PartKind::Review
    }
    fn fields(&self) -> &'static [PartField] {
        F_REVIEW
    }
    fn view(&self, model: &HostModel) -> Node {
        let items: Vec<(String, Node)> = model
            .projection()
            .review()
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let count = item.changes.len().to_string();
                let paths = item
                    .changes
                    .iter()
                    .map(|c| c.path.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let node = el(
                    "li",
                    vec![
                        ("data-step", item.step.0.as_str()),
                        (
                            "data-achieved",
                            if item.achieved { "true" } else { "false" },
                        ),
                        ("data-skipped", if item.skipped { "true" } else { "false" }),
                        ("data-change-count", count.as_str()),
                    ],
                    vec![text(paths)],
                );
                (format!("v{i}"), node)
            })
            .collect();
        let list = keyed_view_or_notice("ul", REVIEW_LIST_ATTRS.to_vec(), &F_REVIEW[0], items);
        el(
            "section",
            vec![("data-testid", "part-review")],
            vec![
                list,
                bind_text(
                    "span",
                    vec![("data-testid", "part-review-consumed")],
                    F_REVIEW[1].name,
                    consumed_text(model),
                ),
            ],
        )
    }
    fn bound_value(&self, field: &str, model: &HostModel) -> Option<BoundValue> {
        consumed_bound(field, F_REVIEW[1].name, model)
    }
}
