//! core の `Op` を `HistoryOp` に束縛したときのワイヤの形の検査（TASK-309 1/3）。
//!
//! contract は core に依存しないため、束縛は使う側（ここ）で行う。包んでも `Op` の形が変わらないこと、
//! 下書き・送信・再読み込みと混ぜても順が保たれること、早送りなどの未知の操作を拒否することを確かめる。

use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::input::{Control, Input, Op};

type H = HistoryOp<Op>;

fn sample_ops() -> Vec<Op> {
    vec![
        Op::Input(Input::Command("ls".to_string())),
        Op::Input(Input::Cancel),
        Op::Tick(TickAddr {
            generation: Generation(2),
            step: StepUid("s1".to_string()),
            index: 0,
        }),
        Op::Control(Control::Save),
        Op::Control(Control::GoTo(StepUid("s2".to_string()))),
    ]
}

#[test]
fn wrapping_does_not_change_engine_op_wire_form() {
    for op in sample_ops() {
        let plain = serde_json::to_value(&op).unwrap();
        let wrapped = serde_json::to_value(H::Engine(op.clone())).unwrap();
        assert_eq!(plain, wrapped);
        let back: H = serde_json::from_value(plain).unwrap();
        assert_eq!(back, H::Engine(op));
    }
}

#[test]
fn mixed_sequence_roundtrips_in_order() {
    let mut seq: Vec<H> = Vec::new();
    for op in sample_ops() {
        seq.push(H::Draft("d".to_string()));
        seq.push(H::Engine(op));
        seq.push(H::Submit("s".to_string()));
        seq.push(H::Reload);
    }
    let s = serde_json::to_string(&seq).unwrap();
    assert_eq!(serde_json::from_str::<Vec<H>>(&s).unwrap(), seq);
}

#[test]
fn unknown_ops_are_rejected() {
    for bad in [
        r#"{"fast_forward":true}"#,
        r#"{"enter":"x"}"#,
        r#""fast_forward""#,
    ] {
        assert!(serde_json::from_str::<H>(bad).is_err(), "accepted: {bad}");
    }
}
