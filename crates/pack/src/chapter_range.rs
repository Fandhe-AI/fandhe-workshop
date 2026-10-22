//! 章の範囲の指定（`compile --chapters`）と、章をまたぐ `next` の拒否（TASK-67・PACK-43・SHELL-12・D-67）。
//!
//! 試行で配る IR は、範囲外の章の開始点を含めない（D-67 の「IR を絞る」案）。このモジュールは 2 つの役割を持つ。
//! - [`check_chapter_local_next`]: 章の `next` が他の章の Step を指すことを `E-REF-STEP` にする。絞った IR で
//!   範囲外の Step への辺が残らないことの前提で、`validate` と `compile` の両方が呼ぶ。
//! - [`narrow_chapters`]: 全章を展開した IR から、選んだ章とそこから呼ばれる実体だけを残す。教材パック全体の
//!   検査を通った IR にだけ働くので、絞っても診断の集合は変わらない（fail-closed）。
//!
//! 呼び出し元は `compile`（検査）・`compile::compile_dir_with`（刈り込み）・`validate`（検査）である。
//! 出力の決定性のため、並びは元の IR のまま保ち、新しい並べ替えをしない。

use std::collections::{BTreeMap, BTreeSet};

use fandhe_workshop_contract::ir::Ir;

use crate::diag::{DiagCode, Diagnostic};
use crate::source::{LoadedScope, ScopeKind};
use crate::validate::{diag, shown};

/// 章の `next` が、自分の章に無く他の章にある Step を指す場合に `E-REF-STEP` を報告する。
///
/// どの章にも無い宛先は既存の「未定義」の診断に任せ、二重に報告しない。モジュールは自分の Step だけの名前空間に
/// 閉じているため対象外。開始点（`starts[].node`）は spec が `next` しか挙げていないため制限しない。
pub(crate) fn check_chapter_local_next(scopes: &[LoadedScope]) -> Vec<Diagnostic> {
    let chapters: Vec<&LoadedScope> = scopes
        .iter()
        .filter(|s| s.kind == ScopeKind::Chapter)
        .collect();
    let mut out = Vec::new();
    for (i, scope) in chapters.iter().enumerate() {
        let own: BTreeSet<&str> = scope.src.steps.iter().map(|s| s.id.as_str()).collect();
        let others: BTreeSet<&str> = chapters
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .flat_map(|(_, c)| c.src.steps.iter().map(|s| s.id.as_str()))
            .collect();
        for step in &scope.src.steps {
            for next in &step.next {
                let to = next.to.as_str();
                if !own.contains(to) && others.contains(to) {
                    out.push(diag(
                        &scope.file,
                        &format!("step:{}", step.id),
                        DiagCode::RefStep,
                        format!("transition target is in another chapter: {}", shown(to)),
                    ));
                }
            }
        }
    }
    out
}

/// 章の選択の誤り。`compile` の終了コード 2（使い方の誤り）に対応する。
#[derive(Debug, PartialEq, Eq)]
pub enum ChapterSelectError {
    /// 該当する章の ID が無い。
    Unknown(String),
    /// 同じ ID の章が 2 つ以上ある。
    Ambiguous(String),
}

/// 選んだ章の ID を、読み込んだ章の ID と照合する。
pub(crate) fn resolve_selection(
    scopes: &[LoadedScope],
    selected: &[String],
) -> Result<BTreeSet<String>, ChapterSelectError> {
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for s in scopes.iter().filter(|s| s.kind == ScopeKind::Chapter) {
        *count.entry(s.src.id.as_str()).or_default() += 1;
    }
    let mut out = BTreeSet::new();
    for id in selected {
        match count.get(id.as_str()) {
            None => return Err(ChapterSelectError::Unknown(id.clone())),
            Some(1) => {
                out.insert(id.clone());
            }
            Some(_) => return Err(ChapterSelectError::Ambiguous(id.clone())),
        }
    }
    Ok(out)
}

/// 全章を展開した IR を、選んだ章とそこから呼ばれる実体だけに絞る。
///
/// 残すのは、選んだ章の `main` の node と、その node から（入れ子を含めて）`use` で呼ばれた実体の node。
/// `starts` は残した node を指すものだけにする。範囲外の node を指す辺・`enter`・`exit_to` が残るなら
/// `E-REF-STEP` で止める（`check_chapter_local_next` があれば起きない二重の安全網）。
pub(crate) fn narrow_chapters(
    mut ir: Ir,
    selected: &BTreeSet<String>,
    file: &str,
) -> Result<Ir, Vec<Diagnostic>> {
    let mut keep: BTreeSet<String> = ir
        .nodes
        .iter()
        .filter(|n| n.scope == "main" && selected.contains(&n.unit))
        .map(|n| n.uid.clone())
        .collect();
    let mut kept_instances: BTreeSet<String> = BTreeSet::from(["main".to_string()]);
    // 親の実体は子より先に並ぶが、並びに依らないよう変化が無くなるまで繰り返す。
    loop {
        let mut changed = false;
        for inst in &ir.instances {
            if kept_instances.contains(&inst.id) {
                continue;
            }
            let Some(caller) = &inst.caller else { continue };
            if keep.contains(caller) {
                kept_instances.insert(inst.id.clone());
                for n in ir.nodes.iter().filter(|n| n.scope == inst.id) {
                    keep.insert(n.uid.clone());
                }
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    ir.nodes.retain(|n| keep.contains(&n.uid));
    ir.instances.retain(|i| kept_instances.contains(&i.id));
    ir.starts.retain(|s| keep.contains(&s.node));
    if ir.starts.is_empty() {
        return Err(vec![diag(
            "chapters",
            "pack",
            DiagCode::Structure,
            "no start is defined",
        )]);
    }
    let escapes = ir.nodes.iter().any(|n| {
        n.step
            .next
            .iter()
            .map(|t| t.to.as_str())
            .chain(n.enter.as_deref())
            .chain(n.exit_to.as_deref())
            .any(|r| !keep.contains(r))
    });
    if escapes {
        return Err(vec![diag(
            file,
            "pack",
            DiagCode::RefStep,
            "a kept step refers to a step outside the selected chapters",
        )]);
    }
    // 使う項目が減るため、出力の版を求め直す。2 つの版は常に同じ値である。
    let format = ir.required_ir_format();
    ir.format_version = format;
    ir.pack.ir_format = format;
    Ok(ir)
}
