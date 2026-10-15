//! 読み込んだ教材パック（`LoadedPack`）から IR を作る。
//!
//! `compile` はメモリ上の値だけを見る純関数で、ファイルシステムには触れない（ディスクに触れるのは
//! `source::load`）。出力は決定的である。連想配列は `BTreeMap`、章は名前の昇順、Step はファイルの中の順で
//! 並べ、絶対パス・時刻・乱数を入れない。#98 の `compare` が同じ `to_json` を使ってバイト列を比べる。
//!
//! この版で作る IR は `ir_format` 1 の形（`body` は参照の文字列のまま）である。モジュールの展開は
//! TASK-50、本文の木と練習データの中身は TASK-60 が扱うため、`use` を持つ Step は拒否する。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fandhe_workshop_contract::ir::{Instance, Ir, Node, Start, IR_FORMAT_VERSION};
use fandhe_workshop_core::check_ir_limits;

use crate::diag::{DiagCode, Diagnostic};
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
    if w.pack.ir_format != IR_FORMAT_VERSION {
        return Err(vec![diag(
            &pack.workshop_file,
            "pack:ir_format",
            DiagCode::FormatVersion,
            format!(
                "ir_format {} is not supported (expected {IR_FORMAT_VERSION})",
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
            nodes.push(Node {
                uid,
                scope: "main".to_string(),
                unit: scope.src.id.clone(),
                step: s,
                enter: None,
                exit_to: None,
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

    Ok(Ir {
        format_version: IR_FORMAT_VERSION,
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
    })
}

/// IR を出力のバイト列にする。整形した JSON に改行を 1 つ付ける。`compare` と共通の関数。
pub fn to_json(ir: &Ir) -> Result<Vec<u8>, serde_json::Error> {
    let mut v = serde_json::to_vec_pretty(ir)?;
    v.push(b'\n');
    Ok(v)
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
