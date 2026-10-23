//! 右（ターミナル風エリア）の描画が左（成果物エリア）の状態を参照しないことの静的な検査（TASK-206。EVT-1・EVT-23）。
//!
//! 両側は `Session::log()` と `Projection` から別々に導く。右が左の状態（世界状態・印・ビューの選択）を読んだり、
//! 左が右に出した文字列（ログの行・拒否の文面の控え）を読んだりする経路を作ると、
//! 「表示層が描画の結果を読んで状態を作る」経路になるため、ソースの静的な検査で 0 件に固定する。
//!
//! 対象のモジュールは下の固定の一覧で持つ。`host.rs`・`engine_parts.rs` は両側を組み立てる層のため、
//! ファイル単位ではなく impl・関数の単位で検査する（R3）。`a11y`・`list_policy`・`parts`・`profile`・`learner_text`・
//! `doc_view`・`shell*` と試験の設定だけのファイルは、両側が共有する部品または枠のため対象外である。
//! 他の検査との分担: DOM の文字列の読み取りは `tools/check-deps.sh`、状態を変える経路は `no_progress.rs`・
//! `state_paths.rs`、core の依存の閉包は `scripts/check-crate-deps.sh`。手順と限界は `docs/testing.md` 5.273。
//! 解析は `common::spaced`（コメントを除き、識別子どうしの間以外の空白を詰める）で照合する。

mod common;

use common::{
    block_from, count, count_ident, fn_body_spaced, ident_positions, impl_body, spaced,
    web_src_sources,
};

/// 右（ターミナル風エリアの描画）のモジュール。
const RIGHT_FILES: &[&str] = &[
    "terminal_area.rs",
    "log_lines.rs",
    "run_status.rs",
    "step_status.rs",
    "offers.rs",
    "candidates.rs",
    "limit_notice.rs",
    "hello.rs",
];

/// 左（成果物エリアの状態の導出と描画）のモジュール。
const LEFT_FILES: &[&str] = &[
    "artifact_view.rs",
    "artifact_rows.rs",
    "artifact_doc_rows.rs",
    "artifact_mark.rs",
    "app_demo_view.rs",
    "card_view.rs",
    "check_view.rs",
    "diff_view.rs",
    "document_view.rs",
    "file_tree_view.rs",
    "task_tree.rs",
    "tree_attrs.rs",
    "view_select.rs",
];

/// 右が読んではならない左の状態の型・値・関数（識別子の境界で照合する）。
const LEFT_STATE_TOKENS: &[&str] = &[
    "World",
    "Mark",
    "MarkKind",
    ".world(",
    "world_consumed",
    ".marks(",
    "F_ARTIFACT",
    "ViewKind",
    "ViewChoice",
    "view_choice",
    "selected_view",
    "effective_view",
    "replay_changes",
];

/// 左が読んではならない右の描画の語。
const RIGHT_RENDER_TOKENS: &[&str] = &[
    "line_from_event",
    "line_node",
    "judged_text",
    "judged_texts",
    "F_LOG",
    "LogPart",
];

/// 右が左のモジュールから使ってよい、状態でない共有の定数。action の契約の属性名で、`shell` などの枠も使う。
const SHARED_LEFT_ITEMS: &[(&str, &str)] = &[
    ("artifact_view", "ACTION_ATTR"),
    ("artifact_view", "PAYLOAD_ATTR"),
];

/// `engine_parts.rs` の右の impl・補助関数（R3）。
const RIGHT_IMPLS: &[&str] = &["LogPart"];
const RIGHT_FNS: &[&str] = &[
    "run_status_node",
    "step_status_text",
    "step_status_node",
    "secret_choice_list",
];
/// `engine_parts.rs` の左・復習の impl・補助関数（R3）。
const LEFT_IMPLS: &[&str] = &["ArtifactPart", "ReviewPart"];
const LEFT_FNS: &[&str] = &[
    "artifact_rows",
    "change_mark",
    "select_buttons",
    "review_row",
];
const ENGINE_PARTS: &str = "engine_parts.rs";

/// `host.rs` で左の状態（世界状態・消化済みの数・Projection）を書く形（空白を全て除いた形）と、その件数（R4）。
/// 新しい書き込みの形が増えれば失敗する。左の状態がイベント列以外から作られる経路を塞ぐ。
/// `engine_snapshot_json`（test-hooks）は観察の出力で状態を書かないため、ここでは数えない。
const HOST: &str = "host.rs";
const ALLOWED_LEFT_STATE_WRITES: &[(&str, usize)] = &[
    ("model.world=", 1),
    ("model.world_consumed=", 2),
    ("model.projection=", 1),
    (".projection.extend(", 1),
    ("replay_changes(", 3),
    ("&mutself.model.world", 1),
    ("model.world.", 0),
    ("model.projection.", 1),
];

/// 左の状態の欄。複合代入・可変借用でも書けるため、許可した形以外の書き方を 0 件で固定する（R4）。
const LEFT_STATE_FIELDS: &[&str] = &["world", "world_consumed", "projection"];
/// 複合代入の演算子（空白を除いた形。`==` を避けるため `=` を含む形で並べる）。
const COMPOUND_OPS: &[&str] = &["+=", "-=", "*=", "/=", "%=", "|=", "&=", "^=", "<<=", ">>="];

fn tight(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

/// `crate::`・`super::` で始まる経路の先頭の名前と、その後ろの文字列の組を集める。
/// `crate::{a, b::C}` のような波括弧の中は、項目ごとに先頭の名前を求める。
/// モジュール名だけの識別子（同名の関数・変数）は経路ではないため数えない。
fn path_heads(code: &str) -> Vec<(String, String)> {
    let is_id = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = Vec::new();
    for pre in ["crate::", "super::"] {
        for pos in ident_positions(code, pre) {
            let rest = &code[pos + pre.len()..];
            let items: Vec<&str> = if rest.starts_with('{') {
                match block_from(rest, 0) {
                    Some(b) => b[1..b.len() - 1].split(',').collect(),
                    None => vec![rest],
                }
            } else {
                vec![rest]
            };
            for item in items {
                let item = item.trim();
                let head: String = item.chars().take_while(|c| is_id(*c)).collect();
                if !head.is_empty() {
                    out.push((head.clone(), item[head.len()..].to_owned()));
                }
            }
        }
    }
    out
}

/// `code` の中の左のモジュールへの参照が、共有の定数の使用だけであることを調べる。
fn left_module_refs(code: &str, who: &str, v: &mut Vec<String>) {
    for (head, rest) in path_heads(code) {
        let is_left = LEFT_FILES.iter().any(|f| f.trim_end_matches(".rs") == head);
        let ok = rest
            .strip_prefix("::")
            .is_some_and(|r| shared_use(&head, r));
        if is_left && !ok {
            v.push(format!("{who}: 左のモジュール {head} への参照"));
        }
    }
}

/// `::` の後ろ（`rest`）が共有の定数の名前、または共有の定数だけの `{ ... }` か。
fn shared_use(module: &str, rest: &str) -> bool {
    let is_shared = |n: &str| {
        SHARED_LEFT_ITEMS
            .iter()
            .any(|(m, i)| *m == module && *i == n)
    };
    if let Some(inner) = rest.strip_prefix('{') {
        let Some(end) = inner.find('}') else {
            return false;
        };
        let names: Vec<&str> = inner[..end].split(',').map(str::trim).collect();
        return !names.is_empty() && names.iter().all(|n| is_shared(n));
    }
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    is_shared(&name)
}

/// 左のコードが右のモジュールを参照していないことを調べる。
fn right_module_refs(code: &str, who: &str, v: &mut Vec<String>) {
    for (head, _) in path_heads(code) {
        if RIGHT_FILES
            .iter()
            .any(|f| f.trim_end_matches(".rs") == head)
        {
            v.push(format!("{who}: 右のモジュール {head} への参照"));
        }
    }
}

fn token_hits(code: &str, tokens: &[&str], who: &str, kind: &str, v: &mut Vec<String>) {
    for t in tokens {
        let n = count_ident(code, t);
        if n != 0 {
            v.push(format!("{who}: {kind}の語 {t} が {n} 箇所"));
        }
    }
}

/// R1: 右のコードが左の状態を読まない。
fn right_violations(code: &str, who: &str, v: &mut Vec<String>) {
    token_hits(code, LEFT_STATE_TOKENS, who, "左の状態", v);
    left_module_refs(code, who, v);
}

/// R2: 左のコードが右に出した文字列を読まない。
fn left_violations(code: &str, who: &str, v: &mut Vec<String>) {
    token_hits(code, RIGHT_RENDER_TOKENS, who, "右の描画", v);
    right_module_refs(code, who, v);
}

fn find<'a>(files: &'a [(String, String)], name: &str) -> Option<&'a str> {
    files
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, s)| s.as_str())
}

/// 違反を文字列で返す（空なら合格）。対象のファイル・impl・関数を切り出せなければ、それも違反にする（fail-closed）。
fn violations(files: &[(String, String)]) -> Vec<String> {
    let mut v = Vec::new();
    for name in RIGHT_FILES {
        match find(files, name) {
            Some(src) => right_violations(&spaced(src), name, &mut v),
            None => v.push(format!("{name}: 右のファイルが無い")),
        }
    }
    for name in LEFT_FILES {
        match find(files, name) {
            Some(src) => left_violations(&spaced(src), name, &mut v),
            None => v.push(format!("{name}: 左のファイルが無い")),
        }
    }
    match find(files, ENGINE_PARTS) {
        Some(src) => {
            let code = spaced(src);
            for ty in RIGHT_IMPLS {
                let who = format!("{ENGINE_PARTS} の impl {ty}");
                match impl_body(&code, ty) {
                    Some(b) => right_violations(&b, &who, &mut v),
                    None => v.push(format!("{who} を切り出せない")),
                }
            }
            for f in RIGHT_FNS {
                let who = format!("{ENGINE_PARTS} の fn {f}");
                match fn_body_spaced(&code, f) {
                    Some(b) => right_violations(b, &who, &mut v),
                    None => v.push(format!("{who} を切り出せない")),
                }
            }
            for ty in LEFT_IMPLS {
                let who = format!("{ENGINE_PARTS} の impl {ty}");
                match impl_body(&code, ty) {
                    Some(b) => left_violations(&b, &who, &mut v),
                    None => v.push(format!("{who} を切り出せない")),
                }
            }
            for f in LEFT_FNS {
                let who = format!("{ENGINE_PARTS} の fn {f}");
                match fn_body_spaced(&code, f) {
                    Some(b) => left_violations(b, &who, &mut v),
                    None => v.push(format!("{who} を切り出せない")),
                }
            }
        }
        None => v.push(format!("{ENGINE_PARTS}: ファイルが無い")),
    }
    v.extend(host_write_violations(files));
    v
}

/// R4: `host.rs` で左の状態を書く形が、固定の一覧の件数と一致する。
fn host_write_violations(files: &[(String, String)]) -> Vec<String> {
    let Some(src) = find(files, HOST) else {
        return vec![format!("{HOST}: ファイルが無い")];
    };
    let code = tight(&spaced(src));
    let mut v: Vec<String> = ALLOWED_LEFT_STATE_WRITES
        .iter()
        .filter_map(|(pat, want)| {
            let got = count(&code, pat);
            (got != *want).then(|| format!("{HOST}: {pat} が {got} 箇所（{want} 箇所のはず）"))
        })
        .collect();
    // 複合代入と、`replay_changes` 以外への可変借用（`mem::swap`・`take` などの経路）は 1 件も許さない。
    for f in LEFT_STATE_FIELDS {
        let compound = COMPOUND_OPS.iter().map(|op| format!("model.{f}{op}"));
        let borrow = [format!("&mutself.model.{f}")];
        for pat in compound.chain(borrow) {
            // 許可した 1 件（`replay_changes(&mut self.model.world, ..)`）だけは別に数えている。
            let allowed = usize::from(pat == "&mutself.model.world");
            let got = count_borrow_or_op(&code, &pat);
            if got != allowed {
                v.push(format!(
                    "{HOST}: {pat} が {got} 箇所（{allowed} 箇所のはず。複合代入・可変借用）"
                ));
            }
        }
    }
    v
}

/// `pat` が識別子の途中でなく欄の名前の完全一致として現れる件数。`model.world` は `model.world_consumed` を数えない。
fn count_borrow_or_op(code: &str, pat: &str) -> usize {
    code.match_indices(pat)
        .filter(|(i, _)| {
            let after = code[i + pat.len()..].chars().next();
            // 借用の形は名前の直後が識別子の文字なら別の欄（`world` と `world_consumed`）。
            !(pat.starts_with('&') && after.is_some_and(|c| c.is_alphanumeric() || c == '_'))
        })
        .count()
}

fn with(mut v: Vec<(String, String)>, name: &str, src: &str) -> Vec<(String, String)> {
    v.iter_mut().find(|(n, _)| n == name).unwrap().1 = src.to_owned();
    v
}

const GOOD_HOST: &str = r#"
fn derive() {
    replay_changes(&mut world, session.log());
    replay_changes(&mut world, session.log());
}
fn install() {
    self.model.projection = d.projection;
    self.model.world = d.world;
    self.model.world_consumed = d.world_consumed;
}
fn update() {
    self.model.projection.extend(log);
    replay_changes(&mut self.model.world, &log[from..]);
    self.model.world_consumed = log.len();
}
"#;

const GOOD_ENGINE: &str = r#"
fn run_status_node(model: &HostModel) -> Node { Node::empty() }
fn step_status_text(model: &HostModel) -> &'static str { "" }
fn step_status_node(model: &HostModel) -> Node { Node::empty() }
fn secret_choice_list(model: &HostModel) -> Node { Node::empty() }
fn change_mark(kind: MarkKind) -> Option<ChangeMark> { None }
fn artifact_rows(world: &World, marks: &[Mark]) -> Vec<u8> { vec![] }
fn select_buttons(session: &Session, selected: ViewKind) -> Vec<u8> { vec![] }
fn review_row(i: usize) -> u8 { 0 }
impl DisplayPart<HostModel> for LogPart {
    fn view(&self, model: &HostModel) -> Node {
        let line = line_from_event(e, model.judged_text(1));
        line_node(g, line)
    }
}
impl DisplayPart<HostModel> for ArtifactPart {
    fn view(&self, model: &HostModel) -> Node {
        artifact_rows(model.world(), model.projection().marks());
        Node::empty()
    }
}
impl DisplayPart<HostModel> for ReviewPart {
    fn view(&self, model: &HostModel) -> Node { Node::empty() }
}
"#;

/// 合成ソースの基準。実ソースと同じ名前のファイルを空の本体で並べ、`engine_parts.rs`・`host.rs` だけ中身を持つ。
fn base() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = RIGHT_FILES
        .iter()
        .chain(LEFT_FILES)
        .map(|n| ((*n).to_owned(), "fn x() {}".to_owned()))
        .collect();
    v.push((ENGINE_PARTS.to_owned(), GOOD_ENGINE.to_owned()));
    v.push((HOST.to_owned(), GOOD_HOST.to_owned()));
    v
}

#[test]
fn 実ソースの右は左の状態を読まず_左は右の文字列を読まない() {
    let found = web_src_sources();
    assert_eq!(violations(&found), Vec::<String>::new());
}

#[test]
fn 実ソースの一覧のファイルと関数がすべて存在する() {
    let found = web_src_sources();
    for n in RIGHT_FILES
        .iter()
        .chain(LEFT_FILES)
        .chain(&[ENGINE_PARTS, HOST])
    {
        assert!(
            find(&found, n).is_some(),
            "{n} が無い（改名で検査が空振りする）"
        );
    }
    let code = spaced(find(&found, ENGINE_PARTS).unwrap());
    for ty in RIGHT_IMPLS.iter().chain(LEFT_IMPLS) {
        assert!(impl_body(&code, ty).is_some(), "impl {ty} を切り出せない");
    }
    for f in RIGHT_FNS.iter().chain(LEFT_FNS) {
        assert!(fn_body_spaced(&code, f).is_some(), "fn {f} を切り出せない");
    }
}

#[test]
fn 実ソースの右が左から使うのは共有の定数だけ() {
    let found = web_src_sources();
    let offers = spaced(find(&found, "offers.rs").unwrap());
    assert_eq!(count_ident(&offers, "artifact_view"), 1);
}

#[test]
fn 正例_基準の合成ソースは違反なし() {
    assert_eq!(violations(&base()), Vec::<String>::new());
}

#[test]
fn 負例_右に左の状態の語を足すと各語を検出する() {
    for t in LEFT_STATE_TOKENS {
        let extra = format!("fn y() {{ let _ = {t}; }}");
        assert!(
            !violations(&with(base(), "run_status.rs", &extra)).is_empty(),
            "{t} を検出できない"
        );
    }
}

#[test]
fn 負例_左に右の描画の語を足すと各語を検出する() {
    for t in RIGHT_RENDER_TOKENS {
        let extra = format!("fn y() {{ let _ = {t}; }}");
        assert!(
            !violations(&with(base(), "task_tree.rs", &extra)).is_empty(),
            "{t} を検出できない"
        );
    }
}

#[test]
fn 負例_右から左のモジュールを参照すると検出し_共有の定数だけは許す() {
    let ok = "use crate::artifact_view::{ACTION_ATTR, PAYLOAD_ATTR};\nuse crate::artifact_view::ACTION_ATTR;";
    assert!(violations(&with(base(), "offers.rs", ok)).is_empty());
    for bad in [
        "use crate::artifact_view::{ACTION_ATTR, ROW_ATTR};",
        "use crate::artifact_view::ROW_ATTR;",
        "use crate::artifact_view;",
        "use crate::{artifact_view, parts};",
        "use super::artifact_view::ACTION_ATTR_X;",
        "use crate::card_view::Card;",
        "use crate::r#task_tree::Item;",
    ] {
        assert!(
            !violations(&with(base(), "offers.rs", bad)).is_empty(),
            "{bad} を検出できない"
        );
    }
}

#[test]
fn 負例_左から右のモジュールを参照すると検出する() {
    for bad in [
        "use crate::log_lines::line_node;",
        "use super::terminal_area::RunStatus;",
        "use crate::{parts, hello};",
    ] {
        assert!(
            !violations(&with(base(), "card_view.rs", bad)).is_empty(),
            "{bad} を検出できない"
        );
    }
}

#[test]
fn 負例_engine_parts_の右の_impl_と補助関数に左の状態を足すと検出する() {
    let a = GOOD_ENGINE.replace("let line =", "let w = model.world(); let line =");
    assert!(!violations(&with(base(), ENGINE_PARTS, &a)).is_empty());
    let b = GOOD_ENGINE.replace(
        "fn run_status_node(model: &HostModel) -> Node {",
        "fn run_status_node(model: &HostModel) -> Node { let _ = model.projection().marks();",
    );
    assert!(!violations(&with(base(), ENGINE_PARTS, &b)).is_empty());
}

#[test]
fn 負例_engine_parts_の左の_impl_と補助関数に右の語を足すと検出する() {
    let a = GOOD_ENGINE.replace(
        "artifact_rows(model.world(), model.projection().marks());",
        "model.judged_text(1); artifact_rows(model.world(), model.projection().marks());",
    );
    assert!(!violations(&with(base(), ENGINE_PARTS, &a)).is_empty());
    let b = GOOD_ENGINE.replace(
        "fn review_row(i: usize) -> u8 {",
        "fn review_row(i: usize) -> u8 { let _ = line_node;",
    );
    assert!(!violations(&with(base(), ENGINE_PARTS, &b)).is_empty());
}

#[test]
fn 負例_後続の_impl_と_inherent_impl_に置いた左の状態の読み取りも検出する() {
    let right_tail =
        "\nimpl Other for LogPart { fn extra(model: &HostModel) { let _ = model.world(); } }\n";
    let right_inherent =
        "\nimpl LogPart { fn extra(model: &HostModel) { let _ = model.world(); } }\n";
    let left_tail =
        "\nimpl Other for ArtifactPart { fn extra(model: &HostModel) { model.judged_text(1); } }\n";
    let left_inherent =
        "\nimpl ReviewPart { fn extra(model: &HostModel) { model.judged_text(1); } }\n";
    let right_where = "\nimpl LogPart where Self: Sized { fn extra(model: &HostModel) { let _ = model.world(); } }\n";
    let left_where = "\nimpl Other for ArtifactPart where Self: Sized { fn extra(model: &HostModel) { model.judged_text(1); } }\n";
    for tail in [
        right_tail,
        right_inherent,
        left_tail,
        left_inherent,
        right_where,
        left_where,
    ] {
        let src = format!("{GOOD_ENGINE}{tail}");
        assert!(
            !violations(&with(base(), ENGINE_PARTS, &src)).is_empty(),
            "{tail}"
        );
    }
}

#[test]
fn 負例_切り出せない_または_ファイルが無いと失敗する() {
    let renamed = GOOD_ENGINE.replace("for LogPart", "for LogPartX");
    assert!(!violations(&with(base(), ENGINE_PARTS, &renamed)).is_empty());
    let nofn = GOOD_ENGINE.replace("fn secret_choice_list", "fn secret_choice_list2");
    assert!(!violations(&with(base(), ENGINE_PARTS, &nofn)).is_empty());
    let missing: Vec<_> = base()
        .into_iter()
        .filter(|(n, _)| n != "hello.rs")
        .collect();
    assert!(!violations(&missing).is_empty());
}

#[test]
fn 負例_host_で左の状態を書く新しい形を足すと検出する() {
    for extra in [
        "fn z() { self.model.world = World::new(); }",
        "fn z() { replay_changes(&mut self.model.world, &x); }",
        "fn z() { self.model.world_consumed = 0; }",
        "fn z() { self.model.projection = Projection::new(); }",
        "fn z() { self.model.world.clear(); }",
        "fn z() { self.model.world_consumed += 1; }",
        "fn z() { self.model.world_consumed -= 1; }",
        "fn z() { self.model.world_consumed *= 2; }",
        "fn z() { self.model.world_consumed <<= 1; }",
        "fn z() { std::mem::swap(&mut self.model.world_consumed, &mut n); }",
        "fn z() { let p = &mut self.model.projection; }",
        "fn z() { let w = &mut self.model.world; }",
    ] {
        let host = format!("{GOOD_HOST}\n{extra}");
        assert!(
            !violations(&with(base(), HOST, &host)).is_empty(),
            "{extra} を検出できない"
        );
    }
    // 書き込みが 0 件でも失敗する。
    assert!(!violations(&with(base(), HOST, "fn x() {}")).is_empty());
}

#[test]
fn 負例_右が左の表示の選択を読むと各形を検出する() {
    // ViewKind の経路を書かずに、取り込み済みの左のヘルパーを呼ぶ形。
    for extra in [
        "fn y(m: &HostModel) { let _ = m.selected_view(); }",
        "fn y(m: &HostModel) { let _ = view_select::effective_view(None, None); }",
        "fn y(m: &HostModel) { let _ = &m.view_choice; }",
        "fn y(c: ViewChoice) {}",
    ] {
        assert!(
            !violations(&with(base(), "run_status.rs", extra)).is_empty(),
            "{extra} を検出できない（右）"
        );
    }
    let a = GOOD_ENGINE.replace("let line =", "let v = model.selected_view(); let line =");
    assert!(!violations(&with(base(), ENGINE_PARTS, &a)).is_empty());
}

#[test]
fn 負例_コメント内の語は数えない() {
    let src = "// World と model.world() と crate::card_view::X\n/* ViewKind */ fn z() {}";
    assert!(violations(&with(base(), "run_status.rs", src)).is_empty());
    let src = "// line_node と crate::hello::X\nfn z() {}";
    assert!(violations(&with(base(), "task_tree.rs", src)).is_empty());
}

#[test]
fn 負例_識別子の一部は数えない() {
    let src = "fn z() { let WorldX = 1; let MyMark = 2; let marks_len = 3; }";
    assert!(violations(&with(base(), "run_status.rs", src)).is_empty());
}

#[test]
fn 負例_文字列と文字リテラルの波括弧で切り出しが閉じず_後ろの禁止参照を検出する() {
    for lit in [
        r#"let s = "}";"#,
        r#"let s = "}}";"#,
        r#"let s = "\"}";"#,
        "let c = '}';",
        r"let c = '\'';",
        "let s: &'static str = \"{\";",
        r#"let s = "http://x}";"#,
        r#"let s = "/*}";"#,
        r##"let s = r#"/"}}"#;"##,
        r###"let s = r##"a"#}"##;"###,
        r#"let s = br"}";"#,
    ] {
        let mut files = base();
        let parts = files.iter_mut().find(|(n, _)| n == ENGINE_PARTS).unwrap();
        parts.1.push_str(&format!(
            "\nimpl LogPart {{ fn z(&self) {{ {lit} let _ = World; }} }}\n"
        ));
        assert!(
            !violations(&files).is_empty(),
            "{lit} の後ろの禁止参照を検出できない"
        );
    }
}

#[test]
fn 負例_リテラル中の斜線で検出が止まらず_生識別子は除く() {
    let mut files = base();
    let parts = files.iter_mut().find(|(n, _)| n == ENGINE_PARTS).unwrap();
    parts.1.push_str(
        "\nimpl LogPart { fn extra(model: &HostModel) { let s = \"http://x\"; let _ = model.world(); } }\n",
    );
    assert!(
        !violations(&files).is_empty(),
        "リテラル中の // で検出が止まる"
    );
    assert_eq!(
        spaced(r#"let u = "http://a"; r#type"#),
        r#"let u="http://a";type"#
    );
}

#[test]
fn block_fromは文字列と文字リテラルの波括弧を数えない() {
    assert_eq!(block_from(r#"{ "}" }x"#, 0), Some(r#"{ "}" }"#));
    assert_eq!(block_from(r#"{ "{" }x"#, 0), Some(r#"{ "{" }"#));
    assert_eq!(block_from("{ '}' }x", 0), Some("{ '}' }"));
    assert_eq!(
        block_from("{ fn f<'a>(x: &'a u8) {} }x", 0),
        Some("{ fn f<'a>(x: &'a u8) {} }")
    );
}
