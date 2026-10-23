//! 現在の Step と案内レベルで出せるヒントの一覧（TASK-293、GUIDE-2）。
//!
//! 教材がヒントに `levels` を付けると、案内レベルを変えたときにその水準のヒントだけが出る。出し分けの規則の正は
//! 契約の [`visible_hints`] の 1 か所で、本モジュールは実行の状態（現在の Step と `RunState.guidance_level`）から
//! その入力を引いて呼ぶだけにする。規則を二重に持たないので、pack の検査（`E-HINT-HIDDEN`）が報告する
//! 「ある水準で 1 つも出ない Step」と、実行時に一覧が空になる Step は、同じ規則の別の入力から導かれる。
//!
//! 返すのは `step.hints` の並びのままのヒントの ID だけで、本文は返さない。`current_hints(..).len()` が
//! 「出せる数」で、ヒントを開く判定（`Control::OpenHint`、TASK-295）が段数の上限に使い、画面（TASK-530）が一覧に使う。
//! 純関数で、読めない定義・IR に無い Step・開始前は空を返す（fail-closed）。I/O・時刻・乱数を持たない。
//! `CONTRACT_VERSION` の署名の対象に含めない（[`crate::text_fill`] と同じ扱い）。

use fandhe_workshop_contract::guidance::{visible_hints, GuidanceLevel};
use fandhe_workshop_contract::ir::Ir;

use crate::addr::StepUid;
use crate::session::RunState;

/// 実体 `uid` の Step で、水準 `level` に出せるヒントの ID を `step.hints` の並びのまま返す。
/// `uid` が IR に無ければ空。
pub fn hints_at<'a>(ir: &'a Ir, uid: &StepUid, level: GuidanceLevel) -> Vec<&'a str> {
    match ir.nodes.iter().find(|n| n.uid == uid.0) {
        Some(node) => visible_hints(ir, &node.step, level),
        None => Vec::new(),
    }
}

/// 現在の Step（`run.current`）と現在の案内レベル（`run.guidance_level`）で出せるヒントの ID の一覧。
/// 開始前（`current` が `None`）や、IR に無い Step のときは空。
pub fn current_hints<'a>(ir: &'a Ir, run: &RunState) -> Vec<&'a str> {
    match &run.current {
        Some(uid) => hints_at(ir, uid, run.guidance_level),
        None => Vec::new(),
    }
}
