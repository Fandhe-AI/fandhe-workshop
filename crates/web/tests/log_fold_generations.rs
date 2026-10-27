//! 2 周分（世代をまたぐ）のログの畳みの契約（TASK-232 2/3。FEND-22・D-46・D-47）。
//!
//! 右のログは (世代, Step) ごとにグループを作り、古い側から約 1,000 項目へ畳む。同じ Step の ID が世代 1 と世代 2 の
//! 両方に出る 2 周分の形でも、グループが世代で分かれること、1 リストが目安以下に収まること、最新の行が残ること、
//! 安全弁（4,096 件の打ち切り）にも上流の拒否にも当たらないことを、実際のログの部品の出力で固定する。
//! 試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::list_policy::{LIST_ERROR_ATTR, LIST_ITEMS_TARGET, LIST_LIMIT_ATTR};
use fandhe_workshop_web::parts::Component;

fn begun_host() -> EngineHost {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let bytes =
        std::fs::read(root.join("engine-event-consistency/test-data/min-3step.json")).unwrap();
    let ir: Ir = load_ir(&bytes).unwrap();
    let sample: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("contract/event-sequence.json")).unwrap())
            .unwrap();
    let state: SessionState = serde_json::from_value(sample["initial"].clone()).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

fn go_to(step: &str) -> HostAction {
    HostAction::Engine(Op::Control(Control::GoTo(StepUid(step.to_owned()))))
}

fn text_in() -> HostAction {
    HostAction::Engine(Op::Input(Input::Text("x".to_owned())))
}

fn count(html: &str, needle: &str) -> usize {
    html.matches(needle).count()
}

/// Step と入力回数の列をたどる。Step を移るたびに世代が替わる。
fn walk(plan: &[(&str, usize)]) -> EngineHost {
    let mut host = begun_host();
    for (step, inputs) in plan {
        host.update(go_to(step));
        for _ in 0..inputs.to_owned() {
            host.update(text_in());
        }
    }
    host
}

/// 2 周分: 同じ 3 Step を、世代を替えながら 2 回たどる。各 Step で `inputs` 回の入力をする
/// （入力 1 回で行が 2 件増える）。
fn two_laps(inputs: usize) -> EngineHost {
    let mut host = begun_host();
    for _lap in 0..2 {
        for step in ["main/st-1", "main/st-2", "main/st-3"] {
            host.update(go_to(step));
            for _ in 0..inputs {
                host.update(text_in());
            }
        }
    }
    host
}

/// `<li ...>` の開始タグの列。
fn li_tags(html: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = html[from..].find("<li") {
        let start = from + pos;
        let end = start + html[start..].find('>').unwrap();
        out.push(&html[start..=end]);
        from = end;
    }
    out
}

/// 開始タグのうち `needle` を含むものの数。
fn tags_with(html: &str, needle: &str) -> usize {
    li_tags(html).iter().filter(|t| t.contains(needle)).count()
}

/// 畳みの有無によらず守る性質: 1 リストが目安以下・通知なし・最新の行が残る。
fn assert_bounded(host: &EngineHost) -> String {
    let html = render_component_html(host);
    let list = &html[html.find("part-log-list").unwrap()..];
    let list = &list[..list.find("</ul>").unwrap()];
    assert!(
        count(list, "<li") <= LIST_ITEMS_TARGET,
        "目安を超えた: {}",
        count(list, "<li")
    );
    assert_eq!(count(&html, LIST_ERROR_ATTR), 0, "上流の拒否に当たった");
    assert_eq!(count(&html, LIST_LIMIT_ATTR), 0, "安全弁に当たった");
    let last = host.model().session().unwrap().log().len() - 1;
    let tags = li_tags(list);
    let newest = tags.last().unwrap();
    let event: usize = newest
        .split("data-event=\"")
        .nth(1)
        .and_then(|r| r.split('"').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("最後の項目が行でない: {newest}"));
    // 末尾の数件（受付などの行にならないイベント）を除き、最新の行が残る。
    assert!(event + 4 >= last, "最新の行が残らない: {event} / {last}");
    html
}

#[test]
fn same_step_in_two_generations_is_two_groups() {
    // 畳まれない小さなログ。同じ Step の ID が別の世代で 2 回出る。
    let host = two_laps(2);
    let html = assert_bounded(&host);
    let gens: std::collections::BTreeSet<_> = li_tags(&html)
        .iter()
        .filter(|t| t.contains("data-step=\"main/st-1\""))
        .filter_map(|t| t.split("data-gen=\"").nth(1))
        .map(|r| r.split('"').next().unwrap().to_owned())
        .collect();
    assert!(gens.len() >= 2, "世代が分かれない: {gens:?}");
}

#[test]
fn folded_same_step_summaries_are_split_by_generation() {
    // 同じ Step（st-1）が連続する世代 2 と世代 3 に出て、どちらも古い側で畳まれる。要約は世代ごとに別の行（別のキー）になる。
    let host = walk(&[
        ("main/st-1", 60),
        ("main/st-1", 60),
        ("main/st-2", 1),
        ("main/st-3", 450),
    ]);
    let html = assert_bounded(&host);
    let summaries: Vec<_> = li_tags(&html)
        .into_iter()
        .filter(|t| t.contains("data-fold=\"step\"") && t.contains("data-step=\"main/st-1\""))
        .collect();
    assert_eq!(
        summaries.len(),
        2,
        "世代ごとの要約が 2 行でない: {summaries:?}"
    );
    assert_ne!(summaries[0], summaries[1]);
}

#[test]
fn far_over_safety_limit_stays_folded() {
    // 5,000 行を超える 2 周分でも、目安以下・通知なし・最新の行が残る。
    let host = two_laps(450);
    assert!(host.model().projection().log().len() > 5_000);
    let html = assert_bounded(&host);
    assert!(tags_with(&html, "data-fold=") >= 1);
}
