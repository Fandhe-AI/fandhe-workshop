//! 記録つきの実行環境 `RecordedSession` の試験（TASK-319 1/3、SAVE-2・SAVE-28）。
//!
//! 記録の規則（結果にかかわらず記録・畳み・空白だけの除外）と、`ops` が 0 件なら保存用の包みを作らないこと、
//! 作った包みが読み込みの段 1〜8 で開けて適用し直しが写しと一致することを確かめる。

mod common;

use common::contract_sample::{load, testdata_dir};
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::load_history;
use fandhe_workshop_core::history_open::{Opened, OpeningCaps};
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::version_record::record_versions;
use fandhe_workshop_core::{load_ir, Session, SessionState};

fn fixture() -> (Ir, SessionState) {
    let sample = load();
    let bytes = std::fs::read(testdata_dir().join(&sample.ir)).expect("read ir");
    (load_ir(&bytes).expect("load ir"), sample.initial)
}

fn recorded() -> (RecordedSession, Ir, SessionState) {
    let (ir, initial) = fixture();
    let rs = RecordedSession::new(Session::from_state(ir.clone(), initial.clone()), "s-1");
    (rs, ir, initial)
}

#[test]
fn 操作が_0_件なら保存用の包みを作らない() {
    let (rs, _, _) = recorded();
    assert!(rs.ops().is_empty());
    assert!(rs.envelope_for_save().is_none());
}

#[test]
fn 下書きが_1_件あれば包みを作る() {
    let (mut rs, _, _) = recorded();
    rs.draft("a".to_owned());
    assert!(rs.envelope_for_save().is_some());
}

#[test]
fn 開始点が教材に無ければ包みを作らない() {
    let (ir, initial) = fixture();
    let mut rs = RecordedSession::new(Session::from_state(ir, initial), "no-such-start");
    rs.draft("a".to_owned());
    assert!(rs.envelope_for_save().is_none());
}

#[test]
fn 記録の規則は記録器と同じ() {
    let (mut rs, _, _) = recorded();
    rs.draft("a".to_owned());
    rs.draft("ab".to_owned());
    assert!(rs.submit("/", "   ".to_owned()).is_empty());
    rs.submit("/", "ab".to_owned());
    rs.apply(Op::Control(Control::Stop));
    rs.reload();
    let ops = rs.ops();
    assert_eq!(
        ops.len(),
        4,
        "連続する下書きは畳み、空白だけの送信は記録しない"
    );
    assert!(matches!(&ops[0], HistoryOp::Draft(t) if t == "ab"));
    assert!(matches!(&ops[1], HistoryOp::Submit(t) if t == "ab"));
    assert!(matches!(&ops[2], HistoryOp::Engine(_)));
    assert!(matches!(&ops[3], HistoryOp::Reload));
}

#[test]
fn 作った包みは読み込みで開けて適用し直しが写しと一致する() {
    let (mut rs, ir, initial) = recorded();
    rs.draft("あ".to_owned());
    rs.submit("/", "/help".to_owned());
    rs.draft("続き".to_owned());
    rs.reload();
    let env = rs.envelope_for_save().expect("envelope");
    let bytes = export_bytes(&env).expect("export");
    let opening = record_versions(&ir, &content_hash(&ir).expect("hash")).expect("versions");
    let opened = load_history(&bytes, &ir, &opening, &OpeningCaps::current(), initial);
    let Opened::Open { state } = opened else {
        panic!("must open: {:?}", opened.verdict());
    };
    assert_eq!(&state, rs.session().state());
}
