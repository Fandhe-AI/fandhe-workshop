//! 復習の畳みを表示部品（`ReviewPart`）まで通して確かめる（TASK-360 1/2。FEND-22・D-46）。
//!
//! 復習の項目は 1 件を 1 グループ（長さ 1）として畳む。1,001 件以上の復習を持つ状態でホストを描き、
//! (1) 要約を含む項目数が目安以下、(2) 最新の項目が残る、(3) 残る行が全体の位置のまま連続する、
//! (4) 古い側が `older` の要約行 1 つに畳まれ件数が合う、(5) 上限の通知が出ない、を確認する。
//! 試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::EngineHost;
use fandhe_workshop_web::list_policy::{LIST_ERROR_ATTR, LIST_ITEMS_TARGET};

/// 達成済みの Step を `n` 個持つログから始めたホスト。Step ごとに復習の項目が 1 件できる。
fn host_with_review(n: usize) -> EngineHost {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let bytes =
        std::fs::read(root.join("engine-event-consistency/test-data/min-3step.json")).unwrap();
    let ir: Ir = load_ir(&bytes).unwrap();
    let sample: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("contract/event-sequence.json")).unwrap())
            .unwrap();
    let mut state: SessionState = serde_json::from_value(sample["initial"].clone()).unwrap();
    for k in 0..n {
        state.log.push(Event::Achieved {
            at: Stamp {
                generation: Generation::INITIAL,
                step: StepUid(format!("main/r{k}")),
            },
            explicit_skip: false,
        });
    }
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

/// 復習の部品（`part-review` の section）の範囲。
fn review_section(html: &str) -> &str {
    let start = html.find("data-testid=\"part-review\"").unwrap();
    let rest = &html[start..];
    let end = rest.find("</section>").unwrap();
    &rest[..end]
}

fn count(s: &str, needle: &str) -> usize {
    s.matches(needle).count()
}

#[test]
fn review_over_target_is_folded_through_the_part() {
    let n = LIST_ITEMS_TARGET + 1;
    let host = host_with_review(n);
    assert_eq!(host.model().projection().review().len(), n, "前提の件数");
    let html = render_component_html(&host);
    let sec = review_section(&html);

    let data = count(sec, "data-step=\"main/r");
    let folds = count(sec, "data-fold=");
    // 要約を含む項目数が目安以下で、古い側は 1 つの要約行に畳まれている。
    assert_eq!(folds, 1, "要約行が 1 つでない");
    assert!(sec.contains("data-fold=\"older\""));
    assert!(
        data + folds <= LIST_ITEMS_TARGET,
        "目安を超えた: {data} + {folds}"
    );
    assert_eq!(count(sec, LIST_ERROR_ATTR), 0, "上限の通知が出た");
    // 畳まれた件数と残る件数の合計が全体に一致する。
    let folded = n - data;
    assert!(
        sec.contains(&format!("data-fold-rows=\"{folded}\"")),
        "畳んだ件数が合わない: {folded}"
    );
    // 最新の項目は残り、最古の項目は畳まれる。
    assert!(sec.contains(&format!("data-step=\"main/r{}\"", n - 1)));
    assert!(!sec.contains("data-step=\"main/r0\""));
    // 残る行は全体の位置のまま古い側の境目から最新まで連続している。
    for i in [folded, folded + 1, n - 1] {
        assert!(
            sec.contains(&format!("data-step=\"main/r{i}\"")),
            "残る行が連続していない: {i}"
        );
    }
    assert!(!sec.contains(&format!("data-step=\"main/r{}\"", folded - 1)));
    // 親の束縛点は 1 つのまま。
    assert_eq!(count(&html, "data-bind-list=\"review_items\""), 1);
}

#[test]
fn review_at_target_is_not_folded() {
    let host = host_with_review(LIST_ITEMS_TARGET);
    let html = render_component_html(&host);
    let sec = review_section(&html);
    assert_eq!(count(sec, "data-fold="), 0);
    assert_eq!(count(sec, "data-step=\"main/r"), LIST_ITEMS_TARGET);
}
