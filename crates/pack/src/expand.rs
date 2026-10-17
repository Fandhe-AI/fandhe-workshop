//! 学習モジュールの `use` を実体に展開する（TASK-50 1/2、PACK-19）。
//!
//! `compile` から呼ばれる。章（実体 `main`）の Step を深さ優先の前順に走査し、`use` を持つ Step があれば
//! 呼び出したモジュールを `<モジュール ID>@<実体名>` の実体として展開する。実体の中の Step は
//! `<実体 ID>/<Step ID>` の `uid` を持つ node になる。入れ子の実体の ID は `<親の実体 ID>/<モジュール ID>@<実体名>`
//! で、core の `StepUid` の形に合わせてある。
//!
//! 展開は教材データを信頼しない入力として扱い、fail-closed にする。
//! - 巡回（展開中の経路にあるモジュールを再び呼ぶ）は、巡回を閉じる `use` の Step に経路つきで
//!   `E-MODULE-CYCLE` を付け、実体を作らず再帰しない（TASK-50 2/2）。
//! - 入れ子は [`MAX_MODULE_DEPTH`] 段まで。それを超える呼び出しは展開せず `E-MODULE-CYCLE` にする
//!   （巡回は上限の前に止まるため、この上限は巡回しない深い入れ子にだけ効く）。
//! - 展開した Step の大きさを、`compile` の予算（`MAX_IR_BYTES`）から引く。足りなければ `E-LIMIT` で止める。
//! - `${名前}` の置換は 1 回だけで、差し込んだ値を再び展開しない。`id`・`body`・`body_levels`・`next[].to` は
//!   置換しない（構造の識別子と本文の参照を、引数で書き換えさせない）。
//!
//! 診断に引数の値・本文は入れない。名前は `safe_ident` を通るものだけを入れる。
//!
//! `use` と `input`、`use` と `confirm` を同時に持つ Step は、展開とは別に [`check_use_steps`] が章・モジュールの定義ごとに拒否する
//! （PACK-19）。`expand_use` 自体は呼び出し元の Step の `input` を見ない。
//!
//! 置換前の値では検査できない宣言（`input`・`confirm` が `${` を含むモジュールの Step）は、置換後の Step を
//! ここで検査する（入力方式・確認・操作プロファイル。PACK-10・PACK-13・PACK-20）。置換前の検査
//! （`input::check_inputs`・`confirm::check_confirms`・`profile::check_profile_steps`）はその Step を飛ばすので、
//! 同じ違反は 1 回だけ報告される。`${` を含まない宣言は置換で変わらないため、置換後には検査しない。

use std::collections::{BTreeMap, BTreeSet};

use fandhe_workshop_contract::doc::Doc;
use fandhe_workshop_contract::ir::{Instance, Node, Start, Step};
use fandhe_workshop_contract::limits::MAX_COLLECTION_ELEMENTS;
use fandhe_workshop_contract::profile::ProfileSetting;
use serde::Deserialize;
use serde_json::Value;

use crate::compile::{body_len, body_tree, diag, Budget};
use crate::diag::{DiagCode, Diagnostic};
use crate::effect_check::{check_effects, check_processing};
use crate::input::has_placeholder;
use crate::source::{safe_ident, LoadedPack, LoadedScope, ScopeKind};

/// モジュールの入れ子の深さの上限。章を深さ 0 とし、深さ 8 の実体の中の `use` は展開しない。
pub(crate) const MAX_MODULE_DEPTH: usize = 8;

/// 置換しない Step の最上位の欄。構造の識別子と本文の参照である（本文は `source::load` が置換前の参照で
/// 読み込み済みで、置換すると引き当てを外す・パスを書き換える経路になる）。`next[].to` は別に除く。
const NO_SUBST_STEP_KEYS: [&str; 3] = ["id", "body", "body_levels"];

/// `use` の中身。契約の `Step.use_module` は自由な値のまま残し、pack の中でだけ型にする。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UseSrc {
    module: String,
    instance: String,
    #[serde(default)]
    args: BTreeMap<String, String>,
}

/// `${名前}` の置換の失敗。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SubstError {
    /// 引数に無い名前。名前は `safe_ident` を通るときだけ持つ。
    Undefined(Option<String>),
    /// 閉じていない `${`。
    Unterminated,
    /// 出力が残りの量を超える。
    TooLarge,
}

impl SubstError {
    fn into_diag(self, file: &str, item: &str) -> Diagnostic {
        match self {
            SubstError::Undefined(Some(n)) => diag(
                file,
                item,
                DiagCode::Structure,
                format!("undefined argument in `${{...}}`: {n}"),
            ),
            SubstError::Undefined(None) => diag(
                file,
                item,
                DiagCode::Structure,
                "undefined argument in `${...}`",
            ),
            SubstError::Unterminated => diag(
                file,
                item,
                DiagCode::Structure,
                "unterminated `${` in a module string",
            ),
            SubstError::TooLarge => diag(
                file,
                item,
                DiagCode::Limit,
                "expanded module strings exceed the IR size limit",
            ),
        }
    }
}

/// 文字列の `${名前}` を引数で 1 回だけ置き換える。差し込んだ値の中の `${` は再び置換しない。
/// 出力の大きさを `left`（残りの量）から引き、足りなければ巨大な確保をする前に打ち切る。
/// 呼び出し側は同じ Step・Start の全置換で `left` を共有し、置換をまたいだ累積を抑える。
pub(crate) fn substitute(
    s: &str,
    args: &BTreeMap<String, String>,
    left: &mut u64,
) -> Result<String, SubstError> {
    if !s.contains("${") {
        return Ok(s.to_string());
    }
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("${") {
        take_left(left, i as u64)?;
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        let Some(j) = after.find('}') else {
            return Err(SubstError::Unterminated);
        };
        let name = &after[..j];
        match args.get(name) {
            Some(v) => {
                take_left(left, v.len() as u64)?;
                out.push_str(v);
            }
            None => {
                return Err(SubstError::Undefined(
                    safe_ident(name).then(|| name.to_string()),
                ))
            }
        }
        rest = &after[j + 1..];
    }
    take_left(left, rest.len() as u64)?;
    out.push_str(rest);
    Ok(out)
}

/// 残りの量から `n` バイトを引く。足りなければ残りを変えずに `TooLarge`。
fn take_left(left: &mut u64, n: u64) -> Result<(), SubstError> {
    if n > *left {
        return Err(SubstError::TooLarge);
    }
    *left -= n;
    Ok(())
}

/// 値の中の文字列をすべて置換する。連想配列のキーは置換しない。
pub(crate) fn subst_value(
    v: &mut Value,
    args: &BTreeMap<String, String>,
    left: &mut u64,
) -> Result<(), SubstError> {
    match v {
        Value::String(s) => *s = substitute(s, args, left)?,
        Value::Array(a) => {
            for x in a {
                subst_value(x, args, left)?;
            }
        }
        Value::Object(m) => {
            for (_, x) in m.iter_mut() {
                subst_value(x, args, left)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Step の欄を置換する。除く欄は [`NO_SUBST_STEP_KEYS`] と `next[].to`。`None` は型への戻しの失敗。
fn subst_step(
    step: &Step,
    args: &BTreeMap<String, String>,
    left: &mut u64,
) -> Option<Result<Step, SubstError>> {
    let mut v = serde_json::to_value(step).ok()?;
    if let Value::Object(m) = &mut v {
        for (k, x) in m.iter_mut() {
            if NO_SUBST_STEP_KEYS.contains(&k.as_str()) {
                continue;
            }
            let r = if k == "next" {
                subst_next(x, args, left)
            } else {
                subst_value(x, args, left)
            };
            if let Err(e) = r {
                return Some(Err(e));
            }
        }
    }
    Some(Ok(serde_json::from_value(v).ok()?))
}

/// `next` の配列で、`to` 以外を置換する。
fn subst_next(
    v: &mut Value,
    args: &BTreeMap<String, String>,
    left: &mut u64,
) -> Result<(), SubstError> {
    if let Value::Array(a) = v {
        for t in a {
            if let Value::Object(m) = t {
                for (k, x) in m.iter_mut() {
                    if k != "to" {
                        subst_value(x, args, left)?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// 遷移先の検査対象。診断は書き換え前の ID と、定義した章・モジュールの定義ファイルで示す。
pub(crate) struct Check {
    pub file: String,
    pub item: String,
    pub inst: String,
    pub targets: Vec<String>,
}

/// 展開した実体の途中開始の元。展開の後に `module_starts` が使う。
struct PendingStarts<'a> {
    inst: String,
    scope: &'a LoadedScope,
    args: BTreeMap<String, String>,
}

/// 章とモジュールを実体に展開し、node・実体・遷移の検査対象を集める。`compile` が章ごとに呼ぶ。
pub(crate) struct Expander<'a> {
    pack: &'a LoadedPack,
    /// 置換後の Step の操作プロファイルの検査に使う設定（読めない `pack.profile` は既定値）。
    profile: &'a ProfileSetting,
    modules: BTreeMap<&'a str, &'a LoadedScope>,
    pub nodes: Vec<Node>,
    pub instances: Vec<Instance>,
    pub checks: Vec<Check>,
    pub diags: Vec<Diagnostic>,
    pub uids: BTreeSet<String>,
    pub budget: Budget,
    over_budget: bool,
    halted: bool,
    inst_ids: BTreeSet<String>,
    pending: Vec<PendingStarts<'a>>,
    /// 今展開している経路のモジュール（宣言された ID、呼び出しのモジュール名が引数に由来したか）。章は積まない。
    module_stack: Vec<(String, bool)>,
}

impl<'a> Expander<'a> {
    pub(crate) fn new(pack: &'a LoadedPack, profile: &'a ProfileSetting, budget: Budget) -> Self {
        let mut diags = Vec::new();
        let mut modules: BTreeMap<&str, &LoadedScope> = BTreeMap::new();
        for s in pack.scopes.iter().filter(|s| s.kind == ScopeKind::Module) {
            if modules.insert(s.src.id.as_str(), s).is_some() {
                diags.push(diag(
                    &s.file,
                    "module",
                    DiagCode::DupId,
                    "duplicate module id",
                ));
            }
        }
        Expander {
            pack,
            profile,
            modules,
            nodes: Vec::new(),
            instances: Vec::new(),
            checks: Vec::new(),
            diags,
            uids: BTreeSet::new(),
            budget,
            over_budget: false,
            halted: false,
            inst_ids: BTreeSet::new(),
            pending: Vec::new(),
            module_stack: Vec::new(),
        }
    }

    /// 章の Step を実体 `main` として展開する。
    pub(crate) fn expand_chapter(&mut self, scope: &'a LoadedScope) {
        self.expand_scope("main", scope, None, 0, None);
    }

    fn limit(&mut self, file: &str, item: &str, msg: &str) {
        self.halted = true;
        self.diags.push(diag(file, item, DiagCode::Limit, msg));
    }

    /// 1 つの実体（章または展開したモジュール）の Step を node にする。`args` は章では `None`
    /// （章は引数を持たず、置換しない）。
    fn expand_scope(
        &mut self,
        inst: &str,
        scope: &'a LoadedScope,
        args: Option<&BTreeMap<String, String>>,
        depth: usize,
        exit_to: Option<&str>,
    ) {
        for step in &scope.src.steps {
            if self.halted {
                return;
            }
            let item = format!("step:{}", step.id);
            let uid = format!("{inst}/{}", step.id);
            if !self.uids.insert(uid.clone()) {
                self.diags.push(diag(
                    &scope.file,
                    &item,
                    DiagCode::DupId,
                    format!("duplicate step id: {}", step.id),
                ));
                continue;
            }
            let mut s = match args {
                Some(a) => match subst_step(step, a, &mut self.budget.left()) {
                    Some(Ok(s)) => s,
                    Some(Err(e)) => {
                        self.halted |= e == SubstError::TooLarge;
                        self.diags.push(e.into_diag(&scope.file, &item));
                        continue;
                    }
                    None => {
                        self.diags.push(diag(
                            &scope.file,
                            &item,
                            DiagCode::Structure,
                            "module step could not be expanded",
                        ));
                        continue;
                    }
                },
                None => step.clone(),
            };
            if args.is_some() {
                // 置換で input・confirm の値が変わりうるため、置換前に `${` を含んでいた宣言は展開後に検査する。
                // 置換前の検査はこれらを飛ばしている。含まない宣言は置換で変わらず、置換前の検査で足りる。
                if has_placeholder(&step.input) {
                    self.diags
                        .extend(crate::input::check_step_input(scope, &s, true));
                    self.diags
                        .extend(crate::profile::check_profile_step(self.profile, scope, &s));
                }
                if has_placeholder(&step.confirm) {
                    self.diags
                        .extend(crate::confirm::check_step_confirm(scope, &s));
                }
                // 展開した Step の大きさを予算から引く。ファンアウトが `check_output` の前に止まる。
                let size = serde_json::to_vec(&s).map_or(u64::MAX, |b| b.len() as u64);
                if !self.budget.take(size) {
                    self.limit(
                        &scope.file,
                        &item,
                        "expanded steps exceed the IR size limit",
                    );
                    return;
                }
            }
            if self.nodes.len() >= MAX_COLLECTION_ELEMENTS {
                self.limit(&scope.file, &item, "too many expanded steps");
                return;
            }
            // 状態変化の語彙（TASK-43 1/2）。置換後の値を検査する。
            for (field, list) in [("on_enter", &s.on_enter), ("on_achieved", &s.on_achieved)] {
                for (code, msg) in
                    check_effects(list, field, &self.pack.workshop.fixtures, args.is_some())
                {
                    self.diags.push(diag(&scope.file, &item, code, msg));
                }
            }
            for (code, msg) in
                check_processing(&s.processing, &self.pack.workshop.fixtures, args.is_some())
            {
                self.diags.push(diag(&scope.file, &item, code, msg));
            }
            self.checks.push(Check {
                file: scope.file.clone(),
                item: item.clone(),
                inst: inst.to_string(),
                targets: s.next.iter().map(|t| t.to.clone()).collect(),
            });
            for t in &mut s.next {
                t.to = format!("{inst}/{}", t.to);
            }
            let (body_doc, body_levels_doc) = self.bodies(scope, step, &item);
            let use_value = s.use_module.clone();
            let idx = self.nodes.len();
            self.nodes.push(Node {
                uid: uid.clone(),
                scope: inst.to_string(),
                unit: scope.src.id.clone(),
                step: s,
                enter: None,
                // 戻り先は実体の終端（`final`）の Step だけに持たせる。途中の Step の達成では実体を出ない。
                exit_to: if step.is_final {
                    exit_to.map(str::to_string)
                } else {
                    None
                },
                body_doc,
                body_levels_doc,
            });
            if let Some(u) = use_value {
                let site = UseSite {
                    inst,
                    scope,
                    uid: &uid,
                    item: &item,
                    node_idx: idx,
                    depth,
                    redact: args.is_some(),
                    // 置換前の Step で判定する（`s` は置換後）。
                    module_from_arg: step
                        .use_module
                        .as_ref()
                        .and_then(|v| v.get("module"))
                        .and_then(Value::as_str)
                        .is_some_and(|m| m.contains("${")),
                };
                self.expand_use(&site, &u);
            }
        }
    }

    /// 木にする本文を予算と照らして作る。超えたら以降の木は作らない。
    fn bodies(
        &mut self,
        scope: &LoadedScope,
        step: &Step,
        item: &str,
    ) -> (Option<Doc>, BTreeMap<String, Doc>) {
        if !self.over_budget {
            for b in step.body.iter().chain(step.body_levels.values()) {
                let len = body_len(self.pack, &scope.dir, b);
                if !self.budget.take(len) {
                    self.over_budget = true;
                    self.diags.push(diag(
                        &scope.file,
                        item,
                        DiagCode::Limit,
                        "bodies to embed exceed the IR size limit",
                    ));
                    break;
                }
            }
        }
        if self.over_budget {
            return (None, BTreeMap::new());
        }
        let doc = step
            .body
            .as_deref()
            .and_then(|b| body_tree(self.pack, &scope.dir, b));
        let levels = step
            .body_levels
            .iter()
            .filter_map(|(l, b)| Some((l.clone(), body_tree(self.pack, &scope.dir, b)?)))
            .collect();
        (doc, levels)
    }

    /// `use` を持つ Step から、モジュールの実体を作って再帰する。
    fn expand_use(&mut self, site: &UseSite<'_>, use_value: &Value) {
        let (file, item) = (&site.scope.file, site.item);
        let Ok(u) = serde_json::from_value::<UseSrc>(use_value.clone()) else {
            self.diags.push(diag(
                file,
                item,
                DiagCode::Parse,
                "invalid `use`: expected module, instance and string args (quote argument values)",
            ));
            return;
        };
        if !safe_ident(&u.module) || !safe_ident(&u.instance) {
            self.diags.push(diag(
                file,
                item,
                DiagCode::Structure,
                "`use` module and instance must be short identifiers ([A-Za-z0-9_.-])",
            ));
            return;
        }
        if let Some(pos) = self.module_stack.iter().position(|(m, _)| *m == u.module) {
            let msg = cycle_message(&self.module_stack[pos..], &u.module, site.module_from_arg);
            self.diags
                .push(diag(file, item, DiagCode::ModuleCycle, msg));
            return;
        }
        if site.depth >= MAX_MODULE_DEPTH {
            self.diags.push(diag(
                file,
                item,
                DiagCode::ModuleCycle,
                format!("module nesting exceeds {MAX_MODULE_DEPTH} levels"),
            ));
            return;
        }
        let Some(module) = self.modules.get(u.module.as_str()).copied() else {
            let msg = if site.redact {
                "module is undefined".to_string()
            } else {
                format!("module is undefined: {}", u.module)
            };
            self.diags.push(diag(file, item, DiagCode::RefModule, msg));
            return;
        };
        let Some(first) = module.src.steps.first() else {
            let msg = if site.redact {
                "module has no step".to_string()
            } else {
                format!("module has no step: {}", u.module)
            };
            self.diags.push(diag(file, item, DiagCode::Structure, msg));
            return;
        };
        let child = if site.inst == "main" {
            format!("{}@{}", u.module, u.instance)
        } else {
            format!("{}/{}@{}", site.inst, u.module, u.instance)
        };
        if !self.inst_ids.insert(child.clone()) {
            let msg = if site.redact {
                "duplicate module instance".to_string()
            } else {
                format!("duplicate module instance: {}@{}", u.module, u.instance)
            };
            self.diags.push(diag(file, item, DiagCode::DupId, msg));
            return;
        }
        // params に無い名前の引数は、綴り違いを黙って既定値に倒さないよう拒否する（PACK-19）。
        let mut unknown = false;
        for k in u.args.keys() {
            if !module.src.params.contains_key(k) {
                unknown = true;
                let msg = if safe_ident(k) {
                    format!("argument is not declared in module params: {k}")
                } else {
                    "argument is not declared in module params".to_string()
                };
                self.diags.push(diag(file, item, DiagCode::Structure, msg));
            }
        }
        if unknown {
            return;
        }
        let mut args = module.src.params.clone();
        args.extend(u.args);
        let args_size: usize = args.iter().map(|(k, v)| k.len() + v.len()).sum();
        if !self.budget.take(args_size as u64) || self.instances.len() >= MAX_COLLECTION_ELEMENTS {
            self.limit(file, item, "expanded instances exceed the IR size limit");
            return;
        }
        self.instances.push(Instance {
            id: child.clone(),
            module: Some(u.module),
            args: args.clone(),
            caller: Some(site.uid.to_string()),
        });
        self.nodes[site.node_idx].enter = Some(format!("{child}/{}", first.id));
        self.pending.push(PendingStarts {
            inst: child.clone(),
            scope: module,
            args: args.clone(),
        });
        self.module_stack
            .push((module.src.id.clone(), site.module_from_arg));
        self.expand_scope(&child, module, Some(&args), site.depth + 1, Some(site.uid));
        self.module_stack.pop();
    }

    /// 展開した実体ごとに、モジュールの途中開始を `<実体 ID>#<開始 ID>` として作る。
    /// 置換するのは `effects` と `requires` だけ。`start_ids` は章の開始の ID を含む既出の集合。
    pub(crate) fn module_starts(&mut self, start_ids: &mut BTreeSet<String>) -> Vec<Start> {
        let mut out = Vec::new();
        let pending = std::mem::take(&mut self.pending);
        'all: for p in &pending {
            for st in &p.scope.src.starts {
                let item = format!("start:{}", st.id);
                let id = format!("{}#{}", p.inst, st.id);
                if !start_ids.insert(id.clone()) {
                    self.diags.push(diag(
                        &p.scope.file,
                        &item,
                        DiagCode::DupId,
                        format!("duplicate start id: {}", st.id),
                    ));
                    continue;
                }
                let mut s = st.clone();
                s.id = id;
                s.node = format!("{}/{}", p.inst, st.node);
                let mut left = self.budget.left();
                let mut r = s
                    .effects
                    .iter_mut()
                    .try_for_each(|v| subst_value(v, &p.args, &mut left));
                if r.is_ok() {
                    if let Some(v) = s.requires.as_mut() {
                        r = subst_value(v, &p.args, &mut left);
                    }
                }
                if let Err(e) = r {
                    // 上限超過は残り全部の開始を打ち切る（同じ巨大な置換を実体ごとに繰り返させない）。
                    let too_large = e == SubstError::TooLarge;
                    self.diags.push(e.into_diag(&p.scope.file, &item));
                    if too_large {
                        self.halted = true;
                        break 'all;
                    }
                    continue;
                }
                for (code, msg) in
                    check_effects(&s.effects, "effects", &self.pack.workshop.fixtures, true)
                {
                    self.diags.push(diag(&p.scope.file, &item, code, msg));
                }
                let size = serde_json::to_vec(&s).map_or(u64::MAX, |b| b.len() as u64);
                if !self.budget.take(size) || out.len() >= MAX_COLLECTION_ELEMENTS {
                    self.limit(
                        &p.scope.file,
                        &item,
                        "expanded starts exceed the IR size limit",
                    );
                    break 'all;
                }
                if !self.uids.contains(&s.node) {
                    self.diags.push(diag(
                        &p.scope.file,
                        &item,
                        DiagCode::RefStep,
                        format!("start node is undefined: {}", st.node),
                    ));
                    continue;
                }
                out.push(s);
            }
        }
        out
    }
}

/// 章・モジュールのすべての Step で、`use` と `input`、`use` と `confirm` の同時指定を `E-STRUCTURE` で拒否する
/// （PACK-19）。`compile` が展開の結果を IR にする前に呼ぶ。`use` の Step は入力も確認も持たない
/// （語彙 v1 の「`use` の Step は確認を持たない」）ため、確認の検査（`confirm::check_confirms`・
/// `check_step_confirm`）は `use` の Step を飛ばす。同時指定をここで拒否しないと、確認の無い `input` を持つ Step が
/// `E-NO-CONFIRM` を免れて IR に載り、`use` の Step に書いた `confirm` は検査されないまま無視される（PACK-13）。
/// 1 つの Step が両方に違反するときは、`input`、`confirm` の順に 2 件を報告する。
/// 判定は宣言の有無だけで行う。置換は値を変えても欄の有無を変えないため、`${` を含むモジュールの宣言も
/// 置換前に判定でき、実体の数によらず定義ごとに 1 件になる。診断は固定文で、引数の値・宣言の中身を写さない。
pub(crate) fn check_use_steps(scopes: &[LoadedScope]) -> Vec<Diagnostic> {
    scopes
        .iter()
        .flat_map(|scope| {
            scope
                .src
                .steps
                .iter()
                .filter(|s| s.use_module.is_some())
                .flat_map(move |s| {
                    [
                        (
                            s.input.is_some(),
                            "a step cannot declare both `use` and `input`",
                        ),
                        (
                            s.confirm.is_some(),
                            "a step cannot declare both `use` and `confirm`",
                        ),
                    ]
                    .into_iter()
                    .filter(|(has, _)| *has)
                    .map(move |(_, msg)| {
                        crate::input::diag(scope, &s.id, DiagCode::Structure, msg.to_string())
                    })
                })
        })
        .collect()
}

/// `use` を持つ Step の位置。展開の文脈を 1 つにまとめて渡す。
struct UseSite<'s> {
    inst: &'s str,
    scope: &'s LoadedScope,
    uid: &'s str,
    item: &'s str,
    node_idx: usize,
    depth: usize,
    /// 引数を置換した後の `use` か（モジュールの中の呼び出し）。置換後のモジュール名・実体名は引数の値を
    /// 含みうるため、真のときは診断に名前を入れない。
    redact: bool,
    /// 置換前の `use.module` が `${` を含むか。真のとき、巡回の経路は引数の値を含みうるため診断に出さない。
    module_from_arg: bool,
}

/// 巡回の診断の文面。経路は教材に書かれた（置換前の）モジュール ID だけで作る。経路の段や閉じる呼び出しが
/// 引数に由来するとき、または ID が `safe_ident` を通らないときは、固定文に倒す（引数の値を写さない）。
fn cycle_message(path: &[(String, bool)], closing: &str, closing_from_arg: bool) -> String {
    let safe = !closing_from_arg
        && path.iter().all(|(m, from_arg)| !from_arg && safe_ident(m))
        && safe_ident(closing);
    if !safe {
        return "module call cycle".to_string();
    }
    let mut route: Vec<&str> = path.iter().map(|(m, _)| m.as_str()).collect();
    route.push(closing);
    format!("module call cycle: {}", route.join(" → "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(kv: &[(&str, &str)]) -> BTreeMap<String, String> {
        kv.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn substitutes_once_and_keeps_literal_text() {
        let a = args(&[("x", "${y}"), ("y", "Y")]);
        assert_eq!(substitute("a-${x}-b", &a, &mut 100).unwrap(), "a-${y}-b");
        assert_eq!(substitute("plain", &a, &mut 100).unwrap(), "plain");
        assert_eq!(substitute("${x}${y}", &a, &mut 100).unwrap(), "${y}Y");
    }

    #[test]
    fn rejects_undefined_unterminated_and_oversize() {
        let a = args(&[("x", "12345")]);
        assert_eq!(
            substitute("${zz}", &a, &mut 100),
            Err(SubstError::Undefined(Some("zz".into())))
        );
        // 名前が識別子として安全でないときは、名前を持たない。
        assert_eq!(
            substitute("${a b}", &a, &mut 100),
            Err(SubstError::Undefined(None))
        );
        assert_eq!(
            substitute("${x", &a, &mut 100),
            Err(SubstError::Unterminated)
        );
        assert_eq!(substitute("${x}", &a, &mut 4), Err(SubstError::TooLarge));
        assert!(substitute("${x}", &a, &mut 5).is_ok());
    }

    #[test]
    fn substitution_budget_accumulates_across_strings() {
        // 1 文字列では収まる引数も、同じ予算を共有する複数の文字列では累積して超える。
        let a = args(&[("x", "1234567890")]);
        let mut left = 25;
        assert!(substitute("${x}", &a, &mut left).is_ok());
        assert!(substitute("${x}", &a, &mut left).is_ok());
        assert_eq!(left, 5);
        assert_eq!(substitute("${x}", &a, &mut left), Err(SubstError::TooLarge));
        // 小さな入力が、1 つの値の中の多数の参照で巨大に展開する場合も打ち切る。
        let big = args(&[("x", &"a".repeat(10_000))]);
        let mut v = serde_json::json!({"rules": vec!["${x}"; 1_000]});
        let mut left = 100_000;
        assert_eq!(
            subst_value(&mut v, &big, &mut left),
            Err(SubstError::TooLarge)
        );
    }

    #[test]
    fn step_substitution_skips_ids_bodies_and_transition_targets() {
        let a = args(&[("x", "X")]);
        let step: Step = serde_json::from_value(serde_json::json!({
            "id": "s-${x}", "title": "t ${x}", "body": "b-${x}.md",
            "body_levels": {"B": "c-${x}.md"},
            "next": [{"to": "n-${x}", "when": {"param": "${x}"}}],
            "rules": [{"k": "${x}"}]
        }))
        .unwrap();
        let s = subst_step(&step, &a, &mut 1000).unwrap().unwrap();
        assert_eq!(s.id, "s-${x}");
        assert_eq!(s.title, "t X");
        assert_eq!(s.body.as_deref(), Some("b-${x}.md"));
        assert_eq!(s.body_levels["B"], "c-${x}.md");
        assert_eq!(s.next[0].to, "n-${x}");
        assert_eq!(s.next[0].when, Some(serde_json::json!({"param": "X"})));
        assert_eq!(s.rules, vec![serde_json::json!({"k": "X"})]);
    }

    #[test]
    fn use_src_rejects_unknown_fields_and_non_string_args() {
        let ok = serde_json::json!({"module": "m", "instance": "i", "args": {"a": "1"}});
        assert!(serde_json::from_value::<UseSrc>(ok).is_ok());
        for bad in [
            serde_json::json!({"module": "m", "instance": "i", "args": {"a": 1}}),
            serde_json::json!({"module": "m", "instance": "i", "extra": 1}),
            serde_json::json!({"module": "m"}),
        ] {
            assert!(serde_json::from_value::<UseSrc>(bad).is_err());
        }
    }

    fn stack(v: &[(&str, bool)]) -> Vec<(String, bool)> {
        v.iter().map(|(m, f)| ((*m).to_string(), *f)).collect()
    }

    #[test]
    fn cycle_message_shows_the_route_only_for_authored_ids() {
        assert_eq!(
            cycle_message(&stack(&[("a", false), ("b", false)]), "a", false),
            "module call cycle: a → b → a"
        );
        assert_eq!(
            cycle_message(&stack(&[("a", false)]), "a", false),
            "module call cycle: a → a"
        );
        // 引数に由来する段・閉じる呼び出し、`safe_ident` を通らない ID は固定文にする。
        assert_eq!(
            cycle_message(&stack(&[("a", true)]), "a", false),
            "module call cycle"
        );
        assert_eq!(
            cycle_message(&stack(&[("a", false)]), "a", true),
            "module call cycle"
        );
        assert_eq!(
            cycle_message(&stack(&[("a b", false)]), "a b", false),
            "module call cycle"
        );
    }
}
