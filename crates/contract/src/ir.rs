//! 実行時に読む IR（JSON）の型。`ir_format` 1 の契約である。
//!
//! pack の `compile` が書き出し、core の `load_ir` が読む。両者が共有する置き場として contract に置く
//! （`docs/architecture.md` 第 6 節の判定規則 1）。連想配列は `BTreeMap`（`serde_json::Value` の
//! `Object` も同様）で、キーの順が決定的になる。これが、同じ教材から同じバイト列を作る前提である。
//! バイト列の形（整形した JSON と末尾の改行 1 つ）は [`to_canonical_json`] が持つ。
//!
//! # 版 1 の固定（TASK-41、PACK-24）
//!
//! `ir_format` は 1 のまま保ち、語彙の追加（v1.1〜v1.3）は任意の欄の加算だけで行う。
//!
//! - 版 1 の後に足す欄は、`Option` か空を既定値とする型にし、`#[serde(default, skip_serializing_if = ...)]`
//!   を付ける。既定値のときは書き出さない。新しい項目を使わない教材パックは、バイト単位で同じ IR になる
//! - 既存の欄の名前・型・意味・必須かどうかは変えない。消す・改名する・必須にする変更は
//!   `ir_format` の版上げ（TASK-60）として扱う
//! - 版 1 の構造の欄は、空でも常に書き出す（`Ir` の最上位の表・`Start::effects`・`Instance::args`）。
//!   明示した空の値と省略は同じバイト列になる
//! - 読むときは `null` を `None` として受ける（試作の golden が `null` を含むため）。未知の欄は拒否する
//! - 固定しないもの: 語彙の中身（[`serde_json::Value`] のまま通す欄。TASK-42 以降）、モジュールの展開
//!   （TASK-50）、本文の木（TASK-60）
//!
//! 版 1 の出力は、エンジン repo の golden（`crates/contract/tests/data/ir_format_1/`・
//! `crates/pack/tests/data/golden/`）とのバイト一致で固定する。golden の書き換えは契約の変更であり、
//! spec を先に直す。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 本リポジトリの IR が受け付ける `format_version`・`pack.ir_format`。両方がこの値でなければならない。
pub const IR_FORMAT_VERSION: u32 = 1;

/// IR を出力のバイト列にする。整形した JSON に改行を 1 つ付ける。
///
/// pack の `compile`・`compare` が書き出す形の正で、golden とのバイト一致の対象である。
/// I/O・時刻・乱数を持たない純関数で、同じ `Ir` からは常に同じバイト列を返す。
pub fn to_canonical_json(ir: &Ir) -> Result<Vec<u8>, serde_json::Error> {
    let mut v = serde_json::to_vec_pretty(ir)?;
    v.push(b'\n');
    Ok(v)
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// 教材の基本情報。`workshop.*` の `pack` 節と IR の `pack` で共通。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackInfo {
    /// 教材の ID。
    pub id: String,
    /// 教材の題名。
    pub title: String,
    /// 教材の版。
    pub version: String,
    /// IR の形式の版。
    pub ir_format: u32,
    /// 案内の水準（`A`・`B`・`C`）。
    pub guidance: String,
    /// 語彙の版の指定。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vocab: Option<Value>,
    /// 水準ごとの規則。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guidance_levels: Option<Value>,
    /// 入力欄の設定。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<Value>,
    /// 見た目の設定。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<Value>,
    /// 左側の表示の設定。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left_pane: Option<Value>,
}

/// 練習データの宣言。中身は IR に載せない（載せるのは TASK-60）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureDef {
    /// パックの根からの相対パス（`fixtures/` の下）。
    pub path: String,
    /// 種類。
    pub kind: String,
}

/// Step の遷移。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    /// 遷移先（人が書く層では Step の ID、IR では `uid`）。
    pub to: String,
    /// 遷移の条件。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Value>,
}

/// Step。人が書く層と IR で同じ型を使う。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// Step の ID。
    pub id: String,
    /// 題名。
    pub title: String,
    /// 本文の参照（章・モジュールのディレクトリからの相対パス）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// 水準ごとの本文の参照。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub body_levels: BTreeMap<String, String>,
    /// 「今の Step でやること」の短い文。
    #[serde(default, rename = "do", skip_serializing_if = "Option::is_none")]
    pub do_: Option<String>,
    /// 学習目標の ID。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lo: Vec<String>,
    /// 最後の Step か。
    #[serde(default, rename = "final", skip_serializing_if = "is_false")]
    pub is_final: bool,
    /// 使うビュー。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<String>,
    /// 使う練習データ。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixtures: Vec<String>,
    /// ヒント。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hints: Vec<String>,
    /// 入るときの効果（語彙の中身は通す）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_enter: Vec<Value>,
    /// 処理中の進行（語彙の中身は通す）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub processing: Vec<Value>,
    /// 入力条件（語彙の中身は通す）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
    /// 規則（語彙の中身は通す）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<Value>,
    /// 拒否の分類ごとの文面の ID。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub on_reject: BTreeMap<String, String>,
    /// 確認の操作（語彙の中身は通す）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm: Option<Value>,
    /// 達成のときの効果（語彙の中身は通す）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_achieved: Vec<Value>,
    /// 遷移。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub next: Vec<Transition>,
    /// モジュールの呼び出し（展開は TASK-50）。
    #[serde(default, rename = "use", skip_serializing_if = "Option::is_none")]
    pub use_module: Option<Value>,
}

/// 途中開始の定義。人が書く層と IR で同じ型を使う。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Start {
    /// 開始の ID。
    pub id: String,
    /// 開始する Step（人が書く層では Step の ID、IR では `uid`）。
    pub node: String,
    /// 開始用の世界状態を作る効果。版 1 の構造の欄で、空でも書き出す。
    #[serde(default)]
    pub effects: Vec<Value>,
    /// 案内の水準。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guidance: Option<Value>,
    /// 開始条件。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Value>,
    /// 開始条件を満たさないときの文面の ID。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unmet: Option<Value>,
}

/// Step の実体。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Node {
    /// 実体の ID（`<実体 ID>/<Step ID>`）。
    pub uid: String,
    /// 属する実体の ID。
    pub scope: String,
    /// 定義した章・モジュールの ID。
    pub unit: String,
    /// `next[].to` を `uid` に直した Step。
    pub step: Step,
    /// `use` の Step で、呼び出した実体の最初の Step の `uid`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enter: Option<String>,
    /// モジュールの最後の Step が達成になったときの戻り先。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_to: Option<String>,
}

/// 章またはモジュールの実体。`main` は章の実体。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instance {
    /// 実体の ID。
    pub id: String,
    /// モジュールの ID（章の実体は `None`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    /// 既定値を重ねた引数。版 1 の構造の欄で、空でも書き出す。
    #[serde(default)]
    pub args: BTreeMap<String, String>,
    /// 呼び出した `use` の Step の `uid`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller: Option<String>,
}

/// 実行時に読む IR 全体。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ir {
    /// IR の形式の版（[`IR_FORMAT_VERSION`]）。
    pub format_version: u32,
    /// 教材の基本情報。
    pub pack: PackInfo,
    /// 参照元の版（語彙の中身は通す）。版 1 の構造の欄で、空でも書き出す。
    #[serde(default)]
    pub refs: Vec<Value>,
    /// コマンドの表（語彙の中身は通す）。版 1 の構造の欄で、空でも書き出す。
    #[serde(default)]
    pub commands: BTreeMap<String, Value>,
    /// ビューの表（語彙の中身は通す）。版 1 の構造の欄で、空でも書き出す。
    #[serde(default)]
    pub views: BTreeMap<String, Value>,
    /// 練習データの宣言。版 1 の構造の欄で、空でも書き出す。
    #[serde(default)]
    pub fixtures: BTreeMap<String, FixtureDef>,
    /// ヒントの表（語彙の中身は通す）。版 1 の構造の欄で、空でも書き出す。
    #[serde(default)]
    pub hints: BTreeMap<String, Value>,
    /// 拒否の文面の表（語彙の中身は通す）。版 1 の構造の欄で、空でも書き出す。
    #[serde(default)]
    pub rejections: BTreeMap<String, Value>,
    /// 問い返しの文面の表（語彙の中身は通す）。版 1 の構造の欄で、空でも書き出す。
    #[serde(default)]
    pub clarifications: BTreeMap<String, Value>,
    /// 拒否の分類ごとの既定の文面の ID。版 1 の構造の欄で、空でも書き出す。
    #[serde(default)]
    pub defaults: BTreeMap<String, String>,
    /// 実体の一覧。版 1 の構造の欄で、空でも書き出す。
    #[serde(default)]
    pub instances: Vec<Instance>,
    /// Step の実体の一覧。
    pub nodes: Vec<Node>,
    /// 途中開始の一覧。
    pub starts: Vec<Start>,
}

#[cfg(test)]
mod tests {
    //! 版 1 の契約適合は `tests/ir_contract.rs` を正とする。ここは型の基本の挙動だけを確かめる。
    use super::*;

    fn step(id: &str) -> Step {
        serde_json::from_str(&format!(r#"{{"id":"{id}","title":"t"}}"#)).unwrap()
    }

    #[test]
    fn ir1_defaults_are_omitted() {
        let s = serde_json::to_string(&step("a")).unwrap();
        assert_eq!(s, r#"{"id":"a","title":"t"}"#);
    }

    #[test]
    fn ir2_map_keys_are_sorted() {
        let mut s = step("a");
        s.on_reject.insert("z".into(), "1".into());
        s.on_reject.insert("b".into(), "2".into());
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.find("\"b\"").unwrap() < j.find("\"z\"").unwrap());
    }

    #[test]
    fn ir3_unknown_field_is_rejected() {
        let r: Result<Step, _> = serde_json::from_str(r#"{"id":"a","title":"t","script":"x"}"#);
        assert!(r.is_err());
    }

    #[test]
    fn ir4_legacy_null_is_accepted() {
        let j = r#"{"id":"a","title":"t","use":null,"input":null,"confirm":null,
            "do":null,"body":null,"next":[],"final":false}"#;
        let s: Step = serde_json::from_str(j).unwrap();
        assert!(s.use_module.is_none() && s.input.is_none() && !s.is_final);
        let n: Node = serde_json::from_str(
            r#"{"uid":"main/a","scope":"main","unit":"u","step":{"id":"a","title":"t"},
            "enter":null,"exit_to":null}"#,
        )
        .unwrap();
        assert!(n.enter.is_none() && n.exit_to.is_none());
    }

    #[test]
    fn ir5_rename_roundtrip() {
        let j = r#"{"id":"a","title":"t","do":"x","final":true,"use":{"module":"m"}}"#;
        let s: Step = serde_json::from_str(j).unwrap();
        assert_eq!(s.do_.as_deref(), Some("x"));
        assert!(s.is_final);
        let back: Step = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(s, back);
    }
}
