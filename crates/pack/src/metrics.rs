//! `fandhe-pack metrics <pack-dir>` の本体。ファイルごとの行数・実効行数・バイト数・入れ子の深さを CSV で出す
//! （AUTHOR-1、TASK-402）。
//!
//! 呼び出し元は `cli::run_metrics` で、出力の形（ヘッダ・行・`TOTAL`）の正は `docs/cli.md`（版 3）の 11 節、
//! 行の組み立ては `cli_contract::metrics_row` にある。形式 A（TOML）と形式 B（YAML）の書きやすさを比べる
//! 指標として試作（PoC-3）が使った数え方を写しており、実効行数と深さは厳密な構文解析でない近似である。
//!
//! 教材データは信頼しない入力として扱う。走査の前に `source::check_limits` で上限・シンボリックリンク・
//! 深さを確かめ、走査中もリンクを辿らず、読むときは上限 + 1 バイトで打ち切る。失敗は診断にし、部分的な表や
//! 0 の表を成功として返さない。標準出力・標準エラー出力への書き出しは呼び出し元が行う。

use std::fs;
use std::path::{Path, PathBuf};

use crate::cli_contract::{metrics_row, METRICS_HEADER, METRICS_TOTAL_LABEL};
use crate::diag::{DiagCode, Diagnostic};
use crate::source;

/// 1 ファイルの 3 つの値（バイト数は読んだ文字列の長さなので呼び出し側で足す）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measure {
    /// 行数（`str::lines` の数。末尾に改行の無い最終行も 1 行と数える）。
    pub lines: u64,
    /// 空行と `#` で始まる行を除いた行数。
    pub effective_lines: u64,
    /// 入れ子の深さ。
    pub max_depth: u64,
}

/// 行内の `{`・`[` の最大の入れ子。`"` の内側は数えない。
fn inline_depth(s: &str) -> u64 {
    let (mut depth, mut max) = (0u64, 0u64);
    let (mut in_str, mut escaped) = (false, false);
    for c in s.chars() {
        if in_str && escaped {
            escaped = false;
        } else if in_str && c == '\\' {
            escaped = true;
        } else if c == '"' {
            in_str = !in_str;
        } else if !in_str {
            match c {
                '{' | '[' => {
                    depth += 1;
                    max = max.max(depth);
                }
                '}' | ']' if depth > 0 => depth -= 1,
                _ => {}
            }
        }
    }
    max
}

/// 文字列から 3 つの値を数える。`yaml` が真なら形式 B の規則、偽なら形式 A の規則で深さを数える。
pub fn measure_text(yaml: bool, text: &str) -> Measure {
    let (mut lines, mut effective, mut max_depth) = (0u64, 0u64, 0u64);
    let mut head_depth = 0u64;
    for line in text.lines() {
        lines += 1;
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        effective += 1;
        let depth = if yaml {
            let indent = (line.len() - line.trim_start().len()) as u64;
            indent / 2 + 1 + u64::from(t.starts_with("- ")) + inline_depth(t)
        } else if t.starts_with('[') && !t.contains('=') {
            let name = t.trim_matches(|c| c == '[' || c == ']');
            head_depth = name.split('.').count() as u64;
            head_depth
        } else {
            let rhs = t.split_once('=').map_or(t, |(_, r)| r);
            head_depth + 1 + inline_depth(rhs)
        };
        max_depth = max_depth.max(depth);
    }
    Measure {
        lines,
        effective_lines: effective,
        max_depth,
    }
}

/// パック根の下の `toml`・`yaml` のファイルを名前の順・深さ優先に集める。リンクは辿らず診断にする。
fn collect(
    root: &Path,
    dir: &Path,
    depth: usize,
    files: &mut Vec<PathBuf>,
    diags: &mut Vec<Diagnostic>,
) {
    let Ok(rd) = fs::read_dir(dir) else {
        diags.push(source::scan_failure(&source::rel(root, dir)));
        return;
    };
    let mut entries = Vec::new();
    for e in rd {
        match e {
            Ok(e) => entries.push(e.path()),
            Err(_) => diags.push(source::scan_failure(&source::rel(root, dir))),
        }
    }
    entries.sort();
    for p in entries {
        let file = source::rel(root, &p);
        let Ok(md) = fs::symlink_metadata(&p) else {
            diags.push(source::scan_failure(&file));
            continue;
        };
        if md.file_type().is_symlink() {
            diags.push(source::limit_diag(&file, DiagCode::LimitSymlink));
        } else if md.is_dir() {
            if depth < source::MAX_DEPTH {
                collect(root, &p, depth + 1, files, diags);
            } else {
                diags.push(source::limit_diag(&file, DiagCode::LimitDepth));
            }
        } else if md.is_file() && p.extension().is_some_and(|x| x == "toml" || x == "yaml") {
            files.push(p);
        }
    }
}

/// 表（ヘッダ・ファイルごとの行・`TOTAL`）を組み立てる。診断が 1 件でもあれば表を返さない。
pub fn report(dir: &Path) -> Result<String, Vec<Diagnostic>> {
    source::detect_format(dir)?;
    let limits = source::check_limits(dir);
    if !limits.is_empty() {
        return Err(limits);
    }
    let mut files = Vec::new();
    let mut diags = Vec::new();
    collect(dir, dir, 0, &mut files, &mut diags);
    let mut out = String::new();
    out.push_str(METRICS_HEADER);
    out.push('\n');
    let (mut t_lines, mut t_eff, mut t_bytes, mut t_depth) = (0u64, 0u64, 0u64, 0u64);
    for p in &files {
        let file = source::rel(dir, p);
        let Some(text) = source::read_diag_text(p, &file, "file", &mut diags) else {
            continue;
        };
        let yaml = p.extension().is_some_and(|x| x == "yaml");
        let m = measure_text(yaml, &text);
        let bytes = text.len() as u64;
        out.push_str(&metrics_row(
            &file,
            m.lines,
            m.effective_lines,
            bytes,
            m.max_depth,
        ));
        out.push('\n');
        t_lines = t_lines.saturating_add(m.lines);
        t_eff = t_eff.saturating_add(m.effective_lines);
        t_bytes = t_bytes.saturating_add(bytes);
        t_depth = t_depth.max(m.max_depth);
    }
    if !diags.is_empty() {
        diags.sort_by(|a, b| (a.file(), a.code().as_str()).cmp(&(b.file(), b.code().as_str())));
        return Err(diags);
    }
    out.push_str(&format!(
        "{METRICS_TOTAL_LABEL},{t_lines},{t_eff},{t_bytes},{t_depth}\n"
    ));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m1_inline_depth_ignores_strings_and_unbalanced_closers() {
        assert_eq!(inline_depth("{ a: [1, 2] }"), 2);
        assert_eq!(inline_depth("\"[[\" x"), 0);
        assert_eq!(inline_depth("\"a\\\"[\" [x]"), 1);
        assert_eq!(inline_depth("]] [x]"), 1);
        // 偶数個のバックスラッシュの直後の `"` は文字列を閉じる。
        assert_eq!(inline_depth("{ path = \"a\\\\\", xs = [[1]] }"), 3);
    }

    #[test]
    fn m2_lines_and_effective_lines() {
        let m = measure_text(true, "a: 1\n\n# c\n  # d\nb: 2");
        assert_eq!((m.lines, m.effective_lines), (5, 2));
        let m = measure_text(false, "a = 1\r\n\r\n");
        assert_eq!((m.lines, m.effective_lines), (2, 1));
    }

    #[test]
    fn m3_yaml_depth() {
        assert_eq!(measure_text(true, "a: 1").max_depth, 1);
        assert_eq!(measure_text(true, "a:\n  b:\n    c: 1").max_depth, 3);
        assert_eq!(measure_text(true, "a:\n  - x").max_depth, 3);
        assert_eq!(measure_text(true, "a:\n  -").max_depth, 2);
        assert_eq!(measure_text(true, "a: { b: [1] }").max_depth, 3);
    }

    #[test]
    fn m4_toml_depth() {
        assert_eq!(measure_text(false, "a = 1").max_depth, 1);
        assert_eq!(measure_text(false, "[a]").max_depth, 1);
        assert_eq!(measure_text(false, "[a.b.c]\nk = 1").max_depth, 4);
        assert_eq!(measure_text(false, "[[a.b]]").max_depth, 2);
        assert_eq!(measure_text(false, "[a]\nk = { x = [1] }").max_depth, 4);
        assert_eq!(
            measure_text(false, "[a]\nk = [\n  [1], v = 2\n]").max_depth,
            3
        );
    }
}
