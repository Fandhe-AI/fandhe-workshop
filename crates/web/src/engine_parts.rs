//! 3 つの表示（右のログ・左の成果物・復習）の部品（TASK-347 2/2。FEND-2）。
//!
//! 呼び出し元は `host::EngineHost` で、[`standard_registry`] の登録簿を持って `view`・`bound_value` を振り分ける。
//! 3 部品はどれも `&HostModel` を読むだけで、状態を変える口を持たない。内容は同じイベント列
//! （`Session::log()`）から core が導いた値（`Projection`・世界状態）だけで決まるため、3 つの表示は
//! 同じ列の別の見え方になる（`tests/host_contract.rs` が増分と一括の一致を、
//! `tools/browser/three-views.test.mjs` が DOM での一致を確かめる）。
//!
//! 仮のもの（持ち主が別にあるため、ここで確定しない）:
//! - `data-testid`（`part-log`・`part-artifact-view`・`part-review`）・束縛点の名前は、
//!   TASK-431・TASK-467・TASK-511 が決める。ログの行の型は TASK-431 に準拠済み（`log_lines`。TASK-434）で、
//!   識別子 `part-log*` だけが仮のまま残る（`log-list` の引き継ぎは TASK-517）。
//! - 親の一覧 `ul` の `data-testid`（`part-log-list`・`part-artifact-list`・`part-review-list`）も同じ持ち主の
//!   仮のものである。親要素の属性は、上流が dirty な更新のたびに `view()` の属性列へ同期し、無い属性を消す
//!   （FEND-15）ため、`view()` に書く。JS で後付けしない。
//! - 受講者の入力文字列は、ログの行（`log_lines`）が `learner_text::learner_part` の文字ノードで描く。
//!   入力欄の送信済み一覧は `learner_text::learner_line` で描く。
//! - 1 リストが 4,096 件を超えるログの分割は EVT-26（TASK-225）の担当である。超えたら
//!   親の `data-bind-list` を保ったまま `data-list-error="limit"` と固定の通知の行を出し、全再描画を避ける
//!   （`list_policy::keyed_view_or_notice`）。
//!
//! 経路・Step の ID は教材由来の信頼できない文字列である。属性値・文字ノードとして `render` の既定の
//! エスケープに通し、生 HTML の経路は使わない（SAFE-11）。

use std::collections::BTreeMap;

use fandhe_frontend_core::{bind_text, el, text, Node};
use fandhe_frontend_wasm_client::BoundValue;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::projection::{Mark, MarkKind};
use fandhe_workshop_core::world::World;

use crate::a11y::{ChangeMark, MARK_ATTR, MARK_GLYPH_TESTID, MARK_TEXT_TESTID};
use crate::host::HostModel;
use crate::list_policy::keyed_view_or_notice;
pub use crate::list_policy::LIST_ERROR_ATTR;
use crate::log_lines::{line_from_event, line_node};
use crate::parts::{DisplayPart, FieldKind, PartField, PartKind, PartRegistry};

/// 親の一覧の属性（FEND-15）。`keyed_view` へ渡す `view()` の属性列で、固定値だけにする。
const LOG_LIST_ATTRS: [(&str, &str); 1] = [("data-testid", "part-log-list")];
const ARTIFACT_LIST_ATTRS: [(&str, &str); 1] = [("data-testid", "part-artifact-list")];
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
];
const F_ARTIFACT: &[PartField] = &[
    PartField {
        name: "artifact_items",
        kind: FieldKind::KeyedList,
    },
    PartField {
        name: "artifact_consumed",
        kind: FieldKind::Bound,
    },
];
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

/// 3 部品を登録した標準の登録簿。field は固定で、入力欄の field（`HELLO_FIELDS`）とは衝突しない。
///
/// 登録の拒否は起きない構成だが、`expect` を避けるため、拒否されたら登録済みの分だけを返す（安全側。
/// 試験が 3 件の登録を固定する）。
pub fn standard_registry() -> PartRegistry<HostModel> {
    let mut r = PartRegistry::new();
    let parts: [Box<dyn DisplayPart<HostModel>>; 3] = [
        Box::new(LogPart),
        Box::new(ArtifactPart),
        Box::new(ReviewPart),
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
                let line = line_from_event(events.get(row.event as usize)?)?;
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
        consumed_bound(field, F_LOG[1].name, model)
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
        let list = keyed_view_or_notice("ul", ARTIFACT_LIST_ATTRS.to_vec(), &F_ARTIFACT[0], items);
        el(
            "section",
            vec![("data-testid", "part-artifact-view")],
            vec![
                list,
                bind_text(
                    "span",
                    vec![("data-testid", "part-artifact-consumed")],
                    F_ARTIFACT[1].name,
                    model.world_consumed().to_string(),
                ),
            ],
        )
    }
    fn bound_value(&self, field: &str, model: &HostModel) -> Option<BoundValue> {
        // 世界状態の導出が読み終えたログの長さ。ログ・復習の値（Projection の consumed）と一致する。
        (field == F_ARTIFACT[1].name).then(|| BoundValue::Text(model.world_consumed().to_string()))
    }
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
