//! 2 つの教材パックの IR をバイト単位で比べる（`fandhe-pack compare`、04-behavior の PACK-6）。
//!
//! `cli::run_compare` から呼ばれる。両方のパックを `compile::compile_dir` に通し（読み込み層と上限の検査は
//! `compile` と同じ）、返ったバイト列そのものを比べる。`serde_json::Value` にして比べると、バイトでは違う
//! のに一致と判定する場合があるため、比較のために別の直列化は書かない。
//! 違いの報告は位置（バイトの位置と行）だけにする。IR の中身・本文・パスは出さない。

use std::path::Path;

use crate::compile::compile_dir;
use crate::diag::Diagnostic;

/// 2 つの IR のバイト列の比較の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// 一致した。`len` は IR のバイト数。
    Same {
        /// IR のバイト数。
        len: usize,
    },
    /// 一致しない。
    Differ {
        /// 最初に違うバイトの位置（0 始まり）。長さだけが違う場合は短い側の長さ。
        offset: usize,
        /// その位置の行（1 始まり。`a` の側で数える）。
        line: usize,
    },
}

/// 片方または両方のパックに診断がある。判定はしない（失敗を一致とみなさない）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareError {
    /// `a` の診断（無ければ空）。
    pub a: Vec<Diagnostic>,
    /// `b` の診断（無ければ空）。
    pub b: Vec<Diagnostic>,
}

/// バイト列を比べる。長さだけが違う場合は、短い側の末尾の位置を違いの位置にする。
pub fn compare_bytes(a: &[u8], b: &[u8]) -> Outcome {
    let offset = match a.iter().zip(b).position(|(x, y)| x != y) {
        Some(i) => i,
        None if a.len() == b.len() => return Outcome::Same { len: a.len() },
        None => a.len().min(b.len()),
    };
    // 違いの位置より前にある改行の数が、行（1 始まり）を決める。
    let line = 1 + a[..offset.min(a.len())]
        .iter()
        .filter(|&&c| c == b'\n')
        .count();
    Outcome::Differ { offset, line }
}

/// 両方のパックを `compile_dir` で IR にして比べる。
///
/// `a` が失敗しても `b` はコンパイルし、両方の診断をまとめて返す（作成者が 1 回で両方を直せるように）。
pub fn compare_dirs(a: &Path, b: &Path) -> Result<Outcome, CompareError> {
    match (compile_dir(a), compile_dir(b)) {
        (Ok(x), Ok(y)) => Ok(compare_bytes(&x, &y)),
        (ra, rb) => Err(CompareError {
            a: ra.err().unwrap_or_default(),
            b: rb.err().unwrap_or_default(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_and_empty() {
        assert_eq!(compare_bytes(b"ab\n", b"ab\n"), Outcome::Same { len: 3 });
        assert_eq!(compare_bytes(b"", b""), Outcome::Same { len: 0 });
    }

    #[test]
    fn differs_at_first_byte() {
        assert_eq!(
            compare_bytes(b"a", b"b"),
            Outcome::Differ { offset: 0, line: 1 }
        );
    }

    #[test]
    fn line_is_counted_on_a() {
        assert_eq!(
            compare_bytes(b"x\ny\nzz\n", b"x\ny\nzy\n"),
            Outcome::Differ { offset: 5, line: 3 }
        );
    }

    #[test]
    fn prefix_differs_at_end_of_shorter() {
        assert_eq!(
            compare_bytes(b"ab\n", b"ab\ncd"),
            Outcome::Differ { offset: 3, line: 2 }
        );
        assert_eq!(
            compare_bytes(b"ab\ncd", b"ab\n"),
            Outcome::Differ { offset: 3, line: 2 }
        );
        assert_eq!(
            compare_bytes(b"", b"x"),
            Outcome::Differ { offset: 0, line: 1 }
        );
    }

    #[test]
    fn missing_dirs_report_each_side() {
        let missing = Path::new("/nonexistent/fandhe-pack-compare");
        let e = compare_dirs(missing, missing).expect_err("both fail");
        assert!(!e.a.is_empty() && !e.b.is_empty());
    }
}
