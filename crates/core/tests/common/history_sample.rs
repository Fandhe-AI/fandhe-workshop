//! 履歴の包みのサンプル（`testdata/contract/history/*.json`、TASK-309 3/3）を core の型で読み、
//! 書き出し→読み込みの往復を照合する部品。
//!
//! 呼び出し元は適合試験 `history_envelope_conformance.rs` と、その変異注入（`mutations/SAVE2-1.mutation`）。
//! 往復の照合 [`check_roundtrip`] は書き出しと読み込みを関数で受け取る。実装の組み立て（`history_envelope`、TASK-315 2/2）は
//! 試験 `history_envelope_build.rs` が同じ部品で照合する。実装の書き出し・読み込み（TASK-330）が出来たら、
//! 同じ照合へ実装の関数を渡して使う。
//! 参照の書き出し・読み込みは serde の往復そのもので、変異はその読み込みの 1 行だけを壊す。
//! 試験の部品なので、読めない場合は理由を添えて panic する。

use std::path::{Component, Path, PathBuf};

use fandhe_workshop_contract::history::HistoryEnvelope;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::session::SessionState;

/// core の `Op`・`SessionState` を束縛した履歴の包み。
pub type H = HistoryEnvelope<Op, SessionState>;

/// サンプルの名前（拡張子なし）。
pub const SAMPLES: [&str; 3] = ["empty", "two-rounds", "draft-rejected"];

/// サンプルのファイルの絶対パス。名前は固定の語だけを受け、`testdata/` の外へ出さない。
pub fn sample_path(name: &str) -> PathBuf {
    let rel = Path::new(name);
    assert!(
        rel.components().all(|c| matches!(c, Component::Normal(_))),
        "unsafe sample name: {name}"
    );
    super::contract_sample::testdata_dir()
        .join("contract/history")
        .join(format!("{name}.json"))
}

/// サンプルのバイト列（golden）。
pub fn read_bytes(name: &str) -> Vec<u8> {
    std::fs::read(sample_path(name)).expect("history sample must be readable")
}

/// サンプルを読む。形が合わなければ panic する。
pub fn load(name: &str) -> H {
    reference_import(&read_bytes(name)).expect("history sample must match the core types")
}

/// 参照の書き出し。整形した JSON と末尾の改行 1 つ（サンプルのファイルと同じ形）。
pub fn reference_export(h: &H) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(h).expect("envelope must serialize");
    bytes.push(b'\n');
    bytes
}

/// 参照の読み込み。変異注入はこの関数の逆直列化の 1 行だけを書き換える。
pub fn reference_import(bytes: &[u8]) -> Result<H, serde_json::Error> {
    let envelope: H = serde_json::from_slice(bytes)?;
    Ok(envelope)
}

/// ログの位置の不変条件を確かめる。破れていれば理由を返す。
///
/// 1. 添字 i の `CheckpointSaved` の `log_len` は i（保存のイベントを足す前の長さ）
/// 2. 表の `checkpoints[id].log_len` は、その id の `CheckpointSaved` の `log_len` と一致する
/// 3. `Restored{id, log_len}` は、`log[log_len]` がその id の `CheckpointSaved` を指す
/// 4. 入口の写しと各チェックポイントの `log_len` はログの長さ以下
/// 5. `next_checkpoint` は既存のどの id より大きい
pub fn check_log_positions(s: &SessionState) -> Result<(), String> {
    for (i, e) in s.log.iter().enumerate() {
        match e {
            Event::CheckpointSaved { id, log_len, .. } => {
                if *log_len as usize != i {
                    return Err(format!("log[{i}] CheckpointSaved.log_len is {log_len}"));
                }
                if let Some(cp) = s.checkpoints.get(id) {
                    if cp.log_len != *log_len {
                        return Err(format!("checkpoint {id:?} log_len differs from its event"));
                    }
                }
            }
            Event::Restored { id, log_len, .. } => match s.log.get(*log_len as usize) {
                Some(Event::CheckpointSaved { id: saved, .. }) if saved == id => {}
                _ => {
                    return Err(format!(
                        "log[{i}] Restored.log_len {log_len} does not point at checkpoint {id:?}"
                    ))
                }
            },
            _ => {}
        }
    }
    if let Some(entry) = &s.entry {
        if entry.log_len as usize > s.log.len() {
            return Err("entry.log_len exceeds the log length".into());
        }
    }
    for (id, cp) in &s.checkpoints {
        if cp.log_len as usize > s.log.len() {
            return Err(format!("checkpoint {id:?} log_len exceeds the log length"));
        }
        if *id >= s.next_checkpoint {
            return Err(format!("next_checkpoint is not above {id:?}"));
        }
    }
    Ok(())
}

/// 書き出し→読み込みの往復で、ログの並びと位置が変わらないことを確かめる（SAVE-2）。
///
/// 書き出しと読み込みは関数で受け取る。往復の前後で、包み全体・ログ・位置が一致し、
/// 位置の不変条件が保たれ、書き出し直しても同じバイトになることを見る。
pub fn check_roundtrip<X, I>(export: X, import: I, original: &H) -> Result<(), String>
where
    X: Fn(&H) -> Vec<u8>,
    I: Fn(&[u8]) -> Result<H, String>,
{
    let bytes = export(original);
    let back = import(&bytes)?;
    if back.snapshot.log != original.snapshot.log {
        return Err("log order changed by the roundtrip".into());
    }
    if back.snapshot.checkpoints != original.snapshot.checkpoints {
        return Err("checkpoint positions changed by the roundtrip".into());
    }
    if back.snapshot.entry != original.snapshot.entry {
        return Err("entry copy changed by the roundtrip".into());
    }
    if &back != original {
        return Err("envelope changed by the roundtrip".into());
    }
    check_log_positions(&back.snapshot)?;
    if export(&back) != bytes {
        return Err("re-export differs from the first export".into());
    }
    Ok(())
}
