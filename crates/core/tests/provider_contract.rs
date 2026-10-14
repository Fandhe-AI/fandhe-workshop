//! 状態変化の提供元の境界（TASK-187、版 1）の適合試験。
//!
//! 版・公開パス・generic と `dyn` の両方で受け取れること（object safe と包括 impl）を固定する。
//! 試験の提供元は試験の中で閉じた中立のもので、エンジンの実装ではない。

use fandhe_workshop_core::provider::{StateChangeProvider, PROVIDER_CONTRACT_VERSION};

/// 入力を状態へ足し、足した値をイベントとして 1 件返すだけの中立の提供元。
struct Adder;

impl StateChangeProvider for Adder {
    type State = u64;
    type Input = u64;
    type Event = u64;

    fn apply(&self, state: u64, input: u64) -> (u64, Vec<u64>) {
        (state + input, vec![input])
    }
}

/// TASK-204 の `Session` の代役。generic に提供元を受け取り、入力列を畳み込む。
fn run<P: StateChangeProvider>(
    p: &P,
    s0: P::State,
    inputs: Vec<P::Input>,
) -> (P::State, Vec<P::Event>) {
    let mut state = s0;
    let mut events = Vec::new();
    for input in inputs {
        let (next, mut ev) = p.apply(state, input);
        state = next;
        events.append(&mut ev);
    }
    (state, events)
}

#[test]
fn version_is_1() {
    assert_eq!(PROVIDER_CONTRACT_VERSION, 1);
}

#[test]
fn provider_is_reachable_from_crate_root() {
    // 公開パスで名前を引けること（use 文のコンパイルが確認の本体）。
    let _: Option<&dyn StateChangeProvider<State = u64, Input = u64, Event = u64>> = None;
}

#[test]
fn generic_consumer_can_take_provider() {
    let (state, events) = run(&Adder, 1, vec![2, 3, 4]);
    assert_eq!(state, 10);
    assert_eq!(events, vec![2, 3, 4]);
}

#[test]
fn dyn_consumer_can_take_provider() {
    let expected = run(&Adder, 1, vec![2, 3, 4]);

    let boxed: Box<dyn StateChangeProvider<State = u64, Input = u64, Event = u64>> =
        Box::new(Adder);
    assert_eq!(run(&boxed, 1, vec![2, 3, 4]), expected);

    let by_ref: &dyn StateChangeProvider<State = u64, Input = u64, Event = u64> = &Adder;
    assert_eq!(run(&by_ref, 1, vec![2, 3, 4]), expected);
}
