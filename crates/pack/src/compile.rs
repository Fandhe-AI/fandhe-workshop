//! 読み込んだ教材パック（`LoadedPack`）から IR を作る。
//!
//! `compile` はメモリ上の値だけを見る純関数で、ファイルシステムには触れない（ディスクに触れるのは
//! `source::load`）。出力は決定的である。連想配列は `BTreeMap`、章は名前の昇順、Step はファイルの中の順で
//! 並べ、絶対パス・時刻・乱数を入れない。`compare` が同じ `compile_dir` のバイト列を比べる。
//!
//! 本文（`body`・`body_levels`）と練習データの内容は、Markdown から安全な木（[`crate::markdown`]）にして
//! IR に載せる（TASK-60、方式 (c)）。`.md` の練習データは Markdown として、それ以外は解釈せずに全文を
//! 1 つのコードブロックにする（`kind` は語彙として開いているため、拡張子で分ける）。`step.body`（参照の
//! 文字列）は残す。
//!
//! 出力の `ir_format` は、宣言した値ではなく、使った項目に足りる最小の版（[`Ir::required_ir_format`]）にする
//! （D-52）。木を使わない教材パックは版 1 のまま、版 1 とバイト単位で同じ IR になる。`workshop.*` に書ける
//! `ir_format` は 1 か 2（PACK-24 の読み）で、それ以外は `E-FORMAT-VERSION` で止める。
//! Step の `input` の宣言は [`crate::input`] が検査する（IR には宣言の値をそのまま載せる）。
//! モジュールの展開は TASK-50 が扱うため、`use` を持つ Step は拒否する。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fandhe_workshop_contract::doc::Doc;
use fandhe_workshop_contract::ir::{is_supported_ir_format, Instance, Ir, Node, Start};
use fandhe_workshop_contract::limits::MAX_IR_BYTES;
use fandhe_workshop_core::check_ir_limits;

use crate::diag::{DiagCode, Diagnostic};
use crate::effect_check::check_effects;
use crate::input;
use crate::markdown;
use crate::source::{self, LoadedPack, ScopeKind};

fn diag(file: &str, item: &str, code: DiagCode, message: impl Into<String>) -> Diagnostic {
    let file = if file.trim().is_empty() { "." } else { file };
    let item = if item.trim().is_empty() { "pack" } else { item };
    // file と item を空でない値に直したので、Diagnostic::new は失敗しない。
    Diagnostic::new(file, item, code, message).expect("file and item are non-empty")
}

/// 読み込んだパックを IR にする。診断が 1 件でもあれば IR は返さない。
pub fn compile(pack: &LoadedPack) -> Result<Ir, Vec<Diagnostic>> {
    let w = &pack.workshop;
    if !is_supported_ir_format(w.pack.ir_format) {
        return Err(vec![diag(
            &pack.workshop_file,
            "pack:ir_format",
            DiagCode::FormatVersion,
            format!(
                "ir_format {} is not supported (expected 1 or 2)",
                w.pack.ir_format
            ),
        )]);
    }

    let (_profile, profile_diags) = crate::profile::check_profile(&pack.workshop_file, w);
    let mut diags = profile_diags;
    let mut nodes: Vec<Node> = Vec::new();
    let mut starts: Vec<Start> = Vec::new();
    let mut uids: BTreeSet<String> = BTreeSet::new();
    // 遷移先の検査対象。node 化しない `use` の Step も含める。file・item・書き換え前の遷移先 ID を保持する。
    let mut checks: Vec<(String, String, Vec<String>)> = Vec::new();
    let mut start_ids: BTreeSet<String> = BTreeSet::new();
    let mut chapters = 0usize;
    // 木にする本文・練習データの元の量の合計。同じ本文を多数の Step が参照しても、木の確保が出力の
    // 上限を超えて増えないようにする（`check_output` は全部を作った後に走るため、それでは間に合わない）。
    let mut budget = Budget::new(MAX_IR_BYTES);
    let mut over_budget = false;

    for scope in pack.scopes.iter().filter(|s| s.kind == ScopeKind::Chapter) {
        chapters += 1;
        for step in &scope.src.steps {
            let item = format!("step:{}", step.id);
            let uid = format!("main/{}", step.id);
            if !uids.insert(uid.clone()) {
                diags.push(diag(
                    &scope.file,
                    &item,
                    DiagCode::DupId,
                    format!("duplicate step id: {}", step.id),
                ));
                continue;
            }
            checks.push((
                scope.file.clone(),
                item.clone(),
                step.next.iter().map(|t| t.to.clone()).collect(),
            ));
            if step.use_module.is_some() {
                diags.push(diag(
                    &scope.file,
                    &item,
                    DiagCode::Structure,
                    "module expansion is not implemented yet (TASK-50)",
                ));
                continue;
            }
            // 状態変化の語彙（TASK-43 1/2）。`use` の Step は上で拒否済みなので検査しない。
            for (field, list) in [
                ("on_enter", &step.on_enter),
                ("on_achieved", &step.on_achieved),
            ] {
                for (code, msg) in check_effects(list, field, &w.fixtures) {
                    diags.push(diag(&scope.file, &item, code, msg));
                }
            }
            let mut s = step.clone();
            for t in &mut s.next {
                t.to = format!("main/{}", t.to);
            }
            // 木を作る前に、積み上げる本文の量を予算と照らす。超えたら以降の木は作らない。
            let mut body_doc = None;
            let mut body_levels_doc = BTreeMap::new();
            if !over_budget {
                for b in step.body.iter().chain(step.body_levels.values()) {
                    let len = body_len(pack, &scope.dir, b);
                    if !budget.take(len) {
                        over_budget = true;
                        diags.push(diag(
                            &scope.file,
                            &item,
                            DiagCode::Limit,
                            "bodies to embed exceed the IR size limit",
                        ));
                        break;
                    }
                }
            }
            if !over_budget {
                body_doc = step
                    .body
                    .as_deref()
                    .and_then(|b| body_tree(pack, &scope.dir, b));
                body_levels_doc = step
                    .body_levels
                    .iter()
                    .filter_map(|(level, b)| Some((level.clone(), body_tree(pack, &scope.dir, b)?)))
                    .collect();
            }
            nodes.push(Node {
                uid,
                scope: "main".to_string(),
                unit: scope.src.id.clone(),
                step: s,
                enter: None,
                exit_to: None,
                body_doc,
                body_levels_doc,
            });
        }
        for st in &scope.src.starts {
            if !start_ids.insert(st.id.clone()) {
                diags.push(diag(
                    &scope.file,
                    &format!("start:{}", st.id),
                    DiagCode::DupId,
                    format!("duplicate start id: {}", st.id),
                ));
                continue;
            }
            for (code, msg) in check_effects(&st.effects, "effects", &w.fixtures) {
                diags.push(diag(&scope.file, &format!("start:{}", st.id), code, msg));
            }
            let mut s = st.clone();
            s.node = format!("main/{}", st.node);
            starts.push(s);
        }
    }
    if chapters == 0 {
        diags.push(diag(
            "chapters",
            "pack",
            DiagCode::Structure,
            "no chapter is defined",
        ));
    } else if starts.is_empty() {
        // 開始点が 0 件の IR は、エンジンが開始できない（PACK-25、D-43 で拒否と決定）。Step が空の章だけの
        // パックもここに当たる（開始点を書けば、宛先の Step が無いことで E-REF-STEP になる）。
        diags.push(diag(
            "chapters",
            "pack",
            DiagCode::Structure,
            "no start is defined",
        ));
    }

    // 遷移先・開始の解決。解決先は実際に node 化した Step だけにする（`use` の Step は含めない）。
    // 診断の file は、最初に定義した章の定義ファイル、遷移先は教材に書かれた元の ID で示す。
    let defined: BTreeSet<&str> = nodes.iter().map(|n| n.uid.as_str()).collect();
    for (file, item, targets) in &checks {
        for to in targets {
            if !defined.contains(format!("main/{to}").as_str()) {
                diags.push(diag(
                    file,
                    item,
                    DiagCode::RefStep,
                    format!("transition target is undefined: {to}"),
                ));
            }
        }
    }
    for scope in pack.scopes.iter().filter(|s| s.kind == ScopeKind::Chapter) {
        for st in &scope.src.starts {
            if !defined.contains(format!("main/{}", st.node).as_str()) {
                diags.push(diag(
                    &scope.file,
                    &format!("start:{}", st.id),
                    DiagCode::RefStep,
                    format!("start node is undefined: {}", st.node),
                ));
            }
        }
    }
    // 入力方式の宣言（章とモジュールの全 Step）を、型で読めるかと意味の制約で検査する（TASK-44）。
    diags.extend(input::check_inputs(&pack.scopes));
    if !diags.is_empty() {
        return Err(diags);
    }

    for id in w.fixtures.keys() {
        let len = pack.fixture_texts.get(id).map_or(0, |t| t.len() as u64);
        if !budget.take(len) {
            return Err(vec![diag(
                &pack.workshop_file,
                &format!("fixture:{id}"),
                DiagCode::Limit,
                "fixtures to embed exceed the IR size limit",
            )]);
        }
    }
    let fixture_docs = w
        .fixtures
        .iter()
        .filter_map(|(id, f)| {
            let text = pack.fixture_texts.get(id)?;
            let is_md = std::path::Path::new(&f.path)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("md"));
            let doc = if is_md {
                markdown::parse(text)
            } else {
                markdown::plain_text(text)
            };
            Some((id.clone(), doc))
        })
        .collect();

    let mut ir = Ir {
        format_version: w.pack.ir_format,
        pack: w.pack.clone(),
        refs: w.refs.clone(),
        commands: w.commands.clone(),
        views: w.views.clone(),
        fixtures: w.fixtures.clone(),
        hints: w.hints.clone(),
        rejections: w.rejections.clone(),
        clarifications: w.clarifications.clone(),
        defaults: w.defaults.clone(),
        instances: vec![Instance {
            id: "main".to_string(),
            module: None,
            args: BTreeMap::new(),
            caller: None,
        }],
        nodes,
        starts,
        fixture_docs,
    };
    // 出力の版は、使った項目に足りる最小の版にする。2 つの版は常に同じ値である。
    let format = ir.required_ir_format();
    ir.format_version = format;
    ir.pack.ir_format = format;
    Ok(ir)
}

/// 木にする元の量の予算。
struct Budget {
    left: u64,
}

impl Budget {
    fn new(limit: u64) -> Self {
        Budget { left: limit }
    }

    /// `n` バイトを使う。足りなければ何も使わず `false`。
    fn take(&mut self, n: u64) -> bool {
        match self.left.checked_sub(n) {
            Some(rest) => {
                self.left = rest;
                true
            }
            None => false,
        }
    }
}

/// 本文の参照が指す本文のバイト数。無ければ 0。
fn body_len(pack: &LoadedPack, scope_dir: &str, body_ref: &str) -> u64 {
    pack.bodies
        .get(&format!("{scope_dir}/{body_ref}"))
        .map_or(0, |t| t.len() as u64)
}

/// 本文の参照から木を作る。`source::load` が読んだ本文だけを見る。無ければ `None`
/// （`load` は参照の欠けを診断にするので、成功した `LoadedPack` では通常は起きない）。
fn body_tree(pack: &LoadedPack, scope_dir: &str, body_ref: &str) -> Option<Doc> {
    pack.bodies
        .get(&format!("{scope_dir}/{body_ref}"))
        .map(|text| markdown::parse(text))
}

/// IR を出力のバイト列にする。形の正は契約の `to_canonical_json`。`compare` と共通の関数。
pub fn to_json(ir: &Ir) -> Result<Vec<u8>, serde_json::Error> {
    fandhe_workshop_contract::ir::to_canonical_json(ir)
}

/// 生成した IR のバイト列が、エンジンの `load_ir` の上限（大きさ・入れ子）に収まることを確かめる。
///
/// パック入力が上限以下でも、JSON のエスケープで出力が膨らむ・未型付けの値が深く入れ子になるなどで
/// `load_ir` が拒否する IR ができうる。成功を返す前に、core の共有の判定で拒否する。
fn check_output(bytes: &[u8], file: &str) -> Result<(), Vec<Diagnostic>> {
    check_ir_limits(bytes).map_err(|reason| {
        vec![diag(
            file,
            "pack",
            DiagCode::Limit,
            format!("generated IR exceeds the engine limit: {reason}"),
        )]
    })
}

/// ディレクトリのパックを読み、IR のバイト列にする。`load`・`compile`・`to_json` をまとめた入口。
///
/// 返すバイト列は `check_ir_limits` を通っており、`load_ir` が大きさ・深さで拒否することはない。
pub fn compile_dir(dir: &Path) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let pack = source::load(dir)?;
    let ir = compile(&pack)?;
    let bytes = to_json(&ir).map_err(|_| {
        vec![diag(
            &pack.workshop_file,
            "pack",
            DiagCode::Structure,
            "IR serialization failed",
        )]
    })?;
    check_output(&bytes, &pack.workshop_file)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fandhe_workshop_contract::limits::MAX_JSON_DEPTH;

    #[test]
    fn check_output_rejects_too_large() {
        let v = vec![b' '; MAX_IR_BYTES as usize + 1];
        let d = check_output(&v, "workshop.yaml").unwrap_err();
        assert_eq!(d[0].code(), DiagCode::Limit);
        assert!(check_output(&vec![b' '; MAX_IR_BYTES as usize], "workshop.yaml").is_ok());
    }

    #[test]
    fn check_output_rejects_too_deep() {
        let nest = |n: usize| format!("{}{}", "[".repeat(n), "]".repeat(n));
        let d = check_output(nest(MAX_JSON_DEPTH + 1).as_bytes(), "workshop.yaml").unwrap_err();
        assert_eq!(d[0].code(), DiagCode::Limit);
        assert!(check_output(nest(MAX_JSON_DEPTH).as_bytes(), "workshop.yaml").is_ok());
    }

    /// 同じ本文を多数の Step が参照しても、木の確保が出力の上限を超える前に E-LIMIT で止まる。
    #[test]
    fn many_steps_sharing_one_body_stop_at_the_budget() {
        use crate::source::{LoadedScope, WorkshopSrc};
        let workshop: WorkshopSrc = yaml_serde::from_str(
            "pack: {id: p, title: t, version: '1', ir_format: 1, guidance: A}\n",
        )
        .unwrap();
        let body = "a".repeat(500_000);
        let n = (MAX_IR_BYTES / body.len() as u64) as usize + 2;
        let steps: String = (0..n)
            .map(|i| format!("  - {{id: s{i}, title: t, body: b.md}}\n"))
            .collect();
        let scope = yaml_serde::from_str(&format!("id: c\ntitle: c\nsteps:\n{steps}")).unwrap();
        let pack = LoadedPack {
            format: source::Format::Yaml,
            workshop_file: "workshop.yaml".to_string(),
            workshop,
            scopes: vec![LoadedScope {
                kind: ScopeKind::Chapter,
                file: "chapters/c/chapter.yaml".to_string(),
                dir: "chapters/c".to_string(),
                src: scope,
            }],
            bodies: BTreeMap::from([("chapters/c/b.md".to_string(), body)]),
            fixture_texts: BTreeMap::new(),
            tests_file: None,
            tests: None,
        };
        let d = compile(&pack).unwrap_err();
        assert_eq!(
            d.iter().filter(|x| x.code() == DiagCode::Limit).count(),
            1,
            "{d:?}"
        );
    }

    #[test]
    fn budget_takes_exactly_up_to_the_limit() {
        let mut b = Budget::new(10);
        assert!(b.take(10));
        assert!(b.take(0));
        assert!(!b.take(1));
    }
}
