//! `fandhe-pack vocab`（TASK-408、AUTHOR-9）の結合試験。実行ファイルを起動して、表・集計行・教材パック全体の行・
//! 失敗時の扱い・ID のエスケープを確かめる。試験名の `author9_`・`author17_` で 04 の ID を追える。
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

const START: &str = "{id: \"s1\", node: \"plain\"}";

/// EP-9 を足した期待値。`plain` の行は `row` に置き換え、（i）（ii）と EP-9 の集計行を直す。
fn with_ep9(row_from: &str, row_to: &str, plain_delta: bool) -> String {
    let mut e = EXPECTED.replacen(row_from, row_to, 1);
    if plain_delta {
        e = e
            .replace(
                "(i) 既存の語彙の組み合わせだけで書けた Step: 3",
                "(i) 既存の語彙の組み合わせだけで書けた Step: 2",
            )
            .replace(
                "(ii) 名前つき拡張点を使って書けた Step: 11",
                "(ii) 名前つき拡張点を使って書けた Step: 12",
            );
    }
    e.replacen(
        "# EP-13 を使った",
        "# EP-9 を使った Step: 1\n# EP-13 を使った",
        1,
    )
}

/// 開始点が指す Step の行に EP-9 を数える（AUTHOR-17）。EP-9 の行は EP-8 と EP-13 の間に出る。
#[test]
fn author17_ep9_counted_on_start_node_row() {
    let t = edited(
        "v-ep9a",
        &[(
            CHAPTER,
            START,
            "{id: \"s1\", node: \"plain\", requires: {flag: \"f\"}, unmet: \"rj-wrong\"}",
        )],
    );
    let (code, out, err) = vocab(&t.0);
    assert_eq!(code, 0, "{out}");
    assert!(err.is_empty());
    assert_eq!(
        out,
        with_ep9("c1,plain,step,\n", "c1,plain,step,EP-9\n", true)
    );
}

/// 付く行は開始点の `node` の行で、scope の先頭の Step ではない。
#[test]
fn author17_ep9_attributed_to_target_step_not_first() {
    let t = edited(
        "v-ep9b",
        &[(
            CHAPTER,
            START,
            "{id: \"s1\", node: \"ep7\", requires: {flag: \"f\"}, unmet: \"rj-wrong\"}",
        )],
    );
    let (code, out, _) = vocab(&t.0);
    assert_eq!(code, 0, "{out}");
    assert_eq!(
        out,
        with_ep9("c1,ep7,step,EP-7\n", "c1,ep7,step,EP-7 EP-9\n", false)
    );
}

/// 片方だけの開始点は `compile` の関門（`E-STRUCTURE`）で止まり、表を出さない（TASK-51）。
#[test]
fn author17_one_sided_ep9_is_refused_by_compile_gate() {
    for (name, start) in [
        (
            "v-ep9c1",
            "{id: \"s1\", node: \"plain\", unmet: \"rj-wrong\"}",
        ),
        (
            "v-ep9c2",
            "{id: \"s1\", node: \"plain\", requires: {flag: \"f\"}}",
        ),
    ] {
        let t = edited(name, &[(CHAPTER, START, start)]);
        let (code, out, _) = vocab(&t.0);
        assert_eq!(code, 1, "{out}");
        assert!(out.contains("E-STRUCTURE"), "{out}");
        assert!(out.contains("start:s1"), "{out}");
        assert!(!out.contains("EP-9"), "{out}");
    }
}

/// 同じ Step を指す開始点が複数あっても、その Step に 1 回だけ数える。
#[test]
fn author17_two_starts_same_step_counted_once() {
    let t = edited(
        "v-ep9d",
        &[(
            CHAPTER,
            START,
            "{id: \"s1\", node: \"plain\", requires: {flag: \"f\"}, unmet: \"rj-wrong\"}\n  - {id: \"s2\", node: \"plain\", requires: {flag: \"g\"}, unmet: \"rj-wrong\"}",
        )],
    );
    let (code, out, _) = vocab(&t.0);
    assert_eq!(code, 0, "{out}");
    assert_eq!(
        out,
        with_ep9("c1,plain,step,\n", "c1,plain,step,EP-9\n", true)
    );
}

/// モジュールの開始点は、そのモジュールの Step の行に数える。
#[test]
fn author17_module_start_counts_on_module_row() {
    let t = edited(
        "v-ep9e",
        &[(
            "modules/m2/module.yaml",
            "steps:",
            "starts:\n  - {id: \"ms\", node: \"m2a\", requires: {flag: \"f\"}, unmet: \"rj-wrong\"}\nsteps:",
        )],
    );
    let (code, out, _) = vocab(&t.0);
    assert_eq!(code, 0, "{out}");
    assert_eq!(
        out,
        with_ep9("m2,m2a,step,EP-7\n", "m2,m2a,step,EP-7 EP-9\n", false)
    );
}

/// `use` の Step を指す開始点は数えない（`use` 行は拡張点を持たない契約を保つ）。
#[test]
fn author17_start_on_use_step_is_not_counted() {
    let t = edited(
        "v-ep9f",
        &[(
            CHAPTER,
            START,
            "{id: \"s1\", node: \"call\", requires: {flag: \"f\"}, unmet: \"rj-wrong\"}",
        )],
    );
    let (code, out, _) = vocab(&t.0);
    assert_eq!(code, 0, "{out}");
    assert_eq!(out, EXPECTED);
}

/// `confirm.options[].label` は拡張点の番号を持たないため数えない。外しても出力が変わらない。
/// 土台の教材は、`label` の必須化（PACK-37・D-67）に合わせて `label` を持つ。`vocab` は `compile` の結果だけを見るので、
/// `label` を外した教材でも出力を出せる。
#[test]
fn author17_label_not_counted() {
    let t = edited(
        "v-label",
        &[(
            CHAPTER,
            "{id: \"opt-a\", correct: true, label: \"答え A\"}",
            "{id: \"opt-a\", correct: true}",
        )],
    );
    let (code, out, _) = vocab(&t.0);
    assert_eq!(code, 0, "{out}");
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

/// `processing` の要素の `effects[].when` にある `clarifying` も数える（EP-4）。`processing` だけで拡張点を使う Step は
/// （ii）に入り、（i）に残らない。
#[test]
fn author9_processing_effects_when_is_counted() {
    let t = edited(
        "v-proc",
        &[(
            CHAPTER,
            "next: [{to: \"ep1\"}]",
            "processing: [{name: \"pp\", effects: [{set_flag: \"f\", when: {clarifying: \"cl-ask\"}}]}]\n    next: [{to: \"ep1\"}]",
        )],
    );
    let (code, out, err) = vocab(&t.0);
    assert_eq!(code, 0, "{out}");
    assert!(err.is_empty());
    let expected = EXPECTED
        .replace("c1,plain,step,\n", "c1,plain,step,EP-4\n")
        .replace("書けた Step: 3", "書けた Step: 2")
        .replace("書けた Step: 11", "書けた Step: 12")
        .replace("# EP-4 を使った Step: 1", "# EP-4 を使った Step: 2");
    assert_eq!(out, expected);
}
