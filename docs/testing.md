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

1. `mktemp -d` の一時ディレクトリに `Cargo.toml`・`Cargo.lock`・`rust-toolchain.toml`・`crates/` を写す
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
| `id` | 必須 | 変異の ID。サンプルは `S1`・`S2`。TASK-220 は `M1` 以降、TASK-244 は `X1` 以降を使う |
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

### 5.3 判定の順序（TASK-157）

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
