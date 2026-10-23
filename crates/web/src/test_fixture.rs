//! ブラウザ試験が mount するホストの開始状態（試験の設定 `test-hooks` だけ。TASK-347 2/2）。
//!
//! 呼び出し元は `host::EngineHost::for_mount` だけである。`lib.rs` が feature `test-hooks` の背後でだけ
//! 組み込むため、配信する設定の wasm には入らない（`tools/check-delivery.sh` が教材の文字列の不在を確かめる）。
//!
//! 開始（`Control::Begin`）はまだ core が実装していないため、契約の試料（`event-sequence.json`）が持つ
//! 開始直後の状態から始める。状態からの `Session` の再構築は、表示層で他に許されない操作（`tests/no_progress.rs`）で、
//! このファイルの 1 箇所だけを例外にしている。教材と状態は埋め込みの試験データで、外から入らない。
//! 確認が `answer` の教材への差し替え（`answer_session`）は、`load-fixture:answer` の合図で使う（TASK-445 2/2）。
//! 複数の種類のビューを宣言する教材（`views_session`）は `load-fixture:views` で使う（TASK-470 2/2）。
//! 入力方式が `ui_action` の Step を持つ教材（`ui_action_session`）は `load-fixture:ui-action` で使う（TASK-450 1/3）。
//! 履歴ファイルの読み込みの試験（TASK-330 2/2）が使う開ける包み（`loadable_envelope`）もここで作る。
//! 再読み込みの一致のブラウザ試験（TASK-319 3/3）は、起動の前に 8 Step の教材（`reload_points_session`）を選ぶ。
//! 入力欄なしの教材（`?fixture=no-input`。`pack.profile.input_field: hidden`）も起動の前に選ぶ（TASK-450 2/3）。
//! 章をまたいで 20 Step を 1 つのページで通す教材（`?fixture=ch0-3`。先頭は `starts[0]` の節点）も起動の前に選ぶ（TASK-450 3/3）。
//! 遷移の上限（SAFE-23）の試験は、巡回する IR 4 本（`cycle_session`）と巡回しない長い連鎖（`chain_session`）を `load-fixture:cycle-*`・`load-fixture:chain-*` で使う（TASK-160 1/3）。
//! 旧いエンジンが読めない未来の IR（`?fixture=future-ir-format`・`?fixture=future-ir-field`。既定の教材から作り、`pack.id` は同じ）は、
//! 起動の前に選ぶと `mount_refusal` が理由を返し、ホストは通知だけを描く（VER-13。TASK-181 2/2）。
//! 同じ教材の別の章（`pack.id` が同じで開始点だけが違う IR 2 本。`?fixture=chapter-a`・`?fixture=chapter-b`）も起動の前に選ぶ。
//! 章ごとの保存先の鍵で別の章の履歴が混ざらないことの試験が使う（SAVE-14・DLV-8。TASK-326 1/2）。
//! 章 A の `pack.version`・題名を変えた変種（`?fixture=chapter-a-next`）は、版・内容が変わっても鍵が変わらないことの試験が使う（SAVE-14。TASK-326 2/2）。
//! 20 Step の通しの教材に章の開始点 10 件を持たせた変種（`?fixture=ch0-3-links`）は、直接リンクと再読み込みの試験が使う（DLV-7。TASK-110 1/2）。
//! 20 Step の通しの ST-2.6 に「直す」の候補と表示名を宣言した変種（`?fixture=ch0-3-revise`）は、誤った候補を含むボタンが表示名で出ることの試験に使う（TERM-28。TASK-448 1/2）。
//! 確認 `answer` の選択肢から `label` を外した変種（`?fixture=ch0-3-nolabel`）は、数え方が ID の表示を検出できることの対照に使う（TERM-25。TASK-446）。
//! 各章の最後の Step を `final` にして開始点を章の先頭 4 件にした変種（`?fixture=ch0-3-chapters`）は、「次の章へ進む」で CH-0 から CH-3 を通す試験が使う（SHELL-10。TASK-526 2/2）。
//! CH-3 を最後に使う絞った IR（`?fixture=ch0-3-trial`。範囲外の章 CH-4 のノードは IR に残り、開始点は CH-0〜CH-3 の先頭 4 件だけ）と、
//! 対照の `?fixture=ch0-4-full`（同じ IR に CH-4 の開始点を足したもの）は、範囲外の章へ進まないことの試験が使う（SHELL-12・D-67。TASK-527）。
//! 選択は URL のクエリの完全一致だけで決める（`mount_fixture_from_search`）。教材の差し替え（`load-fixture:`）は
//! 起動の後に働き、保存先の鍵（`pack.id`）が mount の教材のままになるため、復元の試験には使えない。

use crate::start_select::{select_start, StartRefusal};
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::{load_ir, IrLoadError, Session, SessionState};

const IR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/engine-event-consistency/test-data/min-3step.json"
));
const SAMPLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/contract/event-sequence.json"
));

/// 再読み込みの一致の試験用の 8 Step の教材（`crates/web/tests/data/reload_points_ch2/ir.json`。`pack.id` は mount の既定と別）。
const RELOAD_POINTS_IR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/reload_points_ch2/ir.json"
));

/// 入力欄なしの 3 Step の教材（`crates/web/tests/data/input_field_hidden/ir.json`。`pack.id` は mount の既定と別）。
const NO_INPUT_IR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/input_field_hidden/ir.json"
));

/// 章をまたいで 20 Step を通す教材（`crates/web/tests/data/ch0_3/ir.json`。`pack.id` は mount の既定と別。TASK-450 3/3）。
const CH03_IR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/ch0_3/ir.json"
));

/// mount の教材の選択。既定は `min-3step`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum MountFixture {
    /// 3 Step の `min-3step`（既存のブラウザ試験の前提）。
    #[default]
    Default,
    /// 8 Step の再読み込みの地点の試験用の教材。
    ReloadPoints,
    /// 入力欄なしの 3 Step の教材（`pack.profile.input_field: hidden`。TASK-450 2/3）。
    NoInput,
    /// 章をまたいで 20 Step を通す教材（TASK-450 3/3）。
    Ch03,
    /// 同じ教材の章 A（開始点は `starts` の先頭のまま。SAVE-14。TASK-326 1/2）。
    ChapterA,
    /// 同じ教材の章 B（章 A と `pack.id` が同じで、開始点だけが違う。SAVE-14。TASK-326 1/2）。
    ChapterB,
    /// 章 A と `pack.id`・開始点が同じで、`pack.version` と題名だけが違う IR（SAVE-14。TASK-326 2/2）。
    ChapterANext,
    /// 開始点を 3 件持つ教材（直接リンク `?start=` の試験。TASK-109 1/2）。
    MultiStart,
    /// 20 Step の通しの教材に、章の開始点 10 件を持たせた教材（直接リンクと再読み込みの試験。DLV-7。TASK-110 1/2）。
    Ch03Links,
    /// 20 Step の通しの教材の確認 `answer` の選択肢から `label` を外した対照（選択肢の ID が見える文字に出ることの観察。TERM-25。TASK-446）。
    Ch03NoLabel,
    /// 20 Step の通しの教材の ST-2.6 に、「直す」の対象の候補と表示名（誤った候補つき）を宣言した教材。開始点 `s-2-6` を足す（TERM-28。TASK-448 1/2）。
    Ch03Revise,
    /// 20 Step の通しの教材の各章の最後を `final` にし、開始点を章の先頭 4 件にした教材（「次の章へ進む」の通しの試験。TASK-526 2/2）。
    Ch03Chapters,
    /// CH-3 を最後に使う絞った IR。範囲外の章 CH-4 のノードを持つが、開始点には CH-4 を含めない（SHELL-12・D-67。TASK-527）。
    Ch03Trial,
    /// [`MountFixture::Ch03Trial`] に CH-4 の開始点を足した対照（範囲外の開始点があれば出ることを示す。TASK-527）。
    Ch04Full,
    /// 既定の教材と同じ `pack.id` で、版が新しい IR（`format_version`・`pack.ir_format` が 3。VER-13。TASK-181 2/2）。
    FutureFormat,
    /// 既定の教材と同じ `pack.id` で、版は 1 のまま未知の欄を足した IR（版を上げずに欄だけが足された混在の場面。VER-13）。
    FutureField,
}

/// 未来の IR に足す未知の欄の値。画面・ログ・写しに出ないことを、ブラウザ試験がこの文字列の不在で確かめる（SAFE-11）。
pub(crate) const FUTURE_SENTINEL: &str = "FUTURE-SENTINEL-V3-X";

/// 旧いエンジンが読めない IR の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FutureIr {
    /// 版が 3 で、未知の欄を持つ（D-51: 新しい欄は版を上げて足す）。
    Format,
    /// 版は 1 のまま、未知の欄を持つ。
    Field,
}

/// 既定の教材（`min-3step`）から、旧いエンジンが読めない IR を作る。`pack.id` は既定のまま（同じ教材の新しい版が
/// 旧いエンジンに届いた場面）。新しい試験データのファイルは作らず、埋め込みの IR を書き換える。
pub(crate) fn future_ir_bytes(kind: FutureIr) -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_slice(IR).ok()?;
    let obj = value.as_object_mut()?;
    if kind == FutureIr::Format {
        obj.insert("format_version".to_owned(), 3.into());
        obj.get_mut("pack")?
            .as_object_mut()?
            .insert("ir_format".to_owned(), 3.into());
    }
    obj.insert("future_v3_x".to_owned(), FUTURE_SENTINEL.into());
    serde_json::to_vec(&value).ok()
}

/// 章の試験の教材の `pack.id`。他の試験の鍵と分ける。
pub(crate) const CHAPTER_PACK_ID: &str = "chapter-links";

/// 同じ教材の別の章（開始点の違う IR）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Chapter {
    /// 開始点は元の IR の `starts` のまま。
    A,
    /// 開始点を、別の Step の節点を指す 1 件に置き換える。
    B,
    /// 章 A と `pack.id`・`starts` が同じで、`pack.version`（主版は 0 のまま）と題名だけを変える（内容のハッシュが変わる）。
    ANext,
}

/// 8 Step の教材から、同じ教材の別の章の IR を作る。`pack.id` は章によらず同じで、`starts` と内容のハッシュが違う。
/// 新しい試験データのファイルは作らず、埋め込みの IR を書き換える（`version` を変える変種は、版・内容が変わっても
/// 鍵が変わらないことの試験が `ANext` で足した）。
pub(crate) fn chapter_ir_bytes(chapter: Chapter) -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_slice(RELOAD_POINTS_IR).ok()?;
    let obj = value.as_object_mut()?;
    let pack = obj.get_mut("pack")?.as_object_mut()?;
    pack.insert("id".to_owned(), CHAPTER_PACK_ID.into());
    pack.insert("title".to_owned(), "章のリンクの試験データ".into());
    if chapter == Chapter::ANext {
        pack.insert("version".to_owned(), "0.1.0".into());
        pack.insert("title".to_owned(), "章のリンクの試験データ（改訂）".into());
    }
    if chapter == Chapter::B {
        obj.insert(
            "starts".to_owned(),
            serde_json::json!([{ "id": "s-4", "node": "main/st-4", "effects": [] }]),
        );
    }
    serde_json::to_vec(&value).ok()
}

/// 開始点を複数持つ教材の `pack.id`。他の試験の鍵と分ける。
pub(crate) const MULTI_START_PACK_ID: &str = "start-links";

/// 開始点の複数ある教材の、開始点の ID と節点（先頭から順）。ブラウザ試験の期待値と揃える。
pub(crate) const MULTI_START_POINTS: [(&str, &str); 3] = [
    ("s-1", "main/st-1"),
    ("s-4", "main/st-4"),
    ("s-7", "main/st-7"),
];

/// 8 Step の教材から、開始点を 3 件持つ IR を作る（`pack.id` だけ別にする）。新しい試験データのファイルは作らず、埋め込みの IR を書き換える。
pub(crate) fn multi_start_ir_bytes() -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_slice(RELOAD_POINTS_IR).ok()?;
    let obj = value.as_object_mut()?;
    let pack = obj.get_mut("pack")?.as_object_mut()?;
    pack.insert("id".to_owned(), MULTI_START_PACK_ID.into());
    pack.insert("title".to_owned(), "開始点のリンクの試験データ".into());
    let starts: Vec<serde_json::Value> = MULTI_START_POINTS
        .iter()
        .map(|(id, node)| serde_json::json!({ "id": id, "node": node, "effects": [] }))
        .collect();
    obj.insert("starts".to_owned(), starts.into());
    serde_json::to_vec(&value).ok()
}

/// 章の開始点 10 件を持つ教材の `pack.id`。他の試験の鍵と分ける。
pub(crate) const CH03_LINKS_PACK_ID: &str = "ch0-3-links";

/// 章の開始点 10 件の、開始点の ID と節点（先頭から順）。ブラウザ試験の直接リンク 10 件（`delivery.mjs` の `DIRECT_LINKS`）と揃える。
pub(crate) const CH03_LINK_POINTS: [(&str, &str); 10] = [
    ("s-0-1", "main/st-0-1"),
    ("s-0-2b", "main/st-0-2b"),
    ("s-1-1", "main/st-1-1"),
    ("s-1-4", "main/st-1-4"),
    ("s-1-5", "main/st-1-5"),
    ("s-1-6", "main/st-1-6"),
    ("s-2-1", "main/st-2-1"),
    ("s-2-3", "main/st-2-3"),
    ("s-3-1", "main/st-3-1"),
    ("s-3-2", "main/st-3-2"),
];

/// 20 Step の通しの教材から、開始点を 10 件持つ IR を作る（`pack.id` だけ別にする）。試験データのファイルは増やさず、埋め込みの IR を書き換える。
/// 開始点の `effects` は空のままで、前の Step の効果は持たない（`docs/testing.md` 5.199 と同じ限界）。
pub(crate) fn ch03_links_ir_bytes() -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_slice(CH03_IR).ok()?;
    let obj = value.as_object_mut()?;
    let pack = obj.get_mut("pack")?.as_object_mut()?;
    pack.insert("id".to_owned(), CH03_LINKS_PACK_ID.into());
    pack.insert(
        "title".to_owned(),
        "章のリンクの再読み込みの試験データ".into(),
    );
    let starts: Vec<serde_json::Value> = CH03_LINK_POINTS
        .iter()
        .map(|(id, node)| serde_json::json!({ "id": id, "node": node, "effects": [] }))
        .collect();
    obj.insert("starts".to_owned(), starts.into());
    serde_json::to_vec(&value).ok()
}

/// 対照の教材の `pack.id`。他の試験の鍵と分ける。
pub(crate) const CH03_NOLABEL_PACK_ID: &str = "ch0-3-nolabel";

/// 20 Step の通しの教材から、確認 `answer` の選択肢の `label` をすべて外した IR を作る（`pack.id` だけ別にする）。
/// 呼び出し元は `?fixture=ch0-3-nolabel` の mount で、ブラウザ試験 `choice-labels` が「ID が見える文字に出る」件数を
/// 数える方法の検出力を確かめる対照に使う（TERM-25。TASK-446）。入力方式 `choice` の `label` は残す。
/// `label` の欠けは `validate` が止める（PACK-37）ので、本物の教材ではこの形にならない。`compile` だけを通した IR の形を写す。
pub(crate) fn ch03_nolabel_ir_bytes() -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_slice(CH03_IR).ok()?;
    let obj = value.as_object_mut()?;
    let pack = obj.get_mut("pack")?.as_object_mut()?;
    pack.insert("id".to_owned(), CH03_NOLABEL_PACK_ID.into());
    pack.insert(
        "title".to_owned(),
        "選択肢の表示名の対照の試験データ".into(),
    );
    for n in obj.get_mut("nodes")?.as_array_mut()? {
        let Some(confirm) = n.get_mut("step").and_then(|s| s.get_mut("confirm")) else {
            continue;
        };
        if confirm.get("kind").and_then(|k| k.as_str()) != Some("answer") {
            continue;
        }
        for opt in confirm.get_mut("options")?.as_array_mut()? {
            opt.as_object_mut()?.remove("label");
        }
    }
    serde_json::to_vec(&value).ok()
}

/// 「直す」の候補の試験の教材の `pack.id`。他の試験の鍵と分ける。
pub(crate) const CH03_REVISE_PACK_ID: &str = "ch0-3-revise";

/// 20 Step の通しの教材の ST-2.6（`main/st-2-6`）に、「直す」の候補と表示名を宣言した IR を作る（`pack.id` だけ別にする）。
/// 正しい対象 `t-expect`（規則の対象）と誤った候補 2 件を、表示名つきで並べる。誤った候補は規則に無く、
/// core が拒否する（`rj-need-target`）。開始点 `s-2-6` を足し、前の Step の効果なしで ST-2.6 から開けるようにする
/// （直接リンクと同じ限界。5.199）。呼び出し元は `?fixture=ch0-3-revise` の mount で、ブラウザ試験
/// `approval-revise-labels` が使う（TERM-28・INTRO-29。TASK-448 1/2）。本物の教材の宣言は教材 repo の持ち分。
pub(crate) fn ch03_revise_ir_bytes() -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_slice(CH03_IR).ok()?;
    let obj = value.as_object_mut()?;
    let pack = obj.get_mut("pack")?.as_object_mut()?;
    pack.insert("id".to_owned(), CH03_REVISE_PACK_ID.into());
    pack.insert("title".to_owned(), "直す対象の表示名の試験データ".into());
    for n in obj.get_mut("nodes")?.as_array_mut()? {
        if n.get("uid")?.as_str()? != "main/st-2-6" {
            continue;
        }
        n.get_mut("step")?.as_object_mut()?.insert(
            "input".to_owned(),
            serde_json::json!({
                "kind": "approval",
                "revise": { "candidates": [
                    { "id": "t-other-a", "label": "冒頭の見出し" },
                    { "id": "t-expect", "label": "期待する結果" },
                    { "id": "t-other-b", "label": "末尾の注記" }
                ] }
            }),
        );
    }
    obj.get_mut("starts")?
        .as_array_mut()?
        .push(serde_json::json!(
            { "id": "s-2-6", "node": "main/st-2-6", "effects": [] }
        ));
    serde_json::to_vec(&value).ok()
}

/// 章の先頭 4 件の開始点の ID と節点（先頭から順）。
pub(crate) const CH03_CHAPTER_POINTS: [(&str, &str); 4] = [
    ("s-0-1", "main/st-0-1"),
    ("s-1-1", "main/st-1-1"),
    ("s-2-1", "main/st-2-1"),
    ("s-3-1", "main/st-3-1"),
];

/// 章の境目を無条件の `next` でつなぐ埋め込みの IR を、各章の最後（st-0-2c・st-1-6・st-2-8）を `final` にした形へ書き換える（`pack.id` だけ別にする）。
/// 章の完了が起きるので、「次の章へ進む」が出る。`tests/next_chapter.rs` の `variant_json` と同じ書き換えで、2 か所を揃えて保つ。
pub(crate) fn ch03_chapters_ir_bytes() -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_slice(CH03_IR).ok()?;
    let obj = value.as_object_mut()?;
    let pack = obj.get_mut("pack")?.as_object_mut()?;
    pack.insert("id".to_owned(), "ch0-3-chapters".into());
    pack.insert("title".to_owned(), "章の移動の通しの試験データ".into());
    for n in obj.get_mut("nodes")?.as_array_mut()? {
        let uid = n.get("uid")?.as_str()?.to_owned();
        if ["main/st-0-2c", "main/st-1-6", "main/st-2-8"].contains(&uid.as_str()) {
            let step = n.get_mut("step")?.as_object_mut()?;
            step.insert("final".to_owned(), true.into());
            step.insert("next".to_owned(), serde_json::json!([]));
        }
    }
    let starts: Vec<serde_json::Value> = CH03_CHAPTER_POINTS
        .iter()
        .map(|(id, node)| serde_json::json!({ "id": id, "node": node, "effects": [] }))
        .collect();
    obj.insert("starts".to_owned(), starts.into());
    serde_json::to_vec(&value).ok()
}

/// 範囲外の章（CH-4）の最初の Step の uid・開始点の ID・題。ブラウザ試験の期待値と揃える（`tools/browser/out-of-range-chapter.test.mjs`）。
pub(crate) const OUT_OF_RANGE_NODE: &str = "main/st-4-1";
pub(crate) const OUT_OF_RANGE_START_ID: &str = "s-4-1";
pub(crate) const OUT_OF_RANGE_TITLE: &str = "範囲外の章の最初の Step";

/// CH-3 を最後に使う IR を作る。`ch03_chapters_ir_bytes` と同じ書き換え（各章の最後を `final` にする）をしたうえで、
/// 範囲外の章 `ch-4` のノードを 1 つ足す。開始点は CH-0〜CH-3 の先頭 4 件で、`with_out_of_range_start` が真のときだけ CH-4 の開始点を足す。
/// CH-3 の最後（`main/st-3-2`）は `final` のままで、CH-4 への `next` は作らない（他の章への `next` は `compile` が `E-REF-STEP` にする形。TASK-67）。
/// 試行の IR を `compile` で絞る指定は TASK-67 が持つので、本関数は試験の中で手で作る。
pub(crate) fn ch03_trial_ir_bytes(with_out_of_range_start: bool) -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_slice(&ch03_chapters_ir_bytes()?).ok()?;
    let obj = value.as_object_mut()?;
    let pack = obj.get_mut("pack")?.as_object_mut()?;
    let (id, title) = if with_out_of_range_start {
        ("ch0-4-full", "範囲外の開始点を持つ対照の試験データ")
    } else {
        ("ch0-3-trial", "CH-3 を最後に使う絞った試験データ")
    };
    pack.insert("id".to_owned(), id.into());
    pack.insert("title".to_owned(), title.into());
    let nodes = obj.get_mut("nodes")?.as_array_mut()?;
    let mut out_of_range = nodes
        .iter()
        .find(|n| n.get("uid").and_then(serde_json::Value::as_str) == Some("main/st-3-2"))?
        .clone();
    out_of_range["uid"] = OUT_OF_RANGE_NODE.into();
    out_of_range["unit"] = "ch-4".into();
    out_of_range["step"]["id"] = "st-4-1".into();
    out_of_range["step"]["title"] = OUT_OF_RANGE_TITLE.into();
    nodes.push(out_of_range);
    if with_out_of_range_start {
        obj.get_mut("starts")?
            .as_array_mut()?
            .push(serde_json::json!({
                "id": OUT_OF_RANGE_START_ID, "node": OUT_OF_RANGE_NODE, "effects": []
            }));
    }
    serde_json::to_vec(&value).ok()
}

/// 起動の前に選んだ教材の IR が読めないとき、その理由。読める・読めない教材の選択でなければ `None`。
pub(crate) fn mount_refusal(fixture: MountFixture) -> Option<IrLoadError> {
    let kind = match fixture {
        MountFixture::FutureFormat => FutureIr::Format,
        MountFixture::FutureField => FutureIr::Field,
        _ => return None,
    };
    load_ir(&future_ir_bytes(kind)?).err()
}

/// URL のクエリ（`location.search`）から mount の教材を選ぶ。`?fixture=reload-points` の完全一致だけが
/// 8 Step の教材、`?fixture=no-input` の完全一致だけが入力欄なしの教材、`?fixture=ch0-3` の完全一致だけが 20 Step の通しの教材を選び、
/// 空・未知の名前・余分な文字・他のキーとの併用は、すべて既定に倒す。
/// 例外は直接リンクの `start`（TASK-109 1/2）で、`start=...` の対だけを取り除いた残りで同じ完全一致を行う。
/// `start` の値はここでは解釈しない（開始点の照合は `start_select` が行い、値の出所は `static/app.js` が渡す引数に一本化する）。
pub(crate) fn mount_fixture_from_search(search: &str) -> MountFixture {
    let stripped = strip_start_pairs(search);
    match stripped.as_str() {
        "?fixture=reload-points" => MountFixture::ReloadPoints,
        "?fixture=no-input" => MountFixture::NoInput,
        "?fixture=ch0-3" => MountFixture::Ch03,
        "?fixture=ch0-3-links" => MountFixture::Ch03Links,
        "?fixture=ch0-3-nolabel" => MountFixture::Ch03NoLabel,
        "?fixture=ch0-3-revise" => MountFixture::Ch03Revise,
        "?fixture=ch0-3-chapters" => MountFixture::Ch03Chapters,
        "?fixture=ch0-3-trial" => MountFixture::Ch03Trial,
        "?fixture=ch0-4-full" => MountFixture::Ch04Full,
        "?fixture=multi-start" => MountFixture::MultiStart,
        "?fixture=chapter-a" => MountFixture::ChapterA,
        "?fixture=chapter-b" => MountFixture::ChapterB,
        "?fixture=chapter-a-next" => MountFixture::ChapterANext,
        "?fixture=future-ir-format" => MountFixture::FutureFormat,
        "?fixture=future-ir-field" => MountFixture::FutureField,
        _ => MountFixture::Default,
    }
}

/// クエリ（`?` 付き）から `start=` で始まる対をすべて取り除く。残りが `?` だけなら `?` を返す。
fn strip_start_pairs(search: &str) -> String {
    let Some(rest) = search.strip_prefix('?') else {
        return search.to_owned();
    };
    let kept: Vec<&str> = rest
        .split('&')
        .filter(|pair| !pair.starts_with("start="))
        .collect();
    format!("?{}", kept.join("&"))
}

/// 選んだ教材の IR のバイト列。読めない想定の IR（未来の版）も含む。作れなければ `None`。
fn mount_ir_bytes(fixture: MountFixture) -> Option<Vec<u8>> {
    Some(match fixture {
        MountFixture::Default => IR.to_vec(),
        MountFixture::ReloadPoints => RELOAD_POINTS_IR.to_vec(),
        MountFixture::NoInput => NO_INPUT_IR.to_vec(),
        MountFixture::Ch03 => CH03_IR.to_vec(),
        MountFixture::Ch03Links => ch03_links_ir_bytes()?,
        MountFixture::Ch03NoLabel => ch03_nolabel_ir_bytes()?,
        MountFixture::Ch03Revise => ch03_revise_ir_bytes()?,
        MountFixture::Ch03Chapters => ch03_chapters_ir_bytes()?,
        MountFixture::Ch03Trial => ch03_trial_ir_bytes(false)?,
        MountFixture::Ch04Full => ch03_trial_ir_bytes(true)?,
        MountFixture::MultiStart => multi_start_ir_bytes()?,
        MountFixture::ChapterA => chapter_ir_bytes(Chapter::A)?,
        MountFixture::ChapterB => chapter_ir_bytes(Chapter::B)?,
        MountFixture::ChapterANext => chapter_ir_bytes(Chapter::ANext)?,
        // 読めない想定の IR。万一読めたら session になり、通知が出ないのでブラウザ試験が失敗する（fail-closed）。
        MountFixture::FutureFormat => future_ir_bytes(FutureIr::Format)?,
        MountFixture::FutureField => future_ir_bytes(FutureIr::Field)?,
    })
}

/// 選んだ教材と開始直後の状態から実行環境を作る。読めなければ `None`（単体試験が使う。ホストは `mount_session_at` を使う）。
#[cfg(test)]
pub(crate) fn mount_session(fixture: MountFixture) -> Option<Session> {
    session_of(&mount_ir_bytes(fixture)?)
}

/// [`mount_session`] を、直接リンクの `start`（`None` は指定なし）で開始点を選べるようにしたもの（TASK-109 1/2）。
///
/// 照合は `start_select::select_start`（完全一致だけ）。一致しなければ `Err`（呼び出し側は session を作らず開かない）。
/// 教材を読めない・開始の節点が無いときは `Ok(None)`（呼び出し側は session なしに倒す）。
/// 成功したときは、実行環境と選んだ開始点の ID を返す。
pub(crate) fn mount_session_at(
    fixture: MountFixture,
    start: Option<&str>,
) -> Result<Option<(Session, String)>, StartRefusal> {
    let Some(bytes) = mount_ir_bytes(fixture) else {
        return Ok(None);
    };
    let Ok(ir) = load_ir(&bytes) else {
        return Ok(None);
    };
    let id = select_start(&ir, start)?.id.clone();
    Ok(session_at(&bytes, Some(&id)).map(|session| (session, id)))
}

/// 教材のバイト列と開始直後の状態から実行環境を作る。`Session::from_state` の唯一の呼び出し（`tests/no_progress.rs`）。
///
/// 開始の節点は教材の `starts[0].node` で、契約の試料の開始状態の節点（`main/st-1`）をそれに合わせる。
/// 既存の教材は先頭が `main/st-1` で、結果は変わらない。節点が教材に無い・`starts` が読めないときは `None`。
fn session_of(ir_bytes: &[u8]) -> Option<Session> {
    session_at(ir_bytes, None)
}

/// [`session_of`] の開始の節点を、`starts` の ID で選べるようにしたもの。`None` は `starts[0]`。
///
/// ID は教材の `starts` から引き、Rust のコードに教材の節点の名前を書かない。ID が無いときは `None`。
fn session_at(ir_bytes: &[u8], start_id: Option<&str>) -> Option<Session> {
    let ir = load_ir(ir_bytes).ok()?;
    let value: serde_json::Value = serde_json::from_slice(ir_bytes).ok()?;
    let starts = value.get("starts")?.as_array()?;
    let start = match start_id {
        None => starts.first()?,
        Some(id) => starts
            .iter()
            .find(|s| s.get("id").and_then(serde_json::Value::as_str) == Some(id))?,
    };
    let node = start.get("node")?.as_str()?;
    ir.nodes.iter().any(|n| n.uid == node).then_some(())?;
    Some(Session::from_state(ir, start_state_at(node)?))
}

/// 任意の教材（IR の JSON のバイト列）から実行環境を作る（TASK-175 2/2。`load-ir:` が差し替える）。
///
/// 版の違う履歴 15 件の、開く側の教材をブラウザで入れ替えるために使う。IR を読めなければ `Err`（呼び出し側が
/// 理由コードだけを控える。VER-13）、読めても開始の節点が無いときは `Ok(None)`。
#[cfg(feature = "test-hooks")]
pub(crate) fn load_session_from_ir_bytes(ir_bytes: &[u8]) -> Result<Option<Session>, IrLoadError> {
    load_ir(ir_bytes)?;
    Ok(session_of(ir_bytes))
}

/// 巡回する IR（`use` の `enter` の自己参照・2 者の巡回・`exit_to` の 2 者の巡回・1 入力あたりの遷移が少数の巡回）。
/// 作成者向けの検査を通さずに配られた IR の想定で、遷移の上限（SAFE-23）の試験が使う。spec の写しで、編集しない。
#[cfg(feature = "test-hooks")]
const CYCLE_ENTER_SELF_IR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir/cycle-enter-self.json"
));
#[cfg(feature = "test-hooks")]
const CYCLE_ENTER_2_IR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir/cycle-enter-2.json"
));
#[cfg(feature = "test-hooks")]
const CYCLE_EXIT_2_IR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir/cycle-exit-2.json"
));
#[cfg(feature = "test-hooks")]
const CYCLE_D07_IR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir/cycle-d07-flat.json"
));

/// 巡回する IR の試験用の実行環境（遷移の上限の試験。`load-fixture:cycle-*`）。
///
/// 開始の節点は、`enter` の巡回の 2 本は `starts[0]`（通常の Step。試験が `go_to` で巡回の入口へ移す）、
/// `exit_to` の巡回は `starts` の ID `s-exit-2`、1 入力あたりの遷移が少数の対照は ID `s-d07` の節点にする。
/// 読めなければ `None`（呼び出し側は何もしない）。
#[cfg(feature = "test-hooks")]
pub(crate) fn cycle_session(ir: CycleIr) -> Option<Session> {
    match ir {
        CycleIr::EnterSelf => session_at(CYCLE_ENTER_SELF_IR, None),
        CycleIr::Enter2 => session_at(CYCLE_ENTER_2_IR, None),
        CycleIr::Exit2 => session_at(CYCLE_EXIT_2_IR, Some("s-exit-2")),
        CycleIr::D07 => session_at(CYCLE_D07_IR, Some("s-d07")),
    }
}

/// 巡回する IR の種類（試験の設定だけ）。
#[cfg(feature = "test-hooks")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CycleIr {
    /// `enter` が自分を指す。
    EnterSelf,
    /// `enter` が 2 者で巡回する。
    Enter2,
    /// `exit_to` が 2 者で巡回する。
    Exit2,
    /// 巡回していても 1 入力あたりの遷移が少数の対照。
    D07,
}

/// 巡回しない `use` の長い連鎖の試験用の実行環境（遷移の上限の境界。`load-fixture:chain-1000`・`chain-1001`）。
///
/// 埋め込みの教材の `main/st-2`（選択の入力、`confirm: none`）の達成で、`use` の Step `len` 個を `enter` でたどって
/// 最後の Step `main/st-3` に着く。着地は `len + 1` 回で、`len` が 999 なら 1,000 回ちょうど（通る）、1,000 なら
/// 1,001 回（止まる）。`use` の Step は `main/st-3` の複製で、`enter` だけを足す。
#[cfg(feature = "test-hooks")]
pub(crate) fn chain_session(len: usize) -> Option<Session> {
    let mut value: serde_json::Value = serde_json::from_slice(IR).ok()?;
    let nodes = value.get_mut("nodes")?.as_array_mut()?;
    let template = nodes
        .iter()
        .find(|n| n.get("uid").and_then(|u| u.as_str()) == Some("main/st-3"))?
        .clone();
    let first = if len == 0 {
        "main/st-3".to_owned()
    } else {
        "main/u-0".to_owned()
    };
    let entry = nodes
        .iter_mut()
        .find(|n| n.get("uid").and_then(|u| u.as_str()) == Some("main/st-2"))?;
    entry
        .get_mut("step")?
        .as_object_mut()?
        .insert("next".to_owned(), serde_json::json!([{ "to": first }]));
    let mut added = Vec::with_capacity(len);
    for i in 0..len {
        let mut n = template.clone();
        let target = if i + 1 == len {
            "main/st-3".to_owned()
        } else {
            format!("main/u-{}", i + 1)
        };
        let obj = n.as_object_mut()?;
        obj.insert("uid".to_owned(), serde_json::json!(format!("main/u-{i}")));
        obj.insert("enter".to_owned(), serde_json::json!(target));
        obj.get_mut("step")?
            .as_object_mut()?
            .insert("id".to_owned(), serde_json::json!(format!("u-{i}")));
        added.push(n);
    }
    value.get_mut("nodes")?.as_array_mut()?.extend(added);
    session_of(&serde_json::to_vec(&value).ok()?)
}

/// 確認が `answer` の Step を持つ試験用の実行環境（TASK-445 2/2。`load-fixture:answer` が差し替える）。
///
/// 埋め込みの教材の `main/st-1` の確認を、選択肢 2 件（誤り・正解）の `answer` に置き換える。誤答の文面は教材が
/// もともと持つ `rj-decline` を指す。mount の既定の教材（`min-3step`）は、st-1 の `point_at` に依る既存の試験のため変えない。
/// 読めない・節点が無い・`load_ir` を通らないときは `None`（呼び出し側は何もしない）。
#[cfg(feature = "test-hooks")]
pub(crate) fn answer_session() -> Option<Session> {
    let mut value: serde_json::Value = serde_json::from_slice(IR).ok()?;
    let node = value
        .get_mut("nodes")?
        .as_array_mut()?
        .iter_mut()
        .find(|n| n.get("uid").and_then(|u| u.as_str()) == Some("main/st-1"))?;
    node.get_mut("step")?.as_object_mut()?.insert(
        "confirm".to_owned(),
        serde_json::json!({
            "kind": "answer",
            "options": [
                { "id": "opt-wrong", "correct": false, "label": "誤った案" },
                { "id": "opt-right", "correct": true, "label": "正しい案" }
            ],
            "on_wrong": "rj-decline"
        }),
    );
    session_of(&serde_json::to_vec(&value).ok()?)
}

/// 複数の種類のビューを Step ごとに宣言する試験用の実行環境（TASK-470 2/2。`load-fixture:views`）。
///
/// 埋め込みの教材の `views` に `card_list`・`check_list` を足し、各 Step の宣言を次にする。
/// `main/st-1` は file_tree・card_list・check_list（3 種の切り替え）、`main/st-2` は card_list・file_tree
/// （st-1 から移ると既定が変わる）、`main/st-3` は card_list（st-2 から移っても種類が同じ）。
/// `v-files` は契約の試料の操作列が指すため残す。読めない・節点が無いときは `None`。
#[cfg(feature = "test-hooks")]
pub(crate) fn views_session() -> Option<Session> {
    let mut value: serde_json::Value = serde_json::from_slice(IR).ok()?;
    let views = value.get_mut("views")?.as_object_mut()?;
    views.insert(
        "v-cards".to_owned(),
        serde_json::json!({"kind": "card_list"}),
    );
    views.insert(
        "v-checks".to_owned(),
        serde_json::json!({"kind": "check_list"}),
    );
    let decl = [
        (
            "main/st-1",
            serde_json::json!(["v-files", "v-cards", "v-checks"]),
        ),
        ("main/st-2", serde_json::json!(["v-cards", "v-files"])),
        ("main/st-3", serde_json::json!(["v-cards"])),
    ];
    for node in value.get_mut("nodes")?.as_array_mut()? {
        let uid = node.get("uid").and_then(|u| u.as_str()).map(str::to_owned);
        if let Some((_, d)) = decl.iter().find(|(u, _)| Some(*u) == uid.as_deref()) {
            node.get_mut("step")?
                .as_object_mut()?
                .insert("views".to_owned(), d.clone());
        }
    }
    session_of(&serde_json::to_vec(&value).ok()?)
}

/// 入力方式が `approval` で、対象つきの「直す」の規則を持つ試験用の実行環境（TASK-447 1/2。`load-fixture:approval`）。
///
/// 埋め込みの教材の `main/st-1` を、対象 2 つ（`t-alpha`・`t-beta`）の「直す」で受付、対象なしの「直す」と
/// 早すぎる「はい」を既存の `rj-decline`（次の行動つき）で拒否する規則に置き換える。確認は `approve`。
/// 規則は細い形（`null` の欄なし）で書く。太い形の読みは core の試験が固定する。
#[cfg(feature = "test-hooks")]
pub(crate) fn approval_session() -> Option<Session> {
    let mut value: serde_json::Value = serde_json::from_slice(IR).ok()?;
    let node = value
        .get_mut("nodes")?
        .as_array_mut()?
        .iter_mut()
        .find(|n| n.get("uid").and_then(|u| u.as_str()) == Some("main/st-1"))?;
    let step = node.get_mut("step")?.as_object_mut()?;
    step.insert(
        "input".to_owned(),
        serde_json::json!({ "kind": "approval" }),
    );
    step.insert(
        "confirm".to_owned(),
        serde_json::json!({ "kind": "approve" }),
    );
    let accept = |id: &str, target: &str| {
        serde_json::json!({
            "id": id,
            "on": { "approval": "revise", "target": target },
            "outcome": { "kind": "accept" }
        })
    };
    let reject = |id: &str, on: serde_json::Value| {
        serde_json::json!({
            "id": id,
            "on": on,
            "outcome": { "kind": "reject", "rejection": "rj-decline" }
        })
    };
    step.insert(
        "rules".to_owned(),
        serde_json::json!([
            accept("r-alpha", "t-alpha"),
            accept("r-beta", "t-beta"),
            reject("r-bare-revise", serde_json::json!({ "approval": "revise" })),
            reject("r-early", serde_json::json!({ "approval": "approve" })),
        ]),
    );
    session_of(&serde_json::to_vec(&value).ok()?)
}

/// 入力方式が `approval` で、「直す」の対象の候補と表示名（誤った候補つき）を宣言した Step の教材（TASK-448 1/2。`load-fixture:approval-candidates`）。
///
/// [`approval_session`] と同じ規則で、`main/st-1` の `input` に候補 3 件（宣言順に t-alpha・t-gamma・t-beta）を表示名つきで足す。
/// `t-gamma` は規則に無い誤った候補で、受付にならず対象なしの「直す」と同じ拒否（`rj-decline`）になる。
#[cfg(feature = "test-hooks")]
pub(crate) fn approval_candidates_session() -> Option<Session> {
    let mut value: serde_json::Value = serde_json::from_slice(IR).ok()?;
    let node = value
        .get_mut("nodes")?
        .as_array_mut()?
        .iter_mut()
        .find(|n| n.get("uid").and_then(|u| u.as_str()) == Some("main/st-1"))?;
    let step = node.get_mut("step")?.as_object_mut()?;
    step.insert(
        "input".to_owned(),
        serde_json::json!({
            "kind": "approval",
            "revise": { "candidates": [
                { "id": "t-alpha", "label": "冒頭の見出し" },
                { "id": "t-gamma", "label": "中ほどの図表" },
                { "id": "t-beta", "label": "末尾の注記" }
            ] }
        }),
    );
    step.insert(
        "confirm".to_owned(),
        serde_json::json!({ "kind": "approve" }),
    );
    let accept = |id: &str, target: &str| {
        serde_json::json!({
            "id": id,
            "on": { "approval": "revise", "target": target },
            "outcome": { "kind": "accept" }
        })
    };
    let reject = |id: &str, on: serde_json::Value| {
        serde_json::json!({
            "id": id,
            "on": on,
            "outcome": { "kind": "reject", "rejection": "rj-decline" }
        })
    };
    step.insert(
        "rules".to_owned(),
        serde_json::json!([
            accept("r-alpha", "t-alpha"),
            accept("r-beta", "t-beta"),
            reject("r-bare-revise", serde_json::json!({ "approval": "revise" })),
            reject("r-early", serde_json::json!({ "approval": "approve" })),
        ]),
    );
    session_of(&serde_json::to_vec(&value).ok()?)
}

/// 入力方式が `ui_action` の Step を持つ試験用の実行環境（TASK-450 1/3。`load-fixture:ui-action`）。
///
/// 埋め込みの教材の `main/st-2` を、対象 `v-files` の `notes/draft` を指すと受付になる `ui_action` に置き換える。
/// 確認は `none` のままで、受付がそのまま達成になる。`notes/draft` は st-1 の受付が作る行で、st-2 は `v-files` を
/// 宣言しているため、st-2 に入った時点で左の行は指せる行になる。規則は細い形で書く。
#[cfg(feature = "test-hooks")]
pub(crate) fn ui_action_session() -> Option<Session> {
    let mut value: serde_json::Value = serde_json::from_slice(IR).ok()?;
    let node = value
        .get_mut("nodes")?
        .as_array_mut()?
        .iter_mut()
        .find(|n| n.get("uid").and_then(|u| u.as_str()) == Some("main/st-2"))?;
    let step = node.get_mut("step")?.as_object_mut()?;
    step.insert(
        "input".to_owned(),
        serde_json::json!({
            "kind": "ui_action",
            "targets": [{ "view": "v-files", "path": "notes/draft" }]
        }),
    );
    step.insert(
        "rules".to_owned(),
        serde_json::json!([{
            "id": "r-point",
            "on": { "ui": { "view": "v-files", "path": "notes/draft" } },
            "outcome": { "kind": "accept" }
        }]),
    );
    session_of(&serde_json::to_vec(&value).ok()?)
}

/// 契約の試料の開始状態が現在の Step とする節点。
const SAMPLE_START: &str = "main/st-1";

/// 契約の試料の開始状態の節点を `node` に置き換えた状態。試料の `current`・`status` の鍵・入口の写しの同じ欄が対象で、
/// 他の欄は変えない。`node` は教材の `starts[0].node`（埋め込みで外から入らない）で、JSON の文字列として組み立て直す。
fn start_state_at(node: &str) -> Option<SessionState> {
    let value: serde_json::Value = serde_json::from_slice(SAMPLE).ok()?;
    let initial = value.get("initial")?;
    let from = serde_json::to_string(SAMPLE_START).ok()?;
    let to = serde_json::to_string(node).ok()?;
    let text = serde_json::to_string(initial).ok()?.replace(&from, &to);
    let mut state: SessionState = serde_json::from_str(&text).ok()?;
    // 開始の節点は `use` の Step にしない前提で、切り身の鍵は `uid.root()` で足りる（呼び出しの Step の持ち主は `owner_of`。MOD-3）。
    // 切り身の鍵は Step の実体の根（`main` か、入れ子の実体の ID）。節点が `main` の外にあるときは鍵も合わせる
    // （合わせないと、開始の Step の状態を引けず、入力が受け付けられない）。
    let uid = StepUid(node.to_owned());
    let root = uid.root();
    let main = StepUid(SAMPLE_START.to_owned()).root();
    if root != main {
        if let Some(slice) = state.run.slices.remove(&main) {
            state.run.slices.insert(root.clone(), slice);
        }
        if let Some(entry) = state.entry.as_mut() {
            if let Some(slice) = entry.run.slices.remove(&main) {
                entry.run.slices.insert(root, slice);
            }
        }
    }
    Some(state)
}

/// ツリーの試料の 1 行。`(id, depth, kind, before, after)`。値は汎用のダミーの文字列で、教材固有の語を使わない。
#[cfg(target_arch = "wasm32")]
type TreeSpec = (
    &'static str,
    u8,
    crate::artifact_view::RowKind,
    Option<&'static str>,
    Option<&'static str>,
);

/// ファイルツリーの試料。追加・削除の行を含み、後の状態で 6 行が出る。
#[cfg(target_arch = "wasm32")]
const FILE_TREE_SPECS: [TreeSpec; 6] = {
    use crate::artifact_view::RowKind::{Dir, File};
    [
        ("d-a", 0, Dir, Some("dir-a"), Some("dir-a")),
        ("f-a1", 1, File, Some("file-a1"), Some("file-a1")),
        ("d-a2", 1, Dir, Some("dir-a2"), Some("dir-a2")),
        ("f-a2x", 2, File, None, Some("file-a2x")),
        ("f-b", 0, File, Some("file-b"), Some("file-b")),
        ("f-old", 0, File, Some("file-old"), None),
    ]
};

/// タスクツリーの試料。後の状態で 5 行が出る。
#[cfg(target_arch = "wasm32")]
const TASK_TREE_SPECS: [TreeSpec; 5] = {
    use crate::artifact_view::RowKind::Task;
    [
        ("t-1", 0, Task, Some("task-1"), Some("task-1")),
        ("t-11", 1, Task, Some("task-11"), Some("task-11")),
        ("t-12", 1, Task, Some("task-12"), Some("task-12")),
        ("t-2", 0, Task, Some("task-2"), Some("task-2")),
        ("t-21", 1, Task, Some("task-21"), Some("task-21")),
    ]
};

/// ツリーの試料を、後の状態・指定の変種で描いた HTML 文字列にする（`debug_render_tree` の実体）。
///
/// `kind` はツリーの種類（`file_tree`・`task_tree`）、`variant` は `a`・`b`。どれかが範囲外、または描けなければ `None`。
/// 試料は埋め込みで、外からのデータは受け取らない。行の項目は `tree_attrs` の属性を `view()` の側で直接書く（F-2）。
#[cfg(target_arch = "wasm32")]
pub(crate) fn render_tree_sample(kind: &str, variant: &str) -> Option<String> {
    use crate::artifact_view::{
        tree_view_list, Cell, Phase, Row, RowId, Variant, ViewKind, ROW_ATTR,
    };
    use crate::tree_attrs::shown_tree_attrs;
    use fandhe_frontend_core::{el, render, text};

    let kind = ViewKind::parse(kind).filter(|k| k.is_tree())?;
    let variant = match variant {
        "a" => Variant::A,
        "b" => Variant::B,
        _ => return None,
    };
    let specs: &[TreeSpec] = match kind {
        ViewKind::FileTree => &FILE_TREE_SPECS,
        _ => &TASK_TREE_SPECS,
    };
    let cell = |s: &str| Cell {
        label: s.to_owned(),
        ..Cell::default()
    };
    let rows: Vec<Row<Cell>> = specs
        .iter()
        .map(|&(id, depth, k, before, after)| {
            Row::new(
                RowId::new(id).ok()?,
                depth,
                k,
                before.map(cell),
                after.map(cell),
            )
            .ok()
        })
        .collect::<Option<_>>()?;
    let attrs = shown_tree_attrs(&rows, Phase::After);
    let items = rows
        .iter()
        .zip(attrs)
        .filter_map(|(row, tree)| {
            let shown = row.shown(Phase::After)?;
            let mut a: Vec<(&str, &str)> = vec![(ROW_ATTR, row.id().as_str())];
            // 変種 (a) は対照で、木の属性を付けない。
            let owned = tree.filter(|_| variant == Variant::B);
            if let Some(t) = &owned {
                a.extend(t.pairs());
            }
            Some((
                format!("k{}", row.id().as_str()),
                el("li", a, vec![text(&shown.content.label)]),
            ))
        })
        .collect();
    let node = tree_view_list("ul", kind, variant, true, items).ok()?;
    Some(render(&node))
}

/// 契約の試料の「2 周分」の履歴。書き出しの試験（TASK-330 1/2）が使う。
#[cfg(any(target_arch = "wasm32", test))]
const TWO_ROUNDS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/contract/history/two-rounds.json"
));

/// 2 周分の履歴の包みを返す。`Session` を作らず、読み込みの入口（段 1〜5）だけで読む。読めなければ `None`。
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn two_rounds_envelope(
) -> Option<fandhe_workshop_core::history_envelope::SessionEnvelope> {
    use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
    parse_history_envelope(parse_history_json(TWO_ROUNDS).ok()?).ok()
}

/// 開ける履歴の包みを返す（TASK-330 2/2）。
///
/// 契約の試料（2 周分）の `ops` を、試験の教材と開始直後の状態に適用し直し、得た状態を写しにして包み直す。
/// 試料そのままの写しは再読み込みの意味の実装前の値で、読み込みの段 8 で `replay_mismatch` になるため使えない。
/// 受講中に記録した `ops` ではなく、試料を適用し直したものである（ホストは受講中の `ops` を記録するが、この試料は受講中の記録ではない）。
/// 2 周分・停止・保存・再読み込みを含むため、復習・チェックポイント・世界状態は空でない。
#[cfg(target_arch = "wasm32")]
pub(crate) fn loadable_envelope() -> Option<fandhe_workshop_core::history_envelope::SessionEnvelope>
{
    use fandhe_workshop_core::history_envelope::build_envelope;
    use fandhe_workshop_core::history_replay::replay;

    let ir = load_ir(IR).ok()?;
    let env = two_rounds_envelope()?;
    let state = replay(&ir, start_state_at(SAMPLE_START)?, &env.ops).ok()?;
    build_envelope(&ir, &env.start, env.ops, state).ok()
}

/// 10MB に近い履歴の合成の種類（TASK-166・SAFE-31）。`large_loadable_envelope` が完全一致で受ける。
#[cfg(any(target_arch = "wasm32", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LargeKind {
    /// 秘密情報の形の接頭辞を含まない ASCII の文。
    Plain,
    /// 接頭辞の候補が密に現れるが、長さ・枠を満たさず検出には掛からない文（走査の最悪側）。
    Dense,
    /// `Plain` の中ほどの 1 件にだけ、検出に掛かる値を置く対照。
    Marker,
}

#[cfg(any(target_arch = "wasm32", test))]
impl LargeKind {
    /// 試験用の口が渡す名前から種類を引く。完全一致だけを受け、未知の名前は `None`。
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name {
            "plain" => Some(Self::Plain),
            "dense" => Some(Self::Dense),
            "marker" => Some(Self::Marker),
            _ => None,
        }
    }
}

/// 合成の送信 1 件の文字数と件数。直列化した大きさが 9.5MiB〜10MiB に入るように決めた値
/// （単体試験が範囲を縛る）。受け付けられない入力は状態の写しにも残るため、本文は履歴の中に 2 回入る。1 件は `MAX_INPUT_CODE_POINTS` より十分に小さい。
#[cfg(any(target_arch = "wasm32", test))]
const LARGE_OP_CHARS: usize = 8_000;
#[cfg(any(target_arch = "wasm32", test))]
const LARGE_OP_COUNT: usize = 620;

/// 送信 1 件の合成の本文。乱数は使わず、`index` だけで決まる。
#[cfg(any(target_arch = "wasm32", test))]
fn large_op_text(kind: LargeKind, index: usize) -> String {
    // 接頭辞は 1 つの文字列に連続させない（完成した値の形を repo に置かない）。
    let dense: [String; 10] = [
        format!("{}{} short", "ey", "J"),
        format!("{}{}//x", "ht", "tp:"),
        format!("{}{} x", "-----BEG", "IN "),
        format!("{}{}ZZ", "AK", "IA"),
        format!("{}{}ab", "sk", "-"),
        format!("{}{}b-1", "xo", "x"),
        format!("{}{}=k", "aws_secret_", "access_key"),
        format!("{}{}x", "sk", "_live_"),
        format!("{}{}://u", "my", "sql"),
        format!("{}{}x", "AI", "za"),
    ];
    let mut out = String::with_capacity(LARGE_OP_CHARS + 32);
    let mut n = index;
    while out.len() < LARGE_OP_CHARS {
        match kind {
            LargeKind::Dense => out.push_str(&dense[n % dense.len()]),
            _ => out.push_str("the quick brown fox jumps over the lazy dog"),
        }
        out.push_str(&format!(" note {n} "));
        n += 1;
    }
    out.truncate(LARGE_OP_CHARS);
    if kind == LargeKind::Marker && index == LARGE_OP_COUNT / 2 {
        // 検出に掛かる 1 件。部品から実行時に組み立てる。
        let value = format!(
            "{}{}{}-{}-{}",
            "xo",
            "xb-",
            "1".repeat(12),
            "2".repeat(12),
            "a".repeat(24)
        );
        out.replace_range(0..value.len(), &value);
    }
    out
}

/// 10MB に近い開ける包みを返す（TASK-166・SAFE-31）。
///
/// 契約の試料（2 周分）の `ops` の後ろに、合成の送信を `LARGE_OP_COUNT` 件足し、適用し直して包み直す
/// （写しが一致するので、読み込みの段 8 で `replay_mismatch` にならない）。UI から送信を重ねて伸ばさないのは、
/// 送信のたびに履歴の全体を再直列化・再検査するため、準備の費用が二次になるから。受講者のデータではない。
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn large_loadable_envelope(
    kind: LargeKind,
) -> Option<fandhe_workshop_core::history_envelope::SessionEnvelope> {
    use fandhe_workshop_core::history_envelope::build_envelope;
    use fandhe_workshop_core::history_replay::replay;
    use fandhe_workshop_core::ops_record::SessionOp;

    let ir = load_ir(IR).ok()?;
    let env = two_rounds_envelope()?;
    let mut ops = env.ops;
    ops.extend((0..LARGE_OP_COUNT).map(|i| SessionOp::Submit(large_op_text(kind, i))));
    let state = replay(&ir, start_state_at(SAMPLE_START)?, &ops).ok()?;
    build_envelope(&ir, &env.start, ops, state).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 完全一致のクエリだけが_8_step_の教材を選ぶ() {
        assert_eq!(
            mount_fixture_from_search("?fixture=reload-points"),
            MountFixture::ReloadPoints
        );
        for other in [
            "",
            "?",
            "?fixture=",
            "?fixture=unknown",
            "?fixture=reload-points&x=1",
            "?x=1&fixture=reload-points",
            "?fixture=reload-points2",
            "?fixture=Reload-Points",
            "fixture=reload-points",
            "?tick=5",
        ] {
            assert_eq!(
                mount_fixture_from_search(other),
                MountFixture::Default,
                "{other}"
            );
        }
    }

    #[test]
    fn 完全一致のクエリだけが入力欄なしの教材を選ぶ() {
        assert_eq!(
            mount_fixture_from_search("?fixture=no-input"),
            MountFixture::NoInput
        );
        for other in [
            "?fixture=no-input&x=1",
            "?x=1&fixture=no-input",
            "?fixture=no-input2",
            "?fixture=No-Input",
            "fixture=no-input",
        ] {
            assert_eq!(
                mount_fixture_from_search(other),
                MountFixture::Default,
                "{other}"
            );
        }
    }

    #[test]
    fn 入力欄なしの教材は鍵が別で入力欄なしを宣言する() {
        let a = mount_session(MountFixture::Default).unwrap();
        let b = mount_session(MountFixture::NoInput).unwrap();
        assert_ne!(a.ir().pack.id, b.ir().pack.id);
        assert_eq!(b.ir().nodes.len(), 3);
        let profile =
            fandhe_workshop_contract::profile::read_profile(b.ir().pack.profile.as_ref()).unwrap();
        assert_eq!(
            profile.input_field,
            fandhe_workshop_contract::profile::InputFieldMode::Hidden
        );
    }

    #[test]
    fn 章の移動の教材は_final_が3か所と開始点4件で読める() {
        assert_eq!(
            mount_fixture_from_search("?fixture=ch0-3-chapters"),
            MountFixture::Ch03Chapters
        );
        assert_eq!(
            mount_fixture_from_search("?fixture=ch0-3-chapters&x=1"),
            MountFixture::Default
        );
        let ir = load_ir(&ch03_chapters_ir_bytes().unwrap()).unwrap();
        let finals: Vec<&str> = ir
            .nodes
            .iter()
            .filter(|n| n.step.is_final)
            .map(|n| n.uid.as_str())
            .collect();
        assert!(finals.contains(&"main/st-0-2c") && finals.contains(&"main/st-1-6"));
        assert!(finals.contains(&"main/st-2-8"));
        assert_eq!(ir.starts.len(), 4);
        assert!(mount_session(MountFixture::Ch03Chapters).is_some());
    }

    #[test]
    fn 絞った教材は範囲外の章のノードを持ち開始点には含めない() {
        for (q, f) in [
            ("?fixture=ch0-3-trial", MountFixture::Ch03Trial),
            ("?fixture=ch0-4-full", MountFixture::Ch04Full),
        ] {
            assert_eq!(mount_fixture_from_search(q), f);
            assert_eq!(
                mount_fixture_from_search(&format!("{q}&x=1")),
                MountFixture::Default
            );
            assert!(mount_session(f).is_some());
        }
        let trial = load_ir(&ch03_trial_ir_bytes(false).unwrap()).unwrap();
        let full = load_ir(&ch03_trial_ir_bytes(true).unwrap()).unwrap();
        for ir in [&trial, &full] {
            assert!(ir
                .nodes
                .iter()
                .any(|n| n.uid == OUT_OF_RANGE_NODE && n.unit == "ch-4"));
        }
        assert_eq!(trial.starts.len(), 4);
        assert!(!trial.starts.iter().any(|s| s.node == OUT_OF_RANGE_NODE));
        assert_eq!(full.starts.len(), 5);
        assert!(full.starts.iter().any(|s| s.id == OUT_OF_RANGE_START_ID));
        assert_ne!(trial.pack.id, full.pack.id);
        assert_ne!(trial.pack.id, "ch0-3-chapters");
    }

    #[test]
    fn 完全一致のクエリだけが_ch0_3_の教材を選ぶ() {
        assert_eq!(
            mount_fixture_from_search("?fixture=ch0-3"),
            MountFixture::Ch03
        );
        for other in [
            "?fixture=ch0-3&x=1",
            "?x=1&fixture=ch0-3",
            "?fixture=ch0-3x",
            "?fixture=Ch0-3",
            "?fixture=ch0_3",
            "fixture=ch0-3",
        ] {
            assert_eq!(
                mount_fixture_from_search(other),
                MountFixture::Default,
                "{other}"
            );
        }
    }

    #[test]
    fn 通しの教材は鍵が別で_20_step_を持ち先頭の_step_から始まる() {
        let a = mount_session(MountFixture::Default).unwrap();
        let b = mount_session(MountFixture::Ch03).unwrap();
        assert_ne!(a.ir().pack.id, b.ir().pack.id);
        assert_eq!(b.ir().nodes.len(), 20);
        let s = b.state();
        let uid = s.run.current.as_ref().unwrap();
        assert_eq!(uid.0, "main/st-0-1");
        assert_eq!(s.run.slices[&uid.root()].status.len(), 1);
        assert_eq!(s.entry.as_ref().unwrap().run.current.as_ref(), Some(uid));
    }

    #[test]
    fn 完全一致のクエリだけが未来の_ir_の教材を選ぶ() {
        assert_eq!(
            mount_fixture_from_search("?fixture=future-ir-format"),
            MountFixture::FutureFormat
        );
        assert_eq!(
            mount_fixture_from_search("?fixture=future-ir-field"),
            MountFixture::FutureField
        );
        for other in [
            "?fixture=future-ir-format&x=1",
            "?x=1&fixture=future-ir-field",
            "?fixture=future-ir-format2",
            "?fixture=Future-Ir-Field",
            "fixture=future-ir-field",
        ] {
            assert_eq!(
                mount_fixture_from_search(other),
                MountFixture::Default,
                "{other}"
            );
        }
    }

    #[test]
    fn 未来の_ir_は既定と同じ鍵で未知の欄を持ち断られる() {
        let default_id = mount_session(MountFixture::Default)
            .unwrap()
            .ir()
            .pack
            .id
            .clone();
        for kind in [FutureIr::Format, FutureIr::Field] {
            let bytes = future_ir_bytes(kind).unwrap();
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value["pack"]["id"], default_id.as_str());
            assert_eq!(value["future_v3_x"], FUTURE_SENTINEL);
        }
        assert_eq!(
            mount_refusal(MountFixture::FutureFormat),
            Some(IrLoadError::FormatVersion(3))
        );
        assert!(matches!(
            mount_refusal(MountFixture::FutureField),
            Some(IrLoadError::Json(_))
        ));
        assert_eq!(mount_refusal(MountFixture::Default), None);
        assert!(mount_session(MountFixture::FutureFormat).is_none());
        assert!(mount_session(MountFixture::FutureField).is_none());
    }

    #[test]
    fn 完全一致のクエリだけが章の教材を選ぶ() {
        assert_eq!(
            mount_fixture_from_search("?fixture=chapter-a"),
            MountFixture::ChapterA
        );
        assert_eq!(
            mount_fixture_from_search("?fixture=chapter-b"),
            MountFixture::ChapterB
        );
        for other in [
            "?fixture=chapter-a&x=1",
            "?x=1&fixture=chapter-b",
            "?fixture=chapter-a2",
            "?fixture=Chapter-B",
            "?fixture=chapter-",
            "fixture=chapter-a",
        ] {
            assert_eq!(
                mount_fixture_from_search(other),
                MountFixture::Default,
                "{other}"
            );
        }
    }

    #[test]
    fn 章の教材は同じ教材の_id_で開始点と内容のハッシュと鍵が違う() {
        use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey};
        let a = mount_session(MountFixture::ChapterA).unwrap();
        let b = mount_session(MountFixture::ChapterB).unwrap();
        let d = mount_session(MountFixture::Default).unwrap();
        assert_eq!(a.ir().pack.id, CHAPTER_PACK_ID);
        assert_eq!(a.ir().pack.id, b.ir().pack.id);
        assert_ne!(a.ir().starts[0].id, b.ir().starts[0].id);
        assert_ne!(
            fandhe_workshop_contract::content_hash::content_hash(a.ir()).unwrap(),
            fandhe_workshop_contract::content_hash::content_hash(b.ir()).unwrap()
        );
        assert_eq!(a.state().run.current.as_ref().unwrap().0, "main/st-1");
        assert_eq!(b.state().run.current.as_ref().unwrap().0, "main/st-4");
        let scope = DeliveryScope::named("site");
        let ka = HistoryKey::from_ir(scope.clone(), a.ir()).unwrap().render();
        let kb = HistoryKey::from_ir(scope.clone(), b.ir()).unwrap().render();
        let kd = HistoryKey::from_ir(scope.clone(), d.ir()).unwrap().render();
        let prefix = fandhe_workshop_contract::storage::pack_prefix(&scope, CHAPTER_PACK_ID);
        assert_ne!(ka, kb);
        assert_ne!(ka, kd);
        assert_ne!(kb, kd);
        assert!(ka.starts_with(&prefix) && kb.starts_with(&prefix));
    }

    #[test]
    fn 完全一致のクエリだけが版を変えた章_a_を選ぶ() {
        assert_eq!(
            mount_fixture_from_search("?fixture=chapter-a-next"),
            MountFixture::ChapterANext
        );
        for other in [
            "?fixture=chapter-a-next&x=1",
            "?fixture=chapter-a-nex",
            "?fixture=Chapter-a-next",
            "fixture=chapter-a-next",
        ] {
            assert_eq!(
                mount_fixture_from_search(other),
                MountFixture::Default,
                "{other}"
            );
        }
    }

    #[test]
    fn 版を変えた章_a_は鍵が同じで版と内容のハッシュが違う() {
        use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey};
        let a = mount_session(MountFixture::ChapterA).unwrap();
        let n = mount_session(MountFixture::ChapterANext).unwrap();
        assert_ne!(a.ir().pack.version, n.ir().pack.version);
        assert_ne!(a.ir().pack.title, n.ir().pack.title);
        assert_ne!(
            fandhe_workshop_contract::content_hash::content_hash(a.ir()).unwrap(),
            fandhe_workshop_contract::content_hash::content_hash(n.ir()).unwrap()
        );
        let scope = DeliveryScope::named("site");
        assert_eq!(
            HistoryKey::from_ir(scope.clone(), a.ir()).unwrap().render(),
            HistoryKey::from_ir(scope, n.ir()).unwrap().render()
        );
    }

    #[test]
    fn 既定の教材の開始状態は試料の節点のままである() {
        let a = mount_session(MountFixture::Default).unwrap();
        let uid = a.state().run.current.as_ref().unwrap().clone();
        assert_eq!(uid.0, SAMPLE_START);
    }

    #[test]
    fn 選んだ教材は鍵が別で_8_step_を持つ() {
        let a = mount_session(MountFixture::Default).unwrap();
        let b = mount_session(MountFixture::ReloadPoints).unwrap();
        assert_ne!(a.ir().pack.id, b.ir().pack.id);
        assert_eq!(b.ir().nodes.len(), 8);
    }

    fn large_text(kind: LargeKind) -> String {
        let env = large_loadable_envelope(kind).expect("large envelope");
        String::from_utf8(fandhe_workshop_core::history_export::export_bytes(&env).unwrap())
            .unwrap()
    }

    #[test]
    fn 大きい履歴は保存の上限の内側で_9_5mib_以上になる() {
        use fandhe_workshop_core::history_limit::check_save;
        for kind in [LargeKind::Plain, LargeKind::Dense, LargeKind::Marker] {
            let env = large_loadable_envelope(kind).unwrap();
            let bytes = fandhe_workshop_core::history_export::export_bytes(&env).unwrap();
            assert!(
                (9_961_472..=10_485_760).contains(&bytes.len()),
                "{kind:?}: {}",
                bytes.len()
            );
            assert!(env.ops.len() <= 20_000);
            assert!(check_save(bytes.len(), env.ops.len()).is_ok(), "{kind:?}");
        }
    }

    #[test]
    fn plain_と_dense_は検出が_0_件で_marker_は_1_形だけ() {
        use fandhe_workshop_core::secret_scan::scan_json_text;
        assert!(scan_json_text(&large_text(LargeKind::Plain)).is_empty());
        assert!(scan_json_text(&large_text(LargeKind::Dense)).is_empty());
        // 同じ値が `ops` と状態の写しの 2 か所に入るため、検出は 1 件とは限らない。形は 1 つだけである。
        let found = scan_json_text(&large_text(LargeKind::Marker));
        assert!(!found.is_empty());
        assert_eq!(
            fandhe_workshop_core::secret_scan::shapes_of(&found),
            vec![fandhe_workshop_core::secret_scan::SecretShape::SlackToken]
        );
    }

    #[test]
    fn 大きい履歴の種類は完全一致の名前だけを受ける() {
        assert_eq!(LargeKind::from_name("plain"), Some(LargeKind::Plain));
        assert_eq!(LargeKind::from_name("dense"), Some(LargeKind::Dense));
        assert_eq!(LargeKind::from_name("marker"), Some(LargeKind::Marker));
        for other in ["", "Plain", "plain ", "marker2", "x"] {
            assert_eq!(LargeKind::from_name(other), None, "{other:?}");
        }
    }

    #[test]
    fn start_の対だけは取り除いて完全一致で教材を選ぶ() {
        for (q, want) in [
            ("?fixture=multi-start", MountFixture::MultiStart),
            ("?fixture=multi-start&start=s-4", MountFixture::MultiStart),
            ("?start=s-4&fixture=multi-start", MountFixture::MultiStart),
            (
                "?fixture=multi-start&start=a&start=b",
                MountFixture::MultiStart,
            ),
            ("?fixture=chapter-a&start=s-1", MountFixture::ChapterA),
            ("?fixture=ch0-3-links", MountFixture::Ch03Links),
            ("?fixture=ch0-3-links&start=s-2-1", MountFixture::Ch03Links),
            ("?start=s-2-1&fixture=ch0-3-links", MountFixture::Ch03Links),
            (
                "?fixture=ch0-3-links&start=s-2-1&x=1",
                MountFixture::Default,
            ),
            ("?start=s-4", MountFixture::Default),
            ("?start=", MountFixture::Default),
            // start 以外の余分なキー・大文字違いは既定に倒す（既存の否定と同じ）。
            ("?fixture=multi-start&start=s-4&x=1", MountFixture::Default),
            ("?x=1&fixture=multi-start&start=s-4", MountFixture::Default),
            ("?fixture=multi-start&Start=s-4", MountFixture::Default),
            ("?fixture=multi-start&xstart=s-4", MountFixture::Default),
            ("fixture=multi-start&start=s-4", MountFixture::Default),
        ] {
            assert_eq!(mount_fixture_from_search(q), want, "{q}");
        }
    }

    fn current_node(s: &Session) -> String {
        s.state().run.current.as_ref().map(|u| u.0.clone()).unwrap()
    }

    #[test]
    fn 開始点の指定で開く_step_が変わる() {
        let (first, id) = mount_session_at(MountFixture::MultiStart, None)
            .unwrap()
            .unwrap();
        assert_eq!(
            (id.as_str(), current_node(&first).as_str()),
            ("s-1", "main/st-1")
        );
        for (want_id, want_node) in MULTI_START_POINTS {
            let (s, id) = mount_session_at(MountFixture::MultiStart, Some(want_id))
                .unwrap()
                .unwrap();
            assert_eq!(id, want_id);
            assert_eq!(current_node(&s), want_node);
        }
    }

    #[test]
    fn 一致しない指定は開かずに断る() {
        for v in ["", "S-4", "s-4 ", "s-", "../x", "unknown", "\u{0}"] {
            assert_eq!(
                mount_session_at(MountFixture::MultiStart, Some(v)).err(),
                Some(StartRefusal::Unknown),
                "{v:?}"
            );
        }
        // 開始点が 1 件の既存の教材は、その ID 以外を断る。
        assert!(mount_session_at(MountFixture::ReloadPoints, Some("s-1")).is_ok());
        assert!(mount_session_at(MountFixture::ReloadPoints, Some("s-4")).is_err());
    }

    #[test]
    fn ホストは選んだ開始点で鍵を作り_断ったら_session_も鍵も持たない() {
        use crate::host::EngineHost;
        use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey};
        let scope = || DeliveryScope::named("scope-x");
        let none = EngineHost::for_mount_with(MountFixture::MultiStart, None);
        let ir = mount_session_at(MountFixture::MultiStart, None)
            .unwrap()
            .unwrap()
            .0
            .ir()
            .clone();
        // 指定なしの鍵は、これまでの `from_ir`（先頭の開始点）と同じ文字列である。
        assert_eq!(
            none.history_key(scope()).unwrap().render(),
            HistoryKey::from_ir(scope(), &ir).unwrap().render()
        );
        let s4 = EngineHost::for_mount_with(MountFixture::MultiStart, Some("s-4"));
        let (k1, k4) = (
            none.history_key(scope()).unwrap().render(),
            s4.history_key(scope()).unwrap().render(),
        );
        assert_ne!(k1, k4);
        assert_eq!(
            k4,
            HistoryKey::for_pack(scope(), MULTI_START_PACK_ID, "s-4").render()
        );
        let bad = EngineHost::for_mount_with(MountFixture::MultiStart, Some("nope"));
        assert!(bad.history_key(scope()).is_none());
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&bad.engine_snapshot_json()).unwrap()
                ["irLoaded"],
            false
        );
        assert_eq!(bad.ir_refusal(), Some("start_unknown"));
        assert!(bad.history_key(scope()).is_none());
    }

    /// 開始点ごとに `starts` の先頭を入れ替えた IR の写し（旧方式。PoC-10 の `make-start-irs.mjs` の模写）を作る。
    fn per_start_copy(id: &str) -> Vec<u8> {
        let mut value: serde_json::Value =
            serde_json::from_slice(&multi_start_ir_bytes().unwrap()).unwrap();
        let starts = value["starts"].as_array_mut().unwrap();
        let at = starts.iter().position(|s| s["id"] == id).unwrap();
        starts.swap(0, at);
        serde_json::to_vec(&value).unwrap()
    }

    #[test]
    fn 同じ_ir_はどの開始点から開いても内容のハッシュが同じ() {
        use fandhe_workshop_contract::content_hash::content_hash;
        let want = content_hash(&load_ir(&multi_start_ir_bytes().unwrap()).unwrap()).unwrap();
        let mut opened = std::collections::BTreeSet::new();
        for id in [None, Some("s-1"), Some("s-4"), Some("s-7")] {
            let (session, got_id) = mount_session_at(MountFixture::MultiStart, id)
                .unwrap()
                .unwrap();
            let got = content_hash(session.ir()).unwrap();
            assert_eq!(
                got, want,
                "{id:?}: content_hash is the same for every start"
            );
            opened.insert((got_id, current_node(&session)));
        }
        // 空振り防止: 指定なしは s-1 と同じ開始点で、開いた開始点と節点の組は開始点ごとに違う。
        assert_eq!(opened.len(), MULTI_START_POINTS.len());
    }

    #[test]
    fn 開始点ごとに先頭を入れ替えた写しは内容のハッシュが割れる() {
        use fandhe_workshop_contract::content_hash::content_hash;
        let hash_of = |bytes: &[u8]| content_hash(&load_ir(bytes).unwrap()).unwrap();
        let mut hashes = vec![hash_of(&multi_start_ir_bytes().unwrap())];
        for (id, _) in MULTI_START_POINTS.into_iter().skip(1) {
            hashes.push(hash_of(&per_start_copy(id)));
        }
        // 先頭の入れ替えは `starts` の順を変えるだけだが、`starts` は内容のハッシュに含まれるので、写しごとに違う値になる。
        let distinct: std::collections::BTreeSet<_> =
            hashes.iter().map(|h| h.as_str().to_owned()).collect();
        assert_eq!(distinct.len(), hashes.len());
    }

    #[test]
    fn 章の開始点_10_件の教材は各開始点でその節点で開き_鍵が分かれる() {
        use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey};
        let ir = load_ir(&ch03_links_ir_bytes().unwrap()).unwrap();
        assert_eq!(ir.pack.id, CH03_LINKS_PACK_ID);
        assert_eq!(ir.starts.len(), 10);
        let (first, first_id) = mount_session_at(MountFixture::Ch03Links, None)
            .unwrap()
            .unwrap();
        assert_eq!(
            (first_id.as_str(), current_node(&first).as_str()),
            CH03_LINK_POINTS[0]
        );
        let mut keys = std::collections::BTreeSet::new();
        for (id, node) in CH03_LINK_POINTS {
            assert!(ir.nodes.iter().any(|n| n.uid == node), "{id}");
            let (s, got) = mount_session_at(MountFixture::Ch03Links, Some(id))
                .unwrap()
                .unwrap();
            assert_eq!((got.as_str(), current_node(&s).as_str()), (id, node));
            keys.insert(
                HistoryKey::for_pack(DeliveryScope::named("scope-x"), CH03_LINKS_PACK_ID, id)
                    .render(),
            );
        }
        assert_eq!(keys.len(), CH03_LINK_POINTS.len());
        assert_eq!(
            mount_session_at(MountFixture::Ch03Links, Some("nope")).err(),
            Some(StartRefusal::Unknown)
        );
        // 対照: 開始点 1 件の既存の教材は、同じ ID を断る。
        assert_eq!(
            mount_session_at(MountFixture::Ch03, Some("s-2-1")).err(),
            Some(StartRefusal::Unknown)
        );
    }

    #[test]
    fn 表示名の対照の教材は_answer_の_label_だけを外し_完全一致でだけ選ばれる() {
        for (q, want) in [
            ("?fixture=ch0-3-nolabel", MountFixture::Ch03NoLabel),
            (
                "?fixture=ch0-3-nolabel&start=s-1",
                MountFixture::Ch03NoLabel,
            ),
            ("?fixture=ch0-3-nolabel&x=1", MountFixture::Default),
            ("?fixture=ch0-3-nolabel2", MountFixture::Default),
        ] {
            assert_eq!(mount_fixture_from_search(q), want, "{q}");
        }
        let base = load_ir(CH03_IR).unwrap();
        let ir = load_ir(&ch03_nolabel_ir_bytes().unwrap()).unwrap();
        assert_eq!(ir.pack.id, CH03_NOLABEL_PACK_ID);
        assert_ne!(ir.pack.id, base.pack.id);
        assert_eq!(ir.nodes.len(), base.nodes.len());
        assert!(mount_session(MountFixture::Ch03NoLabel).is_some());
        let raw: serde_json::Value =
            serde_json::from_slice(&ch03_nolabel_ir_bytes().unwrap()).unwrap();
        let orig: serde_json::Value = serde_json::from_slice(CH03_IR).unwrap();
        let (mut answers, mut input_labels) = (0, 0);
        for (n, o) in raw["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .zip(orig["nodes"].as_array().unwrap())
        {
            let confirm = &n["step"]["confirm"];
            if confirm["kind"] == "answer" {
                let ocf = &o["step"]["confirm"];
                assert_eq!(confirm["on_wrong"], ocf["on_wrong"]);
                for (a, b) in confirm["options"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .zip(ocf["options"].as_array().unwrap())
                {
                    answers += 1;
                    assert!(a.get("label").is_none());
                    assert_eq!(a["id"], b["id"]);
                    assert_eq!(a["correct"], b["correct"]);
                }
            }
            // 入力方式 `choice` の `label` は残る。
            if n["step"]["input"]["kind"] == "choice" {
                assert_eq!(n["step"]["input"], o["step"]["input"]);
                input_labels += 1;
            }
        }
        assert!(answers > 0 && input_labels > 0);
    }

    #[test]
    fn 直す候補の教材は_st_2_6_の宣言と開始点を足し_完全一致でだけ選ばれる() {
        for (q, want) in [
            ("?fixture=ch0-3-revise", MountFixture::Ch03Revise),
            (
                "?fixture=ch0-3-revise&start=s-2-6",
                MountFixture::Ch03Revise,
            ),
            ("?fixture=ch0-3-revise&x=1", MountFixture::Default),
            ("?fixture=ch0-3-revise2", MountFixture::Default),
        ] {
            assert_eq!(mount_fixture_from_search(q), want, "{q}");
        }
        let base = load_ir(CH03_IR).unwrap();
        let ir = load_ir(&ch03_revise_ir_bytes().unwrap()).unwrap();
        assert_eq!(ir.pack.id, CH03_REVISE_PACK_ID);
        assert_ne!(ir.pack.id, base.pack.id);
        assert_eq!(ir.nodes.len(), base.nodes.len());
        assert_eq!(ir.starts.len(), base.starts.len() + 1);
        assert!(mount_session_at(MountFixture::Ch03Revise, Some("s-2-6")).is_ok());
        let raw: serde_json::Value =
            serde_json::from_slice(&ch03_revise_ir_bytes().unwrap()).unwrap();
        let node = raw["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["uid"] == "main/st-2-6")
            .unwrap();
        assert_eq!(
            node["step"]["input"]["revise"]["candidates"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }
}
