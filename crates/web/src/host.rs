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
//! 拒否で残した下書きは全体を選択状態にし（D-67。JUDGE-27・TERM-22）、その要求は `set_draft_select_request` の通知先へ渡す。
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

use crate::shell_notice::RefusedNotice;
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
use fandhe_workshop_contract::load_reason::{HistoryLoadStage, LoadReason};
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
use fandhe_workshop_core::learner_os::LearnerOs;
use fandhe_workshop_core::ops_record::OpsRecorder;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::text_fill::{filled_text, filled_texts_of_log_at};
use fandhe_workshop_core::version_record::record_versions;
#[cfg(feature = "test-hooks")]
use fandhe_workshop_core::version_record::record_versions_with;
use fandhe_workshop_core::world::World;
use fandhe_workshop_core::{Projection, Session, SessionState};

use crate::a11y::{self, KeyModifiers, PaneTab, TabKey};
use crate::action::{RuntimeAction, ESC_KEYDOWN_ATTRS};
use crate::artifact_view::{declared_kinds, dirty_list_fields, ViewKind, VIEW_LIST_FIELDS};
use crate::artifact_view::{PointTarget, POINT_ACTION};
use crate::candidates;
use crate::checkpoint_ops::{checkpoint_section, CheckpointOffer, CHECKPOINT_OPS_FIELD};
use crate::engine_parts::standard_registry;
use crate::export_notice::{chapter_end_in, ExportHint, EXPORT_NOTICE_FIELD};
use crate::hello::{HelloAction, HelloView, HELLO_FIELDS};
use crate::history_export::ExportGate;
use crate::history_ops::{history_ops, ExportResult, EXPORT_RESULT_FIELD};
use crate::next_chapter::{next_chapter, next_chapter_list, NextChapter, NEXT_CHAPTER_FIELD};
use crate::parts::{FieldKind, PartRegistry};
use crate::profile::{InputField, LeftPane};
use crate::secret_notice::{unacknowledged, SECRET_CHOICE_FIELD, SECRET_NOTICE_FIELD};
use crate::shell_frame::TOP_PLACE_FIELD;
use crate::shell_frame::{pane_from_payload, ACTION_PANE_KEY, ACTION_SELECT_PANE, PANE_TABS_FIELD};
use crate::shell_notice::SHELL_NOTICES_FIELD;
use crate::stale_notice::STALE_NOTICE_FIELD;
use crate::step_body::STEP_BODY_FIELD;
use crate::storage_select::SyncKv;
use crate::tick_schedule::next_tick;
use crate::view_only_notice::VIEW_ONLY_NOTICE_FIELD;
use crate::view_select::{self, ViewChoice};
use fandhe_workshop_contract::profile::InputFieldMode;
use fandhe_workshop_core::secret_scan::{shapes_of, Finding, SecretShape};

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
    /// 左エリアの設定（`world_list`・`view_tabs`。PACK-22）。教材から導く値で、履歴の差し替えで読み直す。
    left_pane: LeftPane,
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
    /// 直近の保存の失敗の種類（SAVE-10・SAVE-11）。`shell-notice` の `save-failed` 項目の元で、履歴（`ops`）には記録しない表示の状態。
    /// 保存に成功すると `None` に戻る。同期の保存の結果（`EngineHost::save_after_change`）と、非同期の保存の失敗の probe
    /// （`EngineHost::refresh_save_failure`）から決まる。
    save_failure: Option<StorageError>,
    /// 履歴の書き出しの案内（SAVE-29・D-57。`export-notice` の元）。表示の状態で、履歴（`ops`）にも保存先にも書かない。
    /// 次の起点・履歴の差し替えで置き換わり、再読み込みでは出ない（消える時機は仮置き）。
    export_hint: Option<ExportHint>,
    /// 保存の直前に見つけた秘密情報らしい文字列の形（SAFE-27・TASK-164 1/3。`secret-notice` の元）。形だけを持ち、値は持たない。
    /// 表示の状態で、履歴（`ops`）にも保存先にも書かない。見つけた間は、その値を保存先へ積まない。
    secret_warning: Vec<SecretShape>,
    /// このタブが他のタブに追い越された（古いタブ）ことの表示（SAVE-13・D-56。`stale-notice` の元）。
    /// 表示の状態で、履歴（`ops`）にも保存先にも書かない。一度立てたら戻さない（`BootGuard` の stale も戻らない）。
    stale_tab: bool,
    /// `prefers-reduced-motion: reduce` のとき真（TASK-372 1/2）。表示だけの値で、`ops`・履歴・保存先の鍵には入れない。
    reduced_motion: bool,
    /// 表示だけで開いた履歴の理由（VER-8・TASK-176 1/2。`view-only-notice` の元）。`Some` の間は、操作の関門（`update`）が
    /// 状態・ログを変える action を捨て、保存と進行も止める。表示の状態で、履歴（`ops`）にも保存先にも書かない。
    /// 開ける履歴の読み込み（`install_recorded`）で `None` に戻る。
    view_only: Option<LoadReason>,
}

impl HostModel {
    /// 表示だけで開いた理由。表示だけでなければ `None`（`view-only-notice` が空になる）。
    pub fn view_only_reason(&self) -> Option<LoadReason> {
        self.view_only
    }

    /// 警告に出す形。無ければ空（`secret-notice` が空になる）。
    pub fn secret_warning(&self) -> &[SecretShape] {
        &self.secret_warning
    }

    /// 動きの抑制のとき真（TASK-372 1/2）。真なら段階の行を出さない。
    pub fn reduced_motion(&self) -> bool {
        self.reduced_motion
    }

    /// 古いタブか。真なら `stale-notice` に文が出る。
    pub fn stale_tab(&self) -> bool {
        self.stale_tab
    }

    /// 書き出しの案内の種類。無ければ `None`（`export-notice` が空になる）。
    pub fn export_hint(&self) -> Option<ExportHint> {
        self.export_hint
    }

    /// 左の成果物で今見せるビュー。保持した選択が今の Step のもので、今の宣言に含まれるときだけそれを返し、
    /// それ以外は既定のビュー（宣言の先頭。`view_select::effective_view`）を返す。
    pub fn selected_view(&self) -> ViewKind {
        view_select::effective_view(
            self.session(),
            self.view_choice.as_ref(),
            self.left_pane.view_tabs,
        )
    }

    /// 左エリアの設定（`pack.left_pane`）。読めない設定は既定である。
    pub fn left_pane(&self) -> LeftPane {
        self.left_pane
    }

    /// 直近の保存の失敗の種類。失敗していなければ `None`（`shell-notice` の `save-failed` 項目が消える）。
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

/// 画面に出している書き出しの結果。リンクは URL の破棄の責務（`link`）を持つ。
enum ExportShown {
    /// 受講者が押すリンク。
    Link {
        /// ファイル名。
        file_name: String,
        /// URL と破棄の責務。
        link: ExportLink,
    },
    /// 出せない理由の固定の文。
    Status(&'static str),
}

/// ホストが受ける action。
#[derive(Debug)]
pub enum HostAction {
    /// 履歴の書き出し（`export-history`。TASK-531 1/2）。`export_live` の関門を通し、出せるときだけリンクを出す。
    ExportHistory,
    /// 履歴の読み込み（`import-history`。TASK-531 1/2）。本文はファイルの文字列そのもので、`open_history` の段 1〜8 が断る。
    ImportHistory(String),
    /// 入力欄の action。
    Input(HelloAction),
    /// core への操作。
    Engine(Op),
    /// 左の成果物のビューの選択（TASK-470）。今の Step が宣言した種類だけを受け、それ以外は何も変えない。
    SelectView(ViewKind),
    /// 「次の章へ進む」の押下（`next-chapter`。TASK-526 2/2）。payload は開始点の ID で、今の提示との照合は `update` が行う。
    NextChapter(String),
    /// チェックポイントの保存（`save-checkpoint`。TASK-529 1/2）。今の提示に保存のボタンがあるときだけ `Control::Save` を送る。
    SaveCheckpoint,
    /// チェックポイントへの復帰（`restore-checkpoint`。TASK-529 1/2）。payload は正規形の ID で、今の提示との照合は `update` が行う。
    RestoreCheckpoint(u32),
    /// チェックポイントの一覧のページ送り（`checkpoint-page`。FEND-22）。payload は正規形のページ番号で、ページ数との照合は `update` が行う。
    CheckpointPage(usize),
    /// 960px 未満のタブのクリック（`select-pane`。TASK-517 2/2）。表示の状態だけを変える。
    SelectPane(PaneTab),
    /// 枠の通知の「閉じる」（`dismiss-notice`。TASK-535 1/2）。いま描いている種類（変換して開いた）だけを消し、状態・履歴には触れない。
    DismissNotice(crate::shell::NoticeKind),
    /// tablist の上の矢印・Home・End（`pane-key`。TASK-517 2/2）。選択の移り先は `a11y::tab_key_target` が決める。
    PaneKey(TabKey),
    /// 遅延保存の結果（SAVE-10）。先頭は書き込んだ時点の保存世代で、今の世代と違えば古い結果として捨てる。
    /// `None` は成功で通知を消し、`Some` は失敗の種類で通知を出す。
    SaveResult(u64, Option<StorageError>),
    /// 遅延保存の直前の検出（SAFE-27・TASK-164 1/3）。見つけた形だけを持ち、値は持たない。
    SecretHold(u64, Vec<SecretShape>),
    /// 「このまま保存する」（SAFE-28）。警告の対象の値を確認済みにして保存する。payload は読まない。
    SecretKeep,
    /// 「この入力を残さない」（SAFE-28）。保存せずに保存先の内容から読み直す。payload は読まない。
    SecretDiscard,
    /// 「この入力を残さない」の保存先の取り直しの完了（`true` は取り直せた）と、頼んだときの世代。今の要求の世代と違う完了は捨てる。
    SecretDiscardRead(bool, u64),
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
    /// 入力方式が `approval` で、「直す」の候補と表示名（誤った候補つき）を宣言した Step の教材（TASK-448 1/2）。
    ApprovalCandidates,
    /// 差し戻しの Step の次に、再承認の Step を置いた教材（TASK-448 2/2）。
    ApprovalReapprove,
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
/// 後ろは [`FixtureName`] の名前（`answer`・`approval`・`approval-candidates`・`approval-reapprove`・`views`・`ui-action`・`cycle-enter-self`・`cycle-enter-2`・`cycle-exit-2`・`cycle-d07`・`chain-1000`・`chain-1001`）だけ。`Op` の JSON は `{` か `[` で始まるため衝突しない。
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
    /// 左エリアの設定（PACK-22・TASK-481 1/2）。読めない設定は既定（`shown`・`all`）に倒す。
    left_pane: LeftPane,
}

/// 教材の `pack.left_pane` を読む。読めない設定（未知の値・型違い）は既定に倒す（fail-closed）。
fn read_left_pane_setting(session: &Session) -> LeftPane {
    fandhe_workshop_contract::left_pane::read_left_pane(session.ir().pack.left_pane.as_ref())
        .unwrap_or_default()
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
        left_pane: read_left_pane_setting(session),
    }
}

/// 次の進行の宛先の通知先。`apply_engine` の後に、次に届けるべき進行（無ければ `None`）を渡す。
///
/// glue が `set_timeout` の予約に使う。ホストは予約の状態を持たず、重複の抑止も glue が受け持つ。
pub type TickRequest = Box<dyn Fn(Option<TickAddr>)>;

/// 下書きの書き戻しの要求の通知先。送信で下書きを空にしたとき、`update` の中から呼ばれる。
/// 拒否の後に下書きの全体を選択状態にする要求（`set_draft_select_request`）の通知先にも同じ型を使う。
///
/// glue が `<textarea>` の値の同期・選択（タイマー経由）に使う。通知先は Runtime にも App にも触れてはならない（再入になる）。
pub type DraftSyncRequest = Box<dyn Fn()>;

/// 変更のたびに履歴の値を保存先へ書く口（SAVE-10。TASK-323 1/2）。引数は `history_to_save` の値で、結果は書けたかどうか。
///
/// 実体は `persist` が mount の前に注入する（保留と保存先を持つのは `persist` だけ）。書けなかった値は注入側が保留に残す。
/// 値・鍵の文字列は結果に含めない（`StorageError` は種類だけ）。`Runtime` にも `App` にも触れてはならない（再入になる）。
pub type HistorySaver = Box<dyn Fn(&str) -> Result<(), StorageError>>;

/// 保存済みの履歴を読む口（「この入力を残さない」の読み直し用。SAFE-28）。`persist::boot_and_mount` が結ぶ。
///
/// 起動の読み取り（`persist::read`）と同じ値を返す。読めなかったときは `Err` で、ホストは新しく始めて保存を止める（fail-closed）。
/// `Runtime` にも `App` にも触れてはならない（再入になる）。
pub type SavedReader = Box<dyn Fn() -> Result<Option<String>, StorageError>>;

/// 保存先の実データの取り直しを頼む口（「この入力を残さない」。SAFE-28）。`persist::boot_and_mount` が結ぶ。
///
/// IndexedDB は取得の完了が非同期で、保存先の写しは他のタブの保存や自タブの書き込みの失敗を反映していない。呼ぶと取り直しを始め、
/// 完了したら `secret-discard-read` を配送する。ホストは届くまで読み直さない。未設定なら、`SavedReader` をその場で読む。
pub type SavedRefresh = Box<dyn Fn(u64)>;

/// 非同期の保存の失敗（IndexedDB。SAVE-11）を読む口。読み取り専用で、`update` のたびに引かれる。
///
/// IndexedDB の確定は後から届き、そのコールバックからホストへは触れない（`state_paths`）。そのため実体（`persist`）は失敗の種類を
/// 保存先に残し、ホストがこの口で引く。`Runtime` にも `App` にも触れてはならない（再入になる）。
pub type SaveFailureProbe = Box<dyn Fn() -> Option<StorageError>>;

/// このタブが古いタブ（起動番号の照合で追い越された）かを読む口（SAVE-13・D-56）。読み取り専用で、`update` のたびに引かれる。
///
/// 実体（`persist`）は `BootGuard` が覚えた真偽を返すだけで、保存領域は読まない。
/// `Runtime` にも `App` にも触れてはならない（再入になる）。
pub type StaleProbe = Box<dyn Fn() -> bool>;

/// 書き出しのバイト列からダウンロード用の URL を作る口（TASK-531 1/2）。未設定・作れないときは `None`（リンクを出さない）。
///
/// ホストは native でも試験するため `web_sys` に触れず、wasm 側（`glue`）が `history_download::object_url` を包んで結ぶ。
pub type ExportUrlMaker = Box<dyn Fn(&[u8]) -> Option<ExportLink>>;

/// 作った書き出しの URL と、その破棄の責務を持つ値（drop で破棄する）。
pub struct ExportLink {
    /// `blob:` URL。
    pub url: String,
    /// drop で URL を破棄する値（wasm では `ExportUrl`）。ホストは中身を読まず、リンクを外すときに捨てる。
    pub guard: Box<dyn std::any::Any>,
}

/// 送信の後に下書きを残すか。エンジンが拒否したときだけ残す（TERM-1・JUDGE-5）。
///
/// 問い返しの後は空にし、拒否の後は下書きを選択状態で残すと 2026-10-21 に確定した（D-67。JUDGE-27・TERM-22）。
/// 拒否でなければ空にする（問い返しは拒否に数えない）。問い返しの後に空になることは、試験 `clarify_draft` と
/// ブラウザ試験 `clarify-draft` が固定する（TASK-284 1/2）。拒否の後の選択状態は `selects_draft` が決める（TASK-284 2/2）。
pub(crate) fn keeps_draft(rejected: bool) -> bool {
    rejected
}

/// 送信の後に、残した下書きの全体を選択状態にするか。エンジンが拒否したときだけ選ぶ（D-67。JUDGE-27・TERM-22）。
///
/// 選ぶと、続けて打った文字が下書きの後ろに連結されず、下書きを置き換える。問い返し・受け付けでは選ばない
/// （問い返しの後は空、受け付けの後も空）。接頭辞を読めずに残した下書きは拒否ではないので選ばない。
pub(crate) fn selects_draft(rejected: bool) -> bool {
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
    /// 960px 未満で見せる領域（TASK-517 2/2）。表示の状態で、履歴・保存先には書かず、Step が変わっても捨てない。
    pane: PaneTab,
    /// 変換して開いた通知（`shell-notice`。TASK-535 1/2。SHELL-20）を出すか。表示の状態で、`ops`・履歴・保存先には書かない。
    /// 実行環境の差し替え（`install_recorded`）で一度消し、変換して開いたときだけ立て直す。
    version_notice: bool,
    /// Chrome 以外のブラウザへの案内（SHELL-30・D-74。TASK-544）を出すか。表示の状態で、履歴・保存先（履歴の鍵）には書かない。
    /// 実行環境の差し替えでは消さない（ブラウザは変わらないため）。閉じた記録は `browser_store` に書く。
    browser_notice: bool,
    /// 閉じた記録の書き先（`browser_notice::BROWSER_NOTICE_KEY`）。未設定（native の試験など）なら書かない。
    browser_store: Option<Box<dyn SyncKv>>,
    /// 開かない履歴の理由の通知（`shell-notice`。TASK-176 2/2。VER-8・SHELL-16）。表示の状態で、`ops`・履歴・保存先には書かず、控えのバイト列も持たない。
    /// 読み込みで開かなかったとき、または起動時の復元が版の判定の段で開かなかったときに立つ。開ける履歴に替えると消える。
    refused_notice: Option<RefusedNotice>,
    /// チェックポイントの復帰ボタンのページ（新しい側が 0。FEND-22）。表示の状態で、履歴・保存先には書かない。ページ数を超える値は提示の導出で丸める。
    checkpoint_page: usize,
    /// 直近の `update` で変わった field。
    dirty: Vec<&'static str>,
    /// 次の進行の宛先の通知先。未設定（native の試験など）なら通知しない。
    tick_request: Option<TickRequest>,
    /// 履歴の差し替えのとき、これまでの進行のタイマーを無効にさせる通知先（表示側が登録する）。
    tick_reset: Option<Box<dyn Fn()>>,
    /// 下書きの書き戻しの通知先。未設定なら通知しない。
    draft_sync_request: Option<DraftSyncRequest>,
    /// 拒否の後に下書きの全体を選択状態にする要求の通知先。未設定なら通知しない（D-67）。
    draft_select_request: Option<DraftSyncRequest>,
    /// 拒否で残した下書きが、まだ選択されるべき状態にある印。下書きが変わる・次の送信が来ると下りる。
    /// 打鍵がタイマーより先に届いたとき、glue が打った文字を消す全選択をしないための印である。
    draft_select_pending: bool,
    /// キーで選んだタブへフォーカスを移す要求の通知先。未設定なら通知しない（`a11y::tab_key_target` の契約）。
    pane_focus_request: Option<DraftSyncRequest>,
    /// 履歴が変わったときの通知先（変更の後の遅延保存。TASK-318）。未設定なら通知しない。
    save_request: Option<DraftSyncRequest>,
    /// 即時の保存が済んだとき、予約済みの遅延保存を無効にさせる通知先（TASK-318）。未設定なら通知しない。
    save_cancel: Option<DraftSyncRequest>,
    /// この `update` で履歴（op の列・下書きの記録）が変わった印。`update` の末尾で通知して下ろす。
    history_changed: bool,
    /// 変更のたびの保存の口。未設定（native の試験など）なら保存を試さない。
    history_saver: Option<HistorySaver>,
    /// 保存済みの履歴を読む口（「この入力を残さない」の読み直し。SAFE-28）。未設定なら保存済みの履歴が無いものとして読み直す。
    saved_reader: Option<SavedReader>,
    /// 保存先の取り直しを頼む口。未設定ならその場で読み直す。
    saved_refresh: Option<SavedRefresh>,
    /// 取り直しを頼んで、完了を待っている印。完了（`SecretDiscardRead`）でだけ読み直しを行う。
    discard_pending: bool,
    /// 取り直しの要求の世代。「このまま保存する」・履歴の差し替えで進め、古い要求の完了を捨てる（SAFE-28）。
    discard_gen: u64,
    /// 「このまま保存する」で確認済みにした検出（形と指紋だけ。値は持たない）。このページの実行中だけ持ち、保存先にも履歴にも書かない。
    /// 起動時に保存済みの値から種を入れない（旧いエンジンが書いた値・読み込んだ履歴を黙って確認済みにしないため）。差し替え・読み直しで空にする。
    secret_acknowledged: Vec<Finding>,
    /// 非同期の保存の失敗を読む口（SAVE-11）。未設定なら同期の保存の結果だけで表示を決める。
    save_failure_probe: Option<SaveFailureProbe>,
    /// 古いタブかを読む口（SAVE-13）。未設定なら通知は出ない。
    stale_probe: Option<StaleProbe>,
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
    /// 表示だけで開いた履歴のバイト列の控え（VER-8・TASK-176 1/2）。書き出しが読み込んだバイト列そのものを返すための値で、
    /// 保存先へは書かず、ログ・結果にも出さない（履歴は受講者のデータである）。`model.view_only` と同時に入り、同時に消える。
    view_only_bytes: Option<Vec<u8>>,
    /// 書き出しの URL を作る口（TASK-531 1/2）。未設定ならリンクを出さない（fail-closed）。
    export_url_maker: Option<ExportUrlMaker>,
    /// 書き出しの結果（リンクか文。`export_result` の元）。表示の状態で、履歴・保存先には書かない。履歴が変わる・差し替わると外す。
    export_result: Option<ExportShown>,
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
                left_pane: LeftPane::default(),
                judged_texts: BTreeMap::new(),
                view_choice: None,
                save_failure: None,
                export_hint: None,
                secret_warning: Vec::new(),
                stale_tab: false,
                reduced_motion: false,
                view_only: None,
            },
            registry: standard_registry(),
            input: HelloView::new(),
            pane: PaneTab::Ops,
            version_notice: false,
            browser_notice: false,
            browser_store: None,
            refused_notice: None,
            checkpoint_page: 0,
            dirty: Vec::new(),
            tick_request: None,
            tick_reset: None,
            draft_sync_request: None,
            draft_select_request: None,
            draft_select_pending: false,
            pane_focus_request: None,
            save_request: None,
            save_cancel: None,
            history_changed: false,
            history_saver: None,
            saved_reader: None,
            saved_refresh: None,
            discard_pending: false,
            discard_gen: 0,
            secret_acknowledged: Vec::new(),
            save_failure_probe: None,
            stale_probe: None,
            sync_save_failure: None,
            save_epoch: 0,
            save_due: false,
            fresh_start: false,
            first_save_hinted: false,
            command_prefix: None,
            initial: None,
            draft_recorded: String::new(),
            save_blocked: false,
            view_only_bytes: None,
            export_url_maker: None,
            export_result: None,
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

    /// 直接リンクの開始点の指定（`?start=`）を断ったホストを作る（TASK-109 1/2。DLV-9・SAFE-33）。
    ///
    /// [`EngineHost::refused`] と同じく session なしで、理由コード（固定の ASCII。入力値は持たない）だけを入れる。
    /// `history_key` は `None` になり、保存先を開かず何も書かない（保存済みの履歴を壊さない）。
    pub fn refused_start(refusal: crate::start_select::StartRefusal) -> Self {
        let mut host = Self::new();
        host.ir_refusal = Some(refusal.reason_code());
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
        let start = RecordedSession::first_start(session.ir());
        Self::with_session_at(session, registry, start)
    }

    /// [`EngineHost::with_session`] の開始点を、教材の先頭でなく呼び出し側が選んだ ID にしたもの（直接リンク。TASK-109 1/2）。
    ///
    /// `start` は記録の開始点と、保存先の鍵・起動の分岐が使う開始点になる。選ぶ規則は `start_select::select_start`。
    ///
    /// # Errors
    ///
    /// [`EngineHost::with_session`] と同じ。
    pub fn with_session_at(
        session: Session,
        registry: PartRegistry<HostModel>,
        start: String,
    ) -> Result<Self, HostError> {
        if let Some(f) = registry.fields().find(|f| {
            HELLO_FIELDS.contains(&f.name)
                || f.name == PANE_TABS_FIELD.name
                || f.name == SHELL_NOTICES_FIELD.name
                || f.name == NEXT_CHAPTER_FIELD.name
                || f.name == CHECKPOINT_OPS_FIELD.name
                || f.name == TOP_PLACE_FIELD.name
                || f.name == STEP_BODY_FIELD.name
                || f.name == EXPORT_RESULT_FIELD.name
        }) {
            return Err(HostError::FieldCollision(f.name));
        }
        let mut input = HelloView::new();
        input.set_command_table(candidates::command_table(session.ir()));
        let initial = Some(session.state().clone());
        let d = derive(&session);
        input.set_input_field(d.input_field);
        Ok(Self {
            model: HostModel {
                recorded: Some(RecordedSession::new(session, start)),
                projection: d.projection,
                world: d.world,
                world_consumed: d.world_consumed,
                left_pane: d.left_pane,
                judged_texts: d.judged_texts,
                view_choice: None,
                save_failure: None,
                export_hint: None,
                secret_warning: Vec::new(),
                stale_tab: false,
                reduced_motion: false,
                view_only: None,
            },
            registry,
            input,
            pane: PaneTab::Ops,
            version_notice: false,
            browser_notice: false,
            browser_store: None,
            refused_notice: None,
            checkpoint_page: 0,
            dirty: Vec::new(),
            tick_request: None,
            tick_reset: None,
            draft_sync_request: None,
            draft_select_request: None,
            draft_select_pending: false,
            pane_focus_request: None,
            save_request: None,
            save_cancel: None,
            history_changed: false,
            history_saver: None,
            saved_reader: None,
            saved_refresh: None,
            discard_pending: false,
            discard_gen: 0,
            secret_acknowledged: Vec::new(),
            save_failure_probe: None,
            stale_probe: None,
            sync_save_failure: None,
            save_epoch: 0,
            save_due: false,
            fresh_start: false,
            first_save_hinted: false,
            command_prefix: d.command_prefix,
            initial,
            draft_recorded: String::new(),
            save_blocked: false,
            view_only_bytes: None,
            export_url_maker: None,
            export_result: None,
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

    /// `persist::mount_with_history` が mount するホスト（配信する設定）。
    ///
    /// `ir` は `static/app.js` が固定の `./ir/main.json` から取得して渡したバイト列で、同梱の印（meta）が無い配信物では `None`
    /// （[`EngineHost::new`] と同じ。IR を持たない）。`Some` なら `start_select::open_bundled_ir` で開き、読めない・開始点を選べない
    /// ときは session なしの断りのホスト（`refused`・`refused_start`）に倒す（fail-closed。VER-13・DLV-9）。
    ///
    /// 試験の設定（`test-hooks`）では代わりに [`EngineHost::for_mount_with`] が、試験データの教材（既定は
    /// `min-3step.json`）と開始直後の状態を埋め込んで読み込み、3 つの表示が動く状態から始める。
    /// 読み込みに失敗したら session なしに倒す（ブラウザ試験が `irLoaded: true` を必須にするため、失敗は試験で検出される）。
    #[cfg(not(feature = "test-hooks"))]
    pub(crate) fn for_mount(ir: Option<&[u8]>, start: Option<&str>) -> Self {
        use crate::start_select::{open_bundled_ir, BundledIrRefusal};
        let Some(bytes) = ir else {
            return Self::new();
        };
        match open_bundled_ir(bytes, start) {
            Ok((session, start_id)) => {
                Self::with_session_at(session, standard_registry(), start_id)
                    .unwrap_or_else(|_| Self::new())
            }
            Err(BundledIrRefusal::Load(err)) => Self::refused(&err),
            Err(BundledIrRefusal::Start(refusal)) => Self::refused_start(refusal),
        }
    }

    /// 起動の前に選んだ教材で mount するホスト（試験の設定だけ。TASK-319 3/3）。
    ///
    /// `persist::mount_with_history` が URL のクエリから選んだ教材を渡す。読み込みに失敗したら session なしに倒す。
    #[cfg(feature = "test-hooks")]
    pub(crate) fn for_mount_with(
        fixture: crate::test_fixture::MountFixture,
        start: Option<&str>,
    ) -> Self {
        if let Some(err) = crate::test_fixture::mount_refusal(fixture) {
            return Self::refused(&err);
        }
        match crate::test_fixture::mount_session_at(fixture, start) {
            Ok(Some((session, start_id))) => {
                if let Ok(host) = Self::with_session_at(session, standard_registry(), start_id) {
                    return host;
                }
            }
            // 指定された開始点を選べない。session を作らず、理由だけを出して開かない（fail-closed）。
            Err(refusal) => return Self::refused_start(refusal),
            Ok(None) => {}
        }
        Self::new()
    }

    /// 推定した受講者の OS を入力欄の表示に渡す（TASK-285 1/2。JUDGE-28）。
    ///
    /// `persist::mount_with_history` が mount の前に 1 度だけ呼ぶ。表示だけの値で、履歴・実行環境には入れない。
    pub fn set_learner_os(&mut self, os: LearnerOs) {
        self.input.set_learner_os(os);
    }

    /// 動きの抑制の設定（`prefers-reduced-motion: reduce`）を渡す（TASK-372 1/2。A11Y-11）。
    ///
    /// `persist::mount_with_history` が mount の前に 1 度だけ呼ぶ。真のとき、右のログの段階の行を出さない。表示だけの値で、
    /// `ops`・履歴・保存先の鍵には入れない。進行のイベント列は変わらない（EVT-19）。
    pub fn set_reduced_motion(&mut self, on: bool) {
        self.model.reduced_motion = on;
    }

    /// 起動時に保存済みの履歴（あれば）を渡し、新しく始めるか復元するかを決めて実行環境を差し替える（TASK-319 1/3。SAVE-3・SAVE-28）。
    ///
    /// 分岐は core の `boot_session` に任せ、条件は複製しない。保存済みの値が無い・未操作なら、いまの初期状態のまま
    /// 世代を上げずに始める。開けたら再読み込みとして復元する（世代 +1）。開けなければ初期状態のまま始め、
    /// 保存済みの値を上書きしないよう保存を止める（fail-closed）。実行環境が無い（IR 未読込）ときは何もせず
    /// [`BootKind::NoSession`] を返す。`mount` の前に 1 度だけ呼ぶ。
    pub fn boot(&mut self, saved: Option<&[u8]>) -> BootKind {
        let kind = self.restore_from_saved(saved);
        #[cfg(feature = "test-hooks")]
        if kind != BootKind::NoSession {
            self.boot_kind = Some(kind);
        }
        kind
    }

    /// 起動の分岐の本体。[`EngineHost::boot`] と「この入力を残さない」の読み直しが共有する（SAFE-28）。
    ///
    /// 条件は複製せず `boot_session` に任せる。実行環境の差し替えで警告・確認済み・予約済みの保存の世代を落とすのは `install_recorded` である。
    /// 入力欄の下書きは、復元した値があればそれに置き換え、無ければ呼び出し側が先に空にしておく。
    fn restore_from_saved(&mut self, saved: Option<&[u8]>) -> BootKind {
        let (Some(session), Some(initial)) = (self.model.session(), self.initial.as_ref()) else {
            return BootKind::NoSession;
        };
        let ir = session.ir().clone();
        let start = self.model.recorded.as_ref().map_or_else(
            || RecordedSession::first_start(&ir),
            |r| r.start().to_owned(),
        );
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
        let mut converted = false;
        let mut view_only: Option<(LoadReason, Vec<u8>)> = None;
        let mut refused: Option<LoadReason> = None;
        let (recorded, kind) = match outcome {
            BootOutcome::Fresh(r) => (r, BootKind::Fresh),
            BootOutcome::Restored { session, notes } => {
                converted = notes.is_some();
                (session, BootKind::Restored)
            }
            BootOutcome::NotOpened { session, verdict } => {
                self.save_blocked = true;
                // 表示だけの判定のときだけ、理由と控えを持つ。
                match (verdict, saved) {
                    (Verdict::ViewOnly { code }, Some(bytes)) => {
                        view_only = Some((code, bytes.to_vec()));
                    }
                    // 開かないは版の判定の段のものだけ理由を出す。壊れた履歴（他の段）の案内は TASK-533 の範囲。
                    (Verdict::Refused { code }, _)
                        if code.stage() == HistoryLoadStage::VersionJudgment =>
                    {
                        refused = Some(code);
                    }
                    _ => {}
                }
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
        self.install_recorded(recorded);
        // 版の違いを変換して復元したときだけ通知を出す（配信する設定で変換して開く経路はここだけ。SHELL-20）。
        self.set_version_notice(converted);
        if let Some((code, bytes)) = view_only {
            self.enter_view_only(code, bytes);
        }
        // 起動時に開けなかった通知は閉じられない（保存が止まったままであることの手がかりを隠さない）。
        self.set_refused_notice(refused.map(|code| RefusedNotice {
            code,
            dismissable: false,
        }));
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
        // 表示だけの間は記録を増やさない（二重の防御。関門が下書きの変更を先に捨てる）。
        if self.model.view_only.is_some() || self.input.draft() == self.draft_recorded {
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
        // 表示だけの間は保存しない。`save_blocked` は消えない印なので、読み込みで入ったモードには使わない。
        if self.save_blocked || self.model.view_only.is_some() {
            return None;
        }
        let envelope = self.model.recorded.as_ref()?.envelope_for_save()?;
        let bytes = export_bytes(&envelope).ok()?;
        // 読み込めない包みは保存しない。最後に読み込めた保存を残す（`history_limit` の規則 2）。
        history_limit::check_save(bytes.len(), envelope.ops.len()).ok()?;
        String::from_utf8(bytes).ok()
    }

    /// 受講中の履歴を書き出す唯一の入口（SAFE-27・SAFE-28・D-44。TASK-164 2/3）。
    ///
    /// 呼び出し元は書き出しの画面（TASK-531）と試験用の状態 JSON。リンクを描く前にここを呼び、`Held` なら出さない。
    /// 判定の順は、(1) 書き出す履歴が無ければ `Empty`、(2) 警告の間は `Held`、(3) 遅延保存の検出が届く前の下書きだけの値も
    /// 自分で走査して `Held`（fail-closed）、(4) それ以外は `Ready`。UTF-8 として読めないときも `Empty` に倒す。
    /// 読み取りだけで、警告・dirty は変えない（警告の表示は保存の経路が持つ）。
    ///
    /// `save_blocked` と `history_limit::check_save` は見ない。書き出しは保存先を上書きせず、保存できない場面ほど要るため（SAVE-29）。
    /// 読み込みの上限を超える書き出しの扱いは TASK-531 に残す。「このまま保存する」で確認済みにした値は、`secret_gate` が除く（TASK-164 3/3）。
    pub fn export_live(&self) -> ExportGate {
        // 表示だけの間は、読み込んだバイト列そのものを返す（作り直さない。SAVE-25）。秘密情報の検査は通常の書き出しと同じく通す。
        if let Some(bytes) = self.view_only_bytes.as_ref() {
            let Ok(text) = std::str::from_utf8(bytes) else {
                return ExportGate::Empty;
            };
            let found = self.secret_gate(text);
            return if found.is_empty() {
                ExportGate::Ready(bytes.clone())
            } else {
                ExportGate::Held(found)
            };
        }
        let Some(envelope) = self
            .model
            .recorded
            .as_ref()
            .and_then(|r| r.envelope_for_save())
        else {
            return ExportGate::Empty;
        };
        let Ok(bytes) = export_bytes(&envelope) else {
            return ExportGate::Empty;
        };
        if !self.model.secret_warning.is_empty() {
            return ExportGate::Held(self.model.secret_warning.clone());
        }
        let Ok(text) = std::str::from_utf8(&bytes) else {
            return ExportGate::Empty;
        };
        let found = self.secret_gate(text);
        if found.is_empty() {
            ExportGate::Ready(bytes)
        } else {
            ExportGate::Held(found)
        }
    }

    /// 保存先の鍵。教材の `pack.id` と、開いている開始点（直接リンクで選んだもの。指定なしは先頭）から求める（SAVE-14。D-68）。
    /// IR が無い・開始点が無いときは `None`。指定なしの鍵は `HistoryKey::from_ir` と同じ文字列になる。
    pub fn history_key(&self, scope: DeliveryScope) -> Option<HistoryKey> {
        let recorded = self.model.recorded.as_ref()?;
        let ir = recorded.session().ir();
        ir.starts.first()?;
        Some(HistoryKey::for_pack(scope, &ir.pack.id, recorded.start()))
    }

    /// 起動の前に、保存先の読み込みに失敗したことを伝える。保存済みの値を読めなかったまま保存すると失うため、保存を止める。
    pub(crate) fn block_saving(&mut self) {
        self.save_blocked = true;
    }

    /// 次に届けるべき進行の宛先。処理中でなければ・未読込なら `None`。
    pub fn next_tick(&self) -> Option<TickAddr> {
        if self.model.view_only.is_some() {
            return None;
        }
        self.model.session().and_then(next_tick)
    }

    /// 書き出しの URL を作る口を結ぶ（mount の前。TASK-531 1/2）。
    pub fn set_export_url_maker(&mut self, f: ExportUrlMaker) {
        self.export_url_maker = Some(f);
    }

    /// 書き出しの結果を外す（URL は drop で破棄する）。出ていたときだけ field を dirty に積む。
    fn clear_export_result(&mut self) {
        if self.export_result.take().is_some() && !self.dirty.contains(&EXPORT_RESULT_FIELD.name) {
            self.dirty.push(EXPORT_RESULT_FIELD.name);
        }
    }

    /// 「履歴を書き出す」の押下。前の結果を外してから `export_live` の関門を通し、出せるときだけリンクを作る。
    /// 秘密情報の警告の間（`Held`）・履歴が無い（`Empty`）・URL を作る口が無い／作れないときは、リンクを出さず固定の文だけを出す。
    fn export_press(&mut self) {
        self.clear_export_result();
        let shown = match self.export_live() {
            ExportGate::Ready(bytes) => {
                let pack = self
                    .model
                    .session()
                    .map(|s| s.ir().pack.id.clone())
                    .unwrap_or_default();
                match self.export_url_maker.as_ref().and_then(|f| f(&bytes)) {
                    Some(link) => ExportShown::Link {
                        file_name: crate::history_export::file_name(&pack),
                        link,
                    },
                    None => ExportShown::Status(crate::history_export::FAILED_STATUS),
                }
            }
            other => ExportShown::Status(
                crate::history_export::status_text(&other)
                    .unwrap_or(crate::history_export::EMPTY_STATUS),
            ),
        };
        self.export_result = Some(shown);
        if !self.dirty.contains(&EXPORT_RESULT_FIELD.name) {
            self.dirty.push(EXPORT_RESULT_FIELD.name);
        }
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

    /// 拒否の後に下書きの全体を選択状態にする通知先を結ぶ（D-67）。`update` の中から呼ばれ、Runtime にも App にも触れてはならない。
    pub fn set_draft_select_request(&mut self, f: DraftSyncRequest) {
        self.draft_select_request = Some(f);
    }

    /// キーで選んだタブへフォーカスを移す通知先を結ぶ。`pane-key` の `update` の中から呼ばれ、Runtime にも App にも触れてはならない。
    pub fn set_pane_focus_request(&mut self, f: DraftSyncRequest) {
        self.pane_focus_request = Some(f);
    }

    /// 拒否で残した下書きを、まだ選択するべきか。glue が選択の直前に読む（読み取り専用）。
    #[must_use]
    pub fn draft_select_pending(&self) -> bool {
        self.draft_select_pending
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
            if !self.dirty.contains(&SHELL_NOTICES_FIELD.name) {
                self.dirty.push(SHELL_NOTICES_FIELD.name);
            }
        }
    }

    /// 古いタブかを読む口を結ぶ。`persist::boot_and_mount` が mount の前に 1 回だけ呼ぶ。結んだ時点で 1 回引く。
    pub fn set_stale_probe(&mut self, f: StaleProbe) {
        self.stale_probe = Some(f);
        self.refresh_stale();
    }

    /// probe が古いタブを示したとき、`stale_tab` を立てて dirty に積む（SAVE-13）。一度立てたら戻さず、再び積まない。
    /// `update` の末尾で、保存の試みの直後に毎回呼ぶ（その `update` の保存で追い越しが分かった場合に同じ描画で出す）。
    fn refresh_stale(&mut self) {
        if self.model.stale_tab {
            return;
        }
        if self.stale_probe.as_ref().is_some_and(|probe| probe()) {
            self.model.stale_tab = true;
            self.dirty.push(STALE_NOTICE_FIELD);
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
        let held = self.secret_gate(&value);
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

    /// 警告の形を置き換え、変わったときだけ dirty に積む（文の欄と、選択のボタンの一覧）。
    fn set_secret_warning(&mut self, shapes: Vec<SecretShape>) {
        if self.model.secret_warning != shapes {
            let toggled = self.model.secret_warning.is_empty() != shapes.is_empty();
            self.model.secret_warning = shapes;
            self.dirty.push(SECRET_NOTICE_FIELD);
            // ボタンは警告の有無だけで出入りする。形が変わるだけなら一覧は変わらない。
            if toggled {
                self.dirty.push(SECRET_CHOICE_FIELD);
            }
            // 警告が解けたら、警告の間に出した書き出しの文（`HELD_STATUS`）は古いので外す。
            if toggled && self.model.secret_warning.is_empty() {
                self.clear_export_result();
            }
        }
    }

    /// 保存・書き出しの直前の判定の唯一の入口（SAFE-27・SAFE-28）。確認済みの検出を除いた形を、固定の順序で返す。
    ///
    /// 呼び出し元は即時の保存・書き出し（このファイル）と、`glue::secret_gate` 経由の遅延保存・離れる直前の保存（`persist`）。
    /// 値は返さず、保存もしない。
    pub(crate) fn secret_gate(&self, text: &str) -> Vec<SecretShape> {
        shapes_of(&unacknowledged(text, &self.secret_acknowledged))
    }

    /// 保存済みの履歴を読む口を結ぶ。`persist::boot_and_mount` が mount の前に 1 回だけ呼ぶ（「この入力を残さない」の読み直し用）。
    pub fn set_saved_reader(&mut self, f: SavedReader) {
        self.saved_reader = Some(f);
    }

    /// 保存先の取り直しを頼む口を結ぶ。`persist::boot_and_mount` が mount の前に 1 回だけ呼ぶ。
    pub fn set_saved_refresh(&mut self, f: SavedRefresh) {
        self.saved_refresh = Some(f);
    }

    /// 「このまま保存する」（SAFE-28）。警告の対象の値を確認済みにして、今の履歴を保存する。
    ///
    /// 警告が無いとき（古いクリック）は何もしない。警告に出した形と、今の履歴の未確認の検出の形が違うとき（警告の後に
    /// 別の形の値が入った）は、見ていない値を確認済みにせず、警告を今の形へ置き換えて終える。
    /// 同じ形の別の値は区別できない（指紋を action に載せないため。`docs/testing.md` 5.183 の限界）。
    fn secret_keep(&mut self) {
        if self.model.secret_warning.is_empty() {
            return;
        }
        // 取り直し待ちの「残さない」は、選び直した時点で取り下げる（古い完了が新しい警告を破棄しないため）。
        self.invalidate_discard();
        // 確認対象は保存の可否と独立に包みから取る（上限超過でも確認と書き出しの解除は行う）。
        let Some(envelope) = self
            .model
            .recorded
            .as_ref()
            .and_then(|r| r.envelope_for_save())
        else {
            return;
        };
        let Ok(bytes) = export_bytes(&envelope) else {
            return;
        };
        let Ok(value) = String::from_utf8(bytes) else {
            return;
        };
        let pending = unacknowledged(&value, &self.secret_acknowledged);
        let shapes = shapes_of(&pending);
        if !shapes.is_empty() && shapes != self.model.secret_warning {
            self.set_secret_warning(shapes);
            return;
        }
        for f in pending {
            if !self.secret_acknowledged.contains(&f) {
                self.secret_acknowledged.push(f);
            }
        }
        // 予約済みの遅延保存は、この即時の保存が書くので不要になる。`save_due` には依存しない（下書きだけの検出も保存する）。
        self.history_changed = false;
        if let Some(f) = self.save_cancel.as_ref() {
            f();
        }
        self.save_after_change();
        // 保存できない場合（上限超過など）も、確認済みになった分だけ警告と書き出しの保留を解く。保存できた場合は同じ結果で変わらない。
        let held = self.secret_gate(&value);
        self.set_secret_warning(held);
    }

    /// 取り直しの要求を取り下げる。世代を進めるので、届く前の完了は捨てられる。
    fn invalidate_discard(&mut self) {
        self.discard_pending = false;
        self.discard_gen = self.discard_gen.wrapping_add(1);
    }

    /// 「この入力を残さない」（SAFE-28）。保存せずに、保存先の内容から起動の分岐を読み直す。
    ///
    /// ページの再読み込みではなくホストの中で行う（ブラウザが `<textarea>` の値を復元しうるため）。実行環境・入力欄の下書き・送信済みの一覧・
    /// 警告・確認済みを作り直し、`<textarea>` へ書き戻す。保存先へは何も書かない。読めなかったときは新しく始めて保存を止める（fail-closed）。
    ///
    /// 取り直しの口があるときは、保存先の実データの取得が終わってから読み直す（IndexedDB の写しは他のタブの保存や自タブの
    /// 失敗した書き込みを反映しないため。SAFE-28）。頼んだ後の連打は取り直しを重ねない。
    fn secret_discard(&mut self) {
        if self.model.secret_warning.is_empty() || self.model.recorded.is_none() {
            return;
        }
        if let Some(refresh) = self.saved_refresh.as_ref() {
            if !self.discard_pending {
                self.discard_pending = true;
                refresh(self.discard_gen);
            }
            return;
        }
        self.finish_discard(true);
    }

    /// 取り直しの完了を受けて読み直しを行う。頼んでいない完了・警告が消えた後の完了は捨てる。
    fn secret_discard_read(&mut self, ok: bool, generation: u64) {
        if !self.discard_pending || generation != self.discard_gen {
            return;
        }
        self.discard_pending = false;
        if self.model.secret_warning.is_empty() || self.model.recorded.is_none() {
            return;
        }
        self.finish_discard(ok);
    }

    /// 読み直し本体。`fetched` が偽（取り直せなかった）なら保存先を読まずに新しく始めて保存を止める。
    fn finish_discard(&mut self, fetched: bool) {
        let saved = match self.saved_reader.as_ref() {
            None => None,
            Some(_) if !fetched => {
                self.save_blocked = true;
                None
            }
            Some(read) => match read() {
                Ok(v) => v,
                Err(_) => {
                    self.save_blocked = true;
                    None
                }
            },
        };
        self.input.reset_for_reload();
        self.secret_acknowledged.clear();
        self.restore_from_saved(saved.as_deref().map(str::as_bytes));
        // 差し替えが積んだ dirty（部品の全 field）に、入力欄の field を足す。
        for f in HELLO_FIELDS {
            if !self.dirty.contains(&f) {
                self.dirty.push(f);
            }
        }
        // 保存しない。予約済みの遅延保存と、この `update` の保存の印を取り下げる。
        self.save_due = false;
        self.history_changed = false;
        if let Some(f) = self.save_cancel.as_ref() {
            f();
        }
        if let Some(f) = self.draft_sync_request.as_ref() {
            f();
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

    /// 起動の種類（`bootKind`）だけを返す。未定なら空文字列（読み取りのみ。TASK-156）。
    ///
    /// 状態の全体の直列化を避けて、2 万件の履歴を開く時間の計測を歪めないための口。
    #[cfg(feature = "test-hooks")]
    pub fn boot_kind_str(&self) -> &'static str {
        self.boot_kind.map(BootKind::as_str).unwrap_or("")
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
        let gate = self.export_live();
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
                        .filter(|l| crate::motion::shows_line(self.model.reduced_motion, l))
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
            // 確認済みの件数だけ（形・指紋・値は入れない。SAFE-28）。
            "secretAcknowledged": self.secret_acknowledged.len(),
            // 書き出しの関門の判定と長さ（中身は入れない。SAFE-28）。
            "exportGate": match &gate {
                ExportGate::Ready(_) => "ready",
                ExportGate::Held(_) => "held",
                ExportGate::Empty => "empty",
            },
            "exportBytes": match &gate {
                ExportGate::Ready(b) => Some(b.len()),
                _ => None,
            },
            "irRefusal": self.ir_refusal,
            "bootKind": self.boot_kind.map(BootKind::as_str),
            "lastImport": self.last_import_json(),
            "viewOnly": self.model.view_only.map(LoadReason::as_str),
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
            // 動きの抑制の設定（TASK-372 1/2。表示だけの値）。
            "reducedMotion": self.model.reduced_motion,
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
    /// 開かない（表示だけ・開かない）ときも、状態・導出した値には触れない。開かないときは理由の通知（`shell_notices`）だけを dirty に積む。
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
                // 変換して開いたときだけ通知を出す。開かなかったときは触れない（SAVE-19）。
                self.set_version_notice(outcome == ImportOutcome::Converted);
                outcome
            }
            Err(Verdict::ViewOnly { code }) => {
                // 読み込みの通知は直近の結果を表す。表示だけになるときは前の開かない通知を消す。
                self.set_refused_notice(None);
                self.enter_view_only(code, bytes.to_vec());
                self.closed(ImportOutcome::ViewOnly(code))
            }
            Err(Verdict::Refused { code }) => {
                // 状態・ログ・下書き・保存先には触れず、理由だけを通知する。
                self.set_refused_notice(Some(RefusedNotice {
                    code,
                    // 保存が止まっている間（起動時の拒否など）は、後続の拒否でも閉じられない通知のままにする。
                    dismissable: !self.save_blocked,
                }));
                self.closed(ImportOutcome::Refused(code))
            }
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
        self.model.left_pane = d.left_pane;
        // 拒否の文面の控えはイベントの位置がキーのため、差し替え前の控えを残さず新しいログから作り直す。
        self.model.judged_texts = d.judged_texts;
        self.model.view_choice = None;
        // 差し替えた履歴では、前の案内も初回の印も持ち越さない。
        self.model.export_hint = None;
        self.model.secret_warning = Vec::new();
        self.secret_acknowledged.clear();
        self.invalidate_discard();
        // 差し替え前に予約された検出・保存結果が、差し替え後の警告・失敗を書き換えないよう世代を進める。
        self.save_epoch = self.save_epoch.wrapping_add(1);
        self.fresh_start = false;
        self.command_prefix = d.command_prefix;
        self.dirty = self.registry.fields().map(|f| f.name).collect();
        // 変換して開いた通知は前の実行環境のものなので、持ち越さない（立て直すのは呼び出し側）。dirty の作り直しの後に消す。
        self.version_notice = false;
        self.refused_notice = None;
        // 書き出しのリンクは前の履歴のものなので外す（`dirty` の作り直しの後に積む）。
        self.clear_export_result();
        // 表示だけのモードも前の履歴のものなので、開ける履歴に替えるときに出る（`dirty` は全 field で作り直し済み）。
        self.model.view_only = None;
        self.view_only_bytes = None;
        // `shell_notices` は registry の外にあり、dirty の作り直しに入らない。前の通知を DOM から消すため明示して積む（FEND-15）。
        if !self.dirty.contains(&SHELL_NOTICES_FIELD.name) {
            self.dirty.push(SHELL_NOTICES_FIELD.name);
        }
        // 「次の章へ進む」も registry の外にある。前の教材の項目を DOM から消すため明示して積む。
        if !self.dirty.contains(&NEXT_CHAPTER_FIELD.name) {
            self.dirty.push(NEXT_CHAPTER_FIELD.name);
        }
        // 上部のチェックポイントの操作も registry の外にある。前の教材の項目を DOM から消すため明示して積む（TASK-529）。
        if !self.dirty.contains(&CHECKPOINT_OPS_FIELD.name) {
            self.dirty.push(CHECKPOINT_OPS_FIELD.name);
        }
        // 上部の教材名・位置も registry の外にある。教材・復元した Step が変わるので明示して積む（TASK-524）。
        if !self.dirty.contains(&TOP_PLACE_FIELD.name) {
            self.dirty.push(TOP_PLACE_FIELD.name);
        }
        // Step の題・do・本文も registry の外にある。前の Step の文字を DOM から消すため明示して積む（TASK-525）。
        if !self.dirty.contains(&STEP_BODY_FIELD.name) {
            self.dirty.push(STEP_BODY_FIELD.name);
        }
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
        if !view_select::selectable(self.model.left_pane.view_tabs, &declared).contains(&kind) {
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

    /// 表示だけのモードに入る（VER-8・TASK-176 1/2）。実行環境は差し替えず、理由と読み込んだバイト列の控えだけを持つ。
    ///
    /// 以後の操作の関門は `update`、保存の停止は `history_to_save`、書き出しは `export_live` が見る。
    /// 予約済みの進行のタイマーは無効にし、宛先も取り下げる。通知欄の field を dirty に積む。
    fn enter_view_only(&mut self, code: LoadReason, bytes: Vec<u8>) {
        // 「次の章へ進む」が出ていたかは、表示だけに入る前に調べる。
        let offered = self.next_chapter_offer().is_some();
        let ops_shown = self.checkpoint_offer() != CheckpointOffer::default();
        self.model.view_only = Some(code);
        self.view_only_bytes = Some(bytes);
        self.clear_export_result();
        // 保持中の警告・破棄要求・予約済みの検出と保存は、表示だけの間に状態・ログを差し替えさせない（`install_recorded` と同じ世代の進め方）。
        self.set_secret_warning(Vec::new());
        self.invalidate_discard();
        self.save_epoch = self.save_epoch.wrapping_add(1);
        if !self.dirty.contains(&VIEW_ONLY_NOTICE_FIELD) {
            self.dirty.push(VIEW_ONLY_NOTICE_FIELD);
        }
        // 「次の章へ進む」は表示だけの間は出さない。`update` を経由しない公開 API からの切り替えでも、出ていた項目の削除を通知する（FEND-3）。出ていなかったときは変えない（SAVE-19）。
        if offered && !self.dirty.contains(&NEXT_CHAPTER_FIELD.name) {
            self.dirty.push(NEXT_CHAPTER_FIELD.name);
        }
        // チェックポイントの操作も表示だけの間は出さない。出ていたときだけ削除を通知する。
        if ops_shown && !self.dirty.contains(&CHECKPOINT_OPS_FIELD.name) {
            self.dirty.push(CHECKPOINT_OPS_FIELD.name);
        }
        if let Some(reset) = self.tick_reset.as_ref() {
            reset();
        }
        if let Some(notify) = self.tick_request.as_ref() {
            notify(None);
        }
    }

    /// 上部に出す教材名・教材の ID・位置（章の ID と M / K）。表示だけのモードでも、復元した Step の位置を出す。
    /// 位置は導けないとき `None`（出さない）。session が無いときは全体が `None`。
    fn top_place(&self) -> Option<TopPlace> {
        let s = self.model.session()?;
        let ir = s.ir();
        let pos = s
            .state()
            .run
            .current
            .as_ref()
            .and_then(|uid| crate::top_position::top_position(ir, &uid.0));
        Some((ir.pack.title.clone(), ir.pack.id.clone(), pos))
    }

    /// [`Self::top_place`] を描く keyed list。session が無いときは項目 0 件。
    fn top_place_node(&self) -> Node {
        match self.top_place() {
            Some((title, id, pos)) => crate::shell_frame::top_place_list(
                Some(title.as_str()),
                &id,
                pos.as_ref().map(|(c, p)| (c.as_str(), *p)),
            ),
            None => crate::shell_frame::top_place_list(None, "", None),
        }
    }

    /// 題・`do`・本文を描く keyed list。session が無いときは項目 0 件。表示だけのモードでも復元した Step を出す。
    fn step_text_node(&self) -> Node {
        let t = self
            .model
            .session()
            .and_then(|s| crate::step_body::step_text(s.ir(), s.state()));
        crate::step_body::step_text_list(t.as_ref())
    }

    /// 本文の表示を決める値（現在の uid と案内レベル）。
    fn step_text_key(&self) -> Option<(String, fandhe_workshop_contract::guidance::GuidanceLevel)> {
        self.model
            .session()
            .and_then(|s| crate::step_body::step_text_key(s.state()))
    }

    /// 今提示する「次の章へ進む」。表示だけの間と session が無いときは出さない（fail-closed。update が押下を捨てるため）。
    fn next_chapter_offer(&self) -> Option<NextChapter> {
        if self.model.view_only.is_some() {
            return None;
        }
        let s = self.model.session()?;
        next_chapter(s.ir(), s.state())
    }

    /// 今提示するチェックポイントの操作。表示だけの間と session が無いときは空（fail-closed。update が押下を捨てるため）。
    fn checkpoint_offer(&self) -> CheckpointOffer {
        if self.model.view_only.is_some() {
            return CheckpointOffer::default();
        }
        match self.model.session() {
            Some(s) => crate::checkpoint_ops::offer(s.ir(), s.state(), self.checkpoint_page),
            None => CheckpointOffer::default(),
        }
    }

    /// 変換して開いた通知の表示を替える。同じ値なら何もしない。表示の状態だけで、保存・履歴は起こさない。
    ///
    /// `install_recorded` の後（dirty が全 field で作り直された後）に呼ぶ。そのとき `shell_notices` は `install_recorded` が dirty に積み済みなので重ねて積まない。
    fn set_version_notice(&mut self, on: bool) {
        if self.version_notice != on {
            self.version_notice = on;
            if !self.dirty.contains(&SHELL_NOTICES_FIELD.name) {
                self.dirty.push(SHELL_NOTICES_FIELD.name);
            }
        }
    }

    /// ブラウザの案内の表示を替える。同じ値なら何もしない。`store` は閉じた記録の書き先（起動の配線が渡す）。
    ///
    /// 起動の配線から 1 回呼ぶ想定で、`update` の外なので変わったときは `shell_notices` を自分で積む。
    pub fn set_browser_notice(&mut self, show: bool, store: Option<Box<dyn SyncKv>>) {
        self.browser_store = store;
        if self.browser_notice != show {
            self.browser_notice = show;
            if !self.dirty.contains(&SHELL_NOTICES_FIELD.name) {
                self.dirty.push(SHELL_NOTICES_FIELD.name);
            }
        }
    }

    /// ブラウザの案内を閉じる。出ているときだけ消し、閉じた記録を書く（失敗は無視。そのページでは閉じたままにする）。
    fn dismiss_browser_notice(&mut self) {
        if !self.browser_notice {
            return;
        }
        self.browser_notice = false;
        if let Some(store) = self.browser_store.as_ref() {
            let _ = store.set(
                crate::browser_notice::BROWSER_NOTICE_KEY,
                crate::browser_notice::DISMISSED,
            );
        }
        if !self.dirty.contains(&SHELL_NOTICES_FIELD.name) {
            self.dirty.push(SHELL_NOTICES_FIELD.name);
        }
    }

    /// 開かない履歴の通知を替える。同じ値なら何もしない。表示の状態だけで、保存・履歴は起こさない。
    ///
    /// 読み込みは `install_recorded` を通らず `update` が先頭で dirty を空にするため、変えたときは `shell_notices` を自分で積む。
    fn set_refused_notice(&mut self, notice: Option<RefusedNotice>) {
        if self.refused_notice != notice {
            self.refused_notice = notice;
            if !self.dirty.contains(&SHELL_NOTICES_FIELD.name) {
                self.dirty.push(SHELL_NOTICES_FIELD.name);
            }
        }
    }

    /// 960px 未満で見せる領域を替える。同じ値なら何もしない。表示の状態だけで、保存・履歴は起こさない。
    fn select_pane(&mut self, tab: PaneTab) {
        if self.pane != tab {
            self.pane = tab;
            self.dirty.push(PANE_TABS_FIELD.name);
        }
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
        let mut select = false;
        self.draft_select_pending = false;
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
                    select = selects_draft(rejected);
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
        } else if select {
            self.draft_select_pending = true;
            if let Some(f) = self.draft_select_request.as_ref() {
                f();
            }
        }
    }
}

/// 復元で、ログ全体の拒否・問い返しの文面を core に作り直させて控える。
///
/// 各判定の時点の状態の組み直しは core の [`filled_texts_of_log_at`] が持つ。作れなかった拒否・問い返しは控えず、
/// 行を出さない（fail-closed）。
fn restore_judged_texts(texts: &mut BTreeMap<u32, String>, session: &Session) {
    // 案内レベルはログに残らないため、復元後の現在のレベルで充填する（記録時と同じ値）。
    let level = session.state().run.guidance_level;
    for (i, f) in filled_texts_of_log_at(session.ir(), session.log(), level) {
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
        // 受付は適用で現在の Step が進みうるので、適用後の状態では充填しない。core が `at.step`（判定の時点の Step）から
        // 組み直す `filled_texts_of_log` に、入力と判定の 2 件だけを渡す（実行状態の組み立ては web に持ち込まない）。
        // 拒否・問い返しは状態を変えないため、適用後の状態のままでよい。
        if matches!(judgment, Judgment::Accepted(_)) {
            if let Some((_, f)) = log
                .get(i - 1..=i)
                .map(|pair| {
                    filled_texts_of_log_at(session.ir(), pair, session.state().run.guidance_level)
                })
                .and_then(|v| v.into_iter().next())
            {
                texts.insert(key, f.message);
            }
        } else if let Ok(Some(f)) = filled_text(session.ir(), &session.state().run, input, judgment)
        {
            texts.insert(key, f.message);
        }
    }
}

/// 文面を控える判定か。拒否・問い返しと、自然文の受付の注記（応答の行。EXT-3）だけである。
fn keeps_text(j: &Judgment) -> bool {
    matches!(
        j,
        Judgment::Rejected(_) | Judgment::Clarified(_) | Judgment::Accepted(Some(_))
    )
}

/// 上部の教材名・教材の ID・位置。前後で比べて dirty を積むための値。
type TopPlace = (String, String, Option<(String, crate::shell::StepPosition)>);

impl Component for EngineHost {
    type Action = HostAction;

    fn update(&mut self, action: Self::Action) {
        self.dirty.clear();
        // 「次の章へ進む」の提示は、どの経路の更新でも前後で比べて 1 か所で dirty に積む（FEND-3）。
        let offer_before = self.next_chapter_offer();
        // 上部の教材名・位置も同じく前後で比べる（TASK-524）。
        let place_before = self.top_place();
        // チェックポイントの操作の提示も前後で比べる（TASK-529）。
        let ops_before = self.checkpoint_offer();
        // Step の題・do・本文も前後で比べる（TASK-525）。
        let step_before = self.step_text_key();
        // 表示だけの間は、閲覧に必要な action（allowlist）以外を全部捨てる（fail-closed。VER-8）。通すのは表示だけを変える action と、
        // モードから出る経路（試験の設定の読み込み・差し替え）に限る。新しい action は既定で捨てられる。
        // 下書きの変更を捨てたときは、DOM の値を戻させる。
        if self.model.view_only.is_some() {
            let allowed = match &action {
                HostAction::SelectView(_)
                | HostAction::SelectPane(_)
                | HostAction::DismissNotice(_)
                | HostAction::PaneKey(_)
                // 書き出しは読み込んだバイト列そのものを返す（状態を変えない）。読み込みは表示だけから出る経路（TASK-531 1/2）。
                | HostAction::ExportHistory
                | HostAction::ImportHistory(_)
                | HostAction::Input(HelloAction::SetOs(_)) => true,
                #[cfg(feature = "test-hooks")]
                HostAction::Input(HelloAction::KeydownProbe)
                | HostAction::OpenHistory(_)
                | HostAction::LoadFixture(_)
                | HostAction::LoadIr(_)
                | HostAction::OpeningOverride(_) => true,
                _ => false,
            };
            if !allowed {
                if matches!(
                    action,
                    HostAction::Input(HelloAction::SetDraft(_)) | HostAction::Complete
                ) {
                    if let Some(f) = self.draft_sync_request.as_ref() {
                        f();
                    }
                }
                return;
            }
        }
        match action {
            // 入力欄なしの教材（PACK-20）は、口が無いので下書き・送信・補完・OS の選択を受けても状態を変えない（fail-closed）。
            HostAction::Input(
                HelloAction::SetDraft(_) | HelloAction::Submit | HelloAction::SetOs(_),
            )
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
            HostAction::SecretKeep => self.secret_keep(),
            HostAction::SecretDiscard => self.secret_discard(),
            HostAction::SecretDiscardRead(ok, generation) => {
                self.secret_discard_read(ok, generation)
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
                let before = self.input.draft().to_owned();
                self.input.update(a);
                // 打鍵が選択のタイマーより先に届いたら、予約は取り下げる（打った文字を消す全選択をさせない）。
                if self.input.draft() != before {
                    self.draft_select_pending = false;
                }
                self.dirty.extend_from_slice(self.input.dirty_fields());
            }
            HostAction::Engine(op) => {
                self.apply_engine(op);
            }
            HostAction::SelectView(kind) => self.select_view(kind),
            // 今の提示と完全一致したときだけ、一致した開始点を `Control::Begin` で始める。提示なし・処理中・不一致は core を呼ばない（fail-closed）。
            // 表示だけの間は上の allowlist が捨てる（allowlist には足さない）。
            HostAction::NextChapter(id) => {
                let target = self
                    .model
                    .session()
                    .and_then(|s| crate::next_chapter::press_target(s.ir(), s.state(), &id));
                if let Some(start_id) = target {
                    self.apply_engine(Op::Control(Control::Begin(start_id)));
                }
            }
            // 保存は今の提示に保存のボタンがあるときだけ、復帰は今の提示の ID と完全一致したときだけ core へ送る（fail-closed）。
            // 表示だけの間は上の allowlist が捨てる（allowlist には足さない）。
            HostAction::SaveCheckpoint => {
                if self.checkpoint_offer().can_save {
                    self.apply_engine(Op::Control(Control::Save));
                }
            }
            HostAction::RestoreCheckpoint(id) => {
                if let Some(id) =
                    crate::checkpoint_ops::restore_target(&self.checkpoint_offer(), id)
                {
                    self.apply_engine(Op::Control(Control::Restore(
                        fandhe_workshop_core::input::CheckpointId(id),
                    )));
                }
            }
            HostAction::CheckpointPage(page) => {
                if let Some(page) =
                    crate::checkpoint_ops::page_target(&self.checkpoint_offer(), page)
                {
                    self.checkpoint_page = page;
                }
            }
            HostAction::SelectPane(tab) => self.select_pane(tab),
            // 描いている通知（変換して開いた）だけを閉じる。他の種類は枠の通知欄に描いていないので何もしない（fail-closed）。
            HostAction::DismissNotice(kind) => {
                match kind {
                    crate::shell::NoticeKind::VersionConverted => self.set_version_notice(false),
                    crate::shell::NoticeKind::BrowserNotice => self.dismiss_browser_notice(),
                    // 起動時の通知は閉じない（`dismissable` が偽。保存が止まっていることを隠さない）。
                    crate::shell::NoticeKind::ImportRefused
                        if self.refused_notice.is_some_and(|n| n.dismissable) =>
                    {
                        self.set_refused_notice(None);
                    }
                    _ => {}
                }
            }
            HostAction::PaneKey(key) => {
                let current = PaneTab::ALL.iter().position(|t| *t == self.pane);
                let target = current
                    .and_then(|c| {
                        a11y::tab_key_target(key, c, PaneTab::ALL.len(), KeyModifiers::default())
                    })
                    .and_then(|i| PaneTab::ALL.get(i).copied());
                if let Some(tab) = target {
                    self.select_pane(tab);
                    // 選択に合わせてフォーカスも移す（クリックでは移さない。押した要素が既にフォーカスを持つため）。
                    if let Some(f) = self.pane_focus_request.as_ref() {
                        f();
                    }
                }
            }
            #[cfg(feature = "test-hooks")]
            HostAction::Reload => self.reload_engine(),
            HostAction::ExportHistory => self.export_press(),
            HostAction::ImportHistory(text) => {
                let outcome = self.open_history(text.as_bytes());
                #[cfg(feature = "test-hooks")]
                {
                    self.last_import = Some(outcome);
                }
                #[cfg(not(feature = "test-hooks"))]
                let _ = outcome;
            }
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
                    FixtureName::ApprovalCandidates => {
                        crate::test_fixture::approval_candidates_session()
                    }
                    FixtureName::ApprovalReapprove => {
                        crate::test_fixture::approval_reapprove_session()
                    }
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
                    self.draft_select_pending = false;
                    self.extend_input_dirty();
                    if let Some(f) = self.draft_sync_request.as_ref() {
                        f();
                    }
                }
            }
        }
        // 入力欄の値が変わっていれば下書きとして記録する。どの action の後でも 1 か所で行う。
        self.record_draft_change();
        // 履歴（ログ・下書き）が変わったら、出していた書き出しのリンクは古いので外す（下書きも書き出しに入る。SAVE-18）。
        if self.save_due || self.history_changed {
            self.clear_export_result();
        }
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
        self.refresh_stale(); // 保存の試み（照合）の後に求める
        if self.next_chapter_offer() != offer_before
            && !self.dirty.contains(&NEXT_CHAPTER_FIELD.name)
        {
            self.dirty.push(NEXT_CHAPTER_FIELD.name);
        }
        if self.checkpoint_offer() != ops_before && !self.dirty.contains(&CHECKPOINT_OPS_FIELD.name)
        {
            self.dirty.push(CHECKPOINT_OPS_FIELD.name);
        }
        if self.top_place() != place_before && !self.dirty.contains(&TOP_PLACE_FIELD.name) {
            self.dirty.push(TOP_PLACE_FIELD.name);
        }
        if self.step_text_key() != step_before && !self.dirty.contains(&STEP_BODY_FIELD.name) {
            self.dirty.push(STEP_BODY_FIELD.name);
        }
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
        // 部品は種類ごとに左（成果物）・右（操作）へ振り分け、枠（`shell_frame`）の取り付け点に載せる（TASK-517 1/2）。
        // 左は `workshop-parts` に包んで置く。右は入力欄の `main` の後ろに、部品を登録順に直接並べる。
        let mut left: Vec<Node> = Vec::new();
        // Step の題・do・本文は右のエリアの先頭に置く（仮置き。最終の配置は TASK-534・TASK-548）。
        let mut right: Vec<Node> = vec![self.step_text_node(), self.input.view()];
        for (kind, node) in self.registry.views(&self.model) {
            match crate::shell_frame::pane_of(kind) {
                crate::shell::PaneMount::Left => left.push(node),
                crate::shell::PaneMount::Right => right.push(node),
            }
        }
        let left = vec![el("div", vec![("data-testid", "workshop-parts")], left)];
        // 「次の章へ進む」は右のエリアの末尾に置く（仮置き。最終の配置は TASK-534・TASK-548）。
        right.push(next_chapter_list(self.next_chapter_offer().as_ref()));
        let [tablist, panes] = crate::shell_frame::frame(left, right, self.pane);
        // 履歴の書き出し・読み込みの操作は通知欄の後ろ・tablist の前に置く（仮置き。最終の配置は TASK-534・TASK-548）。
        let shown = self.export_result.as_ref().map(|r| match r {
            ExportShown::Link { file_name, link } => ExportResult::Link {
                href: link.url.as_str(),
                file_name: file_name.as_str(),
            },
            ExportShown::Status(m) => ExportResult::Status(m),
        });
        // 上部の区画の中身は教材名・位置とチェックポイントの操作（TASK-529）。
        let top_children = vec![
            self.top_place_node(),
            checkpoint_section(&self.checkpoint_offer()),
        ];
        #[allow(unused_mut, reason = "試験の設定でだけ raw-trigger を足す")]
        // 通知欄は root の先頭に置く（仮置き。配置の最終の決定は TASK-534）。tabpanel の外なので、幅 960px 未満でも見える。
        // 上部の区画（「模擬」。TASK-523）は通知欄より前に置く。tabpanel の外なので、どのタブでも・通知が出ても見える。
        let mut children = vec![
            crate::shell_frame::top_bar(top_children),
            crate::shell_notice::notice_area(
                self.version_notice,
                self.refused_notice,
                self.model.save_failure,
                self.browser_notice,
            ),
            history_ops(shown.as_ref()),
            tablist,
            panes,
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
                    "approval-candidates" => {
                        Some(HostAction::LoadFixture(FixtureName::ApprovalCandidates))
                    }
                    "approval-reapprove" => {
                        Some(HostAction::LoadFixture(FixtureName::ApprovalReapprove))
                    }
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
        // タブのクリック（TASK-517 2/2）。payload は `PaneTab::id` の完全一致だけで、引けなければ何もしない。
        // 「次の章へ進む」（TASK-526 2/2）。空は何もしない。提示との照合は `update` が行う。
        if name == crate::next_chapter::ACTION_NEXT_CHAPTER {
            return (!payload.is_empty()).then(|| HostAction::NextChapter(payload.to_owned()));
        }
        if name == crate::checkpoint_ops::ACTION_SAVE_CHECKPOINT {
            return payload.is_empty().then_some(HostAction::SaveCheckpoint);
        }
        if name == crate::checkpoint_ops::ACTION_RESTORE_CHECKPOINT {
            return crate::checkpoint_ops::parse_restore_payload(payload)
                .map(HostAction::RestoreCheckpoint);
        }
        if name == crate::checkpoint_ops::ACTION_CHECKPOINT_PAGE {
            return crate::checkpoint_ops::parse_page_payload(payload)
                .map(HostAction::CheckpointPage);
        }
        if name == ACTION_SELECT_PANE {
            return pane_from_payload(payload).map(HostAction::SelectPane);
        }
        // tablist の上のキー。Esc は root の Esc と同じ経路（`HelloAction::Esc`）へ渡し、他は矢印・Home・End だけを受ける。
        if name == ACTION_PANE_KEY {
            if payload == crate::action::ESC_KEY_TOKEN {
                return Some(HostAction::Input(HelloAction::Esc));
            }
            return TabKey::from_key(payload).map(HostAction::PaneKey);
        }
        // 通知を閉じる（TASK-535 1/2）。枠の復号（正規形・32 バイト以下）と種類の既知の照合の両方を通ったものだけを受ける。
        // 捨てて始めるはここでは受けない（書き出し・読み込みは下で別に受ける）。
        if name == crate::shell::ACTION_DISMISS_NOTICE {
            return match crate::shell::ShellAction::decode(name, payload) {
                Some(crate::shell::ShellAction::DismissNotice(kind)) => {
                    crate::shell::NoticeKind::parse(&kind).map(HostAction::DismissNotice)
                }
                _ => None,
            };
        }
        // 履歴の書き出し・読み込み（TASK-531 1/2）。復号は枠の契約（`ShellAction::decode`）の 1 か所で、受けるのはこの 2 つだけ。
        if name == crate::shell::ACTION_EXPORT_HISTORY
            || name == crate::shell::ACTION_IMPORT_HISTORY
        {
            return match crate::shell::ShellAction::decode(name, payload) {
                Some(crate::shell::ShellAction::Export) => Some(HostAction::ExportHistory),
                Some(crate::shell::ShellAction::Import(body)) => {
                    Some(HostAction::ImportHistory(body))
                }
                _ => None,
            };
        }
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
        // 警告の選択（SAFE-28）。名前は完全一致だけで、payload は読まない。警告が無いときの扱いは `update` が決める。
        if name == crate::secret_notice::ACTION_SECRET_KEEP {
            return Some(HostAction::SecretKeep);
        }
        if name == crate::secret_notice::ACTION_SECRET_DISCARD {
            return Some(HostAction::SecretDiscard);
        }
        if name == crate::secret_notice::ACTION_SECRET_DISCARD_READ {
            // payload は `ok <世代>` か `fail <世代>`（世代は 10 進の正規形）。
            let (word, generation) = payload.split_once(' ')?;
            if generation.is_empty()
                || generation.len() > 20
                || !generation.bytes().all(|b| b.is_ascii_digit())
                || (generation.len() > 1 && generation.starts_with('0'))
            {
                return None;
            }
            let generation = generation.parse::<u64>().ok()?;
            return match word {
                "ok" => Some(HostAction::SecretDiscardRead(true, generation)),
                "fail" => Some(HostAction::SecretDiscardRead(false, generation)),
                _ => None,
            };
        }
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
