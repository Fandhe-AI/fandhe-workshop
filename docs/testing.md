# 試験の手順（性質テストと変異注入）

```text
決定日: 2026-10-12
対象: TASK-31（性質テストと変異注入の足場）
```

## 1. 位置づけ

- 本文書は、core の性質テストと変異注入の手順、および変異注入の記録の様式を定める
- 具体の試験は後続タスクで育てる。変異の入れ方と記録の様式は本文書に従う（TASK-83・TASK-220・TASK-244）
- 性質テストのヘルパは `crates/core/tests/common/property.rs`、使い方の見本は `crates/core/tests/property_sample.rs`
  にある。見本の `Model` は試験の中で閉じており、エンジンの実装ではない
- CI にはまだ cargo のジョブが無い（TASK-33）。本文書の手順はローカルで流し、CI での実行は未整備である

## 2. 性質テストの手順

ヘルパの API と決定性の規則は `property.rs` のモジュール文書を正とし、ここには転記しない。手順の要点だけを示す。

| 項目 | 内容 |
|---|---|
| 既定の種 | `DEFAULT_SEED`。同じコードなら毎回同じ入力列が流れる |
| 既定のケース数 | `DEFAULT_CASES`（256）。多く流したい試験は、各試験が明示で渡す |
| 探索の仕方 | `FANDHE_PROPTEST_SEED`（64 桁の 16 進）で種を、`FANDHE_PROPTEST_CASES`（正の 10 進整数）でケース数を変える。不正な値は理由を示して失敗する |
| 報告行 | 成功時に `PROPERTY-TEST name=<名前> cases=<ケース数> seed=<種>` を 1 行出す |
| 失敗時の出力 | `cases=` と `seed=` の行と、`minimal input:` の行（縮小後の最小の入力）を含む |
| 再現 | 失敗の出力の種とケース数を環境変数で渡して、同じ試験を再実行する |

再現の例（`<種>`・`<ケース数>` は失敗の出力の値）:

```bash
FANDHE_PROPTEST_SEED=<種> FANDHE_PROPTEST_CASES=<ケース数> cargo test --locked -p fandhe-workshop-core --test property_sample
```

- `failure_persistence` は無効にしている。有効だと `proptest-regressions/` をソースツリーへ書き出し、作業ツリーを書き換えてしまうため
- 試験の成功条件は不変条件だけにする。環境変数で探索を変えても、操作の網羅性に依存しない。網羅性は固定列の試験で確かめる
- 種を変えた探索では、変異注入の記録と結果が変わりうる。記録は既定の種で取る

## 3. 変異注入の手順

変異注入は、実装に誤りを 1 つ入れると、対応する試験が失敗することを機械で確かめる。試験が空振りしていないことの確認である。

### 3.1 方式

`scripts/check-mutations.sh` が、作業ツリーの外の一時コピーを書き換えて試験を流す。

1. `mktemp -d` の一時ディレクトリに `Cargo.toml`・`Cargo.lock`・`rust-toolchain.toml`・`crates/` を写す（`testdata/` があれば、試験の入力として併せて写す）
2. 元の状態（変異なし）で、対象の試験が通り、`expect-fail` の名前の試験が実行されていることを確かめる
3. 変異ごとに写しをやり直し、1 か所を置換して `cargo test --locked` を流す
4. `expect-fail` の試験がすべて失敗していれば DETECTED、1 つでも通っていれば SURVIVED とする
5. 記録の表を標準出力に出す。進行のログは標準エラー出力に出す

- 作業ツリーは書き換えない（`coding-rust.md` が、試験で作業ツリーを書き換えることを禁じている）。中断しても壊れたソースが残らず、並列の worktree でも干渉しない
- `rust-toolchain.toml` を写すのは、rustup が cwd から上へ辿って探すため。写さないと既定のツールチェーンで黙って動く
- `find`・`replace` は文字どおりのデータとして扱う（awk の `index()` による一致）。正規表現・`sed` の式・シェルの展開は使わない

採らなかった案:

| 案 | 採らない理由 |
|---|---|
| `cfg`・feature で変異を切り替える | 配信するコードを変異のために汚す |
| `cargo-mutants` | 新しい供給網の固定が要る。後続タスクは名前を付けた手動の変異を使い、変異ごとに検出した試験を記録する。この形と合わない |
| `.patch` ファイル | 空行の diff 文脈が半角スペース 1 つになり、editorconfig-checker の末尾空白の検査に掛かる |
| 作業ツリーをその場で書き換えて戻す | 中断したときに壊れたソースが残る |

### 3.2 変異の定義

置き場は `crates/<crate>/tests/mutations/<ID>.mutation`。`tests/` の直下でないため、cargo は試験のターゲットとして拾わない。
1 件 1 ファイルで、各行は最初の `: ` で `key: value` に分ける（値は 1 行）。

| キー | 必須 | 内容 |
|---|---|---|
| `id` | 必須 | 変異の ID。サンプルは `S1`・`S2`。TASK-220 は `M1` 以降、TASK-244 は `X1` 以降、TASK-158 は `L1`、TASK-81 は `G1`・`G2`、TASK-180 は `H1`、TASK-290 は `GUIDE17-1`、TASK-256 は `J1`、TASK-291 は `GUIDE18-1`、TASK-309 は `SAVE2-1` 以降、TASK-52 は `PACK20-1` 以降、TASK-47 は `PACK13-1` 以降・`PACK23-1`、TASK-262 は `JUDGE1-1` 以降、TASK-215 は `EVT13-1` 以降、TASK-263 は `JUDGE1-C1`・`JUDGE1-U1`・`JUDGE1-A1`・`JUDGE1-Q1`、TASK-266 は `JUDGE2-1` 以降、TASK-45 は `PACK11-1` 以降、TASK-46 は `PACK15-1` 以降・`PACK16-1` 以降・`PACK12-1` 以降、TASK-272 は `JUDGE6-1` 以降、TASK-305 は `GUIDE18-2` 以降、TASK-207 は `EVT3-1` 以降、TASK-268 は `JUDGE4-1` 以降、TASK-208 は `EVT5-1` 以降、TASK-209 は `EVT7-1` 以降、TASK-269 は `JUDGE5-1` 以降、TASK-315 は `SAVE24-1` 以降、TASK-210 は `EVT6-1` 以降、TASK-343 は `FEND20-1`・`FEND15-1`・`FEND21-1` を使う |
| `package` | 必須 | cargo のパッケージ名（`^[a-z0-9-]+$`） |
| `test` | 必須 | 試験のターゲット名。`--test` に渡す（`^[a-z0-9_]+$`） |
| `target` | 必須 | 書き換えるファイル。repo 根からの相対パス。絶対パス・`..`・symlink は拒否する |
| `find` | 必須 | `target` にちょうど 1 回現れる 1 行の文字列。0 回・2 回以上は定義の不正 |
| `replace` | 必須 | 置き換え後の文字列（1 行） |
| `expect-fail` | 必須・複数可 | 失敗すべき試験関数名（完全一致）。1 行に 1 つ |
| `note` | 任意 | 何を壊したかの説明 |

- 未知のキー・重複した単一キー・必須キーの欠落・空の値は、終了コード 2 で止める
- 1 件の変異は 1 か所の書き換えに限る。複数行・複数か所の変異が要る場合は、後続タスクで形式を拡張する（拡張点: `find` / `replace` を組で複数持てるようにする）
- サンプルは `crates/core/tests/property_sample.rs` の見本を対象にする。エンジンの試験では `crates/core/src/**` などの実装を対象にする

### 3.3 新しい変異を足す手順

1. 壊したい 1 か所を決め、`find`（ちょうど 1 回現れる文字列）と `replace` を書く
2. 失敗すべき試験の関数名を `expect-fail` に書く。名前を推測で書かず、元の状態で `ok` と出る名前を使う
3. `make check-mutations` を流し、DETECTED になることを確かめる
4. 出力された記録の表を、第 5 節の該当の見出しへ貼る（手で作らない）

### 3.4 終了コードと対処

| 終了コード | 意味 | 対処 |
|---|---|---|
| 0 | 全件が検出された | なし |
| 1 | 検出されない変異がある（SURVIVED） | 試験が不足している。試験を足すか、`expect-fail` が事実と合っているかを確かめる |
| 2 | 誤用・定義の不正・元の状態の失敗・コンパイルできない変異 | 標準エラー出力の `error:` に従って直す。2 は合格ではない |

- コンパイルできない変異は、検出に数えない。試験が失敗したのではなく、ビルドが失敗しただけだからである
- `expect-fail` の名前が元の状態の出力に無いと、0 件が実行されて通ったように見える。これも終了コード 2 にする

### 3.5 所要時間と実行の場所

- 所要時間: サンプル 2 件で約 6 秒（2026-10-12、依存を含む冷えた状態のビルド込み）。変異が増えるほど、変異ごとの再ビルドと試験の時間が足される
- 依存のビルドの出力先（`CARGO_TARGET_DIR`）は一時ディレクトリに置き、同じ実行の全変異で共有する
- git hooks と `make check` には入れない。重く、静的な検査だけという `make check` の契約に合わない
- CI への組み込みは TASK-33・TASK-83 が扱う。スクリプト自体の回帰テストは `make test-check-mutations`

## 4. 記録の様式

`scripts/check-mutations.sh` が、次の様式で標準出力に出す。第 5 節にはその出力をそのまま貼る。

- 表の前に、実行日（`YYYY-MM-DD`）・元にしたコミット（短縮ハッシュ。作業ツリーに変更があれば `-dirty`）・実行したコマンドを置く
- 表の列は次のとおり。変異ごとに 1 行を置き、検出した試験の名前を必ず残す

| 列 | 内容 |
|---|---|
| ID | 変異の ID |
| 対象 | `target` と、壊した内容（`note`） |
| 期待した失敗 | `expect-fail` の試験名 |
| 実際に失敗した試験 | 変異を入れて実際に失敗した試験名 |
| 結果 | `DETECTED` または `SURVIVED` |
| 性質テストの種・ケース数・最小の反例 | 性質テストが失敗した場合の `cases=`・`seed=`・`minimal input:`。無ければ `-` |

## 5. 変異注入の記録

系列（タスク）ごとに小見出しを置く。TASK-220 と TASK-244 は、ここに小見出しを足す。

### 5.1 サンプル（TASK-31）

S1 は固定列の試験と性質テストの両方が、S2 は性質テストだけが検出する。S2 が示すとおり、上限を外す誤りは固定列では捉えられず、乱数の入力列が捉える。

```text
実行日: 2026-10-12
元にしたコミット: fc420b3-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| S1 | crates/core/tests/property_sample.rs: 古い進行が状態を変えてしまう（位置を 0 に戻す） | 固定列で古い進行と復元と完了を通る, 乱数の入力列で状態が壊れない | 固定列で古い進行と復元と完了を通る, 乱数の入力列で状態が壊れない | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Advance, Stale] |
| S2 | crates/core/tests/property_sample.rs: 進行の上限を外す（最終位置を超えて進む）。固定列は上限を超えないので、性質テストだけが捉える | 乱数の入力列で状態が壊れない | 乱数の入力列で状態が壊れない | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Advance, Advance, Advance, Advance, Advance] |

### 5.1.1 履歴ファイルの大きさの上限（TASK-155）

ちょうど上限を断る誤りを、境界の試験と性質テストが検出する。一様な `u64` では境界に当たらないため、性質テストは境界付近を混ぜた入力から引く。

```text
実行日: 2026-10-14
元にしたコミット: ab2a1df-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| SAFE18-1 | crates/core/src/history_size.rs: ちょうど 10MB を断る誤り | exact_limit_is_not_rejected_by_size, limit_neighborhood_is_judged_exactly, bytes_are_judged_by_length_without_reading_content, size_rule_holds_for_any_length | exact_limit_is_not_rejected_by_size, limit_neighborhood_is_judged_exactly, size_rule_holds_for_any_length, bytes_are_judged_by_length_without_reading_content | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: 10485760 |

### 5.2 採用する版の feature の宣言（TASK-346）

F1 は、`fandhe-frontend-wasm-full` の `action-keydown` を外す変異である。外してもビルドは通り、`data-action-keydown` と
`data-keys` の属性が黙って効かなくなる。native の試験 `frontend_features` が宣言を検査して検出する。
動作の検出は、ブラウザの試験 `tools/browser/keydown-action.test.mjs` と、feature を外した写しで流す対照の検査
`make test-keydown-control` が担う（第 6 節）。

```text
実行日: 2026-10-14
元にしたコミット: f469e9f-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| F1 | Cargo.toml: action-keydown の feature を外す（keydown の配線が黙って登録されなくなる） | wasm_full_は_既定_feature_なしで_action_keydown_を明示している | wasm_full_は_既定_feature_なしで_action_keydown_を明示している | DETECTED | - |

### 5.3 入力欄の上限（TASK-158）

L1 は、`crates/web/src/input_limit.rs` の上限の判定（`clamp_draft` の近道）を常に真にする変異である。外すとビルドは通り、
20,001 文字目以降が黙って通る。native の統合試験 `input_limit` が検出する。切り詰めた値が欄へ戻ること・案内の表示と消去は、
ブラウザの試験 `tools/browser/input-limit.test.mjs` が担う。

```text
実行日: 2026-10-14
元にしたコミット: ab2a1df-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/L1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| L1 | crates/web/src/input_limit.rs: 上限の判定を外す（20,001 文字目以降が通ってしまう） | 一文字超過は先頭の上限までになり_超過の印が立つ | 一文字超過は先頭の上限までになり_超過の印が立つ, 空_ascii_結合文字の扱い, 十倍の貼り付けも上限になる, サロゲートペアの超過は文字の境界で切る | DETECTED | - |

### 5.3.1 受講者の文字列の非解釈（TASK-150 1/3）

SAFE11-1 は、`crates/web/src/learner_text.rs` の `learner_line` が文字ノードで描く箇所を、生 HTML の出力口に替える変異である。
ビルドは通り、受講者の文字列の記法やタグが要素になる。native の統合試験 `learner_text` が検出する。ソースを読む静的検査
（`raw_html` の不使用）も同時に失敗する。ブラウザでの実行・外部通信・危険な要素の 0 件は
`tools/browser/learner-text.test.mjs` が担う。

```text
実行日: 2026-10-15
元にしたコミット: 0e4f55f-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/SAFE11-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| SAFE11-1 | crates/web/src/learner_text.rs: 受講者の文字列を生 HTML として出す（記法やタグが要素になる） | 記法と_html_は文字のまま描かれ_子の要素にならない | 記法と_html_は文字のまま描かれ_子の要素にならない, web_のソースは_raw_html_を使わない | DETECTED | - |

### 5.3.2 木の読み込みの検査と描画の深さ（TASK-150 2/3）

3 件の変異を足した。いずれもビルドは通り、対応する試験だけが失敗する。

- SAFE12-1: `crates/web/src/doc_view.rs` の描画の深さの上限（32 段）を事実上なくす。native の統合試験 `doc_depth` が検出する
- SAFE12-2: `crates/contract/src/doc.rs` の木の版の検査を外す。版 2 の木が関門と `load_ir` を通るため、`core` の統合試験 `doc_load` が検出する
- SAFE12-3: `crates/web/src/doc_view.rs` の言語名の文字の許可リストを外す。任意の文字列が `class` に入るため、`doc_depth` が検出する

製品の読み込みの経路では、JSON の深さ 64 の関門が引用 50・60 段（JSON で 102・122 段）を先に深さの段で拒否する。
描画の 32 段の上限は、関門を通さない経路（`serde_json` の再帰の上限 128 で読める 50・60 段と、Rust で組み立てた木）で確かめる。
IR に載った木での確認は 5.3.3（TASK-150 の 3/3）に記録した。ブラウザでの確認は `tools/browser/doc-tree.test.mjs` が担う。

```text
実行日: 2026-10-15
元にしたコミット: 0da6993-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/SAFE12-1.mutation crates/core/tests/mutations/SAFE12-2.mutation crates/web/tests/mutations/SAFE12-3.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| SAFE12-1 | crates/web/src/doc_view.rs: 描画の深さの上限を事実上なくす（深い入れ子を省略せず描く） | 引用の入れ子は_32_段で省略になる, リストの入れ子も_32_段で省略になる, 強調の入れ子は上限でインラインの文字が省略になる | 左の 3 件に加え、省略の文言は_p_に入る, 変換器の深さの上限は描画の上限より小さい, 試験データの深い入れ子は単独の経路で期待どおりになる | DETECTED | - |
| SAFE12-2 | crates/contract/src/doc.rs: 木の版の検査を外す（版 2 の木が読み込みを通る） | 語彙の外の種別と未知の項目と木の版の違いは型の段で拒否される, ir_のどの木の版が違っても_load_ir_が拒否する | 左の 2 件 | DETECTED | - |
| SAFE12-3 | crates/web/src/doc_view.rs: 言語名の文字の許可リストを外す（任意の文字列が class に入る） | 任意の言語名で_class_が無いか安全な形になる, 言語名は安全な文字だけのとき_class_になる | 左の 2 件 | DETECTED | - |

### 5.3.3 本文の木を載せた IR の入れ子（TASK-150 3/3）

本文の木・練習データの木を IR の 3 か所（`body_doc`・`body_levels_doc`・`fixture_docs`）に載せ、`load_ir` に通した結果を記録する。
5.3.2 までの根拠は、木を単独で `serde_json` に通す比較用の経路だけだった。製品の経路では木は IR に埋まり、IR 全体が
JSON の深さ 64 の検査（SAFE-20）を解釈の前に受ける。この 2 つの上限の関係を、自動テストで固定した。

- `crates/core/tests/doc_load.rs`: 3 か所 × 50・60 段が `load_ir`・`check_ir_limits` とも `too_deep`（深さの段）で拒否されること、位置ごとの読める最大の引用の段数
- `crates/web/tests/doc_depth.rs`: 読める最大の木が描画の上限（32 段）に届かず省略にならないこと、compile した教材パック（`fixture-pack`）の木が単独の関門を通り、省略・版の拒否の文言なしで描けること（SAFE-37 の側面）

```text
実行日: 2026-10-15
元にしたコミット: 8e81e85-dirty（本変更を含む作業ツリー）
コマンド: cargo test --locked -p fandhe-workshop-core --test doc_load / cargo test --locked -p fandhe-workshop-web --test doc_depth
```

| 木の位置 | 引用の段数 | IR の JSON の深さ | `load_ir` の結果 | 描画の省略 |
|---|---|---|---|---|
| `body_doc` | 29（最大） | 63 | 読める | なし（blockquote 29 個） |
| `body_doc` | 30・50・60 | 64 を超える | `too_deep`（深さの段） | 解釈の前に拒否のため描画に至らない |
| `body_levels_doc` | 29（最大） | 64 | 読める | なし |
| `body_levels_doc` | 30・50・60 | 64 を超える | `too_deep`（深さの段） | 描画に至らない |
| `fixture_docs` | 30（最大） | 64 | 読める | なし |
| `fixture_docs` | 31・50・60 | 64 を超える | `too_deep`（深さの段） | 描画に至らない |

`fixture_docs` で IR の経路から読める最大の入れ子は、リストが 20 段（1 段で JSON 3 段）、強調が 28 段で、どれも描画の上限 32 段に届かない。
結論として、IR の経路では 50・60 段の木は深さの段で解釈の前に拒否され、IR から読んだ本文で描画の省略の文言は出ない。
32 段の上限は、IR の検査を経ない木（単独の経路・Rust で組み立てた木）に対する多層防御として残る。

変異は足さない。自然な対象は深さの段の除去で SAFE20-1 と重なり、`MAX_JSON_DEPTH` の値は SAFE-42 の試験が固定している。
SAFE20-1・SAFE12-1 を再実行し、検出が保たれることを確かめた。この 2 つの変異に設定された試験の対象は既存の試験で、
新しい試験は失敗側に現れなかった（新しい試験は上の結論を固定する回帰試験であり、変異の検出源ではない）。

```text
実行日: 2026-10-15
元にしたコミット: 8e81e85-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAFE20-1.mutation crates/web/tests/mutations/SAFE12-1.mutation
```

| ID | 結果 |
|---|---|
| SAFE20-1 | DETECTED |
| SAFE12-1 | DETECTED |

#### 04 の SAFE-12 への反映案

`docs/spec/**` は変更しない（D-40）。05 の「04 への指摘」の番号 14 を解消する案であり、変更は spec 側で行う。

- 期待: 「50 段・60 段は描画の深さの上限（32 段）で省略になる」を、木を単独で読む経路（PoC-7 の手順 2。`load_ir` を通さない）に限る。
  製品の IR の読み込みの経路では、本文・練習データの木を IR に載せた 50 段・60 段は、IR 全体の入れ子の検査（SAFE-20、64 段）で解釈の前に `too_deep` で拒否され、描画の省略には至らない、を加える
- SAFE-20 との関係: 引用・リスト項目・強調の 1 段は JSON で 2 段以上を使い、木の位置で IR の中での開始の深さが違う
  （`fixture_docs` は 4 段目、`body_doc`・`body_levels_doc` は 5・6 段目から始まる）。IR の経路で読める引用は最大 29〜30 段で、描画の上限 32 段に届かない。
  判定の順序の正は SAFE-20 のままとする
- 根拠: PoC-7 の手順 2 に、TASK-150 の自動テスト（`本文の木を載せた_ir_で_50_段と_60_段は深さの段で拒否される`・`本文の木を載せた_ir_で読める最も深い引用の境界`・`ir_から読める最も深い引用は描画の上限に届かない`）を足す
- 証拠の種類: 「本文の木が IR に載った状態で 50 段・60 段が IR 全体の 64 段の検査を通るかは未確認」を、確認した結果（3 か所すべてで深さの段で拒否）に置き換える
- 参考（SAFE-37）: 「練習データの内容を木にして載せた IR が木の検査を通るか」は、`compile_した教材パックの木は単独の関門も通り省略なしで描ける` で確かめた。SAFE-37 の状態の変更は本タスクの範囲外

### 5.4 判定の順序（TASK-157）

SAFE20-1 は深さの段を外し、SAFE20-2 は深さの段を大きさの段より前に入れ替える。どちらも `tests/load_order.rs` が検出する。

```text
実行日: 2026-10-14
元にしたコミット: ad0b271-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| SAFE20-1 | crates/core/src/load_gate.rs: 入れ子の深さの段を外す（解釈の前に深さで断らない） | depth_is_decided_before_parse, deep_valid_json_is_too_deep, depth_check_precedes_serde_recursion_limit, rejecting_stage_is_the_first_violated_in_contract_order | deep_valid_json_is_too_deep, depth_is_decided_before_parse, depth_check_precedes_serde_recursion_limit, rejecting_stage_is_the_first_violated_in_contract_order | DETECTED | - |
| SAFE20-2 | crates/core/src/load_gate.rs: 深さの段を大きさの段より先にする（判定の順序の入れ替え） | size_is_decided_before_depth, rejecting_stage_is_the_first_violated_in_contract_order | size_is_decided_before_depth, rejecting_stage_is_the_first_violated_in_contract_order | DETECTED | - |

### 5.5 本文の画像の許可（TASK-149 2/2）

SAFE10-1 は画像の src の「スキームを持つものを拒否する」判定を外し、SAFE10-2 は画像がリンクの規則（スキーム相対の拒否）を
通さないようにする。どちらも外部の画像が `img` になり、native の統合試験 `doc_links` が検出する。
SAFE9-1・SAFE9-2 は画像の判定がリンクの規則を流用するため、画像の試験も失敗する（`find` の行が一意であることは確認した）。
ブラウザでの通信 0 と相対画像の表示は `tools/browser/doc-links.test.mjs` が担う。

```text
実行日: 2026-10-15
元にしたコミット: 57925dd-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/SAFE9-1.mutation crates/web/tests/mutations/SAFE9-2.mutation crates/web/tests/mutations/SAFE10-1.mutation crates/web/tests/mutations/SAFE10-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| SAFE9-1 | crates/web/src/doc_view.rs: スキーム相対（// と \ 始まり）の拒否を外す（外部ホストへのリンクが a になる） | 許可外のリンクは文字として出る | スキーム相対の画像は_img_を作らない, 変換器を通した経路でも許可外のリンクは_a_にならない, 許可外のリンクは文字として出る, 変換器を通した経路でも外部の画像は_img_にならない | DETECTED | - |
| SAFE9-2 | crates/web/src/doc_view.rs: スキームの許可リストを外して常に許可にする（javascript: と data: が a になる） | 許可外のリンクは文字として出る | 全件で危険な要素と属性が出ない, 変換器を通した経路でも許可外のリンクは_a_にならない, 許可外のリンクは文字として出る | DETECTED | - |
| SAFE10-1 | crates/web/src/doc_view.rs: 画像のスキームの拒否を外す（https の外部画像が img になる） | 外部と危険な画像は_img_を作らず代替の文字を出す, 変換器を通した経路でも外部の画像は_img_にならない | 全件で危険な要素と属性が出ない, 変換器を通した経路でも外部の画像は_img_にならない, 外部と危険な画像は_img_を作らず代替の文字を出す | DETECTED | - |
| SAFE10-2 | crates/web/src/doc_view.rs: 画像のスキーム相対の拒否を外す（//host の外部画像が img になる） | スキーム相対の画像は_img_を作らない, 変換器を通した経路でも外部の画像は_img_にならない | スキーム相対の画像は_img_を作らない, 変換器を通した経路でも外部の画像は_img_にならない | DETECTED | - |

### 5.6 案内レベルとヒントの契約（TASK-290）

GUIDE17-1 は `visible_hints` の「`levels` が空の配列なら全水準で出す」分岐を外す。空の `levels` を持つヒントが
どの水準でも出なくなり、契約適合試験 `guidance_contract` が検出する。

```text
実行日: 2026-10-15
元にしたコミット: 7b311a1-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/contract/tests/mutations/GUIDE17-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| GUIDE17-1 | crates/contract/src/guidance.rs: levels が空の配列のとき全水準で出す分岐を外す（空の levels のヒントが誰にも出なくなる） | empty_levels_means_all_levels | empty_levels_means_all_levels | DETECTED | - |

### 5.7 判定の型の次の行動（TASK-256）

J1 は `NextAction::new` の空の検査を外す。空の次の行動を持つ拒否・問い返し・注記が作れてしまうため、
`tests/judgment_contract.rs` が検出する。語彙の文書との一致の試験は `docs/` を読むため、変異注入の写しに含まれず、
別のターゲット（`tests/judgment_vocabulary.rs`）に分けた。

```text
実行日: 2026-10-15
元にしたコミット: 7b311a1-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/contract/tests/mutations/J1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| J1 | crates/contract/src/judgment.rs: 次の行動の空の検査を外す（空の次の行動を持つ拒否が作れてしまう） | next_action_new_rejects_empty_and_whitespace, deserialization_rejects_empty_next_action | deserialization_rejects_empty_next_action, next_action_new_rejects_empty_and_whitespace | DETECTED | - |

### 5.7 履歴の包みのログの並び（TASK-309 3/3）

SAVE2-1 は、参照の読み込み（`tests/common/history_sample.rs`）でログの先頭の 2 件を入れ替える。PoC-6 の X-6
（読み込みでログの並びを変える）に当たる誤りで、往復の照合・バイト一致・性質テストが検出する。
決定的な試験だけでは見つからなかった誤りを、性質テストが最小の反例 `[0, 0]`（Step 名の違う 2 件のログ）で検出する。
比較器そのものの自己確認（`comparator_self_check`）は、`Restored.log_len` を 1 ずらす読み込みなども断ることを確かめる。

```text
実行日: 2026-10-15
元にしたコミット: 5557d27-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAVE2-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| SAVE2-1 | crates/core/tests/common/history_sample.rs: 読み込みでログの先頭の 2 件を入れ替える（PoC-6 の X-6 に当たるログの並びの誤り） | samples_roundtrip_preserves_log_order_and_positions, samples_are_byte_identical_after_roundtrip, property_roundtrip_preserves_log_order | samples_roundtrip_preserves_log_order_and_positions, comparator_self_check, property_roundtrip_preserves_log_order, samples_are_byte_identical_after_roundtrip | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [0, 0] |

### 5.8 学習目標の確認の記録（TASK-291）

GUIDE18-1 は `derive_records` で、誤答の後の正答を選び直しの正答ではなく最初の回答の正答として記録する。
最初の回答と選び直しの正答の区別（GUIDE-18）がつぶれ、契約適合試験 `lo_check_contract` が検出する。
語彙の文書との一致の試験は `docs/` を読むため、変異注入の写しに含まれず、別のターゲット
（`tests/lo_check_vocabulary.rs`）に分けた。

```text
実行日: 2026-10-15
元にしたコミット: d58c881-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/contract/tests/mutations/GUIDE18-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| GUIDE18-1 | crates/contract/src/lo_check.rs: 誤答の後の正答を最初の回答の正答として扱う（選び直しの正答と最初の回答の区別がつぶれる） | reselection_after_comparison_is_kept_apart_from_first_answer | derived_records_always_validate, derive_cases_match_samples, reselection_after_comparison_is_kept_apart_from_first_answer | DETECTED | - |

### 5.9 入力の分けの共有（TASK-52 1/2）

画面と教材テストが同じ関数（`contract::profile::classify_input`）で入力を分けることを確かめる。
PACK20-1 は contract 側の分けの表の試験で、PACK20-2 は同じ変異を web の試験（web の `CommandPrefix::classify` と
pack の `classify_play_input` の両方を流す）で検出する。1 つの変異が両方の入口を同時に壊すので、両方が同じ関数を通っている。

```text
実行日: 2026-10-15
元にしたコミット: d977fbe-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/contract/tests/mutations/PACK20-1.mutation crates/web/tests/mutations/PACK20-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| PACK20-1 | crates/contract/src/profile.rs: 入力の分けで、接頭辞で始まらない自然文もコマンドにする（画面と教材テストが共有する分けの核が壊れる） | input_split_table | input_split_table | DETECTED | - |
| PACK20-2 | crates/contract/src/profile.rs: 共有の分けを壊すと、画面側と教材テスト側の両方の入口が同時に壊れる（両方が同じ関数を通っている確認） | screen_and_play_agree | screen_and_play_agree | DETECTED | - |

### 5.10 入力欄の有無と関連する診断（TASK-52 2/2）

教材テストの `text:` の分けを、画面と同じ関数（`classify_play_input`）の結果で判定していることを確かめる。
判定を反転すると、`$` の接頭辞でも接頭辞なしでも `E-PLAY-INPUT-CLASS` の件数が変わり、試験が失敗する。

```text
実行日: 2026-10-15
元にしたコミット: 65a0e90-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/PACK20-3.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| PACK20-3 | crates/pack/src/profile.rs: 教材テストの text: の判定を反転する（接頭辞で始まる text: を見逃し、始まらない text: を誤って報告する） | play_class_dollar | play_class_dollar, play_class_empty_prefix | DETECTED | - |

### 5.11 確認の未指定の検査（TASK-47 1/2）

入力を持つ Step が確認も理由つきの省略も持たないことを `E-NO-CONFIRM` で拒否する検査の 2 か所を壊し、それぞれ試験が失敗することを確かめる。
PACK13-1 は確認の無い Step を検査から外し、PACK13-2 は `none` の理由の有無の判定を外す。

```text
実行日: 2026-10-15
元にしたコミット: 9ef92e7-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/PACK13-1.mutation crates/pack/tests/mutations/PACK13-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| PACK13-1 | crates/pack/src/confirm.rs: 確認の無い Step を検査から外す（入力を持つのに確認も理由つきの省略も無い Step が、検証を通ってしまう） | pack13_missing_confirm_is_no_confirm | pack13_missing_confirm_is_no_confirm | DETECTED | - |
| PACK13-2 | crates/pack/src/confirm.rs: none の理由の有無の判定を外す（理由の無い確認の省略が、検証を通ってしまう） | pack13_none_without_reason_is_no_confirm | pack13_none_without_reason_is_no_confirm | DETECTED | - |

### 5.12 確認の選択肢の表示名（TASK-47 2/2）

表示名の解決で `label` を無視し常に ID を返す誤りを入れ、表示名とログの文面の試験が失敗することを確かめる。

```text
実行日: 2026-10-15
元にしたコミット: 8a63d72-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/contract/tests/mutations/PACK23-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| PACK23-1 | crates/contract/src/confirm_spec.rs: 表示名の解決で label を無視し常に ID を返す（label を書いた選択肢が画面と操作のログで表示名にならない） | pack23_label_is_displayed_and_missing_label_falls_back_to_id | pack23_label_is_displayed_and_missing_label_falls_back_to_id, pack23_selection_log_text_uses_display_name | DETECTED | - |

### 5.13 コマンド入力の字句化（TASK-262 1/2）

結果を契約の不変条件に通さない誤りと、名前つき引数の値を書かれたとおりに保たない誤りを入れ、失敗の表・性質テスト・golden の再現が失敗することを確かめる。

```text
実行日: 2026-10-15
元にしたコミット: 5f15bd3-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/JUDGE1-1.mutation crates/core/tests/mutations/JUDGE1-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE1-1 | crates/core/src/command_lex.rs: 結果を契約の不変条件に通さない（`--` だけの字句と `---x` が通ってしまう） | failure_cases, lexing_properties | failure_cases, lexing_properties | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (1, "- -- ") |
| JUDGE1-2 | crates/core/src/command_lex.rs: 名前つき引数の値を書かれたとおりに保たない（値に余計な文字が付く） | golden_is_reproduced, normal_cases | golden_is_reproduced, normal_cases | DETECTED | - |

### 5.14 変更前の値を持つ Changed（TASK-215 1/2）

操作 1 件の前後の世界状態の差分から `Changed` を作る 2 か所を壊し、それぞれ試験が失敗することを確かめる。
EVT13-1 は差分の比較を常に真にし、EVT13-2 は変更前を変更後の値にする。

```text
実行日: 2026-10-15
元にしたコミット: f477f73-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT13-1.mutation crates/core/tests/mutations/EVT13-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT13-1 | crates/core/src/changed.rs: 差分の比較を常に真にする（値が変わっていない経路にも Changed が出る） | evt13_world_diff_covers_add_remove_change_and_equal, evt13_put_then_remove_and_same_value_put_record_nothing, p1b_effects_only, p1b_with_whole_world_replacement | evt13_world_diff_covers_add_remove_change_and_equal, evt13_put_then_remove_and_same_value_put_record_nothing, p1b_with_whole_world_replacement, p1b_effects_only | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [[Replace(0, 0)], []] |
| EVT13-2 | crates/core/src/changed.rs: Changed の変更前を変更後の値にする（変更前の値が操作前の値でなくなる） | evt13_same_path_changed_twice_is_one_change_with_original_before, p1b_effects_only, p1b_with_whole_world_replacement | evt13_same_path_changed_twice_is_one_change_with_original_before, p1b_effects_only, p1b_with_whole_world_replacement | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [[Put(0, 0, 0)]] |

### 5.15 ログ・変化の印・復習の導出（TASK-215 2/2）

導出の 4 か所を壊し、それぞれ試験が失敗することを確かめる。EVT13-3 は変化の原因の絞り込みを外し、EVT13-4 は印の併合で
最初の変更前を失わせ、EVT13-5 は増分の開始位置を 1 つ戻し、EVT13-6 は復帰で保存の時点の復習を引かない。

```text
実行日: 2026-10-15
元にしたコミット: cf22bb0-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT13-3.mutation crates/core/tests/mutations/EVT13-4.mutation crates/core/tests/mutations/EVT13-5.mutation crates/core/tests/mutations/EVT13-6.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT13-3 | crates/core/src/projection.rs: 変化の原因が Input かの絞り込みを外す（停止・移動・復帰・開始の巻き戻しも印・復習に入る） | evt13_non_input_changes_are_not_marked_or_reviewed, p1c_p1d_without_restore, p1c_p1d_with_save_restore | evt13_non_input_changes_are_not_marked_or_reviewed, p1c_p1d_without_restore, p1c_p1d_with_save_restore | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Input { step: 0, world: [None, None, None, None], extra: 0 }, Input { step: 0, world: [None, None, Some(0), None], extra: 0 }, Stop { step: 0, pick: 0 }]; cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Input { step: 0, world: [None, None, None, None], extra: 0 }, Input { step: 0, world: [None, None, None, None], extra: 0 }, Input { step: 0, world: [None, None, None, None], extra: 0 }, Input { step: 0, world: [None, None, None, Some(0)], extra: 0 }, Stop { step: 0, pick: 105 }] |
| EVT13-4 | crates/core/src/projection.rs: 印の併合で変更前も上書きする（最初の変更前が残らない） | evt1_marks_merge_first_before_last_after | evt1_marks_merge_first_before_last_after | DETECTED | - |
| EVT13-5 | crates/core/src/projection.rs: extend の開始位置を 1 つ戻す（増分だけで直前の 1 件を二重に処理し、一括と食い違う） | p1c_p1d_without_restore, p1c_p1d_with_save_restore | p1c_p1d_with_save_restore, p1c_p1d_without_restore | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Save { step: 0 }, Input { step: 0, world: [None, None, None, None], extra: 0 }]; cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Input { step: 0, world: [None, None, None, None], extra: 0 }] |
| EVT13-6 | crates/core/src/projection.rs: 復帰で保存の時点の復習を引かない（持ち主の根の項目が保存の時点へ戻らず空になる） | evt14_restore_rebuilds_owner_review_and_keeps_other_roots, p1c_p1d_with_save_restore | evt14_restore_rebuilds_owner_review_and_keeps_other_roots, p1c_p1d_with_save_restore | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Input { step: 0, world: [None, None, None, None], extra: 0 }, Input { step: 2, world: [None, None, Some(0), None], extra: 0 }, Save { step: 0 }, Restore { pick: 0 }] |

### 5.16 コマンド入力の宣言との照合（TASK-262 2/2）

名前つき引数の名前の `-`／`_` の同一視を外す誤りと、必須の引数が無いときに拒否しない誤りを入れ、PoC-3 の組・正規化の試験・性質テストが失敗することを確かめる。

```text
実行日: 2026-10-15
元にしたコミット: 6e33ca4-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/JUDGE1-3.mutation crates/core/tests/mutations/JUDGE1-4.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE1-3 | crates/core/src/command_judge.rs: 名前つき引数の名前の `-`／`_` の同一視を外す（`--external-checks` が宣言の `external_checks` に当たらない） | poc3_pairs, named_key_normalization | named_key_normalization, poc3_pairs, unchecked_spec_is_total | DETECTED | - |
| JUDGE1-4 | crates/core/src/command_judge.rs: 必須の引数が無いときに拒否せず通す（必須の検査を外す） | poc3_pairs, positional_resolution, judging_properties | positional_resolution, order_of_violations, poc3_pairs, judging_properties | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (CommandSpec { name: "cmd", args: [ArgSpec { name: "a", positional: false, required: true, shape: KebabCase }] }, LexedCommand { name: "cmd", args: [] }) |

### 5.16 選択肢・画面操作・承認の判定（TASK-263）

宣言にあるかの検査・画面操作の path の比較・承認の対象の比較・`answer` の正誤の判定を 1 か所ずつ外し、受付になってはならない入力が通ることを試験が検出することを確かめる。

```text
実行日: 2026-10-15
元にしたコミット: cf22bb0-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/JUDGE1-C1.mutation crates/core/tests/mutations/JUDGE1-U1.mutation crates/core/tests/mutations/JUDGE1-A1.mutation crates/core/tests/mutations/JUDGE1-Q1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE1-C1 | crates/core/src/select_judge.rs: 選択肢の、宣言にあるかの検査を外す（宣言に無い ID を指す規則が受付になる） | choice_rejects_undeclared_option | choice_rejects_undeclared_option, accepted_only_for_declared_choice_values | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: Choice("ghost") |
| JUDGE1-U1 | crates/core/src/select_judge.rs: 画面操作の宣言の照合で path の比較を外す（view が同じなら宣言に無い path も通る） | ui_action_rejects_other_path | ui_action_rejects_other_path | DETECTED | - |
| JUDGE1-A1 | crates/core/src/select_judge.rs: 承認の対象の比較を外す（対象なしの「直す」が対象つきの規則に一致する） | approval_revise_without_rule_rejects | approval_revise_without_rule_rejects | DETECTED | - |
| JUDGE1-Q1 | crates/core/src/select_judge.rs: answer の correct の判定を外す（誤答でも確認になる） | confirm_answer | confirm_answer | DETECTED | - |

### 5.17 自然文の受付（TASK-266）

受付の適用で Step を達成にする誤りと、受付で保持する文を `trim` 後の値にする誤りを入れ、受付だけでは達成にならないことの試験・原文保持の試験・性質テストが失敗することを確かめる。

```text
実行日: 2026-10-15
元にしたコミット: 216f6b5-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/JUDGE2-1.mutation crates/core/tests/mutations/JUDGE2-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE2-1 | crates/core/src/text_judge.rs: 受付の適用で Step を達成にする（受付だけでは達成にならない、が崩れる） | accept_reflects_into_world_without_achieving, judging_properties | accept_reflects_into_world_without_achieving, judging_properties | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (4, 0, "מּ") |
| JUDGE2-2 | crates/core/src/text_judge.rs: 受付で保持する文を trim 後の値にする（受講者の文が原文のまま残らない） | text_is_kept_verbatim, judging_properties | text_is_kept_verbatim, judging_properties | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (4, 0, "a ") |

### 5.18 誤入力の期待の読み取り（TASK-45 2/2）

`rejected:free_text_unjudged` を別の拒否の分類に読み替える誤りを入れ、受付に付く分類が拒否の期待として通ってしまうことを試験が検出することを確かめる。

```text
実行日: 2026-10-15
元にしたコミット: cf22bb0-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/PACK11-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| PACK11-1 | crates/pack/src/play_expect.rs: rejected:free_text_unjudged を別の拒否の分類に読み替える（受付に付く分類が拒否の期待として通ってしまう） | pack11_expect_grammar_rejects_malformed | pack11_expect_grammar_rejects_malformed, pack11_error_input_expect_diagnostics | DETECTED | - |

### 5.18 誤答ごとの拒否文面 `on_wrong`（TASK-46 1/2）

正答の選択肢の `on_wrong` の検査を外す誤り（PACK15-1）と、選択肢の `on_wrong` の参照先の検査を外す誤り（PACK15-2）を入れ、試験が検出することを確かめる。

```text
実行日: 2026-10-15
元にしたコミット: 13e0e01-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/contract/tests/mutations/PACK15-1.mutation crates/pack/tests/mutations/PACK15-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| PACK15-1 | crates/contract/src/confirm_spec.rs: 正答の選択肢の on_wrong を拒否する検査を外す（正答に誤答の文面が付いたまま通る） | pack15_on_wrong_on_correct_option_is_rejected | pack15_on_wrong_on_correct_option_is_rejected, pack15_error_display_has_no_input_value | DETECTED | - |
| PACK15-2 | crates/pack/src/validate.rs: 選択肢の on_wrong の参照先の検査を外す（存在しない文面を指しても通る） | pack15_undefined_option_on_wrong_is_ref_rejection | pack15_undefined_option_on_wrong_is_ref_rejection | DETECTED | - |

### 5.19 問い返しの判定（TASK-272）

`on.missing` の照合を反転する誤り（JUDGE6-1）と、問い返しを拒否の種類に写す誤り（JUDGE6-2）を入れ、`st-1-4` の問い返しと拒否の件数への非算入を試験が検出することを確かめる。

```text
実行日: 2026-10-15
元にしたコミット: b85febd-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/JUDGE6-1.mutation crates/core/tests/mutations/JUDGE6-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE6-1 | crates/core/src/command_rule.rs: on.missing の照合を反転する（値のある引数で問い返しになり、値なしでは問い返しにならない） | clarify_on_missing_name_ch1_pack, clarify_on_missing_name_compact, clarify_properties | cancel_rule_is_skipped_for_commands, clarify_on_missing_name_compact, clarify_properties, clarify_on_missing_name_ch1_pack, named_arg_present_does_not_clarify | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: 規則 1 件（command=cmd・missing=["a"]・clarify）、引数なし |
| JUDGE6-2 | crates/core/src/command_rule.rs: 問い返しを拒否の種類に写す（問い返しが拒否の件数に数えられる） | clarify_is_not_counted_as_rejection, clarify_on_missing_name_ch1_pack | clarify_on_missing_name_compact, clarify_on_missing_name_ch1_pack, clarify_is_not_counted_as_rejection, clarify_properties | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: 規則 1 件（command=cmd・clarify）、引数なし |

### 5.20 承認の「直す」の対象つき入力（TASK-264）

対象なしの「直す」で拒否以外の規則を飛ばす検査・拒否の文面の `on_reject` の参照・対象の一覧の重複の除去を 1 か所ずつ外し、対象つきの規則がある Step で対象なしの「直す」が受付になることと、文面の解決の誤りを試験が検出することを確かめる。

```text
実行日: 2026-10-15
元にしたコミット: 13e0e01-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/JUDGE10-1.mutation crates/core/tests/mutations/JUDGE10-2.mutation crates/core/tests/mutations/JUDGE10-3.mutation crates/core/tests/mutations/JUDGE1-A1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE10-1 | crates/core/src/select_judge.rs: 対象なしの「直す」で拒否以外の規則を飛ばす検査を外す（対象つきの規則がある Step で受付・問い返しになる） | contradictory_pack_never_accepts_bare_revise | contradictory_pack_never_accepts_bare_revise, targeted_steps_never_accept_bare_revise_and_always_give_next_action | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: Approval { decision: Revise, target: None } |
| JUDGE10-2 | crates/core/src/rejection_text.rs: 拒否の文面の解決で Step の on_reject の参照を外す（既定の文面だけを引く） | on_reject_overrides_default_text | resolution_failures_are_errors_not_empty_actions, on_reject_overrides_default_text | DETECTED | - |
| JUDGE10-3 | crates/core/src/select_judge.rs: 対象の一覧で重複の除去を外す（同じ対象が複数回出る） | revise_targets_lists_in_order_without_duplicates | revise_targets_lists_in_order_without_duplicates | DETECTED | - |
| JUDGE1-A1 | crates/core/src/select_judge.rs: 承認の対象の比較を外す（対象なしの「直す」が対象つきの規則に一致する） | approval_revise_without_rule_rejects | approval_revise_without_rule_rejects | DETECTED | - |

### 5.21 LO-2 の確認の数え方（TASK-305）

誤答の後の選び直しの正答を自力に数える変異と、集計の LO の絞り込みを外す変異を入れ、最初の回答の正答だけを自力に数えることと、指定した LO にだけ当てることを試験が検出することを確かめる。

```text
実行日: 2026-10-15
元にしたコミット: 492c3ea-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/GUIDE18-2.mutation crates/core/tests/mutations/GUIDE18-3.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| GUIDE18-2 | crates/core/src/lo_first_answer.rs: 誤答の後の選び直しの正答を自力に数える（最初の回答の正答だけを自力に数える規則がつぶれる） | reselection_after_comparison_is_not_self_reliant_and_kept_apart | from_history_observations, contract_samples_classify_as_expected, hint_stage_does_not_change_outcome, reselection_after_comparison_is_not_self_reliant_and_kept_apart, reselection_without_comparison_is_not_self_reliant, tally_counts_only_the_designated_lo, self_reliant_iff_first_answer_correct, tally_partitions_matching_records | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: LoCheckRecord { step: "s", lo: ["LO-1"], method: PointAt, first_answer: Wrong, reselection: Correct, after_comparison: false, max_hint: 0 } |
| GUIDE18-3 | crates/core/src/lo_first_answer.rs: 集計の LO の絞り込みを外す（指定した LO 以外の最初の回答の正答も自力に数える。他の LO へ一般化しない規則がつぶれる） | tally_counts_only_the_designated_lo | tally_counts_only_the_designated_lo, tally_partitions_matching_records | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [LoCheckRecord { step: "s", lo: ["LO-3"], method: PointAt, first_answer: Correct, reselection: None, after_comparison: false, max_hint: 0 }] |

### 5.22 受け付けたうえで止まる場面の判定（TASK-273）

受付で Step を達成にする誤り（JUDGE7-1）と、受付の効果を適用しない誤り（JUDGE7-2）を入れ、`st-7-3` の受付と世界状態の停止（`state=blocked`・フラグ `stopped`）を試験が検出することを確かめる。

```text
実行日: 2026-10-15
元にしたコミット: 1345819-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/JUDGE7-1.mutation crates/core/tests/mutations/JUDGE7-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE7-1 | crates/core/src/text_judge.rs: 受付の適用の共通部分で Step を達成にする（受付で止まらず、達成に進んでしまう） | st_7_3_same_input_is_accepted_and_stops_world, accept_properties | st_7_3_same_input_is_accepted_and_stops_world, accept_properties | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([], false, true) |
| JUDGE7-2 | crates/core/src/command_accept.rs: 受付の効果を適用しない（受付になっても世界状態が止まらない） | st_7_3_same_input_is_accepted_and_stops_world, st_7_1_and_st_7_3_differ | st_7_3_same_input_is_accepted_and_stops_world, st_7_1_and_st_7_3_differ | DETECTED | - |

### 5.23 差し込みの枠と次の行動（TASK-46 2/2）

枠の構文（閉じていない枠）、差し込んだ値を再び走査しない性質、未知の枠の名前・分類の制限・代替の文・`do` の要求の各検査、次の行動の空の静的・動的な検査を外す誤りを入れ、試験が検出することを確かめる。

```text
実行日: 2026-10-15
元にしたコミット: b85febd-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/contract/tests/mutations/PACK16-1.mutation crates/contract/tests/mutations/PACK16-6.mutation crates/pack/tests/mutations/PACK16-2.mutation crates/pack/tests/mutations/PACK16-3.mutation crates/pack/tests/mutations/PACK16-4.mutation crates/pack/tests/mutations/PACK16-5.mutation crates/pack/tests/mutations/PACK12-1.mutation crates/pack/tests/mutations/PACK12-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| PACK16-1 | crates/contract/src/slot.rs: 閉じていない枠の検出を外す（{{ が閉じていない文面が枠として通ってしまう） | pack16_parse_rejects_unclosed | pack16_parse_rejects_unclosed, pack16_errors_do_not_embed_input, pack16_substitute_fails_closed | DETECTED | - |
| PACK16-6 | crates/contract/src/slot.rs: 差し込んだ値を再び走査する（受講者の入力に混ぜた {{...}} が展開される） | pack16_substitute_does_not_rescan_inserted_values | pack16_substitute_does_not_rescan_inserted_values | DETECTED | - |
| PACK16-2 | crates/pack/src/slot_check.rs: 未知の枠の名前の検査を外す（閉じた集合にない名前が通る） | pack16_unknown_slot | pack16_unknown_slot, pack16_unknown_slot_name_is_not_echoed_unless_safe | DETECTED | - |
| PACK16-3 | crates/pack/src/slot_check.rs: 分類の制限の検査を外す（busy だけの枠が別の分類・問い返しで通る） | pack16_context_rejection_and_clarification | pack16_context_rejection_and_clarification | DETECTED | - |
| PACK16-4 | crates/pack/src/slot_check.rs: 空になりうる枠の代替の文の要求を外す（値が無いとき文面が欠ける） | pack16_no_fallback | pack12_blank_fallback_is_no_fallback_not_empty, pack16_no_fallback | DETECTED | - |
| PACK16-5 | crates/pack/src/slot_check.rs: step_do の使用に do を要求する検査を外す（do の無い Step で step_do の文面が出る。展開の前後の両方） | pack16_needs_do | pack12_dynamic_detects_after_expansion, pack16_needs_do_for_start_unmet, pack16_needs_do, pack16_needs_do_after_argument_substitution | DETECTED | - |
| PACK12-1 | crates/pack/src/slot_check.rs: 静的な次の行動の空の検査を外す（空の next_action・how_to_answer が通る） | pack12_static_empty_next_action | pack12_static_empty_next_action | DETECTED | - |
| PACK12-2 | crates/pack/src/slot_check.rs: 差し込みの後の空の数え上げを外す（展開の後に do が空になる Step が 0 件と数えられる） | pack12_dynamic_detects_after_expansion | pack12_dynamic_counts_unrenderable_text_as_empty, pack12_dynamic_detects_after_expansion | DETECTED | - |

### 5.24 確認前の達成 0 件（TASK-207 3/3）

受付で Step の状態を達成にする誤り（EVT3-1）と、受付の前の入力を確認の段に渡す誤り（EVT3-2）を入れ、確認前の達成の性質テスト（P-3）と固定列の試験が検出することを確かめる。
判定結果の発行（TASK-267）の後は、達成は判定が確認のときだけになる。そのため EVT3-2 は受付なしの達成にならず、受付が起きない誤りとして固定列の試験が検出する。確認前の達成 0 件の性質テストは EVT3-1 が検出する。

```text
実行日: 2026-10-15
元にしたコミット: 93cc1c2-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT3-2.mutation crates/core/tests/mutations/EVT3-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT3-1 | crates/core/src/accept.rs: 受付で Step の状態を達成にする（受付と達成の分離がつぶれ、状態とログが食い違う） | p3_no_achievement_before_confirmation, p3_fixed_sequence_confirms_all_three_kinds | p3_fixed_sequence_confirms_all_three_kinds, p3_missing_confirm_needs_approval_and_none_is_excluded, p3_no_achievement_before_confirmation | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([PointAt, PointAt, PointAt], 0, [Apply(Command("/make-note first-note"))]) |
| EVT3-2 | crates/core/src/accept.rs: 受付の前（入力待ち）の Step の入力を確認の段に渡す（受付が起きなくなる。達成は判定が確認のときだけなので、受付なしの達成にはならない） | p3_fixed_sequence_confirms_all_three_kinds, p3_missing_confirm_needs_approval_and_none_is_excluded | p3_missing_confirm_needs_approval_and_none_is_excluded, p3_fixed_sequence_confirms_all_three_kinds | DETECTED | - |

### 5.25 未知のコマンドと順序違いの分類（TASK-268 1/2）

`flow` の判定を外す誤り（JUDGE4-1）、表に無い名前を未知のコマンドにしない誤り（JUDGE4-2）、受け付けるかの判定から状態 `Entered` の条件を外す誤り（JUDGE4-3）を入れ、誤入力の組の 100% 試験・性質テスト・状態の境界の試験が検出することを確かめる。

```text
実行日: 2026-10-15
元にしたコミット: 11620e4-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/JUDGE4-1.mutation crates/core/tests/mutations/JUDGE4-2.mutation crates/core/tests/mutations/JUDGE4-3.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE4-1 | crates/core/src/command_class.rs: flow の判定を外す（flow のコマンドが順序違いにならず未分類に落ちる） | wrong_input_pairs_are_classified_100_percent, other_flow_name_is_wrong_order_for_any_status | flow_command_depends_on_status, accepted_status_ignores_broken_declaration, rejection_text_ids_follow_defaults, input_less_and_other_kind_steps_are_not_errors, other_flow_name_is_wrong_order_for_any_status, invalid_kind_values_fail_closed, wrong_input_pairs_are_classified_100_percent | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: NotStarted |
| JUDGE4-2 | crates/core/src/command_class.rs: 表に無い名前を未知のコマンドにしない（順序違いに倒す） | wrong_input_pairs_are_classified_100_percent, unknown_name_is_unknown_for_any_step_and_status | invalid_kind_values_fail_closed, name_is_case_sensitive, rejection_text_ids_follow_defaults, unknown_name_is_unknown_for_any_step_and_status, wrong_input_pairs_are_classified_100_percent | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ("0", 0, NotStarted) |
| JUDGE4-3 | crates/core/src/command_class.rs: 受け付けるかの判定から状態 Entered の条件を外す（受付済みの再送が受付になる） | flow_command_depends_on_status | accepted_status_ignores_broken_declaration, flow_command_depends_on_status | DETECTED | - |

### 5.26 確認を省く明示の Step の受付と同時の達成（TASK-208）

受付と同時の達成の呼び出しを外す誤り（EVT5-1）、省略の理由の有無の判定を外す誤り（EVT5-2）、明示の無い Step を省略の宣言として扱う誤り（EVT5-3）を入れ、`confirm_none_achieve` が検出することを確かめる。

```text
実行日: 2026-10-15
元にしたコミット: 55a053b-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT5-1.mutation crates/core/tests/mutations/EVT5-2.mutation crates/core/tests/mutations/EVT5-3.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT5-1 | crates/core/src/accept.rs: 受付と同時の達成の呼び出しを外す（省略を明示した Step が受付済みで止まる） | explicit_skip_achieves_on_accept_and_transitions, unknown_transition_target_warns_after_achieved | unknown_transition_target_warns_after_achieved, explicit_skip_achieves_on_accept_and_transitions | DETECTED | - |
| EVT5-2 | crates/core/src/confirm.rs: 省略の理由の有無の判定を外す（理由の無い省略が受付と同時の達成になる） | skip_without_reason_is_rejected_without_state_change | skip_without_reason_is_rejected_without_state_change | DETECTED | - |
| EVT5-3 | crates/core/src/confirm.rs: 明示の無い Step を省略の宣言として扱う（確認の操作の前に Step が止まらなくなる） | undeclared_steps_never_achieve_on_accept | undeclared_steps_never_achieve_on_accept | DETECTED | - |

### 5.27 受付の一回性と処理中の拒否（TASK-209 1/2）

処理中でも `busy` に分類しない誤り（EVT7-1）、受付で処理中にしない誤り（EVT7-2）、`Cancel` を `busy` に分類する誤り（EVT7-3）を入れ、`busy_resend` が検出することを確かめる。

```text
実行日: 2026-10-15
元にしたコミット: 70e32dc-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT7-1.mutation crates/core/tests/mutations/EVT7-2.mutation crates/core/tests/mutations/EVT7-3.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT7-1 | crates/core/src/busy_class.rs: 処理中でも busy に分類しない（処理中の入力が確認の判定へ流れる） | burst_of_20_accepts_once_and_rejects_19_as_busy, busy_applies_to_every_input_kind, p6_random_inputs_keep_invariants | busy_rejection_carries_progress, busy_applies_to_every_input_kind, burst_of_20_accepts_once_and_rejects_19_as_busy, p6_burst_accepts_once, p6_random_inputs_keep_invariants | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: 2; cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Apply(Command("/make-note first-note")), Apply(Command("/make-note first-note"))] |
| EVT7-2 | crates/core/src/accept.rs: 受付で processing を持つ Step を処理中にしない（run.busy を立てない） | burst_of_20_accepts_once_and_rejects_19_as_busy, p6_burst_accepts_once | burst_of_20_accepts_once_and_rejects_19_as_busy, busy_applies_to_every_input_kind, p6_burst_accepts_once | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: 2 |
| EVT7-3 | crates/core/src/busy_class.rs: Cancel を busy に分類する | cancel_is_never_busy, p6_random_inputs_keep_invariants | cancel_is_never_busy, p6_random_inputs_keep_invariants | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Apply(Command("/make-note first-note")), Apply(Cancel)] |

### 5.28 残り続けるボタンへの連打と世代の不変（TASK-209 2/2）

ログの記録を欠く誤り（EVT7-4）と、受付前の拒否を受付の経路へ流す誤り（EVT7-5）を入れ、`button_burst` が検出することを確かめる。

```text
実行日: 2026-10-16
元にしたコミット: 3b86f2f-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT7-4.mutation crates/core/tests/mutations/EVT7-5.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT7-4 | crates/core/src/session.rs: 1 回の操作のイベントがログへすべて記録されない（連打 20 回でログが 40 件にならない） | burst_of_20_on_remaining_button_rejects_all_and_keeps_state, p6_remaining_button_burst_keeps_state | burst_of_20_on_remaining_button_rejects_all_and_keeps_state, p6_remaining_button_burst_keeps_state | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: 1 |
| EVT7-5 | crates/core/src/accept.rs: 受付前の拒否が受付の経路へ流れる（規則による拒否の Judged が出ない） | burst_of_20_on_remaining_button_rejects_all_and_keeps_state | burst_of_20_on_remaining_button_rejects_all_and_keeps_state, p6_remaining_button_burst_keeps_state | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: 1 |

### 5.29 引数不足と引数の形の拒否と 4 場面の教材テスト（TASK-268 2/2）

必須引数の不足を引数の形の違いに化けさせる誤り（JUDGE4-4）、引数の形の検査を外す誤り（JUDGE4-5）、判定結果の統合で引数不足を別の分類に写す誤り（JUDGE4-6）を入れ、教材パックの誤入力で 4 場面を数える結合試験 `reject_four_scenes`（pack crate）が検出することを確かめる。

```text
実行日: 2026-10-16
元にしたコミット: 3b86f2f-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/JUDGE4-4.mutation crates/pack/tests/mutations/JUDGE4-5.mutation crates/pack/tests/mutations/JUDGE4-6.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE4-4 | crates/core/src/command_judge.rs: 必須引数の不足を引数の形の違いに化けさせる（missing_arg が bad_arg_shape で返る） | four_scenes_are_classified_100_percent, missing_and_shape_go_through_judge_input | rejection_text_prefers_step_on_reject, missing_arg_is_not_clarified, four_scenes_are_classified_100_percent, missing_and_shape_go_through_judge_input | DETECTED | - |
| JUDGE4-5 | crates/core/src/command_judge.rs: 引数の形の検査を外す（形の違う値が受付になり、bad_arg_shape が返らない） | four_scenes_are_classified_100_percent, missing_and_shape_go_through_judge_input | missing_and_shape_go_through_judge_input, four_scenes_are_classified_100_percent, rejection_text_prefers_step_on_reject | DETECTED | - |
| JUDGE4-6 | crates/core/src/judge.rs: 判定結果の統合で引数不足を別の分類に写す（結線の誤り） | four_scenes_are_classified_100_percent, missing_and_shape_go_through_judge_input, rejection_text_prefers_step_on_reject | missing_arg_is_not_clarified, missing_and_shape_go_through_judge_input, four_scenes_are_classified_100_percent, rejection_text_prefers_step_on_reject | DETECTED | - |

### 5.30 世代の規則の強制（TASK-210 1/3）

世代を変えない操作でも世代を動かす誤り（EVT6-1）、関門を通さず常に世代を増やす誤り（EVT6-2）を入れ、`generation_keep` が検出することを確かめる。

```text
実行日: 2026-10-16
元にしたコミット: 3b86f2f-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT6-1.mutation crates/core/tests/mutations/EVT6-2.mutation

```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT6-1 | crates/core/src/session.rs: 世代を変えない操作でも世代を 1 つ増やす（関門が Keep の操作の世代を進める） | keep_on_accept, keep_on_refuse, keep_on_confirm_achieve, keep_on_resend_burst, keep_on_cancel, keep_on_tick_any_addr, keep_on_save, p2_keep_ops_never_change_generation | keep_on_tick_any_addr, keep_on_resend_burst, keep_on_confirm_achieve, keep_on_refuse, keep_on_accept, keep_on_save, keep_on_cancel, p2_keep_ops_never_change_generation | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Input(Command("/make-note first-note"))] |
| EVT6-2 | crates/core/src/session.rs: 関門を通さず、どの操作の後も世代を 1 つ増やす（Keep の操作で世代が動く） | keep_on_accept, keep_on_refuse, keep_on_confirm_achieve, keep_on_save, p2_keep_ops_never_change_generation | keep_on_refuse, keep_on_cancel, keep_on_confirm_achieve, keep_on_save, keep_on_resend_burst, keep_on_accept, keep_on_tick_any_addr, p2_keep_ops_never_change_generation | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Input(Command("/make-note first-note"))] |

### 5.31 拒否の場面の判定（TASK-269）

教材未対応を `not_in_this_step` に倒す誤り（JUDGE5-1）、処理中の判定を外す誤り（JUDGE5-2）、`builtin`・`skill` を `wrong_order` に倒す誤り（JUDGE5-3）を入れ、`reject_scene` の誤入力の組の 100% 試験・処理中の試験・性質テストが検出することを確かめる。

```text
実行日: 2026-10-16
元にしたコミット: 3b86f2f-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/JUDGE5-1.mutation crates/core/tests/mutations/JUDGE5-2.mutation crates/core/tests/mutations/JUDGE5-3.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE5-1 | crates/core/src/command_class.rs: unsupported を not_in_this_step に倒す（教材未対応が見分けられない） | playthrough_pairs_are_classified_100_percent, not_supported_in_every_status_even_when_the_step_declares_it | not_supported_in_every_status_even_when_the_step_declares_it, other_ir_files_classify_the_three_scenes, property_classification_matches_the_vocabulary_rules, playthrough_pairs_are_classified_100_percent | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (0, NotStarted, false, "sync-cloud") |
| JUDGE5-2 | crates/core/src/judge.rs: 処理中の判定を外す（処理中に別の入力が分類や受付に流れる） | any_other_input_while_processing_is_busy, busy_wins_over_command_classes | busy_wins_over_command_classes, any_other_input_while_processing_is_busy, property_classification_matches_the_vocabulary_rules, playthrough_pairs_are_classified_100_percent | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (0, NotStarted, true, "help") |
| JUDGE5-3 | crates/core/src/command_class.rs: builtin・skill を wrong_order に倒す（この Step では使わない入力が順序違いになる） | playthrough_pairs_are_classified_100_percent, builtin_and_skill_are_not_in_this_step_when_not_accepted | builtin_and_skill_are_not_in_this_step_when_not_accepted, property_classification_matches_the_vocabulary_rules, playthrough_pairs_are_classified_100_percent | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (0, NotStarted, false, "help") |

### 5.32 受講者の操作の記録（TASK-315 1/2）

下書きの畳みで置き換えない誤り（SAVE24-1）、送信の本文を trim して記録する誤り（SAVE24-2）、`apply_recorded` が記録しない誤り（SAVE24-3）、エンジンの操作を先頭に挿入して並びを変える誤り（SAVE24-4）を入れ、`ops_record` の試験が検出することを確かめる。

### 5.33 glue の許される操作と項目数の方針（TASK-343 3/3）

FEND20-1 は glue に要素の中身の書き換え（構造の変更）を足し、FEND15-1 は keyed list の項目数の目安を安全弁より大きくし、FEND21-1 は Tab の `preventDefault` の条件を「かつ」から「または」にする。
いずれも契約適合試験（`glue_contract`・`list_policy_contract`）が検出する。TASK-81 の G1・G2 は、`state_paths` が共有ヘルパ（`tests/common`）を使う形に変えた後も検出することを再確認した。

```text
実行日: 2026-10-16
元にしたコミット: eeb1a73-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/FEND20-1.mutation crates/web/tests/mutations/FEND15-1.mutation crates/web/tests/mutations/FEND21-1.mutation crates/web/tests/mutations/G1.mutation crates/web/tests/mutations/G2.mutation crates/core/tests/mutations/SAVE24-1.mutation crates/core/tests/mutations/SAVE24-2.mutation crates/core/tests/mutations/SAVE24-3.mutation crates/core/tests/mutations/SAVE24-4.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| FEND20-1 | crates/web/src/glue.rs: glue が要素の中身を書き換える（許される操作の一覧に無い構造の変更が入っても気づけなくなる） | 構造を変える操作は_0_件, 実ソースは違反なし | 構造を変える操作は_0_件, 実ソースは違反なし | DETECTED | - |
| FEND15-1 | crates/web/src/list_policy.rs: 項目数の目安を安全弁より大きくする（分割の基準が効かなくなる） | 目安と安全弁の値, 項目数の区分の境界 | 目安と安全弁の値, 項目数の区分の境界 | DETECTED | - |
| FEND21-1 | crates/web/src/glue_ops.rs: Tab の preventDefault の条件を「かつ」から「または」にする（候補が無いときの Tab 移動を奪う） | tab_は候補が開いていて文字があるときだけ_prevent_default | tab_は候補が開いていて文字があるときだけ_prevent_default | DETECTED | - |
| G1 | crates/web/src/glue.rs: App に触れる口を 2 箇所にする（glue が App を読む経路が増えても気づけなくなる） | app_に触れるのは_with_app_の_component_の1箇所だけ | 実ソースは違反なし, app_に触れるのは_with_app_の_component_の1箇所だけ | DETECTED | - |
| G2 | crates/web/src/glue.rs: glue から状態を変える呼び出しを足す（描画の結果から状態へ流す経路が黙って入る） | glue_は状態を変える呼び出しを持たない | dom_の値の読み取りは_sync_draft_の比較だけ, 実ソースは違反なし, glue_は状態を変える呼び出しを持たない | DETECTED | - |
| SAVE24-1 | crates/core/src/ops_record.rs: 連続する下書きを畳むとき最後の値で置き換えない（最初の値が残る） | consecutive_drafts_fold_to_last, whitespace_only_submit_is_not_recorded, property_ops_follow_call_order | consecutive_drafts_fold_to_last, whitespace_only_submit_is_not_recorded, property_ops_follow_call_order | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Draft(""), Draft("")] |
| SAVE24-2 | crates/core/src/ops_record.rs: 送信の本文を trim して記録する（SAVE-24 の受け取った値のまま残す規則に反する） | submit_text_is_not_trimmed_or_classified, property_ops_follow_call_order | submit_text_is_not_trimmed_or_classified, property_ops_follow_call_order | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Submit("a\u{3000}")] |
| SAVE24-3 | crates/core/src/ops_record.rs: apply_recorded が操作を記録しない（拒否・無視も残す規則に反する） | refused_and_ignored_engine_ops_are_recorded, apply_recorded_matches_apply | refused_and_ignored_engine_ops_are_recorded, apply_recorded_matches_apply | DETECTED | - |
| SAVE24-4 | crates/core/src/ops_record.rs: エンジンの操作を先頭に挿入する（PoC-6 の X-6 に当たる並びの誤りの記録側） | property_ops_follow_call_order, refused_and_ignored_engine_ops_are_recorded | consecutive_drafts_fold_to_last, property_ops_follow_call_order, refused_and_ignored_engine_ops_are_recorded | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Engine(0), Engine(0)] |

### 5.34 宛先つきの進行（TASK-210 2/3）

宛先の照合を見ない誤り（EVT6-3）、進行の番号を進めない誤り（EVT6-4）、最後の進行で処理中を解かない誤り（EVT6-5）、`Progress` の番号が 1 ずれる誤り（EVT6-6）を入れ、`tick_progress` が検出することを確かめる。

```text
実行日: 2026-10-16
元にしたコミット: 8856df8-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT6-3.mutation crates/core/tests/mutations/EVT6-4.mutation crates/core/tests/mutations/EVT6-5.mutation crates/core/tests/mutations/EVT6-6.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT6-3 | crates/core/src/tick.rs: 宛先の照合結果を見ずに常に進める（食い違う宛先の進行が効く） | mismatched_tick_changes_nothing, duplicate_index_advances_once, p2_mismatched_tick_keeps_run | mismatched_tick_changes_nothing, duplicate_index_advances_once, p2_mismatched_tick_keeps_run | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Apply(Command("/make-note first-note")), Any(0, 0, false)] |
| EVT6-4 | crates/core/src/tick.rs: 進行の番号を進めない（同じ進行が繰り返し効く） | fixed_sequence_matches_sample_shape, matching_tick_advances_once, duplicate_index_advances_once | fixed_sequence_matches_sample_shape, matching_tick_advances_once, duplicate_index_advances_once | DETECTED | - |
| EVT6-5 | crates/core/src/tick.rs: 最後の進行で処理中を解かない | fixed_sequence_matches_sample_shape | fixed_sequence_matches_sample_shape | DETECTED | - |
| EVT6-6 | crates/core/src/tick.rs: Progress の番号に進めた後の値を渡す（0 始まりが 1 ずれる） | fixed_sequence_matches_sample_shape | fixed_sequence_matches_sample_shape | DETECTED | - |

### 5.35 履歴の包みの組み立てと下書きの同期（TASK-315 2/2）

組み立てでログの先頭の 2 件を入れ替える誤り（SAVE24-5）、`Restored.log_len` を 1 ずらす誤り（SAVE24-6。PoC-6 の X-6）、チェックポイントの持ち主を落とす誤り（SAVE24-7）、送信後に根の下書きを除かない誤り（SAVE24-8）、下書きを写しに書かない誤り（SAVE24-9）を入れ、`history_envelope_build` の試験が検出することを確かめる。

```text
実行日: 2026-10-16
元にしたコミット: 8856df8-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAVE24-5.mutation crates/core/tests/mutations/SAVE24-6.mutation crates/core/tests/mutations/SAVE24-7.mutation crates/core/tests/mutations/SAVE24-8.mutation crates/core/tests/mutations/SAVE24-9.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| SAVE24-5 | crates/core/src/history_envelope.rs: 包む直前にログの先頭の 2 件を入れ替える（SAVE-2、PoC-6 の X-6 に当たるログの並びの誤りの組み立て側） | samples_are_rebuilt_byte_for_byte, sample_rebuild_roundtrips_and_keeps_positions | samples_are_rebuilt_byte_for_byte, sample_rebuild_roundtrips_and_keeps_positions | DETECTED | - |
| SAVE24-6 | crates/core/src/history_envelope.rs: 包む直前に Restored.log_len を 1 ずらす（SAVE-2、PoC-6 の X-6 の定義どおり） | samples_are_rebuilt_byte_for_byte, sample_rebuild_roundtrips_and_keeps_positions | samples_are_rebuilt_byte_for_byte, sample_rebuild_roundtrips_and_keeps_positions | DETECTED | - |
| SAVE24-7 | crates/core/src/history_envelope.rs: 包む直前にチェックポイントの持ち主の根を落とす（SAVE-2） | samples_are_rebuilt_byte_for_byte, checkpoint_owner_is_kept | checkpoint_owner_is_kept, samples_are_rebuilt_byte_for_byte, sample_rebuild_roundtrips_and_keeps_positions | DETECTED | - |
| SAVE24-8 | crates/core/src/history_envelope.rs: 送信の記録の後に現在の根の下書きを写しから除かない（MOD-4・SAVE-18） | submit_clears_only_the_current_root_draft, property_ops_and_drafts_stay_in_sync | submit_clears_only_the_current_root_draft, property_ops_and_drafts_stay_in_sync | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Draft(""), Submit("a")] |
| SAVE24-9 | crates/core/src/history_envelope.rs: 下書きを ops に記録しても写しの drafts に書かない（SAVE-18・SAVE-24） | save24_example_shape_is_in_the_envelope, empty_draft_is_kept_as_empty_string, property_ops_and_drafts_stay_in_sync | empty_draft_is_kept_as_empty_string, submit_clears_only_the_current_root_draft, save24_example_shape_is_in_the_envelope, whitespace_submit_changes_nothing, property_ops_and_drafts_stay_in_sync | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Draft("")] |

## 6. 既存の自動テストの件数の基準値（TASK-346）

採用した版（FEND-1・D-30）でのビルド・lint の確認と、既存の自動テストの件数を記録する。
本リポジトリには「版を上げる前」の値が無い。この値は、今後の版上げ（FEND-17・TASK-101）で比べる基準値である。
PoC-1 の追試の 214 件は上流のリポジトリの値で、数える対象が違うため比べない。

```text
計測日: 2026-10-14
元にしたコミット: f469e9f-dirty（本変更を含む作業ツリー）
```

### 6.1 採用した版（Cargo.lock の解決済みの版）

| crate | 版 |
|---|---|
| `fandhe-frontend-core` | 0.4.3 |
| `fandhe-frontend-interactive` | 0.2.7 |
| `fandhe-frontend-wasm-client` | 0.6.1 |
| `fandhe-frontend-wasm-full` | 0.46.0（`default-features = false`・`features = ["action-keydown"]`） |
| `fandhe-frontend-headless-ui` | 0.69.2（推移的な依存） |

版は `crates/web/tests/frontend_features.rs` が `Cargo.lock` と照合する。版を上げるときは、この表と試験の `ADOPTED` を同時に更新する。

### 6.2 ビルドと lint

| 確認 | コマンド | 結果 |
|---|---|---|
| native のビルド | `cargo build --locked --workspace` | 成功 |
| wasm32 の release（配信設定） | `SKIP_WASM_OPT=1 tools/build-web.sh` | 成功 |
| wasm32 の release + `test-hooks` | `TEST_HOOKS=1 SKIP_WASM_OPT=1 tools/build-web.sh` | 成功 |
| fmt・clippy（`-D warnings`。native と wasm32 の feature なし・`test-hooks`） | `make lint` | 成功 |

### 6.3 Rust の native の試験（`cargo test --locked --workspace`）

| 試験のバイナリ | passed | failed | ignored |
|---|---|---|---|
| contract（unittests） | 0 | 0 | 0 |
| contract（limits_contract） | 8 | 0 | 0 |
| core（unittests） | 0 | 0 | 0 |
| core（property_sample） | 6 | 0 | 0 |
| core（provider_contract） | 4 | 0 | 0 |
| pack（unittests lib） | 12 | 0 | 0 |
| pack（unittests bin） | 0 | 0 | 0 |
| pack（cli） | 3 | 0 | 0 |
| pack（testdata_min_3step） | 3 | 0 | 0 |
| web（unittests） | 3 | 0 | 0 |
| web（frontend_features） | 3 | 0 | 0 |
| doctest（contract・core・pack・web） | 0 | 0 | 0 |
| 合計 | 42 | 0 | 0 |

`web（unittests）` の 3 件のうち 1 件（配信設定の描画に probe が入らないこと）と `frontend_features` の 3 件は、TASK-346 で足した。
`test-hooks` を付けた native の試験（`cargo test -p fandhe-workshop-web --features test-hooks`）は web の unittests が 3 件、`frontend_features` が 3 件である。

### 6.4 node:test の件数

| 対象 | tests | pass | fail | skipped |
|---|---|---|---|---|
| `tools/serve.test.mjs`（`make test-serve`） | 18 | 18 | 0 | 0 |
| `tools/measure/browser.test.mjs` | 11 | 11 | 0 | 0 |
| `tools/browser/*.test.mjs`（`make test-browser`） | 11 | 11 | 0 | 0 |

`make test-browser` の 11 件のうち `keydown-action.test.mjs` の 1 件は TASK-346 で足した。
`make test-keydown-control` は、feature を外した写しで `keydown-action.test.mjs` が `ACTION-KEYDOWN-NOT-DISPATCHED` で失敗することを確かめ、成立した（2026-10-14）。

### 6.5 未計測のもの

- CI での実行結果。wasm32 のビルドとブラウザの試験は CI に組み込まれていない（上表はローカルの計測）
- `tools/measure/` の shell を伴う試験（`make test-measure`・`make test-measure-wasm-size`）の件数
