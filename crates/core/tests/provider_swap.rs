//! 状態変化の提供元の差し替えの自動試験（TASK-187 2/2、EXT-4・D-53）。
//!
//! 役割: 提供元を別の実装へ差し替えても、同じ教材が最後まで動くことを確かめる。教材と画面の側の
//! 代役 [`play`] は、トレイト [`StateChangeProvider`] の境界だけで提供元を受け取り、具体型も
//! 模擬か実かも参照しない。
//!
//! 本段階での「同じ教材が最後まで動く」の定義（core にはまだ `Session` も IR の読み込み器も無い）:
//! - 教材: `testdata/engine-event-consistency/test-data/min-3step.json` の 3 Step を、この試験の中に
//!   手で写したデータ（[`Material`]）として持つ。IR の解釈はしない（TASK-41・TASK-204 の範囲）。
//!   写しが出所と食い違わないことは別の試験で確かめる
//! - 台本: 同じ `testdata` の `playthrough.toml` の `pt-min`。入力列と期待（`accepted`・`progress`・
//!   `achieved`）を読む。期待はデータから得るため、提供元の出力と独立した正解になる
//! - 最後まで動く: 台本の全入力を流し、各入力の結果イベントが台本の期待と一致し、最終状態が完了
//!   （final の Step が達成済み）になること。完了はイベントではなく [`State::done`] で判定する
//!
//! 2 つの提供元（[`InterpretingProvider`]・[`TableProvider`]）は、内部の計算の手順を変えてある。
//! どちらも試験の中に閉じた中立のもので、エンジンの実装ではない。TASK-204 で `Session` と本物の
//! IR が入ったら、[`Material`] と台本をそちらへ付け替える。
//!
//! 対象外: 台本の `error_inputs`（拒否の分類。入力の判定の契約 TASK-256 の後）。
//! データは信頼できない入力として扱い、失敗時の出力にはパスと項目名だけを出す（本文は出さない）。

mod common;

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use common::property::{check, op_sequence, DEFAULT_CASES};
use fandhe_workshop_core::provider::StateChangeProvider;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// 提供元の関連型（2 つの提供元で共有する）
//
// 差し替えには State・Input・Event が同じであることが要る。トレイトの関連型を `play` の境界で
// 固定するため、違う型の提供元はコンパイルできない。
// ---------------------------------------------------------------------------

/// 承認の入力。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Decision {
    Approve,
    Reject,
}

/// 台本の入力の種類だけを持つ入力。
#[derive(Clone, Debug, PartialEq, Eq)]
enum Input {
    Command { name: String, arg: Option<String> },
    Tick,
    Point { view: String, path: String },
    Choice(String),
    Approval(Decision),
}

/// 拒否の理由。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reason {
    Done,
    Busy,
    NotInStep,
}

/// 台本の期待に 1 対 1 で対応するイベント（拒否を加える）。
#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Accepted,
    Progress,
    Achieved,
    Rejected(Reason),
}

/// 世界状態と進行の状態。世界状態は `world`（属性の写し）に置き、提供元は持たない。
#[derive(Clone, Debug, PartialEq, Eq)]
struct State {
    step: usize,
    accepted: bool,
    remaining: usize,
    done: bool,
    world: BTreeMap<String, String>,
}

impl State {
    /// 台本の `start = "s-1"` に当たる初期状態。
    fn start() -> Self {
        State {
            step: 0,
            accepted: false,
            remaining: 0,
            done: false,
            world: BTreeMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// 教材（min-3step.json の手写し）
// ---------------------------------------------------------------------------

/// 属性の設定（キー、値）。
type Effect = (&'static str, &'static str);

/// Step が受け付ける入力。
enum InputSpec {
    Command {
        name: &'static str,
        effects: Vec<Effect>,
    },
    Choice {
        options: Vec<(&'static str, Vec<Effect>)>,
    },
    Approval {
        effects: Vec<Effect>,
    },
}

/// 達成の確認。
enum Confirm {
    PointAt {
        view: &'static str,
        path: &'static str,
    },
    None,
    Approve,
}

/// 1 Step の定義。
struct StepDef {
    id: &'static str,
    input: InputSpec,
    /// 処理の段ごとの属性の設定。段数が `Tick` の回数になる。
    processing: Vec<Vec<Effect>>,
    confirm: Confirm,
    is_final: bool,
}

/// 教材。提供元はこれだけを持つ（不変）。
struct Material {
    steps: Vec<StepDef>,
}

impl Material {
    /// `min-3step.json` の 3 Step。後続で本物の IR へ付け替える。
    fn min_3step() -> Self {
        Material {
            steps: vec![
                StepDef {
                    id: "st-1",
                    input: InputSpec::Command {
                        name: "make-note",
                        effects: vec![
                            ("notes/draft.kind", "none"),
                            ("notes/draft.state", "created"),
                        ],
                    },
                    processing: vec![
                        vec![],
                        vec![],
                        vec![("notes/draft.state", "writing")],
                        vec![],
                        vec![("notes/draft.state", "saved")],
                    ],
                    confirm: Confirm::PointAt {
                        view: "v-files",
                        path: "notes/draft",
                    },
                    is_final: false,
                },
                StepDef {
                    id: "st-2",
                    input: InputSpec::Choice {
                        options: vec![
                            ("opt-memo", vec![("notes/draft.kind", "memo")]),
                            ("opt-todo", vec![("notes/draft.kind", "todo")]),
                        ],
                    },
                    processing: vec![],
                    confirm: Confirm::None,
                    is_final: false,
                },
                StepDef {
                    id: "st-3",
                    input: InputSpec::Approval {
                        effects: vec![("notes/draft.state", "published")],
                    },
                    processing: vec![],
                    confirm: Confirm::Approve,
                    is_final: true,
                },
            ],
        }
    }
}

/// 引数の形 `kebab_case`（小文字・数字・ハイフン）。
fn kebab_ok(arg: &str) -> bool {
    !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn set_all(world: &mut BTreeMap<String, String>, effects: &[Effect]) {
    for (k, v) in effects {
        world.insert((*k).to_string(), (*v).to_string());
    }
}

fn reject(state: State, reason: Reason) -> (State, Vec<Event>) {
    (state, vec![Event::Rejected(reason)])
}

// ---------------------------------------------------------------------------
// 提供元 1: 規則を呼ぶたびに解釈する
// ---------------------------------------------------------------------------

/// `apply` のたびに現在の Step の定義を引き、規則を解釈する提供元。
struct InterpretingProvider {
    material: Material,
}

impl InterpretingProvider {
    fn advance(&self, s: &mut State) {
        if self.material.steps[s.step].is_final {
            s.done = true;
        } else {
            s.step += 1;
        }
        s.accepted = false;
        s.remaining = 0;
    }
}

impl StateChangeProvider for InterpretingProvider {
    type State = State;
    type Input = Input;
    type Event = Event;

    fn apply(&self, mut s: State, input: Input) -> (State, Vec<Event>) {
        if s.done {
            return reject(s, Reason::Done);
        }
        let step = &self.material.steps[s.step];

        // 処理中は Tick だけが進める。
        if s.remaining > 0 {
            if input != Input::Tick {
                return reject(s, Reason::Busy);
            }
            let stage = step.processing.len() - s.remaining;
            set_all(&mut s.world, &step.processing[stage]);
            s.remaining -= 1;
            if s.remaining == 0 && matches!(step.confirm, Confirm::None) {
                self.advance(&mut s);
            }
            return (s, vec![Event::Progress]);
        }

        // 入力の受理。
        if !s.accepted {
            let effects = match (&step.input, &input) {
                (
                    InputSpec::Command { name, effects },
                    Input::Command {
                        name: n,
                        arg: Some(a),
                    },
                ) if n == name && kebab_ok(a) => Some(effects),
                (InputSpec::Choice { options }, Input::Choice(id)) => options
                    .iter()
                    .find(|(o, _)| o == id)
                    .map(|(_, effects)| effects),
                (InputSpec::Approval { effects }, Input::Approval(Decision::Approve)) => {
                    Some(effects)
                }
                _ => None,
            };
            let Some(effects) = effects else {
                return reject(s, Reason::NotInStep);
            };
            set_all(&mut s.world, effects);
            s.accepted = true;
            s.remaining = step.processing.len();
            if s.remaining == 0 && matches!(step.confirm, Confirm::None) {
                self.advance(&mut s);
            }
            return (s, vec![Event::Accepted]);
        }

        // 達成の確認。
        let confirmed = match (&step.confirm, &input) {
            (Confirm::PointAt { view, path }, Input::Point { view: v, path: p }) => {
                v == view && p == path
            }
            (Confirm::Approve, Input::Approval(Decision::Approve)) => true,
            _ => false,
        };
        if !confirmed {
            return reject(s, Reason::NotInStep);
        }
        self.advance(&mut s);
        (s, vec![Event::Achieved])
    }
}

// ---------------------------------------------------------------------------
// 提供元 2: 構築時に遷移表を作り、apply は表を引くだけ
// ---------------------------------------------------------------------------

/// 入力の分類。表の鍵に使う。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Class {
    Command(String),
    Choice(String),
    Approve,
    Decline,
    Tick,
    Point(String, String),
}

/// 進行の局面。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Phase {
    Idle,
    Stage(usize),
    Awaiting,
}

/// 遷移後の位置。
#[derive(Clone, Copy)]
enum Exit {
    Stay,
    Next,
    Finish,
}

/// 表の 1 行。遷移後の値を絶対値で持つ。
struct Row {
    event: Event,
    effects: Vec<Effect>,
    accepted: bool,
    remaining: usize,
    exit: Exit,
}

/// 遷移表を引くだけの提供元。
struct TableProvider {
    rows: HashMap<(usize, Phase, Class), Row>,
    stage_counts: Vec<usize>,
}

fn classify(input: &Input) -> Option<Class> {
    match input {
        Input::Command { name, arg: Some(a) } if kebab_ok(a) => Some(Class::Command(name.clone())),
        Input::Command { .. } => None,
        Input::Tick => Some(Class::Tick),
        Input::Point { view, path } => Some(Class::Point(view.clone(), path.clone())),
        Input::Choice(id) => Some(Class::Choice(id.clone())),
        Input::Approval(Decision::Approve) => Some(Class::Approve),
        Input::Approval(Decision::Reject) => Some(Class::Decline),
    }
}

impl TableProvider {
    fn new(material: &Material) -> Self {
        let mut rows = HashMap::new();
        let mut stage_counts = Vec::new();
        for (i, step) in material.steps.iter().enumerate() {
            let n = step.processing.len();
            stage_counts.push(n);
            let exit_after = |remaining: usize| {
                if remaining != 0 || !matches!(step.confirm, Confirm::None) {
                    Exit::Stay
                } else if step.is_final {
                    Exit::Finish
                } else {
                    Exit::Next
                }
            };

            // 受理。
            let accepts: Vec<(Class, &Vec<Effect>)> = match &step.input {
                InputSpec::Command { name, effects } => {
                    vec![(Class::Command((*name).to_string()), effects)]
                }
                InputSpec::Choice { options } => options
                    .iter()
                    .map(|(id, e)| (Class::Choice((*id).to_string()), e))
                    .collect(),
                InputSpec::Approval { effects } => vec![(Class::Approve, effects)],
            };
            for (class, effects) in accepts {
                rows.insert(
                    (i, Phase::Idle, class),
                    Row {
                        event: Event::Accepted,
                        effects: effects.clone(),
                        accepted: true,
                        remaining: n,
                        exit: exit_after(n),
                    },
                );
            }

            // 処理の各段。
            for (k, effects) in step.processing.iter().enumerate() {
                let remaining = n - k - 1;
                rows.insert(
                    (i, Phase::Stage(k), Class::Tick),
                    Row {
                        event: Event::Progress,
                        effects: effects.clone(),
                        accepted: true,
                        remaining,
                        exit: exit_after(remaining),
                    },
                );
            }

            // 達成の確認。
            let exit = if step.is_final {
                Exit::Finish
            } else {
                Exit::Next
            };
            let confirm_class = match &step.confirm {
                Confirm::PointAt { view, path } => {
                    Some(Class::Point((*view).to_string(), (*path).to_string()))
                }
                Confirm::Approve => Some(Class::Approve),
                Confirm::None => None,
            };
            if let Some(class) = confirm_class {
                rows.insert(
                    (i, Phase::Awaiting, class),
                    Row {
                        event: Event::Achieved,
                        effects: vec![],
                        accepted: false,
                        remaining: 0,
                        exit,
                    },
                );
            }
        }
        TableProvider { rows, stage_counts }
    }
}

impl StateChangeProvider for TableProvider {
    type State = State;
    type Input = Input;
    type Event = Event;

    fn apply(&self, mut s: State, input: Input) -> (State, Vec<Event>) {
        if s.done {
            return reject(s, Reason::Done);
        }
        let phase = if s.remaining > 0 {
            Phase::Stage(self.stage_counts[s.step] - s.remaining)
        } else if s.accepted {
            Phase::Awaiting
        } else {
            Phase::Idle
        };
        let row = classify(&input).and_then(|c| self.rows.get(&(s.step, phase, c)));
        let Some(row) = row else {
            let reason = if s.remaining > 0 {
                Reason::Busy
            } else {
                Reason::NotInStep
            };
            return reject(s, reason);
        };
        set_all(&mut s.world, &row.effects);
        s.accepted = row.accepted;
        s.remaining = row.remaining;
        match row.exit {
            Exit::Stay => {}
            Exit::Next => {
                s.step += 1;
                s.accepted = false;
                s.remaining = 0;
            }
            Exit::Finish => {
                s.done = true;
                s.accepted = false;
                s.remaining = 0;
            }
        }
        (s, vec![row.event.clone()])
    }
}

// ---------------------------------------------------------------------------
// 教材と画面の側の代役、および合成の根
// ---------------------------------------------------------------------------

/// 提供元の種類。合成の根（[`make_provider`]）だけが知る。
#[derive(Clone, Copy, Debug)]
enum Kind {
    Interpreting,
    Table,
}

/// 入力列を初期状態から流す。提供元の具体型・種類を一切参照しない。
fn play<P>(provider: &P, inputs: &[Input]) -> (State, Vec<Event>)
where
    P: StateChangeProvider<State = State, Input = Input, Event = Event> + ?Sized,
{
    let mut state = State::start();
    let mut events = Vec::new();
    for input in inputs {
        let (next, mut ev) = provider.apply(state, input.clone());
        state = next;
        events.append(&mut ev);
    }
    (state, events)
}

/// 種類から実行時に提供元を作る。
fn make_provider(
    kind: Kind,
) -> Box<dyn StateChangeProvider<State = State, Input = Input, Event = Event>> {
    match kind {
        Kind::Interpreting => Box::new(InterpretingProvider {
            material: Material::min_3step(),
        }),
        Kind::Table => Box::new(TableProvider::new(&Material::min_3step())),
    }
}

// ---------------------------------------------------------------------------
// 台本の読み込み
// ---------------------------------------------------------------------------

const TEST_DATA: &str = "engine-event-consistency/test-data";

/// `testdata/` の絶対パス。repo の外へ出ていないことも確かめる。
fn testdata_root() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .join("../..")
        .canonicalize()
        .map_err(|e| format!("repo root not resolvable: {e}"))?;
    let root = manifest_dir
        .join("../../testdata")
        .canonicalize()
        .map_err(|e| format!("testdata/ not found: {e}"))?;
    if !root.starts_with(&repo_root) {
        return Err("testdata/ resolves outside the repository".to_string());
    }
    Ok(root)
}

fn read_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// 台本の 1 要素（`index` 番目）を入力と期待へ変換する。未知のキー・値は `Err`。
fn parse_step(index: usize, table: &toml::Table) -> Result<(Input, Event), String> {
    let at = |msg: &str| format!("playthrough pt-min steps[{index}]: {msg}");
    let text = |key: &str| -> Result<&str, String> {
        table
            .get(key)
            .and_then(toml::Value::as_str)
            .ok_or_else(|| at(&format!("`{key}` is not a string")))
    };
    let input_keys = ["cmd", "tick", "ui", "choice", "approval"];
    let present: Vec<&str> = input_keys
        .iter()
        .copied()
        .filter(|k| table.contains_key(*k))
        .collect();
    if let Some(unknown) = table
        .keys()
        .find(|k| *k != "expect" && !input_keys.contains(&k.as_str()))
    {
        return Err(at(&format!("unknown key `{unknown}`")));
    }
    let input = match present.as_slice() {
        ["cmd"] => {
            let line = text("cmd")?
                .strip_prefix('/')
                .ok_or_else(|| at("`cmd` has no leading slash"))?;
            match line.split_once(' ') {
                Some((name, arg)) => Input::Command {
                    name: name.to_string(),
                    arg: Some(arg.to_string()),
                },
                None => Input::Command {
                    name: line.to_string(),
                    arg: None,
                },
            }
        }
        ["tick"] => Input::Tick,
        ["ui"] => {
            let (view, path) = text("ui")?
                .split_once(' ')
                .ok_or_else(|| at("`ui` is not `<view> <path>`"))?;
            Input::Point {
                view: view.to_string(),
                path: path.to_string(),
            }
        }
        ["choice"] => Input::Choice(text("choice")?.to_string()),
        ["approval"] => match text("approval")? {
            "approve" => Input::Approval(Decision::Approve),
            "reject" => Input::Approval(Decision::Reject),
            _ => return Err(at("unknown `approval` value")),
        },
        _ => return Err(at("expected exactly one input key")),
    };
    let event = match text("expect")? {
        "accepted" => Event::Accepted,
        "progress" => Event::Progress,
        "achieved" => Event::Achieved,
        _ => return Err(at("unknown `expect` value")),
    };
    Ok((input, event))
}

/// `playthrough.toml` の `pt-min` を（入力列, 期待のイベント列）として読む。
fn load_pt_min() -> Result<(Vec<Input>, Vec<Event>), String> {
    let path = testdata_root()?
        .join(TEST_DATA)
        .join("min-3step-pack/tests/playthrough.toml");
    let value: toml::Value =
        toml::from_str(&read_text(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
    let shown = path.display();
    let pt = value
        .get("playthroughs")
        .and_then(toml::Value::as_array)
        .and_then(|all| {
            all.iter()
                .find(|p| p.get("id").and_then(toml::Value::as_str) == Some("pt-min"))
        })
        .ok_or_else(|| format!("{shown}: playthrough pt-min missing"))?;
    if pt.get("end").and_then(toml::Value::as_str) != Some("complete") {
        return Err(format!("{shown}: pt-min `end` is not complete"));
    }
    let segments = pt
        .get("segments")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{shown}: pt-min `segments` missing"))?;
    let [segment] = segments.as_slice() else {
        return Err(format!("{shown}: pt-min must have exactly one segment"));
    };
    if segment.get("start").and_then(toml::Value::as_str) != Some("s-1") {
        return Err(format!("{shown}: pt-min segment does not start at s-1"));
    }
    let steps = segment
        .get("steps")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{shown}: pt-min `steps` missing"))?;
    let mut inputs = Vec::new();
    let mut expects = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        let table = step
            .as_table()
            .ok_or_else(|| format!("{shown}: steps[{i}] is not a table"))?;
        let (input, event) = parse_step(i, table).map_err(|e| format!("{shown}: {e}"))?;
        inputs.push(input);
        expects.push(event);
    }
    Ok((inputs, expects))
}

// ---------------------------------------------------------------------------
// 試験
// ---------------------------------------------------------------------------

#[test]
fn interpreting_provider_plays_the_script_to_completion() -> Result<(), String> {
    let (inputs, expects) = load_pt_min()?;
    let provider = InterpretingProvider {
        material: Material::min_3step(),
    };
    let (state, events) = play(&provider, &inputs);
    assert_eq!(events, expects);
    assert!(state.done);
    Ok(())
}

#[test]
fn table_provider_plays_the_script_to_completion() -> Result<(), String> {
    let (inputs, expects) = load_pt_min()?;
    let provider = TableProvider::new(&Material::min_3step());
    let (state, events) = play(&provider, &inputs);
    assert_eq!(events, expects);
    assert!(state.done);
    Ok(())
}

#[test]
fn swapping_the_provider_keeps_the_result() -> Result<(), String> {
    let (inputs, _) = load_pt_min()?;
    let a = InterpretingProvider {
        material: Material::min_3step(),
    };
    let b = TableProvider::new(&Material::min_3step());
    assert_eq!(play(&a, &inputs), play(&b, &inputs));
    Ok(())
}

#[test]
fn provider_can_be_swapped_at_runtime() -> Result<(), String> {
    let (inputs, expects) = load_pt_min()?;
    let mut results = Vec::new();
    for kind in [Kind::Interpreting, Kind::Table] {
        let provider = make_provider(kind);
        let (state, events) = play(provider.as_ref(), &inputs);
        assert_eq!(events, expects, "{kind:?}");
        assert!(state.done, "{kind:?}");
        results.push((state, events));
    }
    assert_eq!(results[0], results[1]);
    Ok(())
}

/// 台本の外の入力を含む、1 入力の戦略。
fn any_input() -> impl Strategy<Value = Input> {
    let command = |name: &str, arg: Option<&str>| Input::Command {
        name: name.to_string(),
        arg: arg.map(str::to_string),
    };
    let point = |path: &str| Input::Point {
        view: "v-files".to_string(),
        path: path.to_string(),
    };
    prop_oneof![
        3 => Just(Input::Tick),
        2 => Just(command("make-note", Some("first-note"))),
        1 => Just(command("make-note", Some("Bad_Name"))),
        1 => Just(command("make-note", None)),
        1 => Just(command("nothing", Some("x"))),
        2 => Just(point("notes/draft")),
        1 => Just(point("notes/other")),
        1 => Just(Input::Choice("opt-memo".to_string())),
        1 => Just(Input::Choice("opt-todo".to_string())),
        1 => Just(Input::Choice("opt-none".to_string())),
        2 => Just(Input::Approval(Decision::Approve)),
        1 => Just(Input::Approval(Decision::Reject)),
    ]
}

/// 提供元 2 つを 1 入力ずつ並べて流し、毎回の状態とイベントの一致を確かめる。
fn lockstep(inputs: &[Input]) -> Result<(), TestCaseError> {
    let a = InterpretingProvider {
        material: Material::min_3step(),
    };
    let b = TableProvider::new(&Material::min_3step());
    let (mut sa, mut sb) = (State::start(), State::start());
    for input in inputs {
        let (na, ea) = a.apply(sa, input.clone());
        let (nb, eb) = b.apply(sb, input.clone());
        prop_assert_eq!(&na, &nb, "state after {:?}", input);
        prop_assert_eq!(&ea, &eb, "events after {:?}", input);
        (sa, sb) = (na, nb);
    }
    Ok(())
}

#[test]
fn random_inputs_keep_both_providers_in_step() {
    check(
        "provider-swap-random",
        DEFAULT_CASES,
        op_sequence(any_input()),
        |inputs| lockstep(&inputs),
    );
}

#[test]
fn script_with_noise_keeps_both_providers_in_step() -> Result<(), String> {
    let (script, _) = load_pt_min()?;
    let len = script.len();
    // 台本の途中へ乱数の入力を差し込み、完了の近くまで届く列で比べる。
    let noise = proptest::collection::vec((0..=len, any_input()), 0..6);
    check(
        "provider-swap-noisy-script",
        DEFAULT_CASES,
        noise,
        |noise| {
            let mut inputs = script.clone();
            for (offset, (at, input)) in noise.into_iter().enumerate() {
                inputs.insert((at + offset).min(inputs.len()), input);
            }
            lockstep(&inputs)
        },
    );
    Ok(())
}

/// IR の効果の列を、手写しと同じ（`path.attr`、値）の組の列に直す。
fn ir_effects(effects: Option<&serde_json::Value>) -> Result<Vec<(String, String)>, String> {
    let list = effects
        .and_then(serde_json::Value::as_array)
        .ok_or("`effects` missing")?;
    let text = |v: &serde_json::Value, key: &str| -> Result<String, String> {
        v.get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| format!("`{key}` missing"))
    };
    let mut out = Vec::new();
    for effect in list {
        if let Some(set) = effect.get("set_attr") {
            out.push((
                format!("{}.{}", text(set, "path")?, text(set, "attr")?),
                text(set, "value")?,
            ));
        } else if let Some(put) = effect.get("put") {
            let path = text(put, "path")?;
            let attrs = put
                .get("attrs")
                .and_then(serde_json::Value::as_object)
                .ok_or("`attrs` missing")?;
            for (attr, value) in attrs {
                let value = value.as_str().ok_or("attr value is not a string")?;
                out.push((format!("{path}.{attr}"), value.to_string()));
            }
        } else {
            return Err(format!("unsupported effect: {effect}"));
        }
    }
    Ok(out)
}

fn own_effects(effects: &[Effect]) -> Vec<(String, String)> {
    effects
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect()
}

/// 受理する規則（`on` の条件が合い、結果が `accept`）の効果を引く。
fn accept_effects(
    step: &serde_json::Value,
    on_key: &str,
    on_value: &str,
) -> Result<Vec<(String, String)>, String> {
    let rules = step
        .get("rules")
        .and_then(serde_json::Value::as_array)
        .ok_or("`rules` missing")?;
    let rule = rules
        .iter()
        .find(|r| {
            r.get("on")
                .and_then(|o| o.get(on_key))
                .and_then(serde_json::Value::as_str)
                == Some(on_value)
                && r.get("outcome")
                    .and_then(|o| o.get("kind"))
                    .and_then(serde_json::Value::as_str)
                    == Some("accept")
        })
        .ok_or_else(|| format!("no accept rule for {on_key}={on_value}"))?;
    ir_effects(rule.get("outcome").and_then(|o| o.get("effects")))
}

/// 手写しの動作上の値（名前・選択肢 ID・確認先・各段と受理時の効果）を IR と照合する。
fn check_behavior(step: &serde_json::Value, def: &StepDef) -> Result<(), String> {
    let id = def.id;
    let input = step.get("input").ok_or("`input` missing")?;
    let str_of = |v: &serde_json::Value, key: &str| {
        v.get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    };
    match &def.input {
        InputSpec::Command { name, effects } => {
            assert_eq!(str_of(input, "name").as_deref(), Some(*name), "{id}");
            assert_eq!(
                accept_effects(step, "command", name)?,
                own_effects(effects),
                "{id}"
            );
        }
        InputSpec::Choice { options } => {
            let ir_ids: Vec<String> = input
                .get("options")
                .and_then(serde_json::Value::as_array)
                .ok_or("`options` missing")?
                .iter()
                .filter_map(|o| str_of(o, "id"))
                .collect();
            let own_ids: Vec<String> = options.iter().map(|(o, _)| (*o).to_string()).collect();
            assert_eq!(ir_ids, own_ids, "{id}");
            for (opt, effects) in options {
                assert_eq!(
                    accept_effects(step, "option", opt)?,
                    own_effects(effects),
                    "{id}/{opt}"
                );
            }
        }
        InputSpec::Approval { effects } => {
            assert_eq!(
                accept_effects(step, "approval", "approve")?,
                own_effects(effects),
                "{id}"
            );
        }
    }
    let stages = step
        .get("processing")
        .and_then(serde_json::Value::as_array)
        .ok_or("`processing` missing")?;
    assert_eq!(stages.len(), def.processing.len(), "{id}");
    for (n, (stage, own)) in stages.iter().zip(&def.processing).enumerate() {
        assert_eq!(
            ir_effects(stage.get("effects"))?,
            own_effects(own),
            "{id} stage {n}"
        );
    }
    if let Confirm::PointAt { view, path } = &def.confirm {
        let target = step
            .get("confirm")
            .and_then(|c| c.get("targets"))
            .and_then(serde_json::Value::as_array)
            .and_then(|t| t.first())
            .ok_or("`confirm.targets` missing")?;
        assert_eq!(str_of(target, "view").as_deref(), Some(*view), "{id}");
        assert_eq!(str_of(target, "path").as_deref(), Some(*path), "{id}");
    }
    Ok(())
}

#[test]
fn material_matches_the_ir_source() -> Result<(), String> {
    let path = testdata_root()?.join(TEST_DATA).join("min-3step.json");
    let ir: serde_json::Value =
        serde_json::from_str(&read_text(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
    let nodes = ir
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("{}: `nodes` missing", path.display()))?;
    let material = Material::min_3step();
    assert_eq!(nodes.len(), material.steps.len());
    let kind_of = |v: Option<&serde_json::Value>| {
        v.and_then(|x| x.get("kind"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    };
    for (node, def) in nodes.iter().zip(&material.steps) {
        let step = node
            .get("step")
            .ok_or_else(|| format!("{}: `step` missing", path.display()))?;
        assert_eq!(
            step.get("id").and_then(serde_json::Value::as_str),
            Some(def.id)
        );
        assert_eq!(
            step.get("final").and_then(serde_json::Value::as_bool),
            Some(def.is_final),
            "{}",
            def.id
        );
        let input_kind = match def.input {
            InputSpec::Command { .. } => "command",
            InputSpec::Choice { .. } => "choice",
            InputSpec::Approval { .. } => "approval",
        };
        assert_eq!(
            kind_of(step.get("input")).as_deref(),
            Some(input_kind),
            "{}",
            def.id
        );
        let confirm_kind = match def.confirm {
            Confirm::PointAt { .. } => "point_at",
            Confirm::None => "none",
            Confirm::Approve => "approve",
        };
        assert_eq!(
            kind_of(step.get("confirm")).as_deref(),
            Some(confirm_kind),
            "{}",
            def.id
        );
        assert_eq!(
            step.get("processing")
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
            Some(def.processing.len()),
            "{}",
            def.id
        );
        check_behavior(step, def)?;
    }
    Ok(())
}
