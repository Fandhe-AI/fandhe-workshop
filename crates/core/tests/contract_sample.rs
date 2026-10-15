//! 契約のサンプル（TASK-205 1/2）の形の試験。
//!
//! 確かめるのは、サンプルが core の型で読めること、版が契約と合っていること、前後の状態が連なること、
//! 9 種の操作と EVT-6 の (1)〜(6) を覆うこと、`log_len` が数え方と合うことまで。
//! 世代の差の照合（EVT-6）と 2 回適用の一致（EVT-22）は、適合試験（#170）が担う。

mod common;

use std::collections::HashSet;

use common::contract_sample::{load, testdata_dir, Kind};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::{load_ir, CONTRACT_VERSION};

#[test]
fn versions_match_the_contract() {
    let s = load();
    assert_eq!(s.sample_version, 1, "sample_version");
    assert_eq!(
        s.contract_version, CONTRACT_VERSION,
        "contract changed: update the sample and bump sample_version"
    );
}

#[test]
fn ir_path_is_inside_testdata_and_loads() {
    let s = load();
    assert!(!s.ir.contains(".."), "ir path must not contain '..'");
    let bytes = std::fs::read(testdata_dir().join(&s.ir)).expect("sample ir must be readable");
    load_ir(&bytes).expect("sample ir must load");
}

#[test]
fn ids_are_sequential_and_states_chain() {
    let s = load();
    assert!(!s.entries.is_empty(), "no entries");
    assert_eq!(
        s.entries[0].before, s.initial.run,
        "e01 before != initial.run"
    );
    for (i, e) in s.entries.iter().enumerate() {
        assert_eq!(e.id, format!("e{:02}", i + 1), "id at index {i}");
        assert!(!e.note.is_empty(), "{}: empty note", e.id);
        if i > 0 {
            assert_eq!(
                e.before,
                s.entries[i - 1].after,
                "{}: before != previous after",
                e.id
            );
        }
    }
}

#[test]
fn kinds_and_evt6_are_covered() {
    let s = load();
    let kinds: HashSet<Kind> = s
        .entries
        .iter()
        .flat_map(|e| e.kinds.iter().copied())
        .collect();
    for k in Kind::ALL {
        assert!(kinds.contains(&k), "kind not covered: {k:?}");
    }
    let mut seen = HashSet::new();
    for e in &s.entries {
        assert!((1..=6).contains(&e.evt6), "{}: evt6 out of range", e.id);
        seen.insert(e.evt6);
    }
    for n in 1..=6u8 {
        assert!(seen.contains(&n), "evt6 not covered: {n}");
    }
}

#[test]
fn log_len_matches_the_running_count() {
    let s = load();
    let mut len = u32::try_from(s.initial.log.len()).expect("log length fits u32");
    let mut saved = std::collections::HashMap::new();
    for e in &s.entries {
        for ev in &e.events {
            match ev {
                Event::CheckpointSaved { id, log_len, .. } => {
                    assert_eq!(*log_len, len, "{}: CheckpointSaved.log_len", e.id);
                    saved.insert(*id, *log_len);
                }
                Event::Restored { id, log_len, .. } => {
                    assert_eq!(saved.get(id), Some(log_len), "{}: Restored.log_len", e.id);
                }
                _ => {}
            }
        }
        len += u32::try_from(e.events.len()).expect("event count fits u32");
    }
}
