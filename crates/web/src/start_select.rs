//! 直接リンク `?start=<開始点 ID>` の値を、教材（IR）の `starts` と照らして開始点を選ぶ純関数（TASK-109 1/2。DLV-9・SAFE-33・D-68）。
//!
//! 呼び出し元は `host::EngineHost` の構築（`for_mount_with`）で、値の出所は `static/app.js` が `start` だけを読んで
//! `mount_app` の引数として渡す文字列である。IR の場所は URL から受けず（固定の `./ir/main.json`）、値は `starts` の
//! ID と比べるだけで、取得先・パスの組み立てには使わない。
//!
//! 照合はバイト単位の完全一致だけで、大文字小文字・前後の空白・正規化・復号をしない（前方一致も許さない）。
//! 一致しない値は開かずに断る。断りの理由は固定の ASCII のコードだけで、入力値・ID を持たない
//! （`Debug`・`Display` にも出ない。SAFE-7・SAFE-33）。

use fandhe_workshop_contract::ir::{Ir, Start};
use fandhe_workshop_core::{load_ir, IrLoadError, Session};

/// 同梱の IR の受信で読むバイト数の上限（`MAX_IR_BYTES + 1`）。`app.js` が応答の本体をここまでだけ読み、超えたら読み取りを
/// 中止する（メモリ確保の前に止める）。上限を 1 超える長さを渡すので、大きさの断りは `load_ir` が出す。定数を JS に二重に持たない（wasm への公開は `glue::ir_read_limit`）。
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "20 MiB + 1 は f64 で正確に表せる"
)]
pub fn ir_read_limit() -> f64 {
    (fandhe_workshop_contract::limits::MAX_IR_BYTES + 1) as f64
}

/// 開始点を選べなかった理由。固定のコードだけを持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartRefusal {
    /// 指定された値が `starts` のどの ID とも一致しない（空・複数指定を含む）。
    Unknown,
    /// 指定が無く、教材に開始点が 1 件も無い。
    Missing,
}

/// 開始点の断りのコードの接頭辞。通知文が教材の読み込みの断りと文言を分けるために使う。
pub const START_CODE_PREFIX: &str = "start_";

impl StartRefusal {
    /// 固定の ASCII の理由コード。
    pub fn reason_code(self) -> &'static str {
        match self {
            StartRefusal::Unknown => "start_unknown",
            StartRefusal::Missing => "start_missing",
        }
    }
}

/// `start` の指定（`None` は指定なし）から開始点を選ぶ。
///
/// 指定なしは `starts` の先頭（DLV-9）。指定ありは ID が完全一致する要素だけで、無ければ [`StartRefusal::Unknown`]。
///
/// # Errors
///
/// 一致する開始点が無いとき、または指定なしで `starts` が空のとき。
pub fn select_start<'a>(ir: &'a Ir, requested: Option<&str>) -> Result<&'a Start, StartRefusal> {
    match requested {
        None => ir.starts.first().ok_or(StartRefusal::Missing),
        Some(id) => ir
            .starts
            .iter()
            .find(|s| s.id == id)
            .ok_or(StartRefusal::Unknown),
    }
}

/// 同梱の IR（`./ir/main.json` のバイト列）を開けなかった理由。どちらも固定の ASCII のコードだけを持つ。
#[derive(Debug)]
pub enum BundledIrRefusal {
    /// IR を読めない（空・不正な JSON・上限超え・版の違いを含む）。
    Load(IrLoadError),
    /// 開始点を選べない。
    Start(StartRefusal),
}

/// 同梱の IR と `start` の指定から、開始点に入った状態の実行環境と選んだ開始点の ID を作る（配信する設定の起動。TASK-112 2/2）。
///
/// 呼び出し元は `host::EngineHost::for_mount`（`persist::mount_with_history` 経由）で、バイト列は `static/app.js` が固定の
/// `./ir/main.json` から取得して渡す。バイト列は信頼できない入力として `load_ir`（大きさ・深さの上限・版・参照の検査）を必ず通す。
/// 開始の操作（`Control::Begin`。TASK-217・TASK-294）が未実装の間の暫定で、開始点の節点に入った直後の状態
/// は core の `Session::entered_at` が組み立てる（表示層は状態を組み立てない。`no_progress`）。開始時の効果・案内の水準の決定は
/// 担当のタスクが入れる（試験の設定の `test_fixture::session_at` と同じ状態になる）。
///
/// # Errors
///
/// IR を読めないとき、開始点を選べないとき、選んだ開始点の節点が教材に無いとき。
pub fn open_bundled_ir(
    bytes: &[u8],
    requested: Option<&str>,
) -> Result<(Session, String), BundledIrRefusal> {
    let ir = load_ir(bytes).map_err(BundledIrRefusal::Load)?;
    let chosen = select_start(&ir, requested).map_err(BundledIrRefusal::Start)?;
    let (id, node) = (chosen.id.clone(), chosen.node.clone());
    if !ir.nodes.iter().any(|n| n.uid == node) {
        return Err(BundledIrRefusal::Start(StartRefusal::Missing));
    }
    Ok((Session::entered_at(ir, &node), id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fandhe_workshop_core::step_status::StepStatus;

    const BASE: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/reload_points_ch2/ir.json"
    ));

    /// 指定した ID の開始点を持つ IR。
    fn ir_with_starts(ids: &[&str]) -> Ir {
        let mut v: serde_json::Value = serde_json::from_slice(BASE).unwrap();
        let starts: Vec<_> = ids
            .iter()
            .map(|id| serde_json::json!({ "id": id, "node": "main/st-1", "effects": [] }))
            .collect();
        v["starts"] = serde_json::Value::Array(starts);
        load_ir(&serde_json::to_vec(&v).unwrap()).unwrap()
    }

    #[test]
    fn 指定なしは先頭の開始点() {
        let ir = ir_with_starts(&["s-1", "s-4"]);
        assert_eq!(select_start(&ir, None).unwrap().id, "s-1");
    }

    #[test]
    fn 一致する開始点を選ぶ() {
        let ir = ir_with_starts(&["s-1", "s-4"]);
        assert_eq!(select_start(&ir, Some("s-4")).unwrap().id, "s-4");
        assert_eq!(select_start(&ir, Some("s-1")).unwrap().id, "s-1");
    }

    #[test]
    fn 完全一致でない値は断る() {
        let ir = ir_with_starts(&["s-1", "s-4"]);
        let long = "s".repeat(10_000);
        for v in [
            "",
            "../something",
            "s-1/../s-4",
            "S-1",
            " s-1",
            "s-1 ",
            "s-1x",
            "s-",
            "s",
            "%73-1",
            "s-1\n",
            "s-1\0",
            "unknown",
            long.as_str(),
        ] {
            assert_eq!(
                select_start(&ir, Some(v)).unwrap_err(),
                StartRefusal::Unknown,
                "{v:?}"
            );
        }
    }

    #[test]
    fn 開始点が無い教材は指定なしでも断る() {
        let ir = ir_with_starts(&[]);
        assert_eq!(select_start(&ir, None).unwrap_err(), StartRefusal::Missing);
        assert_eq!(
            select_start(&ir, Some("s-1")).unwrap_err(),
            StartRefusal::Unknown
        );
    }

    const CH03: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/ch0_3/ir.json"
    ));

    #[test]
    fn 同梱の_ir_は最初の開始点の節点に入って開く() {
        let (session, id) = open_bundled_ir(CH03, None).unwrap();
        assert_eq!(id, "s-0-1");
        let cur = session.state().run.current.clone().unwrap();
        assert_eq!(cur.0, "main/st-0-1");
        assert_eq!(session.state().run.generation.0, 1);
        assert_eq!(session.step_status(&cur), StepStatus::Entered);
        assert_eq!(
            session.state().entry.as_ref().unwrap().run,
            session.state().run
        );
        assert_eq!(open_bundled_ir(CH03, Some("s-0-1")).unwrap().1, "s-0-1");
    }

    #[test]
    fn 同梱の_ir_は一致しない開始点で断る() {
        for v in ["nope", "", "S-0-1", "s-0-1 ", "\u{0}"] {
            assert!(
                matches!(
                    open_bundled_ir(CH03, Some(v)),
                    Err(BundledIrRefusal::Start(StartRefusal::Unknown))
                ),
                "{v:?}"
            );
        }
    }

    #[test]
    fn 同梱の_ir_は読めない入力を断る() {
        let deep = "[".repeat(100_000);
        let big = vec![b' '; 64 * 1024 * 1024];
        for b in [&b""[..], b"{", b"null", b"[]", deep.as_bytes(), &big] {
            assert!(
                matches!(open_bundled_ir(b, None), Err(BundledIrRefusal::Load(_))),
                "len={}",
                b.len()
            );
        }
    }

    #[test]
    fn 断りは固定のコードだけで入力値を持たない() {
        let ir = ir_with_starts(&["s-1"]);
        let e = select_start(&ir, Some("SENTINEL-START-X")).unwrap_err();
        assert!(!format!("{e:?}").contains("SENTINEL"));
        assert_eq!(e.reason_code(), "start_unknown");
        assert_eq!(StartRefusal::Missing.reason_code(), "start_missing");
        assert!(e.reason_code().starts_with(START_CODE_PREFIX));
        assert!(e.reason_code().is_ascii());
    }
}
