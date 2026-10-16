//! glue.rs のソースを文字列として読んで調べる試験の共有ヘルパ（`state_paths`・`glue_contract` が使う）。
//!
//! `glue.rs` は wasm32 でしかコンパイルされないため、native の試験はソースを文字列として読む。
//! 解析は、コメントを除き、空白と `r#` を除いた文字列で照合する。

#![allow(
    dead_code,
    reason = "試験の crate ごとに使うヘルパが異なり、使わない側で未使用の警告が出るため"
)]

use std::path::PathBuf;

/// コメント（`//` は行末まで、`/* */` は入れ子を数えて）を空白に置き換える。改行は残す。
pub fn strip_comments(src: &str) -> String {
    let b: Vec<char> = src.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '/' && b.get(i + 1) == Some(&'/') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
        } else if b[i] == '/' && b.get(i + 1) == Some(&'*') {
            let mut depth = 1;
            i += 2;
            while i < b.len() && depth > 0 {
                if b[i] == '/' && b.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if b[i] == '*' && b.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            out.push(' ');
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

/// 空白・改行と `r#`（raw identifier）を除いた形。`rt . component ( )` や `r#update(` を同じ形にそろえる。
pub fn normalize(code: &str) -> String {
    let s: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    s.replace("r#", "")
}

pub fn glue_code(src: &str) -> String {
    normalize(&strip_comments(src))
}

pub fn count(hay: &str, needle: &str) -> usize {
    hay.matches(needle).count()
}

/// 正規化済みコードから `fn <name>` の本体（最初の `{` から対応する `}` まで）を切り出す。
pub fn fn_body<'a>(code: &'a str, name: &str) -> Option<&'a str> {
    let head = format!("fn{name}");
    let start = code.find(&head)?;
    let open = start + code[start..].find('{')?;
    let mut depth = 0usize;
    for (i, c) in code[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&code[open..open + i + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

/// `src/glue.rs` の全文。
pub fn glue_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/glue.rs");
    std::fs::read_to_string(path).expect("src/glue.rs を読めない")
}
