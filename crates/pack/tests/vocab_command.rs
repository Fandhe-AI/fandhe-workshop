//! `fandhe-pack vocab`（TASK-408、AUTHOR-9）の結合試験。実行ファイルを起動して、表・集計行・教材パック全体の行・
//! 失敗時の扱い・ID のエスケープを確かめる。試験名の `author9_` で 04 の ID を追える。
//!
//! 試験用の教材は `tests/data/vocab-pack`（架空）。欄を変える試験は、一時ディレクトリへ写してから書き換える。

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{copy_dir, Tmp};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
}

fn pack() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/vocab-pack")
}

/// `(終了コード, 標準出力, 標準エラー出力)`。
fn vocab(dir: &Path) -> (i32, String, String) {
    let o = bin().arg("vocab").arg(dir).output().unwrap();
    (
        o.status.code().unwrap(),
        String::from_utf8(o.stdout).unwrap(),
        String::from_utf8(o.stderr).unwrap(),
    )
}

/// 試験用の教材の写し。`edit` で `(相対パス, 置換前, 置換後)` を適用する。置換前が無ければ試験を失敗させる。
fn edited(tag: &str, edits: &[(&str, &str, &str)]) -> Tmp {
    let t = Tmp::new(tag);
    copy_dir(&pack(), &t.0);
    for (rel, from, to) in edits {
        let p = t.0.join(rel);
        let s = fs::read_to_string(&p).unwrap();
        assert!(s.contains(from), "{rel}: {from:?} not found");
        fs::write(&p, s.replacen(from, to, 1)).unwrap();
    }
    t
}

const WORKSHOP: &str = "workshop.yaml";
const CHAPTER: &str = "chapters/c1/chapter.yaml";
const VOCAB_LINE: &str = "  vocab: \"1.4\"\n";

const EXPECTED: &str = "\
scope,step,kind,extension_points
c1,plain,step,
c1,ep1,step,EP-1
c1,ep2,step,EP-2
c1,ep4,step,EP-4
c1,ep5,step,EP-5
c1,ep6,step,EP-6
c1,ep7,step,EP-7
c1,ep8,step,EP-8
c1,ep13,step,EP-13
c1,multi,step,EP-1 EP-7 EP-13
c1,call,use,
c1,end,step,
m1,m1a,step,EP-3
m1,m1b,step,
m2,m2a,step,EP-7
# (i) 既存の語彙の組み合わせだけで書けた Step: 3
# (ii) 名前つき拡張点を使って書けた Step: 11
# EP-1 を使った Step: 2
# EP-2 を使った Step: 1
# EP-3 を使った Step: 1
# EP-4 を使った Step: 1
# EP-5 を使った Step: 1
# EP-6 を使った Step: 1
# EP-7 を使った Step: 3
# EP-8 を使った Step: 1
# EP-13 を使った Step: 2
";

/// 表（章・呼ばれるモジュール・呼ばれないモジュール・`use` 行）と、（i）（ii）・EP ごとの行が期待どおりに出る。
/// EP の並びは番号順で、`EP-13` が `EP-2` より前に来ない。教材パック全体の行は出ない。
#[test]
fn author9_table_and_summary() {
    let (code, out, err) = vocab(&pack());
    assert_eq!(code, 0);
    assert!(err.is_empty(), "{err}");
    assert_eq!(out, EXPECTED);
    assert!(!out.contains("教材パック全体"));
}

fn pack_wide(out: &str) -> Vec<&str> {
    out.lines()
        .filter(|l| l.contains("教材パック全体"))
        .collect()
}

const PROFILE: (&str, &str) = (
    "profile: {command_prefix: \">\"}\n",
    "# EP-10 を使った（教材パック全体）: pack.profile",
);
const APPEARANCE: (&str, &str) = (
    "appearance: {theme: mono}\n",
    "# EP-11 を使った（教材パック全体）: pack.appearance",
);
const LEFT_PANE: (&str, &str) = (
    "left_pane: {world_list: hidden}\n",
    "# EP-12 を使った（教材パック全体）: pack.left_pane",
);

fn with_pack_fields(tag: &str, fields: &[&str]) -> Tmp {
    let add: String = fields.iter().map(|f| format!("  {f}")).collect();
    edited(
        tag,
        &[(WORKSHOP, VOCAB_LINE, &format!("{VOCAB_LINE}{add}"))],
    )
}

/// 3 つの欄を単独で既定から変えると、該当する 1 行だけが末尾に出る。3 つ同時なら EP-10・11・12 の順に出る。
/// いずれも（i）（ii）の数は変わらない。
#[test]
fn author9_pack_wide_lines_only_when_changed() {
    for (decl, line) in [PROFILE, APPEARANCE, LEFT_PANE] {
        let t = with_pack_fields("v-one", &[decl]);
        let (code, out, err) = vocab(&t.0);
        assert_eq!(code, 0, "{decl}: {out}");
        assert!(err.is_empty());
        assert_eq!(pack_wide(&out), [line], "{decl}");
        assert!(out.ends_with(&format!("{line}\n")), "末尾に出る: {decl}");
        assert_eq!(out.replace(&format!("{line}\n"), ""), EXPECTED, "{decl}");
    }
    let t = with_pack_fields("v-all", &[PROFILE.0, APPEARANCE.0, LEFT_PANE.0]);
    let (code, out, _) = vocab(&t.0);
    assert_eq!(code, 0, "{out}");
    assert_eq!(pack_wide(&out), [PROFILE.1, APPEARANCE.1, LEFT_PANE.1]);
    let body = out.replace(
        &format!("{}\n{}\n{}\n", PROFILE.1, APPEARANCE.1, LEFT_PANE.1),
        "",
    );
    assert_eq!(body, EXPECTED);
}

/// 欄を既定と同じ値で明示的に書いても、教材パック全体の行は出ない（欄の有無ではなく値で判定する）。
#[test]
fn author9_explicit_default_values_emit_nothing() {
    let t = with_pack_fields(
        "v-default",
        &[
            "profile: {command_prefix: \"/\", input_field: shown}\n",
            "appearance: {theme: standard}\n",
            "left_pane: {world_list: shown, view_tabs: all}\n",
        ],
    );
    let (code, out, err) = vocab(&t.0);
    assert_eq!(code, 0, "{out}");
    assert!(err.is_empty());
    assert_eq!(out, EXPECTED);
}

/// EP-9（`starts[].requires`）と `confirm.options[].label` は数えない（TASK-409 の範囲）。足しても出力が変わらない。
#[test]
fn author9_ep9_and_label_not_counted() {
    let t = edited(
        "v-ep9",
        &[
            (
                CHAPTER,
                "{id: \"s1\", node: \"plain\"}",
                "{id: \"s1\", node: \"plain\", requires: {flag: \"f\"}}",
            ),
            (
                CHAPTER,
                "{id: \"opt-a\", correct: true}",
                "{id: \"opt-a\", label: \"A\", correct: true}",
            ),
        ],
    );
    let (code, out, err) = vocab(&t.0);
    assert_eq!(code, 0, "{out}");
    assert!(err.is_empty());
    assert_eq!(out, EXPECTED);
}

/// 読み込めないディレクトリでは、終了コード 1 で診断（4 欄）だけを標準出力に出し、表のヘッダを出さない。
#[test]
fn author9_load_failure_prints_diagnostics_only() {
    let t = Tmp::new("v-load");
    let (code, out, err) = vocab(&t.0);
    assert_eq!(code, 1);
    assert!(err.is_empty(), "{err}");
    assert!(!out.is_empty());
    assert!(!out.contains("scope,step,kind"));
    for l in out.lines() {
        assert!(l.splitn(4, ": ").count() == 4 && l.contains("E-"), "{l}");
    }
}

/// compile が失敗するパック（語彙にない条件の語）では、終了コード 1 で、表を出さず診断だけを出す。
#[test]
fn author9_compile_failure_prints_diagnostics_only() {
    let t = edited(
        "v-compile",
        &[(CHAPTER, "{flag: \"f\"}", "{no_such_word: \"f\"}")],
    );
    let (code, out, err) = vocab(&t.0);
    assert_eq!(code, 1, "{out}");
    assert!(err.is_empty());
    assert!(out.contains("E-"), "{out}");
    assert!(!out.contains("scope,step,kind") && !out.contains("# (i)"));
}

/// `scope` と `step` の ID に `,`・`"`・改行・先頭の `#` を含めても、各行が 4 欄に保たれ、`\u{XXXX}` に直り、
/// 表の行が集計行（`# ` 始まり）に見えない。
#[test]
fn author9_hostile_ids_are_escaped() {
    let t = edited(
        "v-hostile",
        &[
            (CHAPTER, "id: \"c1\"\n", "id: \"#a,b\\\"c\\nd\"\n"),
            (CHAPTER, "id: \"plain\"", "id: \"#x,y\\\"z\\ne\""),
            (CHAPTER, "node: \"plain\"", "node: \"#x,y\\\"z\\ne\""),
        ],
    );
    let (code, out, err) = vocab(&t.0);
    assert_eq!(code, 0, "{out}");
    assert!(err.is_empty());
    let first = "\\u{0023}a\\u{002C}b\\u{0022}c\\u{000A}d,#x\\u{002C}y\\u{0022}z\\u{000A}e,step,";
    assert!(out.lines().any(|l| l == first), "{out}");
    let mut in_table = true;
    for l in out.lines().skip(1) {
        if l.starts_with("# ") {
            in_table = false;
            continue;
        }
        assert!(in_table, "表の行が集計行の後ろに出た: {l}");
        assert_eq!(l.matches(',').count(), 3, "{l}");
        assert!(!l.starts_with('#'), "{l}");
    }
    assert_eq!(out.lines().filter(|l| l.starts_with("# (")).count(), 2);
}

/// 呼ばれないモジュールの Step も行と（i）（ii）に入り、`use` 行は（i）（ii）に入らない。
#[test]
fn author9_module_scope_rows() {
    let (_, out, _) = vocab(&pack());
    assert!(out.contains("\nm2,m2a,step,EP-7\n"));
    assert!(out.contains("\nc1,call,use,\n"));
    let rows = out
        .lines()
        .skip(1)
        .take_while(|l| !l.starts_with("# "))
        .count();
    let (i, ii) = (3, 11);
    // 行の数は、`use` 行 1 つを除いて（i）（ii）の合計に等しい。
    assert_eq!(rows, i + ii + 1);
}
