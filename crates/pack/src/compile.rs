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
//! モジュールの展開は TASK-50 が扱うため、`use` を持つ Step は拒否する。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fandhe_workshop_contract::doc::Doc;
use fandhe_workshop_contract::ir::{is_supported_ir_format, Instance, Ir, Node, Start};
use fandhe_workshop_core::check_ir_limits;

use crate::diag::{DiagCode, Diagnostic};
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

    let mut diags = Vec::new();
    let mut nodes: Vec<Node> = Vec::new();
    let mut starts: Vec<Start> = Vec::new();
    let mut uids: BTreeSet<String> = BTreeSet::new();
    // 遷移先の検査対象。node 化しない `use` の Step も含める。file・item・書き換え前の遷移先 ID を保持する。
    let mut checks: Vec<(String, String, Vec<String>)> = Vec::new();
    let mut start_ids: BTreeSet<String> = BTreeSet::new();
    let mut chapters = 0usize;

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
            let mut s = step.clone();
            for t in &mut s.next {
                t.to = format!("main/{}", t.to);
            }
            let body_doc = step
                .body
                .as_deref()
                .and_then(|b| body_tree(pack, &scope.dir, b));
            let body_levels_doc = step
                .body_levels
                .iter()
                .filter_map(|(level, b)| Some((level.clone(), body_tree(pack, &scope.dir, b)?)))
                .collect();
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
    if !diags.is_empty() {
        return Err(diags);
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
    use fandhe_workshop_contract::limits::{MAX_IR_BYTES, MAX_JSON_DEPTH};

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
}
