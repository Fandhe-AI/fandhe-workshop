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
//! 3 つの表示（ログ・左の成果物・復習）の部品の登録は TASK-347 2/2 が、action から `Op` を作る経路は
//! 後続のタスクが足す。`new()` は登録簿を空にする。
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

use fandhe_frontend_core::{el, Node};
use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_frontend_wasm_client::{BindingSource, BoundValue};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::{Projection, Session};

use crate::hello::{HelloAction, HelloView, HELLO_FIELDS};
use crate::parts::{FieldKind, PartRegistry};

/// 部品が読む表示用の値。読み出しの口だけを持ち、可変の口は持たない。
#[derive(Debug)]
pub struct HostModel {
    /// 実行環境。`None` は IR が未読込であることを表す。
    session: Option<Session>,
    /// ログから導いた表示用の値。
    projection: Projection,
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
}

/// ホストが受ける action。
#[derive(Debug)]
pub enum HostAction {
    /// 入力欄の action。
    Input(HelloAction),
    /// core への操作。
    Engine(Op),
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
    /// IR なし・登録簿が空のホストを作る。ブラウザでの IR の取得は後続のタスクが足す。
    pub fn new() -> Self {
        Self {
            model: HostModel {
                session: None,
                projection: Projection::new(),
            },
            registry: PartRegistry::new(),
            input: HelloView::new(),
            dirty: Vec::new(),
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
        let projection = Projection::from_events(session.log());
        Ok(Self {
            model: HostModel {
                session: Some(session),
                projection,
            },
            registry,
            input: HelloView::new(),
            dirty: Vec::new(),
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

    /// 部品が読む値。
    pub fn model(&self) -> &HostModel {
        &self.model
    }

    /// 表示部品の登録簿。
    pub fn registry(&self) -> &PartRegistry<HostModel> {
        &self.registry
    }

    /// 入力欄の値。glue が DOM へ書き戻すために読む。
    pub(crate) fn draft(&self) -> &str {
        self.input.draft()
    }

    /// 直近の入力で切り捨てが起きたか（試験用の口が読む）。
    #[cfg(feature = "test-hooks")]
    pub(crate) fn over_limit(&self) -> bool {
        self.input.over_limit()
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

    /// 登録済みの束縛点の現在値を、field 名と組で控える。
    fn bound_snapshot(&self) -> Vec<(&'static str, Option<BoundValue>)> {
        self.registry
            .fields()
            .filter(|f| f.kind == FieldKind::Bound)
            .map(|f| (f.name, self.registry.bound_value(f.name, &self.model)))
            .collect()
    }

    /// core への操作を適用し、変わった field を dirty に積む。未読込なら何もしない。
    fn apply_engine(&mut self, op: Op) {
        if self.model.session.is_none() {
            return;
        }
        let before = self.bound_snapshot();
        let Some(session) = self.model.session.as_mut() else {
            return;
        };
        let len_before = session.log().len();
        let returned = session.apply(op).len();
        // extend はログの全体を受け取る（処理済みの先頭は読み飛ばす）。返ったイベントの列を渡さない。
        let log = session.log();
        self.model.projection.extend(log);
        let changed = returned > 0 || log.len() != len_before;
        for (name, old) in before {
            if self.registry.bound_value(name, &self.model) != old {
                self.dirty.push(name);
            }
        }
        if changed {
            let lists = self
                .registry
                .fields()
                .filter(|f| f.kind == FieldKind::KeyedList)
                .map(|f| f.name);
            self.dirty.extend(lists);
        }
    }
}

impl Component for EngineHost {
    type Action = HostAction;

    fn update(&mut self, action: Self::Action) {
        self.dirty.clear();
        match action {
            HostAction::Input(a) => {
                self.input.update(a);
                self.dirty.extend_from_slice(self.input.dirty_fields());
            }
            HostAction::Engine(op) => self.apply_engine(op),
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
        el(
            "div",
            vec![("data-testid", "workshop-root")],
            vec![
                self.input.view(),
                el("div", vec![("data-testid", "workshop-parts")], parts),
            ],
        )
    }

    fn decode_action(name: &str, payload: &str) -> Option<Self::Action> {
        HelloView::decode_action(name, payload).map(HostAction::Input)
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
