# CLI の外部仕様

```text
CLI の外部仕様の版: 8
決定日: 2026-10-23
状態: 固定（契約）
```

## 1. 位置づけと版

- 本文書は、作成者向け CLI `fandhe-pack` の外部仕様（バイナリ名・コマンド一覧・引数の形・使い方・終了コード・診断 1 行・項目の書式・最終行・診断の出し先・`compile` の出力・`metrics` の出力・`vocab` の出力）を 1 か所に固定する（TASK-399 1/2・2/2・TASK-407・TASK-402・TASK-408・TASK-409・TASK-403、AUTHOR-1〜AUTHOR-3・AUTHOR-8・AUTHOR-9・AUTHOR-17）
- 後続のタスク（TASK-401〜TASK-410）は、入力に「`docs/cli.md` CLI の外部仕様の版 8」の形で書ける
- コード側の正は `crates/pack/src/cli_contract.rs` で、本文書との一致は `crates/pack/tests/cli_contract_doc.rs` が機械で照合する
- 仕様は private のため、本文を転記せず ID（AUTHOR-1〜AUTHOR-3・AUTHOR-9・PACK-2）で参照する
- 版の規則: 固定した行は書き換えない。足すときは版を上げ、変更履歴に記録する。`contract_version`・`ir_format`・診断コードの一覧の版とは別に管理する

## 2. バイナリ名

`fandhe-pack`

## 3. コマンド一覧と引数の形

| コマンド | 引数の形 | 内容 |
|---|---|---|
| `validate` | `<pack-dir>` | 静的検査（TASK-403） |
| `test` | `<pack-dir>` | 検査の後に、模範解答の完走と誤入力の分類（TASK-404） |
| `compile` | `<pack-dir> [--chapters <id>[,<id>...]] [-o <file>]` | IR（JSON）を標準出力、または `-o` のファイルへ出す。`--chapters` で選んだ章だけに絞る |
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
  compile <pack-dir> [--chapters <id>[,<id>...]] [-o <file>]
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

AUTHOR-2 が名指しする 5 種（`step`・`rejection`・`start`・`playthrough`・`error_input`）は、必ずこの名前の種別で出す。次の表は、版 4 時点で実装が使う種別の参考の一覧である。種別を足しても版は上げない（形は上の規則が決める）。

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
- `validate` の最終行の結線と、読み込み失敗での打ち切りは版 8（TASK-403）で `cli::run_validate` に結線した。形の組み立ては `cli_contract::final_line` が持つ
- `validate` の最終行の材料は `source::load` の成否で決める。失敗なら参照などの検査を流さず、読み込みの診断だけを出して `NG: 診断 N 件（読み込めません）` で終わる。成功なら検査の診断を全件出し、要約つきの `OK:`・`NG:` で終わる。`E-REF-BODY`・`E-REF-FIXTURE` は読み込みの層が出すので `読み込めません` 側になり、`E-FORMAT-VERSION` も読み込みの層（`source::load`）が先に出すので `読み込めません` 側になる
- 読み込みの失敗（`E-PARSE`・`E-LIMIT-*`）を直すと、次の実行で参照などの新しい診断が出ることがある。この案内は出力に行を足さず、文書（`docs/pack-authoring.md`）に置く。出力は診断の行の後に最終行がちょうど 1 行で、標準エラー出力は空である
- `compile` は最終行を出さない。成功のときは IR だけを、診断があるときは診断の行だけを標準出力に出す（IR は出さない）。終了コードで区別する
- 終了コードは 5 節のとおり（`OK:` は 0、`NG:` は 1）

## 9. 診断の出し先

| 出力 | 出し先 |
|---|---|
| 診断の行・最終行・`compile` の IR・`compare` の結果行 | 標準出力 |
| 使い方・使い方の誤りの `error:` の行 | 標準エラー出力 |

- 診断のある実行では、標準エラー出力は空である。使い方の誤り（終了コード 2）のときは、標準出力は空である
- 経過措置（契約外）の `not implemented` の行は標準エラー出力に出す（5 節）

## 10. compile の出力

`compile <pack-dir> [--chapters <id>[,<id>...]] [-o <file>]` の出力の契約（AUTHOR-8、TASK-407、TASK-67）。

| 場面 | 標準出力 | 標準エラー出力 | `-o` のファイル | 終了コード |
|---|---|---|---|---|
| `-o` なし・診断 0 件 | IR | 空 | なし | 0 |
| `-o` あり・診断 0 件 | 空 | 空 | 標準出力の場合と同じバイト列 | 0 |
| `-o` あり・診断あり | 診断の行（最終行なし） | 空 | 作らない。既存のファイルにも触れない | 1 |
| `-o` あり・書き出せない | 空 | `error: fandhe-pack compile: cannot write output <path>: <原因>` の 1 行 | 中途半端なファイルを残さない | 2 |

- 標準出力と `-o` のファイルは同じバイト列である。`-o` のとき標準出力へは出さない
- 書き出しは compile が診断なしで終わった後にだけ行う（診断があれば何も書かない。fail-closed）
- 書き出しは、同じディレクトリの一時ファイル（`.<ファイル名>.fandhe-pack-<pid>.tmp`）へ書いて `rename` で置き換える。途中で失敗しても途中までの IR が残らず、既存のファイルより短い IR でも末尾が残らない。失敗した場合は一時ファイルを消す。置き換えるので、既存のファイルの権限は引き継がない
- 書き出せない場合は、親ディレクトリが無い・権限が無い・既存のディレクトリを指す・ファイル名を持たない場合を含み、終了コードは 2 である（5 節）。`<path>` は引用符つきで出し、教材の本文・IR の中身は出さない
- `-o` の値は `-` で始められない。`-o -` を標準出力の意味にはしない
- 末尾の形は、整形した JSON の最後の `}` の後に LF が 1 つだけで、CRLF・BOM を含まず、UTF-8 である
- 同じ内容の教材パックの IR は、何度 compile しても、別のディレクトリへ写しても、ファイルの列挙順が違っても、絶対パスでも相対パスでも、バイト単位で同じになる。IR に絶対パス・時刻・乱数は入らない
- `--chapters <id>[,<id>...]` は、章の定義の `id` で章の範囲を選ぶ（PACK-43・SHELL-12・D-67。試行で配る IR に範囲外の開始点を含めないため）。`-o` と順不同で、どちらも高々 1 回。値が無い・`-` で始まる・空の要素・同じ ID の重複・2 回目の指定は使い方の誤り（終了コード 2）
- 教材パック全体を従来どおり検査してから絞る。診断は指定の有無で変わらず、診断があれば 1 の通り IR を出さない。選んだ ID に該当する章が無い、または同じ ID の章が 2 つ以上あるときは、`error: fandhe-pack compile: unknown chapter: "<id>"`（または `ambiguous chapter: "<id>"`）の 1 行を標準エラー出力に出して終了コード 2。標準出力は空で、`-o` のファイルは作らない
- 絞った IR は、選んだ章の Step・その章から `use` で呼ばれた実体（入れ子を含む）・それらを指す開始点だけを持つ。範囲外の章の開始点・Step・実体と、範囲外の章を宛先にした開始点は含まない。絞った後に開始点が 0 件なら `E-STRUCTURE`（終了コード 1）。全章を選ぶと、指定なしとバイト単位で同じ IR になる。`ir_format` は残った項目に足りる最小の版に求め直す
- 章の `next` は同じ章の中に限る。他の章の Step を指す `next` は `validate`・`compile` とも `E-REF-STEP`（`docs/diagnostics.md` の版 15 の注）
- 画面に配る IR は、この `compile` の出力そのものである。配信物には `IR_FILE=<file> tools/build-web.sh` で `ir/main.json` として入る。配信の実装は TASK-109・TASK-121 の側にある

## 11. `metrics` の出力

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

## 12. vocab の出力

`vocab <pack-dir>` の出力の契約（AUTHOR-9、TASK-408）。教材が既存の語彙の組み合わせだけで書けたのか、名前つき拡張点に頼ったのかを、作成者が機械で確かめる。コード側の正は `cli_contract` の `VOCAB_HEADER`・`vocab_row`・`vocab_summary_lines`・`vocab_pack_line`、数え方は `vocab_report`。

| 場面 | 標準出力 | 標準エラー出力 | 終了コード |
|---|---|---|---|
| 読み込めて、`compile` の診断が 0 件 | 表と集計行（下の例） | 空 | 0 |
| 読み込めない、または `compile` に診断がある | 診断の行だけ（表・集計行なし） | 空 | 1 |

```text
scope,step,kind,extension_points
c1,s1,step,
c1,s2,step,EP-1 EP-7
c1,s3,use,
m1,s4,step,EP-3
# (i) 既存の語彙の組み合わせだけで書けた Step: 1
# (ii) 名前つき拡張点を使って書けた Step: 2
# EP-1 を使った Step: 1
# EP-3 を使った Step: 1
# EP-7 を使った Step: 1
# EP-11 を使った（教材パック全体）: pack.appearance
```

- 表: 先頭はヘッダ `scope,step,kind,extension_points` で、1 行が 1 つの Step の定義である。`scope` は章・モジュールの ID、`step` は Step の ID。行の順は、章の名前の昇順、続いてモジュールの名前の昇順で、各ファイルの中は書かれた順である。呼ばれないモジュールの Step も含め、展開後の複製は数えない
- `kind` は `step` か `use`。`use` は他モジュールの呼び出しで、`extension_points` は空にし、（i）（ii）にも数えない（展開した先の Step で数える）
- `extension_points` は、その Step が使った拡張点の ID を半角空白でつないだもの。並びは拡張点の番号順（`EP-2` の後に `EP-13`）で、使っていなければ空
- 数える拡張点は EP-1〜EP-9 と EP-13 である。EP-3・EP-4 は条件の欄（`next[].when`・規則と効果の `when`）の中の `param`・`clarifying` を数える。EP-7 は Step の `do` だけ、EP-8 は Step の `body_levels` とヒントの `levels` だけを数え、教材パック全体の `guidance_levels`・開始点の `guidance` は数えない。EP-9（開始条件）は、`requires` または `unmet` を持つ開始点が指す Step（`node`）の行に、Step 単位で 1 回数える（語彙の文書が EP-9 を `starts[].requires`・`starts[].unmet` の 2 欄で定めるため、`validate` の使用の判定と同じにする）。片方だけの開始点は `compile` の関門が `E-STRUCTURE` で止めるため、表は出ない（TASK-51）。同じ Step を指す開始点が複数あっても 1 回で、モジュールの開始点はそのモジュールの Step の行に数える。`use` の Step を指す開始点は、`use` 行の拡張点を空に保つため数えない（13 節）。`confirm.options[].label` は拡張点の番号を持たないため数えない（`pack.vocab` の宣言と使用の一致は `validate` が検査する）。決定: D-64、AUTHOR-17
- 集計行は `# ` で始まる。（i）は拡張点を 1 つも使わなかった `step` 行の数、（ii）は 1 つ以上使った `step` 行の数。続けて、1 回以上使われた拡張点だけを、番号順に `# EP-n を使った Step: N` として出す
- 教材パック全体の行（EP-10〜EP-12）は、`pack.profile`・`pack.appearance`・`pack.left_pane` を**読んだ値が既定と違う**ときだけ、末尾に番号順で出す。欄を既定と同じ値で書いても出さない。これらは Step ごとの数（i）（ii）に入れない
- ID の欄に含まれる制御文字（改行を含む）・`,`・`"`・U+2028・U+2029、および `scope` の先頭の `#` は、`\u{XXXX}`（16 進 4 桁・大文字）に直す。1 行が 4 欄に保たれ、表の行が集計行に見えない。出力に写すのは ID・拡張点の ID・固定の欄名だけで、本文・練習データ・条件の値は写さない
- 診断があるときは表を出さない（fail-closed）。診断は 6 節の形で 1 件 1 行、最終行はない。`validate` 専用の検査（`E-VOCAB-MISMATCH`・`E-REF-ELEMENT`・到達性）は関門に入れない
- 集計行は日本語、診断は英語の文言が混ざる。最終行（8 節）と同じ扱いで、機械で読むときは `# ` で始まる行を集計行とみなす

## 13. 本版で扱わない事項

- `use` の Step を指す開始点の EP-9 の数え上げ（`use` 行は拡張点を持たない契約のため数えない）
- `validate` の最終行の結線・読み込み失敗での打ち切り（TASK-403。版 8 で実装した）
- `test` の PASS・FAIL の行・集計・最終行（TASK-404・TASK-405）
- 引数の数の検査と振り分け（TASK-401）
- 診断コードの一覧と実装の一致の検査（TASK-400）
- エスケープ後の文字列を元に戻す規則（`\u{000A}` という文字列そのものを写した場合と区別できない。本版は 4 欄の分割の保証までを契約とする）

## 14. 適合試験

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
| `usage_declares_compile_output_option` | 使い方に `compile <pack-dir> [--chapters <id>[,<id>...]] [-o <file>]` の行がある（動作は `compile_output.rs`） |
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
| `author8_stdout_and_output_file_are_byte_identical`（`compile_output.rs`） | 標準出力と `-o` のファイルが同じバイト列で、`-o` のとき標準出力・標準エラー出力は空 |
| `author8_repeated_runs_are_byte_identical` | 何度 compile しても同じバイト列 |
| `author8_copies_in_other_directories_are_byte_identical` | 別のディレクトリへの写し・列挙順を変えた写し・相対パスでも同じバイト列で、IR にパスが入らない |
| `author8_output_tail_is_fixed` | 末尾が `}` と LF 1 つで、CRLF・BOM がなく、読み戻せる |
| `author8_output_replaces_existing_file_entirely` | 既存のファイルを丸ごと置き換える |
| `author8_diagnostics_do_not_write_output` | 診断があればファイルを作らず、既存にも触れない |
| `author8_unwritable_output_exits_2` | 書き出せないとき 2 と `error:` の 1 行で、一時ファイルを残さない |
| `author8_no_temporary_file_is_left_after_success` | 成功後に一時ファイルが残らない |
| `cli_doc_vocab_example_matches_contract`（`cli_contract_doc.rs`） | 12 節の例のブロックが `VOCAB_HEADER`・`vocab_row`・`vocab_summary_lines`・`vocab_pack_line` の出力と完全一致する |
| `author9_table_and_summary`（`vocab_command.rs`） | 表・`use` 行・（i）（ii）・拡張点ごとの行が期待どおりで、並びが番号順 |
| `author9_pack_wide_lines_only_when_changed` | 教材パック全体の行が、既定から変えた欄だけ末尾に番号順で出る |
| `author9_explicit_default_values_emit_nothing` | 既定と同じ値の明示では教材パック全体の行が出ない |
| `author17_ep9_counted_on_start_node_row` | 開始点に `requires` を足すと、`node` の Step の行に EP-9 が付き、EP-9 の集計行が EP-8 と EP-13 の間に出る |
| `author17_ep9_attributed_to_target_step_not_first` | EP-9 は先頭の Step ではなく開始点の指す Step の行に付く |
| `author17_one_sided_ep9_is_refused_by_compile_gate` | `requires` だけ・`unmet` だけの開始点は `compile` の関門で `E-STRUCTURE`（item は `start:s1`）になり、表を出さず終了コード 1 になる（TASK-51） |
| `author17_two_starts_same_step_counted_once` | 同じ Step を指す開始点が 2 つでも 1 回 |
| `author17_module_start_counts_on_module_row` | モジュールの開始点はそのモジュールの行に付く |
| `author17_start_on_use_step_is_not_counted` | `use` の Step を指す開始点は数えず、出力が変わらない |
| `author17_label_not_counted` | `confirm.options[].label` を足しても出力が変わらない |
| `author9_load_failure_prints_diagnostics_only` | 読み込めないときは 1・診断だけ・標準エラー出力は空 |
| `author9_compile_failure_prints_diagnostics_only` | `compile` に診断があるときは 1・表を出さない |
| `author9_hostile_ids_are_escaped` | ID の `,`・`"`・改行・先頭の `#` を直し、各行が 4 欄に保たれる |
| `author9_module_scope_rows` | 呼ばれないモジュールの Step も数え、`use` 行は（i）（ii）に入らない |
| `author2_ok_pack_prints_ok_line_and_exits_0` | 診断 0 件で `OK:` の 1 行だけ・終了コード 0（`validate_command.rs`） |
| `author2_defective_pack_prints_diagnostics_then_ng_line_and_exits_1` | 診断の行の後に要約つきの `NG:`・件数一致・終了コード 1・標準エラー出力は空 |
| `author2_diagnostic_lines_match_library` | 診断の行が `validate::validate` の結果と完全に一致する |
| `author3_unreadable_causes_end_with_unreadable_line` | 構文・未知の欄・型・必須欄・上限超過・存在しないパスで `読み込めません` |
| `author3_load_failure_suppresses_checks` | 読み込めないとき参照などの診断が出ない（直した写しでは出る負の対照） |
| `author3_format_version_mismatch_is_unreadable` | `E-FORMAT-VERSION` は読み込みの層が出し `読み込めません` で終わる |
| `author2_missing_dir_does_not_leak_path` | 存在しないパスの文字列を標準出力に写さない |
| `i16_validate_is_dispatched_to_validate` | `validate` が未実装の案内ではなく本体に届く（`cli.rs`） |

## 15. 変更履歴

| 版 | 日付 | 内容 |
|---|---|---|
| 1 | 2026-10-19 | 初版（バイナリ名・コマンド 6 つ・引数の形・使い方・終了コード） |
| 2 | 2026-10-19 | 診断 1 行の形・項目の書式・最終行（`validate`・`compare`）・診断の出し先を足す（TASK-399 2/2） |
| 3 | 2026-10-19 | `compile` の出力（`-o` の書き出し・標準出力との同一バイト・末尾・決定性）を 10 節に固定し、以降の節を繰り下げる（TASK-407、AUTHOR-8） |
| 4 | 2026-10-19 | `metrics` の出力の形（ヘッダ・行・`TOTAL`・数え方・診断時の扱い）を足す（TASK-402） |
| 5 | 2026-10-19 | `vocab` の出力（表・集計行・教材パック全体の行・エスケープ・診断時に表を出さない）を 12 節に固定し、以降の節を繰り下げる（TASK-408、AUTHOR-9） |
| 6 | 2026-10-20 | `vocab` が EP-9 を数える（`requires` または `unmet` を持つ開始点が指す Step の行。`label` は数えない）。`use` の Step を指す開始点は数えないことを 13 節に足す（TASK-409、AUTHOR-17、D-64） |
| 7 | 2026-10-22 | `compile` に章の範囲の指定 `--chapters <id>[,<id>...]` を足す（絞った IR・選択の誤りの終了コード 2・全章の指定で同じバイト列。10 節）（TASK-67、PACK-43、D-67） |
| 8 | 2026-10-23 | `validate` を結線する（診断の行・最終行・終了コード 0／1・読み込み失敗で検査を流さず `読み込めません` で終える。8 節の注を足す）（TASK-403、AUTHOR-2・AUTHOR-3） |
