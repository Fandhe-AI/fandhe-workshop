//! 秘密情報らしい値を `secret-shapes.json` の部品から実行時に組み立てる（TASK-164 1/3）。
//!
//! 完成した値は repo のどこにも置かない（push protection と pre-commit の検査のため）。
//! 展開は `{N×c}`・`{seqN}`・`\n` の 3 つで、知らない記法は試験の失敗にする。
#![allow(dead_code, reason = "試験ごとに使う関数が違う")]

use serde_json::Value;

fn fixture() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../testdata/untrusted-input-safety/test-data-addendum-4/secret-shapes.json"
    );
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn expand(part: &str) -> String {
    let mut out = String::new();
    let mut rest = part;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("\\n") {
            out.push('\n');
            rest = r;
        } else if let Some(r) = rest.strip_prefix('{') {
            let (body, tail) = r.split_once('}').unwrap();
            if let Some(n) = body.strip_prefix("seq") {
                out.extend(
                    "0123456789abcdefghijklmnopqrstuvwxyz"
                        .chars()
                        .take(n.parse().unwrap()),
                );
            } else {
                let (n, c) = body.split_once('\u{00D7}').unwrap();
                out.extend(std::iter::repeat_n(
                    c.chars().next().unwrap(),
                    n.parse().unwrap(),
                ));
            }
            rest = tail;
        } else {
            let end = rest.find(['{', '\\']).unwrap_or(rest.len()).max(1);
            let (head, tail) = rest.split_at(end);
            out.push_str(head);
            rest = tail;
        }
    }
    out
}

/// 10 形の（試験データの ID, 期待する形の ID, 組み立てた値）。
pub fn shapes() -> Vec<(String, String, String)> {
    fixture()["shapes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            let v: String = s["parts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| expand(p.as_str().unwrap()))
                .collect();
            (
                s["id"].as_str().unwrap().to_owned(),
                s["expect"].as_str().unwrap().to_owned(),
                v,
            )
        })
        .collect()
}

/// 対照 6 件の（ID, 文）。
pub fn controls() -> Vec<(String, String)> {
    let f = fixture();
    ["controls", "extra_controls"]
        .iter()
        .flat_map(|k| f[*k].as_array().unwrap().clone())
        .map(|c| {
            (
                c["id"].as_str().unwrap().to_owned(),
                c["text"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}
