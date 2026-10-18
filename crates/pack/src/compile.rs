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
//!
//! モジュールの `use` は [`crate::expand`] が実体に展開する（TASK-50 1/2）。章は実体 `main`、呼ばれた
//! モジュールは `<モジュール ID>@<実体名>` の実体で、Step の `uid` は `<実体 ID>/<Step ID>` になる。遷移先と
//! 開始の解決は、実体からの相対で行う。呼ばれないモジュールは IR に出さないが、定義の段で行う検査
//! （`use` と `input` の同時指定・`input`・`confirm`・拒否の分類・差し込みの枠・操作プロファイル）は
//! `pack.scopes` の全スコープに働くため、その中身も診断の対象になる（fail-closed）。
//! モジュール呼び出しの巡回の検出（`E-MODULE-CYCLE`、TASK-50 2/2）も、展開の途中で [`crate::expand`] が行う。
//! 展開を要するこの検査と、`${` を含む宣言の置換後の検査は、呼ばれないモジュールでは働かない。
//! `pack.appearance` の検査（`E-PARSE`、TASK-53）は [`crate::appearance`] が、`pack.left_pane` の検査と
//! `E-LEFT-*`（TASK-53 2/2）は [`crate::left_pane`] が行う。
//! Step の `input` の宣言は [`crate::input`]、`confirm` の宣言と未指定は [`crate::confirm`] が検査する（IR には宣言の値をそのまま載せる）。
//! モジュールの宣言が `${` を含む場合は、引数を置換した後の値を [`crate::expand`] が検査する。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use fandhe_workshop_contract::doc::Doc;
use fandhe_workshop_contract::ir::{is_supported_ir_format, Instance, Ir, Start};
use fandhe_workshop_contract::limits::MAX_IR_BYTES;
use fandhe_workshop_core::check_ir_limits;

use crate::condition_check::check_optional;
use crate::confirm;
use crate::diag::{DiagCode, Diagnostic};
use crate::effect_check::check_effects;
use crate::expand::Expander;
use crate::input;
use crate::markdown;
use crate::source::{self, LoadedPack, ScopeKind};

pub(crate) fn diag(
    file: &str,
    item: &str,
    code: DiagCode,
    message: impl Into<String>,
) -> Diagnostic {
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

    let (profile, profile_diags) = crate::profile::check_profile(&pack.workshop_file, w);
    let appearance_diags = crate::appearance::check_appearance(&pack.workshop_file, w);
    let mut starts: Vec<Start> = Vec::new();
    let mut start_ids: BTreeSet<String> = BTreeSet::new();
    let mut chapters = 0usize;
    // 木にする本文・練習データ・展開した Step の元の量の合計。同じ本文を多数の Step が参照しても、
    // 展開が連鎖しても、確保が出力の上限を超えて増えないようにする（`check_output` は全部を作った後に
    // 走るため、それでは間に合わない）。
    let mut ex = Expander::new(pack, &profile, Budget::new(MAX_IR_BYTES));
    ex.diags.extend(profile_diags);
    ex.diags.extend(appearance_diags);
    // 読めない `left_pane` は `E-PARSE` だけを報告し、モードが決まらないため `E-LEFT-*` は検査しない。
    let (left_pane, left_parse) = crate::left_pane::check_left_pane(&pack.workshop_file, w);
    let left_pane_ok = left_parse.is_empty();
    if left_pane_ok {
        ex.diags
            .extend(crate::left_pane::check_left_targets(pack, left_pane));
    }
    ex.diags.extend(left_parse);

    for scope in pack.scopes.iter().filter(|s| s.kind == ScopeKind::Chapter) {
        chapters += 1;
        ex.expand_chapter(scope);
        for st in &scope.src.starts {
            if !start_ids.insert(st.id.clone()) {
                ex.diags.push(diag(
                    &scope.file,
                    &format!("start:{}", st.id),
                    DiagCode::DupId,
                    format!("duplicate start id: {}", st.id),
                ));
                continue;
            }
            for (code, msg) in check_effects(&st.effects, "effects", &w.fixtures, false) {
                ex.diags
                    .push(diag(&scope.file, &format!("start:{}", st.id), code, msg));
            }
            for (code, msg) in check_optional(st.requires.as_ref(), "requires") {
                ex.diags
                    .push(diag(&scope.file, &format!("start:{}", st.id), code, msg));
            }
            let mut s = st.clone();
            s.node = format!("main/{}", st.node);
            starts.push(s);
        }
    }
    if chapters == 0 {
        ex.diags.push(diag(
            "chapters",
            "pack",
            DiagCode::Structure,
            "no chapter is defined",
        ));
    } else if starts.is_empty() {
        // 開始点が 0 件の IR は、エンジンが開始できない（PACK-25、D-43 で拒否と決定）。Step が空の章だけの
        // パックもここに当たる（開始点を書けば、宛先の Step が無いことで E-REF-STEP になる）。
        ex.diags.push(diag(
            "chapters",
            "pack",
            DiagCode::Structure,
            "no start is defined",
        ));
    }

    // 遷移先・開始の解決。解決先は展開した実体の node（`use` の Step も node である）。
    // 診断の file は、Step を定義した章・モジュールの定義ファイル、遷移先は教材に書かれた元の ID で示す。
    let mut diags = std::mem::take(&mut ex.diags);
    for c in &ex.checks {
        for to in &c.targets {
            if !ex.uids.contains(&format!("{}/{to}", c.inst)) {
                diags.push(diag(
                    &c.file,
                    &c.item,
                    DiagCode::RefStep,
                    format!("transition target is undefined: {to}"),
                ));
            }
        }
    }
    for scope in pack.scopes.iter().filter(|s| s.kind == ScopeKind::Chapter) {
        for st in &scope.src.starts {
            if !ex.uids.contains(&format!("main/{}", st.node)) {
                diags.push(diag(
                    &scope.file,
                    &format!("start:{}", st.id),
                    DiagCode::RefStep,
                    format!("start node is undefined: {}", st.node),
                ));
            }
        }
    }
    ex.diags = Vec::new();
    starts.extend(ex.module_starts(&mut start_ids));
    diags.append(&mut ex.diags);
    // 引数に依存する左エリアの宛先は、展開の後の Step で検査する。
    if left_pane_ok {
        diags.extend(crate::left_pane::check_left_expanded(
            pack, left_pane, &ex.nodes, &starts,
        ));
    }
    // 入力方式の宣言（章とモジュールの全 Step）を、型で読めるかと意味の制約で検査する（TASK-44）。
    // 入力を持つ Step が確認も理由つきの省略も持たないことと、確認の宣言を検査する（TASK-47、PACK-13）。
    // 接頭辞と入力欄の有無に反する入力・規則を検査する（TASK-52、PACK-20）。
    // モジュールの `input`・`confirm` が `${` を含む Step は、ここでは飛ばし、展開（`expand`）が引数を
    // 置換した後の Step を検査する（置換前は読めない・置換後に違反になりうるため）。
    // `use` と `input`、`use` と `confirm` の同時指定を拒否する（PACK-19）。確認の検査は `use` の Step を
    // 飛ばすため、ここで拒否しないと確認の無い入力が IR に載り、`use` の Step の `confirm` は無視される。
    diags.extend(crate::expand::check_use_steps(&pack.scopes));
    diags.extend(input::check_inputs(&pack.scopes));
    diags.extend(confirm::check_confirms(&pack.scopes));
    // 拒否の分類の語（`rejections`・`defaults`・`on_reject`）を contract の 8 語で検査する（TASK-45）。
    diags.extend(crate::rejection::check_categories(pack));
    // 差し込みの枠と次の行動（TASK-46）。不正な枠を含む IR を出さない（fail-closed）。
    diags.extend(crate::slot_check::check_texts(pack));
    // 展開の後（引数の置換の後）の Step と開始点でも、`step_do` を使う文面に `do` があることを検査する。
    diags.extend(crate::slot_check::check_expanded(pack, &ex.nodes, &starts));
    diags.extend(crate::profile::check_profile_steps(&profile, &pack.scopes));
    // 実体ごとに同じ定義から同じ診断が重なるため、同じ内容のものは 1 件にする。
    let mut seen = BTreeSet::new();
    diags.retain(|d| seen.insert(format!("{d:?}")));
    if !diags.is_empty() {
        return Err(diags);
    }
    let Expander {
        nodes,
        instances: child_instances,
        mut budget,
        ..
    } = ex;

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

    let mut instances = vec![Instance {
        id: "main".to_string(),
        module: None,
        args: BTreeMap::new(),
        caller: None,
    }];
    instances.extend(child_instances);
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
        instances,
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
pub(crate) struct Budget {
    left: u64,
}

impl Budget {
    pub(crate) fn new(limit: u64) -> Self {
        Budget { left: limit }
    }

    /// 残りのバイト数。
    pub(crate) fn left(&self) -> u64 {
        self.left
    }

    /// `n` バイトを使う。足りなければ何も使わず `false`。
    pub(crate) fn take(&mut self, n: u64) -> bool {
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
pub(crate) fn body_len(pack: &LoadedPack, scope_dir: &str, body_ref: &str) -> u64 {
    pack.bodies
        .get(&format!("{scope_dir}/{body_ref}"))
        .map_or(0, |t| t.len() as u64)
}

/// 本文の参照から木を作る。`source::load` が読んだ本文だけを見る。無ければ `None`
/// （`load` は参照の欠けを診断にするので、成功した `LoadedPack` では通常は起きない）。
pub(crate) fn body_tree(pack: &LoadedPack, scope_dir: &str, body_ref: &str) -> Option<Doc> {
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
            vocab_explicit_null: false,
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
