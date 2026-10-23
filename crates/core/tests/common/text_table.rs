//! 教材の表示文字列を差し替え用の表（A・B）に置き換える部品（TASK-218 2/3。EVT-2・性質 P-7）。
//!
//! 呼び出し元は `text_independence.rs`。3/3（ID の変更の負の対照）も、同じ読み込み口 [`load_variant`] と
//! 写し [`mask_display`] を再利用できるよう共通部品に置く。
//!
//! # 差し替える場所
//!
//! 表示文字列は 31 個で、キーは `testdata/engine-event-consistency/test-data/text-table-{A,B}.json` と同じ
//! 「`pack_title`」「`step_titles/<uid>`」「`option_labels/<id>`」「`hints/<id>`」「`rejections/<id>/message`」
//! 「`rejections/<id>/next_action`」である。表は spec の写しで、本ファイルは変更しない。
//!
//! # 失敗側に倒す
//!
//! 差し替えが空振りして試験が素通りしないよう、[`substitute`] は「表のキーが IR の表示文字列のキーとちょうど
//! 一致すること」「各キーがちょうど 1 か所の既存の文字列に当たること」を `assert` で確かめる。
//!
//! # `Judged.next_action` の扱い
//!
//! `Event::Judged` は差し込み後の次の行動（表示文字列）を保持する。差し替えるとそこだけ必ず変わるので、
//! 状態・イベント列の比較は [`mask_display`] で次の行動を文面の ID に写してから行う。ID に写すため、
//! ID の変更（3/3）は差として残る。
//! 試験の部品なので、読めない・形が違う場合は理由を添えた `expect` / `assert` で止める（本番コードの規則の対象外）。

use std::collections::BTreeMap;

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::{Judgment, NextAction};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::SessionState;
use serde_json::Value;

use super::contract_sample::{load, testdata_dir};
use super::trace::StepTrace;

/// 差し替えの種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// 元の文字列のまま。
    Original,
    /// 表 A。
    A,
    /// 表 B。
    B,
}

/// 表示文字列の表（キー → 文字列）。
pub type Strings = BTreeMap<String, String>;

fn min_ir_path() -> std::path::PathBuf {
    testdata_dir().join("engine-event-consistency/test-data/min-3step.json")
}

/// 元の IR を JSON の値で読む。
pub fn original_value() -> Value {
    serde_json::from_slice(&std::fs::read(min_ir_path()).expect("read min-3step"))
        .expect("parse min-3step")
}

/// 差し替え表を読み、キーを平らにした表にする。`table` の欄は除く。
pub fn load_table(name: &str) -> Strings {
    let path = testdata_dir().join(format!(
        "engine-event-consistency/test-data/text-table-{name}.json"
    ));
    let value: Value =
        serde_json::from_slice(&std::fs::read(path).expect("read table")).expect("parse table");
    let mut out = Strings::new();
    for (key, v) in value.as_object().expect("table is an object") {
        match key.as_str() {
            "table" => {}
            "pack_title" => {
                out.insert(key.clone(), v.as_str().expect("pack_title").to_string());
            }
            "clarifications" => assert!(
                v.as_object().is_some_and(|o| o.is_empty()),
                "clarifications in the table must be empty"
            ),
            "rejections" => {
                for (id, body) in v.as_object().expect("rejections") {
                    for field in ["message", "next_action"] {
                        out.insert(
                            format!("rejections/{id}/{field}"),
                            body[field].as_str().expect("rejection field").to_string(),
                        );
                    }
                }
            }
            group => {
                for (id, s) in v.as_object().expect("group is an object") {
                    out.insert(
                        format!("{group}/{id}"),
                        s.as_str().expect("string").to_string(),
                    );
                }
            }
        }
    }
    out
}

/// 表のキーに対応する IR 中の文字列の場所を返す（無ければ `None`）。
fn slot<'a>(ir: &'a mut Value, key: &str) -> Option<&'a mut Value> {
    let (group, rest) = key.split_once('/').unwrap_or((key, ""));
    match group {
        "pack_title" => ir.pointer_mut("/pack/title"),
        "step_titles" => ir
            .get_mut("nodes")?
            .as_array_mut()?
            .iter_mut()
            .find(|n| n["uid"] == rest)?
            .pointer_mut("/step/title"),
        "option_labels" => {
            let mut found = None;
            for node in ir.get_mut("nodes")?.as_array_mut()? {
                let Some(opts) = node
                    .pointer_mut("/step/input/options")
                    .and_then(Value::as_array_mut)
                else {
                    continue;
                };
                for o in opts {
                    if o["id"] == rest {
                        assert!(found.is_none(), "option id {rest} appears twice");
                        found = o.get_mut("label");
                    }
                }
            }
            found
        }
        "hints" => ir.pointer_mut(&format!("/hints/{rest}/text")),
        "rejections" => {
            let (id, field) = rest.split_once('/')?;
            ir.pointer_mut(&format!("/rejections/{id}/{field}"))
        }
        _ => None,
    }
}

/// IR の表示文字列を、表と同じキーで集める。
pub fn display_strings(ir: &Value) -> Strings {
    let mut keys: Vec<String> = vec!["pack_title".to_string()];
    for node in ir["nodes"].as_array().expect("nodes") {
        keys.push(format!(
            "step_titles/{}",
            node["uid"].as_str().expect("uid")
        ));
        if let Some(opts) = node
            .pointer("/step/input/options")
            .and_then(Value::as_array)
        {
            for o in opts {
                keys.push(format!("option_labels/{}", o["id"].as_str().expect("id")));
            }
        }
    }
    for id in ir["hints"].as_object().expect("hints").keys() {
        keys.push(format!("hints/{id}"));
    }
    for id in ir["rejections"].as_object().expect("rejections").keys() {
        keys.push(format!("rejections/{id}/message"));
        keys.push(format!("rejections/{id}/next_action"));
    }
    let mut copy = ir.clone();
    keys.into_iter()
        .map(|k| {
            let s = slot(&mut copy, &k)
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| panic!("display slot {k} is not a string"))
                .to_string();
            (k, s)
        })
        .collect()
}

/// 表のとおりに IR の表示文字列を置き換える。キーの集合が IR と食い違えば止める。
pub fn substitute(ir: &Value, table: &Strings) -> Value {
    let present = display_strings(ir);
    assert_eq!(
        present.keys().collect::<Vec<_>>(),
        table.keys().collect::<Vec<_>>(),
        "table keys must equal the IR's display string keys"
    );
    let mut out = ir.clone();
    for (key, text) in table {
        let s = slot(&mut out, key).unwrap_or_else(|| panic!("no slot for {key}"));
        assert!(s.is_string(), "slot {key} is not a string");
        *s = Value::String(text.clone());
    }
    out
}

/// 表示文字列の場所をすべて空文字にした写し。構造（ID・分類・規則）の一致の比較に使う。
pub fn blank_display(ir: &Value) -> Value {
    let blank: Strings = display_strings(ir)
        .into_keys()
        .map(|k| (k, String::new()))
        .collect();
    substitute(ir, &blank)
}

/// 差し替えた IR（JSON の値）。
pub fn variant_value(v: Variant) -> Value {
    let original = original_value();
    match v {
        Variant::Original => original,
        Variant::A => substitute(&original, &load_table("A")),
        Variant::B => substitute(&original, &load_table("B")),
    }
}

/// 差し替えの種類ごとの表示文字列。
pub fn variant_strings(v: Variant) -> Strings {
    display_strings(&variant_value(v))
}

/// 差し替えた IR と初期状態を読む。初期状態は契約のサンプルのものである。
pub fn load_variant(v: Variant) -> (Ir, SessionState) {
    let bytes = serde_json::to_vec(&variant_value(v)).expect("serialize ir");
    (load_ir(&bytes).expect("load ir"), load().initial)
}

const MASK_PREFIX: &str = "<next_action:";

fn masked(text_id: &str) -> NextAction {
    NextAction::new(format!("{MASK_PREFIX}{text_id}>")).expect("non-empty masked next_action")
}

fn mask_event(event: &mut Event) {
    if let Event::Judged { judgment, .. } = event {
        match judgment {
            Judgment::Rejected(r) => r.next_action = masked(&r.text),
            Judgment::Clarified(c) => c.next_action = masked(&c.text),
            Judgment::Accepted(Some(n)) => n.next_action = masked(&n.text),
            _ => {}
        }
    }
}

/// `Judged` の次の行動を、文面の ID から作った値に置き換えた写し。文面の ID・分類・種類はそのまま残す。
pub fn mask_display(traces: &[StepTrace]) -> Vec<StepTrace> {
    traces
        .iter()
        .cloned()
        .map(|mut t| {
            t.events.iter_mut().for_each(mask_event);
            t.after.log.iter_mut().for_each(mask_event);
            t
        })
        .collect()
}

/// 記録の `Judged` の（操作の番号, 文面の ID, 次の行動）を、イベント列の側から集める。
pub fn judged_next_actions(traces: &[StepTrace]) -> Vec<(usize, String, String)> {
    let mut out = Vec::new();
    for (i, t) in traces.iter().enumerate() {
        for e in &t.events {
            let Event::Judged { judgment, .. } = e else {
                continue;
            };
            let found = match judgment {
                Judgment::Rejected(r) => Some((&r.text, &r.next_action)),
                Judgment::Clarified(c) => Some((&c.text, &c.next_action)),
                Judgment::Accepted(Some(n)) => Some((&n.text, &n.next_action)),
                _ => None,
            };
            if let Some((text, next)) = found {
                out.push((i, text.clone(), next.as_str().to_string()));
            }
        }
    }
    out
}

/// 記録を直列化した文字列に、与えた文字列のどれかが現れれば、その文字列を返す。
pub fn leaked_string<'a>(
    traces: &[StepTrace],
    strings: impl Iterator<Item = &'a String>,
) -> Option<String> {
    let mut hay = String::new();
    for t in traces {
        for part in [
            serde_json::to_string(&t.events),
            serde_json::to_string(&t.after),
            serde_json::to_string(&t.log_rows),
            serde_json::to_string(&t.marks),
            serde_json::to_string(&t.review),
        ] {
            hay.push_str(&part.expect("serialize trace part"));
        }
    }
    strings
        .filter(|s| !s.is_empty())
        .find(|s| hay.contains(s.as_str()))
        .cloned()
}
