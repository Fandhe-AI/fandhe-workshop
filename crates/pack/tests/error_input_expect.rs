//! 教材テストの誤入力の期待の読み取りと、拒否の文面の検査の試験（PACK-11、TASK-45 2/2）。
//!
//! 確かめること: `expect` の書き方 3 形が contract の判定の型で読めること、読めない書き方が `E-PARSE` になり値を出さないこと、
//! 架空の教材の誤入力 20 件の分類、ST-1.4・ST-7.1・ST-7.3 相当の 3 場面の区別、拒否の文面と次の行動が空でないこと。
//! 確かめないこと: 誤入力をエンジンへ流して実際の判定と期待を比べること（`fandhe-pack test`、TASK-404。
//! core の入力の判定は TASK-267 待ち）。3 場面の区別は、型の上での区別である。
//! 一時ディレクトリは `CARGO_TARGET_TMPDIR` の下に作る。

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use common::{assert_no_secret, testdata, Tmp, SECRET, WORKSHOP};

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::{
    AcceptNote, Category, Clarification, Judgment, JudgmentKind, NextAction, RejectCategory,
    Rejection,
};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::play_expect::{
    check_error_input_expects, parse_expect, read_error_input_expects, resolve_rejection,
    ExpectError, ResultExpect,
};
use fandhe_workshop_pack::source;
use fandhe_workshop_pack::vocab::rejection_category;
use fandhe_workshop_pack::DiagCode;
use serde_json::Value;

fn judgment_pack() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/judgment-pack")
}

fn public_packs() -> Vec<(&'static str, PathBuf, usize)> {
    vec![
        (
            "min-3step-pack",
            testdata("engine-event-consistency/test-data/min-3step-pack"),
            7,
        ),
        (
            "module-reuse-rounds",
            testdata("module-reuse-rounds/test-data/pack"),
            8,
        ),
        (
            "min-3step-pack-b",
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/min-3step-pack-b"),
            1,
        ),
    ]
}

fn compile_ir(dir: &Path) -> Ir {
    let bytes = compile_dir(dir).unwrap_or_else(|d| panic!("compile {dir:?}: {d:?}"));
    serde_json::from_slice(&bytes).expect("ir")
}

fn next_action() -> NextAction {
    NextAction::new("次の行動").expect("non-empty")
}

#[test]
fn pack11_expect_grammar_accepts_three_results() {
    let plain = parse_expect("accepted").expect("accepted");
    assert_eq!(plain, ResultExpect::Accepted { free_text: false });
    assert_eq!(plain.kind(), JudgmentKind::Accepted);
    assert_eq!(plain.category(), None);

    let free = parse_expect("accepted:free_text_unjudged").expect("free text");
    assert_eq!(free, ResultExpect::Accepted { free_text: true });
    assert_eq!(free.kind(), JudgmentKind::Accepted);
    assert_eq!(free.category(), Some(Category::FreeTextUnjudged));

    for c in RejectCategory::ALL {
        let e = parse_expect(&format!("rejected:{}", c.as_str()))
            .unwrap_or_else(|_| panic!("{}", c.as_str()));
        assert_eq!(e, ResultExpect::Rejected(c));
        assert_eq!(e.kind(), JudgmentKind::Rejected);
        assert_eq!(e.category(), Some(Category::from(c)));
    }

    let ask = parse_expect("clarified").expect("clarified");
    assert_eq!(ask, ResultExpect::Clarified);
    assert_eq!(ask.kind(), JudgmentKind::Clarified);
    assert_eq!(ask.category(), None);
}

#[test]
fn pack11_expect_grammar_rejects_malformed() {
    let bad = [
        "rejected",
        "rejected:free_text_unjudged",
        "accepted:busy",
        "clarified:busy",
        "clarified:",
        "confirmed",
        "progressed",
        "ignored",
        "progress",
        "achieved",
        "Rejected:busy",
        " rejected:busy",
        "rejected:busy ",
        "rejected:Busy",
        "rejected:unknown",
        "rejected:",
        "rejected:busy:x",
        "",
    ];
    for s in bad {
        assert!(parse_expect(s).is_err(), "{s:?} must be rejected");
    }
    assert_eq!(parse_expect("rejected"), Err(ExpectError::MissingCategory));
    assert_eq!(
        parse_expect("rejected:free_text_unjudged"),
        Err(ExpectError::NotARejection)
    );
    assert_eq!(parse_expect("confirmed"), Err(ExpectError::UnknownResult));
    // 固定文の Display は入力値を含まない。
    assert!(!ExpectError::UnknownCategory.to_string().contains("busy"));
}

/// 型の上での 3 場面の区別。エンジンの判定ではなく、期待と判定の型の照合規則を確かめる。
#[test]
fn pack11_three_scenes_are_distinct() {
    let clarified = Judgment::Clarified(Clarification {
        text: "cl".into(),
        next_action: next_action(),
    });
    let missing = Judgment::Rejected(Rejection {
        category: RejectCategory::MissingArg,
        text: "rj".into(),
        next_action: next_action(),
    });
    let accepted = Judgment::Accepted(None);
    let noted = Judgment::Accepted(Some(AcceptNote {
        text: "rj".into(),
        next_action: next_action(),
    }));

    let expects = [
        parse_expect("clarified").expect("e"),
        parse_expect("rejected:missing_arg").expect("e"),
        parse_expect("accepted").expect("e"),
    ];
    for (i, j) in [&clarified, &missing, &accepted].into_iter().enumerate() {
        let hits: Vec<usize> = expects
            .iter()
            .enumerate()
            .filter(|(_, e)| e.matches(j))
            .map(|(k, _)| k)
            .collect();
        assert_eq!(hits, vec![i], "{j:?}");
    }
    // 自然文の受付の注記の有無は区別される。
    let free = parse_expect("accepted:free_text_unjudged").expect("e");
    assert!(free.matches(&noted) && !free.matches(&accepted));
    assert!(expects[2].matches(&accepted) && !expects[2].matches(&noted));
    // 別の分類の拒否には一致しない。
    assert!(!parse_expect("rejected:busy").expect("e").matches(&missing));
    // Confirmed・Progressed・Ignored はどの期待にも一致しない。
    for j in [Judgment::Confirmed, Judgment::Progressed, Judgment::Ignored] {
        assert!(expects.iter().all(|e| !e.matches(&j)), "{j:?}");
        assert!(!free.matches(&j));
    }
}

fn expect_table(pack: &Path) -> Vec<(String, ResultExpect)> {
    let lp = source::load(pack).expect("load");
    read_error_input_expects(&lp).expect("expects")
}

#[test]
fn pack11_judgment_pack_classifies_20_error_inputs() {
    let lp = source::load(&judgment_pack()).expect("load");
    assert!(check_error_input_expects(&lp).is_empty());
    let expects = read_error_input_expects(&lp).expect("expects");
    assert_eq!(expects.len(), 20);

    let mut table: BTreeMap<String, usize> = BTreeMap::new();
    for (_, e) in &expects {
        let key = match e {
            ResultExpect::Accepted { free_text: false } => "accepted".to_string(),
            ResultExpect::Accepted { free_text: true } => "accepted:free_text_unjudged".to_string(),
            ResultExpect::Rejected(c) => format!("rejected:{}", c.as_str()),
            ResultExpect::Clarified => "clarified".to_string(),
        };
        *table.entry(key).or_default() += 1;
    }
    let want: BTreeMap<String, usize> = [
        ("accepted", 1),
        ("accepted:free_text_unjudged", 1),
        ("clarified", 1),
        ("rejected:unknown_command", 3),
        ("rejected:wrong_order", 2),
        ("rejected:missing_arg", 1),
        ("rejected:bad_arg_shape", 2),
        ("rejected:busy", 2),
        ("rejected:not_in_this_step", 5),
        ("rejected:not_supported", 2),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    assert_eq!(table, want);

    let covered: BTreeSet<&str> = expects
        .iter()
        .filter_map(|(_, e)| match e {
            ResultExpect::Rejected(c) => Some(c.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(covered.len(), RejectCategory::ALL.len());
}

#[test]
fn pack11_judgment_pack_scenes() {
    let table: BTreeMap<String, ResultExpect> =
        expect_table(&judgment_pack()).into_iter().collect();
    assert_eq!(table["e-clarify"], ResultExpect::Clarified);
    assert_eq!(
        table["e-missing-arg"],
        ResultExpect::Rejected(RejectCategory::MissingArg)
    );
    assert_eq!(
        table["e-accepted"],
        ResultExpect::Accepted { free_text: false }
    );

    // ST-7.1 相当と ST-7.3 相当は、同じ入力で開始点だけが違う。
    let lp = source::load(&judgment_pack()).expect("load");
    let tests = lp.tests.as_ref().expect("tests");
    let find = |id: &str| -> &Value {
        tests
            .error_inputs
            .iter()
            .find(|v| v.get("id").and_then(Value::as_str) == Some(id))
            .expect("error input")
    };
    let (a, b) = (find("e-missing-arg"), find("e-accepted"));
    assert_eq!(a["input"]["cmd"], b["input"]["cmd"]);
    assert_ne!(a["start"], b["start"]);
}

fn assert_filled(label: &str, value: &Value) {
    let msg = value.get("message").and_then(Value::as_str).unwrap_or("");
    assert!(!msg.trim().is_empty(), "{label}: message is empty");
    let next = value
        .get("next_action")
        .and_then(Value::as_str)
        .unwrap_or("");
    assert!(
        NextAction::new(next).is_ok(),
        "{label}: next_action is empty"
    );
}

#[test]
fn pack11_rejections_have_message_and_next_action() {
    let mut packs = public_packs();
    packs.push(("judgment-pack", judgment_pack(), 20));
    for (name, dir, _) in packs {
        let ir = compile_ir(&dir);
        for (id, value) in &ir.rejections {
            let category = value
                .get("category")
                .and_then(Value::as_str)
                .and_then(rejection_category)
                .expect("category");
            if category.is_rejection() {
                assert_filled(&format!("{name}: {id}"), value);
            }
        }
        let lp = source::load(&dir).expect("load");
        let tests = lp.tests.as_ref().expect("tests");
        let expects = read_error_input_expects(&lp).expect("expects");
        for ((id, e), raw) in expects.iter().zip(&tests.error_inputs) {
            let ResultExpect::Rejected(c) = e else {
                continue;
            };
            let start = raw.get("start").and_then(Value::as_str).expect("start");
            let (text_id, value) = resolve_rejection(&ir, start, *c)
                .unwrap_or_else(|| panic!("{name}: {id}: no rejection text"));
            assert_filled(&format!("{name}: {id} -> {text_id}"), value);
        }
    }

    // ST-7.1 相当は、既定ではなく Step 専用の文面に解決される。
    let ir = compile_ir(&judgment_pack());
    let (id, _) = resolve_rejection(&ir, "s-two-args", RejectCategory::MissingArg).expect("text");
    assert_eq!(id, "rj-run-missing-mode");
    let (id, _) = resolve_rejection(&ir, "s-one-arg", RejectCategory::MissingArg).expect("text");
    assert_eq!(id, "rj-default-missing-arg");
    assert!(resolve_rejection(&ir, "no-such-start", RejectCategory::Busy).is_none());
}

#[test]
fn pack11_existing_error_inputs_parse() {
    for (name, dir, count) in public_packs() {
        let lp = source::load(&dir).expect("load");
        assert!(check_error_input_expects(&lp).is_empty(), "{name}");
        assert_eq!(expect_table(&dir).len(), count, "{name}");
    }
}

#[test]
fn pack11_error_input_expect_diagnostics() {
    let t = Tmp::new("p11-diag");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put("chapters/c1/chapter.yaml", common::CHAPTER.as_bytes());
    t.put("chapters/c1/a.md", format!("# a\n{SECRET}\n").as_bytes());
    let tests = format!(
        concat!(
            "playthroughs:\n  - {{id: pt, segments: [{{steps: [{{cmd: x, expect: progress}}]}}]}}\n",
            "error_inputs:\n",
            "  - {{id: ok, start: s1, input: {{cmd: /x, expect: 'rejected:busy'}}}}\n",
            "  - {{id: e1, start: s1, input: {{cmd: /x, expect: 'rejected:unknown'}}}}\n",
            "  - {{id: e2, start: s1, input: {{cmd: /x, expect: 'rejected:free_text_unjudged'}}}}\n",
            "  - {{id: e3, start: s1, input: {{cmd: /x}}}}\n",
            "  - {{id: e4, start: s1, input: {{cmd: /x, expect: 5}}}}\n",
            "  - {{id: e5, start: s1, input: {{cmd: /x, expect: '{secret}'}}}}\n",
            "  - {{id: 'a b', start: s1, input: {{cmd: /x, expect: rejected}}}}\n",
            "  - {{id: e7, start: s1, pre: [{{cmd: /y, expect: progress}}], input: {{cmd: /x, expect: ok}}}}\n",
        ),
        secret = SECRET
    );
    t.put("tests/playthrough.yaml", tests.as_bytes());
    let lp = source::load(&t.0).expect("load");
    let d = check_error_input_expects(&lp);
    let items: Vec<&str> = d.iter().map(|x| x.item()).collect();
    assert_eq!(
        items,
        [
            "error_input:e1",
            "error_input:e2",
            "error_input:e3",
            "error_input:e4",
            "error_input:e5",
            "error_input:#7",
            "error_input:e7",
        ]
    );
    for x in &d {
        assert_eq!(x.code(), DiagCode::Parse, "{x}");
        assert_eq!(x.file(), "tests/playthrough.yaml");
    }
    assert_no_secret(&d);
    assert_eq!(
        read_error_input_expects(&lp).err().map(|e| e.len()),
        Some(7)
    );
}

#[test]
fn pack11_no_tests_file_is_empty() {
    let t = common::minimal();
    let lp = source::load(&t.0).expect("load");
    assert!(lp.tests.is_none());
    assert!(check_error_input_expects(&lp).is_empty());
    assert_eq!(read_error_input_expects(&lp), Ok(Vec::new()));
}
