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
- 操作列の生成器とハーネスは `crates/core/tests/common/op_gen.rs`、流し直しの一致の試験（TASK-219 1/3、性質 P-5）は
  `crates/core/tests/op_sequence_replay.rs` にある。1,000 通りの実測は dev プロファイルで約 3 秒（2026-10-16、`cargo test` の試験の所要時間）
- 各操作の後の性質 P-1a〜P-1d・P-2・P-3・P-4・P-6（TASK-219 2/3）は `crates/core/tests/op_sequence_properties.rs`、検査器は
  `crates/core/tests/common/op_props.rs` にある。1,000 通りの実測は dev プロファイルで約 2 秒（2026-10-16、`cargo test` の試験の所要時間）
- 誘導つきの生成（TASK-219 3/3）は `op_gen.rs` の `guided_op_specs`（操作の 40% を `OpSpec::Guided`＝現在の状態での正しい次の操作に
  する。固定の生成では深い状態に届く列が少ない、という PoC-1 の発見事項 F-11 への対応）。`op_sequence_properties.rs` の
  `properties_p1_to_p6_hold_with_guided_sequences` と `op_sequence_replay.rs` の
  `replay_from_start_matches_after_every_op_with_guided_sequences` が、固定の生成と同じく 1,000 通り流す。誘導つきの 1,000 通りの実測は
  dev プロファイルで約 2〜3 秒（2026-10-16）
- 適用し直しによる復元の一致（TASK-316 1/3、SAVE-1）は `crates/core/tests/history_replay.rs` にある。下書きを挟んだ記録の経路で得た写しと、
  その `ops` を初期状態に適用し直した状態が一致することを、固定・誘導つきの生成で各 1,000 通り確かめる。2 本あわせた実測は
  dev プロファイルで約 3 秒（2026-10-16、`cargo test` の試験の所要時間）
- 写しとの照合（TASK-316 2/3、SAVE-1）は `crates/core/tests/history_verify.rs` にある。実在の読み込みの口（`load_gate::load_json`）を通し、
  「書き出し→読み込みの一致」と「任意の時点の書き出し→読み込みの後の一致」（時点 t の包みを読み込んで得た状態に残りの `ops` を流した結果が、
  最後まで流した結果と一致する。ケースごとに t を 1 つ引く）を、固定・誘導つきの生成で各 1,000 通り確かめる。再読み込みの意味（世代 +1・`Reloaded`）は
  含めず、TASK-320 で格上げする。12 本あわせた実測は dev プロファイルで約 4.4 秒（2026-10-16、`cargo test` の試験の所要時間）
- 復元の性質と変異 6/6（TASK-316 3/3、SAVE-1）は、同じ 2 つの試験ファイルに足した。各枠の下書きは 0〜2 件で引き、送信を挟まず連続する
  下書きを列の途中にも出す。試作の H-2（写しから作り直した実行が写しを返す）・H-7（同じ包みの書き出しが同じバイト列）を書き出し→読み込みの
  性質テストの中で確かめ、H-8（`ops` を 1 件消した包みは、開けるなら写しと一致し、開けないなら `replay_mismatch`）を固定・誘導つきで各 1,000 通り
  確かめる。H-6（再読み込みで世代が上がる）は TASK-320 の範囲。14 本あわせた実測は dev プロファイルで約 4.8 秒（2026-10-16、
  `cargo test` の試験の所要時間）。変異は 5.59 節。自動ブラウザでの「`ops` だけで復元した最終の世界状態の一致」は、web の保存・読み込みの結線が
  未実装のため未確認
- `guided_generation_reaches_deeper_states_than_fixed_generation` は、既定の種に固定して固定・誘導の深さを比べる網羅性の試験
  （環境変数を受けない）。実測（2026-10-16、1,000 通り）は、確認による達成が固定 29 件・誘導 1,075 件、最後の Step（`main/st-3`）が
  達成まで届いた列が固定 28 通り・誘導 509 通り、達成の直後に同じ操作で次の Step へ入った件数が固定 150 件・誘導 1,083 件。章の完了
  （`ChapterCompleted`）はエンジンが未実装（TASK-233 以降）のため、代わりに最後の Step の達成を指標にした。下限は
  確認による達成 500 件超・最後の Step の達成 200 通り超

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
| `id` | 必須 | 変異の ID。サンプルは `S1`・`S2`。TASK-220 は `M1` 以降、TASK-244 は `X1` 以降、TASK-158 は `L1`、TASK-81 は `G1`・`G2`、TASK-180 は `H1`、TASK-290 は `GUIDE17-1`、TASK-256 は `J1`、TASK-291 は `GUIDE18-1`、TASK-309 は `SAVE2-1` 以降、TASK-52 は `PACK20-1` 以降、TASK-47 は `PACK13-1` 以降・`PACK23-1`、TASK-262 は `JUDGE1-1` 以降、TASK-215 は `EVT13-1` 以降、TASK-263 は `JUDGE1-C1`・`JUDGE1-U1`・`JUDGE1-A1`・`JUDGE1-Q1`、TASK-266 は `JUDGE2-1` 以降、TASK-45 は `PACK11-1` 以降、TASK-46 は `PACK15-1` 以降・`PACK16-1` 以降・`PACK12-1` 以降、TASK-272 は `JUDGE6-1` 以降、TASK-305 は `GUIDE18-2` 以降、TASK-207 は `EVT3-1` 以降、TASK-268 は `JUDGE4-1` 以降、TASK-208 は `EVT5-1` 以降、TASK-209 は `EVT7-1` 以降、TASK-269 は `JUDGE5-1` 以降、TASK-315 は `SAVE24-1` 以降、TASK-210 は `EVT6-1` 以降、TASK-310 は `SAVE14-1`・`SAVE15-1` 以降・`SAVE19-1` 以降、TASK-343 は `FEND20-1`・`FEND15-1`・`FEND21-1`、TASK-359 は `FEND15-2`・`FEND15-3`、TASK-274 は `JUDGE8-1` 以降、TASK-257 は `JUDGE14-1`・`JUDGE17-1`・`JUDGE19-1`、TASK-211 は `EVT9-1` 以降、TASK-213 は `EVT11-1` 以降、TASK-212 は `EVT10-1` 以降、TASK-214 は `EVT12-1` 以降、TASK-229 は `EVT18-1`、TASK-275 は `JUDGE13-1`、TASK-278 は `JUDGE19-2`・`JUDGE19-3`、TASK-277 は `JUDGE17-2`〜`JUDGE17-4`、TASK-282 は `JUDGE18-2`・`JUDGE21-2`、TASK-316 は `SAVE1-1` 以降、TASK-467 は `ART3-1`・`ART2-1`・`ART5-1`・`ART11-1`・`ART7-1`、TASK-174 は `VER3-1`・`VER4-1`・`VER4-2`・`VER6-1`・`VER7-1`、TASK-174 2/3 は `VER6-2`・`VER6-3`・`VER7-2`〜`VER7-4`、TASK-174 3/3 は `VER7-5`〜`VER7-10`、TASK-349 は `FEND4-1`〜`FEND4-3`、TASK-472 は `ART9-1`・`ART9-2`・`ART13-1` を使う |
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

### 5.60 keyed list の親要素の属性（TASK-359 1/2）

FEND15-2 はログの親 `ul` の属性を `view()` から外し、FEND15-3 は `keyed_view` を迂回して上流の `keyed_list` を直接呼ぶ。
前者は親の属性列の試験、後者は `src/` で `keyed_list` を呼ぶのが `list_policy.rs` だけであることの静的検査が、どちらも `list_parent_attrs` で検出する。

```text
実行日: 2026-10-16
元にしたコミット: ed3a165-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/FEND15-2.mutation crates/web/tests/mutations/FEND15-3.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| FEND15-2 | crates/web/src/engine_parts.rs: ログの親 ul の属性を view() から外す（更新のたびに同期されて属性が消える） | 親の属性列は_view_に書かれ_操作の前後で変わらない | 親の属性列は_view_に書かれ_操作の前後で変わらない | DETECTED | - |
| FEND15-3 | crates/web/src/engine_parts.rs: keyed_view を迂回して上流の keyed_list を直接呼ぶ | 上流の_keyed_list_を呼ぶのは_list_policy_だけである | 上流の_keyed_list_を呼ぶのは_list_policy_だけである | DETECTED | - |

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

### 5.36 保存先の鍵の生成規則（TASK-310 1/3）

SAVE14-1 は鍵に教材の版を混ぜ、SAVE15-1 は配信の区間を無視して常に根の印にする。教材の版・内容に依らず鍵が不変であること（SAVE-14）と、
配信先ごとに鍵が分かれること（D-56）を、契約適合試験 `storage_contract` が検出する。

```text
実行日: 2026-10-16
元にしたコミット: 4e2337a
コマンド: scripts/check-mutations.sh crates/contract/tests/mutations/SAVE14-1.mutation crates/contract/tests/mutations/SAVE15-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| SAVE14-1 | crates/contract/src/storage.rs: 鍵に教材の版を混ぜる（版が変わると履歴の鍵が変わり、保存した履歴が読めなくなる） | key_ignores_pack_version_and_content | key_ignores_pack_version_and_content, key_uses_first_start_only | DETECTED | - |
| SAVE15-1 | crates/contract/src/storage.rs: 配信の区間を無視して常に根の印にする（別の配信先の履歴が同じ鍵になる） | key_differs_by_delivery_scope | pack_named_unreadable_does_not_collide, pack_prefix_does_not_reach_other_scope_or_pack, key_differs_by_delivery_scope, key_uses_first_start_only, prototype_ids_render_verbatim, render_parse_roundtrip_and_injective, unreadable_key_form, store_port_roundtrip | DETECTED | - |

### 5.37 食い違う進行の無視の記録（TASK-210 3/3）

宛先の照合結果を見ない誤り（EVT6-3）、無視を記録しない誤り（EVT6-7）、2 件記録する誤り（EVT6-8）、記録の世代を操作前の状態とずらす誤り（EVT6-9）を入れ、`tick_progress` が検出することを確かめる。EVT6-3 は照合行の変更に合わせて `find` と期待する試験を更新した。EVT6-4〜6 は同じ実行で引き続き検出された。

```text
実行日: 2026-10-16
元にしたコミット: 7f9b271-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT6-3.mutation crates/core/tests/mutations/EVT6-7.mutation crates/core/tests/mutations/EVT6-8.mutation crates/core/tests/mutations/EVT6-9.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT6-3 | crates/core/src/tick.rs: 宛先の照合結果を見ずに常に進める（食い違う宛先の進行が効く） | mismatched_tick_records_one_ignored, duplicate_index_advances_once, p2_mismatched_tick_keeps_run | mismatch_is_checked_before_processing_declaration, mismatched_tick_records_one_ignored, duplicate_index_advances_once, duplicate_of_last_tick_is_no_processing, p2_mismatched_tick_keeps_run | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Any(0, 0, false)] |
| EVT6-7 | crates/core/src/tick.rs: 食い違う進行を無視として記録しない（イベントが 0 件になる） | mismatched_tick_records_one_ignored, duplicate_index_advances_once, p2_mismatched_tick_keeps_run | mismatch_is_checked_before_processing_declaration, duplicate_of_last_tick_is_no_processing, mismatched_tick_records_one_ignored, duplicate_index_advances_once, p2_mismatched_tick_keeps_run | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Any(0, 0, false)] |
| EVT6-8 | crates/core/src/tick.rs: 無視を 2 件記録する（「1 件だけ」の違反） | mismatched_tick_records_one_ignored, duplicate_index_advances_once, p2_mismatched_tick_keeps_run | duplicate_index_advances_once, mismatch_is_checked_before_processing_declaration, duplicate_of_last_tick_is_no_processing, mismatched_tick_records_one_ignored, p2_mismatched_tick_keeps_run | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Any(0, 0, false)] |
| EVT6-9 | crates/core/src/tick.rs: 無視の記録の世代を操作前の状態の世代と食い違わせる | mismatched_tick_records_one_ignored | mismatched_tick_records_one_ignored | DETECTED | - |

### 5.38 拒否・解説への値の差し込み（TASK-274）

JUDGE8-1 は次の行動の差し込みを外し、JUDGE8-2 は近い候補から `unsupported` の除外を外し、JUDGE8-3 は近い候補の距離の上限を 3 にし、
JUDGE8-4 は残りの回数に終わった回数を入れ、JUDGE8-5 は `do` の無い Step の `step_do` に題名を入れる。文面の確認 38 件（群 A は 19 件）と、
`do` を持たない Step の試験 `judge8_step_do_is_not_replaced_by_title` が、それぞれ検出する。試験は pack の結合試験 `text_fill` に置く。

```text
実行日: 2026-10-16
元にしたコミット: 7f9b271-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/JUDGE8-1.mutation crates/pack/tests/mutations/JUDGE8-2.mutation crates/pack/tests/mutations/JUDGE8-3.mutation crates/pack/tests/mutations/JUDGE8-4.mutation crates/pack/tests/mutations/JUDGE8-5.mutation
```

| ID | 対象 | 期待した失敗 | 結果 |
|---|---|---|---|
| JUDGE8-1 | crates/core/src/judge.rs: 次の行動に差し込みをせず、教材の生の値のまま返す | judge8_text_checks_all_38, judge8_text_checks_group_a_19 | DETECTED |
| JUDGE8-2 | crates/core/src/text_fill.rs: 近い候補の選び方から `unsupported` の除外を外す | judge8_text_checks_all_38, judge8_text_checks_group_a_19 | DETECTED |
| JUDGE8-3 | crates/core/src/text_fill.rs: 近い候補の距離の上限を 3 にする | judge8_text_checks_all_38, judge8_text_checks_group_a_19 | DETECTED |
| JUDGE8-4 | crates/core/src/text_fill.rs: 残りの回数に終わった回数を入れる | judge8_text_checks_all_38, judge8_text_checks_group_a_19 | DETECTED |
| JUDGE8-5 | crates/core/src/text_fill.rs: `do` の無い Step で `step_do` に題名を入れる | judge8_step_do_is_not_replaced_by_title | DETECTED |

### 5.39 キー判定表 decide（TASK-257）

Esc の処理中の優先を外す誤り（JUDGE17-1）、Tab の補完の条件から `has_text` を外す誤り（JUDGE19-1）、変換中の判定から自前の印を外す誤り（JUDGE14-1）を入れ、契約適合試験 `key_decide_contract` が検出することを確かめる。

```text
実行日: 2026-10-16
元にしたコミット: 44dcfe7-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/contract/tests/mutations/JUDGE14-1.mutation crates/contract/tests/mutations/JUDGE17-1.mutation crates/contract/tests/mutations/JUDGE19-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE14-1 | crates/contract/src/key_decide.rs: 変換中の判定から自前の印を外す（印だけが立つ変換中に Enter が送信になる） | judge14_any_single_signal_means_composing, expected_table_matches | judge14_any_single_signal_means_composing, expected_table_matches | DETECTED | - |
| JUDGE17-1 | crates/contract/src/key_decide.rs: Esc の処理中の優先を外す（処理中でも停止せず、候補を閉じるか入力を取り消してしまう） | judge17_three_ways_with_busy_priority, expected_table_matches | expected_table_matches, judge17_three_ways_with_busy_priority | DETECTED | - |
| JUDGE19-1 | crates/contract/src/key_decide.rs: Tab の補完の条件から has_text を外す（文字が無いのに Tab を奪い、フォーカス移動できなくなる） | judge19_prevent_default_on_tab_only_when_open_and_text, expected_table_matches | expected_table_matches, judge19_prevent_default_on_tab_only_when_open_and_text | DETECTED | - |

### 5.39 処理中の停止と入口の写しへの復帰（TASK-211 1/2）

世界状態を写しへ戻さない誤り（EVT9-1）、世代を +1 しない誤り（EVT9-2）、`Stopped.at` に操作後の世代を使う誤り（EVT9-3）、写しの Step と現在の Step の照合を外す誤り（EVT9-4）を入れ、`stop_restore` が検出することを確かめる。

```text
実行日: 2026-10-16
元にしたコミット: 86c1a69-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT9-1.mutation crates/core/tests/mutations/EVT9-2.mutation crates/core/tests/mutations/EVT9-3.mutation crates/core/tests/mutations/EVT9-4.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT9-1 | crates/core/src/stop.rs: 世界状態を入口の写しへ戻さない（受付後の変化が残る） | stop_after_two_ticks_removes_draft, stop_matches_sample_e06, p4_stop_returns_to_entry | stop_after_two_ticks_removes_draft, stop_matches_sample_e06, p4_stop_returns_to_entry | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: Plan { ticks: 0, resends: 0 } |
| EVT9-2 | crates/core/src/stop.rs: 世代を +1 せず写しの世代のままにする（関門が戻すため状態の世代は据え置きになる） | stop_after_two_ticks_removes_draft, stop_matches_sample_e06, p4_stop_returns_to_entry | stop_after_two_ticks_removes_draft, stop_matches_sample_e06, p4_stop_returns_to_entry | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: Plan { ticks: 0, resends: 0 } |
| EVT9-3 | crates/core/src/stop.rs: Stopped.at に操作後の世代を使う | stop_after_two_ticks_removes_draft, stop_matches_sample_e06 | stop_after_two_ticks_removes_draft, stop_matches_sample_e06 | DETECTED | - |
| EVT9-4 | crates/core/src/stop.rs: 写しの Step と現在の Step の照合を外す（別の Step の写しへ戻る） | stop_with_other_step_entry_warns_only | stop_with_other_step_entry_warns_only | DETECTED | - |

### 5.40 停止前の進行の無視と停止後の入力（TASK-211 2/2）

進行の宛先の照合から世代を外す誤り（EVT9-5）を入れ、停止前の世代の進行が再入力後の処理中に効いてしまうことを `stop_restore` が検出することを確かめる。EVT9-1〜4 は、`stop_matches_sample_e06` を e08 まで広げた後も検出が崩れないことの再確認である。

```text
実行日: 2026-10-16
元にしたコミット: 91329d3-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT9-1.mutation crates/core/tests/mutations/EVT9-2.mutation crates/core/tests/mutations/EVT9-3.mutation crates/core/tests/mutations/EVT9-4.mutation crates/core/tests/mutations/EVT9-5.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT9-1 | crates/core/src/stop.rs: 世界状態を入口の写しへ戻さない | stop_after_two_ticks_removes_draft, stop_matches_sample_e06, p4_stop_returns_to_entry | input_after_stop_is_accepted_and_completes, stop_after_two_ticks_removes_draft, stop_matches_sample_e06, p4_stop_returns_to_entry | DETECTED | cases=256; minimal input: Plan { ticks: 0, resends: 0 } |
| EVT9-2 | crates/core/src/stop.rs: 世代を +1 しない | stop_after_two_ticks_removes_draft, stop_matches_sample_e06, p4_stop_returns_to_entry | stale_tick_after_stop_is_ignored, input_after_stop_is_accepted_and_completes, stale_tick_after_reinput_is_ignored, stop_matches_sample_e06, stop_after_two_ticks_removes_draft, p4_stop_returns_to_entry, p2_pre_stop_ticks_never_change_run | DETECTED | cases=256; minimal input: (Plan { ticks: 0, resends: 0 }, [Old { second_step: false, index: 0 }]) |
| EVT9-3 | crates/core/src/stop.rs: Stopped.at に操作後の世代を使う | stop_after_two_ticks_removes_draft, stop_matches_sample_e06 | stop_matches_sample_e06, stop_after_two_ticks_removes_draft | DETECTED | - |
| EVT9-4 | crates/core/src/stop.rs: 写しの Step と現在の Step の照合を外す | stop_with_other_step_entry_warns_only | stop_with_other_step_entry_warns_only | DETECTED | - |
| EVT9-5 | crates/core/src/judge.rs: 進行の宛先の照合から世代を外す（停止前の進行が、再入力後の処理中に効いてしまう） | stale_tick_after_reinput_is_ignored, p2_pre_stop_ticks_never_change_run | input_after_stop_is_accepted_and_completes, stale_tick_after_reinput_is_ignored, stale_tick_after_stop_is_ignored, stop_matches_sample_e06, p2_pre_stop_ticks_never_change_run | DETECTED | cases=256; minimal input: (Plan { ticks: 0, resends: 0 }, [Old { second_step: false, index: 0 }]) |

### 5.41 Step の移動 Control::GoTo（TASK-213 1/2）

処理中を解かない誤り（EVT11-1）、世代を +1 しない誤り（EVT11-2）、`Moved.at` に操作後の世代を使う誤り（EVT11-3）、達成済みの移動先を `Entered` に戻さない誤り（EVT11-4）、IR に無い Step の検査を外す誤り（EVT11-5）、入口の写しを無効化しない誤り（EVT11-6）を入れ、`move_step` が検出することを確かめる。

```text
実行日: 2026-10-16
元にしたコミット: 40e20ed-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT11-1.mutation crates/core/tests/mutations/EVT11-2.mutation crates/core/tests/mutations/EVT11-3.mutation crates/core/tests/mutations/EVT11-4.mutation crates/core/tests/mutations/EVT11-5.mutation crates/core/tests/mutations/EVT11-6.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT11-1 | crates/core/src/go_to.rs: 移動で処理中を解かない | move_while_busy_increments_generation_and_clears_busy, p4_move_clears_busy_and_increments | move_to_same_step_reenters, move_while_busy_increments_generation_and_clears_busy, p4_move_clears_busy_and_increments | DETECTED | cases=256; minimal input: Plan { ticks: 0, resends: 0, target: 0 } |
| EVT11-2 | crates/core/src/go_to.rs: 移動で世代を +1 しない | move_while_busy_increments_generation_and_clears_busy, idle_move_also_increments, p4_move_clears_busy_and_increments | idle_move_also_increments, move_while_busy_increments_generation_and_clears_busy, move_to_same_step_reenters, p4_move_clears_busy_and_increments | DETECTED | cases=256; minimal input: Plan { ticks: 0, resends: 0, target: 0 } |
| EVT11-3 | crates/core/src/go_to.rs: Moved.at に操作後の世代を使う | move_while_busy_increments_generation_and_clears_busy | move_while_busy_increments_generation_and_clears_busy, move_to_unknown_step_is_refused | DETECTED | - |
| EVT11-4 | crates/core/src/go_to.rs: 達成済みの移動先を Entered に戻さない | move_to_achieved_step_sets_entered | move_to_same_step_reenters, move_to_achieved_step_sets_entered, move_invalidates_entry_copy, p4_move_clears_busy_and_increments | DETECTED | cases=256; minimal input: Plan { ticks: 0, resends: 0, target: 0 } |
| EVT11-5 | crates/core/src/go_to.rs: IR に無い Step の検査を外す（無い Step へ移動できてしまう） | move_to_unknown_step_is_refused | move_to_unknown_step_is_refused | DETECTED | - |
| EVT11-6 | crates/core/src/go_to.rs: 入口の写しを無効化しない | move_invalidates_entry_copy | move_invalidates_entry_copy | DETECTED | - |

### 5.42 処理中でない停止の取り消しの入力としての扱い Control::Stop（TASK-212）

処理中でない停止を取り消しの入力に渡さない誤り（EVT10-1）、処理中の停止まで取り消しの入力に渡す誤り（EVT10-2）、取り消しの受付の規則の効果を適用しない誤り（EVT10-3）、受付済みの Step の取り消しの受付で `Warning` でなく `Judged` を出す誤り（EVT10-4）、処理中でない停止の `Changed.cause` を停止のままにする誤り（EVT10-5）を入れ、`cancel_input`・`stop_restore` が検出することを確かめる。世代の +1 は `apply` の関門が戻すため、変異では見えにくい。`cancel_input` の世代の表明で担保する。

```text
実行日: 2026-10-16
元にしたコミット: d104ab4-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT10-1.mutation crates/core/tests/mutations/EVT10-2.mutation crates/core/tests/mutations/EVT10-3.mutation crates/core/tests/mutations/EVT10-4.mutation crates/core/tests/mutations/EVT10-5.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT10-1 | crates/core/src/session.rs: 処理中でない停止を取り消しの入力として扱わない（何も起きない） | idle_stop_without_cancel_rule_is_rejected_in_entered_step, idle_stop_without_cancel_rule_is_rejected_in_accepted_step, idle_stop_equals_cancel_input | idle_stop_without_cancel_rule_is_rejected_in_entered_step, idle_stop_with_reject_rule_judges_only, idle_stop_with_accept_rule_in_accepted_step_warns_only, idle_stop_without_cancel_rule_is_rejected_in_accepted_step, idle_stop_equals_cancel_input, idle_stop_with_accept_rule_accepts_and_applies_effects | DETECTED | - |
| EVT10-2 | crates/core/src/session.rs: 処理中の停止も取り消しの入力に渡す（入口の写しへ戻らない） | stop_after_two_ticks_removes_draft, stop_matches_sample_e06 | stale_tick_after_stop_is_ignored, input_after_stop_is_accepted_and_completes, stale_tick_after_reinput_is_ignored, stop_after_two_ticks_removes_draft, stop_with_other_step_entry_warns_only, stop_overflow_warns_only, stop_without_entry_warns_only, p4_stop_returns_to_entry, stop_matches_sample_e06, p2_pre_stop_ticks_never_change_run | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: Plan { ticks: 0, resends: 0 }; cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (Plan { ticks: 0, resends: 0 }, [Old { second_step: false, index: 0 }]) |
| EVT10-3 | crates/core/src/accept.rs: 取り消しの受付の規則の効果を適用しない | idle_stop_with_accept_rule_accepts_and_applies_effects | idle_stop_with_accept_rule_accepts_and_applies_effects | DETECTED | - |
| EVT10-4 | crates/core/src/accept.rs: 受付済みの Step の取り消しの受付で Warning でなく Judged を出す | idle_stop_with_accept_rule_in_accepted_step_warns_only | idle_stop_with_accept_rule_in_accepted_step_warns_only | DETECTED | - |
| EVT10-5 | crates/core/src/session.rs: 処理中でない停止の Changed.cause を停止のままにする | idle_stop_with_accept_rule_accepts_and_applies_effects | idle_stop_with_accept_rule_accepts_and_applies_effects | DETECTED | - |

### 5.43 移動前に予約された進行の無視（TASK-213 2/2）

進行の宛先の照合から世代を外す誤り（EVT11-7）を入れ、`move_step` が検出することを確かめる。同じ Step へ入り直して受け付け直した後の処理中でも、移動前の世代の進行が効かないことを固定する。EVT11-1〜6 は試験を足した後も検出が崩れないことの再確認である。

```text
実行日: 2026-10-16
元にしたコミット: 71b322c-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT11-1.mutation crates/core/tests/mutations/EVT11-2.mutation crates/core/tests/mutations/EVT11-3.mutation crates/core/tests/mutations/EVT11-4.mutation crates/core/tests/mutations/EVT11-5.mutation crates/core/tests/mutations/EVT11-6.mutation crates/core/tests/mutations/EVT11-7.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT11-1 | crates/core/src/go_to.rs: 移動で処理中を解かない | move_while_busy_increments_generation_and_clears_busy, p4_move_clears_busy_and_increments | move_while_busy_increments_generation_and_clears_busy, move_to_same_step_reenters, stale_tick_after_reenter_and_reinput_is_ignored, reserved_ticks_after_reenter_are_ignored, p4_move_clears_busy_and_increments | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: Plan { ticks: 0, resends: 0, target: 0 } |
| EVT11-2 | crates/core/src/go_to.rs: 移動で世代を +1 しない | move_while_busy_increments_generation_and_clears_busy, idle_move_also_increments, p4_move_clears_busy_and_increments | idle_move_also_increments, move_to_same_step_reenters, move_while_busy_increments_generation_and_clears_busy, reserved_ticks_after_move_are_ignored, reserved_ticks_after_reenter_are_ignored, stale_tick_after_reenter_and_reinput_is_ignored, p4_move_clears_busy_and_increments, p2_pre_move_ticks_never_change_run | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: Plan { ticks: 0, resends: 0, target: 0 }; cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (Plan { ticks: 0, resends: 0, target: 0 }, [Old { step: 0, index: 0 }]) |
| EVT11-3 | crates/core/src/go_to.rs: Moved.at に操作後の世代を使う | move_while_busy_increments_generation_and_clears_busy | move_while_busy_increments_generation_and_clears_busy, move_to_unknown_step_is_refused | DETECTED | - |
| EVT11-4 | crates/core/src/go_to.rs: 達成済みの移動先を Entered に戻さない | move_to_achieved_step_sets_entered | move_to_achieved_step_sets_entered, move_to_same_step_reenters, move_invalidates_entry_copy, stale_tick_after_reenter_and_reinput_is_ignored, p4_move_clears_busy_and_increments | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: Plan { ticks: 0, resends: 0, target: 0 } |
| EVT11-5 | crates/core/src/go_to.rs: IR に無い Step の検査を外す（無い Step へ移動できてしまう） | move_to_unknown_step_is_refused | move_to_unknown_step_is_refused, refused_move_keeps_reservation | DETECTED | - |
| EVT11-6 | crates/core/src/go_to.rs: 入口の写しを無効化しない | move_invalidates_entry_copy | move_invalidates_entry_copy | DETECTED | - |
| EVT11-7 | crates/core/src/judge.rs: 進行の宛先の照合から世代を外す（移動前の進行が、同じ Step へ戻って受け付け直した後の処理中に効いてしまう。EVT9-5 と同じ誤りを移動の側から確かめる） | stale_tick_after_reenter_and_reinput_is_ignored, p2_pre_move_ticks_never_change_run | reserved_ticks_after_reenter_are_ignored, stale_tick_after_reenter_and_reinput_is_ignored, p2_pre_move_ticks_never_change_run | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (Plan { ticks: 0, resends: 0, target: 0 }, [Old { step: 0, index: 0 }, Old { step: 0, index: 0 }, Old { step: 0, index: 0 }, Old { step: 0, index: 0 }, Old { step: 0, index: 0 }, Old { step: 0, index: 0 }]) |

### 5.44 チェックポイントの保存と復帰（TASK-214 1/3）

復帰で世代を +1 しない誤り（EVT12-1）、世界状態を戻さない誤り（EVT12-2）、無い ID を拒否せず別の写しへ戻る誤り（EVT12-3）、`Restored.log_len` に現在のログの長さを使う誤り（EVT12-4）、復帰で処理中を解く誤り（EVT12-5）、保存で次の ID を進めない誤り（EVT12-6）、入口の写しを戻さない誤り（EVT12-7）、世代のあふれを検査しない誤り（EVT12-8）を入れ、`checkpoint_restore` が検出することを確かめる。

```text
実行日: 2026-10-16
元にしたコミット: 3b3b810-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT12-1.mutation crates/core/tests/mutations/EVT12-2.mutation crates/core/tests/mutations/EVT12-3.mutation crates/core/tests/mutations/EVT12-4.mutation crates/core/tests/mutations/EVT12-5.mutation crates/core/tests/mutations/EVT12-6.mutation crates/core/tests/mutations/EVT12-7.mutation crates/core/tests/mutations/EVT12-8.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT12-1 | crates/core/src/checkpoint.rs: 復帰で世代を +1 しない（現在の世代のまま） | restore_after_saving_at_three_ticks_returns_saved_run | restore_after_saving_at_three_ticks_returns_saved_run, restore_keeps_log_drafts_hints_and_checkpoints, p4_restore_returns_saved_run | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (0, [Next]) |
| EVT12-2 | crates/core/src/checkpoint.rs: 復帰で世界状態を戻さない | restore_after_stop_brings_back_world | restore_after_stop_brings_back_world, p4_restore_returns_saved_run | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (0, [Stop, Stop]) |
| EVT12-3 | crates/core/src/checkpoint.rs: 無い ID を拒否せず別のチェックポイントへ戻る | restore_unknown_checkpoint_is_refused_without_change | restore_unknown_checkpoint_is_refused_without_change, p4_failed_restore_keeps_generation | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (0, [Next]) |
| EVT12-4 | crates/core/src/checkpoint.rs: Restored.log_len に現在のログの長さを使う | restore_after_saving_at_three_ticks_returns_saved_run | restore_after_saving_at_three_ticks_returns_saved_run | DETECTED | - |
| EVT12-5 | crates/core/src/checkpoint.rs: 復帰で処理中を解く（写しの busy を残さない） | restore_after_saving_at_three_ticks_returns_saved_run | restore_after_saving_at_three_ticks_returns_saved_run, restore_after_stop_brings_back_world, p4_restore_returns_saved_run | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (0, [Next]) |
| EVT12-6 | crates/core/src/checkpoint.rs: 保存で次の ID を進めない（2 回目の保存が 1 件目を上書きする） | second_save_does_not_overwrite_first | second_save_does_not_overwrite_first, save_keeps_run_and_generation | DETECTED | - |
| EVT12-7 | crates/core/src/checkpoint.rs: 復帰で入口の写しを戻さない | restore_after_go_to_brings_back_entry | restore_after_go_to_brings_back_entry | DETECTED | - |
| EVT12-8 | crates/core/src/checkpoint.rs: 世代のあふれで警告を出さない | restore_overflow_warns_only | restore_overflow_warns_only | DETECTED | - |

### 5.45 Restored による復習の作り直し（TASK-214 2/3）

復帰で変化の印を作り直さない誤り（EVT12-9）、保存の位置の次の控えを引く誤り（EVT12-10）、保存の時点の復習を控えない誤り（EVT12-11）、停止で Step の項目の変更を初期化しない誤り（EVT12-12）を入れ、`restore_review` が検出することを確かめる。

```text
実行日: 2026-10-16
元にしたコミット: 38b2216-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT12-9.mutation crates/core/tests/mutations/EVT12-10.mutation crates/core/tests/mutations/EVT12-11.mutation crates/core/tests/mutations/EVT12-12.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT12-9 | crates/core/src/projection.rs: 復帰を基準点にせず、変化の印を作り直さない | p1c_marks_reset_on_restore, c1_p1d_restore_rebuilds_review_from_saved_prefix | p1c_marks_reset_on_restore, c1_p1d_restore_rebuilds_review_from_saved_prefix | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Save, Resend, Restore(1)] |
| EVT12-10 | crates/core/src/projection.rs: 保存の位置の次の位置の控えを引く（log_len の取り違え） | evt14_review_after_restore_matches_saved_review, c1_p1d_restore_rebuilds_review_from_saved_prefix | evt14_restore_after_stop_brings_back_review, evt14_restore_same_checkpoint_twice_and_nested, evt14_review_after_restore_matches_saved_review, c1_p1d_restore_rebuilds_review_from_saved_prefix | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Save, Resend, Save, Save, Restore(3)] |
| EVT12-11 | crates/core/src/projection.rs: 保存の時点の復習を控えず、空を控える | evt14_review_after_restore_matches_saved_review, c1_p1d_restore_rebuilds_review_from_saved_prefix | evt14_restore_after_stop_brings_back_review, evt14_review_after_restore_matches_saved_review, evt14_restore_same_checkpoint_twice_and_nested, c1_p1d_restore_rebuilds_review_from_saved_prefix | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Save, Resend, Save, Save, Restore(2)] |
| EVT12-12 | crates/core/src/projection.rs: 停止で Step の項目の変更を初期化しない | evt14_restore_after_stop_brings_back_review | evt14_restore_after_stop_brings_back_review | DETECTED | - |

### 5.46 復帰前の進行の無視と処理中の再開（TASK-214 3/3）

進行の宛先の照合から世代を外す誤り（EVT12-13。復帰の後は Step と番号が保存時点に戻るため、復帰前の進行が二重に効く）、復帰で処理中の番号を保存時点の値にしない誤り（EVT12-14）を入れ、`restore_resume` が検出することを確かめる。

```text
実行日: 2026-10-16
元にしたコミット: 0b1bed4-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT12-13.mutation crates/core/tests/mutations/EVT12-14.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT12-13 | crates/core/src/judge.rs: 進行の宛先の照合から世代を外す | pre_restore_tick_with_saved_index_is_ignored, p2_pre_restore_ticks_never_change_run | pre_restore_tick_with_saved_index_is_ignored, restore_to_idle_checkpoint_ignores_pre_restore_ticks, pre_restore_tick_after_go_to_and_restore_is_ignored, each_generation_index_progresses_once, restore_resumes_progress_in_new_generation, pre_restore_tick_after_stop_reinput_and_restore_is_ignored, p2_pre_restore_ticks_never_change_run | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (3, [Next, Stop], [Old(15)]) |
| EVT12-14 | crates/core/src/checkpoint.rs: 復帰で処理中の番号を保存時点の値にしない（再開が先頭からになる） | restore_resumes_progress_in_new_generation, p4_restore_resumes_from_saved_done | restore_resumes_progress_in_new_generation, pre_restore_tick_after_stop_reinput_and_restore_is_ignored, pre_restore_tick_with_saved_index_is_ignored, each_generation_index_progresses_once, pre_restore_tick_after_go_to_and_restore_is_ignored, p4_restore_resumes_from_saved_done | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (1, [Next]) |

### 5.47 状態・イベントの変異注入（TASK-220）

状態・イベントの実装を 7 か所壊した変異を、`op_sequence_properties`（TASK-219 の性質テスト）が検出することを確かめた。
定義は `crates/core/tests/mutations/M1.mutation`〜`M7.mutation` である。

| ID | 壊したこと |
|---|---|
| M1 | 宛先（世代と Step）が食い違う進行を弾かない |
| M2 | 復帰で世代を上げない |
| M3 | 処理中の停止で、世界状態の変化を記録しない |
| M4 | 確認を待たず、受付の直後に達成にする |
| M5 | Step に入った時に、変化の印の元を消さない |
| M6 | 復帰で、復習を保存の時点に戻さない |
| M7 | 同じ番号の進行が二重に届いても通す |

- M1〜M4・M6・M7 は、固定の生成の試験 `properties_p1_to_p6_hold_after_every_op` の失敗を期待し、すべて検出された
- M5 は、固定の生成の試験が失敗しなかった。表の「実際に失敗した試験」に `properties_p1_to_p6_hold_after_every_op` が無いことが、その証拠である。そのため期待する失敗は、誘導つきの生成の `properties_p1_to_p6_hold_with_guided_sequences` にした。M5 の経緯は 5.47.1 に記録した。`M5` の定義は新しく足していない
- 「実際に失敗した試験」には、期待した試験のほかに、誘導つきの生成や固定列の試験も並ぶ。性質テストの種は既定で、ケース数は 1000 である

```text
実行日: 2026-10-16
元にしたコミット: e5287bc-dirty
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/M1.mutation crates/core/tests/mutations/M2.mutation crates/core/tests/mutations/M3.mutation crates/core/tests/mutations/M4.mutation crates/core/tests/mutations/M5.mutation crates/core/tests/mutations/M6.mutation crates/core/tests/mutations/M7.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| M1 | crates/core/src/judge.rs: 宛先（世代と Step）の照合を丸ごと外し、食い違う進行を弾かない（世代だけを外す EVT9-5 とは別） | properties_p1_to_p6_hold_after_every_op | properties_p1_to_p6_hold_with_guided_sequences, properties_p1_to_p6_hold_after_every_op, guided_generation_reaches_deeper_states_than_fixed_generation | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Guided(0), Tick(StaleGen)]; cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Cmd(0), Tick(StaleGen)] |
| M2 | crates/core/src/checkpoint.rs: 復帰で世代を上げない（EVT12-1 と同じ置換を性質テストで検出する） | properties_p1_to_p6_hold_after_every_op | stale_ticks_after_stop_move_and_restore_are_ignored, checker_detects_each_broken_property, p3_accepts_achievement_whose_acceptance_has_an_older_generation, p3_rejects_achievement_after_reentry_invalidates_acceptance, p3_rejects_achievement_when_restore_revoked_the_logged_acceptance, scripted_sequence_satisfies_properties_and_covers_each_check, properties_p1_to_p6_hold_after_every_op, guided_generation_reaches_deeper_states_than_fixed_generation, properties_p1_to_p6_hold_with_guided_sequences | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Save, Restore(1)]; cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Save, Restore(1)] |
| M3 | crates/core/src/session.rs: 処理中の停止で、世界状態の変化を Changed として記録しない | properties_p1_to_p6_hold_after_every_op | p3_rejects_achievement_after_reentry_invalidates_acceptance, p3_accepts_achievement_whose_acceptance_has_an_older_generation, checker_detects_each_broken_property, stale_ticks_after_stop_move_and_restore_are_ignored, scripted_sequence_satisfies_properties_and_covers_each_check, p3_rejects_achievement_when_restore_revoked_the_logged_acceptance, properties_p1_to_p6_hold_with_guided_sequences, properties_p1_to_p6_hold_after_every_op, guided_generation_reaches_deeper_states_than_fixed_generation | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Guided(0), Stop]; cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Cmd(0), Stop] |
| M4 | crates/core/src/accept.rs: 確認を待たず、受付の直後に達成にする | properties_p1_to_p6_hold_after_every_op | p3_rejects_achievement_when_restore_revoked_the_logged_acceptance, checker_detects_each_broken_property, scripted_sequence_satisfies_properties_and_covers_each_check, p3_accepts_achievement_whose_acceptance_has_an_older_generation, p3_rejects_achievement_after_reentry_invalidates_acceptance, properties_p1_to_p6_hold_with_guided_sequences, properties_p1_to_p6_hold_after_every_op, guided_generation_reaches_deeper_states_than_fixed_generation | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0)]; cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [GoTo(2), Approval(0)] |
| M5 | crates/core/src/projection.rs: Step に入っても変化の印の元（pending）を消さない（到達しない腕へ退避） | properties_p1_to_p6_hold_with_guided_sequences | checker_detects_each_broken_property, p3_rejects_achievement_after_reentry_invalidates_acceptance, p3_rejects_achievement_when_restore_revoked_the_logged_acceptance, p3_accepts_achievement_whose_acceptance_has_an_older_generation, scripted_sequence_satisfies_properties_and_covers_each_check, properties_p1_to_p6_hold_with_guided_sequences, guided_generation_reaches_deeper_states_than_fixed_generation | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0)] |
| M6 | crates/core/src/projection.rs: 復帰で持ち主の根の復習を保存の時点へ戻さない | properties_p1_to_p6_hold_after_every_op | p3_accepts_achievement_whose_acceptance_has_an_older_generation, p3_rejects_achievement_after_reentry_invalidates_acceptance, checker_detects_each_broken_property, scripted_sequence_satisfies_properties_and_covers_each_check, p3_rejects_achievement_when_restore_revoked_the_logged_acceptance, properties_p1_to_p6_hold_after_every_op, guided_generation_reaches_deeper_states_than_fixed_generation, properties_p1_to_p6_hold_with_guided_sequences | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Save, Cmd(0), Restore(1)]; cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Save, Guided(0), Restore(1)] |
| M7 | crates/core/src/judge.rs: 適用済みの番号の進行（二重の届き）を弾かない（先の番号は弾く） | properties_p1_to_p6_hold_after_every_op | checker_detects_each_broken_property, p3_rejects_achievement_when_restore_revoked_the_logged_acceptance, p3_rejects_achievement_after_reentry_invalidates_acceptance, p3_accepts_achievement_whose_acceptance_has_an_older_generation, scripted_sequence_satisfies_properties_and_covers_each_check, properties_p1_to_p6_hold_with_guided_sequences, properties_p1_to_p6_hold_after_every_op, guided_generation_reaches_deeper_states_than_fixed_generation | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Guided(0), Guided(0), Tick(Dup)]; cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Cmd(0), Tick(Current), Tick(Dup)] |

#### 5.47.1 M5 の経緯（TASK-220 2/2）

M5 は、固定の生成では検出できず、誘導つきの生成が検出した。この事実を、PoC-1 の発見事項 F-11 と EVT-15 の証拠の種類に沿って記録する。
ここでの「固定の生成」は、乱数で操作列を作る `properties_p1_to_p6_hold_after_every_op` を指す。固定列（`scripted-inputs.json`）とは別物である。

- 壊したこと: `Event::Entered` で変化の印の元（`pending`）を消さない。Step に入った後の入力で、前の Step の印が残って見える
- 違反した性質: 誘導つきの生成の失敗は、P-1c（変化の印が期待と違う）である。7 操作目の学習者の入力で `marks [Mark { path: "notes/draft", kind: Put }] != expected []` と出た（2026-10-16、写しに M5 を当てて `cargo test` で確認）
- 固定の生成が見逃す理由: 違反が表に出るには、達成の後に次の Step へ入り、その後に変化の印を見る深い状態が要る。第 2 節の実測（2026-10-16）では、確認による達成が固定 29 件・誘導 1,075 件、最後の Step の達成が固定 28 通り・誘導 509 通り、達成の直後に次の Step へ入った件数が固定 150 件・誘導 1,083 件だった。最小の反例 `[Guided(0)×7]`（正しい次の操作を 7 回続ける列）も、深さが要ることを示す
- 主な証拠: 上の表と下の再実行の、M5 の「実際に失敗した試験」に `properties_p1_to_p6_hold_after_every_op` が無い（M1〜M4・M6・M7 には有る）。スクリプトは試験のターゲットの全試験を流して失敗をすべて列挙するため、一覧に無いことは、変異を入れた状態でその試験が通ったことを意味する。検出したのは、誘導つきの生成のほか、固定列の `scripted_sequence_satisfies_properties_and_covers_each_check` と P-3 の決定的な試験などである
- 限界: この事実は、既定の種・1,000 通りでの結果である。種を変えると、固定の生成が M5 を検出することもありうる（第 2 節）

M5 の再実行の記録（手で作らず、スクリプトの出力をそのまま貼る）。

```text
実行日: 2026-10-16
元にしたコミット: 193c38f
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/M5.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| M5 | crates/core/src/projection.rs: Step に入っても変化の印の元（pending）を消さない（到達しない腕へ退避） | properties_p1_to_p6_hold_with_guided_sequences | p3_accepts_achievement_whose_acceptance_has_an_older_generation, checker_detects_each_broken_property, p3_rejects_achievement_after_reentry_invalidates_acceptance, scripted_sequence_satisfies_properties_and_covers_each_check, p3_rejects_achievement_when_restore_revoked_the_logged_acceptance, properties_p1_to_p6_hold_with_guided_sequences, guided_generation_reaches_deeper_states_than_fixed_generation | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0)] |

補助の証拠として、対照を流した。対照は、M5 と同じ `find`・`replace` で、期待する失敗だけを固定の生成の試験にしたものである。
定義はコミットしない（既定の置き場に置くと `make check-mutations` が常に失敗するため）。リポジトリの写し（一時ディレクトリ）に置いて流した。

```text
id: M5-FIXED-CONTROL
package: fandhe-workshop-core
test: op_sequence_properties
target: crates/core/src/projection.rs
find:             Event::Entered { .. } => {
replace:             Event::Entered { .. } => { self.push_row(i, event); } Event::Entered { .. } if false => {
expect-fail: properties_p1_to_p6_hold_after_every_op
```

この対照は、結果が SURVIVED（終了コード 1）になるのが期待どおりである。固定の生成の試験が失敗しないことを確かめるためである。
写しには `.git` が無いため、「元にしたコミット」は `unknown` と出る。写しの元にしたコミットは `193c38f` である。

```text
実行日: 2026-10-16
元にしたコミット: unknown
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/M5-FIXED-CONTROL.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| M5-FIXED-CONTROL | crates/core/src/projection.rs: 対照。M5 と同じ壊し方を、固定の生成の試験に期待する | properties_p1_to_p6_hold_after_every_op | scripted_sequence_satisfies_properties_and_covers_each_check, p3_rejects_achievement_when_restore_revoked_the_logged_acceptance, p3_rejects_achievement_after_reentry_invalidates_acceptance, p3_accepts_achievement_whose_acceptance_has_an_older_generation, checker_detects_each_broken_property, properties_p1_to_p6_hold_with_guided_sequences, guided_generation_reaches_deeper_states_than_fixed_generation | SURVIVED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0)] |

### 5.48 Shift+Enter の改行（TASK-275 2/2）

JUDGE13-1 は、`key_route` の `Newline` の効果を「`preventDefault` して送信」に変える。Shift+Enter が改行を奪い、複数行の下書きを書けなくなる。
契約試験 `key_route_contract` が検出する。DOM での結果は `tools/browser/enter-submit.test.mjs`（`make test-browser`）が確かめる。

```text
実行日: 2026-10-16
元にしたコミット: fde9fd1-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/JUDGE13-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE13-1 | crates/web/src/key_route.rs: Shift+Enter が改行を奪って送信になる（複数行の下書きを書けなくなる） | shift_enter_は_prevent_default_せず送信もしない | shift_enter_は_prevent_default_せず送信もしない | DETECTED | - |

### 5.49 Tab の補完の条件（TASK-278 1/3）

JUDGE19-2 は、`key_route` の `Complete` の効果を「何もしない」に戻す。候補が開いていて文字があっても Tab の既定動作を止めず、補完の代わりにフォーカスが移る。
契約試験 `key_route_contract` の表の試験が検出する。候補が開く状態は TASK-440 が結ぶまで画面から作れないため、ブラウザ試験（`tools/browser/tab-complete.test.mjs`）は候補なしの行だけを確かめる。

```text
実行日: 2026-10-16
元にしたコミット: b6a2c49-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/JUDGE19-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE19-2 | crates/web/src/key_route.rs: 候補が開いて文字があっても Tab の既定動作を止めず、補完の代わりにフォーカスが移る | tab_は候補が開いていて文字があるときだけ_prevent_default_する | tab_は候補が開いていて文字があるときだけ_prevent_default_する | DETECTED | - |

### 5.50 処理中の Esc の停止（TASK-277 1/2）

処理中の Esc の優先を外す変異を、2 段で検出する。JUDGE17-1（contract）は判定表 `decide` の優先順位（処理中なら停止を最優先）を外し、JUDGE17-2（web）は `key_route::esc_route` の `Stop` の腕を「何もしない」に戻す（停止が core の `Control::Stop` に結ばれず、候補を閉じるだけになる）。
JUDGE17-2 は契約試験 `key_route_contract` が検出する。処理中の状態は TASK-347 でエンジンを結ぶまで画面から作れないため、ブラウザ試験（`tools/browser/esc-key.test.mjs`）は処理中でない行だけを確かめる。

```text
実行日: 2026-10-16
元にしたコミット: ef8a15e-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/JUDGE17-2.mutation crates/contract/tests/mutations/JUDGE17-1.mutation crates/web/tests/mutations/JUDGE13-1.mutation crates/web/tests/mutations/JUDGE19-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE17-2 | crates/web/src/key_route.rs: 処理中の Esc が停止にならず、候補が開いていれば閉じるだけ・処理中の効果が残る（処理中の Esc の優先を外す） | 処理中の_esc_は候補が開いていても停止になり候補も閉じる | 処理中の_esc_は候補が開いていても停止になり候補も閉じる | DETECTED | - |
| JUDGE17-1 | crates/contract/src/key_decide.rs: Esc の処理中の優先を外す（処理中でも停止せず、候補を閉じるか入力を取り消してしまう） | judge17_three_ways_with_busy_priority, expected_table_matches | expected_table_matches, judge17_three_ways_with_busy_priority | DETECTED | - |
| JUDGE13-1 | crates/web/src/key_route.rs: Shift+Enter が改行を奪って送信になる（複数行の下書きを書けなくなる） | shift_enter_は_prevent_default_せず送信もしない | shift_enter_は_prevent_default_せず送信もしない | DETECTED | - |
| JUDGE19-2 | crates/web/src/key_route.rs: 候補が開いて文字があっても Tab の既定動作を止めず、補完の代わりにフォーカスが移る | tab_は候補が開いていて文字があるときだけ_prevent_default_する | tab_は候補が開いていて文字があるときだけ_prevent_default_する | DETECTED | - |

### 5.51 処理中でない Esc の 2 通り（TASK-277 2/2）

処理中でない Esc の行き先を外す変異を検出する。JUDGE17-3 は `key_route::esc_route` の `CloseCandidates` の腕を「何もしない」に戻し（候補が開いていても Esc で閉じない）、JUDGE17-4 は `CancelInput` の腕を同様に戻す（取り消しの入力がエンジンへ届かず、規則が無い Step の拒否も記録されない）。JUDGE17-2 は同じファイルを変えたため再確認として含める。
検出先はいずれも契約試験 `key_route_contract` である。候補が開く状態は TASK-440、Component の結線は TASK-347 の後でないと画面から作れないため、この 2 通りのブラウザ試験はまだ無い。

```text
実行日: 2026-10-16
元にしたコミット: 409dc04-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/JUDGE17-3.mutation crates/web/tests/mutations/JUDGE17-4.mutation crates/web/tests/mutations/JUDGE17-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE17-3 | crates/web/src/key_route.rs: 処理中でなく候補が開いていても、Esc で候補が閉じない | 処理中でなく候補が開いている_esc_は候補を閉じるだけで操作を出さない | 処理中でなく候補が開いている_esc_は候補を閉じるだけで操作を出さない | DETECTED | - |
| JUDGE17-4 | crates/web/src/key_route.rs: 取り消しの入力がエンジンへ届かず、規則が無い Step の拒否も記録されない | 処理中でなく候補が閉じている_esc_は取り消しの入力になる | 処理中でなく候補が閉じている_esc_は取り消しの入力になる, 取り消しの入力は規則の無い_step_で拒否として記録される | DETECTED | - |
| JUDGE17-2 | crates/web/src/key_route.rs: 処理中の Esc が停止にならず、候補が開いていれば閉じるだけ・処理中の効果が残る（処理中の Esc の優先を外す） | 処理中の_esc_は候補が開いていても停止になり候補も閉じる | 処理中の_esc_は候補が開いていても停止になり候補も閉じる | DETECTED | - |

### 5.52 Shift+Tab の逆向きのフォーカス移動（TASK-278 2/3）

JUDGE19-3 は、判定表 `decide` の Tab の行から Shift の条件を外す。候補が開いて文字があると Shift+Tab まで補完になり、逆向きのフォーカス移動を奪う。
契約試験 `key_decide_contract` が検出する。候補が開く状態は TASK-440 が結ぶまで画面から作れないため、ブラウザ試験（`tools/browser/shift-tab.test.mjs`）は候補なしの行（`preventDefault` されない・入力欄から `body` へ出る・送信ボタンから入力欄へ戻る）だけを確かめる。

```text
実行日: 2026-10-16
元にしたコミット: 1547929-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/contract/tests/mutations/JUDGE19-3.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE19-3 | crates/contract/src/key_decide.rs: Tab の補完の条件から Shift を外す（候補が開いて文字があると Shift+Tab まで補完になり、逆向きのフォーカス移動を奪う） | judge19_prevent_default_on_tab_only_when_open_and_text, expected_table_matches | expected_table_matches, judge19_prevent_default_on_tab_only_when_open_and_text | DETECTED | - |

### 5.53 キー判定表の版 2（TASK-281）

版 2 で確定した JUDGE-18・21・22 の変異を足した。JUDGE22-1・2 は、修飾キー付き Enter の行を外す・Shift+Ctrl+Enter を外すもので、送信・改行になることを検出する。
JUDGE18-1 は 1 回目の Esc で取り消すもの、JUDGE21-1 は補完を末尾の候補にするものである。既存の 4 件（JUDGE14-1・17-1・19-1・19-3）も、版 2 で検出できることを確かめた。
web の `key_route` の変異（JUDGE13-1・17-2 から 4・19-2）は、今回は流していない（未実施）。

```text
実行日: 2026-10-16
元にしたコミット: 288b430（本変更の直前のコミット。変異は本変更を含む作業ツリーに対して流した）
コマンド: scripts/check-mutations.sh crates/contract/tests/mutations/JUDGE14-1.mutation crates/contract/tests/mutations/JUDGE17-1.mutation crates/contract/tests/mutations/JUDGE18-1.mutation crates/contract/tests/mutations/JUDGE19-1.mutation crates/contract/tests/mutations/JUDGE19-3.mutation crates/contract/tests/mutations/JUDGE21-1.mutation crates/contract/tests/mutations/JUDGE22-1.mutation crates/contract/tests/mutations/JUDGE22-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE14-1 | crates/contract/src/key_decide.rs: 変換中の判定から自前の印を外す | judge14_any_single_signal_means_composing, expected_table_matches | judge14_any_single_signal_means_composing, expected_table_matches | DETECTED | - |
| JUDGE17-1 | crates/contract/src/key_decide.rs: Esc の処理中の優先を外す | judge17_three_ways_with_busy_priority, expected_table_matches | expected_table_matches, judge17_three_ways_with_busy_priority | DETECTED | - |
| JUDGE18-1 | crates/contract/src/key_decide.rs: 候補が開いていても 1 回目の Esc で取り消す | judge18_two_escapes_close_then_cancel, expected_table_matches | judge18_two_escapes_close_then_cancel, expected_table_matches, judge17_three_ways_with_busy_priority | DETECTED | - |
| JUDGE19-1 | crates/contract/src/key_decide.rs: Tab の補完の条件から has_text を外す | judge19_prevent_default_on_tab_only_when_open_and_text, expected_table_matches | expected_table_matches, judge19_prevent_default_on_tab_only_when_open_and_text | DETECTED | - |
| JUDGE19-3 | crates/contract/src/key_decide.rs: Tab の補完の条件から Shift を外す | judge19_prevent_default_on_tab_only_when_open_and_text, expected_table_matches | judge19_prevent_default_on_tab_only_when_open_and_text, expected_table_matches | DETECTED | - |
| JUDGE21-1 | crates/contract/src/key_decide.rs: 先頭ではなく末尾の候補で補完する | judge21_completes_with_first_candidate | judge21_completes_with_first_candidate | DETECTED | - |
| JUDGE22-1 | crates/contract/src/key_decide.rs: 修飾キー付き Enter の行を外す | judge22_modified_enter_is_nothing, expected_table_matches | expected_table_matches, judge22_modified_enter_is_nothing | DETECTED | - |
| JUDGE22-2 | crates/contract/src/key_decide.rs: Shift+Ctrl+Enter を修飾キー付きから外す | judge22_modified_enter_is_nothing, expected_table_matches | expected_table_matches, judge22_modified_enter_is_nothing | DETECTED | - |

### 5.54 候補が開いているときの Esc と補完の選び方（TASK-282 1/2）

JUDGE18-2 は `key_route::after_esc` が候補を閉じたままにしない変異で、1 回目の Esc の後も候補が開いたままになり、2 回目の Esc が取り消しに届かない。JUDGE21-2 は `completion_choice` が先頭ではなく末尾の候補を選ぶ変異である。
検出先はいずれも契約試験 `key_route_contract` である。候補の一覧が TASK-440 まで画面に無いため、ブラウザ試験はまだ無い。同じ `key_route.rs` を変えたため、既存の web の変異（JUDGE13-1・17-2 から 4・19-2）も再確認として含めた。

```text
実行日: 2026-10-16
元にしたコミット: 859b0e3-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/JUDGE18-2.mutation crates/web/tests/mutations/JUDGE21-2.mutation crates/web/tests/mutations/JUDGE13-1.mutation crates/web/tests/mutations/JUDGE17-2.mutation crates/web/tests/mutations/JUDGE17-3.mutation crates/web/tests/mutations/JUDGE17-4.mutation crates/web/tests/mutations/JUDGE19-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE18-2 | crates/web/src/key_route.rs: 1 回目の Esc の後も候補が開いたままで、2 回目の Esc も閉じるだけになり、取り消しに届かない | 候補が開いているときの_esc_は1回目で閉じ2回目で取り消しになる | 候補が開いているときの_esc_は1回目で閉じ2回目で取り消しになる, 処理中の_esc_は何回押しても停止で取り消しにならない | DETECTED | - |
| JUDGE21-2 | crates/web/src/key_route.rs: 先頭ではなく末尾の候補で補完する | 補完は先頭の候補を選ぶ | 補完は先頭の候補を選ぶ | DETECTED | - |
| JUDGE13-1 | crates/web/src/key_route.rs: Shift+Enter が改行を奪って送信になる | shift_enter_は_prevent_default_せず送信もしない | shift_enter_は_prevent_default_せず送信もしない | DETECTED | - |
| JUDGE17-2 | crates/web/src/key_route.rs: 処理中の Esc の優先を外す | 処理中の_esc_は候補が開いていても停止になり候補も閉じる | 処理中の_esc_は何回押しても停止で取り消しにならない, 処理中の_esc_は候補が開いていても停止になり候補も閉じる | DETECTED | - |
| JUDGE17-3 | crates/web/src/key_route.rs: 処理中でなく候補が開いていても、Esc で候補が閉じない | 処理中でなく候補が開いている_esc_は候補を閉じるだけで操作を出さない | 候補が開いているときの_esc_は1回目で閉じ2回目で取り消しになる, 処理中でなく候補が開いている_esc_は候補を閉じるだけで操作を出さない, 閉じた後に候補がまた開けば_esc_は再び閉じるだけになる | DETECTED | - |
| JUDGE17-4 | crates/web/src/key_route.rs: 取り消しの入力がエンジンへ届かない | 処理中でなく候補が閉じている_esc_は取り消しの入力になる | 処理中でなく候補が閉じている_esc_は取り消しの入力になる, 候補が開いているときの_esc_は1回目で閉じ2回目で取り消しになる, 取り消しの入力は規則の無い_step_で拒否として記録される | DETECTED | - |
| JUDGE19-2 | crates/web/src/key_route.rs: 候補が開いて文字があっても Tab の既定動作を止めない | tab_は候補が開いていて文字があるときだけ_prevent_default_する | tab_は候補が開いていて文字があるときだけ_prevent_default_する | DETECTED | - |

### 5.55 修飾キー付きの Enter（TASK-282 2/2）

JUDGE22-3 は `key_route::effect_for` の `Nothing` の腕を送信にする変異で、Ctrl・Alt・Meta 付きの Enter（Shift 併用を含む）が誤送信になる。検出先は契約試験 `key_route_contract` の `judge22_...` である。契約側の変異 JUDGE22-1・JUDGE22-2 は 5.53 で記録済みである。
ブラウザ試験（`tools/browser/enter-submit.test.mjs`）は、修飾キー付きの Enter で一覧が増えず、欄の値が変わらず（改行が入らず）、`preventDefault` されないことを確かめた。Linux の Chromium で改行は観察されなかった。`make test-browser` は 105 件が成功した（失敗 0）。Playwright の `keyboard.press` は Shift 以外の修飾キーを押している間は text を送らないため、Windows の実機での差は確かめていない（D-48 で別タスク）。

```text
実行日: 2026-10-16
元にしたコミット: c598d3b-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/JUDGE22-3.mutation crates/web/tests/mutations/JUDGE13-1.mutation crates/web/tests/mutations/JUDGE17-2.mutation crates/web/tests/mutations/JUDGE17-3.mutation crates/web/tests/mutations/JUDGE17-4.mutation crates/web/tests/mutations/JUDGE18-2.mutation crates/web/tests/mutations/JUDGE19-2.mutation crates/web/tests/mutations/JUDGE21-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| JUDGE22-3 | crates/web/src/key_route.rs: 修飾キー付きの Enter が送信になる（誤送信） | judge22_修飾キー付きの_enter_は送信も_prevent_default_もしない | 同左を含む 9 件 | DETECTED | - |
| JUDGE13-1 | crates/web/src/key_route.rs: Shift+Enter が改行を奪って送信になる | shift_enter_は_prevent_default_せず送信もしない | 同左 | DETECTED | - |
| JUDGE17-2 | crates/web/src/key_route.rs: 処理中の Esc の優先を外す | 処理中の_esc_は候補が開いていても停止になり候補も閉じる | 同左を含む 2 件 | DETECTED | - |
| JUDGE17-3 | crates/web/src/key_route.rs: 候補が開いていても Esc で閉じない | 処理中でなく候補が開いている_esc_は候補を閉じるだけで操作を出さない | 同左を含む 3 件 | DETECTED | - |
| JUDGE17-4 | crates/web/src/key_route.rs: 取り消しの入力がエンジンへ届かない | 処理中でなく候補が閉じている_esc_は取り消しの入力になる | 同左を含む 3 件 | DETECTED | - |
| JUDGE18-2 | crates/web/src/key_route.rs: 1 回目の Esc の後も候補が開いたまま | 候補が開いているときの_esc_は1回目で閉じ2回目で取り消しになる | 同左を含む 2 件 | DETECTED | - |
| JUDGE19-2 | crates/web/src/key_route.rs: 候補が開いて文字があっても Tab を止めない | tab_は候補が開いていて文字があるときだけ_prevent_default_する | 同左 | DETECTED | - |
| JUDGE21-2 | crates/web/src/key_route.rs: 末尾の候補で補完する | 補完は先頭の候補を選ぶ | 同左 | DETECTED | - |

### 5.56 読み込みの拒否理由コード（TASK-310 2/3）

SAVE20-1 は `other_pack` の文字列を `malformed` に重ねる変異で、理由コードの名前の重複を作る。検出先は契約試験 `load_reason_contract` の `codes_are_unique` である。
SAVE20-2 は `too_large` を `LimitReason` に委ねず文字列で再定義する変異で、上限の契約とのずれを作る。検出先は `limit_codes_delegate_to_limit_reason` である。
VER7-1 は `content_major` の `ReasonKind` への対応を落とす変異で、検出先は `reason_kind_mapping` である。

```text
実行日: 2026-10-16
元にしたコミット: e9306ca-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/contract/tests/mutations/SAVE20-1.mutation crates/contract/tests/mutations/SAVE20-2.mutation crates/contract/tests/mutations/VER7-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| SAVE20-1 | crates/contract/src/load_reason.rs: 拒否理由コードの名前を重複させる | codes_are_unique | codes_are_unique を含む 4 件 | DETECTED | - |
| SAVE20-2 | crates/contract/src/load_reason.rs: 上限の理由コードを委ねずに再定義する | limit_codes_delegate_to_limit_reason | 同左を含む 3 件 | DETECTED | - |
| VER7-1 | crates/contract/src/load_reason.rs: content_major の分類を落とす | reason_kind_mapping | 同左を含む 2 件 | DETECTED | - |

### 5.57 読み込みの判定の順序（TASK-310 3/3）

SAVE19-1 は判定の順序（`HISTORY_LOAD_ORDER`）の中で `vocab_newer` の位置を `engine_newer` に置き換え、重複を作る変異で、検出先は契約試験 `history_load_order_contract` の `order_matches_contract` である。
SAVE19-2 は `too_many_ops` の段を大きさの段にする変異で、上限の契約（`LimitReason::stage`）とのずれを作る。検出先は `limit_reasons_agree_with_limits_contract` である。
SAVE19-3 は `other_pack` を適用し直しの段にする変異で、段の単調性を崩す。検出先は `stage_is_non_decreasing_along_order` である。最初の案（`other_pack` を版の判定の段にする）は、次の `bad_version` と同じ段で単調性が崩れず SURVIVED になったため、適用し直しの段へ改めた。

### 5.58 左の成果物ビューの型と印の対応（TASK-467 1/2）

ART3-1 は `change_mark_of` が置かれた印（`Put`）を削除の印に対応させる変異で、検出先は契約試験 `artifact_view_contract` の `change_mark_of_の対応表` である。
ART2-1 は前後が同じ行に変更の印を付け、違う行には付けない変異で、検出先は `shown_後の状態の追加_削除_変更_不変` と、印の数を前後の組から数え直す `後の状態の印の数は前後の組から数えた変化の数と一致する` である。

```text
実行日: 2026-10-16
元にしたコミット: 10b0a5f-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/contract/tests/mutations/SAVE19-1.mutation crates/contract/tests/mutations/SAVE19-2.mutation crates/contract/tests/mutations/SAVE19-3.mutation crates/web/tests/mutations/ART3-1.mutation crates/web/tests/mutations/ART2-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| SAVE19-1 | crates/contract/src/load_reason.rs: 判定の順序の中の版の判定の要素を重複させる | order_matches_contract | order_is_permutation_of_all を含む 2 件 | DETECTED | - |
| SAVE19-2 | crates/contract/src/load_reason.rs: 件数の上限の段を大きさの段にする | limit_reasons_agree_with_limits_contract | 同左を含む 3 件 | DETECTED | - |
| SAVE19-3 | crates/contract/src/load_reason.rs: other_pack を適用し直しの段にする | stage_is_non_decreasing_along_order | 同左を含む 3 件 | DETECTED | - |
| ART3-1 | crates/web/src/artifact_view.rs: 置かれた印を削除に対応させる（追加が削除の印で出る） | change_mark_of_の対応表 | change_mark_of_の対応表 | DETECTED | - |
| ART2-1 | crates/web/src/artifact_view.rs: 前後が同じ行に変更の印を付け、違う行には付けない | shown_後の状態の追加_削除_変更_不変, 後の状態の印の数は前後の組から数えた変化の数と一致する | 同左を含む 4 件 | DETECTED | - |

### 5.58 写しとの照合（TASK-316 2/3）

SAVE1-1 は、`verify_envelope` が適用し直した結果を写しと比べず、常に一致とみなす変異である（試作の X-4 に当たる、写しを信じる誤り）。
検出先は、写しを書き換えた包み・`ops` を書き換えた包みが `replay_mismatch` になることを確かめる固定の試験 2 本である。
ほかに失敗した 2 本は、食い違いが出なくなって期待の `Err` が来ない試験である。

```text
実行日: 2026-10-16
元にしたコミット: 10b0a5f-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAVE1-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| SAVE1-1 | crates/core/src/history_replay.rs: 照合を省いて写しを信じる（試作の X-4 相当。適用し直した結果を写しと比べない） | tampered_snapshot_is_replay_mismatch, tampered_ops_is_replay_mismatch | tampered_snapshot_is_replay_mismatch, tampered_ops_is_replay_mismatch, errors_do_not_carry_user_strings, view_only_source_reads_the_snapshot_without_replay | DETECTED | - |

### 5.59 復元の性質と変異 6/6（TASK-316 3/3）

試作の X-1〜X-6 を、本 repo の構成に読み替えた `SAVE1-1`〜`SAVE1-6` の 6 件を置いた。性質テストと固定の試験は、記録の経路
（`apply` で進めた状態）と適用し直した状態を比べる。このため、両側が共有する遷移（`session::apply` など）に入れた変異は両側が同じように
壊れて一致したままになる。6 件はすべて、適用し直し・照合・記録・包みの組み立てのように片側だけにある箇所に置いた。

| ID | 試作 | 壊し方 | 主な検出先 |
|---|---|---|---|
| SAVE1-1 | X-4 | 照合を省く | 固定の試験 2 本と、H-8 の性質テスト 2 本 |
| SAVE1-2 | X-3 | 適用し直しで `Control` を適用しない | 台本と適用し直しの性質テスト 2 本 |
| SAVE1-3 | X-5 | 記録の下書きの畳みで最初の値が残る | 台本と適用し直しの性質テスト 2 本（各枠 0〜2 件の下書きの生成が前提） |
| SAVE1-4 | X-6 | 照合の後に返す状態の下書きを落とす | 照合後の状態の試験と書き出し→読み込みの性質テスト |
| SAVE1-5 | X-2 | 書き出す包みの写しでチェックポイントを落とす | 台本と書き出し→読み込みの性質テスト |
| SAVE1-6 | X-1 の代替 | 再開の元の状態の下書きを落とす | 任意の時点の再読み込みの後の一致の性質テスト 2 本 |

X-1（`reloaded()` が世代を上げない。性質 H-6）は、`Reload` が TASK-320 まで `ReplayError::UnsupportedReload` で拒否されるため、
そのままの対応物が無い。代替の SAVE1-6 を置き、X-1・H-6 そのものは TASK-320 へ送る。
SAVE1-6 は、時点 t の状態に下書きが残る場合だけ食い違うため、性質テストだけが検出する。

```text
実行日: 2026-10-16
元にしたコミット: 95a91d3-dirty
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAVE1-1.mutation crates/core/tests/mutations/SAVE1-2.mutation crates/core/tests/mutations/SAVE1-3.mutation crates/core/tests/mutations/SAVE1-4.mutation crates/core/tests/mutations/SAVE1-5.mutation crates/core/tests/mutations/SAVE1-6.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| SAVE1-1 | crates/core/src/history_replay.rs: 照合を省いて写しを信じる（試作の X-4 相当。適用し直した結果を写しと比べない） | tampered_snapshot_is_replay_mismatch, tampered_ops_is_replay_mismatch, property_tampered_ops_never_open_wrong_state_fixed, property_tampered_ops_never_open_wrong_state_guided | tampered_ops_is_replay_mismatch, errors_do_not_carry_user_strings, tampered_snapshot_is_replay_mismatch, view_only_source_reads_the_snapshot_without_replay, property_tampered_ops_never_open_wrong_state_fixed, property_tampered_ops_never_open_wrong_state_guided | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([Cmd(0), Cmd(0), Cmd(0), Cmd(0)], [[], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], []], 611); cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([Guided(0), Guided(0), Guided(0), Guided(0), Guided(0)], [[], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], []], 1) |
| SAVE1-2 | crates/core/src/history_replay.rs: 適用し直しで Control の操作を適用しない（試作の X-3 相当。記録側と適用し直しの遷移が食い違う） | scripted_history_replays_to_snapshot, property_replay_matches_snapshot_fixed, property_replay_matches_snapshot_guided | scripted_history_replays_to_snapshot, property_replay_matches_snapshot_fixed, property_replay_matches_snapshot_guided | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([GoTo(0)], [[], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], []]); cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([GoTo(0)], [[], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], []]) |
| SAVE1-3 | crates/core/src/ops_record.rs: 記録で連続する下書きを畳むとき最初の値が残る（試作の X-5 相当。適用し直した下書きが写しと食い違う） | scripted_history_replays_to_snapshot, property_replay_matches_snapshot_fixed, property_replay_matches_snapshot_guided | scripted_history_replays_to_snapshot, property_replay_matches_snapshot_guided, property_replay_matches_snapshot_fixed | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([Cmd(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Guided(0), Guided(0), Guided(0), Guided(0)], [[], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [1, 2], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], []]); cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0)], [[], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [1, 2], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], []]) |
| SAVE1-4 | crates/core/src/history_replay.rs: 照合に通った後、返す状態の下書きを落とす（試作の X-6 相当。開いた状態が適用し直した状態と食い違う） | verified_state_is_the_replayed_state, scripted_history_round_trips_and_verifies, property_export_import_verifies_fixed, property_export_import_verifies_guided | scripted_history_round_trips_and_verifies, verified_state_is_the_replayed_state, property_export_import_verifies_guided, property_export_import_verifies_fixed, property_reload_at_any_point_guided, property_reload_at_any_point_fixed, property_tampered_ops_never_open_wrong_state_guided, property_tampered_ops_never_open_wrong_state_fixed | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([Guided(0)], [[], [1], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], []], 0); cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([Cmd(0), Cmd(0), Cmd(0), Choice(0)], [[], [], [], [1], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], []], 0); cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0)], [[], [], [], [], [], [], [], [], [], [], [], [1], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], []], 272); cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Choice(0)], [[], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [1], [], [], [], [], [], [], [], [], [], [], [], [], [], []], 417); cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([Cmd(1), Cmd(1), Cmd(1), Cmd(1), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Choice(0), Guided(0), Guided(0), Guided(0), Guided(0), Choice(0), Choice(0), Choice(0), Choice(0), Choice(0), Guided(0), Guided(0)], [[], [], [], [], [], [1], [1], [], [1], [1], [], [1], [1], [], [1], [], [1], [], [1], [], [1], [], [], [1], [1], [], [1], [], [], [1], [], [1], [1], [], [], [1], [], [], [1], [], [1], [], [], [], [], [], [], [], [], [], []], 312); cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cancel, Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cancel, Cmd(0), Cancel, Cmd(0), Cmd(0), Cancel, Choice(0), Cmd(0), Cmd(0), Choice(0)], [[], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [1], [], [], [], [], [], [], [], [], [], []], 6) |
| SAVE1-5 | crates/core/src/history_envelope.rs: 書き出す包みの写しでチェックポイントを落とす（試作の X-2 相当） | scripted_history_round_trips_and_verifies, property_export_import_verifies_fixed, property_export_import_verifies_guided | tampered_ops_is_replay_mismatch, tampered_snapshot_is_replay_mismatch, scripted_history_round_trips_and_verifies, verified_state_is_the_replayed_state, property_export_import_verifies_fixed, property_export_import_verifies_guided, property_reload_at_any_point_guided, property_reload_at_any_point_fixed | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([Save], [[], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], []], 0); cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([Save], [[], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], []], 0); cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([Save, Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0)], [[], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], []], 1); cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([Save, Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0)], [[], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], []], 1) |
| SAVE1-6 | crates/core/src/history_replay.rs: 再開の元の状態の下書きを落とす（試作の X-1 の代替。時点 t から残りを流す場合だけ食い違う。X-1 そのものは TASK-320） | property_reload_at_any_point_fixed, property_reload_at_any_point_guided | property_reload_at_any_point_guided, property_reload_at_any_point_fixed | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0), Guided(0)], [[], [], [], [], [], [], [], [], [], [], [], [1], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], []], 272); cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: ([Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Cmd(0), Choice(0)], [[], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [], [1], [], [], [], [], [], [], [], [], [], [], [], [], [], []], 417) |

### 5.61 大きさ・深さの判定の一本化（Issue #1394）

大きさの比較を `crates/core/src/load_gate.rs` の `check_size` の 1 か所に寄せ、`check_limits`（大きさ → 深さ）を
IR・履歴・関門が共有するようにした。そのため `SAFE20-1`・`SAFE20-2`・`SAFE18-1` の対象の行を `load_gate.rs` へ移した。
`SAFE20-2` の置換は、`Result<(), LimitReason>` を返す関数の中で `LoadError` を返せないため、深さの検査を大きさの前に置く形に変えた。

```text
実行日: 2026-10-16
元にしたコミット: ed3a165-dirty
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAFE20-1.mutation crates/core/tests/mutations/SAFE20-2.mutation crates/core/tests/mutations/SAFE18-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| SAFE20-1 | crates/core/src/load_gate.rs: 入れ子の深さの段を外す | depth_is_decided_before_parse, deep_valid_json_is_too_deep, depth_check_precedes_serde_recursion_limit, rejecting_stage_is_the_first_violated_in_contract_order | 期待どおり（4 件） | DETECTED |
| SAFE20-2 | crates/core/src/load_gate.rs: 深さの段を大きさの段より先にする | size_is_decided_before_depth, rejecting_stage_is_the_first_violated_in_contract_order | 期待どおり（2 件） | DETECTED |
| SAFE18-1 | crates/core/src/load_gate.rs: ちょうど 10MB を断る誤り | exact_limit_is_not_rejected_by_size, limit_neighborhood_is_judged_exactly, bytes_are_judged_by_length_without_reading_content, size_rule_holds_for_any_length | 期待どおり（4 件。最小の反例 10485760） | DETECTED |

### 5.60 左の成果物ビューの束縛点・指せる行・変種の口（TASK-467 2/2）

ART5-1 は選ばれていないビューに `hidden` を付けない変異で、検出先は `親の属性の表` と `view_list_の選択と件数` である。
ART11-1 は指せる行の属性の組から `tabindex` を落とす変異で、検出先は `属性の組は_4_つで順が固定_値はエスケープされる` である。
ART7-1 は文書に変種 (a) を許す変異で、検出先は `変種の許可の表` と `変種の方針は許されない組と重複を拒否する` である。
試験は `artifact_view_binding_contract` にある。

```text
実行日: 2026-10-16
元にしたコミット: 95a91d3-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART5-1.mutation crates/web/tests/mutations/ART11-1.mutation crates/web/tests/mutations/ART7-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| ART5-1 | crates/web/src/artifact_view.rs: 選ばれていないビューに hidden を付けない（ART-12。隠すべきビューが見えたままになる） | 親の属性の表, view_list_の選択と件数 | view_list_の選択と件数, 親の属性の表 | DETECTED | - |
| ART11-1 | crates/web/src/artifact_view.rs: 指せる行の属性の組から tabindex を落とす（キーボードで指せない行になる。ART-11・FEND-13） | 属性の組は_4_つで順が固定_値はエスケープされる | 属性の組は_4_つで順が固定_値はエスケープされる | DETECTED | - |
| ART7-1 | crates/web/src/artifact_view.rs: 文書に変種 (a) を許す（ART-7 で (b) だけと確定済みの文書が (a) で作れてしまう） | 変種の許可の表, 変種の方針は許されない組と重複を拒否する | 変種の許可の表, 変種の方針は許されない組と重複を拒否する | DETECTED | - |

### 5.61 進行と補完の配送（TASK-349 1/2）

FEND4-1 は補完を `set_timeout` の予約を通さず、リスナーの中から直接 `deliver` で届ける変異である（更新の最中の再入で黙って捨てられる誤り）。
検出先は、配送の口の規則を固定する `state_paths` である。FEND4-2 は次の進行の番号を 1 つ先にする変異で、`host_contract` の宛先の通知の試験が検出する。
`dispatch_action` の許可を `deliver` の 1 箇所に見直したため、G1・G2 と、`KeydownEffect` に欄を足した影響を受ける JUDGE13-1・JUDGE19-2 も再確認した。


```text
実行日: 2026-10-16
元にしたコミット: 95a91d3-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/FEND4-1.mutation crates/web/tests/mutations/FEND4-2.mutation crates/web/tests/mutations/G2.mutation crates/web/tests/mutations/G1.mutation crates/web/tests/mutations/JUDGE13-1.mutation crates/web/tests/mutations/JUDGE19-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| FEND4-1 | crates/web/src/glue.rs: 補完を set_timeout の予約を通さず、リスナーの中から直接 deliver で届ける（更新の最中の再入で黙って捨てられる） | 実ソースは違反なし, glue_は状態を変える呼び出しを持たない | 実ソースは違反なし, glue_は状態を変える呼び出しを持たない | DETECTED | - |
| FEND4-2 | crates/web/src/tick_schedule.rs: 次の進行の番号を 1 つ先にする（届いた進行が EVT-8 の番号の照合で無視され、処理中が進まなくなる） | 進行の宛先は設定した時点と更新のたびに通知される, 最後の進行の後は_none_が通知される | 停止の後の古い宛先は_ignored_1件で状態を変えず_再入力で新しい世代の宛先になる, 進行の宛先は設定した時点と更新のたびに通知される, 最後の進行の後は_none_が通知される | DETECTED | - |
| G2 | crates/web/src/glue.rs: glue から状態を変える呼び出しを足す（描画の結果から状態へ流す経路が黙って入る） | glue_は状態を変える呼び出しを持たない | 実ソースは違反なし, glue_は状態を変える呼び出しを持たない | DETECTED | - |
| G1 | crates/web/src/glue.rs: App に触れる口を 2 箇所にする（glue が App を読む経路が増えても気づけなくなる） | app_に触れるのは_with_app_の_component_の1箇所だけ | app_に触れるのは_with_app_の_component_の1箇所だけ, 実ソースは違反なし | DETECTED | - |
| JUDGE13-1 | crates/web/src/key_route.rs: Shift+Enter が改行を奪って送信になる（複数行の下書きを書けなくなる） | shift_enter_は_prevent_default_せず送信もしない | shift_enter_は_prevent_default_せず送信もしない | DETECTED | - |
| JUDGE19-2 | crates/web/src/key_route.rs: 候補が開いて文字があっても Tab の既定動作を止めず、補完の代わりにフォーカスが移る | tab_は候補が開いていて文字があるときだけ_prevent_default_する | tab_は候補が開いていて文字があるときだけ_prevent_default_する | DETECTED | - |

### 5.62 隠し要素と data-payload の経路の検査（TASK-349 2/2）

FEND4-3 は、配信する設定の画面に、進行の宛先を `data-payload` で届ける隠し要素 `tick-trigger` を足す変異である。
検出先は、ソースの静的検査 `no_hidden_triggers` の「実ソースは違反なし」である。
配信用 WASM・JS・HTML の文字列検索は `tools/check-delivery.sh`（項目 9・10）と `scripts/test-check-delivery.sh` が持つ。

```text
実行日: 2026-10-16
元にしたコミット: 89beeb6-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/FEND4-3.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| FEND4-3 | crates/web/src/host.rs: 配信する設定に、進行の宛先を data-payload で届ける隠し要素が入る（FEND-4 が禁じる経路） | 実ソースは違反なし | 実ソースは違反なし | DETECTED | - |

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

### 5.60 表示だけになる版の違いの変異（TASK-174 2/3）

`content_major`・`format_older`・`engine_major` の判定（`crates/core/src/history_open.rs`）に、`VER6-2`・`VER6-3`・`VER7-2`〜`VER7-4` の
5 件を置いた。`VER6-2` は結果の種類（表示だけ）を変えず理由コードだけを取り違えるため、既存の `VER6-1` では検出できない種類である。
`VER6-3` は向き、`VER7-2`・`VER7-3` は各判定の欠落、`VER7-4` は判定の順序の崩れを狙う。既存の `VER6-1`・`VER7-1` の退行も併せて確かめた。

```text
実行日: 2026-10-16
元にしたコミット: ed3a165-dirty
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/VER6-2.mutation crates/core/tests/mutations/VER6-3.mutation crates/core/tests/mutations/VER7-2.mutation crates/core/tests/mutations/VER7-3.mutation crates/core/tests/mutations/VER7-4.mutation crates/core/tests/mutations/VER6-1.mutation crates/core/tests/mutations/VER7-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| VER6-2 | crates/core/src/history_open.rs: 教材の主版の違いの理由コードを取り違える（表示だけには変わらないため、理由コードの値を固定する試験だけが検出する） | ver6_content_major_boundaries | ver6_content_major_with_removed_step_is_view_only_at_any_position, ver7_view_only_order_follows_the_contract, view_only_reasons_are_non_empty_and_classified, ver6_content_major_boundaries | DETECTED | - |
| VER6-3 | crates/core/src/history_open.rs: 教材の主版の違いを片方向（履歴が古い向き）だけ見る | ver6_content_major_boundaries | fail_closed_for_versions_that_must_not_open, ver6_content_major_boundaries | DETECTED | - |
| VER7-2 | crates/core/src/history_open.rs: 履歴の IR の版が古いことを判定しない（変換して開く経路に着く） | ver7_format_older_is_view_only | ver7_view_only_order_follows_the_contract, ver7_format_older_is_view_only, view_only_reasons_are_non_empty_and_classified | DETECTED | - |
| VER7-3 | crates/core/src/history_open.rs: エンジンの主版の違いを判定しない | ver7_engine_major_is_view_only | ver7_engine_major_is_view_only, ver7_view_only_order_follows_the_contract, view_only_reasons_are_non_empty_and_classified | DETECTED | - |
| VER7-4 | crates/core/src/history_open.rs: format_older を engine_major の後へ回したのと同じ効果（判定の順序を崩す） | ver7_view_only_order_follows_the_contract | ver7_view_only_order_follows_the_contract | DETECTED | - |
| VER6-1 | crates/core/src/history_open.rs: 教材の主版の違いを判定しない（主版の違う履歴が変換して開く経路に着く） | fail_closed_for_versions_that_must_not_open | ver6_content_major_boundaries, fail_closed_for_versions_that_must_not_open, ver6_content_major_with_removed_step_is_view_only_at_any_position, ver7_view_only_order_follows_the_contract, view_only_reasons_are_non_empty_and_classified | DETECTED | - |
| VER7-1 | crates/core/src/history_open.rs: 新しい IR の構造の版の履歴を拒否しない（エンジンの最大版・開く教材の IR の版のどちらを超える場合も対象） | fail_closed_for_versions_that_must_not_open | fail_closed_for_versions_that_must_not_open, history_ir_format_newer_than_opening_pack_is_refused | DETECTED | - |

### 5.61 開かない版の違いの変異と線引きの照合（TASK-174 3/3）

`format_newer`・`vocab_newer`・`engine_newer` の判定（`crates/core/src/history_open.rs`）に、`VER7-5`〜`VER7-10` の 6 件を置いた。
`VER7-5` は語彙の判定の欠落、`VER7-6` は開く側の語彙の条件、`VER7-7` は副版・修正版の新しさ、`VER7-8` はエンジンの上限側の条件、
`VER7-9` は等しい版の境界、`VER7-10` は判定の順序の崩れを狙う。既存の `VER7-1`（開く教材側の条件）の退行も併せて確かめた。

```text
実行日: 2026-10-16
元にしたコミット: 841fd59-dirty
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/VER7-5.mutation crates/core/tests/mutations/VER7-6.mutation crates/core/tests/mutations/VER7-7.mutation crates/core/tests/mutations/VER7-8.mutation crates/core/tests/mutations/VER7-9.mutation crates/core/tests/mutations/VER7-10.mutation crates/core/tests/mutations/VER7-1.mutation
```

| ID | 期待した失敗 | 結果 |
|---|---|---|
| VER7-5 | ver7_vocab_newer_is_refused | DETECTED |
| VER7-6 | ver7_vocab_newer_on_the_opening_side_is_refused | DETECTED |
| VER7-7 | ver7_engine_newer_is_refused | DETECTED |
| VER7-8 | ver7_format_newer_is_refused | DETECTED |
| VER7-9 | ver7_vocab_newer_is_refused | DETECTED |
| VER7-10 | ver7_stage6_order_follows_history_load_order | DETECTED |
| VER7-1 | fail_closed_for_versions_that_must_not_open | DETECTED |

#### 線引きと決定の記録（D-52・D-59）の照合

| 理由コード・結果 | 根拠 | 照合の結果 |
|---|---|---|
| `content_major`（表示だけ。向きを問わない） | D-52 の 6（VER-18） | 一致する |
| 副版以下の違い（適用し直して写しと一致したときだけ変換して開く） | D-52 の 6（VER-4・VER-5） | 一致する |
| `format_older`（表示だけ） | D-52 の 7・D-59 の 2 | 一致する |
| `format_newer`（開かない） | D-59 の 2 | 一致する。ただし理由は Claude Code の判断で、ユーザーの理由としては記録されていない（D-59 の 4）。「開く側」をエンジンの上限と開く教材の両方に読むのは実装の読みである |
| `vocab_newer`・`engine_newer`（開かない）、`engine_major`（表示だけ） | VER-7（PoC-6 の試作の線引き。V-iv-2・V-v-2） | D-52・D-59 はこの 3 種を直接は決めていない。VER-7（確定）と VER-18 の「試作のとおり」に従う |

エンジンが読める語彙の上限の正は core の `VOCAB_MAX` で、pack の `vocab::LATEST` との一致は `crates/pack/tests/vocabulary_terms.rs` が見る。

### 5.62 ツリーの属性の計算と全項目が開いた表示（TASK-472）

ツリーの位置の計算（`tree_attrs`）、親の `role=tree`（`artifact_view::tree_view_list`）、a11y 契約の版 2（`treeitem` の 3 属性・`aria-expanded` は `true` のみ）を
次の 3 層で確かめた。

- native: `tree_attrs_contract`（試作の正解値・端の場合・長さ 0〜6・深さ 0〜3 の全数を素朴な O(n^2) の数え方と照合・性質・前後で表示行が変わる例・親の `role` の 4 条件の表・変種 (a) の対照）と、
  `a11y_tree_contract`（版 2 の 4 規則の負の対照 13 件と正の見本）
- ブラウザ: `tools/browser/tree-attrs.test.mjs`（ファイルツリー・タスクツリーの (b) で `role=tree` が 1 個・`treeitem` の数・3 属性の値の表・全項目が見えること・安全性の計数 0、(a) の対照、範囲外の引数は空文字列）。
  `make test-browser` は 122 件が通過した（2026-10-16）。読み上げは対象外で、A11Y-14・TASK-381 の人の確認で扱う
- 変異: `ART9-1`（兄弟の位置を常に 1 にする）・`ART9-2`（親の `role=tree` を別の属性名にする）・`ART13-1`（子を持つ項目を閉じた状態で出す）。既存の `ART5-1` の退行も併せて確かめた

```text
実行日: 2026-10-16
元にしたコミット: 841fd59-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART9-1.mutation crates/web/tests/mutations/ART9-2.mutation crates/web/tests/mutations/ART13-1.mutation crates/web/tests/mutations/ART5-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| ART9-1 | crates/web/src/tree_attrs.rs: 兄弟の中の位置を常に 1 にする（aria-posinset が壊れ、支援技術が項目の順を読めない。ART-9） | 試作の正解値と一致する, 全数照合_素朴な数え方と一致する | 全数照合_素朴な数え方と一致する, 前後で表示行と兄弟の数が変わる, 端の場合, 試作の正解値と一致する, 長い並びでも_panic_しない | DETECTED | - |
| ART9-2 | crates/web/src/artifact_view.rs: 親の role=tree を別の属性名に変える（子の treeitem が木の外に出て、支援技術が木として読めない。ART-9） | 親の_role_は_4_条件がそろったときだけ付く, 変種_b_のファイルツリーとタスクツリーは全項目が開いた木になる | 親の_role_は_4_条件がそろったときだけ付く, 変種_b_のファイルツリーとタスクツリーは全項目が開いた木になる | DETECTED | - |
| ART13-1 | crates/web/src/tree_attrs.rs: 子を持つ項目を閉じた状態で出す（開閉の口が無いので、子が読み上げから隠れたままになる。ART-13） | 変種_b_のファイルツリーとタスクツリーは全項目が開いた木になる | 前後で表示行と兄弟の数が変わる, 変種_b_のファイルツリーとタスクツリーは全項目が開いた木になる | DETECTED | - |
| ART5-1 | crates/web/src/artifact_view.rs: 選ばれていないビューに hidden を付けない（ART-12。隠すべきビューが見えたままになる） | 親の属性の表, view_list_の選択と件数 | view_list_の選択と件数, 親の属性の表 | DETECTED | - |
