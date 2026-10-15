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

## YAML の値の書き方（形式 B）

文字列の欄は、引用符なしで `true`・`no`・`1.10`・`010`・`null` と書いても、書いたとおりの文字列として読みます
（PACK-4）。`1.10` が `1.1` に、`010` が `10` や `8` に変わることはありません。

`do`・`body` のような省略できる欄に引用符なしの `null`・`~` を書くと、未設定ではなく、その字面の文字列になります。
未設定にしたいときは、欄そのものを書かないでください。

同じ表に同じキーを 2 回書くと `E-PARSE` で拒否します。引用符の有無だけが違うキー（`true:` と `'true':`）も同じキーです。
構文の誤りには、行と桁が付きます（複数文書の YAML は位置が付きません）。

`commands`・`views` など、語彙の型付けが入るまで自由な値として通す欄の中の引用符なしの値は、文字列にならず
YAML の型のままです。重複キーの拒否だけが及びます。

## 入力方式（`input`）

Step の `input` は、`kind` で 5 方式から選びます（PACK-10）。ここに書けるのは次の欄だけで、未知の欄・未知の `kind` は
`E-PARSE`、意味の制約に反する宣言は `E-STRUCTURE` になります。`wait` は今は欄を検査せずに通し、`wait` と規則の
`on.cancel` は別の節で足します。

| `kind` | 欄 | 意味の制約 |
|---|---|---|
| `command` | `name`、`args`（各要素は `name`・`positional`・`required`・`shape`） | 名前は空でなく、引数名は重複しない。`shape` は `kebab_case`・`digits`・`any`（既定は `any`） |
| `text` | `min`・`max`（0 以上の整数。どちらも必須） | `min` は `max` 以下 |
| `choice` | `options`（各要素は `id`・`label`） | 1 件以上で、`id` は重複しない |
| `ui_action` | `targets`（各要素は `view`・`path`） | 1 件以上で、どれも空でない |
| `approval` | なし | 欄を足すと `E-PARSE` |

`text` が判定するのは、空でないことと文字数だけです。内容の良し悪しを判定する欄は書けません（`contains` や
`pattern` は `E-PARSE`）。受け付けた入力の分類は `free_text_unjudged`、範囲外は `bad_arg_shape` で拒否します。

- 文字数はコードポイントで数えます（絵文字は 1 文字）
- 前後の空白は数えません。空白だけの入力は空として常に拒否します（`min: 0` でも同じです）。この扱いは、spec が
  「空でないこと」としか書いていないことからの推定による決定です（2026-10-15）
- 下限・上限はどちらも含みます

## 根拠となる試験

| 試験 | 確かめること |
|---|---|
| `crates/contract/tests/content_hash_contract.rs` | 本文の木を除いて求めること、本文だけの修正でハッシュが変わらないこと |
| `crates/core/tests/version_record.rs` の `body_only_edit_yields_the_same_version_record` | 本文だけを直した IR から作る版の記録が、全欄で同じになること |
| `crates/contract/tests/fixed_histories.rs` | 固定の履歴 4 本の版の記録が全軸で同一で、期待値を D-51 に合わせて変えていないこと |
| `crates/pack/tests/format_b.rs` の `pack4_` | YAML の値の文字列としての読み取り・重複キーの拒否・構文エラーの行と桁（PACK-4） |
| `crates/pack/tests/input_vocabulary.rs` の `pack10_` | 5 方式の宣言が検証を通ること、`text` に内容を判定する欄が書けないこと、不正な宣言の診断と値を出さないこと（PACK-10） |
| `crates/contract/tests/input_spec_contract.rs` | `text` の空・下限・上限・コードポイントの数え方と、各方式の意味の制約（PACK-10） |
