//! 案内レベルとヒントの値の型（TASK-290、版 1）。
//!
//! 案内レベル（A・B・C）は実行全体で 1 つであり（GUIDE-3）、ヒントは「現在の案内レベルで出せる数まで」
//! 1 段ずつ開く（GUIDE-17）。表示・入力・最初の教材・core の本文の選択が同じ型と規則に従うよう、
//! 値の型と、出せるヒントを返す口（[`visible_hints`]）をこの 1 か所に置く。
//!
//! 呼び出し元は次のとおり。
//! - core: `Control::OpenHint`・`Event::HintOpened` の判定、`RunState.guidance_level`、本文の水準の選択（TASK-292）
//! - pack: 水準とヒントの静的な検査（TASK-49 の `E-PARSE`・`E-HINT-HIDDEN`）
//! - web: 案内レベルの切り替えとヒントの表示
//!
//! 決定 D-55 により、次は本契約に含めない。
//! - 実体ごとの案内レベル（`use.guidance`。語彙に足さない）
//! - ヒントの上限 3 段の定数（3 段は教材側の約束で、語彙・エンジンでは固定しない）
//! - `do_levels`（語彙の担当 TASK-70・TASK-49）
//!
//! 値・規則・型を変える場合は、先に spec を直し、[`GUIDANCE_CONTRACT_VERSION`] を上げる。
//!
//! IR の `Ir::hints` は語彙の中身を通す `Value` のままで、定義の型を厳密にしない。ここでは `levels` の鍵だけを
//! 寛容に読む。IR は未信頼入力のため、読み取れない定義は出さない側（fail-closed）に倒す。

use core::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ir::{Ir, Step};

/// この契約の版。値・規則・型を変えたら上げる。
pub const GUIDANCE_CONTRACT_VERSION: u32 = 1;

/// 案内レベル。A が最も手厚く、C が最も薄い。
///
/// 直列化は `"A"`・`"B"`・`"C"` で、IR が持つ `"guidance": "A"` と同じ綴りである。小文字や未知の値は
/// 逆直列化で拒否する。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub enum GuidanceLevel {
    /// 最も手厚い案内。既定値（実際の開始値は開始時に `pack.guidance`・`starts[].guidance` から決める。TASK-294）。
    #[default]
    A,
    /// 中間の案内。
    B,
    /// 最も薄い案内。
    C,
}

impl GuidanceLevel {
    /// 全水準を A・B・C の順に並べたもの。
    pub const ALL: [GuidanceLevel; 3] = [GuidanceLevel::A, GuidanceLevel::B, GuidanceLevel::C];

    /// IR の文字列から読む。`"A"`・`"B"`・`"C"` 以外は `None`（全域で panic しない）。
    pub fn parse(s: &str) -> Option<GuidanceLevel> {
        match s {
            "A" => Some(GuidanceLevel::A),
            "B" => Some(GuidanceLevel::B),
            "C" => Some(GuidanceLevel::C),
            _ => None,
        }
    }

    /// IR・直列化と同じ綴り。
    pub fn as_str(self) -> &'static str {
        match self {
            GuidanceLevel::A => "A",
            GuidanceLevel::B => "B",
            GuidanceLevel::C => "C",
        }
    }

    /// 本文が無いときに濃い側へ倒れる順（GUIDE-1）。C は B、A の順に、B は A に倒れる。
    /// 自分自身を先頭に含む。本文の選択の実装は TASK-292 が持つ。
    pub fn fallback_chain(self) -> &'static [GuidanceLevel] {
        match self {
            GuidanceLevel::A => &[GuidanceLevel::A],
            GuidanceLevel::B => &[GuidanceLevel::B, GuidanceLevel::A],
            GuidanceLevel::C => &[GuidanceLevel::C, GuidanceLevel::B, GuidanceLevel::A],
        }
    }
}

impl fmt::Display for GuidanceLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 現在の案内レベルで出せるヒントの ID を、`step.hints` の並びのまま返す（GUIDE-17）。
///
/// 「出せる数」は戻り値の `len()` である。判定の規則は次のとおりで、読めない定義は数えない（fail-closed）。
/// - `ir.hints` に定義が無い ID は数えない
/// - 定義がオブジェクトでなければ数えない
/// - `levels` が無い、または空の配列なら全水準で出す（語彙の「省略は全水準」）
/// - `levels` が文字列の配列で、`level` の綴りを含めば出す。配列でない、文字列でない要素や未知の水準を含む
///   場合は数えない（B・C で伏せたい語を誤って見せないため。静的な報告は TASK-49）
///
/// 純粋関数で、どんな `Value` でも panic しない。
pub fn visible_hints<'a>(ir: &'a Ir, step: &'a Step, level: GuidanceLevel) -> Vec<&'a str> {
    step.hints
        .iter()
        .filter(|id| {
            ir.hints
                .get(id.as_str())
                .is_some_and(|def| is_visible(def, level))
        })
        .map(String::as_str)
        .collect()
}

fn is_visible(def: &Value, level: GuidanceLevel) -> bool {
    let Some(obj) = def.as_object() else {
        return false;
    };
    let Some(levels) = obj.get("levels") else {
        return true;
    };
    let Some(arr) = levels.as_array() else {
        return false;
    };
    if arr.is_empty() {
        return true;
    }
    let mut found = false;
    for v in arr {
        let Some(s) = v.as_str().and_then(GuidanceLevel::parse) else {
            return false;
        };
        found |= s == level;
    }
    found
}
