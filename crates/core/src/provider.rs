//! 状態変化の提供元の境界の型（TASK-187、版 1）。
//!
//! 世界状態の変化は `apply(状態, 入力) → (新しい状態, イベント列)` が作る（EXT-4、D-53）。
//! 将来、模擬の提供元を実際の CLI の状態に置き換えられるよう、この境界を [`StateChangeProvider`] として
//! 明示する。教材と画面は、提供元が模擬か実かを知らない。呼び出し元は TASK-204 の `Session` などで、
//! 具体型（State・Input・Event）は TASK-204 が関連型へ束縛する。
//!
//! 設計の判断:
//! - 関連型で抽象化する。TASK-204 の具体型を先取りせず、1 つの提供元に型の組が 1 つに決まるため
//!   ジェネリクス引数にしない
//! - 受け手は `&self`。世界状態をすべて `State` に置くことを型で強制し、停止・復帰と
//!   同じ入力列から同じ結果になること（EVT-22）を守る。提供元は教材（IR）など不変のものだけを持つ
//! - 状態は値で受けて値で返す。実装の中でその場で書き換えてもよい
//! - 関連型に trait bound を付けない（要否は TASK-204 が決める。後から足すと版上げになる）。
//!   I/O・時刻・乱数・async も要求しない（core は wasm32 で動く）
//!
//! 【推論】失敗は `Result` で返さず、拒否・無視はイベントとして表す（全域の関数）。実 CLI の失敗の
//! 表し方は橋渡しの範囲で、要るなら版を上げる。
//!
//! 【推論】差し替えを generic と `dyn` のどちらで行うかは TASK-204 の設計で決まるため、object safe に保ち、
//! `&P`・`Box<P>` への包括 impl を置いてどちらも妨げない。
//!
//! 意図して固定しないもの:
//! - State・Input・Event の具体型、`Session`・世代・宛先（TASK-204）
//! - 模擬の提供元の実装、差し替えの自動テスト（TASK-187 2/2）
//! - 実 CLI の橋渡し（作らない。D-53）
//!
//! 形（関連型・メソッドの署名・bound）を変える場合は、[`PROVIDER_CONTRACT_VERSION`] を上げる。

/// この契約の版（TASK-187、版 1）。トレイトの形を変えたら上げる。
pub const PROVIDER_CONTRACT_VERSION: u32 = 1;

/// 状態変化の提供元。`apply(状態, 入力) → (新しい状態, イベント列)` の境界（EXT-4、D-53）。
pub trait StateChangeProvider {
    /// 世界状態（と進行の状態）。具体型は TASK-204 が決める。
    type State;
    /// 入力。具体型は TASK-204 が決める。
    type Input;
    /// イベント。具体型は TASK-204 が決める。
    type Event;

    /// 状態に入力を適用し、新しい状態とイベント列を返す。
    fn apply(&self, state: Self::State, input: Self::Input) -> (Self::State, Vec<Self::Event>);
}

impl<P: StateChangeProvider + ?Sized> StateChangeProvider for &P {
    type State = P::State;
    type Input = P::Input;
    type Event = P::Event;

    fn apply(&self, state: Self::State, input: Self::Input) -> (Self::State, Vec<Self::Event>) {
        (**self).apply(state, input)
    }
}

impl<P: StateChangeProvider + ?Sized> StateChangeProvider for Box<P> {
    type State = P::State;
    type Input = P::Input;
    type Event = P::Event;

    fn apply(&self, state: Self::State, input: Self::Input) -> (Self::State, Vec<Self::Event>) {
        (**self).apply(state, input)
    }
}
