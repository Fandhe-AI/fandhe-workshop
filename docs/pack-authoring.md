# 教材の作成者向けの文書

教材（パック）を作る人向けの文書です。仕様は private のため本文を転記せず、ID（VER-11・VER-12・D-51・D-52）で参照します。

現時点で書いているのは、版と保存済みの履歴の節だけです（TASK-180 で追加）。主版（`pack.version` の主版）を上げる条件と
`pack.vocab` の書き方の規約は、TASK-179（#453）がこの文書に書き足します。それまでは未整備です。

## 教材の版と保存済みの履歴

保存済みの履歴は、4 軸の版（エンジン・教材形式・教材内容・参照元）を記録します。教材内容の軸は、`pack.version` と
`content_hash` の組です。

`content_hash` は、IR から本文の木（`body_doc`・`body_levels_doc`）だけを除いて求めます（D-51）。求め方の実装は
`crates/contract/src/content_hash.rs` にあります。

## 本文だけの修正は `content_hash` を変えない

本文だけを直した教材は、`content_hash` が変わりません。保存済みの履歴から見ると同じ版のままで、受講中の画面の本文だけが
新しくなります。

本文以外の欄（例: 選択肢の `label`）を足すと IR が変わり、`content_hash` も変わります。

## 主版を上げ忘れた本文の修正は判定に現れない

主版を上げ忘れて本文だけを直すと、その修正は履歴の版の判定に現れません（VER-12）。エンジンは、本文の修正を履歴の版の違いとして
検出できません。

本文の版は `pack.version` で示す運用で補います（VER-11）。上げ忘れを警告する検査はありません（D-52）。主版を上げる判断の基準は、
TASK-179 でこの文書に書き足します。

## 根拠となる試験

| 試験 | 確かめること |
|---|---|
| `crates/contract/tests/content_hash_contract.rs` | 本文の木を除いて求めること、本文だけの修正でハッシュが変わらないこと |
| `crates/core/tests/version_record.rs` の `body_only_edit_yields_the_same_version_record` | 本文だけを直した IR から作る版の記録が、全欄で同じになること |
| `crates/contract/tests/fixed_histories.rs` | 固定の履歴 4 本の版の記録が全軸で同一で、期待値を D-51 に合わせて変えていないこと |
