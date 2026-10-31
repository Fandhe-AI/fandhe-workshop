//! 閉じた周回の Step への移動の判定（TASK-249、MOD-15・D-24）。
//!
//! 閉じた周回のルートは開き直さない（D-24）。`Control::GoTo` が閉じた周回の Step へ入ると、入ったときの状態変化が
//! 前の周回の成果物を書き換え、閉じたルートを開き直したのと同じ結果になる（MOD-8 とも合わない）。
//! そこで [`crate::go_to`] は、本モジュールの `is_closed` が真のとき状態に触れずに `Refused`（`ClosedRound`）を返す。
//! チェックポイントへの復帰（MOD-9・EVT-12）は別の経路で、本モジュールの対象外である。
//!
//! 周回はエンジンの概念として持たない。周回の実体は学習モジュールの実体（`Instance`。`caller` を持つ）で、章（`Node.unit`）ごとに
//! `main` の呼び出しの Step が呼ぶ。閉じたかどうかは新しい欄を持たず、IR・状態・ログから導く
//! （欄を足すと履歴の写しの形と VER-2・VER-3 の照合に響くため）。
//!
//! # 用語
//!
//! - 周回の錨: 節点から `Instance::caller` を `scope` が `main` になるまでたどって着く `main` の節点。
//!   たどる回数は実体の数 + 1 までで、巡回・不整合な IR でも止まる。決まらなければ `None`
//! - 周回の順位: 錨の `unit` が、`scope` が `main` の節点として IR に最初に現れる位置。同じ章の呼び出しの Step は同じ順位になる
//!   （`begin` の省略の規則が「IR の順」を使う前例に合わせる）
//! - 開始した周回: ログの各 `Begun.start` を `ir.starts` で引いた節点の周回の順位
//!
//! # 規則
//!
//! 移動先 `to` は、次の 3 つをすべて満たすときに限り閉じた周回の Step とする。
//!
//! - (a) `to` の持ち主（[`owner_of`]）が `main` ではない（学習モジュールの実体の Step か、それを呼ぶ呼び出しの Step）。
//!   `main` の章の Step は周回の実体ではないので対象にしない
//! - (b) `to` の周回の順位が、現在の Step の周回の順位と異なる（現在の周回の中の移動は閉じていない。現在の Step が `main` 所有の通常の Step のときは
//!   同じ章の実体も含め、どの周回の Step への移動も (b) を満たす。`main` の Step を経由しても閉じた周回へは戻れない）
//! - (c) ログに、`to` の周回より順位が大きい周回を開始した `Begun` が 1 件以上ある
//!
//! どれかが決まらないとき（錨が `None`・現在の Step が無い）は閉じていないとみなし、従来の移動に任せる。
//!
//! # 場面
//!
//! - 後の周回を開始した後に前の周回の Step へ移る（`main` の Step を経由した後の移動を含む）: 閉じている
//! - 同じ周回の中の移動・`Begin` を経ない移動・後の周回の開始より先の周回への移動: 閉じていない
//! - 前の周回のチェックポイントへ復帰した後の、その周回の中の移動: 閉じていない（現在の Step が前の周回にある）
//!
//! # 採らなかった案
//!
//! 現在の位置に関係なく、後の周回の `Begun` があれば今の周回の中の移動まで閉じる案は、復帰で戻った周回の中の移動（MOD-9）を
//! 拒否してしまうため採らない。順位の大小だけで解く案は、`main` の Step への移動で現在位置の順位が下がると解除されるため採らない（#1941）。`RunState`・`SessionState` に欄を足す案は履歴の形が変わるため採らない。
//!
//! # 限界
//!
//! - 上限で止まった `Begin` の後に、`GoTo` だけで後の周回へ入って前の周回へ戻ると、残った `Begun` により拒否される
//! - 周回の順位は章が IR に現れる順に依る（compile は章を宣言の順に並べる）
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!`・添字の直接参照を使わない）で決定的。I/O・時刻・乱数を持たない。
//! 計算量はログを 1 回なめる程度（ログの長さ × 開始の数）。

use fandhe_workshop_contract::ir::{Ir, Node};

use crate::addr::StepUid;
use crate::event::Event;
use crate::instance::owner_of;
use crate::session::SessionState;

/// 章の実体の ID。
const MAIN: &str = "main";

/// `uid` の周回の錨（`scope` が `main` の節点）。決まらなければ `None`。
fn round_anchor<'a>(ir: &'a Ir, uid: &str) -> Option<&'a Node> {
    let mut cur = uid.to_string();
    for _ in 0..=ir.instances.len() {
        let node = ir.nodes.iter().find(|n| n.uid == cur)?;
        if node.scope == MAIN {
            return Some(node);
        }
        let instance = ir.instances.iter().find(|i| i.id == node.scope)?;
        cur = instance.caller.clone()?;
    }
    None
}

/// `uid` の周回の順位。
fn round_index(ir: &Ir, uid: &str) -> Option<usize> {
    let anchor = round_anchor(ir, uid)?;
    ir.nodes
        .iter()
        .position(|n| n.scope == MAIN && n.unit == anchor.unit)
}

/// 移動先が閉じた周回の Step か。
pub(crate) fn is_closed(ir: &Ir, state: &SessionState, to: &StepUid) -> bool {
    if owner_of(ir, to).0 == MAIN {
        return false;
    }
    let Some(current) = state.run.current.as_ref() else {
        return false;
    };
    let (Some(to_round), Some(cur_round)) = (round_index(ir, &to.0), round_index(ir, &current.0))
    else {
        return false;
    };
    // 現在の Step が main 所有の通常 Step のときは、同じ章の順位でも周回の中にいるとはみなさない
    if to_round == cur_round && owner_of(ir, current).0 != MAIN {
        return false;
    }
    state.log.iter().any(|e| match e {
        Event::Begun { start, .. } => ir
            .starts
            .iter()
            .find(|s| &s.id == start)
            .and_then(|s| round_index(ir, &s.node))
            .is_some_and(|began| began > to_round),
        _ => false,
    })
}
