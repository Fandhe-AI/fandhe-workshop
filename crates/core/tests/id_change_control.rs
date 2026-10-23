//! 文言ではなく ID を変えると差が出ることの負の対照（TASK-218 3/3。EVT-2・P-7）。
//!
//! `text_independence.rs`（2/3）は「表示文字列が違っても、次の行動を文面の ID に写せば差 0」を主張する。
//! その比べ方が鈍く、何でも一致にしてしまうのでは主張が空になる。そこで拒否の文面の ID だけを付け替えた IR
//! （表示文字列は同一）に同じ入力列を流し、2/3 と同じ比べ方（`mask_display` 後の `compare_traces`）で差が
//! 出ることを確かめる。PoC-1 の (a) の前例（`defaults` の拒否の ID を変えると差が出る）に倣う。
//!
//! 誤った理由で通る対照にしないため、差が ID の文字列の置き換えだけに由来すること（エンジンが文面の未解決など
//! 別の経路に入った差でないこと）を、写す前の記録を直列化して `new` を `old` へ戻すと元と一致することで確かめる。
//! `load_ir` は拒否の参照の解決を検査しないので、この局所性の検査が要る。
//!
//! 扱わないこと: ヒント・選択肢・Step の ID の付け替え（選択肢と Step の ID は入力列・初期状態が参照するため
//! 対照として純粋にならない）、1,000 通りの性質テスト（負の対照は決定的な自動テストで足りる）。
//! 決定性は `determinism.rs`（1/3）、表示文字列の独立は `text_independence.rs`（2/3）の持ち分である。

mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::op_gen::{script_specs, Harness, OpSpec};
use common::text_table::{
    display_strings, judged_next_actions, load_value, mask_display, original_value, rejection_refs,
    rename_rejection_id, variant_value, Variant,
};
use common::trace::{compare_traces, trace, StepTrace};
use serde_json::Value;

const VARIANTS: [Variant; 3] = [Variant::Original, Variant::A, Variant::B];

/// 付け替え後の ID。元の IR・記録のどこにも現れない合成の値。
fn new_id(n: usize) -> String {
    format!("zz-renamed-{n}")
}

fn run(ir: &Value, specs: &[OpSpec], via_session: bool) -> Vec<StepTrace> {
    let (ir, init) = load_value(ir);
    if via_session {
        trace(Harness::via_session(&ir, init), specs)
    } else {
        trace(Harness::pure(&ir, init), specs)
    }
}

/// 元の IR の 37 操作で `Judged` に現れる拒否の文面の ID（重複なし・昇順）。
fn judged_ids() -> Vec<String> {
    let runs = run(&original_value(), &script_specs(), false);
    let ids: BTreeSet<String> = judged_next_actions(&runs)
        .into_iter()
        .map(|(_, id, _)| id)
        .collect();
    assert!(!ids.is_empty(), "the script must produce judged rejections");
    ids.into_iter().collect()
}

fn parts(t: &StepTrace) -> [String; 5] {
    [
        serde_json::to_string(&t.events),
        serde_json::to_string(&t.after),
        serde_json::to_string(&t.log_rows),
        serde_json::to_string(&t.marks),
        serde_json::to_string(&t.review),
    ]
    .map(|r| r.expect("serialize trace part"))
}

#[test]
fn renamed_ids_keep_display_strings() {
    let ids = judged_ids();
    for v in VARIANTS {
        let base = variant_value(v);
        let base_strings = display_strings(&base);
        for (n, old) in ids.iter().enumerate() {
            let new = new_id(n);
            let renamed = rename_rejection_id(&base, old, &new);
            load_value(&renamed);
            assert_ne!(renamed, base, "{v:?}: {old} was not renamed");
            // キーの ID を戻せば、表示文字列の値はすべて元と同じ。
            let back: BTreeMap<String, String> = display_strings(&renamed)
                .into_iter()
                .map(|(k, s)| {
                    (
                        k.replace(&format!("rejections/{new}/"), &format!("rejections/{old}/")),
                        s,
                    )
                })
                .collect();
            assert_eq!(
                back, base_strings,
                "{v:?}: display strings changed for {old}"
            );
        }
    }
}

#[test]
fn id_change_is_detected_by_the_same_comparison() {
    let specs = script_specs();
    assert_eq!(specs.len(), 37);
    let ids = judged_ids();
    for v in VARIANTS {
        let base_ir = variant_value(v);
        for via_session in [false, true] {
            let base = mask_display(&run(&base_ir, &specs, via_session));
            for (n, old) in ids.iter().enumerate() {
                let renamed_ir = rename_rejection_id(&base_ir, old, &new_id(n));
                let renamed = mask_display(&run(&renamed_ir, &specs, via_session));
                let err = compare_traces(&base, &renamed)
                    .expect_err(&format!("{v:?}/{old}: an id change must be detected"));
                assert!(
                    err.contains("events differ") || err.contains("state differs in log"),
                    "{v:?}/{old}: unexpected reason: {err}"
                );
            }
        }
    }
}

#[test]
fn id_change_difference_is_only_the_id() {
    let specs = script_specs();
    let ids = judged_ids();
    for v in VARIANTS {
        let base_ir = variant_value(v);
        let base = run(&base_ir, &specs, false);
        for (n, old) in ids.iter().enumerate() {
            let new = new_id(n);
            let renamed = run(&rename_rejection_id(&base_ir, old, &new), &specs, false);
            assert_eq!(base.len(), renamed.len());
            let (q_old, q_new) = (format!("\"{old}\""), format!("\"{new}\""));
            let mut hits = 0;
            for (i, (b, r)) in base.iter().zip(renamed.iter()).enumerate() {
                assert_eq!(b.op, r.op, "{v:?}/{old}: op #{i}");
                for (k, (pb, pr)) in parts(b).iter().zip(parts(r).iter()).enumerate() {
                    assert!(!pb.contains(&q_new), "{v:?}/{old}: new id in base");
                    assert!(!pr.contains(&q_old), "{v:?}/{old}: old id remains");
                    hits += pr.matches(&q_new).count();
                    assert_eq!(
                        &pr.replace(&q_new, &q_old),
                        pb,
                        "{v:?}/{old}: op #{i} part {k} differs beyond the id"
                    );
                }
            }
            assert!(hits > 0, "{v:?}/{old}: the new id never reached the trace");
        }
    }
}

#[test]
fn judged_ids_follow_the_rename() {
    let specs = script_specs();
    let ids = judged_ids();
    for v in VARIANTS {
        let base_ir = variant_value(v);
        let base = judged_next_actions(&run(&base_ir, &specs, false));
        for (n, old) in ids.iter().enumerate() {
            let new = new_id(n);
            let got = judged_next_actions(&run(
                &rename_rejection_id(&base_ir, old, &new),
                &specs,
                false,
            ));
            let want: Vec<_> = base
                .iter()
                .map(|(i, id, next)| {
                    let id = if id == old { new.clone() } else { id.clone() };
                    (*i, id, next.clone())
                })
                .collect();
            assert_eq!(got, want, "{v:?}/{old}");
        }
    }
}

#[test]
fn unjudged_id_rename_is_not_detected() {
    let specs = script_specs();
    let judged: BTreeSet<String> = judged_ids().into_iter().collect();
    let base_ir = original_value();
    let base = mask_display(&run(&base_ir, &specs, false));
    let mut tried = 0;
    for id in base_ir["rejections"]
        .as_object()
        .expect("rejections")
        .keys()
    {
        if judged.contains(id) || rejection_refs(&base_ir, id) == 0 {
            continue;
        }
        let renamed = rename_rejection_id(&base_ir, id, &new_id(900 + tried));
        let got = mask_display(&run(&renamed, &specs, false));
        compare_traces(&base, &got)
            .unwrap_or_else(|e| panic!("renaming unjudged {id} must not differ: {e}"));
        tried += 1;
    }
    println!("unjudged rejection ids renamed: {tried}");
}

#[test]
fn rename_fails_closed() {
    let ir = original_value();
    let judged = judged_ids();
    let old = judged[0].clone();
    let other = judged.get(1).cloned().expect("two judged ids");
    let unref = ir["rejections"]
        .as_object()
        .expect("rejections")
        .keys()
        .find(|k| rejection_refs(&ir, k) == 0)
        .cloned();
    let missing = std::panic::catch_unwind(|| rename_rejection_id(&ir, "no-such-id", "zz-x"));
    assert!(missing.is_err(), "a missing id must be refused");
    let clash = std::panic::catch_unwind(|| rename_rejection_id(&ir, &old, &other));
    assert!(clash.is_err(), "an existing new id must be refused");
    if let Some(u) = unref {
        let r = std::panic::catch_unwind(|| rename_rejection_id(&ir, &u, "zz-y"));
        assert!(r.is_err(), "an unreferenced id must be refused");
    }
}
