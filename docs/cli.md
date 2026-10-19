# CLI の外部仕様

```text
CLI の外部仕様の版: 1
決定日: 2026-10-19
状態: 固定（契約）
```

## 1. 位置づけと版

- 本文書は、作成者向け CLI `fandhe-pack` の外部仕様（バイナリ名・コマンド一覧・引数の形・使い方・終了コード）を 1 か所に固定する（TASK-399 1/2、AUTHOR-1・AUTHOR-2）
- 後続のタスク（TASK-401〜TASK-408・TASK-410）は、入力に「`docs/cli.md` CLI の外部仕様の版 1」の形で書ける
- コード側の正は `crates/pack/src/cli_contract.rs` で、本文書との一致は `crates/pack/tests/cli_contract_doc.rs` が機械で照合する
- 仕様は private のため、本文を転記せず ID（AUTHOR-1・AUTHOR-2・PACK-2）で参照する
- 版の規則: 固定した行は書き換えない。足すときは版を上げ、変更履歴に記録する。`contract_version`・`ir_format`・診断コードの一覧の版とは別に管理する

## 2. バイナリ名

`fandhe-pack`

## 3. コマンド一覧と引数の形

| コマンド | 引数の形 | 内容 |
|---|---|---|
| `validate` | `<pack-dir>` | 静的検査（TASK-403） |
| `test` | `<pack-dir>` | 検査の後に、模範解答の完走と誤入力の分類（TASK-404） |
| `compile` | `<pack-dir> [-o <file>]` | IR（JSON）を標準出力、または `-o` のファイルへ出す（`-o` は TASK-407） |
| `vocab` | `<pack-dir>` | 拡張点を使った Step の数え上げ（TASK-408） |
| `metrics` | `<pack-dir>` | ファイルごとの行数・実効行数・バイト数・入れ子の深さ（TASK-402） |
| `compare` | `<pack-dir-a> <pack-dir-b>` | 2 つの教材パックの IR の完全一致の比較 |

- `<pack-dir>` は `workshop.yaml` があるディレクトリである（AUTHOR-1。`workshop.toml` の扱いは PACK-2 に従う）
- `-o <file>` は `<pack-dir>` の後ろにだけ置ける。それ以外の位置・未知のオプションは使い方の誤りである
- `defects` は作成者向けの一覧・使い方に出さない。未知のコマンドとして扱う（試験用のもので、製品には置かない）

## 4. 使い方

標準エラー出力に出し、標準出力には何も出さない。英語で、末尾は改行 1 つで終わる。

```text
usage: fandhe-pack <command> [<args>]

commands:
  validate <pack-dir>
  test <pack-dir>
  compile <pack-dir> [-o <file>]
  vocab <pack-dir>
  metrics <pack-dir>
  compare <pack-dir-a> <pack-dir-b>
```

## 5. 終了コード

| コード | 意味 |
|---|---|
| 0 | 診断 0 件で完了した。`compare` は IR が一致した |
| 1 | 診断が 1 件以上ある。`compare` は IR が一致しなかった場合も 1 とする（作成者が直すべき結果で、使い方の誤りではないため） |
| 2 | 使い方の誤り（引数なし・未知のコマンド・引数の数や形の不一致）。結果を書き出せない場合も 2 とする（成功の 0 と取り違えないため） |

- 0・1・2 以外の値は使わない
- 経過措置（契約外）: 未実装のコマンドは「not implemented」を標準エラー出力に出して 2 で終わる。各実装タスクで消える

## 6. 本版で扱わない事項

診断 1 行・項目の書式・最終行（`OK:`・`NG:`）・診断の出し先は、TASK-399 2/2（#471）が本文書に節を足して固定する。別名の文書は作らない。

## 7. 適合試験

`crates/pack/tests/cli_contract_doc.rs`（文書とコードの一致）と `crates/pack/tests/cli_contract.rs`（実行ファイルの挙動）が次を確かめる。

| 試験 | 内容 |
|---|---|
| `cli_doc_version_matches_constant` | 版の欄が `CLI_CONTRACT_VERSION` と一致し、状態が固定（契約）である |
| `cli_doc_command_table_matches_contract` | 3 節の表が `Subcommand::ALL` の順・名前・引数の形と一致する |
| `cli_doc_usage_block_matches_usage` | 4 節のブロックが `usage()` と完全一致する |
| `cli_doc_exit_code_table_matches_constants` | 5 節の表のコードが 0・1・2 と一致する |
| `usage_declares_compile_output_option` | 使い方に `compile <pack-dir> [-o <file>]` の行がある（`-o` の動作は確かめない） |
| `exit_codes_are_fixed_values` | 終了コードの定数が 0・1・2 である |
| `commands_are_the_six_in_contract_order` | コマンドが 6 つで、AUTHOR-1 の順である |
| `defects_is_not_an_author_command` | `defects` が未知のコマンドで、使い方に出ない |
| `author1_no_args_prints_usage_and_exits_2` | 引数なしで 2・標準エラー出力に使い方・標準出力は空 |
| `unknown_command_exits_2_with_usage` | 未知のコマンドで 2 |
| `argument_shape_errors_exit_2` | 引数の数の誤りで 2・標準出力は空 |
| `exit_0_and_1_follow_diagnostics` | 診断なしで 0、診断ありで 1。`compare` は一致で 0、不一致で 1 |

## 8. 変更履歴

| 版 | 日付 | 内容 |
|---|---|---|
| 1 | 2026-10-19 | 初版（バイナリ名・コマンド 6 つ・引数の形・使い方・終了コード） |
