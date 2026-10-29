//! 受け口の有効化の関門（TASK-191 1/2、版 1。EXT-10・D-53）。
//!
//! 将来の受け口（例: 助言の提供元）を、誰がどこで有効にできるかの規則である。
//! 呼び出し元は web の合成の根（`EngineHost`）で、受け口を実際に差し込む処理は、この関門が
//! [`Activation::Enabled`] を返したときだけ進める。
//!
//! 規則（D-53。案のうち「教材パックの欄で選べるが既定は無効」は採っていない）:
//! - 受け口は、実行環境（[`ReceptorSettings`]。エンジンの設定）だけで有効にできる
//! - 教材パックからは有効にできない。[`activation`] は IR・教材・状態を入力に取らず、
//!   受け口の名（[`ReceptorId`]）もエンジンのコードの定数だけが持てる
//! - 既定は無効。設定が無い・受講者の同意（[`LearnerConsent`]）が無い状態では、どの受け口も有効にならない
//!
//! 【推論】D-53 は同意の永続化を定めていないため、同意は保存しない（安全側）。履歴の包み・`SessionState`・
//! 保存先に入れず、再読み込み・履歴の読み込みの後は未同意から始める。
//!
//! 設定・同意・受け口の名は serde を実装しない。外から文字列で作る経路を型の段で無くすためである。
//!
//! ```compile_fail
//! // 設定・同意・名は実行時の値（JSON など）から復号できない。
//! use fandhe_workshop_core::receptor_gate::{LearnerConsent, ReceptorId, ReceptorSettings};
//! fn needs_deserialize<T: serde::de::DeserializeOwned>() {}
//! needs_deserialize::<ReceptorSettings>();
//! needs_deserialize::<LearnerConsent>();
//! needs_deserialize::<ReceptorId>();
//! ```
//!
//! ```compile_fail
//! // 受け口の名は実行時の文字列から作れない。
//! let s = String::from("advice");
//! let _ = fandhe_workshop_core::receptor_gate::ReceptorId::from(s);
//! ```
//!
//! 意図して作らないもの: 受け口そのもの・同意を求める画面・配信物で有効にする手段。
//! 有効にしても任意のコードが実行できず外部通信が増えないことの確認は、`crates/web/tests/receptor_enabled_safety.rs` と `tools/browser/receptor-safety.test.mjs` が行う（TASK-191 2/2）。
//!
//! 形を変える場合は、[`RECEPTOR_GATE_CONTRACT_VERSION`] を上げる。

use std::collections::BTreeSet;

/// この契約の版（TASK-191、版 1）。`CONTRACT_VERSION` とは別に持つ。
pub const RECEPTOR_GATE_CONTRACT_VERSION: u32 = 1;

/// 受け口の名。エンジンのコードの定数だけが作れる（`String` などの実行時の値からは作れない）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReceptorId(&'static str);

impl ReceptorId {
    /// 定数の名から作る。
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// 名を返す。
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

/// 実行環境の設定。許す受け口の集合で、既定は空（どれも許さない）。合成の根だけが組み立てる。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReceptorSettings {
    allowed: BTreeSet<ReceptorId>,
}

impl ReceptorSettings {
    /// 受け口を 1 つ許した設定を返す。
    #[must_use]
    pub fn allow(mut self, id: ReceptorId) -> Self {
        self.allowed.insert(id);
        self
    }

    fn allows(&self, id: ReceptorId) -> bool {
        self.allowed.contains(&id)
    }
}

/// 受講者の明示の同意。同意済みの受け口の集合で、既定は空。保存しない。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LearnerConsent {
    granted: BTreeSet<ReceptorId>,
}

impl LearnerConsent {
    /// 受け口ごとに同意を記録する。
    pub fn grant(&mut self, id: ReceptorId) {
        self.granted.insert(id);
    }

    /// 同意を取り消す。
    pub fn revoke(&mut self, id: ReceptorId) {
        self.granted.remove(&id);
    }

    fn has(&self, id: ReceptorId) -> bool {
        self.granted.contains(&id)
    }
}

/// 無効の理由。判定の順は [`DisabledReason::NotConfigured`] が先（決定的にする）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisabledReason {
    /// 実行環境が許していない。
    NotConfigured,
    /// 環境は許しているが、受講者の同意が無い。
    NoConsent,
}

/// 受け口の有効・無効。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activation {
    /// 有効（環境が許し、かつ同意済み）。
    Enabled,
    /// 無効。
    Disabled(DisabledReason),
}

/// 受け口の有効・無効を決める純関数。全域で、I/O・時刻・乱数を持たない。
pub fn activation(
    settings: &ReceptorSettings,
    consent: &LearnerConsent,
    id: ReceptorId,
) -> Activation {
    if !settings.allows(id) {
        Activation::Disabled(DisabledReason::NotConfigured)
    } else if !consent.has(id) {
        Activation::Disabled(DisabledReason::NoConsent)
    } else {
        Activation::Enabled
    }
}

/// [`activation`] が [`Activation::Enabled`] か。
pub fn is_enabled(settings: &ReceptorSettings, consent: &LearnerConsent, id: ReceptorId) -> bool {
    activation(settings, consent, id) == Activation::Enabled
}
