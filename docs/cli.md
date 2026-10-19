# CLI の外部仕様

```text
CLI の外部仕様の版: 3
決定日: 2026-10-19
状態: 固定（契約）
```

## 1. 位置づけと版

- 本文書は、作成者向け CLI `fandhe-pack` の外部仕様（バイナリ名・コマンド一覧・引数の形・使い方・終了コード・診断 1 行・項目の書式・最終行・診断の出し先）を 1 か所に固定する（TASK-399 1/2・2/2、AUTHOR-1〜AUTHOR-3）
- 後続のタスク（TASK-401〜TASK-408・TASK-410）は、入力に「`docs/cli.md` CLI の外部仕様の版 3」の形で書ける
- コード側の正は `crates/pack/src/cli_contract.rs` で、本文書との一致は `crates/pack/tests/cli_contract_doc.rs` が機械で照合する
- 仕様は private のため、本文を転記せず ID（AUTHOR-1〜AUTHOR-3・PACK-2）で参照する
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

## 6. 診断 1 行の形

診断 1 件は 1 行で、次の 4 欄を `: `（コロンと半角空白）で区切って書く（AUTHOR-2）。行・桁は持たない。

```text
<ファイル>: <項目>: <コード>: <メッセージ>
```

| 欄 | 内容 |
|---|---|
| ファイル | 診断の対象のファイル。パック根からの相対パスで、絶対パスは出さない。ファイルに結びつかない診断（ディレクトリ全体の読み込みの失敗など）は、パック根を表す `.` などを書く |
| 項目 | 診断の対象の項目（7 節）。空にしない |
| コード | `E-` で始まる診断コード（`docs/diagnostics.md`） |
| メッセージ | 人が読む説明。英語で書く。教材の本文・秘密の値は写さない |

- ファイルと項目は必ず持つ。空（空白だけを含む）の診断は作れない（`Diagnostic::new` が型で拒否する）
- 先頭から 3 つの `: ` で区切ると、ちょうど 4 欄になる。ファイルと項目の欄に `: ` が含まれるときは、空白を `\u{0020}` に直して書く（例: `a: b.yaml` は `a:\u{0020}b.yaml`）。ID・パスは教材データ由来で、直さないと偽の欄を作れてしまうため（A03）。メッセージは最後の欄なので `: ` を直さない
- 制御文字（改行を含む）は、どの欄でも `\u{XXXX}`（16 進 4 桁・大文字）に直す。1 診断が必ず 1 行になる
- メッセージは英語、最終行（8 節）は日本語で書く。使い方・`error:` の行を英語とする既存の規約（`coding-rust.md`）に診断のメッセージは従い、最終行は AUTHOR-2・AUTHOR-3 の文言をそのまま固定するため、2 つが混ざる。本版は仕様の文言を優先する

## 7. 項目の書式

項目は `<種別>`、または `<種別>:<名前>` の形で書く。種別は `[a-z][a-z_]*` で、名前は空でない。名前を安全に写せない（空・`safe_ident` を通らない）ときは、名前の代わりに `#<n>`（1 始まりの位置）を使う（例: `command:#2`）。

AUTHOR-2 が名指しする 5 種（`step`・`rejection`・`start`・`playthrough`・`error_input`）は、必ずこの名前の種別で出す。次の表は、版 2 以降で実装が使う種別の参考の一覧である。種別を足しても版は上げない（形は上の規則が決める）。

| 種別 | 項目の形 | 備考 |
|---|---|---|
| `step` | `step:<ID>` | Step |
| `rejection` | `rejection:<ID>` | 拒否理由 |
| `start` | `start:<ID>` | 開始の定義 |
| `playthrough` | `playthrough:<ID>` | 通し試験。実装は TASK-404 で使う |
| `error_input` | `error_input:<ID>`・`error_input:#<n>` | 教材テストの誤入力 |
| `fixture` | `fixture:<ID>` | フィクスチャ |
| `view` | `view:<ID>` | ビュー |
| `command` | `command:<名前>`・`command:#<n>` | コマンド |
| `defaults` | `defaults:<分類>` | 既定の文面 |
| `pack` | `pack`・`pack:<欄>` | `pack:ir_format`・`pack:profile`・`pack:vocab` など |
| `file` | `file` | ファイル全体 |
| `module` | `module` | モジュール |

## 8. 最終行

`validate` は、診断を出した後に最終行を 1 行出す（AUTHOR-2・AUTHOR-3）。`OK:` になるのは、パックを読み込めて診断が 0 件のときだけである。読み込めなかったときは、件数によらず `NG:` にする（失敗を成功とみなさない）。

`compare` の結果行は、同じ `OK:`・`NG:` の形で固定する。どちらかのパックに診断があるときは、a の診断、b の診断の順に出し、最後に診断の件数の行を出す。一致・不一致の行は出さない。a と b のどちらの診断かは、件数と順で区別する。

```text
OK: 診断 0 件（形式 B(YAML)、章 1、モジュール 0、Step の定義 2）
NG: 診断 3 件（形式 B(YAML)、章 1、モジュール 0、Step の定義 2）
NG: 診断 8 件（読み込めません）
OK: IR が一致しました（120 バイト）
NG: IR が一致しません（3 行目、58 バイト目）
NG: 診断 4 件（比べられません。a 0 件、b 4 件）
```

- 「バイト目」は 1 始まりの序数である（先頭のバイトの不一致は 1 バイト目）。内部の 0 始まりのバイト位置に 1 を足して書く
- 形式のラベルは、形式 A が `A(TOML)`、形式 B が `B(YAML)` である。章・モジュールは数、Step の定義はモジュールを含む全スコープの Step の定義の数である。数とラベルはコードが作り、教材データの文字列は写さない
- `validate` の最終行の結線と、読み込み失敗での打ち切りは TASK-403 が行う。形の組み立ては `cli_contract::final_line` が持つ
- `compile` は最終行を出さない。成功のときは IR だけを、診断があるときは診断の行だけを標準出力に出す（IR は出さない）。終了コードで区別する
- 終了コードは 5 節のとおり（`OK:` は 0、`NG:` は 1）

## 9. 診断の出し先

| 出力 | 出し先 |
|---|---|
| 診断の行・最終行・`compile` の IR・`compare` の結果行 | 標準出力 |
| 使い方・使い方の誤りの `error:` の行 | 標準エラー出力 |

- 診断のある実行では、標準エラー出力は空である。使い方の誤り（終了コード 2）のときは、標準出力は空である
- 経過措置（契約外）の `not implemented` の行は標準エラー出力に出す（5 節）

## 10. `metrics` の出力

`metrics <pack-dir>` は、パック根の下の拡張子が `toml` か `yaml` の通常ファイルについて、4 つの値を CSV で標準出力に出す（AUTHOR-1）。`.md` などは数えない。

```text
file,lines,effective_lines,bytes,max_depth
chapters/c1/chapter.yaml,12,10,230,4
workshop.yaml,6,5,120,3
TOTAL,18,15,350,4
```

- 1 行目は `METRICS_HEADER` で、続けてファイルごとに 1 行、最後に `TOTAL` の 1 行を出す。各行はちょうど 5 欄で、行の組み立ては `cli_contract::metrics_row` が持つ
- 並びは、各ディレクトリのエントリを名前の順に並べ、ディレクトリはその位置で深さ優先に潜る順である。同じ入力は同じバイト列になる
- `file` 欄はパック根からの相対パスで、区切りは `/`。制御文字は `\u{XXXX}`（16 進 4 桁・大文字）、`,` は `\u{002C}`、`"` は `\u{0022}` に直し（CSV の引用欄と解釈されないようにし）、1 ファイル 1 行・5 欄を保つ。絶対パスは出さない
- `lines` は行数（末尾に改行の無い最終行も 1 行）、`effective_lines` は空行と `#` で始まる行を除いた数、`bytes` は読んだバイト数である
- `max_depth` は近似である（試作 PoC-3 の数え方）。YAML は実効行ごとの「先頭の空白のバイト数 / 2 + 1 + 行が `- ` で始まれば 1 + 行内の `{`・`[` の最大の入れ子」の最大。TOML は `[` で始まり `=` を含まない行を表の見出しとし、その名前を `.` で分けた数を見出しの深さ（深さの候補にもする）とし、それ以外の行は「見出しの深さ + 1 + `=` の右側の行内の入れ子」の最大。文字列（`"` の内側）の括弧は数えない
- `TOTAL` は行数・実効行数・バイト数が和、`max_depth` が最大である
- 診断（`workshop.*` が無い・両方ある、上限・リンク・深さの超過、読めない・UTF-8 でない・大きすぎるファイル）があるときは、表を出さず診断の行だけを標準出力に出して終了コード 1 にする。最終行は出さない。標準エラー出力は空である

## 11. 本版で扱わない事項

- `validate` の最終行の結線・読み込み失敗での打ち切り（TASK-403）
- `test` の PASS・FAIL の行・集計・最終行（TASK-404・TASK-405）
- `compile -o` と末尾の扱い（TASK-407）
- 引数の数の検査と振り分け（TASK-401）
- 診断コードの一覧と実装の一致の検査（TASK-400）
- エスケープ後の文字列を元に戻す規則（`\u{000A}` という文字列そのものを写した場合と区別できない。本版は 4 欄の分割の保証までを契約とする）

## 12. 適合試験

`crates/pack/tests/cli_contract_doc.rs`（文書とコードの一致）と `crates/pack/tests/cli_contract.rs`（実行ファイルの挙動）が次を確かめる。

| 試験 | 内容 |
|---|---|
| `cli_doc_version_matches_constant` | 版の欄が `CLI_CONTRACT_VERSION` と一致し、状態が固定（契約）である |
| `cli_doc_command_table_matches_contract` | 3 節の表が `Subcommand::ALL` の順・名前・引数の形と一致する |
| `cli_doc_usage_block_matches_usage` | 4 節のブロックが `usage()` と完全一致する |
| `cli_doc_exit_code_table_matches_constants` | 5 節の表のコードが 0・1・2 と一致する |
| `cli_doc_item_kinds_table_matches` | 7 節の表の種別が `ITEM_KINDS_V2` と一致し、AUTHOR-2 の 5 種を含む |
| `cli_doc_final_line_examples_match` | 8 節の例のブロックが `final_line`・`compare_*_line` の出力と完全一致する |
| `cli_doc_destination_section_present` | 9 節に標準出力と標準エラー出力の両方がある |
| `usage_declares_compile_output_option` | 使い方に `compile <pack-dir> [-o <file>]` の行がある（`-o` の動作は確かめない） |
| `exit_codes_are_fixed_values` | 終了コードの定数が 0・1・2 である |
| `commands_are_the_six_in_contract_order` | コマンドが 6 つで、AUTHOR-1 の順である |
| `defects_is_not_an_author_command` | `defects` が未知のコマンドで、使い方に出ない |
| `author1_no_args_prints_usage_and_exits_2` | 引数なしで 2・標準エラー出力に使い方・標準出力は空 |
| `unknown_command_exits_2_with_usage` | 未知のコマンドで 2 |
| `argument_shape_errors_exit_2` | 引数の数の誤りで 2・標準出力は空 |
| `exit_0_and_1_follow_diagnostics` | 診断なしで 0、診断ありで 1。`compare` は一致で 0、不一致で 1 |
| `diagnostic_requires_file_and_item` | 空のファイル・項目の診断を作れない |
| `author2_diagnostic_lines_have_four_fields` | 実行ファイルの診断の全行が 4 欄で、ファイルと項目が空でなく、項目が形に合い、コードが `E-` で始まる |
| `diagnostic_separator_in_fields_is_escaped` | ファイル・項目の `: ` を直し、4 欄の分割が崩れない |
| `author2_item_kinds_include_spec_five` | 5 種が項目の形に合い、参考の一覧に含まれる |
| `is_item_rejects_malformed` | 項目の形の判定が、崩れた形を偽にする |
| `author2_final_line_forms` | 最終行の OK・NG の形とラベル |
| `author3_unreadable_final_line` | 読み込めない最終行が常に NG |
| `pack_summary_counts` | 要約の章・モジュール・Step の定義の数 |
| `compare_lines_follow_contract` | `compare` の 3 つの行 |
| `cli_doc_metrics_block_matches_contract` | 10 節の例のブロックが `METRICS_HEADER`・`metrics_row` と一致し、最後の行が `TOTAL` である |
| `diagnostics_go_to_stdout` | 診断は標準出力、標準エラー出力は空 |

## 13. 変更履歴

| 版 | 日付 | 内容 |
|---|---|---|
| 1 | 2026-10-19 | 初版（バイナリ名・コマンド 6 つ・引数の形・使い方・終了コード） |
| 2 | 2026-10-19 | 診断 1 行の形・項目の書式・最終行（`validate`・`compare`）・診断の出し先を足す（TASK-399 2/2） |
| 3 | 2026-10-19 | `metrics` の出力の形（ヘッダ・行・`TOTAL`・数え方・診断時の扱い）を足す（TASK-402） |
