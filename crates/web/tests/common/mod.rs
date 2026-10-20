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

/// `src/` 配下の全 `.rs` を（src からの相対パス, 全文）の組で返す。表示層の静的な検査が使う。
pub fn web_src_sources() -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let rel = p.strip_prefix(&root).unwrap_or(&p).display().to_string();
            let src = std::fs::read_to_string(&p).expect("src を読めない");
            (rel, src)
        })
        .collect()
}

/// `src/host.rs` の全文。
pub fn host_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/host.rs");
    std::fs::read_to_string(path).expect("src/host.rs を読めない")
}

/// `lib.rs` の本文から、`cfg` に `target_arch = "wasm32"` を含む属性の直後にある `mod X;` の名前を求める。
///
/// wasm32 でしかコンパイルされない web-sys 層（glue・保存・書き出し・試験用の口）を、
/// ファイル名の決め打ちではなく宣言から求めるための関数。属性と宣言の間の doc コメント・他の属性は読み飛ばす。
pub fn wasm_layer_module_names(lib_src: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut pending = false;
    for line in lib_src.lines() {
        let t = line.trim();
        if t.starts_with("#[cfg(") {
            if t.contains("target_arch = \"wasm32\"") {
                pending = true;
            }
        } else if t.starts_with("#[") || t.starts_with("//") {
            // 属性の続き・コメントは読み飛ばす。
        } else {
            if pending {
                let rest = t.strip_prefix("pub ").unwrap_or(t);
                if let Some(n) = rest.strip_prefix("mod ").and_then(|r| r.strip_suffix(';')) {
                    names.push(n.trim().to_owned());
                }
            }
            pending = false;
        }
    }
    names
}

/// wasm32 専用の web-sys 層のモジュールを（名前, 全文）の組で返す。読めない・空なら panic（fail-closed）。
pub fn wasm_layer_sources() -> Vec<(String, String)> {
    let src_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let lib = std::fs::read_to_string(src_dir.join("lib.rs")).expect("src/lib.rs を読めない");
    let names = wasm_layer_module_names(&lib);
    assert!(
        !names.is_empty(),
        "wasm32 専用のモジュールを 1 件も求められない"
    );
    names
        .into_iter()
        .map(|n| {
            let body = std::fs::read_to_string(src_dir.join(format!("{n}.rs")))
                .unwrap_or_else(|e| panic!("src/{n}.rs を読めない: {e}"));
            (n, body)
        })
        .collect()
}
