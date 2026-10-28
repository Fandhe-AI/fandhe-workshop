//! モジュールの 1 か所の修正が全実体に反映されることの受け入れ試験（TASK-246。MOD-13。記録は `docs/testing.md` の 5.337）。
//!
//! `module-reuse-rounds` のモジュール `issue-to-merge` は 1・2 周目で 2 回実体化される。`st-6-1` の規則 `r-new-root` が子 Issue を
//! 作る `put` の 1 行（`module.yaml` の 138 行目）に `label: tracked` を足すだけで、次の 4 点が成り立つことを確かめる。
//! (1) 直したのは 1 ファイルの 1 行だけで、同じ書き方の別の行（途中開始 `s-7-1` の 33 行目）と `testdata` は変わらない。
//! (2) 直す前後の IR の差は `issue-to-merge@round1/st-6-1` と `issue-to-merge@round2/st-6-1` の 2 つの Step の実体だけで、
//!     その差も `r-new-root` の子 Issue の `put` の `attrs` に `label: tracked` が増えただけである。
//! (3) 1 周目から 2 周目まで流すと、世界状態の `issues/child-1` と `issues/child-9` が両方 `label=tracked` を持つ。
//! (4) `validate` の診断は前後で変わらない。
//!
//! 診断の読み替え: spec の写しは `label` の必須化（PACK-37）と展開後の `do` の漏れの検査（PACK-52）より前の形で、生の診断が
//! 前後とも 14 件ある。この 2 種を除くと 0 件になるため、「診断 0 件」は「除いた残りが 0 件で、前後の一覧が等しい」と読み替える。
//! 入力列は `common/rounds.rs` の計画を使い、未実装の部品のため `GoTo` で飛ばす箇所がある（5.258・5.260）。
//! core 側ではなく pack 側に置くのは、形式 B の教材のコンパイルが pack にしかなく、core から pack へは依存できないため。
//! 呼び出し元は CI の `make test` だけである。

mod common;
#[path = "common/rounds.rs"]
mod rounds;
#[path = "common/sha256.rs"]
mod sha256;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use common::{
    copy_dir, files_of, testdata, without_answer_label_required, without_expanded_do_leak, Tmp,
};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::Session;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use rounds::{begin_round2_plan, round1_plan, round2_plan, run_plan};
use serde_json::Value;

const PACK: &str = "module-reuse-rounds/test-data/pack";
const MODULE: &str = "modules/issue-to-merge/module.yaml";
const UIDS: [&str; 2] = [
    "issue-to-merge@round1/st-6-1",
    "issue-to-merge@round2/st-6-1",
];
/// 138 行目（`st-6-1` の `r-new-root`。行頭の空白 12 個）の元の行。
const OLD_LINE: &str = "            - put: { path: \"issues/child-${child}\", fixture: fx-kakuu-issue-child, attrs: { state: open } }";
const NEW_LINE: &str = "            - put: { path: \"issues/child-${child}\", fixture: fx-kakuu-issue-child, attrs: { state: open, label: tracked } }";

/// `r-new-root` の子 Issue の `put` に `label: tracked` を足したパックの写し。`testdata` は読むだけで書き換えない。
fn edited_pack() -> Tmp {
    let t = Tmp::new("one-edit");
    copy_dir(&testdata(PACK), &t.0);
    let text = fs::read_to_string(t.0.join(MODULE)).expect("read module");
    // 空白を除くと同じ書き方の行は 2 つある（33 行目と 138 行目）。行頭の空白 12 個の方だけが直す対象。
    let same_shape = text.lines().filter(|l| l.trim() == OLD_LINE.trim()).count();
    assert_eq!(same_shape, 2, "同じ書き方の行の数が想定と違う");
    assert_eq!(
        text.matches(OLD_LINE).count(),
        1,
        "直す行がちょうど 1 つでない"
    );
    let edited = text.replacen(OLD_LINE, NEW_LINE, 1);
    t.put(MODULE, edited.as_bytes());
    t
}

/// 相対パス（`/` 区切り）から SHA-256 の 16 進への対応。
fn manifest(dir: &Path) -> BTreeMap<String, String> {
    let mut files = Vec::new();
    files_of(dir, dir, &mut files);
    files
        .into_iter()
        .map(|rel| {
            let bytes = fs::read(dir.join(&rel)).expect("read");
            let key = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            (key, sha256::hex(&sha256::sha256(&bytes)))
        })
        .collect()
}

fn ir_of(dir: &Path) -> Ir {
    load_ir(&compile_dir(dir).expect("compile")).expect("load_ir")
}

/// 1 周目から 2 周目の終わりまで流した状態と、1 周目の終わりの状態、全イベント。
fn run_two_rounds(ir: &Ir) -> (Session, Session, Vec<Event>) {
    let mut s = Session::new(ir.clone());
    let mut ev = run_plan(&mut s, &round1_plan(ir));
    let mid = s.clone();
    ev.extend(run_plan(&mut s, &begin_round2_plan()));
    ev.extend(run_plan(&mut s, &round2_plan(ir)));
    (s, mid, ev)
}

fn attr_of<'a>(s: &'a Session, path: &str, name: &str) -> Option<&'a str> {
    s.state()
        .run
        .world
        .get(path)
        .and_then(|e| e.attrs.get(name))
        .map(String::as_str)
}

/// ノードの JSON のうち、`r-new-root` の子 Issue の `put` の `attrs` へ `label: tracked` を足した値。
fn with_label(node: &Value) -> Value {
    let mut v = node.clone();
    let rules = v["step"]["rules"].as_array_mut().expect("rules");
    let rule = rules
        .iter_mut()
        .find(|r| r["id"] == "r-new-root")
        .expect("r-new-root");
    let effects = rule["outcome"]["effects"].as_array_mut().expect("effects");
    let mut hit = 0;
    for e in effects {
        let path = e["put"]["path"].as_str().unwrap_or("");
        if path.starts_with("issues/child-") {
            e["put"]["attrs"]["label"] = Value::String("tracked".into());
            hit += 1;
        }
    }
    assert_eq!(hit, 1, "子 Issue の put がちょうど 1 つでない");
    v
}

#[test]
fn mod13_edit_touches_one_line_of_one_file() {
    let src = testdata(PACK);
    let before = manifest(&src);
    let t = edited_pack();
    let after = manifest(&t.0);
    assert_eq!(before.len(), 44);
    assert_eq!(after.len(), 44);
    let changed: Vec<&String> = before
        .keys()
        .filter(|k| before.get(*k) != after.get(*k))
        .collect();
    assert_eq!(changed, vec![MODULE], "変わったファイルが 1 件でない");
    let old = fs::read_to_string(src.join(MODULE)).expect("read");
    let new = fs::read_to_string(t.0.join(MODULE)).expect("read");
    let (ol, nl): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    assert_eq!(ol.len(), nl.len());
    let diff: Vec<usize> = (0..ol.len()).filter(|&i| ol[i] != nl[i]).collect();
    assert_eq!(diff, vec![137], "変わった行が 138 行目だけでない");
    assert_eq!(ol[32], nl[32], "33 行目（途中開始）を直してはいけない");
    // 試験の実行が testdata を書き換えない。
    let _ = ir_of(&t.0);
    let _ = validate_dir(&t.0);
    assert_eq!(manifest(&src), before);
}

#[test]
fn mod13_ir_differs_only_in_two_step_instances() {
    let t = edited_pack();
    let before = ir_of(&testdata(PACK));
    let after = ir_of(&t.0);
    // 空振りの防止。
    assert_eq!(before.instances.len(), 5);
    assert_eq!(before.nodes.len(), 36);
    assert_eq!(before.starts.len(), 19);
    let mut jb = serde_json::to_value(&before).expect("json");
    let mut ja = serde_json::to_value(&after).expect("json");
    let nb = jb["nodes"].take();
    let na = ja["nodes"].take();
    assert_eq!(
        jb, ja,
        "nodes 以外（実体・開始点・練習データなど）が変わった"
    );
    let nb = nb.as_array().expect("nodes");
    let na = na.as_array().expect("nodes");
    assert_eq!(nb.len(), na.len());
    let mut differing = BTreeSet::new();
    for (b, a) in nb.iter().zip(na) {
        assert_eq!(b["uid"], a["uid"], "節点の並びが変わった");
        if b != a {
            differing.insert(b["uid"].as_str().expect("uid").to_string());
            // 差は label を足しただけである。
            assert_eq!(*a, with_label(b), "{} の差が狙った属性だけでない", b["uid"]);
        }
    }
    let want: BTreeSet<String> = UIDS.iter().map(|s| s.to_string()).collect();
    assert_eq!(differing, want);
    // 33 行目（途中開始 s-7-1）は直していないので、開始点に label は出ない。
    let starts = serde_json::to_string(&after.starts).expect("json");
    assert!(!starts.contains("tracked"), "開始点の効果に label が入った");
}

#[test]
fn mod13_edit_reaches_both_rounds_at_runtime() {
    let t = edited_pack();
    let ir = ir_of(&t.0);
    let (s, mid, ev) = run_two_rounds(&ir);
    // 1 周目の終わりでは child-1 だけが label を持つ（child-9 は 2 周目の実体が書く）。
    assert_eq!(attr_of(&mid, "issues/child-1", "label"), Some("tracked"));
    assert!(!mid.state().run.world.contains_key("issues/child-9"));
    for p in ["issues/child-1", "issues/child-9"] {
        assert_eq!(attr_of(&s, p, "label"), Some("tracked"), "{p}");
        assert_eq!(attr_of(&s, p, "state"), Some("closed"), "{p}");
    }
    // 対照: 元の教材では label が付かない。
    let orig = ir_of(&testdata(PACK));
    let (so, _, evo) = run_two_rounds(&orig);
    for p in ["issues/child-1", "issues/child-9"] {
        assert_eq!(attr_of(&so, p, "label"), None, "{p}");
        assert_eq!(attr_of(&so, p, "state"), Some("closed"), "{p}");
    }
    // 差はこの 2 経路の変更イベントと世界状態だけである。
    assert_eq!(ev.len(), evo.len());
    let mut changed = 0;
    for (a, b) in ev.iter().zip(&evo) {
        if a != b {
            match a {
                Event::Changed { path, .. } => {
                    assert!(
                        path == "issues/child-1" || path == "issues/child-9",
                        "{path}"
                    );
                    changed += 1;
                }
                other => panic!("変更イベント以外が違う: {other:?}"),
            }
        }
    }
    assert!(changed >= 2, "違うイベントが {changed} 件しかない");
    let (wa, wo) = (&s.state().run.world, &so.state().run.world);
    assert_eq!(wa.keys().collect::<Vec<_>>(), wo.keys().collect::<Vec<_>>());
    for (k, e) in wa {
        if k != "issues/child-1" && k != "issues/child-9" {
            assert_eq!(Some(e), wo.get(k), "{k}");
        }
    }
    assert_eq!(s.state().run.current, so.state().run.current);
}

#[test]
fn mod13_validate_stays_clean() {
    let t = edited_pack();
    let d_before = validate_dir(&testdata(PACK));
    let d_after = validate_dir(&t.0);
    assert_eq!(d_before, d_after, "修正で診断の一覧が変わった");
    let (rest_b, label_b) = without_answer_label_required(d_before);
    let (rest_b, leak_b) = without_expanded_do_leak(rest_b);
    let (rest_a, label_a) = without_answer_label_required(d_after);
    let (rest_a, leak_a) = without_expanded_do_leak(rest_a);
    assert!(rest_b.is_empty(), "{rest_b:?}");
    assert!(rest_a.is_empty(), "{rest_a:?}");
    assert_eq!((label_b, leak_b), (label_a, leak_a));
    assert!(label_a > 0 && leak_a > 0);
}
