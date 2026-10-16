//! エンジンを持つホストの Component（TASK-347 1/2。FEND-2）。`glue::mount_app` が `Runtime::mount` に渡す。
//!
//! `Runtime::mount` は Component を 1 つだけ受け取るため、表示部品（TASK-343 の [`DisplayPart`]）は個別に
//! mount されず、ホストが [`PartRegistry`] を持って `view`・`dirty_fields`・`bound_value` を振り分ける。
//! 既存の入力欄（[`HelloView`]）は、ホストの入力欄の部分として内包する。
//!
//! 表示層は進行管理を持たない。状態を変える経路は、[`EngineHost`] の `update` から core の `Session::apply`
//! を呼ぶ 1 本だけである（`tests/no_progress.rs` が静的に固定し、`tests/host_contract.rs` が core の純関数との
//! 一致で確かめる）。世代・Step の状態・処理中かどうかは core が持ち、ホストは判定も変更もしない。
//!
//! dirty field の求め方は、束縛点（Bound）は操作の前後で値が変わったものだけ、keyed list は
//! イベントが返った（またはログの長さが変わった）ときだけである（FEND-3）。
//! 3 つの表示（ログ・左の成果物・復習。`engine_parts`）は `new()` が登録する標準の登録簿に入る（TASK-347 2/2）。
//! 3 部品は同じイベント列（`Session::log()`）から導いた値だけを読む。`tick`・`complete` の action は
//! `Runtime::dispatch_action` から届く（TASK-349。`decode_action` が `action::RuntimeAction` の検査を通す）。
//! 送信（Enter・送信ボタン。TASK-438）は、入力欄の一覧に積んだ後、操作プロファイルの接頭辞で分類して
//! `Op::Input` として `apply_engine` に渡す。下書きは、エンジンが拒否したとき以外は空にし、`<textarea>` への
//! 書き戻しは `set_draft_sync_request` の通知先に頼む（上流は `<textarea>` の値を同期しないため）。
//! 進行の宛先は、`update` の後に `set_tick_request` の通知先へ渡す（表示側のタイマーが予約する）。試験の設定（`test-hooks`）だけ、`raw` の action（`Op` の JSON）・見えない `raw-trigger`・
//! 状態の書き出し（`engine_snapshot_json`）・試験用の教材の読み込み（`for_mount`）を持つ（BUILD-27・SHELL-35）。
//!
//! [`DisplayPart`]: crate::parts::DisplayPart

// native ビルドでは glue が無く、試験以外から使われない。
#![cfg_attr(
    not(target_arch = "wasm32"),
    allow(
        dead_code,
        reason = "native ビルドでは glue が無く、試験以外から使われない"
    )
)]

use core::fmt;

#[cfg(feature = "test-hooks")]
use fandhe_frontend_core::text;
use fandhe_frontend_core::{el, Node};
use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_frontend_wasm_client::{BindingSource, BoundValue};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::Judgment;
use fandhe_workshop_core::addr::TickAddr;
use fandhe_workshop_core::changed::replay_changes;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_replay::submit_input;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::world::World;
use fandhe_workshop_core::{Projection, Session};

use crate::action::RuntimeAction;
use crate::candidates;
use crate::engine_parts::standard_registry;
use crate::hello::{HelloAction, HelloView, HELLO_FIELDS};
use crate::parts::{FieldKind, PartRegistry};
use crate::tick_schedule::next_tick;

/// 部品が読む表示用の値。読み出しの口だけを持ち、可変の口は持たない。
#[derive(Debug)]
pub struct HostModel {
    /// 実行環境。`None` は IR が未読込であることを表す。
    session: Option<Session>,
    /// ログから導いた表示用の値。
    projection: Projection,
    /// ログの `Changed` だけから組み立てた世界状態（P-1b。左の成果物の元）。
    world: World,
    /// `world` が読み終えたログの長さ。
    world_consumed: usize,
}

impl HostModel {
    /// 実行環境。未読込なら `None`。
    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    /// ログから導いた表示用の値。
    pub fn projection(&self) -> &Projection {
        &self.projection
    }

    /// ログの `Changed` から導いた世界状態。`Session` の状態は読まない（イベント列だけで左が描けることの確認）。
    pub fn world(&self) -> &World {
        &self.world
    }

    /// 世界状態の導出が読み終えたログの長さ。
    pub fn world_consumed(&self) -> usize {
        self.world_consumed
    }
}

/// ホストが受ける action。
#[derive(Debug)]
pub enum HostAction {
    /// 入力欄の action。
    Input(HelloAction),
    /// core への操作。
    Engine(Op),
    /// 補完（FEND-4）。挿入は TASK-440 2/2（#332）が足すまで、状態を変えない。
    Complete,
}

/// 次の進行の宛先の通知先。`apply_engine` の後に、次に届けるべき進行（無ければ `None`）を渡す。
///
/// glue が `set_timeout` の予約に使う。ホストは予約の状態を持たず、重複の抑止も glue が受け持つ。
pub type TickRequest = Box<dyn Fn(Option<TickAddr>)>;

/// 下書きの書き戻しの要求の通知先。送信で下書きを空にしたとき、`update` の中から呼ばれる。
///
/// glue が `<textarea>` の値の同期（タイマー経由）に使う。通知先は Runtime にも App にも触れてはならない（再入になる）。
pub type DraftSyncRequest = Box<dyn Fn()>;

/// 送信の後に下書きを残すか。エンジンが拒否したときだけ残す（TERM-1・JUDGE-5）。
///
/// 問い返しの後の扱いは JUDGE-27・TERM-22（検討中）に属するため、ここでは固定しない。
/// 拒否でなければ空にする。
pub(crate) fn keeps_draft(rejected: bool) -> bool {
    rejected
}

/// 構築の拒否の理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostError {
    /// 部品の field 名がホスト自身の field と衝突した。
    FieldCollision(&'static str),
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HostError::FieldCollision(n) => write!(f, "field collides with the host: {n}"),
        }
    }
}

impl std::error::Error for HostError {}

/// mount するホスト。
pub struct EngineHost {
    /// 部品が読む値。
    model: HostModel,
    /// 表示部品の登録簿。
    registry: PartRegistry<HostModel>,
    /// 既存の入力欄。
    input: HelloView,
    /// 直近の `update` で変わった field。
    dirty: Vec<&'static str>,
    /// 次の進行の宛先の通知先。未設定（native の試験など）なら通知しない。
    tick_request: Option<TickRequest>,
    /// 下書きの書き戻しの通知先。未設定なら通知しない。
    draft_sync_request: Option<DraftSyncRequest>,
    /// 操作プロファイルのコマンド接頭辞。session が無い・読めないときは `None`（送信をエンジンへ送らない）。
    command_prefix: Option<String>,
    /// 補完の action を受け取った回数（試験用の口が読む）。
    #[cfg(feature = "test-hooks")]
    complete_count: u32,
}

impl fmt::Debug for EngineHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EngineHost")
            .field("model", &self.model)
            .field("parts", &self.registry.len())
            .finish_non_exhaustive()
    }
}

impl Default for EngineHost {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineHost {
    /// IR なし・標準の登録簿（3 つの表示）のホストを作る。未読込のときの部品は空の表示になる。
    /// ブラウザでの IR の取得は後続のタスクが足す。
    pub fn new() -> Self {
        Self {
            model: HostModel {
                session: None,
                projection: Projection::new(),
                world: World::new(),
                world_consumed: 0,
            },
            registry: standard_registry(),
            input: HelloView::new(),
            dirty: Vec::new(),
            tick_request: None,
            draft_sync_request: None,
            command_prefix: None,
            #[cfg(feature = "test-hooks")]
            complete_count: 0,
        }
    }

    /// 実行環境と登録簿からホストを作る。登録簿の field がホスト自身の field と衝突したら拒否する。
    ///
    /// # Errors
    ///
    /// 部品の field 名が入力欄の field と同じなら [`HostError::FieldCollision`]。
    pub fn with_session(
        session: Session,
        registry: PartRegistry<HostModel>,
    ) -> Result<Self, HostError> {
        if let Some(f) = registry.fields().find(|f| HELLO_FIELDS.contains(&f.name)) {
            return Err(HostError::FieldCollision(f.name));
        }
        let mut input = HelloView::new();
        input.set_command_table(candidates::command_table(session.ir()));
        let projection = Projection::from_events(session.log());
        let mut world = World::new();
        replay_changes(&mut world, session.log());
        let world_consumed = session.log().len();
        let command_prefix =
            fandhe_workshop_contract::profile::read_profile(session.ir().pack.profile.as_ref())
                .ok()
                .map(|p| p.command_prefix);
        Ok(Self {
            model: HostModel {
                session: Some(session),
                projection,
                world,
                world_consumed,
            },
            registry,
            input,
            dirty: Vec::new(),
            tick_request: None,
            draft_sync_request: None,
            command_prefix,
            #[cfg(feature = "test-hooks")]
            complete_count: 0,
        })
    }

    /// 教材からホストを作る。`Session::new` を包むだけである。
    ///
    /// # Errors
    ///
    /// [`EngineHost::with_session`] と同じ。
    pub fn with_ir(ir: Ir, registry: PartRegistry<HostModel>) -> Result<Self, HostError> {
        Self::with_session(Session::new(ir), registry)
    }

    /// `glue::mount_app` が mount するホスト。配信する設定は [`EngineHost::new`] と同じ。
    ///
    /// 試験の設定（`test-hooks`）では、試験データの教材（`min-3step.json`）と開始直後の状態を埋め込んで読み込み、
    /// 3 つの表示が動く状態から始める。読み込みに失敗したら session なしに倒す（ブラウザ試験が
    /// `irLoaded: true` を必須にするため、失敗は試験で検出される）。
    pub(crate) fn for_mount() -> Self {
        #[cfg(feature = "test-hooks")]
        {
            if let Some(session) = crate::test_fixture::start_session() {
                if let Ok(host) = Self::with_session(session, standard_registry()) {
                    return host;
                }
            }
        }
        Self::new()
    }

    /// 次に届けるべき進行の宛先。処理中でなければ・未読込なら `None`。
    pub fn next_tick(&self) -> Option<TickAddr> {
        self.model.session().and_then(next_tick)
    }

    /// 進行の宛先の通知先を結ぶ。結んだ時点で、現在の宛先を 1 回通知する（開始時にすでに処理中の場合に備える）。
    ///
    /// 通知は `Component::update` の中から呼ばれる。通知先は Runtime にも App にも触れてはならない（再入になる）。
    pub fn set_tick_request(&mut self, f: TickRequest) {
        f(self.next_tick());
        self.tick_request = Some(f);
    }

    /// 下書きの書き戻しの通知先を結ぶ。送信で下書きを空にしたとき、`update` の中から呼ばれる。
    pub fn set_draft_sync_request(&mut self, f: DraftSyncRequest) {
        self.draft_sync_request = Some(f);
    }

    /// 部品が読む値。
    pub fn model(&self) -> &HostModel {
        &self.model
    }

    /// 表示部品の登録簿。
    pub fn registry(&self) -> &PartRegistry<HostModel> {
        &self.registry
    }

    /// 入力欄の値。glue が DOM へ書き戻すために読む。
    pub fn draft(&self) -> &str {
        self.input.draft()
    }

    /// 直近の入力で切り捨てが起きたか（試験用の口が読む）。
    #[cfg(feature = "test-hooks")]
    pub(crate) fn over_limit(&self) -> bool {
        self.input.over_limit()
    }

    /// 3 つの表示の照合用の状態を JSON 文字列で返す（読み取りのみ。試験の設定だけ）。
    ///
    /// 表示の導出（増分）とは別に、ログの全体から一括で導いた値を出す。ブラウザ試験が、DOM がこの値と
    /// 一致することを確かめる。文字列のエスケープは `serde_json` に任せる。
    ///
    /// 拒否の分類（`judgedCategories`）は、`busy` の回数を数えるために足した（TASK-222）。
    /// `stateWithoutLog`（ログを除いた状態の全体）と `ignoredReasons`（無視の理由）は、B-3〜B-5 の
    /// 状態の一致と無視の理由を確かめるために足した（TASK-222）。
    #[cfg(feature = "test-hooks")]
    pub(crate) fn engine_snapshot_json(&self) -> String {
        use crate::engine_parts::{artifact_rows, event_kind_name};
        use serde_json::json;

        let Some(session) = self.model.session() else {
            return json!({ "irLoaded": false }).to_string();
        };
        let log = session.log();
        let bulk = Projection::from_events(log);
        let mut world = World::new();
        replay_changes(&mut world, log);
        let rows: Vec<_> = bulk
            .log()
            .iter()
            .map(|r| {
                json!({
                    "event": r.event,
                    "generation": r.at.generation.0,
                    "step": r.at.step.0,
                    "kind": log.get(r.event as usize).map_or("other", event_kind_name),
                    // DOM の `data-kind`（行の種類）と照合する。行にしないイベントは null（DOM に行が無い）。
                    "line": log
                        .get(r.event as usize)
                        .and_then(crate::log_lines::line_from_event)
                        .map(|l| l.kind().as_str()),
                })
            })
            .collect();
        let artifacts: Vec<_> = artifact_rows(&world, bulk.marks())
            .into_iter()
            .map(
                |(path, mark)| json!({ "path": path, "mark": mark.map_or("", |m| m.data_value()) }),
            )
            .collect();
        let review: Vec<_> = bulk
            .review()
            .iter()
            .map(|i| {
                json!({
                    "step": i.step.0,
                    "achieved": i.achieved,
                    "skipped": i.skipped,
                    "changeCount": i.changes.len(),
                    "paths": i.changes.iter().map(|c| c.path.clone()).collect::<Vec<_>>(),
                    "keptInWorld": i.changes.iter().filter(|c| c.after.is_some() && world.contains_key(&c.path)).map(|c| c.path.clone()).collect::<Vec<_>>(),
                })
            })
            .collect();
        // 拒否の `Judged` だけの分類。EVT-7 の `busy` の回数をブラウザ試験が数える（TASK-222）。
        // ログ行の欄は DOM の照合（three-views）が使うため、増やさずここへ分けて置く。
        let judged_categories: Vec<_> = log
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match e {
                fandhe_workshop_core::event::Event::Judged {
                    at,
                    judgment: fandhe_workshop_contract::judgment::Judgment::Rejected(r),
                    ..
                } => Some(json!({
                    "event": i,
                    "generation": at.generation.0,
                    "step": at.step.0,
                    "category": r.category.as_str(),
                })),
                _ => None,
            })
            .collect();
        // 受け付けた入力の本文（TASK-438）。Enter と送信ボタンが同じ記録になること、改行が保たれることを
        // ブラウザ試験が照合する。ログ行の欄は DOM の照合が使うため、増やさず別のキーに置く。
        let received: Vec<_> = log
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match e {
                Event::InputReceived { input, .. } => Some(json!({
                    "event": i,
                    "input": serde_json::to_value(input).unwrap_or(serde_json::Value::Null),
                })),
                _ => None,
            })
            .collect();
        // 無視の理由は `IgnoreReason`（non_exhaustive）の serde の表記に任せる。
        let ignored_reasons: Vec<_> = log
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match e {
                fandhe_workshop_core::event::Event::Ignored { at, reason } => Some(json!({
                    "event": i,
                    "generation": at.generation.0,
                    "step": at.step.0,
                    "reason": serde_json::to_value(reason).unwrap_or(serde_json::Value::Null),
                })),
                _ => None,
            })
            .collect();
        // ログは無視のたびに伸びるため、状態の一致の比較からは外す。失敗は Null に倒す。
        let mut state_without_log =
            serde_json::to_value(session.state()).unwrap_or(serde_json::Value::Null);
        if let Some(obj) = state_without_log.as_object_mut() {
            obj.remove("log");
        }
        json!({
            "irLoaded": true,
            "logLen": log.len(),
            "judgedCategories": judged_categories,
            "received": received,
            "ignoredReasons": ignored_reasons,
            "stateWithoutLog": state_without_log,
            "consumed": bulk.consumed(),
            "generation": session.generation().0,
            "current": session.state().run.current.as_ref().map(|s| s.0.clone()),
            // DOM の `run-status` の `data-state` と照合する（TASK-436）。
            "runStatus": crate::run_status::run_status(Some(session)).data_state(),
            "log": rows,
            "artifacts": artifacts,
            "review": review,
            "ticks": log
                .iter()
                .filter_map(|e| match e {
                    fandhe_workshop_core::event::Event::Progress { at, index, .. } => {
                        Some((at.generation.0, *index))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>(),
            "worldMatchesState": world == session.state().run.world,
            "incrementalMatches": &bulk == self.model.projection() && world == self.model.world,
        })
        .to_string()
    }

    /// keydown の probe が受け取った回数（試験用の口が読む）。
    #[cfg(feature = "test-hooks")]
    pub(crate) fn keydown_probe(&self) -> u32 {
        self.input.keydown_probe()
    }

    /// root の中の Esc を受け取った回数（試験用の口が読む）。
    #[cfg(feature = "test-hooks")]
    pub(crate) fn esc_count(&self) -> u32 {
        self.input.esc_count()
    }

    /// 補完の action を受け取った回数（試験用の口が読む）。
    #[cfg(feature = "test-hooks")]
    pub(crate) fn complete_count(&self) -> u32 {
        self.complete_count
    }

    /// 登録済みの束縛点の現在値を、field 名と組で控える。
    fn bound_snapshot(&self) -> Vec<(&'static str, Option<BoundValue>)> {
        self.registry
            .fields()
            .filter(|f| f.kind == FieldKind::Bound)
            .map(|f| (f.name, self.registry.bound_value(f.name, &self.model)))
            .collect()
    }

    /// core への操作を適用し、変わった field を dirty に積む。未読込なら何もしない。
    ///
    /// 戻りは、今回の操作で拒否の判定（`Judged` の `Rejected`）が出たか。送信の後に下書きを残すかの判断に使う。
    fn apply_engine(&mut self, op: Op) -> bool {
        if self.model.session.is_none() {
            return false;
        }
        let before = self.bound_snapshot();
        let Some(session) = self.model.session.as_mut() else {
            return false;
        };
        let len_before = session.log().len();
        let events = session.apply(op);
        let returned = events.len();
        let rejected = events.iter().any(|e| {
            matches!(
                e,
                Event::Judged {
                    judgment: Judgment::Rejected(_),
                    ..
                }
            )
        });
        // extend はログの全体を受け取る（処理済みの先頭は読み飛ばす）。返ったイベントの列を渡さない。
        let log = session.log();
        self.model.projection.extend(log);
        let changed = returned > 0 || log.len() != len_before;
        // 世界状態も同じ列から、未処理の分だけ導く。session の借用中のため、欄ごとに触る。
        let from = self.model.world_consumed.min(log.len());
        replay_changes(&mut self.model.world, &log[from..]);
        self.model.world_consumed = log.len();
        for (name, old) in before {
            if self.registry.bound_value(name, &self.model) != old {
                self.dirty.push(name);
            }
        }
        // 次の進行の宛先を通知する（`None` も渡す。予約済みの印を glue が消すため）。
        if let (Some(f), Some(session)) = (self.tick_request.as_ref(), self.model.session.as_ref())
        {
            f(next_tick(session));
        }
        if changed {
            let lists = self
                .registry
                .fields()
                .filter(|f| f.kind == FieldKind::KeyedList)
                .map(|f| f.name);
            self.dirty.extend(lists);
        }
        rejected
    }

    /// 送信を受ける（Enter・送信ボタン）。入力欄の一覧に積み、エンジンへ渡し、必要なら下書きを空にする。
    ///
    /// 本文は加工しない（trim も分割もしない。改行は保つ）。空白だけの本文はエンジンへ送らない
    /// （後から適用し直す処理が空白だけの送信を拒否するのと同じ基準）。接頭辞を読めない・session が無い
    /// 場合はエンジンへ送らず、読めないときは入力を失わないよう下書きを残す。
    fn submit(&mut self) {
        let text = self.input.draft().to_owned();
        if text.is_empty() {
            return;
        }
        self.input.update(HelloAction::Submit);
        let mut clear = true;
        if self.model.session.is_some() {
            match self.command_prefix.clone() {
                None => clear = false,
                Some(prefix) if !text.trim().is_empty() => {
                    let rejected = self.apply_engine(Op::Input(submit_input(&prefix, text)));
                    clear = !keeps_draft(rejected);
                }
                Some(_) => {}
            }
        }
        // apply_engine は dirty を積むので、入力欄の分はその後に足す。
        self.dirty.extend_from_slice(self.input.dirty_fields());
        if clear {
            self.input.clear_draft();
            let from = self.input.dirty_fields();
            for f in from {
                if !self.dirty.contains(f) {
                    self.dirty.push(f);
                }
            }
            if let Some(f) = self.draft_sync_request.as_ref() {
                f();
            }
        }
    }
}

impl Component for EngineHost {
    type Action = HostAction;

    fn update(&mut self, action: Self::Action) {
        self.dirty.clear();
        match action {
            HostAction::Input(HelloAction::Submit) => self.submit(),
            HostAction::Input(a) => {
                self.input.update(a);
                self.dirty.extend_from_slice(self.input.dirty_fields());
            }
            HostAction::Engine(op) => {
                self.apply_engine(op);
            }
            // 補完の挿入は TASK-440 2/2（#332）の担当。ここでは状態もログも変えず、dirty も積まない。
            HostAction::Complete => {
                #[cfg(feature = "test-hooks")]
                {
                    self.complete_count = self.complete_count.saturating_add(1);
                }
            }
        }
    }

    fn view(&self) -> Node {
        let parts: Vec<Node> = self
            .registry
            .views(&self.model)
            .into_iter()
            .map(|(_, node)| node)
            .collect();
        // 部品の画面上の配置は、TASK-517 が決めるまでの仮のもの（入力欄の後ろに登録順に並べる）。
        #[allow(unused_mut, reason = "試験の設定でだけ raw-trigger を足す")]
        let mut children = vec![
            self.input.view(),
            el("div", vec![("data-testid", "workshop-parts")], parts),
        ];
        // 試験の設定だけ: `raw` の action を送る見えないボタン。hidden のため Tab の巡回に入らない。
        // 試験は `data-payload` を書き換えて click する。
        #[cfg(feature = "test-hooks")]
        children.push(el(
            "button",
            vec![
                ("type", "button"),
                ("data-testid", "raw-trigger"),
                ("data-action", "raw"),
                ("data-payload", ""),
                ("hidden", ""),
            ],
            vec![text("raw")],
        ));
        el("div", vec![("data-testid", "workshop-root")], children)
    }

    fn decode_action(name: &str, payload: &str) -> Option<Self::Action> {
        // 試験の設定だけ: `Op` の JSON をそのまま core へ送る（BUILD-27）。大きさ・深さの関門を通し、
        // 復号できなければ何もしない（安全側）。
        #[cfg(feature = "test-hooks")]
        if name == "raw" {
            use fandhe_workshop_contract::limits::LoadEntry;
            return fandhe_workshop_core::load_gate::load_json::<Op>(
                LoadEntry::History,
                payload.as_bytes(),
            )
            .ok()
            .map(HostAction::Engine);
        }
        // `tick`・`complete` は `RuntimeAction::decode` の正規形の検査を必ず通す（payload は信頼しない）。
        // `esc` と未知の名前は、これまでどおり入力欄に委ねる。
        match RuntimeAction::decode(name, payload) {
            Some(RuntimeAction::Tick(addr)) => Some(HostAction::Engine(Op::Tick(addr))),
            Some(RuntimeAction::Complete) => Some(HostAction::Complete),
            Some(RuntimeAction::Esc) | None => {
                HelloView::decode_action(name, payload).map(HostAction::Input)
            }
        }
    }
}

impl DirtyTracked for EngineHost {
    fn dirty_fields(&self) -> &[&'static str] {
        &self.dirty
    }
}

impl BindingSource for EngineHost {
    fn bound_value(&self, field: &str) -> Option<BoundValue> {
        if HELLO_FIELDS.contains(&field) {
            return self.input.bound_value(field);
        }
        self.registry.bound_value(field, &self.model)
    }
}
