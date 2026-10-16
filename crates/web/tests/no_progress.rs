//! 表示層（`crates/web/src/**`）が進行管理を持たないことの静的な検査（TASK-347 1/2。FEND-2）。
//!
//! 状態を変える経路は、ホストの `update` から core の `Session::apply` を呼ぶ 1 本だけである。
//! 進行の規則（世代・Step の状態・処理中・チェックポイント）は core が持つため、表示層が core の純関数や
//! 状態の型を直に使うことを禁止する。動的な確認（core の純関数との一致）は `host_contract.rs` が持つ。
//! 解析は、コメントを除き、空白と `r#` を除いた文字列で照合する（`common::glue_code` と同じ）。
//! 後続のタスクが `Op::Tick` を作ること（時刻は web が持ち、判定は core が持つ）は禁止しない。

mod common;

use common::{count, fn_body, glue_code, web_src_sources};

/// 禁止の形（正規化後）と、それが守るもの。
const FORBIDDEN: &[(&str, &str)] = &[
    // core の純関数 apply の直接の呼び出し。進行の規則を表示層で再実装する抜け道になる。
    ("session::apply", "core の純関数 apply の経路指定の呼び出し"),
    (
        "fandhe_workshop_core::apply",
        "core の純関数 apply の再公開の呼び出し",
    ),
    // StateChangeProvider・Engine は、状態遷移を直に駆動する core の口。
    ("StateChangeProvider", "状態遷移の提供の口"),
    ("session::Engine", "core の Engine の直接の使用"),
    ("Engine::new", "core の Engine の生成"),
    // 世代の規則の表。世代を表示層が決めない。
    ("GenerationRule", "世代の規則の表"),
    // 状態の取り出しと差し替え。保存復元の担当以外が状態を作り直さない。
    ("Session::from_state", "状態からの Session の再構築"),
    (".into_state(", "状態の取り出し"),
    // 状態の型の構造体リテラル。状態を表示層で組み立てない。
    ("SessionState{", "状態の構造体リテラル"),
    ("RunState{", "実行状態の構造体リテラル"),
    ("Busy{", "処理中の構造体リテラル"),
    ("Slice{", "slice の構造体リテラル"),
    ("EntryCopy{", "入口の写しの構造体リテラル"),
    ("Checkpoint{", "チェックポイントの構造体リテラル"),
    // 状態の持ち出し・差し替え。
    ("mem::take", "状態の持ち出し"),
    ("mem::replace", "状態の差し替え"),
    // core の進行のモジュール。表示層が進行の規則を直に呼ばない。
    ("tick::", "core の tick の規則"),
    ("stop::", "core の stop の規則"),
    ("go_to::", "core の go_to の規則"),
    ("checkpoint::", "core の checkpoint の規則"),
];

/// `.apply(` が許される唯一のファイルと関数。
const APPLY_FILE: &str = "host.rs";
const APPLY_FN: &str = "apply_engine";

/// `.reload(` が許される唯一の関数と、その直前に必要な属性（正規化後）。
const RELOAD_FN: &str = "reload_engine";
const RELOAD_CFG: &str = "#[cfg(feature=\"test-hooks\")]";

/// `Session::from_state` が 1 箇所だけ許されるファイル（試験の設定だけの開始状態。`lib.rs` が feature で囲む）。
const FROM_STATE_FILE: &str = "test_fixture.rs";
const FROM_STATE_PAT: &str = "Session::from_state";

/// `.` の付かない `apply(`（純関数の直接の呼び出し）の数。
fn bare_apply_calls(code: &str) -> usize {
    code.match_indices("apply(")
        .filter(|(i, _)| {
            let before = &code[..*i];
            !before.ends_with('.')
                && !before.ends_with("fn")
                && !before.ends_with("_")
                && !before
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric())
        })
        .count()
}

/// 違反を文字列で返す（空なら合格）。`files` は（名前, ソース）の組。
fn violations(files: &[(String, String)]) -> Vec<String> {
    let mut v = Vec::new();
    let mut apply_sites = 0usize;
    for (name, src) in files {
        let code = glue_code(src);
        for (pat, why) in FORBIDDEN {
            let mut n = count(&code, pat);
            if *pat == FROM_STATE_PAT && name == FROM_STATE_FILE && n == 1 {
                n = 0;
            }
            if n != 0 {
                v.push(format!("{name}: {pat} が {n} 箇所（{why}）"));
            }
        }
        let bare = bare_apply_calls(&code);
        if bare != 0 {
            v.push(format!(
                "{name}: 純関数 apply の直接の呼び出しが {bare} 箇所"
            ));
        }
        let method = count(&code, ".apply(");
        apply_sites += method;
        if method != 0 {
            let in_fn = fn_body(&code, APPLY_FN).map_or(0, |b| count(b, ".apply("));
            if name != APPLY_FILE || in_fn != 1 || method != 1 {
                v.push(format!(
                    "{name}: .apply( が {method} 箇所（{APPLY_FILE} の {APPLY_FN} の 1 箇所だけ）"
                ));
            }
        }
    }
    if apply_sites != 1 {
        v.push(format!(
            ".apply( が全体で {apply_sites} 箇所（1 箇所のはず）"
        ));
    }
    v.extend(reload_violations(files));
    v
}

/// 再読み込みの模擬（`Session::reload`）の呼び出しの違反。`.reload(` は `host.rs` の `reload_engine` の 1 箇所だけで、
/// その関数は `test-hooks` の feature の背後にある（配信する設定には入らない。TASK-221 2/2）。
fn reload_violations(files: &[(String, String)]) -> Vec<String> {
    let mut v = Vec::new();
    let mut sites = 0usize;
    for (name, src) in files {
        let code = glue_code(src);
        let method = count(&code, ".reload(");
        sites += method;
        if method == 0 {
            continue;
        }
        let in_fn = fn_body(&code, RELOAD_FN).map_or(0, |b| count(b, ".reload("));
        if name != APPLY_FILE || in_fn != 1 || method != 1 {
            v.push(format!(
                "{name}: .reload( が {method} 箇所（{APPLY_FILE} の {RELOAD_FN} の 1 箇所だけ）"
            ));
        }
        let gated = code
            .find(&format!("fn{RELOAD_FN}("))
            .is_some_and(|i| code[..i].ends_with(RELOAD_CFG));
        if !gated {
            v.push(format!(
                "{name}: {RELOAD_FN} が test-hooks の feature の背後にない"
            ));
        }
    }
    if sites != 1 {
        v.push(format!(".reload( が全体で {sites} 箇所（1 箇所のはず）"));
    }
    v
}

fn files(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(n, s)| ((*n).to_owned(), (*s).to_owned()))
        .collect()
}

#[test]
fn 開始状態のファイルは_feature_で囲まれている() {
    let found = web_src_sources();
    let lib = found.iter().find(|(n, _)| n == "lib.rs").unwrap();
    assert!(glue_code(&lib.1).contains("#[cfg(feature=\"test-hooks\")]modtest_fixture;"));
}

#[test]
fn 負例_from_state_は例外のファイルの_1_箇所だけ許す() {
    let one = "fn s() { Session::from_state(ir, st) }";
    let two = "fn s() { Session::from_state(a, b); Session::from_state(c, d) }";
    assert!(violations(&files(&[("host.rs", GOOD_HOST), ("test_fixture.rs", one)])).is_empty());
    assert!(!violations(&files(&[("host.rs", GOOD_HOST), ("test_fixture.rs", two)])).is_empty());
    assert!(!violations(&files(&[("host.rs", GOOD_HOST), ("a.rs", one)])).is_empty());
}

#[test]
fn 実ソースに進行管理は無い() {
    let found = web_src_sources();
    assert!(found.iter().any(|(n, _)| n == APPLY_FILE));
    assert_eq!(violations(&found), Vec::<String>::new());
}

const GOOD_HOST: &str = r#"
fn apply_engine(&mut self, op: Op) {
    // session.apply(op) はコメントなので数えない
    let events = session.apply(op);
}
#[cfg(feature = "test-hooks")]
fn reload_engine(&mut self) {
    let events = session.reload();
}
"#;

#[test]
fn 負例_基準となる合成ソースは違反なし() {
    assert_eq!(
        violations(&files(&[("host.rs", GOOD_HOST), ("a.rs", "fn x() {}")])),
        Vec::<String>::new()
    );
}

#[test]
fn 負例_禁止の形を足すと各語を検出する() {
    for (pat, _) in FORBIDDEN {
        let extra = format!("fn y() {{ let _ = {pat}; }}");
        let found = violations(&files(&[("host.rs", GOOD_HOST), ("a.rs", &extra)]));
        assert!(!found.is_empty(), "{pat} を検出できない");
    }
}

#[test]
fn 負例_純関数_apply_の直接の呼び出しを検出する() {
    let found = violations(&files(&[
        ("host.rs", GOOD_HOST),
        ("a.rs", "fn y() { let _ = apply(&ir, s, op); }"),
    ]));
    assert!(!found.is_empty());
}

#[test]
fn 負例_apply_を別の場所で呼ぶと検出する() {
    let found = violations(&files(&[
        ("host.rs", GOOD_HOST),
        ("a.rs", "fn y() { s.apply(op); }"),
    ]));
    assert!(!found.is_empty());
    let twice = GOOD_HOST.replace("let events", "session.apply(op); let events");
    assert!(!violations(&files(&[("host.rs", &twice)])).is_empty());
}

#[test]
fn 実ソースの_reload_は_test_hooks_の背後の_1_箇所だけ() {
    let found = web_src_sources();
    let host = found.iter().find(|(n, _)| n == APPLY_FILE).unwrap();
    assert_eq!(count(&glue_code(&host.1), ".reload("), 1);
    assert_eq!(reload_violations(&found), Vec::<String>::new());
}

const NO_RELOAD_HOST: &str = "fn apply_engine() { s.apply(op); }";

#[test]
fn 負例_reload_の良い例は違反なし() {
    assert_eq!(
        violations(&files(&[("host.rs", GOOD_HOST)])),
        Vec::<String>::new()
    );
}

#[test]
fn 負例_reload_を別の場所や_feature_なしで呼ぶと検出する() {
    // 別のファイル。
    let other = files(&[("host.rs", GOOD_HOST), ("a.rs", "fn y() { s.reload(); }")]);
    assert!(!violations(&other).is_empty());
    // 同じ関数で 2 回。
    let twice = GOOD_HOST.replace("session.reload();", "session.reload(); session.reload();");
    assert!(!violations(&files(&[("host.rs", &twice)])).is_empty());
    // 別の関数。
    let moved = GOOD_HOST.replace("fn reload_engine", "fn other_engine");
    assert!(!violations(&files(&[("host.rs", &moved)])).is_empty());
    // feature の背後にない。
    let ungated = GOOD_HOST.replace("#[cfg(feature = \"test-hooks\")]\n", "");
    assert!(!violations(&files(&[("host.rs", &ungated)])).is_empty());
    // 呼び出しが無くなっても全体の件数が合わず検出する。
    assert!(!violations(&files(&[("host.rs", NO_RELOAD_HOST)])).is_empty());
}

#[test]
fn 負例_コメント内の禁止の形は数えない() {
    let src = "// Session::from_state と mem::take\n/* apply(x) */ fn z() {}";
    assert_eq!(
        violations(&files(&[("host.rs", GOOD_HOST), ("a.rs", src)])),
        Vec::<String>::new()
    );
}
