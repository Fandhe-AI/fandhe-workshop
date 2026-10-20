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
//! 3 つの表示の部品は同じイベント列（`Session::log()`）から導いた値だけを読み、選択肢の部品（`offers`）は `Session` の現在の Step と IR を読む。`tick`・`complete` の action は
//! `Runtime::dispatch_action` から届く（TASK-349。`decode_action` が `action::RuntimeAction` の検査を通す）。
//! 送信（Enter・送信ボタン。TASK-438）は、入力欄の一覧に積んだ後、操作プロファイルの接頭辞で分類して
//! `Op::Input` として `apply_engine` に渡す。下書きは、エンジンが拒否したとき以外は空にし、`<textarea>` への
//! 書き戻しは `set_draft_sync_request` の通知先に頼む（上流は `<textarea>` の値を同期しないため）。
//! 進行の宛先は、`update` の後に `set_tick_request` の通知先へ渡す（表示側のタイマーが予約する）。試験の設定（`test-hooks`）だけ、`raw` の action（`Op` の JSON）・見えない `raw-trigger`・
//! 状態の書き出し（`engine_snapshot_json`）・試験用の教材の読み込み（`for_mount`）・再読み込みの模擬（`raw` の
//! 固定の合図。TASK-221 2/2）・履歴ファイルの読み込み（`raw` の固定の接頭辞。TASK-330 2/2）・試験用の教材への差し替え（`raw` の固定の接頭辞 `load-fixture:`。TASK-445 2/2）を持つ（BUILD-27・SHELL-35）。
//! 選択肢のクリック（`choose`）は配信する設定にも入り、`decode_action` が `Op::Input(Input::Choice)` に変える。
//! ビューの選択（`select-view`。TASK-470）も配信する設定に入り、`decode_action` が `HostAction::SelectView` に変える。
//! 承認のクリック（`decide`・`revise-target`。TASK-447 1/2）も同じで、`Op::Input(Input::Approval)` に変える。
//! 指せる行のクリック（`ui`。TASK-480 2/2）も配信する設定に入り、`PointTarget::decode` を通して `Op::Input(Input::Ui)` に変える。
//!
//! 入力欄なしの教材（`pack.profile.input_field: hidden`。PACK-20。TASK-450 2/3）では、入力欄・候補の一覧・送信を
//! [`HelloView`] が描かない。ホストは教材を読んだとき（`with_session`・`load-fixture:`）に 1 度だけ設定を渡し、mount の後には
//! 変えない（入力欄の有無は構造の違いで、束縛点の差分更新に乗らないため）。口が無いので、下書き・送信・補完の action は
//! 状態を変えず、保存済みの下書きも入力欄へ戻さない。停止（Esc・停止ボタン）と選択肢・承認・画面操作の経路は変えない。
//!
//! 履歴ファイルの読み込み（[`EngineHost::open_history`]）は、core の `load_session`（段 1〜8）が開いたときだけ `Session` を差し替える。
//! 開かないときは状態・導出した値・dirty のどれにも触れない（SAVE-19）。ファイルの選択と読み取りは DOM の値を読むため、
//! Rust ではなく試験の設定だけの `static/test-hooks.js` が行う（`make check-dom-reads`）。可視の画面は TASK-531 の担当である。
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
use std::collections::BTreeMap;

#[cfg(feature = "test-hooks")]
use fandhe_frontend_core::text;
use fandhe_frontend_core::{el, Node};
use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_frontend_wasm_client::{BindingSource, BoundValue};
use fandhe_workshop_contract::content_hash::content_hash;
#[cfg(feature = "test-hooks")]
use fandhe_workshop_contract::history_version::ReasonCode;
use fandhe_workshop_contract::history_version::{ContentHash, HistoryVersions, Verdict};
#[cfg(feature = "test-hooks")]
use fandhe_workshop_contract::history_version::{
    ConversionNotes, Version3, VersionAxis, VocabVersion,
};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::Judgment;
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey, StorageError};
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::addr::TickAddr;
use fandhe_workshop_core::changed::replay_changes;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_boot::{boot_session, BootOutcome};
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_limit;
use fandhe_workshop_core::history_load::load_session;
use fandhe_workshop_core::history_open::OpeningCaps;
use fandhe_workshop_core::input::{Control, Decision, Input, Op};
use fandhe_workshop_core::ir_load::IrLoadError;
use fandhe_workshop_core::ops_record::OpsRecorder;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::text_fill::{filled_text, filled_texts_of_log};
use fandhe_workshop_core::version_record::record_versions;
#[cfg(feature = "test-hooks")]
use fandhe_workshop_core::version_record::record_versions_with;
use fandhe_workshop_core::world::World;
use fandhe_workshop_core::{Projection, Session, SessionState};

use crate::action::{RuntimeAction, ESC_KEYDOWN_ATTRS};
use crate::artifact_view::{declared_kinds, dirty_list_fields, ViewKind, VIEW_LIST_FIELDS};
use crate::artifact_view::{PointTarget, POINT_ACTION};
use crate::candidates;
use crate::engine_parts::standard_registry;
use crate::export_notice::{chapter_end_in, ExportHint, EXPORT_NOTICE_FIELD};
use crate::hello::{HelloAction, HelloView, HELLO_FIELDS};
use crate::parts::{FieldKind, PartRegistry};
use crate::profile::InputField;
use crate::save_notice::SAVE_NOTICE_FIELD;
use crate::secret_notice::{secret_hold, SECRET_NOTICE_FIELD};
use crate::tick_schedule::next_tick;
use crate::view_select::{self, ViewChoice};
use fandhe_workshop_contract::profile::InputFieldMode;
use fandhe_workshop_core::secret_scan::SecretShape;

/// 部品が読む表示用の値。読み出しの口だけを持ち、可変の口は持たない。
#[derive(Debug)]
pub struct HostModel {
    /// 操作を `ops` に記録する実行環境。`None` は IR が未読込であることを表す。
    ///
    /// 記録（`ops`）は離れる直前の保存の元になる（TASK-319）。表示の部品は [`HostModel::session`] で `Session` だけを読む。
    recorded: Option<RecordedSession>,
    /// ログから導いた表示用の値。
    projection: Projection,
    /// ログの `Changed` だけから組み立てた世界状態（P-1b。左の成果物の元）。
    world: World,
    /// `world` が読み終えたログの長さ。
    world_consumed: usize,
    /// 拒否の `Judged` ごとの、充填済みの理由の文面（キーはイベントの位置）。
    ///
    /// 文面は IR の文面の ID の解決と実行時の値の差し込みが要り、拒否を適用した直後の状態でしか作れない
    /// （後の操作で状態が進むため）。そのため `apply_engine` が適用の直後に 1 度だけ作って控える。
    /// 作れなかった拒否（文面の解決の失敗）は控えず、行を出さない（fail-closed）。
    /// 復元（`with_session`）と履歴の差し替え（`install_session`）では、ログの各判定の時点の Step・進み具合から全件を作り直す。
    judged_texts: BTreeMap<u32, String>,
    /// 左の成果物で選んだビュー（TASK-470。`select-view`）。表示の状態で、履歴（`ops`）には記録しない。
    /// 読み方は [`HostModel::selected_view`] が決め、宣言に無い種類のときは読まれない。今の Step が変わると
    /// `step_engine` が捨てる（ART-12。元の Step へ戻っても以前の選択は戻らない）。
    view_choice: Option<ViewChoice>,
    /// 直近の保存の失敗の種類（SAVE-10・SAVE-11）。`save-notice` の元で、履歴（`ops`）には記録しない表示の状態。
    /// 保存に成功すると `None` に戻る。同期の保存の結果（`EngineHost::save_after_change`）と、非同期の保存の失敗の probe
    /// （`EngineHost::refresh_save_failure`）から決まる。
    save_failure: Option<StorageError>,
    /// 履歴の書き出しの案内（SAVE-29・D-57。`export-notice` の元）。表示の状態で、履歴（`ops`）にも保存先にも書かない。
    /// 次の起点・履歴の差し替えで置き換わり、再読み込みでは出ない（消える時機は仮置き）。
    export_hint: Option<ExportHint>,
    /// 保存の直前に見つけた秘密情報らしい文字列の形（SAFE-27・TASK-164 1/3。`secret-notice` の元）。形だけを持ち、値は持たない。
    /// 表示の状態で、履歴（`ops`）にも保存先にも書かない。見つけた間は、その値を保存先へ積まない。
    secret_warning: Vec<SecretShape>,
}

impl HostModel {
    /// 警告に出す形。無ければ空（`secret-notice` が空になる）。
    pub fn secret_warning(&self) -> &[SecretShape] {
        &self.secret_warning
    }

    /// 書き出しの案内の種類。無ければ `None`（`export-notice` が空になる）。
    pub fn export_hint(&self) -> Option<ExportHint> {
        self.export_hint
    }

    /// 左の成果物で今見せるビュー。保持した選択が今の Step のもので、今の宣言に含まれるときだけそれを返し、
    /// それ以外は既定のビュー（宣言の先頭。`view_select::effective_view`）を返す。
    pub fn selected_view(&self) -> ViewKind {
        view_select::effective_view(self.session(), self.view_choice.as_ref())
    }

    /// 直近の保存の失敗の種類。失敗していなければ `None`（`save-notice` が空になる）。
    pub fn save_failure(&self) -> Option<StorageError> {
        self.save_failure
    }

    /// 位置 `event` の拒否の理由の文面（core の `filled_text` が返した値）。控えが無ければ `None`。
    pub fn judged_text(&self, event: u32) -> Option<&str> {
        self.judged_texts.get(&event).map(String::as_str)
    }

    /// 実行環境。未読込なら `None`。
    pub fn session(&self) -> Option<&Session> {
        self.recorded.as_ref().map(RecordedSession::session)
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
    /// 左の成果物のビューの選択（TASK-470）。今の Step が宣言した種類だけを受け、それ以外は何も変えない。
    SelectView(ViewKind),
    /// 遅延保存の結果（SAVE-10）。先頭は書き込んだ時点の保存世代で、今の世代と違えば古い結果として捨てる。
    /// `None` は成功で通知を消し、`Some` は失敗の種類で通知を出す。
    SaveResult(u64, Option<StorageError>),
    /// 遅延保存の直前の検出（SAFE-27・TASK-164 1/3）。見つけた形だけを持ち、値は持たない。
    SecretHold(u64, Vec<SecretShape>),
    /// 補完（FEND-4）。配送の時点で一覧が開いていて文字があるときだけ、下書きを先頭の候補に置き換える（TERM-11）。
    Complete,
    /// 再読み込みの模擬（試験の設定だけ）。メモリ上の `Session` の世代を上げ、予約済みのタイマーは取り消さない。
    ///
    /// `Session::apply` 以外で状態を変える唯一の経路であり、配信する設定には入らない。web 本来の再読み込み
    /// （保存した状態の復元）は起動の分岐（[`EngineHost::boot`]、TASK-319 1/3）が担い、この模擬は使わない。
    #[cfg(feature = "test-hooks")]
    Reload,
    /// 履歴ファイルの読み込み（試験の設定だけ。本文は未検査の文字列で、`open_history` の段 1〜8 が断る）。
    ///
    /// 状態を変える口は `Session::apply` と再読み込みの模擬に加え、開いたときの `Session` の差し替えだけである。
    /// 選んだファイルの読み取りは試験の設定の `test-hooks.js` が行い、この action で届ける。
    #[cfg(feature = "test-hooks")]
    OpenHistory(String),
    /// 試験用の教材への差し替え（試験の設定だけ。TASK-445 2/2）。名前は固定の一覧（[`FixtureName`]）だけ。
    ///
    /// 開始の状態・補完表・導出した値を新しい教材から作り直す。作れなければ何もしない。
    #[cfg(feature = "test-hooks")]
    LoadFixture(FixtureName),
    /// 任意の教材（IR の JSON）への差し替え（試験の設定だけ。TASK-175 2/2）。本文は未検査の文字列で、読めなければ何もしない。
    ///
    /// 版の違う履歴 15 件のうち、開く側の教材が mount の教材と違う行を開くために使う。判定は `load_session` に任せたまま、
    /// 開く側の教材だけを入れ替える。
    #[cfg(feature = "test-hooks")]
    LoadIr(String),
    /// 開く側の版の上書き（試験の設定だけ。TASK-175 2/2）。読めなければ何もしない。
    #[cfg(feature = "test-hooks")]
    OpeningOverride(OpeningOverride),
}

/// 開く側の版の上書き（試験の設定だけ）。`None` の欄は、教材と現行のエンジンから求めた値のまま使う。
///
/// 実在しない別の版のエンジンや、語彙の上限が低い実装を模して、版の違う履歴 15 件の開く側の条件を作る（VER-2）。
/// 配信する設定には存在せず、`open_history` の配信の経路は常に現行の値を使う。
#[cfg(feature = "test-hooks")]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpeningOverride {
    /// 開く側のエンジンの版。
    pub engine: Option<Version3>,
    /// 開く側が扱える語彙の版の上限。
    pub vocab_max: Option<VocabVersion>,
}

/// 版の軸の固定の名前（snake_case）。試験の期待の表の綴りと一致させ、Debug 名には頼らない。
#[cfg(feature = "test-hooks")]
fn axis_name(axis: VersionAxis) -> &'static str {
    match axis {
        VersionAxis::Engine => "engine",
        VersionAxis::IrFormat => "ir_format",
        VersionAxis::Vocab => "vocab",
        VersionAxis::Content => "content",
        VersionAxis::ContentHash => "content_hash",
        VersionAxis::Refs => "refs",
    }
}

/// 変換して開いたときの違いの軸の名前（`notes` の順）。
#[cfg(feature = "test-hooks")]
fn axes_of(notes: &ConversionNotes) -> Vec<&'static str> {
    notes.diffs().iter().map(|d| axis_name(d.axis)).collect()
}

/// 履歴の読み込みの観察の控え（試験の設定だけ）。結果の種類以外の、差し替えの回数・違いの軸・開いた直後の状態を持つ。
#[cfg(feature = "test-hooks")]
#[derive(Debug, Default)]
struct ImportProbe {
    /// `install_recorded` を呼んだ回数。開かない履歴で実行環境を作り直していないことを数で見る。
    install_count: u32,
    /// 直近の読み込みで変換して開いたときの違いの軸の名前。それ以外は空。
    last_axes: Vec<&'static str>,
    /// 直近に開いた直後（進行の前）の状態の JSON。開いていなければ `None`。
    last_state: Option<String>,
    /// 開く側の版の上書き。
    opening: OpeningOverride,
}

/// 差し替えられる試験用の教材の名前（試験の設定だけ）。未知の名前は復号の時点で断る。
#[cfg(feature = "test-hooks")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureName {
    /// 確認が `answer` の Step を持つ教材（選択肢のボタンの試験）。
    Answer,
    /// 入力方式が `approval` で、対象つきの「直す」の規則を持つ Step の教材（承認のボタンの試験。TASK-447 1/2）。
    Approval,
    /// 複数の種類のビューを Step ごとに宣言する教材（ビューの切り替えの試験。TASK-470 2/2）。
    Views,
    /// 入力方式が `ui_action` の Step を持つ教材（入力方式ごとの入力の口の試験。TASK-450 1/3）。
    UiAction,
    /// `enter` が自分を指して巡回する IR（遷移の上限の試験。TASK-160 1/3）。
    CycleEnterSelf,
    /// `enter` が 2 者で巡回する IR。
    CycleEnter2,
    /// `exit_to` が 2 者で巡回する IR。
    CycleExit2,
    /// 巡回していても 1 入力あたりの遷移が少数の IR（上限に当たらない対照）。
    CycleD07,
    /// 巡回しない `use` の連鎖で、着地がちょうど 1,000 回（通る）。
    Chain1000,
    /// 巡回しない `use` の連鎖で、着地が 1,001 回（止まる）。
    Chain1001,
}

/// `raw` の action で試験用の教材への差し替えを指す固定の接頭辞（試験の設定だけ）。
///
/// 後ろは [`FixtureName`] の名前（`answer`・`approval`・`views`・`ui-action`・`cycle-enter-self`・`cycle-enter-2`・`cycle-exit-2`・`cycle-d07`・`chain-1000`・`chain-1001`）だけ。`Op` の JSON は `{` か `[` で始まるため衝突しない。
#[cfg(feature = "test-hooks")]
pub const RAW_LOAD_FIXTURE: &str = "load-fixture:";

/// `raw` の action で再読み込みの模擬を指す固定の合図（試験の設定だけ）。
///
/// 完全に一致したときだけ [`HostAction::Reload`] になる。`Op` の JSON としては復号できない形のため、
/// `Op` との衝突は無い（`tests/host_contract.rs` が固定する）。新しい隠し要素は足さず、`raw-trigger` を使う。
#[cfg(feature = "test-hooks")]
pub const RAW_RELOAD: &str = "{\"reload\":{}}";

/// `raw` の action で履歴ファイルの読み込みを指す固定の接頭辞（試験の設定だけ）。
///
/// 接頭辞が一致したときだけ、後ろの本文を [`HostAction::OpenHistory`] として届ける。`Op` の JSON は `{` か `[` で
/// 始まるため衝突しない。`RuntimeAction`（版つきの契約）は変えず、新しい隠し要素も足さない。
#[cfg(feature = "test-hooks")]
pub const RAW_OPEN_HISTORY: &str = "open-history:";

/// `raw` の action で任意の教材（IR の JSON）への差し替えを指す固定の接頭辞（試験の設定だけ。TASK-175 2/2）。
#[cfg(feature = "test-hooks")]
pub const RAW_LOAD_IR: &str = "load-ir:";

/// `raw` の action で開く側の版の上書きを指す固定の接頭辞（試験の設定だけ。TASK-175 2/2）。
///
/// 後ろは `{"engine": "<x.y.z>" | null, "vocab_max": "<x.y>" | null}`。両方 `null` で解除する。
#[cfg(feature = "test-hooks")]
pub const RAW_OPENING_OVERRIDE: &str = "opening-override:";

/// 開く側の版の上書きの本文を読む。鍵は `engine`・`vocab_max` の 2 つだけで、値は版の文字列か `null`。
/// 未知の鍵・読めない版・大きさや深さの関門を通らない本文は `None`。
#[cfg(feature = "test-hooks")]
fn parse_opening_override(body: &str) -> Option<OpeningOverride> {
    use fandhe_workshop_contract::limits::LoadEntry;
    let value: serde_json::Value =
        fandhe_workshop_core::load_gate::load_json(LoadEntry::History, body.as_bytes()).ok()?;
    let obj = value.as_object()?;
    if obj.keys().any(|k| k != "engine" && k != "vocab_max") {
        return None;
    }
    let field = |name: &str| -> Option<Option<String>> {
        match obj.get(name) {
            None | Some(serde_json::Value::Null) => Some(None),
            Some(serde_json::Value::String(s)) => Some(Some(s.clone())),
            Some(_) => None,
        }
    };
    let engine = match field("engine")? {
        None => None,
        Some(s) => Some(Version3::parse(&s)?),
    };
    let vocab_max = match field("vocab_max")? {
        None => None,
        Some(s) => Some(VocabVersion::parse(&s)?),
    };
    Some(OpeningOverride { engine, vocab_max })
}

/// 履歴ファイルの読み込みの結果（TASK-330 2/2）。理由は固定のコードだけで、ファイルの内容を含めない（SAFE-40）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportOutcome {
    /// 変換なしで開いた。
    Open,
    /// 変換して開いた。
    Converted,
    /// 表示だけ。状態は変えない。
    ViewOnly(LoadReason),
    /// 開かない。状態は変えない。
    Refused(LoadReason),
    /// 開く側の条件（実行環境・開始時の状態・開く側の版）が無く、判定できなかった。状態は変えない。
    Unavailable,
}

impl ImportOutcome {
    /// 結果の種類を表す固定の名前。
    pub fn as_str(self) -> &'static str {
        match self {
            ImportOutcome::Open => "open",
            ImportOutcome::Converted => "converted",
            ImportOutcome::ViewOnly(_) => "view_only",
            ImportOutcome::Refused(_) => "refused",
            ImportOutcome::Unavailable => "unavailable",
        }
    }

    /// 開かなかったときの理由コード。
    pub fn reason(self) -> Option<LoadReason> {
        match self {
            ImportOutcome::ViewOnly(c) | ImportOutcome::Refused(c) => Some(c),
            _ => None,
        }
    }

    /// 状態を差し替えたか。
    pub fn is_loaded(self) -> bool {
        matches!(self, ImportOutcome::Open | ImportOutcome::Converted)
    }
}

/// `Session` から導く表示用の値。`with_session` と読み込みの差し替えが同じ導出を使う。
struct Derived {
    projection: Projection,
    world: World,
    world_consumed: usize,
    judged_texts: BTreeMap<u32, String>,
    command_prefix: Option<String>,
    /// 入力欄の表示（PACK-20）。読めない設定は既定の `Shown` に倒す。
    input_field: InputField,
}

/// 教材の `pack.profile.input_field` を表示層の型で返す。契約の `Hidden` のときだけ `Hidden` で、
/// 読めない設定・指定なしは既定の `Shown` にする（`DisplaySettings::default` と同じ）。
fn read_input_field(session: &Session) -> InputField {
    match fandhe_workshop_contract::profile::read_profile(session.ir().pack.profile.as_ref()) {
        Ok(p) if p.input_field == InputFieldMode::Hidden => InputField::Hidden,
        _ => InputField::Shown,
    }
}

/// ログ全体から表示用の値を導く。
fn derive(session: &Session) -> Derived {
    let projection = Projection::from_events(session.log());
    let mut world = World::new();
    replay_changes(&mut world, session.log());
    let mut judged_texts = BTreeMap::new();
    restore_judged_texts(&mut judged_texts, session);
    let command_prefix =
        fandhe_workshop_contract::profile::read_profile(session.ir().pack.profile.as_ref())
            .ok()
            .map(|p| p.command_prefix);
    Derived {
        projection,
        world,
        world_consumed: session.log().len(),
        judged_texts,
        command_prefix,
        input_field: read_input_field(session),
    }
}

/// 次の進行の宛先の通知先。`apply_engine` の後に、次に届けるべき進行（無ければ `None`）を渡す。
///
/// glue が `set_timeout` の予約に使う。ホストは予約の状態を持たず、重複の抑止も glue が受け持つ。
pub type TickRequest = Box<dyn Fn(Option<TickAddr>)>;

/// 下書きの書き戻しの要求の通知先。送信で下書きを空にしたとき、`update` の中から呼ばれる。
///
/// glue が `<textarea>` の値の同期（タイマー経由）に使う。通知先は Runtime にも App にも触れてはならない（再入になる）。
pub type DraftSyncRequest = Box<dyn Fn()>;

/// 変更のたびに履歴の値を保存先へ書く口（SAVE-10。TASK-323 1/2）。引数は `history_to_save` の値で、結果は書けたかどうか。
///
/// 実体は `persist` が mount の前に注入する（保留と保存先を持つのは `persist` だけ）。書けなかった値は注入側が保留に残す。
/// 値・鍵の文字列は結果に含めない（`StorageError` は種類だけ）。`Runtime` にも `App` にも触れてはならない（再入になる）。
pub type HistorySaver = Box<dyn Fn(&str) -> Result<(), StorageError>>;

/// 非同期の保存の失敗（IndexedDB。SAVE-11）を読む口。読み取り専用で、`update` のたびに引かれる。
///
/// IndexedDB の確定は後から届き、そのコールバックからホストへは触れない（`state_paths`）。そのため実体（`persist`）は失敗の種類を
/// 保存先に残し、ホストがこの口で引く。`Runtime` にも `App` にも触れてはならない（再入になる）。
pub type SaveFailureProbe = Box<dyn Fn() -> Option<StorageError>>;

/// 送信の後に下書きを残すか。エンジンが拒否したときだけ残す（TERM-1・JUDGE-5）。
///
/// 問い返しの後の扱いは JUDGE-27・TERM-22（検討中）に属するため、ここでは固定しない。
/// 拒否でなければ空にする。
pub(crate) fn keeps_draft(rejected: bool) -> bool {
    rejected
}

/// 起動の結果の種類（TASK-319 1/3）。保存済みの履歴の中身は含めない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootKind {
    /// 新しく始めた（保存済みの履歴が無い、または未操作）。世代は上がらない。
    Fresh,
    /// 保存済みの履歴を開いて復元した（世代 +1）。
    Restored,
    /// 保存済みの履歴を開かなかった。初期状態で始め、保存は止める。
    NotOpened,
    /// 実行環境が無く、何もしなかった。
    NoSession,
}

impl BootKind {
    /// 固定の名前。
    pub fn as_str(self) -> &'static str {
        match self {
            BootKind::Fresh => "fresh",
            BootKind::Restored => "restored",
            BootKind::NotOpened => "not_opened",
            BootKind::NoSession => "no_session",
        }
    }
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
    /// 履歴の差し替えのとき、これまでの進行のタイマーを無効にさせる通知先（表示側が登録する）。
    tick_reset: Option<Box<dyn Fn()>>,
    /// 下書きの書き戻しの通知先。未設定なら通知しない。
    draft_sync_request: Option<DraftSyncRequest>,
    /// 履歴が変わったときの通知先（変更の後の遅延保存。TASK-318）。未設定なら通知しない。
    save_request: Option<DraftSyncRequest>,
    /// 即時の保存が済んだとき、予約済みの遅延保存を無効にさせる通知先（TASK-318）。未設定なら通知しない。
    save_cancel: Option<DraftSyncRequest>,
    /// この `update` で履歴（op の列・下書きの記録）が変わった印。`update` の末尾で通知して下ろす。
    history_changed: bool,
    /// 変更のたびの保存の口。未設定（native の試験など）なら保存を試さない。
    history_saver: Option<HistorySaver>,
    /// 非同期の保存の失敗を読む口（SAVE-11）。未設定なら同期の保存の結果だけで表示を決める。
    save_failure_probe: Option<SaveFailureProbe>,
    /// 直近の同期の保存（saver）の失敗。表示は「これ、無ければ probe の値」で決める。
    sync_save_failure: Option<StorageError>,
    /// 即時の保存の世代。遅延保存の結果は、書き込んだ時点の世代が今と一致するときだけ反映する。
    save_epoch: u64,
    /// 今の `update` で状態を変えた（ログが伸びた）印。`update` の末尾で 1 回だけ保存を試して下ろす。
    /// 下書きだけの変更では立てない（打鍵ごとに履歴の全体を書く費用を避ける。下書きは離れる直前に書く）。
    save_due: bool,
    /// 保存済みの履歴が無い状態で起動した（`boot` が `Fresh`）印。本番でも使う（`boot_kind` は試験の設定だけ）。
    fresh_start: bool,
    /// 初回の保存時の案内を 1 度出した印（SAVE-29）。
    first_save_hinted: bool,
    /// 操作プロファイルのコマンド接頭辞。session が無い・読めないときは `None`（送信をエンジンへ送らない）。
    command_prefix: Option<String>,
    /// 開始時の状態の控え。履歴ファイルを適用し直す出発点になる。session が無ければ `None`。
    ///
    /// 【推論】`Control::Begin` と `start` からの初期状態の構築（TASK-217・TASK-294）が入るまでは、
    /// 組み立て時の状態を出発点にする。
    initial: Option<SessionState>,
    /// 直近に `ops` へ記録した下書き。入力欄の値がこれと違うときだけ、下書きを記録する（未操作の入力欄は記録しない）。
    draft_recorded: String,
    /// 保存を止める印。起動時に保存済みの履歴を開けなかった（上書きすると失う）、または読めなかったときに立てる（fail-closed）。
    save_blocked: bool,
    /// 教材（IR）を読めなかった理由コード（VER-13。固定の ASCII だけで、入力値・ID を含めない）。
    ///
    /// 起動で断ったホスト（[`EngineHost::refused`]）は session が無く、`view` がこのコードの通知だけを描く。
    /// 起動後の `load-ir:` で読めなかったときは、session を差し替えずにコードだけを控える（試験の設定だけ）。
    ir_refusal: Option<&'static str>,
    /// 起動の結果の種類（試験用の口が読む）。`boot` を呼ばなかったときは `None`。
    #[cfg(feature = "test-hooks")]
    boot_kind: Option<BootKind>,
    /// 直近の読み込みの結果（試験用の口が読む）。
    #[cfg(feature = "test-hooks")]
    last_import: Option<ImportOutcome>,
    /// 読み込みの観察の控え（試験用の口が読む）。
    #[cfg(feature = "test-hooks")]
    probe: ImportProbe,
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
                recorded: None,
                projection: Projection::new(),
                world: World::new(),
                world_consumed: 0,
                judged_texts: BTreeMap::new(),
                view_choice: None,
                save_failure: None,
                export_hint: None,
                secret_warning: Vec::new(),
            },
            registry: standard_registry(),
            input: HelloView::new(),
            dirty: Vec::new(),
            tick_request: None,
            tick_reset: None,
            draft_sync_request: None,
            save_request: None,
            save_cancel: None,
            history_changed: false,
            history_saver: None,
            save_failure_probe: None,
            sync_save_failure: None,
            save_epoch: 0,
            save_due: false,
            fresh_start: false,
            first_save_hinted: false,
            command_prefix: None,
            initial: None,
            draft_recorded: String::new(),
            save_blocked: false,
            ir_refusal: None,
            #[cfg(feature = "test-hooks")]
            boot_kind: None,
            #[cfg(feature = "test-hooks")]
            last_import: None,
            #[cfg(feature = "test-hooks")]
            probe: ImportProbe::default(),
            #[cfg(feature = "test-hooks")]
            complete_count: 0,
        }
    }

    /// 教材（IR）を読めなかったホストを作る（旧いエンジンが新しい教材を断る場面。VER-13）。
    ///
    /// [`EngineHost::new`] と同じ空の状態に、理由コード（`IrLoadError::reason_code`）だけを入れる。
    /// session が無いので `history_key` は `None` になり、保存先を開かず、何も書かない（fail-closed）。
    /// `view` は教材を描かず、通知（`ir-notice`）だけを描く。
    pub fn refused(err: &IrLoadError) -> Self {
        let mut host = Self::new();
        host.ir_refusal = Some(err.reason_code());
        host
    }

    /// 教材を読めなかった理由コード。読めた・断っていないときは `None`。
    pub fn ir_refusal(&self) -> Option<&'static str> {
        self.ir_refusal
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
        let initial = Some(session.state().clone());
        let d = derive(&session);
        input.set_input_field(d.input_field);
        let start = RecordedSession::first_start(session.ir());
        Ok(Self {
            model: HostModel {
                recorded: Some(RecordedSession::new(session, start)),
                projection: d.projection,
                world: d.world,
                world_consumed: d.world_consumed,
                judged_texts: d.judged_texts,
                view_choice: None,
                save_failure: None,
                export_hint: None,
                secret_warning: Vec::new(),
            },
            registry,
            input,
            dirty: Vec::new(),
            tick_request: None,
            tick_reset: None,
            draft_sync_request: None,
            save_request: None,
            save_cancel: None,
            history_changed: false,
            history_saver: None,
            save_failure_probe: None,
            sync_save_failure: None,
            save_epoch: 0,
            save_due: false,
            fresh_start: false,
            first_save_hinted: false,
            command_prefix: d.command_prefix,
            initial,
            draft_recorded: String::new(),
            save_blocked: false,
            ir_refusal: None,
            #[cfg(feature = "test-hooks")]
            boot_kind: None,
            #[cfg(feature = "test-hooks")]
            last_import: None,
            #[cfg(feature = "test-hooks")]
            probe: ImportProbe::default(),
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

    /// `persist::mount_with_history` が mount するホスト（配信する設定）。[`EngineHost::new`] と同じである。
    ///
    /// 試験の設定（`test-hooks`）では代わりに [`EngineHost::for_mount_with`] が、試験データの教材（既定は
    /// `min-3step.json`）と開始直後の状態を埋め込んで読み込み、3 つの表示が動く状態から始める。
    /// 読み込みに失敗したら session なしに倒す（ブラウザ試験が `irLoaded: true` を必須にするため、失敗は試験で検出される）。
    #[cfg(not(feature = "test-hooks"))]
    pub(crate) fn for_mount() -> Self {
        Self::new()
    }

    /// 起動の前に選んだ教材で mount するホスト（試験の設定だけ。TASK-319 3/3）。
    ///
    /// `persist::mount_with_history` が URL のクエリから選んだ教材を渡す。読み込みに失敗したら session なしに倒す。
    #[cfg(feature = "test-hooks")]
    pub(crate) fn for_mount_with(fixture: crate::test_fixture::MountFixture) -> Self {
        if let Some(err) = crate::test_fixture::mount_refusal(fixture) {
            return Self::refused(&err);
        }
        if let Some(session) = crate::test_fixture::mount_session(fixture) {
            if let Ok(host) = Self::with_session(session, standard_registry()) {
                return host;
            }
        }
        Self::new()
    }

    /// 起動時に保存済みの履歴（あれば）を渡し、新しく始めるか復元するかを決めて実行環境を差し替える（TASK-319 1/3。SAVE-3・SAVE-28）。
    ///
    /// 分岐は core の `boot_session` に任せ、条件は複製しない。保存済みの値が無い・未操作なら、いまの初期状態のまま
    /// 世代を上げずに始める。開けたら再読み込みとして復元する（世代 +1）。開けなければ初期状態のまま始め、
    /// 保存済みの値を上書きしないよう保存を止める（fail-closed）。実行環境が無い（IR 未読込）ときは何もせず
    /// [`BootKind::NoSession`] を返す。`mount` の前に 1 度だけ呼ぶ。
    pub fn boot(&mut self, saved: Option<&[u8]>) -> BootKind {
        let (Some(session), Some(initial)) = (self.model.session(), self.initial.as_ref()) else {
            return BootKind::NoSession;
        };
        let ir = session.ir().clone();
        let start = RecordedSession::first_start(&ir);
        let initial = initial.clone();
        let Ok(hash) = content_hash(&ir) else {
            self.save_blocked = true;
            return BootKind::NoSession;
        };
        let Ok(opening) = record_versions(&ir, &hash) else {
            self.save_blocked = true;
            return BootKind::NoSession;
        };
        let outcome = boot_session(
            saved,
            ir,
            &opening,
            &OpeningCaps::current(),
            initial,
            &start,
        );
        let (recorded, kind) = match outcome {
            BootOutcome::Fresh(r) => (r, BootKind::Fresh),
            BootOutcome::Restored { session, .. } => (session, BootKind::Restored),
            BootOutcome::NotOpened { session, .. } => {
                self.save_blocked = true;
                (session, BootKind::NotOpened)
            }
        };
        // 復元した現在の根の下書きを入力欄へ戻す（記録の基準も同じ値にして、戻しただけでは記録を増やさない）。
        let restored_draft = recorded
            .session()
            .state()
            .run
            .current
            .as_ref()
            .map(StepUid::root)
            .and_then(|root| recorded.session().state().drafts.get(&root).cloned());
        #[cfg(feature = "test-hooks")]
        {
            self.boot_kind = Some(kind);
        }
        self.install_recorded(recorded);
        // `install_recorded` が落とした印を、起動の種類で立て直す（案内は Fresh の最初の操作だけ。SAVE-29）。
        self.fresh_start = kind == BootKind::Fresh;
        // 入力欄なしの教材は、復元した下書きを戻す先が無い。
        if let Some(text) = restored_draft.filter(|_| self.input.shows_input_area()) {
            self.input.update(HelloAction::SetDraft(text));
            self.draft_recorded = self.input.draft().to_owned();
        }
        kind
    }

    /// 入力欄の値が最後に記録した下書きと違うときだけ、下書きとして記録する。
    fn record_draft_change(&mut self) {
        if self.input.draft() == self.draft_recorded {
            return;
        }
        let draft = self.input.draft().to_owned();
        if let Some(recorded) = self.model.recorded.as_mut() {
            recorded.draft(draft.clone());
            self.history_changed = true;
        }
        self.draft_recorded = draft;
    }

    /// 離れる直前に保存する値（履歴ファイルと同じ書き出し）。保存しない場合は `None`。
    ///
    /// 保存しない場合: 読み込みの上限（`history_limit::check_save`）を超える、記録した `ops` が 0 件（未操作。最初の変更まで保存しない。SAVE-28）、起動時に開けなかった履歴がある
    /// （上書きすると失う）、包みを組み立てられない・書き出せない（fail-closed）。値は呼び出し側が保存先へ渡すだけで、
    /// ログにも結果にも出さない（履歴は下書きを含む受講者のデータである）。
    pub fn history_to_save(&self) -> Option<String> {
        if self.save_blocked {
            return None;
        }
        let envelope = self.model.recorded.as_ref()?.envelope_for_save()?;
        let bytes = export_bytes(&envelope).ok()?;
        // 読み込めない包みは保存しない。最後に読み込めた保存を残す（`history_limit` の規則 2）。
        history_limit::check_save(bytes.len(), envelope.ops.len()).ok()?;
        String::from_utf8(bytes).ok()
    }

    /// 保存先の鍵。教材の `pack.id` と最初の開始点から求める（`HistoryKey::from_ir`）。IR が無い・開始点が無いときは `None`。
    pub fn history_key(&self, scope: DeliveryScope) -> Option<HistoryKey> {
        HistoryKey::from_ir(scope, self.model.session()?.ir())
    }

    /// 起動の前に、保存先の読み込みに失敗したことを伝える。保存済みの値を読めなかったまま保存すると失うため、保存を止める。
    pub(crate) fn block_saving(&mut self) {
        self.save_blocked = true;
    }

    /// 次に届けるべき進行の宛先。処理中でなければ・未読込なら `None`。
    pub fn next_tick(&self) -> Option<TickAddr> {
        self.model.session().and_then(next_tick)
    }

    /// 履歴の差し替えで進行のタイマーを無効にする通知先を結ぶ。通知先は Runtime にも App にも触れてはならない。
    pub fn set_tick_reset(&mut self, f: Box<dyn Fn()>) {
        self.tick_reset = Some(f);
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

    /// 履歴が変わった `update` の末尾で呼ぶ通知先を結ぶ（変更の後の遅延保存。TASK-318）。
    ///
    /// 通知は `Component::update` の中から呼ばれる。通知先は Runtime にも App にも触れず、タイマーを予約するだけにする（再入になる）。
    /// 起動（`boot`）は `update` を通らないので通知しない（未操作の再読み込みは保存しない。SAVE-28）。
    pub fn set_save_request(&mut self, f: DraftSyncRequest) {
        self.save_request = Some(f);
    }

    /// 即時の保存が済んだときに、予約済みの遅延保存を無効にさせる通知先を結ぶ（同じ履歴の二重の書き込みを避ける）。
    pub fn set_save_cancel(&mut self, f: DraftSyncRequest) {
        self.save_cancel = Some(f);
    }

    /// 変更のたびの保存の口を結ぶ。`persist::boot_and_mount` が mount の前に 1 回だけ呼ぶ。
    ///
    /// 保存の起点は状態を変える操作の `update` の末尾に 1 回で、遅延も畳み込みも無い。下書きだけの変更は `set_save_request` の遅延保存が受ける（TASK-318）。
    pub fn set_history_saver(&mut self, f: HistorySaver) {
        self.history_saver = Some(f);
    }

    /// 非同期の保存の失敗を読む口を結ぶ。`persist::boot_and_mount` が mount の前に 1 回だけ呼ぶ。
    ///
    /// 結んだ時点で 1 回引き、最初の描画から表示に反映する（開けたが読めなかった保存先は、操作の前から理由が出る）。
    pub fn set_save_failure_probe(&mut self, f: SaveFailureProbe) {
        self.save_failure_probe = Some(f);
        self.refresh_save_failure();
    }

    /// 表示する保存の失敗を求め直し、前と違うときだけ `save_failure` を更新して dirty に積む。
    /// 同期の保存の失敗を先に採り、無ければ非同期の probe の値を採る（SAVE-10・SAVE-11）。
    /// `update` の末尾で毎回呼ぶ。非同期の失敗は次の `update` で表示される（IndexedDB の確定が後から届くため）。
    fn refresh_save_failure(&mut self) {
        let failure = self
            .sync_save_failure
            .or_else(|| self.save_failure_probe.as_ref().and_then(|probe| probe()));
        if failure != self.model.save_failure {
            self.model.save_failure = failure;
            self.dirty.push(SAVE_NOTICE_FIELD);
        }
    }

    /// 起動が `Fresh` だったページで最初に状態を変えたとき、書き出しの案内を出す（SAVE-29・D-57）。
    /// 保存を止めた状態でも出す（失う場面ほど必要）。同じ操作で章の終わりが立っていればそれを残す。
    fn note_first_save(&mut self) {
        if !self.fresh_start || self.first_save_hinted {
            return;
        }
        self.first_save_hinted = true;
        if self.model.export_hint.is_none() {
            self.model.export_hint = Some(ExportHint::FirstSave);
            self.dirty.push(EXPORT_NOTICE_FIELD);
        }
    }

    /// 状態を変えた操作の後に履歴を保存し、結果を `save_failure` と dirty に反映する（SAVE-10）。
    ///
    /// 保存する値が無い（未操作・保存停止・上限超過）ときは何もしない。表示も変えない（それぞれ別の規則が持つ）。
    /// 失敗しても状態は巻き戻さない。書けなかった値は注入側の保留に残り、次の変更か離れる直前に再び試される。
    fn save_after_change(&mut self) {
        if self.history_saver.is_none() {
            return;
        }
        let Some(value) = self.history_to_save() else {
            return;
        };
        // 秘密情報らしい文字列は保存先へ積まない（D-44 の失敗側）。形だけを警告に出す。世代・保存の失敗の表示は変えない。
        // 止めた場合も世代を進め、それ以前の値の遅延保存の結果が警告を消さないようにする。
        self.save_epoch = self.save_epoch.wrapping_add(1);
        let held = secret_hold(&value);
        let found = !held.is_empty();
        self.set_secret_warning(held);
        if found {
            return;
        }
        let Some(saver) = self.history_saver.as_ref() else {
            return;
        };
        let failure = saver(&value).err();
        self.set_save_failure(failure);
    }

    /// 警告の形を置き換え、変わったときだけ dirty に積む。
    fn set_secret_warning(&mut self, shapes: Vec<SecretShape>) {
        if self.model.secret_warning != shapes {
            self.model.secret_warning = shapes;
            self.dirty.push(SECRET_NOTICE_FIELD);
        }
    }

    /// 保存の結果（即時・遅延の同期の書き込み）を `sync_save_failure` に入れ、表示する失敗を求め直す。
    fn set_save_failure(&mut self, failure: Option<StorageError>) {
        self.sync_save_failure = failure;
        self.refresh_save_failure();
    }

    /// 今の保存世代。即時の保存のたびに進み、遅延保存の結果の配送が古いかの判定に使う（SAVE-10）。
    pub fn save_epoch(&self) -> u64 {
        self.save_epoch
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

    /// 候補の一覧が開いているか。glue が Tab の判定（補完か通常のフォーカス移動か）に使う（TERM-42）。
    pub fn candidates_open(&self) -> bool {
        self.input.candidates_open()
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
    pub fn engine_snapshot_json(&self) -> String {
        use crate::engine_parts::{artifact_rows, event_kind_name};
        use serde_json::json;

        let Some(session) = self.model.session() else {
            return json!({
                "irLoaded": false,
                "irRefusal": self.ir_refusal,
                "lastImport": self.last_import_json(),
            })
            .to_string();
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
                        .and_then(|e| {
                            crate::log_lines::line_from_event(e, self.model.judged_text(r.event))
                        })
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
                    // 画面の理由・次の行動との照合用（TASK-437）。控えが無ければ null。
                    "reason": self.model.judged_text(i as u32),
                    "nextAction": r.next_action.as_str(),
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
        // 警告の文面（TASK-160）。エンジンの固定文だけで、教材・入力の値は含まない。遷移の上限で止めた警告を、効果の
        // 適用などの他の警告と区別して数えるために、ブラウザ試験が使う。
        let warnings: Vec<_> = log
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match e {
                fandhe_workshop_core::event::Event::Warning { message, .. } => {
                    Some(json!({ "event": i, "message": message }))
                }
                _ => None,
            })
            .collect();
        // 遷移の上限で止めた専用のイベント（SAFE-34）。着地の回数だけで、文面は含まない。DOM の見出しの行・通知欄と
        // 件数を照合するために、ブラウザ試験が使う。
        let limits: Vec<_> = log
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match e {
                fandhe_workshop_core::event::Event::TransitionLimitReached { landings, .. } => {
                    Some(json!({ "event": i, "landings": landings }))
                }
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
            // 警告に出す形の ID だけ（値は入れない。SAFE-27）。
            "secretWarning": self.model.secret_warning.iter().map(|s| s.id()).collect::<Vec<_>>(),
            "irRefusal": self.ir_refusal,
            "bootKind": self.boot_kind.map(BootKind::as_str),
            "lastImport": self.last_import_json(),
            "installCount": self.probe.install_count,
            "logLen": log.len(),
            "judgedCategories": judged_categories,
            "received": received,
            "ignoredReasons": ignored_reasons,
            "warnings": warnings,
            "limits": limits,
            "stateWithoutLog": state_without_log,
            "consumed": bulk.consumed(),
            "generation": session.generation().0,
            "current": session.state().run.current.as_ref().map(|s| s.0.clone()),
            // DOM の `run-status` の `data-state` と照合する（TASK-436）。
            "runStatus": crate::run_status::run_status(Some(session)).data_state(),
            // DOM の `step-status` の語と照合する（TASK-435）。
            "stepStatus": match crate::step_status::step_status(Some(session)) {
                Some(fandhe_workshop_core::step_status::StepStatus::Accepted) => Some("accepted"),
                Some(fandhe_workshop_core::step_status::StepStatus::Achieved) => Some("achieved"),
                _ => None,
            },
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

    /// 履歴ファイルのバイト列を読み込み、開いたときだけ `Session` を差し替える（SAVE-16・SAVE-19）。
    ///
    /// 判定は core の `load_session`（段 1〜8）に任せ、条件は複製しない。開く側の版は、いま載せている教材の IR から求める。
    /// 実行環境・開始時の状態・開く側の版のどれかが無ければ、何も変えずに `Unavailable` を返す（開けない側に倒す）。
    /// 開かない（表示だけ・開かない）ときも、状態・導出した値・dirty のどれにも触れない。
    /// 差し替えでは、導出した値を作り直し、全 field を dirty に積み、次の進行の宛先を通知する。
    /// 保存先へは書かない（離れる直前の書き出しは `persist`。TASK-319）。入力欄の下書きも変えない（SHELL-22・TASK-531）。
    /// 開いた履歴の `ops` の続きに以後の操作を記録する。
    pub fn open_history(&mut self, bytes: &[u8]) -> ImportOutcome {
        let (Some(session), Some(initial)) = (self.model.session(), self.initial.as_ref()) else {
            return ImportOutcome::Unavailable;
        };
        let ir = session.ir().clone();
        let Ok(hash) = content_hash(&ir) else {
            return ImportOutcome::Unavailable;
        };
        let Some((opening, caps)) = self.opening_conditions(&ir, &hash) else {
            return ImportOutcome::Unavailable;
        };
        match load_session(bytes, ir, &opening, &caps, initial.clone()) {
            Ok(loaded) => {
                #[cfg(feature = "test-hooks")]
                {
                    self.probe.last_axes = loaded.notes.as_ref().map(axes_of).unwrap_or_default();
                    self.probe.last_state = serde_json::to_string(loaded.session.state()).ok();
                }
                let outcome = if loaded.notes.is_some() {
                    ImportOutcome::Converted
                } else {
                    ImportOutcome::Open
                };
                // 開いた履歴の `ops` の続きに記録する（開いた後に離れても、開いた内容から続きが復元される）。
                let recorded = RecordedSession::with_recorder(
                    loaded.session,
                    OpsRecorder::from_ops(loaded.ops),
                    loaded.start,
                );
                self.install_recorded(recorded);
                outcome
            }
            Err(Verdict::ViewOnly { code }) => self.closed(ImportOutcome::ViewOnly(code)),
            Err(Verdict::Refused { code }) => self.closed(ImportOutcome::Refused(code)),
            // `load_session` は開く 2 種を `Err` にしない。来ても開かない側に倒す。
            Err(_) => self.closed(ImportOutcome::Unavailable),
        }
    }

    /// 開かなかった結果を返す。試験の設定では、前の読み込みの軸・状態の控えを捨てる（開いていないのに残さない）。
    fn closed(&mut self, outcome: ImportOutcome) -> ImportOutcome {
        #[cfg(feature = "test-hooks")]
        {
            self.probe.last_axes = Vec::new();
            self.probe.last_state = None;
        }
        outcome
    }

    /// 開く側の版と上限。配信する設定では、教材と現行のエンジンから求めた値だけを使う。
    #[cfg(not(feature = "test-hooks"))]
    fn opening_conditions(
        &self,
        ir: &Ir,
        hash: &ContentHash,
    ) -> Option<(HistoryVersions, OpeningCaps)> {
        record_versions(ir, hash)
            .ok()
            .map(|v| (v, OpeningCaps::current()))
    }

    /// 開く側の版と上限。試験の設定では、上書き（[`OpeningOverride`]）があればそれを使う。
    #[cfg(feature = "test-hooks")]
    fn opening_conditions(
        &self,
        ir: &Ir,
        hash: &ContentHash,
    ) -> Option<(HistoryVersions, OpeningCaps)> {
        let o = &self.probe.opening;
        let versions = match &o.engine {
            Some(e) => record_versions_with(ir, *e, hash),
            None => record_versions(ir, hash),
        }
        .ok()?;
        let caps = match &o.vocab_max {
            Some(v) => OpeningCaps::with_vocab_max(*v),
            None => OpeningCaps::current(),
        };
        Some((versions, caps))
    }

    /// 開いた履歴の実行環境に差し替え、導出した値・dirty・進行の宛先を作り直す。
    ///
    /// 下書きの記録の基準は空に戻す。入力欄に文字が残っていれば、次の操作で新しい記録の先頭に下書きとして記録される。
    fn install_recorded(&mut self, recorded: RecordedSession) {
        #[cfg(feature = "test-hooks")]
        {
            self.probe.install_count = self.probe.install_count.saturating_add(1);
        }
        let d = derive(recorded.session());
        self.draft_recorded = String::new();
        self.model.recorded = Some(recorded);
        self.model.projection = d.projection;
        self.model.world = d.world;
        self.model.world_consumed = d.world_consumed;
        // 拒否の文面の控えはイベントの位置がキーのため、差し替え前の控えを残さず新しいログから作り直す。
        self.model.judged_texts = d.judged_texts;
        self.model.view_choice = None;
        // 差し替えた履歴では、前の案内も初回の印も持ち越さない。
        self.model.export_hint = None;
        self.model.secret_warning = Vec::new();
        // 差し替え前に予約された検出・保存結果が、差し替え後の警告・失敗を書き換えないよう世代を進める。
        self.save_epoch = self.save_epoch.wrapping_add(1);
        self.fresh_start = false;
        self.command_prefix = d.command_prefix;
        self.dirty = self.registry.fields().map(|f| f.name).collect();
        // 差し替え前のタイマーは宛先が違えば `Ignored` を新しいログへ足し、一致すれば読み込み後の処理を進めてしまう。先に無効にする。
        if let Some(reset) = self.tick_reset.as_ref() {
            reset();
        }
        if let (Some(notify), Some(session)) = (self.tick_request.as_ref(), self.model.session()) {
            notify(next_tick(session));
        }
    }

    /// 教材ごと差し替える（試験の設定だけ）。開始の状態・補完表・導出した値を新しい教材から作り直す。
    ///
    /// 入力欄の有無は構造の違いで mount 後に変えられない。差し替えでは更新せず、構築時の値を保つ。
    #[cfg(feature = "test-hooks")]
    fn replace_pack(&mut self, session: Session) {
        self.initial = Some(session.state().clone());
        self.input
            .set_command_table(candidates::command_table(session.ir()));
        let start = RecordedSession::first_start(session.ir());
        self.install_recorded(RecordedSession::new(session, start));
    }

    /// 直近の読み込みの結果（試験の設定だけ）。まだ読み込んでいなければ `None`。
    #[cfg(feature = "test-hooks")]
    pub fn last_import(&self) -> Option<ImportOutcome> {
        self.last_import
    }

    /// 直近の読み込みの結果を JSON にする（種類と理由のコードだけ。内容は含めない）。
    #[cfg(feature = "test-hooks")]
    fn last_import_json(&self) -> serde_json::Value {
        match self.last_import {
            None => serde_json::Value::Null,
            Some(o) => serde_json::json!({
                "verdict": o.as_str(),
                "reason": o.reason().map(|r| ReasonCode::as_str(&r)),
                // 変換して開いたときの違いの軸。それ以外は空（理由コードと同じく固定の名前だけ）。
                "axes": self.probe.last_axes,
            }),
        }
    }

    /// 直近に開いた直後（進行の前）の状態の JSON。開いていなければ `None`（写しの一致の確認用。試験の設定だけ）。
    #[cfg(feature = "test-hooks")]
    pub fn last_import_state_json(&self) -> Option<&str> {
        self.probe.last_state.as_deref()
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
        self.step_engine(|session| events_summary(session.apply(op)))
    }

    /// 送信の本文を記録してから、操作プロファイルの接頭辞で分類して適用する。戻りは `apply_engine` と同じ。
    fn submit_engine(&mut self, prefix: &str, text: String) -> bool {
        self.step_engine(|session| events_summary(session.submit(prefix, text)))
    }

    /// 再読み込みの模擬（試験の設定だけ）。`apply_engine` と同じ後処理を行い、新しい世代の宛先を通知する。
    /// 記録（`ops`）にも `Reload` を足す（SAVE-4）。
    ///
    /// 再読み込みの前に glue が予約したタイマーは取り消されず、古い宛先のまま後から届く。届いた分は core が
    /// `Ignored(Stale)` として記録する（EVT-6・EVT-8・EVT-12）。
    #[cfg(feature = "test-hooks")]
    fn reload_engine(&mut self) {
        self.step_engine(|session| events_summary(session.reload()));
    }

    /// `Session` を 1 段進める共通の後処理。`f` が返したイベント列を使い、projection・world・dirty・宛先の通知を更新する。
    /// 状態を変える口（`apply_engine`・`reload_engine`）だけが呼ぶ。未読込なら何もしない。戻りは拒否の判定が出たか。
    fn step_engine(&mut self, f: impl FnOnce(&mut RecordedSession) -> (usize, bool)) -> bool {
        if self.model.recorded.is_none() {
            return false;
        }
        let before = self.bound_snapshot();
        let prev_view = self.model.selected_view();
        let prev_step = self
            .model
            .session()
            .and_then(|s| s.state().run.current.clone());
        let Some(recorded) = self.model.recorded.as_mut() else {
            return false;
        };
        let len_before = recorded.session().log().len();
        let (returned, rejected) = f(recorded);
        self.history_changed = true;
        let session = recorded.session();
        // extend はログの全体を受け取る（処理済みの先頭は読み飛ばす）。返ったイベントの列を渡さない。
        let log = session.log();
        self.model.projection.extend(log);
        let changed = returned > 0 || log.len() != len_before;
        // 拒否の理由の文面は、適用の直後（拒否は状態を変えないため、判定の直前と同じ状態）でだけ作れる。
        record_judged_texts(&mut self.model.judged_texts, session, len_before);
        // 世界状態も同じ列から、未処理の分だけ導く。session の借用中のため、欄ごとに触る。
        let from = self.model.world_consumed.min(log.len());
        replay_changes(&mut self.model.world, &log[from..]);
        self.model.world_consumed = log.len();
        // 今の Step が選んだときの Step と違えば、ビューの選択を捨てる（ART-12。移動すると宣言の先頭に戻る）。
        // 読まないだけで残すと、元の Step へ戻ったときに以前の選択が戻ってしまう。Step を変える口はすべてここを通り、
        // 差し替え（`install_recorded`）と構築は選択を持たずに始める。dirty は下の `changed` が list をすべて積む。
        if self
            .model
            .view_choice
            .as_ref()
            .is_some_and(|c| Some(&c.step) != session.state().run.current.as_ref())
        {
            self.model.view_choice = None;
        }
        // 今の操作で伸びたログに章の終わりがあれば、書き出しを案内する（SAVE-29。起動時に復元した分は見ない）。
        if chapter_end_in(session.ir(), &log[len_before.min(log.len())..]) {
            self.model.export_hint = Some(ExportHint::ChapterEnd);
        }
        for (name, old) in before {
            if self.registry.bound_value(name, &self.model) != old {
                self.dirty.push(name);
            }
        }
        // 次の進行の宛先を通知する（`None` も渡す。予約済みの印を glue が消すため）。
        if let (Some(notify), Some(session)) = (self.tick_request.as_ref(), self.model.session()) {
            notify(next_tick(session));
        }
        if changed {
            self.save_due = true;
            self.push_changed_lists(prev_view, prev_step);
        }
        rejected
    }

    /// ログが伸びたときに dirty へ積む keyed list を決める（ART-5・ART-12）。
    ///
    /// ビューの list 7 本は `dirty_list_fields` で絞る（ビューが変われば旧と新、同じなら今のビューだけ）。
    /// 選択のボタンの list は、Step かビューが変わったときだけ積む（宣言の集合は Step で決まるため）。
    /// それ以外の list（ログ・選択肢・候補など）は従来どおりすべて積む。行の中身が変わったかは再導出せず
    /// `changed`（ログが伸びた）で代用する。行は世界状態と印だけから導かれ、どちらもログが伸びたときにしか
    /// 変わらない。迷う側は積みすぎる側で、束縛点として解決できるため全再描画にはならない。
    fn push_changed_lists(&mut self, prev_view: ViewKind, prev_step: Option<StepUid>) {
        let next_view = self.model.selected_view();
        let next_step = self
            .model
            .session()
            .and_then(|s| s.state().run.current.clone());
        let view_lists: Vec<&'static str> = VIEW_LIST_FIELDS.iter().map(|f| f.name).collect();
        let narrowed = dirty_list_fields(Some(prev_view), Some(next_view), true);
        let select_changed = prev_step != next_step || prev_view != next_view;
        let lists: Vec<&'static str> = self
            .registry
            .fields()
            .filter(|f| f.kind == FieldKind::KeyedList)
            .map(|f| f.name)
            .filter(|n| {
                if view_lists.contains(n) {
                    narrowed.contains(n)
                } else if *n == view_select::VIEW_SELECT_FIELD.name {
                    select_changed
                } else {
                    true
                }
            })
            .collect();
        self.dirty.extend(lists);
    }

    /// ビューを選ぶ。session があり、今の Step の宣言に含まれ、今の選択と違うときだけ状態を変える。
    /// dirty は旧と新のビューの list と選択のボタンの list である（上流が親の属性を同期する束縛点）。
    fn select_view(&mut self, kind: ViewKind) {
        let Some(session) = self.model.session() else {
            return;
        };
        let Some(step) = session.state().run.current.clone() else {
            return;
        };
        let views = view_select::declared_views(session);
        let declared = declared_kinds(&views);
        if !view_select::selectable(&declared).contains(&kind) {
            return;
        }
        let prev = self.model.selected_view();
        if prev == kind {
            return;
        }
        self.model.view_choice = Some(ViewChoice { step, kind });
        self.dirty
            .extend(dirty_list_fields(Some(prev), Some(kind), false));
        self.dirty.push(view_select::VIEW_SELECT_FIELD.name);
    }

    /// 処理中の Step があるか（Esc・停止ボタンが停止を送るかの判断に使う）。
    fn is_busy(&self) -> bool {
        self.model
            .session()
            .is_some_and(|s| s.state().run.busy.is_some())
    }

    /// 入力欄の dirty を、既に積んだ分と重複させずに足す。
    fn extend_input_dirty(&mut self) {
        for f in self.input.dirty_fields() {
            if !self.dirty.contains(f) {
                self.dirty.push(f);
            }
        }
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
        if self.model.recorded.is_some() {
            match self.command_prefix.clone() {
                None => clear = false,
                Some(prefix) if !text.trim().is_empty() => {
                    // 直前の下書きを先に記録してから送信する（`ops` は操作の順のまま残す）。
                    self.record_draft_change();
                    let rejected = self.submit_engine(&prefix, text);
                    // 送信の記録は、その根の下書きを写しから除く。入力欄に残る下書き（拒否のとき）は、
                    // この後の `record_draft_change` が記録し直す。
                    self.draft_recorded.clear();
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

/// 復元で、ログ全体の拒否・問い返しの文面を core に作り直させて控える。
///
/// 各判定の時点の状態の組み直しは core の [`filled_texts_of_log`] が持つ。作れなかった拒否・問い返しは控えず、
/// 行を出さない（fail-closed）。
fn restore_judged_texts(texts: &mut BTreeMap<u32, String>, session: &Session) {
    for (i, f) in filled_texts_of_log(session.ir(), session.log()) {
        if let Ok(key) = u32::try_from(i) {
            texts.insert(key, f.message);
        }
    }
}

/// `from` 以降の新しいイベントのうち、`InputReceived` の直後の拒否・問い返しの `Judged` について文面を充填して控える。
///
/// 充填に失敗した（`Err`・`None`）拒否・問い返しは控えない。問い返しも状態を変えないため、適用直後の状態で充填できる。エラーは文面も入力値も含まないが、ここでは出力しない。
fn record_judged_texts(texts: &mut BTreeMap<u32, String>, session: &Session, from: usize) {
    let log = session.log();
    for i in from..log.len() {
        let (Some(Event::InputReceived { input, .. }), Some(judged)) =
            (i.checked_sub(1).and_then(|p| log.get(p)), log.get(i))
        else {
            continue;
        };
        let Event::Judged { judgment, .. } = judged else {
            continue;
        };
        if !keeps_text(judgment) {
            continue;
        }
        let Ok(key) = u32::try_from(i) else {
            continue;
        };
        if let Ok(Some(f)) = filled_text(session.ir(), &session.state().run, input, judgment) {
            texts.insert(key, f.message);
        }
    }
}

/// 文面を控える判定か。拒否と問い返しだけで、受付や変化は応答の行の担当（TASK-435・TASK-438）。
fn keeps_text(j: &Judgment) -> bool {
    matches!(j, Judgment::Rejected(_) | Judgment::Clarified(_))
}

impl Component for EngineHost {
    type Action = HostAction;

    fn update(&mut self, action: Self::Action) {
        self.dirty.clear();
        match action {
            // 入力欄なしの教材（PACK-20）は、口が無いので下書き・送信・補完を受けても状態を変えない（fail-closed）。
            HostAction::Input(HelloAction::SetDraft(_) | HelloAction::Submit)
            | HostAction::Complete
                if !self.input.shows_input_area() => {}
            // 書き込みの後に即時の保存が走っていれば（世代が進んでいる）、古い結果は捨てる。新しい通知を消さない・古い失敗を出さない。
            HostAction::SaveResult(epoch, failure) => {
                if epoch == self.save_epoch {
                    // 書き込みが走った値は、検出に掛からなかった値である。
                    self.set_secret_warning(Vec::new());
                    self.set_save_failure(failure);
                }
            }
            // 履歴の差し替え・即時の保存で世代が進んでいれば、古い検出は捨てる（存在しない警告を出さない）。
            HostAction::SecretHold(epoch, shapes) => {
                if epoch == self.save_epoch {
                    self.set_secret_warning(shapes);
                }
            }
            HostAction::Input(HelloAction::Submit) => self.submit(),
            HostAction::Input(HelloAction::Esc) => {
                let busy = self.is_busy();
                let route = self.input.esc(busy);
                // 処理中の停止と、処理中でなく候補が閉じているときの取り消しの入力（TASK-441 2/2。TERM-8）を
                // Session へ適用する。候補を閉じるだけの Esc は op を持たず、ログを増やさない。取り消しの規則が
                // 無い Step の拒否は core が記録する。許可する op を明示し、将来の行き先が黙って流れないようにする。
                if let Some(op @ (Op::Control(Control::Stop) | Op::Input(Input::Cancel))) = route.op
                {
                    self.apply_engine(op);
                }
                self.extend_input_dirty();
            }
            HostAction::Input(HelloAction::Stop) => {
                if self.is_busy() {
                    self.apply_engine(Op::Control(Control::Stop));
                }
            }
            HostAction::Input(a) => {
                self.input.update(a);
                self.dirty.extend_from_slice(self.input.dirty_fields());
            }
            HostAction::Engine(op) => {
                self.apply_engine(op);
            }
            HostAction::SelectView(kind) => self.select_view(kind),
            #[cfg(feature = "test-hooks")]
            HostAction::Reload => self.reload_engine(),
            #[cfg(feature = "test-hooks")]
            HostAction::OpenHistory(text) => {
                self.last_import = Some(self.open_history(text.as_bytes()));
            }
            #[cfg(feature = "test-hooks")]
            HostAction::LoadIr(text) => {
                match crate::test_fixture::load_session_from_ir_bytes(text.as_bytes()) {
                    Ok(Some(session)) => self.replace_pack(session),
                    // 読めない IR は session を差し替えず、理由コードだけを控える（履歴・画面の構造は変えない）。
                    Err(e) => self.ir_refusal = Some(e.reason_code()),
                    Ok(None) => {}
                }
            }
            #[cfg(feature = "test-hooks")]
            HostAction::OpeningOverride(o) => self.probe.opening = o,
            #[cfg(feature = "test-hooks")]
            HostAction::LoadFixture(name) => {
                let built = match name {
                    FixtureName::Answer => crate::test_fixture::answer_session(),
                    FixtureName::Approval => crate::test_fixture::approval_session(),
                    FixtureName::Views => crate::test_fixture::views_session(),
                    FixtureName::UiAction => crate::test_fixture::ui_action_session(),
                    FixtureName::CycleEnterSelf => {
                        crate::test_fixture::cycle_session(crate::test_fixture::CycleIr::EnterSelf)
                    }
                    FixtureName::CycleEnter2 => {
                        crate::test_fixture::cycle_session(crate::test_fixture::CycleIr::Enter2)
                    }
                    FixtureName::CycleExit2 => {
                        crate::test_fixture::cycle_session(crate::test_fixture::CycleIr::Exit2)
                    }
                    FixtureName::CycleD07 => {
                        crate::test_fixture::cycle_session(crate::test_fixture::CycleIr::D07)
                    }
                    FixtureName::Chain1000 => crate::test_fixture::chain_session(999),
                    FixtureName::Chain1001 => crate::test_fixture::chain_session(1000),
                };
                if let Some(session) = built {
                    self.replace_pack(session);
                }
            }
            // 補完は入力欄だけの操作で、エンジンへは何も渡さない（ログ・世代・projection は変わらない）。
            // 下書きが変わったときだけ、`<textarea>` への書き戻しを頼む（上流は値を同期しない）。
            HostAction::Complete => {
                #[cfg(feature = "test-hooks")]
                {
                    self.complete_count = self.complete_count.saturating_add(1);
                }
                if self.input.complete() {
                    self.extend_input_dirty();
                    if let Some(f) = self.draft_sync_request.as_ref() {
                        f();
                    }
                }
            }
        }
        // 入力欄の値が変わっていれば下書きとして記録する。どの action の後でも 1 か所で行う。
        self.record_draft_change();
        // 状態を変えた操作の後は、下書きの記録も済んだ値を保存する（SAVE-10）。遅延保存の予約は、この即時の保存が書くので不要になる。
        if self.save_due {
            self.save_due = false;
            self.note_first_save();
            self.history_changed = false;
            if let Some(f) = self.save_cancel.as_ref() {
                f();
            }
            self.save_after_change();
        } else if self.history_changed {
            // 下書きだけの変更は、打鍵ごとに書かず遅延保存へ畳む（TASK-318。SAVE-7）。
            self.history_changed = false;
            if let Some(f) = self.save_request.as_ref() {
                f();
            }
        }
        // 非同期の保存の失敗（IndexedDB）の表示を、打鍵・送信・進行のどの `update` でも求め直す（SAVE-11）。
        self.refresh_save_failure(); // update の末尾
    }

    fn view(&self) -> Node {
        // 教材を読めずに起動したホストは、教材を描かず通知だけを出す（入力欄・部品・raw-trigger は描かない）。
        if let (None, Some(code)) = (self.model.session(), self.ir_refusal) {
            return el(
                "div",
                vec![("data-testid", "workshop-root")],
                vec![el(
                    "p",
                    vec![("data-testid", "ir-notice"), ("role", "status")],
                    vec![fandhe_frontend_core::text(
                        crate::ir_notice::ir_notice_text(code),
                    )],
                )],
            );
        }
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
        // `main` の外の部品（`workshop-parts`）の Esc も上流の属性で届けるため、Esc の keydown 属性を root にも付ける。
        // `main` の属性と入れ子になっても、上流は最も近い 1 つにだけ届ける（二重に届かない。FEND-5・FEND-6・TASK-353 2/2）。
        // 教材を読めない場合の通知の root には付けない（入力欄が無く、Esc の宛先が無い）。
        let mut root_attrs = vec![("data-testid", "workshop-root")];
        root_attrs.extend(ESC_KEYDOWN_ATTRS);
        el("div", root_attrs, children)
    }

    fn decode_action(name: &str, payload: &str) -> Option<Self::Action> {
        // 試験の設定だけ: `Op` の JSON をそのまま core へ送る（BUILD-27）。大きさ・深さの関門を通し、
        // 復号できなければ何もしない（安全側）。
        #[cfg(feature = "test-hooks")]
        if name == "raw" {
            use fandhe_workshop_contract::limits::LoadEntry;
            // 再読み込みの模擬の合図は完全一致だけ。これ以外は `Op` として復号する。
            if payload == RAW_RELOAD {
                return Some(HostAction::Reload);
            }
            // 履歴ファイルの読み込みは接頭辞が一致したときだけ。本文の大きさ・深さ・JSON は `open_history` の段 1〜3 が断る。
            if let Some(body) = payload.strip_prefix(RAW_OPEN_HISTORY) {
                return Some(HostAction::OpenHistory(body.to_owned()));
            }
            // 任意の教材（IR の JSON）への差し替え。本文は `load_ir` の関門が断る（読めなければ何もしない）。
            if let Some(body) = payload.strip_prefix(RAW_LOAD_IR) {
                return Some(HostAction::LoadIr(body.to_owned()));
            }
            // 開く側の版の上書き。形が違えば何もしない（判定が食い違って試験が落ちる側に倒す）。
            if let Some(body) = payload.strip_prefix(RAW_OPENING_OVERRIDE) {
                return parse_opening_override(body).map(HostAction::OpeningOverride);
            }
            // 試験用の教材の差し替えは接頭辞が一致したときだけ。未知の名前は何もしない。
            if let Some(name) = payload.strip_prefix(RAW_LOAD_FIXTURE) {
                return match name {
                    "answer" => Some(HostAction::LoadFixture(FixtureName::Answer)),
                    "approval" => Some(HostAction::LoadFixture(FixtureName::Approval)),
                    "views" => Some(HostAction::LoadFixture(FixtureName::Views)),
                    "ui-action" => Some(HostAction::LoadFixture(FixtureName::UiAction)),
                    "cycle-enter-self" => {
                        Some(HostAction::LoadFixture(FixtureName::CycleEnterSelf))
                    }
                    "cycle-enter-2" => Some(HostAction::LoadFixture(FixtureName::CycleEnter2)),
                    "cycle-exit-2" => Some(HostAction::LoadFixture(FixtureName::CycleExit2)),
                    "cycle-d07" => Some(HostAction::LoadFixture(FixtureName::CycleD07)),
                    "chain-1000" => Some(HostAction::LoadFixture(FixtureName::Chain1000)),
                    "chain-1001" => Some(HostAction::LoadFixture(FixtureName::Chain1001)),
                    _ => None,
                };
            }
            return fandhe_workshop_core::load_gate::load_json::<Op>(
                LoadEntry::History,
                payload.as_bytes(),
            )
            .ok()
            .map(HostAction::Engine);
        }
        // ビューの選択（TASK-470）。payload は種類の綴りの完全一致だけ。宣言に含まれるかは `update` が照合する。
        if name == view_select::ACTION_SELECT_VIEW {
            return ViewKind::parse(payload).map(HostAction::SelectView);
        }
        // 指せる行のクリック（TASK-480 2/2）。復号は `PointTarget::decode` の 1 か所だけで、正規形でないものは何もしない。
        // 宛先が今の Step に合うかの照合と判定は core（`select_judge`）に任せ、ここでは判定しない。
        if name == POINT_ACTION {
            return PointTarget::decode(name, payload)
                .map(|t| HostAction::Engine(Op::Input(t.to_input())));
        }
        // 選択肢のクリック（TASK-445 2/2）。payload は選択肢の ID で、照合と判定は core に任せる。空は何もしない。
        if name == crate::offers::ACTION_CHOOSE {
            return (!payload.is_empty())
                .then(|| HostAction::Engine(Op::Input(Input::Choice(payload.to_owned()))));
        }
        // 承認のクリック（TASK-447 1/2）。`decide` の payload は `approve`・`reject`・`revise` の完全一致だけ。
        // 対象つきは payload が対象そのもの（分解しない）。照合と判定は core に任せ、空・未知の値は何もしない。
        if name == crate::offers::ACTION_DECIDE {
            return crate::terminal_area::decision_from_value(payload).map(|decision| {
                HostAction::Engine(Op::Input(Input::Approval {
                    decision,
                    target: None,
                }))
            });
        }
        if name == crate::offers::ACTION_REVISE_TARGET {
            return (!payload.is_empty()).then(|| {
                HostAction::Engine(Op::Input(Input::Approval {
                    decision: Decision::Revise,
                    target: Some(payload.to_owned()),
                }))
            });
        }
        // 遅延保存の結果（SAVE-10）。payload は完全一致だけを受け、未知の値は何もしない。
        // 遅延保存の検出（SAFE-27）。payload は完全一致だけを受け、未知の値は何もしない。
        if name == crate::secret_notice::ACTION_SECRET_HOLD {
            return crate::secret_notice::decode_secret_hold(payload)
                .map(|(epoch, shapes)| HostAction::SecretHold(epoch, shapes));
        }
        if name == crate::save_notice::ACTION_SAVE_RESULT {
            return crate::save_notice::decode_save_result(payload)
                .map(|(epoch, failure)| HostAction::SaveResult(epoch, failure));
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

/// 返ったイベントの列から、件数と、拒否の判定（`Judged` の `Rejected`）が含まれるかを求める。
fn events_summary(events: &[Event]) -> (usize, bool) {
    let rejected = events.iter().any(|e| {
        matches!(
            e,
            Event::Judged {
                judgment: Judgment::Rejected(_),
                ..
            }
        )
    });
    (events.len(), rejected)
}
