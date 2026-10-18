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
  最後まで流した結果と一致する。ケースごとに t を 1 つ引く）を、固定・誘導つきの生成で各 1,000 通り確かめる。後者は TASK-320 1/2 で
  再読み込みを挟む形（H-5。世代 +1・`Reloaded`・`reload` の記録）へ格上げした（5.65）。12 本あわせた実測は dev プロファイルで約 4.4 秒（2026-10-16、`cargo test` の試験の所要時間）
- 復元の性質と変異 6/6（TASK-316 3/3、SAVE-1）は、同じ 2 つの試験ファイルに足した。各枠の下書きは 0〜2 件で引き、送信を挟まず連続する
  下書きを列の途中にも出す。試作の H-2（写しから作り直した実行が写しを返す）・H-7（同じ包みの書き出しが同じバイト列）を書き出し→読み込みの
  性質テストの中で確かめ、H-8（`ops` を 1 件消した包みは、開けるなら写しと一致し、開けないなら `replay_mismatch`）を固定・誘導つきで各 1,000 通り
  確かめる。H-6（再読み込みの前の宛先の進行の無視）は TASK-320 2/2 で足した（5.67）。14 本あわせた実測は dev プロファイルで約 4.8 秒（2026-10-16、
  `cargo test` の試験の所要時間）。変異は 5.59 節。自動ブラウザでの「`ops` だけで復元した最終の世界状態の一致」は、web の保存・読み込みの結線が
  未実装のため未確認
- `guided_generation_reaches_deeper_states_than_fixed_generation` は、既定の種に固定して固定・誘導の深さを比べる網羅性の試験
  （環境変数を受けない）。実測（2026-10-16、1,000 通り）は、確認による達成が固定 29 件・誘導 1,075 件、最後の Step（`main/st-3`）が
  達成まで届いた列が固定 28 通り・誘導 509 通り、達成の直後に同じ操作で次の Step へ入った件数が固定 150 件・誘導 1,083 件。章の完了
  （`ChapterCompleted`）はエンジンが未実装（TASK-233 以降）のため、代わりに最後の Step の達成を指標にした。下限は
  確認による達成 500 件超・最後の Step の達成 200 通り超
- 再読み込みを混ぜた列（TASK-221 1/2、SAVE-4）は `op_gen.rs` の `OpSpec::Reload` と、`op_specs_with_reload`・`guided_op_specs_with_reload` が出す。
  `property-test.md` の操作の表には無く、表の外から足したものである。既存の戦略は変えていない（種ごとの列と記録を保つため）。
  `op_sequence_properties.rs` と `op_sequence_replay.rs` に各 2 本の性質テスト（1,000 通り）と、固定列・既定の種に固定した網羅の試験を足した（5.68）。
  4 本あわせて約 8 秒（2026-10-16、dev プロファイル）

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
| `id` | 必須 | 変異の ID。サンプルは `S1`・`S2`。TASK-220 は `M1` 以降、TASK-244 は `X1` 以降、TASK-158 は `L1`、TASK-81 は `G1`・`G2`、TASK-180 は `H1`、TASK-290 は `GUIDE17-1`、TASK-256 は `J1`、TASK-291 は `GUIDE18-1`、TASK-309 は `SAVE2-1` 以降、TASK-52 は `PACK20-1` 以降、TASK-53 1/2 は `PACK21-1`・`PACK21-2`、TASK-53 2/2 は `PACK22-1`〜`PACK22-3`、TASK-47 は `PACK13-1` 以降・`PACK23-1`、TASK-262 は `JUDGE1-1` 以降、TASK-215 は `EVT13-1` 以降、TASK-263 は `JUDGE1-C1`・`JUDGE1-U1`・`JUDGE1-A1`・`JUDGE1-Q1`、TASK-266 は `JUDGE2-1` 以降、TASK-45 は `PACK11-1` 以降、TASK-46 は `PACK15-1` 以降・`PACK16-1` 以降・`PACK12-1` 以降、TASK-272 は `JUDGE6-1` 以降、TASK-305 は `GUIDE18-2` 以降、TASK-207 は `EVT3-1` 以降、TASK-268 は `JUDGE4-1` 以降、TASK-208 は `EVT5-1` 以降、TASK-209 は `EVT7-1` 以降、TASK-269 は `JUDGE5-1` 以降、TASK-315 は `SAVE24-1` 以降、TASK-210 は `EVT6-1` 以降、TASK-310 は `SAVE14-1`・`SAVE15-1` 以降・`SAVE19-1` 以降、TASK-343 は `FEND20-1`・`FEND15-1`・`FEND21-1`、TASK-359 は `FEND15-2`〜`FEND15-4`、TASK-274 は `JUDGE8-1` 以降、TASK-257 は `JUDGE14-1`・`JUDGE17-1`・`JUDGE19-1`、TASK-211 は `EVT9-1` 以降、TASK-213 は `EVT11-1` 以降、TASK-212 は `EVT10-1` 以降、TASK-214 は `EVT12-1` 以降、TASK-229 は `EVT18-1`、TASK-275 は `JUDGE13-1`、TASK-278 は `JUDGE19-2`・`JUDGE19-3`、TASK-277 は `JUDGE17-2`〜`JUDGE17-4`、TASK-282 は `JUDGE18-2`・`JUDGE21-2`、TASK-316 は `SAVE1-1` 以降、TASK-467 は `ART3-1`・`ART2-1`・`ART5-1`・`ART11-1`・`ART7-1`、TASK-468 1/2 は `ART2-2`〜`ART2-4`、TASK-469 1/2 は `ART3-2`・`ART3-3`、TASK-469 2/2 は `ART3-4`、TASK-474 1/2 は `A11Y22-1`・`ART9-3`、TASK-474 2/2 は `A11Y22-2`・`ART9-4`・`ART9-5`、TASK-475 1/2 は `ART8-1`・`ART8-2`、TASK-477 1/2 は `ART17-1`〜`ART17-3`、TASK-477 2/2 は `ART17-4`〜`ART17-6`、TASK-174 は `VER3-1`・`VER4-1`・`VER4-2`・`VER6-1`・`VER7-1`、TASK-174 2/3 は `VER6-2`・`VER6-3`・`VER7-2`〜`VER7-4`、TASK-174 3/3 は `VER7-5`〜`VER7-10`、TASK-349 は `FEND4-1`〜`FEND4-3`、TASK-472 は `ART9-1`・`ART9-2`・`ART13-1`、TASK-331 1/3 は `SAVE19-4`〜`SAVE19-6`、TASK-317 は `SAVE8-1` 以降、TASK-317 2/2 は `SAVE9-1` 以降、TASK-330 1/2 は `SAVE17-1`、TASK-221 1/2 は `EVT8-1`・`EVT8-2`、TASK-221 2/2 は `EVT8-3`、#1413 は `MOD4-1` を、TASK-57 1/2 は `PACK5-1` を、TASK-57 2/2 は `PACK5-2`・`PACK5-3`・`PACK25-1` を、TASK-58 2/2 は `PACK30-1` を、TASK-61 は `PACK27-1`〜`PACK27-3` を、TASK-66 1/2 は `PACK38-1`・`PACK38-2` を、#1417 は `SAVE14-2`・`SAVE14-3`・`SAVE15-2`・`SAVE32-1`・`SAVE32-2` を使う |
| `package` | 必須 | cargo のパッケージ名（`^[a-z0-9-]+$`） |
| `test` | 必須 | 試験のターゲット名。`--test` に渡す（`^[a-z0-9_]+$`） |
| `target` | 必須 | 書き換えるファイル。repo 根からの相対パス。絶対パス・`..`・symlink は拒否する |
| `find` | 必須 | `target` にちょうど 1 回現れる 1 行の文字列。0 回・2 回以上は定義の不正 |
| `replace` | 必須 | 置き換え後の文字列（1 行） |
| `features` | 任意 | 試験に付ける cargo の feature（`^[a-z0-9-]+$`、1 つ）。`test-hooks` の背後にある試験が対象のときに使う（TASK-221 2/2） |
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
新しい節の番号は、rebase した時点の `origin/main` の最大の番号の次を使う。マージの後に重複が残った場合は、既存の節の番号を変えず、後から来た節に英小文字の枝番（`b`・`c`…）を付ける。

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

### 5.3.1a 右のログに入力が文字として出ること（TASK-434 2/2）

TERM21-1 は、`crates/web/src/learner_text.rs` の `learner_part`（右のログの行の本文）を生 HTML の出力口に替える変異である。
native の適合試験 `log_lines_contract` が検出する。同じ修正で、SAFE11-1 の `find` を `learner_line` にだけ当たる文字列に直した
（`learner_part` の追加で `vec![text(s)]` が 2 か所になり、変異の一意性の検査に通らなくなっていた）。

実ブラウザでの確認は `tools/browser/log-lines-xss.test.mjs` が担う。`testdata` の `xss-cases.md` のうち入り口に E5 を含む 49 件を、
「そのまま」「`/` の後ろ」「コマンドの引数」の 3 通りで送り（147 回。TASK-147 の SAFE-5 と同数）、`received` の行の本文が文字ノード
1 つだけで送った文字列と一致すること、危険な要素・実行・外部通信・CSP 違反・ダイアログ・ポップアップ・ページの例外が 0 件であることを、
CSP あり・なしで確かめる。作り直し（`rerender`）の後も確かめる。この段に限り、U+0000 を除いた文字列との一致も許す（HTML の
構文解析が NUL を落としうるためで、表示の安全性とは関係しない）。

- 注入の口は、試験の設定の `raw-trigger`。TASK-438（入力欄からエンジンへの結線）が入ったため `sendPayload` を入力欄の操作に差し替えられるが、別の Issue に回している
- 拒否の行（`{{input_head}}` の差し込み）は TASK-437 1/2 で観察の対象へ足した（受付の行に続く `rejected` の行の子が文字ノードだけであること）。問い返しの行は、試験の設定の mount に問い返しを持つ教材が無いため未対応（5.3.1d）
- 教材の文面の網羅は E2（TASK-146 系）の担当。全 6 種の行を文字ノードで描くことは `log_lines_contract` が代わる

```text
実行日: 2026-10-16
元にしたコミット: e6a486b-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/SAFE11-1.mutation crates/web/tests/mutations/TERM21-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| SAFE11-1 | crates/web/src/learner_text.rs: 受講者の文字列を生 HTML として出す（`find` を一意に直した再確認） | 記法と_html_は文字のまま描かれ_子の要素にならない | 左の 1 件に加え、web_のソースは_raw_html_を使わない | DETECTED | - |
| TERM21-1 | crates/web/src/learner_text.rs: ログの行の本文を生 HTML として出す | html_風の文字列は要素にならずエスケープされる | 左の 1 件に加え、合成した全種の行の適合の 2 件 | DETECTED | - |

### 5.3.1b 送信の結線（TASK-438。TERM-1・TERM-2）

TERM1-1 は `crates/web/src/host.rs` の送信で下書きを空にする呼び出しを消す変異、TERM2-1 は送信の本文の改行を潰す変異である。
native の適合試験 `submit_wiring` が検出する。

実ブラウザでの確認は `tools/browser/submit-wiring.test.mjs` が担う。Enter と送信ボタンの click が、エンジンの記録（本文つきの
`InputReceived`）と右のログの行で一致すること、拒否のときだけ下書きが残ること、Shift+Enter が送信せず改行を入れること、
最後の Enter で改行を保った 1 件になり、ログの本文が文字ノード 1 つのまま改行を保つことを、CSP あり・なしで確かめる。
`<textarea>` への書き戻しは非同期（`set_timeout` の 0 ミリ秒）のため、欄の値は空になるのを待って読む。

- hello の `log-list` の退役は TASK-517 に回した（配信設定には教材が無く、エンジンのログが空のため）
- 問い返しの後の欄の扱いは JUDGE-27・TERM-22（検討中）に属し、固定しない（拒否のときだけ残す）
- IME の模擬（試験文 30 文）は TASK-276・TASK-439 の担当

```text
実行日: 2026-10-16
元にしたコミット: b6d5b51-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/TERM1-1.mutation crates/web/tests/mutations/TERM2-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| TERM1-1 | crates/web/src/host.rs: 送信を受け入れた後に下書きを空にしない | 拒否されない送信は本文つきで記録され_下書きが空になり_書き戻しが要求される | 左の 1 件に加え、session なし・空白だけの 2 件 | DETECTED | - |
| TERM2-1 | crates/web/src/host.rs: 送信の本文の改行を潰す | 複数行の本文は改行を保った_1_件として記録され_ログの行にも保たれる | 左の 1 件 | DETECTED | - |

### 5.3.1c 拒否の理由と次の行動が右のログに出ること（TASK-437 1/2）

TERM32-1 は、`crates/web/src/host.rs` の `record_judged_texts` が拒否の理由の文面を控える処理を無効にする変異である。
理由の行が出なくなり、native の `crates/web/tests/reject_lines.rs` が検出する。
TERM32-2 は、`crates/web/src/host.rs` の `install_session`（履歴の差し替え）が拒否の文面の控えを新しいログから作り直す処理を無効にする変異である。
控えのキーはイベントの位置のため、差し替え前の位置の文面が残り、新しいログの拒否の行が出なくなる。`reject_lines.rs` の
`open_history` で拒否の位置が違う履歴を取り込む試験が検出する。

誤入力の出どころは、spec の「47 件」（private の試作パック）ではなく、repo 内のパックの `error_inputs`（期待が `rejected:*` のもの）である。
試作パックは公開の可否（D-11・TASK-14）が未決で、`testdata/README.md` のとおり取り込んでいない。native の試験は
`min-3step` 7 件・`judgment` 17 件・`module-reuse` 8 件の計 32 件（件数は定数で固定）と、`fill-pack` の `text_checks` のうち拒否のもの
（期待の文面と次の行動つき）を `EngineHost` に流す。`slot-pack` の誤入力は静的な検査用で、実行して期待の分類にならないため使わない。
ブラウザ試験 `tools/browser/reject-lines.test.mjs` は `min-3step-pack` の 7 件を、CSP あり・なしで流す。
いずれも、拒否 1 件につき `rejected` の行が 1 件、理由と次の行動が空でなく、表示がエンジンの値と一致することを確かめる。

```text
実行日: 2026-10-16
元にしたコミット: abb853e-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/TERM32-2.mutation crates/web/tests/mutations/TERM32-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| TERM32-1 | crates/web/src/host.rs: 拒否の理由の控えを作らない（行が出ない） | 誤入力の拒否は理由と空でない次の行動の行として出る | 左の 1 件に加え、fill_pack_の拒否は期待の文面と次の行動のとおりに出る・履歴の差し替えで拒否の文面の控えは新しいログから作り直される の 2 件 | DETECTED | - |
| TERM32-2 | crates/web/src/host.rs: 履歴の差し替えで拒否の文面の控えを作り直さない（差し替え前の位置の控えが残る） | 履歴の差し替えで拒否の文面の控えは新しいログから作り直される | 左の 1 件 | DETECTED | - |

### 5.3.1d 問い返しが拒否と区別されて右のログに出ること（TASK-437 2/2）

TERM32-3 は、`crates/web/src/host.rs` の `keeps_text`（文面を控える判定の選別）が問い返しを外す変異である。
問い返しの文面の控えが作られず行が出なくなり、native の `crates/web/tests/clarify_lines.rs` が検出する。
TERM32-4 は、`crates/web/src/log_lines.rs` で問い返しの行の種類を `rejected` と同じにする変異である（拒否との区別の喪失）。
`log_lines_contract` の種類の試験が検出する。

native の試験 `clarify_lines` は `ch1-pack` の `main/st-1-4` で、引数なしの `/new-idea`（問い返し）と順序違いの `/advance-phase`
（`rejected:wrong_order`）を `EngineHost` に流す。問い返しの行（子は `line-question`・`line-next-action`、`data-category` なし）が
拒否の行の件数に数えられないこと、拒否の行が入力の行の直後に出ること、復元と履歴の差し替えで控えが作り直されることを確かめる。

- 212 文字の応答は試作パック（公開の可否が未決。D-11・TASK-14）で測った値で、実文は repo に無い。`ch1-pack` の ST-1.4 と、240 文字の合成の理由・問い（`log_lines_contract`。切り詰めず文字ノード 1 つで出る）で代える
- 問い返しのブラウザ試験は未対応。試験の設定の mount は `min-3step` の IR を埋め込み、その `clarifications` は空で、`open_history` は IR を固定するため、問い返しを起こせない。別の Issue で扱う

```text
実行日: 2026-10-16
元にしたコミット: 2bf37a9-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/TERM32-3.mutation crates/web/tests/mutations/TERM32-4.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| TERM32-3 | crates/web/src/host.rs: 問い返しの文面の控えを作らない（行が出ない） | 引数なしの_new_idea_は_clarified_の行になり_拒否の行に数えない | 左の 1 件に加え、順序違いの_advance_phase_は…・復元でも問い返しの行が同じ文面で出る の 2 件 | DETECTED | - |
| TERM32-4 | crates/web/src/log_lines.rs: 問い返しの行の種類が拒否と同じになる（区別の喪失） | 拒否と問い返しは文面が渡されたときだけ行になり_種類で区別される | 左の 1 件に加え、合成した全種の行が_log_list_の下で_check_area_と_check_view_を通る の 2 件 | DETECTED | - |


### 5.3.1e 選択肢のクリックが core の判定に届くこと（TASK-445 2/2）

TERM24-1 は、`crates/web/src/host.rs` の `decode_action` が `choose`（選択肢のクリック）を `Op::Input(Input::Choice)` に変える処理を無効にする変異である。
ボタンを押しても core へ届かず、誤答の理由も達成も起きなくなり、native の `crates/web/tests/choice_answer.rs` が検出する。
TERM24-2 は、`crates/web/src/offers.rs` の `choice_button` から `data-action` を外す変異である（クリックの宛先が無くなる）。

mount の教材（`min-3step`）の確認には `answer` の Step が無く、そのままではブラウザ試験を書けない。mount の教材は既存の試験が st-1 の
`point_at` などに依るため変えず、試験の設定だけの `raw` の接頭辞 `load-fixture:answer` で、st-1 の確認を `answer`（誤り 1・正解 1）に
置き換えた教材へ差し替える（`test_fixture::answer_session`）。native の試験は `--features test-hooks` を付けたときだけ差し替えを確かめる。
ブラウザ試験 `tools/browser/choice-answer.test.mjs` は、実クリックで誤答 → 選び直し → 達成を CSP あり・なしで流す。Step の状態表示
（`step-status`）の語は `confirm-status.test.mjs`（5.116）が確かめるため、ここでは達成を復習の `data-achieved`・エンジンの書き出し・ボタンの消滅で確かめる。

```text
実行日: 2026-10-16
元にしたコミット: e8bffb8-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/TERM24-1.mutation crates/web/tests/mutations/TERM24-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| TERM24-1 | crates/web/src/host.rs: choose の変換を無効にする | choose_は_id_を_choice_の_op_にし_空の_payload_は何もしない | 左の 1 件に加え、誤答・正答・偽造 ID の 4 件 | DETECTED | - |
| TERM24-2 | crates/web/src/offers.rs: ボタンから data-action を外す | ボタンの_action_と_payload_は選択肢の_id_で_文字には_id_が出ない | 左の 1 件 | DETECTED | - |

### 5.3.1f 承認のボタンのクリックが core の判定に届くこと（TASK-447 1/2）

TERM27-1 は、`crates/web/src/host.rs` の `decode_action` が `revise-target`（対象つきの「直す」のクリック）を
`Op::Input(Input::Approval)` に変える処理を無効にする変異である。対象つきの「直す」が core へ届かず、受付にならない。
TERM27-2 は、`crates/web/src/offers.rs` の `approval_offers` が対象つきのボタンをすべて落とす変異である
（規則に書かれた対象の数だけ出る受入を壊す）。どちらも native の `crates/web/tests/approval_buttons.rs` が検出する。

ブラウザ試験 `tools/browser/approval-buttons.test.mjs` は、試験の設定だけの `load-fixture:approval`（対象 `t-alpha`・`t-beta`）で、
実クリックの拒否（対象なしの「直す」）と、対象つきの「直す」による確認の段への切り替えを CSP あり・なしで流す。

```text
実行日: 2026-10-17
元にしたコミット: 665042c-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/TERM27-1.mutation crates/web/tests/mutations/TERM27-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| TERM27-1 | crates/web/src/host.rs: revise-target の変換を無効にする | revise_target_は対象をそのまま_op_にし_空は何もしない | 左の 1 件に加え、受付・確認の段・拒否の流れの 4 件 | DETECTED | - |
| TERM27-2 | crates/web/src/offers.rs: 対象つきのボタンを出さない | 入力の段は三つと対象の数だけのボタンが出る | 左の 1 件に加え、重複・太い形・拒否・エスケープの 5 件 | DETECTED | - |

### 5.3.1g 承認の操作のログ文言と CH-2 相当の完走（TASK-447 2/2）

TERM27-3 は、`crates/web/src/log_lines.rs` の承認の操作の行から接頭辞「承認の操作: 」の文字ノードを落とす変異である。
行の文字が spec の文言（`承認の操作: 直す（対象: t-alpha）` 等）にならなくなり、native の `crates/web/tests/approval_flow.rs` が検出する。

ブラウザ試験 `tools/browser/approval-flow.test.mjs` は、本 repo に CH-2 の教材が無いため、ST-2.6 と同じ構造の承認の Step
（`load-fixture:approval` の `st-1`）から、選択の `st-2`、最終の承認の `st-3` までを CSP あり・なしで流す。
`st-2` は入力方式 `choice` の画面の口が TASK-450 の担当で未実装のため `Op` で送る。章の完了（`ChapterCompleted`）は core が
未実装（TASK-233 以降）のため、最後の Step の達成までを確かめる。

```text
実行日: 2026-10-17
元にしたコミット: 8c36c5c-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/TERM27-3.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| TERM27-3 | crates/web/src/log_lines.rs: 承認の操作の行の接頭辞を落とす | 承認の操作の行は文言になり_はいで先へ進んで最後の_step_まで通る | 左の 1 件 | DETECTED | - |

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

### 5.7b 判定の型の次の行動（TASK-256）

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

#1398 で PACK13-1 の `find` を `check_confirms` の現在の行に合わせ直した再確認。

```text
実行日: 2026-10-17
元にしたコミット: b7489c6-dirty（本変更を含む作業ツリー）
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

### 5.16b 選択肢・画面操作・承認の判定（TASK-263）

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

### 5.18b 誤答ごとの拒否文面 `on_wrong`（TASK-46 1/2）

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

#1518 で EVT7-4 の `find` を `apply` の `record_changes` の行に移した再確認。`reload`（#1478、TASK-320 1/2）が同じ `state.log.extend(...)` の行を持ち、`find` が 2 回一致して検査が止まっていた。`replace` はその行の後ろで先頭を除いてログへ足して返すため、元の変異と同じ誤り（ログが 1 件少なく、戻り値は変わらない）になる。到達しなくなる元の 2 行に `unreachable statement` の警告が出るが、検査はコンパイルの成否だけを見る。

```text
実行日: 2026-10-17
元にしたコミット: 6deaec1-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT7-4.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| EVT7-4 | crates/core/src/session.rs: 1 回の操作のイベントがログへすべて記録されない（連打 20 回でログが 40 件にならない） | burst_of_20_on_remaining_button_rejects_all_and_keeps_state, p6_remaining_button_burst_keeps_state | burst_of_20_on_remaining_button_rejects_all_and_keeps_state, p6_remaining_button_burst_keeps_state | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: 1 |

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

### 5.64 keyed list の上限と `Err` の代替（TASK-359 2/2）

FEND15-4 は、`Err` の代替（`keyed_view_or_notice`）から `data-bind-list` を外して空の要素に落とす。
上流は dirty な field を束縛点にも keyed list にも解決できないと全再描画に落ちるため、ログが 4,097 行になった直後の描画で、親が 1 つでないこと・dirty な field が束縛点として解決できないことを `list_limit` が検出する。

```text
実行日: 2026-10-16
元にしたコミット: 6d6b1ca-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/FEND15-4.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| FEND15-4 | crates/web/src/list_policy.rs: Err の代替から data-bind-list を外す（dirty な field が解決できず全再描画になる） | limit_4097_fallback_keeps_bind_list_with_zero_data_rows, limit_4097_host_log_does_not_trigger_full_rerender | limit_4097_fallback_keeps_bind_list_with_zero_data_rows, limit_4097_host_log_does_not_trigger_full_rerender | DETECTED | - |

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

### 5.39b 処理中の停止と入口の写しへの復帰（TASK-211 1/2）

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

### 5.58b 左の成果物ビューの型と印の対応（TASK-467 1/2）

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
X-1 は TASK-320 1/2 で `SAVE4-1` として置いた（5.66）。H-6 は TASK-320 2/2 で `SAVE4-4`・`SAVE4-5` として置いた（5.67）。
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

### 5.60b 左の成果物ビューの束縛点・指せる行・変種の口（TASK-467 2/2）

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

### 5.61b 進行と補完の配送（TASK-349 1/2）

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

### 5.60c 表示だけになる版の違いの変異（TASK-174 2/3）

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

### 5.61c 開かない版の違いの変異と線引きの照合（TASK-174 3/3）

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

### 5.62b ツリーの属性の計算と全項目が開いた表示（TASK-472）

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

### 5.63 履歴の読み込みの段 1〜3（TASK-331 1/3）

履歴の読み込みの大きさ・入れ子の深さ・JSON の段を `history_load::parse_history_json` に実装し、`history_load` の試験で確かめた。
本節の番号は、この文書の 5 節に既にある重複（5.7・5.16・5.18・5.39・5.60・5.61）を直さずに、既存の最大の次を使った。

`truncated` の位置は、実装を通して確かめた（2026-10-16）。結果は contract の仮置き（Json 段。`not_json` と同じ段）と一致したため、契約の版は上げていない。
判定は大きさ → 深さ → UTF-8 → JSON の解釈の順で、末尾が不完全な UTF-8 と入力の終わりで途切れた JSON（0 バイト・空白だけを含む）は `truncated`、
途中の不正なバイトと構文の誤りは `not_json` とする。spec 側への提案は、SAVE-19 の「文字コードの検査は順序列に含めない。位置は未確認」を、この規則に直すことである。
100MB の実バッファは作らず、1 バイト超過と 2 倍で確かめた（長さだけの判定は `history_size` の試験が持つ）。
画面の状態が変わらないことのブラウザの確認は、本節の範囲に含めない（未実施。後続で扱う）。

SAVE19-4 は文字の途中の切れを `not_json` にする誤り、SAVE19-5 は入力の終わりで途切れた JSON を `not_json` にする誤り、SAVE19-6 は大きさ・深さの段を外す誤りである。

```text
実行日: 2026-10-16
元にしたコミット: dbe9c07-dirty
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAVE19-4.mutation crates/core/tests/mutations/SAVE19-5.mutation crates/core/tests/mutations/SAVE19-6.mutation
```

| ID | 対象 | 結果 |
|---|---|---|
| SAVE19-4 | crates/core/src/history_load.rs: 文字の途中で切れた入力を truncated ではなく not_json にする | DETECTED（cut_inside_multibyte_char_is_truncated, utf8_precedes_parse） |
| SAVE19-5 | crates/core/src/history_load.rs: 入力の終わりで途切れた JSON を not_json にする | DETECTED（cut_at_char_boundary_is_truncated, empty_input_is_truncated, rejection_leaves_store_unchanged） |
| SAVE19-6 | crates/core/src/history_load.rs: 大きさ・深さの段を外す | DETECTED（over_by_one_is_too_large, depth_over_is_too_deep, size_precedes_depth ほか 13 件） |
| SAVE9-1 | crates/web/src/save_pending.rs: 同じ鍵の値を置き換えず、最初の値を残す（離れる直前の最新の変更が落ちる） | DETECTED（同じ鍵に_2_回積むと最後の値だけを_1_回書く） |
| SAVE9-2 | crates/web/src/save_pending.rs: 書けなかった鍵も保留から外す（失敗した未保存の値が失われる） | DETECTED（書き込みの失敗では保留に残り再びの_flush_で書ける） |
| SAVE9-3 | crates/web/src/save_pending.rs: 書けた鍵を保留から外さない（二重に書く） | DETECTED（flush_の後に保留は空になり_2_回目は書かない ほか 1 件） |

### 5.64b 履歴の読み込みの段 4〜6（TASK-331 2/3）

履歴の読み込みの版の欄・型・版の判定の段を `history_load::load_history`（段 1〜6 の入口）に実装し、`history_load` の試験で確かめた。
段 4 は `check_history_head`、段 5 の前半は `parse_history_envelope`、段 5 の後半と段 6 は既存の `open_history` を呼ぶ。
本節の番号は、5.63 と同じく既存の重複を直さずに、既存の最大の次を使った。

段 4 の `missing_version` は、`versions` の表の欠落に加えて `format` の違いと `history_format` の欠落を含める。
根拠は `version-expectations.md` の 1 節と試作の `open_history` で、実装側の読みである（【推論】）。spec 側への提案は、SAVE-19・VER-1 でこれを明記することである。
画面の状態が変わらないことのブラウザの確認は、web に履歴を読み込む経路が無いため未実施（TASK-531・TASK-532・TASK-176 の後）。
段 7・8 の試験・所要時間・契約適合は 5.65（TASK-331 3/3）で扱う。

SAVE19-7 は版の欄の段（`versions` の表の検査）を外す誤り、SAVE19-8 は `format` の照合を外す誤り、SAVE19-9 は型の失敗を `missing_version` にする誤り、
SAVE19-10 は `ops` の件数の比較を `>=` にする誤り、SAVE19-11 は件数の検査を外す誤りである。

```text
実行日: 2026-10-16
元にしたコミット: 6d6b1ca-dirty
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAVE19-7.mutation crates/core/tests/mutations/SAVE19-8.mutation crates/core/tests/mutations/SAVE19-9.mutation crates/core/tests/mutations/SAVE19-10.mutation crates/core/tests/mutations/SAVE19-11.mutation
```

| ID | 対象 | 結果 |
|---|---|---|
| SAVE19-7 | crates/core/src/history_load.rs: 版の欄の段（versions の表の検査）を外す | DETECTED（stage4_no_versions_poc_file, stage4_versions_not_a_table ほか 4 件） |
| SAVE19-8 | crates/core/src/history_load.rs: format の照合を外す | DETECTED（stage4_format_missing_or_wrong, stages_4_to_6_follow_history_load_order） |
| SAVE19-9 | crates/core/src/history_load.rs: 型の失敗を missing_version にする | DETECTED（stage5_malformed, stage4_boundary_inner_defects_are_malformed ほか 4 件） |
| SAVE19-10 | crates/core/src/history_load.rs: ops の件数の比較を >= にする | DETECTED（stage5_ops_count_boundary, stage5_too_many_ops_through_load_history） |
| SAVE19-11 | crates/core/src/history_load.rs: ops の件数の検査を外す | DETECTED（stage5_ops_count_boundary, stage5_too_many_ops_through_load_history ほか 1 件） |

### 5.65 履歴の読み込みの段 7・8（TASK-331 3/3）

段 7（開始点）と段 8（適用し直しと写しとの照合）は `open_history` にあり、`history_load::load_history`（段 1〜8 の入口）から通して `history_load` の試験で確かめた。判定のコードは変えていない。
開く履歴は記録の経路（`RecordPath`）で作った。固定の履歴 `two-rounds` は `reload` を含み、再読み込みの意味の実装（TASK-320）まで同一版でも `replay_mismatch` になる（`reload_history_is_replay_mismatch_until_task320` で固定）。
19 種の理由コードがすべて `load_history` の 1 つの入口から返り、並びが `HISTORY_LOAD_ORDER` に沿うことは `all_nineteen_reasons_reach_the_single_entry` が見る。contract 側の適合試験は変更していない。
画面の状態が変わらないことのブラウザの確認は、web に履歴を読み込む経路が無いため未実施（TASK-531・TASK-532・TASK-176 の後）。本節の番号は、5.63・5.64 と同じく既存の重複を直さずに、既存の最大の次を使った。

適用し直しの所要時間（`max_ops_history_opens_within_a_loose_bound`・`replay_cost_grows_linearly_with_op_count`）。
spec SAVE-19 の実測（PoC-6 の `wrong_input` 29,138 件・9,937,328 バイトで 660ms）は現行の件数の上限（20,000 件）を超え、段 5 の `too_many_ops` で断たれるため再現できない。
計測値は機械に依存するので、試験は緩い上限（debug で 10 秒未満）と、件数が 4 倍のとき時間が 12 倍未満であること（二乗にならない）だけを表明し、実測は次に記録する。
履歴は誤った入力の送信を 20,000 件記録して作る（試験の準備に約 10 秒かかる。`make test` では流す）。

```text
実行日: 2026-10-16
ビルド: dev プロファイル（debug）・native
20,000 件・9,970,781 バイト: 約 330〜360ms（全段。適用し直しを含む）
5,000 件: 約 80ms
コマンド: cargo test -p fandhe-workshop-core --test history_load -- --nocapture max_ops replay_cost
```

変異は、SAVE19-12 が段 7 の開始点の検査を外す誤り、SAVE19-13 が段 8 を通らずに初期状態で開く誤り、SAVE19-14 が写しとの一致の判定を常に真にする誤りである。

```text
実行日: 2026-10-16
元にしたコミット: 73c5411-dirty
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAVE19-12.mutation crates/core/tests/mutations/SAVE19-13.mutation crates/core/tests/mutations/SAVE19-14.mutation
```

| ID | 対象 | 結果 |
|---|---|---|
| SAVE19-12 | crates/core/src/history_open.rs: 段 7 の開始点の検査を外す | DETECTED（stage7_start_missing, all_nineteen_reasons_reach_the_single_entry ほか 4 件） |
| SAVE19-13 | crates/core/src/history_load.rs: 段 8 を通らずに初期状態で開く | DETECTED（stage8_tampered_snapshot_is_replay_mismatch, all_nineteen_reasons_reach_the_single_entry ほか 14 件） |
| SAVE19-14 | crates/core/src/history_open.rs: 写しとの一致の判定を常に真にする | DETECTED（stage8_tampered_snapshot_is_replay_mismatch, stage8_diverged_needs_a_version_difference ほか 5 件） |

### 5.66 処理中の再読み込み（TASK-320 1/2）

処理中の再読み込みで世代を +1 し、処理中のまま続ける分岐を `reload` モジュール（`session::reload`・`Session::reload`）に実装した。
`apply()` の `Op` には含めず、`CONTRACT_VERSION` を 3 から 4 へ上げた（`Event::Reloaded` の追加）。
固定の試験は `crates/core/tests/reload_busy.rs`、性質テストは `history_verify` の `property_reload_at_any_point_*` を H-5 へ格上げした。
後者は、時点 t で再読み込みを挟んだ列を続けて流した結果と、時点 t の包みを書き出し→読み込みで復元して再読み込みしてから残りを流した結果が
一致すること、再読み込みを含む全体の包みの照合が通ること、再読み込みの後の世代が時点 t の世代 +1 であることを確かめる。
共有の関数が世代を上げ損ねると両経路が同じに壊れて一致するため、世代の増分は一致とは別に主張する。
再読み込みの前の宛先の進行の無視（H-6）と再読み込みの後の停止は TASK-320 2/2（5.67）で固定した。
自動ブラウザ（S-5）は、web に履歴を読み込んで復元する経路が無いため未実施（TASK-319・TASK-331 3/3 の後）。

SAVE4-1 は再読み込みで世代を上げない誤り（試作の X-1）、SAVE4-2 は適用し直しで `Reload` を飛ばす誤り、SAVE4-3 は記録側が `Reload` を残さない誤りである。
既存の SAVE1-1〜SAVE1-6 も同じ実行で流し、`replay` の変更で検出が失われていないことを確かめた。

```text
実行日: 2026-10-16
元にしたコミット: 8a9fede-dirty
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAVE4-1.mutation crates/core/tests/mutations/SAVE4-2.mutation crates/core/tests/mutations/SAVE4-3.mutation crates/core/tests/mutations/SAVE1-1.mutation crates/core/tests/mutations/SAVE1-2.mutation crates/core/tests/mutations/SAVE1-3.mutation crates/core/tests/mutations/SAVE1-4.mutation crates/core/tests/mutations/SAVE1-5.mutation crates/core/tests/mutations/SAVE1-6.mutation
```

| ID | 対象 | 結果 |
|---|---|---|
| SAVE4-1 | crates/core/src/reload.rs: 再読み込みで世代を上げない | DETECTED（reload_while_busy_keeps_busy_and_bumps_generation, two_reloads_add_two_generations ほか 3 件） |
| SAVE4-2 | crates/core/src/history_replay.rs: 適用し直しで Reload を飛ばす | DETECTED（property_reload_at_any_point_fixed, property_reload_at_any_point_guided ほか 1 件） |
| SAVE4-3 | crates/core/src/ops_record.rs: 記録側が Reload を残さない | DETECTED（property_reload_at_any_point_fixed, property_reload_at_any_point_guided） |
| SAVE1-1〜SAVE1-6 | crates/core/src/history_replay.rs ほか（5.58・5.59 の定義） | 6 件とも DETECTED |

### 5.67 古い宛先の進行の無視（TASK-320 2/2）

再読み込みの前に予約された進行の宛先（古い世代）が後から届いても、状態が変わらず `Ignored(Stale)` が 1 件だけ増えることを試験で固定した。
製品コードの動作は変えていない（`judge_tick` は世代の食い違いを既に `Stale` にし、`reload` は入口の写しに触れない）。変えたのは `reload.rs` のコメントだけである。
固定の試験は `crates/core/tests/reload_busy.rs` に足した。

- `stale_tick_after_reload_is_ignored_once`: 再読み込みの前の宛先で `Ignored(Stale)` が 1 件だけ増え（`at` は操作前の世代。宛先の世代を写さない）、ログ以外の状態が変わらず、新しい宛先で最後まで流した結果が再読み込みなしの経路と一致する
- `stale_ticks_from_before_two_reloads_are_each_ignored`: 再読み込みを 2 回挟み（間に 1 回進める場合と進めない場合）、どちらの前の宛先も 1 件ずつ無視される
- `stale_tick_after_reload_when_not_busy_is_stale`: 処理中でない状態でも、世代の照合が `NoProcessing` より先に当たる
- `stop_after_reload_returns_to_step_entry`: 再読み込みの後の停止が、再読み込みの前の入口の写しへ戻る（入口の写しは作り直されない）。停止の後も、停止の前の宛先は無視される
- `session_reload_then_apply_ignores_stale_tick`: `Session::reload` → `Session::apply` の経路でも純粋関数の経路と同じ結果になる

性質テスト H-6 は `history_verify` の `reload_at_any_point` に足した。時点 t が処理中なら、再読み込みの前の宛先を、再読み込みして得た状態の複製に送り、
`Ignored(Stale)` が 1 件だけで状態が変わらないことを確かめる。探りは `ops` に記録しない（片側だけに入れると A/B の一致が壊れ、両側に入れると H-5 の意味が濁るため）。
処理中でない時点は宛先が無いので飛ばす。探りの件数は成功条件にしない（成功条件は不変条件だけ）。
自動ブラウザ（S-5・S-6）は、web に履歴を読み込んで再読み込みする結線が無いため未実施（TASK-319・TASK-331 の後）。

SAVE4-4 は進行の照合から世代を外す誤り（固定の試験で検出）、SAVE4-5 は同じ誤りを性質テスト H-6 で検出することの確認、SAVE4-6 は再読み込みで入口の写しを作り直す誤りである。
既存の SAVE4-1〜SAVE4-3・SAVE1-1〜SAVE1-6 も同じ実行で流し、検出が失われていないことを確かめた。

```text
実行日: 2026-10-16
元にしたコミット: b3fec6a-dirty
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAVE4-1.mutation 〜 SAVE4-6.mutation と SAVE1-1.mutation 〜 SAVE1-6.mutation
```

| ID | 対象 | 結果 |
|---|---|---|
| SAVE4-1〜SAVE4-3 | 5.66 の定義 | 3 件とも DETECTED |
| SAVE4-4 | crates/core/src/judge.rs: 進行の照合から世代を外す | DETECTED（stale_tick_after_reload_is_ignored_once, stale_ticks_from_before_two_reloads_are_each_ignored ほか 3 件） |
| SAVE4-5 | crates/core/src/judge.rs: SAVE4-4 と同じ置換（H-6 の確認） | DETECTED（property_reload_at_any_point_fixed, property_reload_at_any_point_guided） |
| SAVE4-6 | crates/core/src/reload.rs: 再読み込みで入口の写しを作り直す | DETECTED（stop_after_reload_returns_to_step_entry ほか 2 件） |
| SAVE1-1〜SAVE1-6 | 5.58・5.59 の定義 | 6 件とも DETECTED |

### 5.68 履歴の書き出し（TASK-330 1/2）

書き出しは契約の包み（`ops` と写し。D-52・SAVE-17）を compact な 1 行の JSON にしたものである（`history_export::export_bytes`）。
往復（書き出し→読み込みの段 1〜5）・同じ包みから同じバイト列が出ること・最上位の欄・上限超過でも書き出しを拒まないことを `crates/core/tests/history_export.rs` が、
ファイル名の整え（`pack_id` は教材由来で信頼できない）を `crates/web/tests/history_export.rs` が、
ダウンロードで保存されたファイルが `ops` と写しを持つこと・外部への要求と CSP 違反が 0 件・URL（`location`・履歴・ハッシュ）が変わらないこと・破棄した `blob:` URL から保存できないことを `tools/browser/history-export.test.mjs` が確かめる。
`blob:` URL は CSP の `connect-src 'self'` で `fetch` が止まるため、読めるかどうかはダウンロードで確かめた。
ホストに受講中の `ops` の記録（`OpsRecorder`）がまだ無いため、ブラウザ試験は契約の試料（`two-rounds.json`）を書き出す。受講中の状態からの書き出しは TASK-319・TASK-531 の結線の後に確かめる。本節の番号は、既存の重複を直さずに、既存の最大の次を使った（5.66 は別の変更が先に取った）。

変異は SAVE17-1（書き出しを整形つきの JSON にする誤り）である。

```text
実行日: 2026-10-16
元にしたコミット: e6a486b-dirty
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAVE17-1.mutation
```

| ID | 対象 | 結果 |
|---|---|---|
| SAVE17-1 | crates/core/src/history_export.rs: 書き出しを整形つきの JSON にする | DETECTED（export_is_compact_single_line） |

#### 大きさの測定（SAVE-17・SAVE-26）

```text
実行日: 2026-10-16
元にしたコミット: e6a486b-dirty
ビルド: dev プロファイル（debug）・native（バイト数は profile に依らない）
コマンド: cargo test -p fandhe-workshop-core --test history_export -- --nocapture record_export_sizes
```

| サンプル | 全体（バイト） | `ops` | 写し | `ops` の件数 |
|---|---|---|---|---|
| empty | 783 | 2 | 503 | 0 |
| two-rounds | 8,964 | 1,166 | 7,520 | 28 |
| draft-rejected | 1,156 | 126 | 752 | 4 |

いずれも保存先の目安の 5MB（U-5・SAVE-26）と読み込みの上限 10MB を大きく下回る。
PoC-6 の基点（2 周分で 77,300〜81,432 バイト。`h-full.json` は 77,300 バイト）とは直接比べられない。基点は試作の教材（Step の実体 36）の 2 周分を、PoC 時点の写しの形（`snapshot` が `state`・`draft`・`draft_root`）で測った値で、今の `SessionEnvelope` としては読めない。
一方 `two-rounds` は 3 Step の教材の試料で、教材の大きさも写しの形も違う。基点の教材を今のエンジンで動かせるようになった後（実際の教材の全章は SAVE-27・PoC-14 の担当）に、同じコマンドで測り直す。

### 5.69 処理中の状態表示（TASK-436 1/2）

状態表示 `run-status` の導出（`run_status`）を実装した。処理中の Step（`run.busy` が `Some`）で「処理中」、それ以外と未読込で「待機中」にする。
停止（`Stopped`）は TASK-436 2/2 の範囲である。固定の試験は `crates/web/tests/run_status_contract.rs`、ブラウザ試験は `tools/browser/run-status.test.mjs` である。
TERM33-1 は処理中の判定を反転する変異で、入力の後に「処理中」へ、最後の進行の後に「待機中」へ変わることを確かめる試験が検出する。

```text
実行日: 2026-10-16
元にしたコミット: b3fec6a-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/TERM33-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| TERM33-1 | crates/web/src/run_status.rs: 処理中の判定を反転する（処理中でも「待機中」のままになり、待機中が「処理中」と表示される。TERM-33） | 入力で処理中になり_最後の進行の後に待機中へ戻る | 入力で処理中になり_最後の進行の後に待機中へ戻る | DETECTED | - |

### 5.69b 停止の操作と停止後のやり直し（TASK-436 2/2）

状態表示 `run-status` に「停止」を足し、Esc（処理中のみ）と停止ボタン（`stop-btn`）を core の停止（`Control::Stop`）へ結んだ。
導出は、処理中でなければログを末尾から読み、`Changed`・`Ignored`・`Warning` を読み飛ばして最初に当たったイベントが `Stopped` なら「停止」にする。
処理中でないときの Esc・停止ボタンはログを変えない（取り消しの入力への結線は対象外）。固定の試験は `crates/web/tests/run_status_contract.rs`、
ブラウザ試験は `tools/browser/stop-redo.test.mjs`（Esc・停止ボタンのそれぞれで、停止・入口の状態・効果の取り消し・再送信を確かめる）である。
TERM33-2 は停止の分岐を潰す変異、TERM33-3 は Esc の経路で処理中を常に偽にする変異で、どちらも `run_status_contract` が検出する。

```text
実行日: 2026-10-16
元にしたコミット: 317e86e-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/TERM33-2.mutation（TERM33-3 も同じ）
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| TERM33-2 | crates/web/src/run_status.rs: 停止の分岐を潰す（停止しても「待機中」のままになる。TERM-33） | 処理中の停止で_停止_になり_古い進行が届いても保たれる | esc_と停止ボタンは処理中だけ停止し_待機中ではログを変えない, 処理中の停止で_停止_になり_古い進行が届いても保たれる | DETECTED | - |
| TERM33-3 | crates/web/src/host.rs: Esc の経路で処理中を常に偽にする（Esc を押しても停止しない。TERM-33） | esc_と停止ボタンは処理中だけ停止し_待機中ではログを変えない | esc_と停止ボタンは処理中だけ停止し_待機中ではログを変えない | DETECTED | - |

### 5.70 再読み込みを含む列の性質（TASK-221 1/2）

TASK-219 の操作列に再読み込み（SAVE-4）を足し、停止・再送・Step の移動・保存・復帰と任意に混ぜた列でも、
再読み込みの前に予約された進行（古い世代の宛先）が後から届いたとき、状態が変わらず `Ignored` が 1 件だけ増えることを確かめた。
製品コード（`crates/core/src/**`）は変えていない。変えたのは試験のハーネスと検査器である。

- `OpSpec::Reload` を足した。再読み込みは `Op` ではないので `Applied::op` を `Option<Op>` にし、`None` を再読み込みとした。宛先の履歴（`seen`・`last_tick`）は再読み込みで変えない
- P-2: 古い進行の判定は変えていない。統計 `stale_after_reload`（操作前のログに、宛先の世代より大きい `new_generation` の `Reloaded` がある古い進行）を足した。出力と網羅の試験にだけ使う
- P-4: `gen_reload` の腕が再読み込みの操作で出るようになった。再読み込みは世代とログ以外を変えないことも確かめる
- 性質テスト: `properties_p1_to_p6_hold_with_reload_sequences`・`..._with_guided_reload_sequences`（P-1〜P-6）、`replay_from_start_matches_after_every_op_with_reload_sequences`・`..._with_guided_reload_sequences`（P-5。純粋関数の `session::reload` と `Session::reload` の一致）
- 固定列 `stale_ticks_after_reload_in_sequences_are_ignored`: 再読み込みの直前に予約されていた宛先が、再読み込みの後に `Ignored(Stale)` 1 件だけで状態を変えないことを列の中で直接確かめる
- 網羅 `reload_generation_exercises_stale_ticks_after_reload`: 既定の種・1,000 通りの実測（2026-10-16）は、固定が `gen_reload` 1,593 件・`stale_after_reload` 732 件、誘導つきが 1,070 件・387 件。下限は 500 件・200 件

EVT8-1 は進行の宛先の照合から世代を外す誤り、EVT8-2 は再読み込みで世代を上げない誤りである。
既存の M1〜M7・SAVE4-1〜SAVE4-6・SAVE1-1〜SAVE1-6 も同じ検査器とハーネスを通るため、検出が失われていないことを確かめた。

```text
実行日: 2026-10-16
元にしたコミット: e7aa95b-dirty
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EVT8-1.mutation crates/core/tests/mutations/EVT8-2.mutation
```

| ID | 対象 | 結果 |
|---|---|---|
| EVT8-1 | crates/core/src/judge.rs: 進行の宛先の照合から世代を外す | DETECTED（stale_ticks_after_reload_in_sequences_are_ignored, properties_p1_to_p6_hold_with_reload_sequences, properties_p1_to_p6_hold_with_guided_reload_sequences ほか 4 件。最小の反例 `[Cmd(0), Tick(StaleGen)]`） |
| EVT8-2 | crates/core/src/reload.rs: 再読み込みで世代を上げない | DETECTED（stale_ticks_after_reload_in_sequences_are_ignored, properties_p1_to_p6_hold_with_reload_sequences, properties_p1_to_p6_hold_with_guided_reload_sequences ほか 1 件。最小の反例 `[Reload]`） |
| M1〜M7・SAVE4-1〜SAVE4-6・SAVE1-1〜SAVE1-6 | 既存の定義 | 19 件とも DETECTED |

自動ブラウザの場面（S-5・S-6 系）のうち、再読み込みの後の実際のタイマーの進行は 5.72（TASK-221 2/2）で扱った。

### 5.71 停止・移動・復帰の直後の再読み込み（TASK-321 1/2）

停止・Step の移動・チェックポイントの復帰の直後に、保存の遅延を待たずに再読み込みしても、直後の状態へ戻り、操作の前の宛先の進行が状態を変えないことを固定した（SAVE-5。PoC-6 の S-6・S-7・S-19）。
製品コードは変えていない。復元した状態は、世代の +1 とログ末尾の `Reloaded` 1 件を除いて直後の状態と等しい。操作の前の宛先は `Ignored(Stale)` が 1 件ずつ増えるだけで、状態は変わらない（S-19 は復帰の前に予約された 2 件）。

| 試験 | 内容 |
|---|---|
| `crates/core/tests/reload_after_control.rs` の `reload_right_after_stop_restores_the_stopped_state` | S-6。処理中に停止し、記録 → 包み → 書き出し → 読み込みの段 1〜8 → 再読み込みを通す。処理中でなく Step の入口と等しい |
| 同 `reload_right_after_move_restores_the_moved_state` | S-7。処理中・処理中でないの 2 通りで、世代が移動の前 +2 になる |
| 同 `reload_right_after_restore_restores_the_restored_state` | S-19。復習・ログ・チェックポイントの一覧・下書きも等しい |
| 同 `session_entry_agrees_for_all_three_scenes` | `Session` の入口が純粋関数の経路と同じ状態になる |
| `crates/web/tests/reload_after_control_store.rs` | 3 場面を、`PendingSave` に積み、`flush`（`pagehide` 相当）で localStorage の偽物へ書き、新しい保存先から読み戻して `load_history` で開き、再読み込みする。`flush` までは保存先に何も書かれない |

自動ブラウザ（S-6・S-7・S-19）は未実施である。web に受講中の `ops` の記録・変更ごとの `PendingSave::stage`・読み込み時の `load_history` による復元と `Session::reload` の結線が無く、それは TASK-319 の範囲のため。
結線の後に `tools/browser/reload-after-control.test.mjs`（`scripts/test-browser.sh` に登録）として足す。確認の操作の連打（S-9）は TASK-321 2/2 で扱う。

SAVE5-1 は停止で入口の写しへ戻さない誤り、SAVE5-2 は移動で世代を上げない誤り、SAVE5-3 は再読み込みでチェックポイントの一覧を消す誤りである。
既存の SAVE4-1〜SAVE4-6・SAVE1-1〜SAVE1-6 も同じ実行で流し、検出が失われていないことを確かめた。

```text
実行日: 2026-10-16
元にしたコミット: b6d5b51-dirty
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAVE5-{1,2,3}.mutation crates/core/tests/mutations/SAVE4-{1,2,3,4,5,6}.mutation crates/core/tests/mutations/SAVE1-{1,2,3,4,5,6}.mutation
```

| ID | 対象 | 結果 |
|---|---|---|
| SAVE5-1 | crates/core/src/stop.rs: 停止で入口の写しへ戻さない | DETECTED（reload_right_after_stop_restores_the_stopped_state） |
| SAVE5-2 | crates/core/src/go_to.rs: 移動で世代を上げない | DETECTED（reload_right_after_move_restores_the_moved_state） |
| SAVE5-3 | crates/core/src/reload.rs: 再読み込みでチェックポイントの一覧を消す | DETECTED（reload_right_after_restore_restores_the_restored_state） |
| SAVE4-1〜SAVE4-6・SAVE1-1〜SAVE1-6 | 5.58・5.59・5.66・5.67 の定義 | 12 件とも DETECTED |

### 5.72 再読み込みの後に届く古いタイマーの決定的な試験（TASK-221 2/2）

5.70 の続きで、実際のタイマー（表示側の `set_timeout`。取り消さない設計）が、再読み込みの後に古い宛先で届く場面を確かめた。
製品の振る舞い（core）は変えていない。変えたのは、試験の設定（`test-hooks`）だけにある再読み込みの口と、試験である。

- 再読み込みの口: `raw` の action に固定の合図 `{"reload":{}}`（`host::RAW_RELOAD`）を完全一致で渡すと、ホストが `Session::reload` を呼ぶ（`HostAction::Reload`）。新しい隠し要素は足していない（`raw-trigger` を使う）。合図は `Op` として復号できず、衝突しない。配信する設定では復号されず `None` になる。
- ホストの試験（`crates/web/tests/host_contract.rs` の `reload_stale`。`--features test-hooks`）: 新しい世代の宛先の通知、古い宛先の無視 1 件と二重の届き、完走（`Progress` は世代 1 が 0・1、世代 2 が 2・3・4）、再読み込み 2 回、再読み込みと停止の組み合わせ、処理中でないときの再読み込みを確かめる
- 静的な検査（`crates/web/tests/no_progress.rs`）: `.reload(` が `host.rs` の `reload_engine` の 1 箇所だけで、その関数が `test-hooks` の背後にあることを、件数を固定して検査する
- ブラウザ試験（`tools/browser/reload-ignore.test.mjs`。`make test-browser` に追加）: R-1 実タイマー（再読み込みの前に予約された古いタイマーが後から届き、無視は 1 件だけ。待つ条件は時間ではなく件数）、R-2 再読み込みの直後に再読み込み前の宛先を送る（S-5 の類比）、R-3 再読み込みの後の停止（S-6 の類比）、配信設定に口が無いこと。R-3 では、再読み込みで予約された世代 2 のタイマーと、再読み込みの前の世代 1 のタイマーの 2 本が後から届き、無視は 2 件になる
- 変異の定義に任意のキー `features` を足し（`scripts/check-mutations.sh`）、`test-hooks` の背後にある試験を対象にできるようにした。EVT8-3 は、ホストの再読み込みの模擬が core の再読み込みを呼ばない誤りである

実際の `page.reload()` ではなく、ページ内の模擬の再読み込みを選んだ。web には保存した状態の復元の結線が無く（TASK-319。#340〜#343）、`page.reload()` では教材の開始状態から作り直されるだけで、再読み込みの前の世代を引き継げないためである。停止の注入と同じ方式（`stop-move-ignore.test.mjs`）に合わせた。

```text
実行日: 2026-10-16
元にしたコミット: 1338226-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/EVT8-3.mutation
```

| ID | 対象 | 結果 |
|---|---|---|
| EVT8-3 | crates/web/src/host.rs: ホストの再読み込みの模擬が core の再読み込みを呼ばない | DETECTED（reload_stale の 7 件が失敗。期待した 2 件を含む） |

未実施: 実際の `page.reload()` を使う場面（browser-scenes.md の S-5・S-6 ほか）は、web の保存・復元の結線（TASK-319 3/3。#343）の後に行う。

### 5.73 確認の操作の連打の直後の再読み込み（TASK-321 2/2）

確認の操作を 20 回連打し、保存の遅延を待たずにその直後に再読み込みしても、達成が 1 回であり直後の状態へ戻ることを固定した（SAVE-5。PoC-6 の S-9）。
製品コードは変えていない。進行を流し終えた後に確認を送ると、1 回目だけが達成（`Achieved`・`Transitioned`・`Entered`）になり、2 回目以降は次の Step（確認が `none` の選択の Step）に届いて `NotInThisStep` で拒否されるだけである。
受付も達成も起きず、`run` も変わらない。復元した状態は、世代の +1 とログ末尾の `Reloaded` 1 件を除いて連打の直後の状態と等しく、`Achieved` は再読み込みの前も後も 1 件である。

| 試験 | 内容 |
|---|---|
| `crates/core/tests/reload_after_control.rs` の `reload_right_after_confirm_burst_keeps_one_achievement` | S-9。記録 → 包み → 書き出し → 読み込みの段 1〜8 → 再読み込みを通す。復習の項目が 1 件で達成済み、再読み込みの後の確認でも達成が増えない。連打の前の世代の宛先は `Ignored(Stale)` になる |
| `crates/web/tests/reload_after_control_store.rs` の `連打の直後に保存を待たず再読み込みしても達成は1回で直後の状態へ戻る` | `PendingSave` → `flush` → localStorage の偽物 → 読み戻し → `load_history` → `reload` の経路で同じことを確かめる |

自動ブラウザ（S-6・S-7・S-9・S-19）は未実施である。理由は 5.71 と同じで、web の記録・復元の結線（TASK-319）の後に足す。

SAVE5-4 は、確認の達成で現在位置を次の Step へ移さない誤りである。連打の 2 回目以降が達成済みの Step に届き、達成が重複する。
既存の SAVE5-1〜SAVE5-3・SAVE4-1〜SAVE4-6・SAVE1-1〜SAVE1-6 も同じ実行で流し、検出が失われていないことを確かめた。

```text
実行日: 2026-10-16
元にしたコミット: 31bde1d-dirty
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAVE5-{1,2,3,4}.mutation crates/core/tests/mutations/SAVE4-{1,2,3,4,5,6}.mutation crates/core/tests/mutations/SAVE1-{1,2,3,4,5,6}.mutation
```

| ID | 対象 | 結果 |
|---|---|---|
| SAVE5-4 | crates/core/src/confirm.rs: 確認の達成で現在位置を次の Step へ移さない | DETECTED（reload_right_after_confirm_burst_keeps_one_achievement） |
| SAVE5-1〜SAVE5-3・SAVE4-1〜SAVE4-6・SAVE1-1〜SAVE1-6 | 5.72・5.71・5.58・5.59・5.66・5.67 の定義 | 15 件とも DETECTED |

### 5.74 再読み込み後の告知（TASK-322）

再読み込みを右のログの 1 行にする（SAVE-6・D-57）。`Projection` が `Event::Reloaded` 1 件につき 1 行を出し、web の `log_lines` が行の種類 `reloaded`（固定の文言「再読み込みしました」）で描く。右のエリアの契約（`terminal_area`）は版 1 から版 2 に上げた。文言は D-57 が決めた「趣旨」から実装側で選んだもので、言い回しは未決である。変えるときは `log_lines::RELOADED_TEXT` だけを直す。

- core の試験: `projection_derive` の `reloaded_adds_one_row_and_keeps_marks`（行が回数だけ増える・変化の印を消さない・増分と一括の一致）、`reload_busy` の `each_reload_adds_one_log_row`、`reload_after_control`（ログ末尾に `Reloaded` を指す 1 行だけ増える）
- web の試験: `log_lines_contract`（`Reloaded` の写しと文言・`test-hooks` の背後で再読み込みの模擬を 1 回・2 回挟んだ後の `reloaded` の行の数）、`terminal_area_contract`（版 2・種類 7 件）
- ブラウザ試験（`tools/browser/reload-notice.test.mjs`。`make test-browser` に追加）: N-1 再読み込み 1 回で `reloaded` の行が 1 件・処理中のまま・完走の後も 1 件、N-2 2 回で 2 件（`data-gen` は 1・2）、N-3 配信設定に口が無いこと。実際の `page.reload()` ではなくページ内の模擬の再読み込みで行う（5.72 と同じ理由）
- 範囲外: ライブの欄の読み上げが 1 回だけ届くことと枠の二重の通知が 0 件であることは TASK-536 が持つ。再読み込みの行は `a11y::LIVE_EXCLUDED_LINE_KINDS` に入れていない（更新に数える側）

変異の ID は `SAVE6-1`（導出が `Reloaded` を行にしない）・`SAVE6-2`（web が `Reloaded` を行に写さない）である。

```text
実行日: 2026-10-16
元にしたコミット: 094b1df-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAVE6-1.mutation crates/web/tests/mutations/SAVE6-2.mutation
```

| ID | 対象 | 結果 |
|---|---|---|
| SAVE6-1 | crates/core/src/projection.rs: 再読み込みを右のログの行にしない | DETECTED（each_reload_adds_one_log_row） |
| SAVE6-2 | crates/web/src/log_lines.rs: 再読み込みを右のログの行に写さない | DETECTED（再読み込みは固定の文言の_reloaded_行になる） |

未実施: 実際の `page.reload()` での 1 回・2 回の確認は、TASK-319 3/3（#343）の後に行う。

### 5.75 別のオリジンへの履歴の読み込み（TASK-330 2/2）

書き出したファイルを別のオリジンで選び、同じ状態にして続きを操作できることを固定した（SAVE-16。SHELL-15 の自動ブラウザの部分）。
core に開いたときだけ `Session` を作る入口 `history_load::load_session` を足し、web のホストの `EngineHost::open_history` がそれを呼ぶ。
判定は `load_history`（段 1〜8）に任せて複製せず、開かないときは状態・導出した値・dirty に触れない。保存先へは書かない（保存の結線は TASK-319）。

ブラウザ試験（`tools/browser/history-import.test.mjs`）は、配信を 2 つ起動して別のオリジン（ポートが違う）を作り、別の context で開く。
オリジン A で書き出してダウンロードしたファイルを、オリジン B で `setInputFiles` により選ぶ。ファイルの選択と読み取りは DOM の値を読むため、Rust ではなく試験の設定だけの `static/test-hooks.js` が行い、`raw-trigger` へ固定の接頭辞（`open-history:`）つきで届ける（`RuntimeAction` は変えていない）。
確かめることは次のとおりである。結果は `stateJson().engine.lastImport` の固定のコードで読み、履歴の中身は出力に出さない。

| 試験 | 内容 |
|---|---|
| 別のオリジンで同じ状態になり、続きを操作できる | B の Step・世代・世界状態・チェックポイント・下書き・ログ・復習・成果物が A の基準と一致する。続きの操作（次の Step への移動）で A と B が同じ結果になる。読み込みの前後で location・履歴・ハッシュが同じ。外部への要求（相手のオリジンへのものを含む）・CSP 違反・ページのエラーが 0 件。A に書いた保存領域の値は B から見えない |
| 書き換えたファイル・JSON でないファイルは開かず、状態を変えない | `refused` / `replay_mismatch` と `refused` / `not_json`。B の状態は読み込みの前と同じ。そのあとに開けるファイルを選ぶと開く |

native の試験は `crates/core/tests/history_load_session.rs`（開く・表示だけ・開かない・写しの書き換え）と `crates/web/tests/history_import.rs`（`Session` の差し替え・導出した値が一括の導出と一致・全 field の dirty と宛先の通知・拒否で何も変わらない・`raw` の接頭辞の一致）である。

開ける包みの作り方と限界: 契約の試料（`two-rounds.json`）の写しは再読み込みの意味の実装前の値で、いまの main では段 8 で `replay_mismatch` になり開けない。そこで試験の設定の `test_fixture::loadable_envelope` が、試料の `ops` を試験の教材と開始直後の状態に適用し直し、得た状態を写しにして包み直す。
ホストに受講中の `ops` の記録（`OpsRecorder`）がまだ無いため、受講中の実際の状態の書き出しと読み込みは TASK-319・TASK-531 の結線の後に確かめる。
適用し直しの出発点は、ホストの組み立て時の状態である（`Control::Begin` と `start` からの初期状態の構築が入るまでの間。TASK-217・TASK-294）。
実際の GitHub Pages の間と、macOS のファイル画面での確認（SAVE-30）は人が行う。

変異は SAVE16-1（読み込みで projection を作り直さない）・SAVE16-2（適用し直しの出発点を既定値にする）・SAVE16-3（読み込みの後に dirty を積まない）・SAVE16-4（表示だけの履歴でも実行環境を作る）である。

```text
実行日: 2026-10-16
元にしたコミット: 68ac983-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/SAVE16-{1,2,3}.mutation crates/core/tests/mutations/SAVE16-4.mutation
```

| ID | 対象 | 結果 |
|---|---|---|
| SAVE16-1 | crates/web/src/host.rs: 読み込みで projection を作り直さない | DETECTED（open_replaces_the_session_with_the_snapshot_and_rederives_views） |
| SAVE16-2 | crates/web/src/host.rs: 適用し直しの出発点を既定値にする | DETECTED（open_replaces_the_session_with_the_snapshot_and_rederives_views ほか） |
| SAVE16-3 | crates/web/src/host.rs: 読み込みの後に field を dirty に積まない | DETECTED（open_marks_every_field_dirty_and_notifies_the_next_tick） |
| SAVE16-4 | crates/core/src/history_load.rs: 表示だけの履歴でも実行環境を作る | DETECTED（view_only_yields_no_session） |

### 5.76 Tab の補完と対象外コマンドの除外（TASK-440 2/2）

一覧が開いていて文字がある入力欄で Tab を押すと、`preventDefault` され、下書きが先頭の候補（名前順）に置き換わり、`<textarea>` に書き戻され、フォーカスは入力欄に残ることを固定した（TERM-11・D-60。PoC-2 の K-03・K-07）。
`unsupported` のコマンドは候補の表に入らないため、一覧にも Tab の補完にも出ない（TERM-42）。

結線は次の 3 か所である。glue の Tab の経路が `candidates_open` を `field_state` へ渡す。配送された `HostAction::Complete` を、ホストが `HelloView::complete` に渡す。`complete` は配送の時点の状態で `decide` をやり直し、`completion_choice`（契約の `first_completion`）で先頭を選ぶ。下書きが変わったときだけ、ホストが `draft_sync_request` で書き戻しを頼む。補完はエンジンへ何も渡さず、ログ・世代・projection は変わらない。

native の試験は `crates/web/tests/candidates_contract.rs`（補完・複数候補で先頭・補完しない場面・Esc で閉じた後・コマンド表の無いホスト・書き戻しの回数）、`crates/web/src/hello.rs` の単体試験、`crates/web/tests/key_route_contract.rs`（`field_state` の素通し）である。ブラウザ試験は `tools/browser/candidates.test.mjs`（K-03・複数候補・補完後の Tab・`unsupported`・K-07。CSP あり・なし）である。

glue（wasm32 専用）の結線の変異は native の試験で検出できないため、ブラウザ試験の K-03 が担う。実 IME での変換中の Tab の確認は人が行う（TASK-93）。

`make test-browser` では `submit-wiring.test.mjs` の「送信の直後に変換が始まっても…」の 2 件（CSP あり・なし）が失敗する。本変更を入れる前の `origin/main`（59f2ad9）でも同じ 2 件が失敗することを確かめており、本タスクの範囲外である。

```text
実行日: 2026-10-16
元にしたコミット: 59f2ad9-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/TERM11-1.mutation crates/web/tests/mutations/TERM11-2.mutation crates/web/tests/mutations/TERM42-3.mutation
```

| ID | 対象 | 結果 |
|---|---|---|
| TERM11-1 | crates/web/src/host.rs: 補完で書き戻しの要求を出さない | DETECTED（補完は先頭の候補に置き換え_一覧を閉じ_書き戻しを一度だけ頼む） |
| TERM11-2 | crates/web/src/hello.rs: 先頭ではなく末尾の候補を挿入する | DETECTED（複数の候補があれば名前順の先頭で補完し_unsupported_は選ばない） |
| TERM42-3 | crates/web/src/candidates.rs: unsupported を候補の表から除外しない | DETECTED（開く場面と開かない場面 ほか） |

### 5.77 左のビュー共通の検査器（TASK-471 2/3）

PoC-8 の検査を、実装 repo の共通のブラウザ試験の道具（`tools/browser/artifact-view-check.mjs`）として整えた。
本節の番号は、この文書の 5 節に既にある重複を直さずに、既存の最大の次を使った（base 取り込み時に 5.65・5.66 の衝突を、取り込みのたびに既存の最大の次へ付け直し、5.77 にした）。

検査器は ART-1 の 14 通り（7 種 × 2 変種）を流せる枠である。各通りで前 → 後 → 前 → 後 → 前の 4 回の切り替え（2 往復）を行い、各状態で次を数える。

- 前後の項目数と key の並びが、JSON から Node 側で独立に数えた期待値（`expectedFromRows`）と一致する
- 印の 3 手段（記号・文字・`data-mark`）が対応し、印の宿主が項目ごとにちょうど 1 つである
- key が重複しない
- 選ばれていない list が空で `hidden`、選ばれた list は `hidden` でない
- 要素の同一性（両方の状態にある行は同じ要素で、置換が 0。保持の数は前の行数に等しい）
- 外部へのリクエスト・ページのエラー・ダイアログが全区分 0（`harness.mjs` の `collectSafety`）

14 通りの数え方は、契約（`ViewKind::allows`）が許す 12 通りを `runViewCase` で流し、許さない 2 通り（文書 (a)・差分 (a)）を `runRejectedCase` で流す。
後者は、拒否されること、状態も画面も変わらないこと、(b) へ黙って退避しないことを確かめる。基準の状態に (b) を使うと黙った退避を見分けられないため、
基準は card_list の (a) にした。後続の検査は、自分のビューを差し込むアダプタ（`select`・`phase`）を渡して使う。

文書だけは JSON の行が無く、期待値を手で数えた固定値（前 10 ブロック・後 13 行。未変更 7・追加 3・削除 3・変更 0）にした。
出所は `doc-before.md`・`doc-after.md` の最上位のブロックである。TASK-476 で本文の木の粒度を結んだときに見直す既知の制約である。

製品の `view()` には 7 本の list が入った（TASK-470）が各ビューの本体が無い（TASK-473 以降）ため、試験の設定だけに、マウントする代役の部品（`crates/web/src/view_fixture.rs`）を足した。
`#app` の外の別の root にマウントし、本体の画面とは状態を共有しない。(a)・(b) は印の付け先の違いだけを再現し、`wireframe-ui`・`headless-ui` の部品は使わない。
そのため本節の合格は検査器の枠が動くことの証拠で、各ビューの合格ではない（ビューごとの証拠は TASK-473 以降）。

空振りでないことは、自己確認（`artifact-view-check.test.mjs`）の T4 で示した。マウントしない空の画面と存在しない root では違反が出る。
負の対照 3 種（印を消す・部品を項目の外に置く・`innerHTML`）と教材固有の語の検査は TASK-471 3/3 が持つ（5.84）。

`make test-browser` は 158 件が通過した（2026-10-16。新しい試験は 21 件）。`make test`・`make lint`・`make check`・`make check-deps`・`make check-dom-reads` も通過した。
配信設定（`make build-web`）は `make check-delivery` を通り、wasm に `memo-app`・`view-fixture` の文字列が 0 件であることを `grep` で確かめた。

### 5.78 貼り付けと上限の案内文の表示（TASK-442）

画面に出る上限の案内文（TERM-15）と貼り付けの扱い（TERM-19）を、PoC-2 の P-1〜P-10 に当たる自動試験で確かめた。切り捨て・案内の表示・4 種類の貼り付けの本体は先行タスク（TASK-158・TASK-279・TASK-280）で実装済みで、本タスクは本番のコードを変えず、観察の隙間だけを試験と変異で埋めた。

| 層 | 試験 | 確かめること |
|---|---|---|
| native | `crates/web/tests/input_notice.rs` | 案内欄が 1 個・`p` 要素・`role=status`・子が文字だけ・入力欄より後ろ。超過で束縛と描画に固定の案内文が出て、1 文字消すと消える。PoC-2 の写し（20,000・20,000 のサロゲートペア入り・20,001）。4 種類の貼り付けで送信が起きない。超過で切ってもログが増えない |
| ブラウザ | `tools/browser/paste-notice.test.mjs` | PoC-2 の写しでの P-3・P-4・P-7 と、P-5・P-6（CSP あり・なし。毎回ログが増えない）。案内欄の DOM の性質。合成の paste の後に案内が出ずログが増えない（P-10）。試験の設定で、`/` 始まりの貼り付けでも候補の一覧が開かない（P-9）ことと、4 種類の貼り付けでログが変わらないこと |

P-9 は、配信設定の web がコマンド表を持たず一覧が常に閉じるため、教材（`min-3step.json`）を読む試験の設定でだけ観察できる。

案内文の位置は、右のエリアの領域（`aria-label`）の配置が TASK-517 の範囲であるため、`terminal_area::check_view` ではなく、案内欄の個数・要素・属性・並びを直接照合した。

`make test-browser` では `submit-wiring.test.mjs` の「送信の直後に変換が始まっても…」の 2 件（CSP あり・なし）が失敗する。5.76 に記したとおり本変更の前からの失敗で、本タスクの範囲外である。本タスクで足した 10 件は通過した。

```text
実行日: 2026-10-16
元にしたコミット: 2bf37a9-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/TERM15-1.mutation crates/web/tests/mutations/TERM15-2.mutation
```

| ID | 対象 | 結果 |
|---|---|---|
| TERM15-1 | crates/web/src/hello.rs: 案内欄の束縛の値を常に空にする | DETECTED（上限までは案内が空で_超過すると固定の案内文が束縛と描画に出る ほか） |
| TERM15-2 | crates/web/src/hello.rs: 案内欄の描画の値を常に空にする | DETECTED（上限までは案内が空で_超過すると固定の案内文が束縛と描画に出る） |

### 5.79 Esc の画面の結果（TASK-441 1/2）

変換中でない入力欄での Esc のうち、(1) 処理中なら候補の一覧が開いていても停止して一覧も閉じる、(2) 処理中でなく一覧が開いていれば閉じるだけ、の 2 つを右のログ・状態表示・候補の一覧が揃った画面で確かめた（TERM-7。PoC-2 の K-09・K-11・K-14）。経路の本体は先行タスクで実装済みで、本タスクは本番のコードを変えず、観察の隙間だけを試験と変異で埋めた。(3) の取り消しの入力と、取り消しを受け付けない Step の拒否の行は TASK-441 2/2（#335）の持ち分である。

| 層 | 試験 | 確かめること |
|---|---|---|
| native | `crates/web/tests/run_status_contract.rs` | ホスト経由で、処理中に候補が開いたまま Esc を押すと停止（世代 +1・停止の行 1 件・入口へ戻る）になり、一覧が閉じて下書きが残る。停止の後に一覧が開いているときの Esc は閉じるだけで、世代もログも変わらない |
| ブラウザ | `tools/browser/esc-outcomes.test.mjs` | 試験の設定（CSP あり・なし）で、処理中＋一覧が開いた画面（raw の送信と、実 UI の送信後に打鍵する 2 経路）の Esc が「停止」と一覧の閉鎖を同時に起こすこと、待機中・停止の後の Esc が一覧を閉じるだけで状態表示・ログの行・世代を変えないこと |

各ケースは Esc を 1 回押すところで止め、閉じた後の Esc の結果は検査しない（#335 で取り消しの入力に結ばれる）。

実行結果（2026-10-16）。`make test`・`make lint`・`make check`・`make check-deps` は通過した。新しいブラウザ試験 8 件（4 ケース × CSP あり・なし）は、単独で 2 回流していずれも通過した。`make test-browser` の全体は、本タスクでは流していない（未検証）。

```text
実行日: 2026-10-16
元にしたコミット: 41d3239-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/TERM7-1.mutation
```

| ID | 対象 | 結果 |
|---|---|---|
| TERM7-1 | crates/web/src/hello.rs: 停止の経路で候補の一覧を閉じない | DETECTED（処理中に候補が開いていても_esc_は停止になり候補も閉じる） |

### 5.80 取り消しの入力と不受理の Step の拒否（TASK-441 2/2）

変換中でない入力欄での Esc の 3 つ目、(3) 処理中でなく候補も閉じていれば取り消しの入力にする、を画面で確かめた（TERM-8。JUDGE-17・JUDGE-18。PoC-2 の K-09・K-10）。5.79 と違い、本タスクは本番のコードを変えた。`crates/web/src/host.rs` の Esc の分岐は、従来は停止だけを Session へ適用していた。これを停止と取り消しの入力（`Op::Input(Input::Cancel)`）の 2 つに広げた。許可する op を明示し、`EscRoute` に行き先が増えても黙って Session へ流れないようにした。判定（`esc_route`）と core 側の拒否（`on.cancel` の規則が無い Step の `not_in_this_step`）は実装済みで、欠けていたのはホストの結線だけだった。試験の教材（`min-3step.json`）はどの Step も取り消しを受け付けないため、拒否の側だけを確かめる。取り消しを受け付ける Step の画面の結果と下書きの扱いは仕様が定めておらず、本タスクの範囲外である。

| 層 | 試験 | 確かめること |
|---|---|---|
| native | `crates/web/tests/run_status_contract.rs` | 待機中で候補が閉じている Esc（下書きは空と `hello`）で、ログの末尾が `InputReceived { Cancel }` と `Judged { Rejected(not_in_this_step) }` になり、世代・状態表示・下書きが変わらず、文面の控えが `filled_text` と一致し、`rejected` の行が 1 件（子は理由と次の行動）になること。候補が開いていれば 1 回目で閉じ 2 回目で取り消しになること。停止の後の Esc が取り消しになり、停止の行は 1 件のままで世代が変わらないこと |
| ブラウザ | `tools/browser/esc-outcomes.test.mjs` | 試験の設定（CSP あり・なし）で、上の 3 通りの拒否の行が出て、文字が IR の既定の拒否の文面・次の行動と一致し、子が文字ノード 1 つで、`logLen` は +2 でも `rejected` の行は 1 件であること |

結線で期待が変わった既存の試験は、検査を消さずに直した。`esc_と停止ボタンは…`（名前は変異 TERM33-3 が参照するため維持）は、待機中の Esc で拒否が 2 件増え世代が変わらないことを検査する。`candidates_contract.rs` の 2 件は名前を実態に合わせた。`tools/browser/candidates.test.mjs` は、閉じた後の 2 回目の Esc で `logLen` が +2 になることを検査する。

実行結果（2026-10-17）。ブラウザ試験は `esc-outcomes`・`candidates`・`esc-key`・`esc-attr`・`stop-redo`・`restore-cancel`・`reject-lines`・`keydown-action`・`run-status` を個別に流し（58 件）、全件通過した。`esc-outcomes` と `candidates` は続けてもう 1 回流して通過した。`make test-browser` の全体は流していない（未検証）。`make lint`・`make test`・`make check`・`make check-deps`・`make check-dom-reads` は通過した。`make check-delivery` は流していない（未検証。本タスクは試験用の口を足していない）。

```text
実行日: 2026-10-17
元にしたコミット: 665042c-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/TERM8-1.mutation crates/web/tests/mutations/TERM33-3.mutation crates/web/tests/mutations/TERM7-1.mutation crates/web/tests/mutations/JUDGE17-4.mutation crates/web/tests/mutations/JUDGE18-2.mutation
```

| ID | 対象 | 結果 |
|---|---|---|
| TERM8-1 | crates/web/src/host.rs: 待機中の Esc の取り消しの入力が Session へ届かない | DETECTED（待機中で候補が閉じている_esc_は取り消しの入力になり_拒否の行と次の行動が出る） |
| TERM33-3 | crates/web/src/host.rs: Esc の経路で処理中を常に偽にする | DETECTED（流し直し） |
| TERM7-1 | crates/web/src/hello.rs: 停止の経路で候補の一覧を閉じない | DETECTED（流し直し） |
| JUDGE17-4 | crates/web/src/key_route.rs: 取り消しの入力がエンジンへ届かない | DETECTED（流し直し） |
| JUDGE18-2 | crates/web/src/key_route.rs: 1 回目の Esc の後も候補が開いたまま | DETECTED（流し直し） |

### 5.81 入れ子の実体の根（#1413）

MOD-4・TASK-236 の「最初の `/` まで」と、実装の「最後の `/` より前（`Instance.id`・`Node.scope`）」は、入れ子の実体（`impl-run@round1/merge-gate@gate/st-7-4`）でだけ食い違う。文言の出所は入れ子の無い PoC-4 の試作で、入れ子を意識して決めた記録は見つからなかった（2026-10-17）。実装は語彙 v1 の 9 節（フラグなどは実体ごと）・`Root` の文書・pack の展開と合うため変えず、入れ子の実体で根が `Instance.id` になることを試験で固定した。仕様側の決定は未了で、人が spec のリポジトリで決める（`docs/engine-api.md` の 6 節）。

| 層 | 試験 | 確かめること |
|---|---|---|
| native | `crates/core/tests/history_envelope_build.rs` | 入れ子の実体・親・別の周回の下書きが別々の鍵に入ること。入れ子の送信がその実体の鍵だけを除くこと。期待の根は文字列で直に書く |
| native | `crates/core/tests/session_contract.rs` | testdata の IR の全ノードで `StepUid::root` が `Node.scope` と一致すること（入れ子を含む IR は testdata に無い） |

```text
実行日: 2026-10-17
元にしたコミット: 0a3707e-dirty
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/MOD4-1.mutation crates/core/tests/mutations/SAVE24-8.mutation crates/core/tests/mutations/SAVE24-9.mutation

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| MOD4-1 | crates/core/src/addr.rs: 根を最初の `/` までにする（MOD-4 の文言どおりの読み。入れ子の実体の下書きが親と混ざる。#1413） | nested_instance_draft_root_is_its_instance_id, nested_submit_clears_only_its_own_root | nested_instance_draft_root_is_its_instance_id, nested_submit_clears_only_its_own_root | DETECTED | - |
| SAVE24-8 | crates/core/src/history_envelope.rs: 送信の記録の後に現在の根の下書きを写しから除かない（MOD-4・SAVE-18） | submit_clears_only_the_current_root_draft, property_ops_and_drafts_stay_in_sync | nested_submit_clears_only_its_own_root, submit_clears_only_the_current_root_draft, property_ops_and_drafts_stay_in_sync | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Draft(""), Submit("a")] |
| SAVE24-9 | crates/core/src/history_envelope.rs: 下書きを ops に記録しても写しの drafts に書かない（SAVE-18・SAVE-24） | save24_example_shape_is_in_the_envelope, empty_draft_is_kept_as_empty_string, property_ops_and_drafts_stay_in_sync | nested_submit_clears_only_its_own_root, empty_draft_is_kept_as_empty_string, nested_instance_draft_root_is_its_instance_id, submit_clears_only_the_current_root_draft, save24_example_shape_is_in_the_envelope, whitespace_submit_changes_nothing, property_ops_and_drafts_stay_in_sync | DETECTED | cases=256 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Draft("")] |
```

### 5.82 記録中に履歴の件数の上限を超えたときの保存の判定（#1414）

記録中に `ops` が `MAX_HISTORY_OPS` を超えたときの規則は、spec に無い。次を安全側の仮の規則（推論）として実装した。記録は止めず、保存は読み込めない包みを書かず（最後に読み込めた保存を残す）、書き出しは止めない。判定は `history_limit::check_save` が理由コード（`too_large`・`too_many_ops`）で返し、文面は持たない。規則の決定（D-n）と SAVE-19・SAFE-41・SAVE-24 への記録は、人が spec のリポジトリで行う（未了）。保存の経路への結線は TASK-318・TASK-319、受講者への案内は TASK-531 の後に確かめる（2026-10-17）。

| 層 | 試験 | 確かめること |
|---|---|---|
| native | `crates/core/tests/history_limit.rs` | 20,000 件と 20,001 件の境界、大きさを先に見る順、保存の判定が読み込みの判定と一致すること、20,001 件目以降も記録が続くこと、件数の性質テスト |
| native | `crates/core/tests/history_export.rs` | ちょうど上限の件数で `exceeds_ops_limit` が偽であること（超えても書き出しは止まらない既存の確認に足した） |

```text
実行日: 2026-10-17
元にしたコミット: 5631b7a-dirty
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAVE19-15.mutation crates/core/tests/mutations/SAVE19-16.mutation crates/core/tests/mutations/SAVE19-10.mutation crates/core/tests/mutations/SAVE19-11.mutation

| ID | 対象 | 結果 |
|---|---|---|
| SAVE19-15 | crates/core/src/history_limit.rs: 件数の比較を <= から < にする（上限ちょうどの保存を断つ） | DETECTED（ops_boundary_at_and_over_limit, property_ops_boundary, save_gate_agrees_with_load_gate） |
| SAVE19-16 | crates/core/src/history_limit.rs: 件数の検査を外す | DETECTED（ops_boundary_at_and_over_limit, property_ops_boundary, save_gate_agrees_with_load_gate） |
| SAVE19-10 | crates/core/src/history_load.rs: 読み込みの件数の比較を >= にする（既存） | DETECTED |
| SAVE19-11 | crates/core/src/history_load.rs: 読み込みの件数の検査を外す（既存） | DETECTED |
```

### 5.83 保存先の鍵の規則の変異の追加（#1417）

§5.36（TASK-310 1/3）の SAVE14-1・SAVE15-1 に続き、`crates/contract/src/storage.rs` の鍵の規則のうち変異で守っていなかった 4 点に定義を足した。`from_ir` は `starts` の先頭だけを読むこと（SAVE14-2）、区間の `/` を符号化して名前空間を分けること（SAVE14-3）、教材の ID が `unreadable` のときの特例（SAVE15-2）、`belongs_to` が配信の区間と教材の ID の両方で判定すること（SAVE32-1・SAVE32-2。条件ごとに 1 件）である。SAVE32-1・SAVE32-2 を捉える試験は `store_port_roundtrip`（SAVE-32 相当の削除）の 1 本だけで、検出面が狭い。実装と試験は変えていない（2026-10-17）。

```text
実行日: 2026-10-17
元にしたコミット: 6801eaf-dirty
コマンド: scripts/check-mutations.sh crates/contract/tests/mutations/SAVE14-2.mutation crates/contract/tests/mutations/SAVE14-3.mutation crates/contract/tests/mutations/SAVE15-2.mutation crates/contract/tests/mutations/SAVE32-1.mutation crates/contract/tests/mutations/SAVE32-2.mutation

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| SAVE14-2 | crates/contract/src/storage.rs: 鍵を最初でなく最後の開始点から作る（開始点を足すと鍵が変わり、保存した履歴が読めなくなる） | key_uses_first_start_only | key_uses_first_start_only | DETECTED | - |
| SAVE14-3 | crates/contract/src/storage.rs: 区間の / を符号化しない（区間の数が崩れ、接頭辞が別の配信先・別の教材の鍵に届く） | render_parse_roundtrip_and_injective, pack_prefix_does_not_reach_other_scope_or_pack | pack_prefix_does_not_reach_other_scope_or_pack, render_parse_roundtrip_and_injective | DETECTED | - |
| SAVE15-2 | crates/contract/src/storage.rs: 教材の ID が unreadable のときの特例を外す（読めない IR の鍵と衝突する） | pack_named_unreadable_does_not_collide, render_parse_roundtrip_and_injective | pack_named_unreadable_does_not_collide, render_parse_roundtrip_and_injective | DETECTED | - |
| SAVE32-1 | crates/contract/src/storage.rs: 削除の対象の判定で配信の区間を見ない（別の配信先の同じ教材の履歴まで消える） | store_port_roundtrip | store_port_roundtrip | DETECTED | - |
| SAVE32-2 | crates/contract/src/storage.rs: 削除の対象の判定で教材の ID の一致を反転する（指定した教材が残り、別の教材が消える） | store_port_roundtrip | store_port_roundtrip | DETECTED | - |
```

### 5.84 負の対照と教材固有の語の検査（TASK-471 3/3）

TASK-471 の残りの 2 つを足した。検査器（5.77）が差を見つけられることの負の対照 3 種と、エンジンの crate に教材固有の語が無いことの検査（W-22）である。

#### 検査器の拡張

`tools/browser/artifact-view-check.mjs` に 2 つの検査を足した。PoC-8 が危険な要素を数えた選択子の写し（`DANGER_SELECTOR`）で root の下を数え、0 でなければ違反にする。root 直下の部品の根が前後で同じ要素であることも比べる（全再描画を、行の置換とは別の違反として名指しするため）。

#### 負の対照 3 種（`tools/browser/artifact-view-controls.test.mjs`）

どれも「変えない側は違反 0、変えた側は特定の違反」の対で主張し、違反は固定の文言で特定する。

| ID | 対照 | 期待 |
|---|---|---|
| N1 | 後の状態で、印の記号・文字・`data-mark` のどれか 1 つを消す（3 通りを別の試験にする） | `glyph/text/data-mark do not correspond` |
| N2 | 代役の部品に `fixture_control` で項目の外の部品を足す（束縛の無い dirty field により、上流は全再描画に倒れる） | 行の置換と根の置換の違反。保持の数がすべて 0。部品を項目の中に戻した対は違反 0 |
| N3a | escape の行の文字列を、不活性の文書で `innerHTML` と `textContent` に入れて数える | `innerHTML` は目で数えた件数（14 文字列で 1・1・0・1・1・0・1・1・0 …、合計 6）、`textContent` は 0。`window.__escape_hit` は未定義のまま |
| N3b | 後の状態で、ラベルへ `innerHTML` で `img`・`iframe`・`javascript:` の `a` を入れる | `dangerous elements or attributes are present` |

`fixture_control` は `view_fixture.rs`（試験の設定だけ）にあり、配信物には入らない。24 通り（6 ビュー × 2 変種 × 前後）で escape の行を実際に流す正の試験は、代役の部品が sample の JSON しか持たないため ART-6 の担当に残す。

#### 教材固有の語の検査（`tools/check-generic-words.mjs`）

- 語の一覧の正は `docs/vocabulary/vocabulary.md` の付録 A で、検査は写さずに読む（既存の 2 試験と同じ照合の規則）。付録 A が読めない・0 語・未知の照合の値は終了コード 2 で、合格にしない
- 対象は `crates/*/src/**` の `.rs`、`crates/*/static/**` の `.js`・`.mjs`・`.html`・`.svg`・`.css`、`crates/*/Cargo.toml`。`crates/*/tests/**` は、教材を模した試験データを意図的に持つため対象外にした
- `.rs` と `Cargo.toml` は字句解析でコメントを除き、識別子と文字列リテラルの字句を照合する
- static は字句解析をせず、コメントを含むファイル全体の素の文字列と、エスケープを文脈を見ずに全体で復号した文字列で照合し、どれか 1 つの読みで語が見つかれば違反にする。復号の形式は HTML の文字参照（数値の参照は `;` の有無を問わず先頭の 0 も許す。`;` の無い旧来の名前を含む）・CSS のエスケープ（16 進と 1 文字）・JS のエスケープ（`\xHH`・`\uXXXX`・`\u{...}`・旧来の 8 進・行の継続）で、形式が重なる場合のために 2 形式の順序つきの組（HTML→CSS・JS→HTML など）でも照合する
- static のコメントも照合するのは、字句の読み違いで語を見逃す経路を無くすためである。JS・HTML・CSS を自前で字句解析していた版では、正規表現の開始の判定（`)`・`}`・`++`・`for await` の後の `/`）、`<script>` の中の `<!-- -->`、`;` の無い数値文字参照、`<style>` や引用符の無い style 属性の CSS エスケープなど、読み違いの経路を Codex のレビューが見つけ続けた。仕様が求めるのは語が 0 件であることと、語を足した対照が失敗することだけで、コメントを除くことは実装の選択だった
- `.rs`・`Cargo.toml` の閉じない文字列・コメント、シンボリックリンク、UTF-8 でない static（BOM つきの UTF-16・不正なバイト列）は終了コード 2 にする。static の復号は全域の関数で、読めない形（範囲外の参照など）はブラウザと同じく置換文字にするか文字のまま残す。生の読みも必ず照合するため読めない形で語を隠すことはできず、正当な JS・HTML・CSS で 2 にはならない
- 例外は 2 種類で、理由をスクリプトに書いた。字句の完全一致の許可は `thread_local`（Rust 標準のマクロ）と `SlashCount`（区切り文字 `/` の個数）。件数を固定した例外は `crates/core/src/command_class.rs` の `skill` が 3 件（コマンドの種類 `skill` と付録 A の `skill` の実際の衝突。増減すると失敗する）
- 回帰テスト（`make test-check-generic-words`）は、実 repo で 0、語を足した写しで 1、`.rs` のコメント・`tests/` の語は 0、static のコメントの語は 1、Codex が指摘した読み違いの例（上記）がすべて 1、例外の件数の増減で 1、判定できない入力で 2 を確かめる

CI・`make check`・lefthook には組み込んでいない。失敗したら止めるガードを足すことは、仕様が人間の承認点に挙げている変更だからである。組み込むときは `ci.yml` の変更と、ruleset の必須チェックの更新が要る。

実行結果（2026-10-17）。`make test-browser` は 314 件中 312 件が通過し、2 件（`ime-input` の「送信の直後に変換が始まっても…」。CSP あり・なし）が 30 秒のタイムアウトで失敗した。本変更と無関係な試験で、個別に流し直すと 23 件すべてが通過した（負荷による揺れと見られる。原因は未調査）。新しい試験（N1〜N3b の 12 件と、検査器の自己確認 T1〜T4）は通過した。`make test-check-generic-words`（17 件。Codex の指摘の修正後は 42 件で再実行し、通過した）・`make check-generic-words`・`make check`・`make lint`・`make test`・`make check-deps`・`make check-dom-reads` も通過した。`make check-delivery` は流していない（未検証）。

static を素の文字列の照合へ作り直した後（2026-10-17）は、`make test-check-generic-words`（48 件）・`make check-generic-words`・`make check` を流し、通過した。実 repo の static で、コメントの語による新たな違反は 0 件だった。

### 5.85 送信の直後に変換が始まったときの書き戻し（#1510）

5.76・5.78 に範囲外として記した `submit-wiring.test.mjs` の「送信の直後に変換が始まっても…」の 2 件（CSP あり・なし）の失敗は、負荷による揺れでも試験の待ち方の誤りでもなく、glue の不具合だった。導入した 31bde1d（TASK-438）の時点から、単独の実行でも毎回失敗する（2026-10-17 に 31bde1d と d9a8b0c で確認）。

- 原因: 送信で空にした下書きの書き戻しは、変換中は保留になる。上流の `Runtime` は root の `compositionend`（泡立ちの段）で欄の値を確定値として状態へ届ける（FEND-10）。glue の `compositionend` のリスナーは上流より後に登録されるため、保留した書き戻しより先に、欄に残った送信済みの本文が状態へ戻っていた。欄は空にならず、続く Enter で同じ本文が 2 件目として送られた（誤送信。修正前のビルドで記録 2 件を確認）
- 修正: `crates/web/src/glue.rs` の `compositionend` のリスナーを捕捉の段（capture）で登録し、書き戻しの保留（ホストの要求で立て、変換中でない `sync_draft` が下ろす）があるときだけ、上流が欄を読む前に反映する。保留が無いときは書かない（変換中は上流が `input` を延期するため状態は変換前のままで、書くと確定した文字を消す）。保留の印は書き戻し 1 回分で下ろす（変換中でない `sync_draft` は、欄が無い・App を借りられず書けなかったときも下ろし、root が無いタイマーも下ろす。下ろす口は `clear_draft_sync_pending` の 1 つ）。DOM の値の読み取り（`field.value()` の 1 件）と状態を変える経路は増やしていない
- 契約: `crates/web/tests/glue_contract.rs` のリスナーの登録に `add_event_listener_with_callback_and_bool` を足し、第 1 引数のイベント名を同じ許可の表で照合する（負例 2 件を追加）。`GlueOp` は変えないため `PARTS_CONTRACT_VERSION` は 1 のまま
- 試験: 欄が空になるのに加え、状態の下書きも空になること、続く Enter で 2 件目が記録されないこと（誤送信 0）を確かめる。変換中に書き戻さないことの確認は、固定の 100 ミリ秒の待ちをやめ、書き戻しのタイマーの後ろに積んだタイマーで待つ。書き戻しのタイマーが欄を見つけられない経路（送信の直後に欄の `data-testid` を一時的に変える）の後の通常の変換で、確定した文字が欄と状態の下書きに残ることも確かめる（印が残ると捕捉の段が変換前の下書きを書き、文字が消える。修正前のビルドでは 2 件とも「確定した文字が欄に残る」で失敗）
- 残る限界: 送信から書き戻しのタイマーまでの間に始まった変換で文字を確定すると、変換の終了時に保留した書き戻し（空）が勝ち、確定した文字は欄に残らない（TASK-438 の設計どおり）。Safari のように `compositionend` が確定の `input` より前に来るブラウザでも同じで、印が立っている間（送信から書き戻しのタイマーまでの間に変換が始まった場合）は、捕捉の段の書き戻しが確定した文字より先に欄へ入り、確定した文字は消える。欄の値から送信済みの本文を除いて残す形は、DOM の値を状態へ流すことになり BUILD-6 に反するため採らない。glue の結線は native の試験で検出できないため、ブラウザ試験が担う

```text
実行日: 2026-10-17
元にしたコミット: d9a8b0c-dirty
コマンド: node --test --test-name-pattern='送信の直後に変換が始まっても' tools/browser/submit-wiring.test.mjs（単独で 10 回）・make test-browser（2 回）
```

| 実行 | 結果 |
|---|---|
| 修正前（d9a8b0c・31bde1d）の単独実行 | 2 件とも `waitEmpty` の 30 秒の時間切れで失敗 |
| 修正後の単独実行 10 回 | 10 回とも 2 件が通過 |
| 修正後の `make test-browser` 2 回 | 2 回とも 302 件がすべて通過（ime-input・ime を含む） |

保留の印の寿命の修正（レビューの反映）の記録。

```text
実行日: 2026-10-17
元にしたコミット: 14c630e-dirty
コマンド: node --test tools/browser/submit-wiring.test.mjs（単独で 5 回）・make test-browser（1 回）
```

| 実行 | 結果 |
|---|---|
| 修正前（14c630e）のビルドで、欄を見つけられない経路の試験 2 件 | 2 件とも「確定した文字が欄に残る」で失敗（欄・状態とも空） |
| 修正後の `submit-wiring.test.mjs` 単独 5 回 | 5 回とも 10 件がすべて通過 |
| 修正後の `make test-browser` 1 回 | 316 件がすべて通過 |

### 5.86 Enter 系のキーの不変条件の基準（TASK-351 1/2）

変換中でない状態の Enter・Shift+Enter・修飾キー付き Enter について、観察できる結果（keydown の `preventDefault` の有無・右のログの行・入力欄の値）を、受け持ちに依らない表として固定した（FEND-21）。受け持ちは現状の glue（D-42）。TASK-352 が受け持ちを置き換えるとき、置き換えの前後で `tools/browser/key-invariants.test.mjs` を同じ件数で通して比べる基準である。Tab・Shift+Tab の行は 5.86b（TASK-351 2/2）で同じファイルに足した。

- 観察の方法: 配信設定（`TEST_HOOKS` なし）だけを使い、内部の印（`__fandheTest`・`data-action-keydown`）は読まない。入力は信頼入力（`page.keyboard.press`）。`preventDefault` は、`window` の捕捉の段で入力欄が対象の keydown の Event を保持し、押した後にその `defaultPrevented` を読む（誰がどの段で呼んでも観察できる）。押した 1 回につき Enter の keydown がちょうど 1 件であることも確かめる
- 表の列: `id`・`press`・`draft`・`prevented`・`lines`・`value`。各行は新しいページで実行する。ENTER-TEXT は送信後の空化（TASK-438）を待ってから欄の値を確かめる。送信が無い行は、タイマーを 2 回挟んだ後で 0 件を確かめる
- 現状で観察した値は、事前の見込み（`decide` と既存の試験）と全行で一致した。食い違いは無い

| id | キー | 下書き | preventDefault | ログの行 | 欄の値 |
|---|---|---|---|---|---|
| ENTER-TEXT | Enter | `x` | あり | `x` | 空 |
| ENTER-EMPTY | Enter | 空 | あり | なし | 空 |
| SHIFT-ENTER-TEXT | Shift+Enter | `x` | なし | なし | `x` と改行 |
| SHIFT-ENTER-EMPTY | Shift+Enter | 空 | なし | なし | 改行のみ |
| CTRL-ENTER | Control+Enter | `x` | なし | なし | `x` |
| ALT-ENTER | Alt+Enter | `x` | なし | なし | `x` |
| META-ENTER | Meta+Enter | `x` | なし | なし | `x` |
| SHIFT-CTRL-ENTER | Shift+Control+Enter | `x` | なし | なし | `x` |
| SHIFT-ALT-ENTER | Shift+Alt+Enter | `x` | なし | なし | `x` |
| SHIFT-META-ENTER | Shift+Meta+Enter | `x` | なし | なし | `x` |
| CTRL-ALT-ENTER | Control+Alt+Enter | `x` | なし | なし | `x` |

```text
実行日: 2026-10-17
元にしたコミット: 0b607a7-dirty
コマンド: node --test tools/browser/key-invariants.test.mjs（単独で 3 回）・make test-browser（1 回）
```

| 実行 | 結果 |
|---|---|
| `key-invariants.test.mjs` 単独 3 回 | 3 回とも 12 件（表 11 行 + 行数の照合 1 件）がすべて通過 |
| `make test-browser` 1 回 | 331 件がすべて通過（ローカルの計測。CI には未組み込み） |

負の対照（製品コードを一時的に変えてビルドし、単独で実行した。変更は戻し、コミットしていない）。

| 変更 | 結果 |
|---|---|
| `glue.rs` の keydown で `prevent_default()` を呼ばない | ENTER-TEXT・ENTER-EMPTY の 2 件が失敗（ENTER-TEXT は改行が入り空にならない） |
| `key_route.rs` の `Newline` を送信（click と `preventDefault`）に変える（JUDGE13-1 相当） | SHIFT-ENTER-TEXT・SHIFT-ENTER-EMPTY の 2 件が失敗 |

残る限界: Playwright の `keyboard.press` は、Shift 以外の修飾キーを押している間は text を送らない。修飾キー付き Enter の OS ごとの差（とくに Windows）は D-48 で別に扱う。変換中の Enter は対象外（`ime-input`・`enter-submit` の担当）。

### 5.86b Tab 系のキーの不変条件の基準（TASK-351 2/2）

Tab・Shift+Tab について、keydown の `preventDefault` の有無・ログの行・欄の値・押した後のフォーカス先を、5.86 と同じファイル `tools/browser/key-invariants.test.mjs` の表に 8 行足した（FEND-21）。Tab は、候補の一覧が開いていて入力欄に文字があるときだけ `preventDefault` され、それ以外では呼ばれない。Shift+Tab は常に `preventDefault` されない（JUDGE-19）。受け持ちは現状の glue（D-42）。TASK-352 は、置き換えの前後でこのファイルを同じ件数（20 件）で通して比べる。

- 観察の方法の差分: 配信設定は IR を読まず候補の一覧が常に閉じるため、一覧が開いている行（TAB-OPEN・SHIFT-TAB-OPEN）と、一覧が閉じたままの `/zzz`・`/help` の行は試験の設定（`TEST_HOOKS=1`。埋め込み教材 `min-3step`）で観察する。基準を 1 ファイルに置き、置き換えの前後で件数を比べるためである。内部の印（`__fandheTest`・`data-action-keydown`）を読まない原則は保ち、DOM と捕捉した Event の `defaultPrevented` だけで観察する。入力は `FANDHE_WEB_DIR` と `FANDHE_WEB_TEST_DIR` の両方で、どちらかが未設定・不在なら skip でなく失敗にする
- 列の追加（Enter の 11 行にも付与した）: `web`（`delivery` か `test`）・`open`（押す前の一覧の開閉。前提として照合する）・`key`（捕捉する `KeyboardEvent.key`）・`focus`（押した後の `document.activeElement` の `data-testid`。`body` は `BODY`）。ログの行は「押す前の行 + `lines` = 押した後の行」で照合する。欄の値が変わる行は `waitForFunction` で待つ（Tab の補完は `set_timeout(0)` を 2 段通る）
- Enter の 11 行の `focus` は全行 `draft-input` で、事前の見込みと一致した

| id | web | 一覧（前提） | キー | 下書き | preventDefault | ログの行 | 欄の値 | フォーカス先 |
|---|---|---|---|---|---|---|---|---|
| TAB-TEXT | 配信 | 閉 | Tab | `x` | なし | なし | `x` | `submit-input` |
| TAB-EMPTY | 配信 | 閉 | Tab | 空 | なし | なし | 空 | `submit-input` |
| SHIFT-TAB-TEXT | 配信 | 閉 | Shift+Tab | `x` | なし | なし | `x` | `BODY` |
| SHIFT-TAB-EMPTY | 配信 | 閉 | Shift+Tab | 空 | なし | なし | 空 | `BODY` |
| TAB-OPEN | 試験 | 開 | Tab | `/he` | あり | なし | `/help` | `draft-input` |
| SHIFT-TAB-OPEN | 試験 | 開 | Shift+Tab | `/he` | なし | なし | `/he` | `keydown-probe` |
| TAB-NOMATCH | 試験 | 閉 | Tab | `/zzz` | なし | なし | `/zzz` | `submit-input` |
| TAB-EXACT | 試験 | 閉 | Tab | `/help` | なし | なし | `/help` | `submit-input` |

現状で観察した値は、事前の見込み（`decide` と既存の試験）と全行で一致した。食い違いは無い。SHIFT-TAB-OPEN のフォーカス先 `keydown-probe` は、試験の設定にだけある入力欄の前の要素で、配信設定では `BODY` へ出る（SHIFT-TAB-TEXT）。

```text
実行日: 2026-10-17
元にしたコミット: 669ab6e-dirty
コマンド: node --test tools/browser/key-invariants.test.mjs（単独で 3 回）・make test-browser（1 回）
```

| 実行 | 結果 |
|---|---|
| `key-invariants.test.mjs` 単独 3 回 | 3 回とも 20 件（表 19 行 + 行数の照合 1 件）がすべて通過 |
| `make test-browser` 1 回 | 339 件がすべて通過（5.86 の 331 件から 8 件増。ローカルの計測。CI には未組み込み） |

負の対照（製品コードを一時的に変えて試験の設定を再ビルドし、単独で実行した。変更は戻し、コミットしていない）。

| 変更 | 結果 |
|---|---|
| `key_route.rs` の `KeyAction::Complete` を `KeydownEffect::NONE` にする（JUDGE19-2 相当） | TAB-OPEN の 1 件が失敗 |
| `key_decide.rs` の Tab の条件から `!input.shift` を外す（JUDGE19-3 相当） | SHIFT-TAB-OPEN の 1 件が失敗 |

TASK-352 の比較対象の件数（2026-10-17、`669ab6e` に本変更を加えた状態の単独実行）: `key-invariants` 20 件、`tab-cycle` 11 件、`tab-complete` 4 件、`shift-tab` 6 件、`candidates` 18 件。

残る限界:

- 一覧が開いていて欄が空の組み合わせは画面から作れない（空にすると一覧は閉じる）。contract の `key_decide_contract`（JUDGE19-1）が固定している
- Esc で閉じた後に文字が残る状態の Tab は、2 回押す操作になり 1 行 1 押下の表に収まらない。`candidates`（K-06）の担当
- Tab の一巡・罠 0 件は `tab-cycle` の担当。修飾キー付き Tab は対象外
- OS ごとの差（とくに Windows）は D-48 で別に扱う

### 5.87 前後の行データと変化の印の導出（TASK-468 1/2）

世界状態とイベント列から左のビューの行の前後の組を導く `artifact_rows::derive_rows` について、sample の 7 種 × 前後で項目数と印の数が期待値と一致し、前の状態で印が 0 件であることを native の試験で固定した（ART-2・ART-3）。

- 受入（`crates/web/tests/artifact_rows_sample.rs`）: 6 種の期待値は導出のコードを通さず、sample の JSON から直接数える。文書は JSON が無いため、手で数えた固定値（前 10・後 13、`+3 -3 ~0`）を使う。行ごとの印、削除の行（後の状態で前の中身と削除の印で残る）、`Projection::marks` を `change_mark_of` で写した印との照合、印の総数と `world_diff` の件数の一致、7 種の行の ID が世界状態の経路を重ならず覆うことも確かめる
- 文書は 1/2 では、変更前後のブロックを別の経路（`-old`・`-new`）として置き、他の 6 種と同じ汎用の導出で通した。変更を `-` と `+` の 2 行にする突き合わせは 2/2（#367）が持つ
- 規則（`artifact_rows_contract.rs`）: 値を戻した経路は印なし、置いてから消した経路とビューの外の要素は行にならない、ビューの外へ出入りした要素は削除・追加、読み方の誤りと空の経路は `Err`、導出した全行が指せて `Input::Ui` の `path` が世界状態の経路と一致、`Changed` 以外のイベントは無視、決定的
- 変異: ART2-2 は後の世界状態に `Changed` を当てず、ART2-3 は走査を後の世界状態の鍵だけにし、ART2-4 は前後とも読めない経路を飛ばさない。ART2-4 の最初の案（置いてから消した経路の試験）は、経路がどちらの世界状態にも無く走査に入らないため生き残った。試験を「ビューの外の要素は行にならない」に替えて検出された

```text
実行日: 2026-10-17
元にしたコミット: 74dcd72-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART2-2.mutation crates/web/tests/mutations/ART2-3.mutation crates/web/tests/mutations/ART2-4.mutation
```

| ID | 結果 | 検出した試験（期待した失敗を含む） |
|---|---|---|
| ART2-2 | DETECTED | 受入の試験が複数失敗（期待した `六種の前後の項目数と印の数がjsonから数えた期待値と一致する` を含む） |
| ART2-3 | DETECTED | 受入の試験が複数失敗（期待した `削除の行は後の状態で前の中身と削除の印で残る` を含む） |
| ART2-4 | DETECTED | `ビューの外の要素は行にならない` |

### 5.88 入力欄の Enter・Tab・Space の受け持ち（TASK-352 1/3）

入力欄の Enter・Tab・Space の状態に依存する `preventDefault` は、D-42 のとおり glue が受け持つ（属性化も上流の口も採らない。FEND-7）。Enter・Tab は TASK-351 の `key-invariants`（5.86・5.86b）が固定している。本節では、試験が無かった Space を `tools/browser/field-space.test.mjs` と native の契約試験で固定した。Space は判定が `Nothing` で、glue は何もせず、textarea の既定動作で空白が入る。

- 本変更は glue の動作を変えない（コメントと試験の追加だけ）。そのため「置き換えの前後」は無く、TASK-352 3/3 が比べる起点は 5.86b の件数である
- `key-invariants` に行を足さなかったのは、基準の件数（20 件）を保つため。観察の方法は `key-invariants` と同じ（`window` の捕捉の段で keydown の Event を保持し、押した後に `defaultPrevented` を読む。内部の印は読まない）
- native: `key_route_contract.rs` の `space_は状態に関わらず_prevent_default_せず送信もしない` が、`has_text` と `candidates_open` の 4 通りで `Nothing` かつ効果なしを確かめる。変異 `FEND7-1`（`Nothing` の腕を `preventDefault` ありにする）の検出先

| id | web | 一覧（前提） | 下書き | preventDefault | ログの増分 | 欄の値 | フォーカス先 |
|---|---|---|---|---|---|---|---|
| SPACE-TEXT | 配信 | 閉 | `x` | なし | なし | `x` と空白 | `draft-input` |
| SPACE-EMPTY | 配信 | 閉 | 空 | なし | なし | 空白 1 つ | `draft-input` |
| SPACE-OPEN | 試験 | 開 | `/he` | なし | なし | `/he` と空白 | `draft-input` |

観察した値は、事前の見込みと全行で一致した。食い違いは無い。SPACE-OPEN は押した後の一覧の開閉を照合しない（候補の絞り込みは `candidates` の担当）。

```text
実行日: 2026-10-17
元にしたコミット: 02b76ce-dirty
コマンド: node --test tools/browser/field-space.test.mjs（単独で 3 回）・TASK-351 の 5 ファイルの単独実行・make test-browser（1 回）・scripts/check-mutations.sh crates/web/tests/mutations/FEND7-1.mutation
```

| 実行 | 結果 |
|---|---|
| `field-space.test.mjs` 単独 3 回 | 3 回とも 4 件（表 3 行 + 行数の照合 1 件）がすべて通過 |
| TASK-351 の比較対象の単独実行 | `key-invariants` 20 件・`tab-cycle` 11 件・`tab-complete` 4 件・`shift-tab` 6 件・`candidates` 18 件の計 59 件がすべて通過（5.86b と同じ件数） |
| `make test-browser` 1 回 | 343 件がすべて通過（5.86b の 339 件から 4 件増。ローカルの計測。CI には未組み込み） |
| 変異 `FEND7-1` | DETECTED（期待した試験 `space_は状態に関わらず_prevent_default_せず送信もしない` を含む複数の試験が失敗） |

負の対照（製品コードを一時的に変えて両方の設定を再ビルドし、単独で実行した。変更は戻し、コミットしていない）。

| 変更 | 結果 |
|---|---|
| `glue.rs` の keydown で、効果に関わらず `ev.prevent_default()` を呼ぶ | `field-space` の 3 件が失敗（空白が入らず、`waitForFunction` が時間切れになる） |

TASK-352 3/3 のための参考値（正式な記録は 3/3 が行う。2026-10-17、`02b76ce` 時点）:

- `crates/web/src/glue.rs`: `wc -l` で 482 行（本変更のコメント 2 行を含む。コメント・試験用の口・空行を含む数え方）。FEND-7 は決定時点で 349 行と記録しており、数え方（コメント・試験用の口を含むか）と、その後の機能追加で差がある
- `web-sys` の feature: ルート `Cargo.toml` の `[workspace.dependencies]` に 21 個（決定時点の記録は 9 個）。本変更は feature を足さない
- D-42 で上流の口を採らなかったため、3/3 の「上流の口を採用する場合」という前提は成り立たない。3/3 は記録と比較だけになる見込みである

限界: 修飾キー付き Space・変換中の Space（`ime-input` の担当）・OS ごとの差（D-48）は対象外。

### 5.89 再読み込みの 33 地点の一致（TASK-319 2/3）

3 Step 以上の教材の途中で再読み込みしたとき、同じ Step・同じ左の状態・同じ復習・同じログ・同じ入力欄の値で再開すること（世代を除く）と、各地点から完走した最終の世界状態が再読み込みなしの完走と一致することを、native の 33 地点で確かめる（SAVE-3）。経路は本番と同じ保存→起動（`RecordedSession::envelope_for_save` → `export_bytes` → `boot_session`）で、`Session::reload` を直接呼ぶだけの試験にはしない。自動ブラウザの 15 地点は 5.90（#343）が持つ。

出所は PoC-6 の native 試験（v1.0.0 の 1 周目 16 Step を誘導つきで進め、33 地点で再読み込み）である。ただし本物の教材は、`on_enter` の効果・モジュールの出口・遷移の `when: param`・`RunState.completed` を立てる処理が core に未実装のため、今のエンジンでは完走できない（2026-10-17、`85d03e6` で確認）。そこで 16 Step の形（入力方式・確認の種類・処理中の数）だけを写した平らな 1 本の教材を、手書きの IR として `crates/core/tests/data/reload_points_1/ir.json` に置いた。未実装の語は使わず、完走は最後の Step（`final`）の達成で判定する（`completed` が実装されてもそのまま通る）。

地点の数え方。1 地点は「誘導つきの 1 手を打つ前の、完走前の状態」で、各地点で地点ごとに違う下書きを記録してから保存する（未操作の履歴は復元されない SAVE-28 の対象外にし、入力欄の値の比較を空同士にしないため）。

| 種類 | 数 | 内容 |
|---|---|---|
| 入った直後 | 16 | 各 Step の入口 |
| 処理中 | 5 | `processing` を持つ 2 つの Step（3 件と 2 件）の進行の前 |
| 確認待ち | 12 | 確認が `none` 以外の Step の受付後 |
| 合計 | 33 | PoC の下限つきの assert ではなく、数を固定する |

| 試験 | 内容 |
|---|---|
| `crates/core/tests/reload_points.rs` の地点の巡回 | 33 地点のそれぞれで、世代がちょうど +1、`run`（Step・左の状態・切り身・処理中）・下書き・チェックポイント・入口の写し・ヒントが世代を除いて一致、ログは前のログに `Reloaded` が 1 件足されたもの、復習が一致、復元した側を完走させた世界状態が再読み込みなしの完走と一致。地点の数（33・16・5・12）を `assert_eq!` で固定し、`OBS` 行で出す |
| 同ファイルのデータの形 | IR が 16 Step で、処理中の合計が 5、`final` が最後の 1 つだけ、確認が `none` でない Step が 12、未実装の語（`on_enter`・`use`・`exit_to`・条件つきの `next`・規則の `when`）を含まない |

変異の定義（`crates/core/tests/mutations/SAVE3-*.mutation`）。

| ID | 誤り | 検出する試験 |
|---|---|---|
| SAVE3-1 | 再読み込みで処理中を止める | 地点の巡回（処理中の地点） |
| SAVE3-2 | 再読み込みで下書きを捨てる | 地点の巡回 |
| SAVE3-3 | 復元で再読み込みをかけない | 地点の巡回（世代 +1・`Reloaded`） |
| SAVE3-4 | 再読み込みで Step の状態（切り身）を捨てる | 地点の巡回 |

```text
実行日: 2026-10-17
元にしたコミット: 85d03e6-dirty
コマンド: cargo test --locked -p fandhe-workshop-core --test reload_points -- --nocapture・scripts/check-mutations.sh（SAVE3-1〜4）
```

| 実行 | 結果 |
|---|---|
| `reload_points`（`OBS Counts { points: 33, entered: 16, busy: 5, awaiting_confirm: 12 }`） | 2 件とも通過 |
| SAVE3-1〜SAVE3-4 | 4 件とも DETECTED |

残る限界。CH-2 の実データでの網羅は、未実装の語が入った後に v1.0.0 の 1 周目へ切り替える。世代を除く一致の確認は復元した側で行い、web の画面での一致は 5.90（#343）が持つ。

### 5.90 自動ブラウザの 15 地点の一致（TASK-319 3/3）

実際の画面で `page.reload()` をかけたとき、CH-2 に相当する 15 地点（入った直後 8・確認待ち 7）のすべてで、再読み込みの前後の状態（Step・左の状態・復習・ログ・入力欄の値）が世代を除いて一致し、各地点から完走した最終の世界状態が再読み込みなしの完走と一致することを確かめる（SAVE-3）。出所は PoC-6 の S-1（地点 15・`busy` は 0。各手は処理中が終わるのを待ってから再読み込みした）。native の 33 地点は 5.89 が持つ。

教材。`crates/web/tests/data/reload_points_ch2/ir.json` は、5.89 の 16 Step の教材（`crates/core/tests/data/reload_points_1/ir.json`）の先頭 8 節点の写しで、CH-2（`st-2-1`〜`st-2-8`）と同じ形（入力方式と確認の種類）を持つ。`st-8` を `final` にし、`pack.id` を `reload-points-ch2`（mount の既定の `min-3step` と別）にした。処理中は `st-2` の 3 件だけである。core の試験データを crate をまたいで読まないのは、web の試験が core の都合で黙って変わらないようにするためである。未実装の語は使わない。

起動の前に教材を選ぶ口（試験の設定だけ）。教材の差し替え `load-fixture:` はマウントの後に働き、保存先の鍵（`pack.id`）が `min-3step` のままになるため、再読み込みからの復元を試せない。そこで、URL のクエリが `?fixture=reload-points` に完全一致したときだけ、`persist::mount_with_history` が起動（`boot`）の前に 8 Step の教材を選ぶ（`test_fixture::mount_fixture_from_search`）。空・未知の名前・余分な文字・他のキーとの併用はすべて既定の `min-3step` に倒す。クエリを読む処理は、走査の対象外の `test_hooks.rs` に置き、配信する経路に URL の読み取りが無いこと（`tests/tick_interval.rs`）を保つ。配信物にこの教材の ID や `fixture=` は入らない（`make check-delivery`・`make test-build-web` で確認）。

地点の数え方。1 地点は「誘導つきの 1 手を打つ前の、完走前の状態」で、各地点で地点ごとに違う空でない下書きを入れてから再読み込みする（未操作の履歴は復元されない SAVE-28 の対象外にし、入力欄の値の比較を空同士にしないため）。

| 種類 | 数 | 内容 |
|---|---|---|
| 入った直後 | 8 | 各 Step の入口 |
| 確認待ち | 7 | 確認が `none` 以外の Step の受付後（`st-4` は `none`） |
| 処理中 | 0 | 各手の後に進行を終えてから次へ進むため地点に現れない（PoC の S-1 と同じ）。処理中の地点は 5.89 の native が持つ |
| 合計 | 15 | 数を固定する |

手の経路。15 手のうち、実際の画面が 8 手（`st-1` 受付、`st-2` 受付と確認、`st-3` 受付、`st-6` 受付と確認、`st-7` 確認、`st-8` 受付）、テスト専用の口（`raw-trigger`）が 7 手（`st-1`・`st-3`・`st-5`・`st-8` の確認、`st-4`・`st-5` の受付、`st-7` の受付）である。spec の検証方法の「10 手が画面・5 手がテスト専用」とは違う。差の理由は、入力方式 choice の画面の口（TASK-450 1/3。#404）と、確認の point_at・ui_action の `Input::Ui` への結線が未実装なことである。試験を 10/5 に合わせるために教材の形は変えない。これらが入った後は、実際の画面の手へ切り替える余地がある。

| 試験 | 内容 |
|---|---|
| `tools/browser/reload-points.test.mjs` の地点の巡回（CSP あり・なし） | 15 地点のそれぞれで新しい context を開き、誘導つきの手を打って下書きを入れ、`page.reload()` する。起動の種類が `restored`、世代がちょうど +1、engine のログは前のログに `reloaded` が 1 件足されたもの、ログを除いた状態（世代を除く）・復習・左の行・現在の Step・状態表示が一致、画面の左の行と復習の行が一致、画面のログの行は前の行に告知（SAVE-6）が 1 件足されたもの、入力欄の値とホストの下書きが一致、鍵が教材の `pack.id` の側にだけ書かれる、復元した側から完走した世界状態が再読み込みなしの完走と一致、CSP 違反・ページ内エラー・console のエラーが 0 件。地点の数と手の経路の数を `assert` で固定し、`OBS` 行で出す |
| 同ファイルの対照（CSP あり・なし） | クエリなし・`?fixture=unknown`・`?fixture=reload-points&x=1` では `min-3step` で始まり、鍵が `min-3step` の側に書かれる（教材の選択が完全一致だけであること） |
| `crates/web/tests/reload_points_host.rs` の地点の巡回 | 同じ教材と誘導で、web の保存の経路（ホストの記録・`history_to_save`・保留と書き出し・偽物の保存先・`boot`）を native で通す。状態全体（世代を除く）・下書き・左の行・復習・世界状態が一致し、完走した世界状態が一致。地点（15・8・7・0）と手の経路（8・7）を固定する。変異の検出先 |
| 同ファイルのデータの形 | IR が 8 Step、処理中の合計が 3、`final` が最後の 1 つだけ、確認が `none` でない Step が 7、未実装の語を含まない、`pack.id` が `min-3step` と違う |
| `test_fixture` の単体 | 完全一致のクエリだけが 8 Step の教材を選ぶ。選んだ教材は鍵が別で 8 Step を持つ（`cargo test -p fandhe-workshop-web --features test-hooks --lib`） |

変異の定義（`crates/web/tests/mutations/SAVE3-*.mutation`。`SAVE3-1`〜`SAVE3-4` は core）。

| ID | 誤り | 検出する試験 |
|---|---|---|
| SAVE3-5 | 復元したログから世界状態を導き直さず空にする | `reload_points_host` の地点の巡回 |
| SAVE3-6 | 復元したログから復習・変化の印を導き直さず空にする | 同上 |
| SAVE3-7 | 復元した下書きを入力欄へ戻さない | 同上 |

教材の選択（`test_fixture`）は native の統合試験には組み込まれないため、変異の対象にせず、負の対照を手で一度流した。`?fixture=` の比較を常に偽にして再ビルドすると、ブラウザの試験の地点の巡回が失敗する（完走の基準を作る段階で止まる）ことを確かめ、変更は戻した。

```text
実行日: 2026-10-17
元にしたコミット: dc72a67-dirty
コマンド: cargo test --locked -p fandhe-workshop-web --test reload_points_host -- --nocapture・scripts/check-mutations.sh（SAVE3-5〜7）・make test-browser・make lint・make test・make check-deps・make check-dom-reads・make test-build-web・make check-delivery
```

| 実行 | 結果 |
|---|---|
| `reload_points_host`（`OBS Counts { points: 15, entered: 8, busy: 0, awaiting_confirm: 7, screen: 8, raw: 7 }`） | 2 件とも通過 |
| `reload-points`（CSP あり・なしとも `OBS {"points":15,"entered":8,"busy":0,"awaitingConfirm":7,"screen":8,"raw":7}`） | 4 件とも通過（単独で約 46 秒） |
| SAVE3-5〜SAVE3-7 | 3 件とも DETECTED |
| 手で流した負の対照 | ブラウザの地点の巡回が失敗（CSP あり） |
| `make test-browser` | 355 件がすべて通過（`reload-points` の 4 件を含む） |
| `make lint`・`make test`・`make check-deps`・`make check-dom-reads`・`make check-generic-words` | 通過 |
| `make test-build-web`・`make check-delivery` | 通過（配信物に試験用の口・試験の教材の ID・`fixture=` が無い） |

残る限界。CH-2 の実データ（v1.0.0 の `idea-cycle`）での網羅は、`on_enter`・モジュールの出口・`when` などの未実装の語が core に入った後になる。処理中の地点は native の 33 地点（5.89）が持つ。`make test-browser` は CI に組み込まれていない（組み込みは必須チェックの更新と合わせて扱う）。

### 5.91 標準でない要素の Enter・Space と送信ボタンの一致（TASK-352 2/3）

受け持ちは D-42 のとおり glue に残す（属性化も上流の口も採らない。FEND-7）。そのため「置き換え」は起きず、3/3 は 5.88 と本節の記録と比べるだけになる見込みである。本変更は glue のコードを変えない。

1. FEND-14 の 2 場面を `tools/browser/submit-disabled.test.mjs` で固定した。配信設定の画面で、Playwright の信頼入力（`page.keyboard.press`）を使い、内部の印は読まない。
2. 標準でない要素（`data-action` を持つ左の行など）の Enter・Space の判定を、純関数 `key_route::row_key_effect(input, repeat, pointable)` にした。native の契約試験と変異で固定している。

| 場面 | 作り方 | 確かめたこと |
|---|---|---|
| DISABLED | 下書きを入れ、試験の側で送信ボタンに `disabled` を立てる | Enter の keydown は `preventDefault` される・ログの増分は 0・欄の値は変わらない。DOM の `click()` を呼んでも増分は 0 |
| ENABLED | 2 つのコンテキストで、片方は click、もう片方は Enter で送る | ログの行の本文と件数が一致し、送信後の欄は空 |

無効なボタンへの `HTMLElement.click()` は Chromium で click を配らないことを観察で確かめた（HTML の仕様どおり）。このため「無効なら 0 件」は click 経路そのものが持つ性質で、`dispatch_action("submit")` のように無効なボタンを素通りする経路は試さない。製品の画面には送信ボタンが無効になる状態がまだ無く（空の下書きと処理中は core が拒否する。EVT-7）、無効の場面は試験の側で作った。

`row_key_effect` の判定（上から順に評価する）。

| 条件 | 結果 |
|---|---|
| `pointable` が偽（宛先の無い行） | 何もしない（Space の既定のスクロールも妨げない） |
| 変換中 | 何もしない |
| Ctrl・Alt・Meta・Shift のいずれかが付く | 何もしない（A11Y-18 は定めておらず、保守側に倒した） |
| `key` が `"Enter"` か `" "` | `preventDefault`。`repeat` でなければ click |
| それ以外のキー | 何もしない |

変異（`crates/web/tests/mutations/`）。

| ID | 誤り | 検出する試験 |
|---|---|---|
| FEND7-2 | 行の Space の腕を外す | `key_route_contract` の `fend7_標準でない要素の_enter_と_space_は_prevent_default_と_click_になる` |
| FEND14-1 | 修飾なしの Enter の `click_submit` を偽にする | `key_route_contract` の `fend14_修飾なしの_enter_は送信ボタンへの_click_と_prevent_default_になる` |

```text
実行日: 2026-10-17
元にしたコミット: 4835336-dirty
コマンド: node --test tools/browser/submit-disabled.test.mjs（単独で 3 回）・TASK-351 の 5 ファイルの単独実行・make test-browser（1 回）・scripts/check-mutations.sh（FEND7-2・FEND14-1）
```

| 実行 | 結果 |
|---|---|
| `submit-disabled.test.mjs` 単独 3 回 | 3 回とも 2 件がすべて通過 |
| TASK-351 の比較対象の単独実行 | `key-invariants` 20 件・`tab-cycle` 11 件・`tab-complete` 4 件・`shift-tab` 6 件・`candidates` 18 件の計 59 件がすべて通過（5.86b・5.88 と同じ件数） |
| `make test-browser` 1 回 | 357 件がすべて通過（本変更前は 355 件。ローカルの計測。CI には未組み込み） |
| FEND7-2・FEND14-1 | 2 件とも DETECTED |

負の対照（`glue.rs` の送信ボタンへの `button.click()` を一時的に外して再ビルドし、単独で実行した。変更は戻し、コミットしていない）。

| 変更 | 結果 |
|---|---|
| `wire_enter_submit` の `button.click()` を外す | ENABLED の 1 件が失敗（Enter では送信されない）。DISABLED は通る |

TASK-352 3/3 のための記録（2026-10-17、`4835336` 時点）。

- `crates/web/src/glue.rs`: `wc -l` で 500 行（本変更は変えない。5.88 の 482 行からの増分は、5.88 以後に入った別タスクの変更による）
- `web-sys` の feature: ルート `Cargo.toml` の `[workspace.dependencies]` に 21 個（5.88 と同じ。本変更は足さない）
- TASK-351 の件数: 59 件（同じ件数で通る）

左の行の Enter・Space の結線は、行がまだ画面に無く、行を見分ける口（`closest`・`get_attribute`）が `tools/check-deps.sh` と `glue_contract` で禁じられているため、本変更では行わない。TASK-358 1/2（#398）と TASK-480 が、`row_key_effect` を呼ぶ結線と `pointable` の出所を受け持つ。

限界: 修飾キー付きの行の Enter・Space は A11Y-18 が定めておらず、spec 側での確定が要る。送信ボタンを製品で無効にする状態は spec に無い。

### 5.92 上流の口を採らない場合の前後の比較と glue の規模（TASK-352 3/3）

受け持ちは D-42 の案 (c)（glue を維持する）で決まっており、上流への追加要望も行わない（FEND-7・FEND-8）。そのため「上流の口への置き換え」は起きない。3/3 は、TASK-352 の 3 件が glue のコードを変えていないことの確認と、前後の試験の件数・glue の規模の記録だけを行う。コードと試験は変えていない。

「前」は TASK-352 に着手する前の基準（5.86b。`669ab6e`）、「後」は 3 件の完了後（`bddd086`）とする。

TASK-352 の 3 件は、1/3 が `85d03e6`、2/3 が `bddd086`、3/3 が本変更（コードを変えない記録のみ）である。1/3 は `glue.rs` に 2 行を足したが、どちらも `//` のコメント行（受け持ちが glue にあることと Space の扱いの説明）で、コード行の増分は 0 である（直前の `2be0b3f` は 498 行・コード 324 行、`85d03e6` は 500 行・コード 324 行）。2/3 の `bddd086` は `glue.rs` を変えていない（`85d03e6` から `bddd086` までの空白無視の差分は 0 行）。`669ab6e` から TASK-352 の直前（`2be0b3f`）までの非コメントの差分は、TASK-319 の保存・復元の結線（`mount_host`・`history_to_save`・`mount_app` の戻り値）だけで、キー処理の glue ではない。

| 時点 | コミット | `key-invariants` | `tab-cycle` | `tab-complete` | `shift-tab` | `candidates` | 計 |
|---|---|---|---|---|---|---|---|
| 前（5.86b） | `669ab6e` | 20 | 11 | 4 | 6 | 18 | 59 |
| 1/3（5.88） | `85d03e6` | 20 | 11 | 4 | 6 | 18 | 59 |
| 2/3（5.91） | `bddd086` | 20 | 11 | 4 | 6 | 18 | 59 |
| 後（本節） | `bddd086` | 20 | 11 | 4 | 6 | 18 | 59 |

置き換えが無いため、「同じ件数で通る」ことは置き換えの成否ではなく、TASK-352 がキー処理の不変条件を壊していないことの確認である。

glue の規模（2026-10-17。`glue.rs` は `git show <commit>:crates/web/src/glue.rs` で読み、コード行は空行と `//` で始まる行を除いた数。`/* */` のブロックコメントは 0 件）。

| 時点 | `wc -l` | コード行 |
|---|---|---|
| `669ab6e`（前） | 480 | 319 |
| `2be0b3f`（TASK-352 の直前。TASK-319 1/3） | 498 | 324 |
| `85d03e6`（1/3） | 500 | 324 |
| `bddd086`（2/3、後） | 500 | 324 |

- 前から後への増分（コード +5 行、全体 +20 行）のうち、コード +5 行と全体 +18 行は TASK-319 の保存・復元の結線による。TASK-352 による増分は、1/3 のコメント 2 行（全体 +2）だけで、コード行では 0 である（`2be0b3f` から `bddd086` まで）。
- FEND-7・FEND-20 の「349 行・9 個」は、決定時点の PoC の追試の値（`wc -l` で 438 行）で、数え方と範囲が今と異なる。今の glue は `key_route.rs`（514 行・コード 369 行）・`glue_ops.rs`（94 行・コード 42 行）などへ分割されており、行数の減少を削減とは読めない（分割が理由とみられる。推測）。

`web-sys` の feature は、ルート `Cargo.toml` の `[workspace.dependencies]` に 21 個ある（5.88・5.91 と同じ。TASK-352 による増分は 0）。

| 用途 | 個数 | feature |
|---|---|---|
| キー入力・textarea の glue | 8 | `Element`・`Event`・`EventTarget`・`HtmlElement`・`HtmlTextAreaElement`・`InputEvent`・`KeyboardEvent`・`Window` |
| 保存先（localStorage・IndexedDB） | 10 | `Location`・`Storage`・`DomException`・`IdbFactory`・`IdbOpenDbRequest`・`IdbRequest`・`IdbDatabase`・`IdbTransaction`・`IdbTransactionMode`・`IdbObjectStore` |
| 書き出し | 3 | `Blob`・`BlobPropertyBag`・`Url` |

決定時点の 9 個に対し、キー入力・textarea の用途は 8 個で、保存先・書き出しの用途が後から加わった。

親 #347 の受け入れ条件との対応。

| 条件 | 確かめた場所 |
|---|---|
| TASK-351 の試験が同じ件数で通る | 上の件数の表と、下の実行の記録（59 件） |
| FEND-14（Enter と送信ボタンの一致）の 2 場面 | 5.91 と、本節の再実行（`submit-disabled.test.mjs` の 2 件） |
| glue のコード行数と `web-sys` の feature の数の記録 | 上の 2 つの表 |

```text
実行日: 2026-10-17
元にしたコミット: bddd086
コマンド: node --test tools/browser/{key-invariants,tab-cycle,tab-complete,shift-tab,candidates}.test.mjs（各ファイルを単独で 3 回）・node --test tools/browser/submit-disabled.test.mjs（単独で 1 回）・make test-browser（1 回）
```

| 実行 | 結果 |
|---|---|
| TASK-351 の 5 ファイル 単独 3 回 | 3 回とも 20・11・4・6・18 件の計 59 件がすべて通過 |
| `submit-disabled.test.mjs` 単独 1 回 | 2 件がすべて通過 |
| `make test-browser` 1 回 | 357 件がすべて通過（5.91 と同じ件数。ローカルの計測。CI には未組み込み） |

限界と残る作業。

- 左の行の Enter・Space の結線（`row_key_effect` を呼ぶ側）は TASK-358 1/2 と TASK-480 が受け持つ。結線は 5.113 で行った。
- 修飾キー付きの行の Enter・Space は A11Y-18 が定めておらず、spec 側での確定が要る。
- FEND-7・FEND-20 の「349 行・9 個」を、今の数え方との関係で注記するかは spec 側で扱う。
- ブラウザ試験は CI に未組み込みである。

### 5.93 文書の導出と増分・一括の一致（TASK-468 2/2）

1/2（5.87）が送った 2 件を入れた。文書のブロックの突き合わせ（`artifact_doc_rows`）と、増分と一括の一致（`PhaseWorlds`）である。コードは純関数だけで、ブラウザ試験は無い。

- 受入（文書）: `doc-before.md`・`doc-after.md` を `markdown::parse` でブロックにし、期待値は導出を通さずにパース結果から数える（重複が無いことを確かめた上で、前のブロックのうち後に含まれるものを未変更とする）。未変更 + 追加 = 後のブロック数、未変更 + 削除 = 前のブロック数、`~` が 0 件、前の状態は印 0 件
- 規則: 空・同一・全追加・全削除、変更は `-` が先の組、重複したブロックを含む列、ID の形、空の経路、読み取りの誤り、上限（`LIST_ITEMS_LIMIT` 超と中央の表 `MAX_ALIGN_CELLS` 超は `TooLarge`）、`Changed` 以外の無視
- 増分と一括: 6 種の sample・同じ経路を何度も変える列・`Changed` 以外を混ぜた列・文書を 1 ブロックずつ変える列で、全接頭辞 × 4 通りの分け方（1・2・3 件ずつ・全体 1 回）の増分の結果が一括と一致する。`Projection::extend` に同じ分け方で渡した結果の `consumed` と `Projection::from_events` との一致も確かめる
- 仮置き: 同点のときは `-` を `+` より先に出す（spec は順を決めていない）。基準点での「前」の取り直しは TASK-470・TASK-347

```text
実行日: 2026-10-17
元にしたコミット: d64e78a-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART2-{2,3,4,5,6,7}.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART2-2 | 後の世界状態に `Changed` を当てない | DETECTED | `artifact_rows_sample` の 7 件 |
| ART2-3 | 走査を後の世界状態の鍵だけにする | DETECTED | `artifact_rows_sample` の 7 件 |
| ART2-4 | 前後とも読めない経路を飛ばさない | DETECTED | `artifact_rows_contract` の「ビューの外の要素は行にならない」 |
| ART2-5 | 前後が違うブロックを一致として組にする | DETECTED | `artifact_doc_rows` の 4 件 |
| ART2-6 | 増分の開始位置を 1 つ飛ばす | DETECTED | `artifact_rows_incremental` の 2 件 |
| ART2-7 | 前にだけあるブロックの行を出さない | DETECTED | `artifact_doc_rows` の 5 件 |

### 5.94 変化の印の部品と外部スタイルシート（TASK-469 1/2）

印の部品（`artifact_mark`）と、印の見た目・インデントを置く外部スタイルシート（`crates/web/static/app.css`）を足した。keyed list の項目の作りの詰めと、印の文字を消す負の対照は 2/2 の担当である。

- 受入（印）: `artifact_mark_contract` が、印なし・追加・削除・変更 × 変種 (a)・(b) で、`a11y::check_no_undefined` の違反 0、`data-mark` の宿主が 1 項目に 1 つ、(b) は根・(a) は外枠が宿主、記号・文字が宿主の直接の子で `a11y` の値と一致、印なしは 3 手段とも空、`aria-hidden` が無いことを確かめる。既存の検査器（`artifact-view-check.mjs` の T2）は代役の部品がこの部品を使うようになったため、12 通り × 2 往復の 3 手段の対応を新しい部品で確かめる
- 受入（スタイル）: `<style` 要素と `style` 属性が 0 件であることを 3 層で確かめる。描画結果（`artifact_mark_contract`）、配信物の静的な検査（`check-delivery.sh` の `index.html`・`app.js`）、描画後の DOM の数え上げ（`collectSafety`）である。`app.css` の同一オリジンの取得と `text/css` は `smoke-hello` が確かめる。CSS が Rust の定数のクラス（`d0`〜`d6`・`mk-glyph`・`mk-text`）と `[data-mark]` を定義していることも `artifact_mark_contract` が確かめる
- 深さの範囲外（7 以上）は `d6` に丸める（fail-closed）
- WASM のバイト列と glue の JS の `<style`・`style=` は、2026-10-17 に配信する設定の release 一式で実測し、いずれも 0 件だった。ただし依存由来の文字列で 0 件でなくなりうるため、静的な検査に加えず、描画後の DOM の数え上げに任せる
- 範囲外: `wireframe_css()` の書き出しと `app.css` への連結は、変種 (a) で実際の部品を使うビュー（TASK-473 以降）が持つ。色のコントラストは TASK-490 の担当

```text
実行日: 2026-10-17
元にしたコミット: 727a981-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART3-{2,3}.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART3-2 | 変種 (a) で項目の根にも `data-mark` を付ける | DETECTED | `artifact_mark_contract` の 3 件 |
| ART3-3 | 深さの丸めを剰余に替える | DETECTED | `artifact_mark_contract` の「深さのクラスは d0 から d6」 |

### 5.95 前後の切り替えでの差分更新（TASK-469 2/2）

変化の印の部品を使う項目が、前後の切り替えで keyed list の項目として in-place に更新されることを、検査器の `deep` 検査と自動ブラウザの試験で確かめた（ART-1・ART-3）。

- 測ったもの: 契約が許す 12 通り（7 種 × 2 変種から文書 (a)・差分 (a) を除く）を、前 → 後 → 前 → 後 → 前の 5 状態で流し、各遷移で、両方にある key の行の要素の同一性（行の根）と、項目の内側の全子孫要素（印の宿主・記号・文字・内容の `span`）の同一性を数える。結果は 12 通りの全遷移で、行の保持が前の行数に等しく（未変更 100%・変更行の置換 0）、内側の保持も前の行数に等しかった（2026-10-17 計測）
- 検査器: `artifact-view-check.mjs` の `runViewCase` に任意の `deep`（既定 `false`。既定の経路の挙動・違反文言は変えていない）を足した。`deep: true` のとき、内側の置換を固定文言 `inner elements of a row were replaced` の違反にし、保持数を `stats.innerPreserved` に出す。負の対照用に `holdRows`・`compareHeldRows` を export した
- マウント時の DOM: 代役の部品の初期状態は file_tree・(b)・前で、最初の `select` は DOM を更新しない。このため file_tree (b) だけがマウント時の `innerHTML` の DOM から始まる（他の 11 通りは keyed の挿入から始まる）。この 1 通りでも内側の保持は全遷移で前の行数に等しく、項目の作りの手直しは要らなかった
- 項目の形の不変条件: 印の値（印なし・追加・削除・変更）によって、項目のタグ・属性名の集合・子の数と種別が変わらないことを `artifact_mark_contract` の「項目の形は印によって変わらない」が (a)・(b) の両方で固定する。形が変わると上流の子ノード列の位置対応が崩れ、項目の子が作り直される
- 負の対照: C1 は後の状態で印の文字を DOM 上で消し、`deep` の経路でも「3 手段が対応しない」違反が出ること（対は違反 0）。C2 は別のビューへ切り替えて戻し、行の保持が 0 件になること（同一性の検査が置換を検出できる。PoC-8 の 0/11 の写し）。T4 は空の画面で `deep` でも違反が出ること
- 範囲外: 製品の `view()` への 7 本の list の結線（TASK-470）、`wireframe_css()` の書き出し（TASK-473 以降）、色のコントラスト（TASK-490）、性能の測定（ART-4）。`view_fixture` の key の安定性の単体試験（変異 ART2-8）は、非公開 mod のため結合試験から触れず、行の同一性が P1 で 4 遷移すべて 100% であることで代えた

```text
実行日: 2026-10-17
元にしたコミット: 2c4d231-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART3-4.mutation・make test-browser
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART3-4 | 印なしのとき記号・文字の `span` を省く | DETECTED | `artifact_mark_contract` の「項目の形は印によって変わらない」ほか 3 件 |

`make test-browser` は 374 件がすべて通過した（`artifact-mark-toggle` の 17 件を含む。ローカルの計測。CI には未組み込み）。

### 5.87b 履歴が無い場合と未操作の場合の起動（TASK-319 1/3）

起動時に保存済みの履歴の有無で分岐し、保存済みの履歴が無い場合と未操作の場合は新しい状態で始めて世代を上げない（SAVE-3・SAVE-28。PoC-6 の F-9）。復元の一致を地点ごとに確かめる試験（native の 33 地点・自動ブラウザの 15 地点）は #342・#343 が持ち、本節は分岐と、それを観測できる最小の保存の結線を対象にする。

- 起動の分岐: core の `history_boot::boot_session`。保存済みが無い → 新しく始める（再読み込みを呼ばない）、開けて `ops` が 0 件 → 新しく始める、開けた → `Session::reload` に当たる処理を 1 回かけて復元する（世代 +1・`Reloaded` 1 件）、表示だけ・開かない → 初期状態で始め、呼び出し側は保存済みの値を上書きしない。判定は `history_load::load_history_with_envelope`（`load_history` と `load_session` の共通の下請け）に任せ、段 1〜8 の条件は複製しない
- 「未操作」の定義: 記録した `ops` が 0 件であること。下書きの入力も `ops` に記録する（SAVE-2）ため、文字を打っただけでも未操作ではない。`RecordedSession::envelope_for_save` は 0 件のとき `None` を返し、最初の変更まで保存しない
- web の結線: ホストは `RecordedSession` を通して操作を記録する（`apply_engine`・`submit_engine`・入力欄の下書きの変更）。起動の前に `persist::mount_with_history` が保存先を開いて保存済みの値を読み、`EngineHost::boot` の後にマウントする（`mount_app` は `Promise` を返し、`app.js` が `await` する）。離れる直前（`pagehide`）に `history_to_save` が値を返すときだけ保存先へ書く。保存の遅延のタイマーは置かない（TASK-318）。保存済みの値を読めない・開けないときは保存を止める（fail-closed）
- 本番の設定（IR を持たないホスト）は保存先を開かず、今までどおり即座にマウントする。IR の取得は後続のタスクである
- 試験用の口の保存先・保留とは別の保存先・保留を持つ。`pagehide` のリスナーは名前ごとに 1 回だけ登録する（`leave_flush::install_pagehide_flush_as`）ため、試験用の口の登録を塞がない

試験の一覧。

| 試験 | 内容 |
|---|---|
| `crates/core/tests/history_boot.rs` | 保存済みが無ければ新しく始まり世代・ログ・`Reloaded` が不変、任意の初期状態（性質テスト 256 通り）で恒等、`ops` が 0 件の包みは未操作、陽性の対照（操作 1 件は復元され世代 +1・`Reloaded` 1 件）、開けない値・別の教材は初期状態のまま |
| `crates/core/tests/recorded_session.rs` | 0 件なら包みを作らない、記録の規則が記録器と一致、作った包みが段 1〜8 で開けて適用し直しが写しと一致 |
| `crates/web/tests/boot_store.rs` | 保存先の偽物を通して、新規・未操作を 3 回続けても鍵が 0 件、陽性の対照（保存・復元・さらに復元で世代 +1 ずつ）、入力途中の下書きが復元される、開けない値を上書きしない |
| `tools/browser/reload-fresh.test.mjs` | 実際の画面（CSP あり・なし）で、保存済みが無い・未操作の再読み込みを 2 回続けても鍵が書かれず世代 1・`reloaded` の行 0 件で状態がマウント直後と等しい、陽性の対照（1 手を送信して再読み込み → 鍵が書かれ世代 +1・`reloaded` の行 1 件）、未操作の再読み込みの後でも操作すれば保存される |

変異の定義（`crates/*/tests/mutations/SAVE28-*.mutation`）。

| ID | 誤り | 検出する試験 |
|---|---|---|
| SAVE28-1 | 新しく始めるときに再読み込みをかける | `history_boot` の新規・0 件の包み |
| SAVE28-2 | `ops` が 0 件の包みを未操作として扱う規則を外す | `history_boot` の 0 件の包み |
| SAVE28-3 | `ops` が 0 件でも保存用の包みを作る | `recorded_session` |
| SAVE28-4 | 新しく始めるときに初期状態を捨てて既定の状態にする | `history_boot` の新規・恒等 |
| SAVE28-5 | 開けなかった履歴があっても保存を止めない | `boot_store` の上書きしない試験 |

既存の変異の定義は、コードの移動に合わせて `find` を直した（SAVE16-4・SAVE19-13・TERM2-1。`load_session` の本体と送信の経路が変わったため）。SAVE16-2 は `initial.clone()` が `boot` と重ならないよう `boot` の側を直した。`FEND15-2`・`FEND15-3` は本変更の前から `find` が一致しない（`engine_parts.rs` の `keyed_view` の呼び出しの形が変わっている）ため、本変更の範囲外として記録した。

```text
実行日: 2026-10-17
元にしたコミット: 0b607a7-dirty
コマンド: scripts/check-mutations.sh（SAVE28-1〜5。変更したファイルを対象にする既存の 35 件）・make test・make lint・make test-browser
```

| 実行 | 結果 |
|---|---|
| SAVE28-1〜SAVE28-5 | 5 件とも DETECTED |
| 変更したファイルを対象にする既存の変異 35 件（SAVE1・SAVE4・SAVE16・SAVE19・SAVE24・TERM・FEND・EVT8・G） | 35 件とも DETECTED |
| `make test`（`history_boot` 6 件・`recorded_session` 5 件・`boot_store` 6 件を含む） | 全件が通過 |
| `make test-browser` | 327 件がすべて通過（`reload-fresh` の 8 件を含む） |
| `make check-delivery`・`make test-build-web` | 通過（配信物に試験用の口が無い） |

残る限界と未実施。

- 地点ごとの復元の一致（native 33 地点・自動ブラウザ 15 地点・完走した最終の世界状態）は #342（5.89）・#343（5.90）
- 停止・移動・復帰の直後の実際の `page.reload()`（5.71 の `reload-after-control`）は TASK-321 の残り
- 開けない履歴の告知と「捨てて始める」ボタン（SAVE-21・SHELL-17）、起動番号による古いタブの保存の禁止（SAVE-12）、`visibilitychange` での書き出しと IndexedDB の書き込みの確定（SAVE-9 の検討中の論点）は範囲外

### 5.96 ビューごとの keyed list とビューの選択（TASK-470 1/2）

製品の左の成果物の部品（`engine_parts::ArtifactPart`）を、ビューの種類ごとの keyed list 7 本と、選択のボタンの list 1 本の形にした。
選ばれていない list は項目 0 件で `hidden` を持つ。選択は `select-view` の action（`view_select`）で、今の Step が宣言した種類だけを受ける。
行の中身はいまのまま、選ばれたビューの list に入れた。ブラウザの試験用の教材（複数のビューを宣言するもの）と、部品を項目の内側に置く規則・101 回の属性の不変は 2/2 の担当で、本節は native の試験で確かめた。

- 受入: 選ばれていないビューの list が項目 0 件で `hidden` を持つ。親の属性が `list_attrs(kind, selected)` と一致する（開始直後・選択の後・Step の移動の後）
- 試験（`crates/web/tests/view_select.rs`）: 開始時の描画（7 本・`hidden` は 6 本・宣言の先頭だけが見える・ボタンは宣言の順で一覧外の種類と未定義の ID は出ない）、選択の dirty（旧と新の list とボタンの list）、同じ種類と宣言に無い種類の無視、session なし、綴りの完全一致の復号、Step の移動で先頭に戻ること（A→B→A で戻っても以前の選択が戻らないことを含む）、行が上限（4,096 件）を超えると選ばれたビューの list に通知の行と `data-list-error` が出ること（ちょうど 4,096 件では出ない）、a11y の検査
- 既存の試験の更新: `list_parent_attrs`（期待を 7 本とボタンの list に置き換え）、`host_contract`（field 数 9 から 16）、`artifact_view_binding_contract`（標準の登録簿が 7 本を持つ）、`no_hidden_triggers`（`engine_parts.rs` の `PAYLOAD_ATTR` 2 件）、ブラウザの `list-parent-attrs`・`reload-points`（選択子）
- 仮置き: 宣言が無い Step の既定は、`view_tabs` が `all` ならファイルツリー（ホストは `all` だけを渡す。`pack.left_pane` の読み取りは TASK-53 2/2）。`view_tabs: all` で 7 種を選べるようにするのは TASK-481
- 入れていないもの: 親の `aria-label`・ボタンの `aria-pressed`（`a11y` の閉じた一覧に無い。契約の版 3 と spec の更新が先に要る）。エンジンの操作の dirty の絞り込みは 2/2

```text
実行日: 2026-10-17
元にしたコミット: 727a981-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART12-1.mutation crates/web/tests/mutations/ART5-2.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART12-1 | 宣言に無い種類の選択も受け付ける | DETECTED | `view_select` の「宣言に無い種類の選択は無視され_dirty_を積まない」 |
| ART5-2 | 選ばれていないビューの list も選択済みとして描く | DETECTED | `view_select` の 5 件（開始直後の描画を含む） |

PR #1534 のレビューの指摘で、2 件を直した。どちらも直す前に新しい試験が失敗し、直した後に通ることを確かめた。

- 上限を超えた行の `Err` を、行 0 件の keyed list に黙って倒していた。元の行を `keyed_view_or_notice` に渡し、通知の行と `data-list-error` を出す形に戻した（FEND-22・D-46）
- 別の Step では選択を読まないだけで、捨てていなかった。A で選び、B へ移って A へ戻ると以前の選択が戻った。`step_engine` が、今の Step が選んだときの Step と違えば選択を捨てる（ART-12）。Step を変える口（操作・送信・再読み込みの模擬）はすべて `step_engine` を通り、差し替え（`install_recorded`）と構築は選択を持たずに始める

```text
実行日: 2026-10-17
元にしたコミット: 2088de2-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART12-1.mutation crates/web/tests/mutations/ART5-2.mutation crates/web/tests/mutations/ART12-2.mutation crates/web/tests/mutations/FEND22-1.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART12-1 | 宣言に無い種類の選択も受け付ける | DETECTED | `view_select` の「宣言に無い種類の選択は無視され_dirty_を積まない」 |
| ART5-2 | 選ばれていないビューの list も選択済みとして描く | DETECTED | `view_select` の 8 件（開始直後の描画を含む） |
| ART12-2 | Step が変わってもビューの選択を捨てない | DETECTED | `view_select` の「先頭以外を選んで別の_step_へ移り_戻ると先頭に戻る」 |
| FEND22-1 | 上限を超えた行を通知なしの空の一覧に倒す | DETECTED | `view_select` の「行が上限を超えると選ばれたビューの_list_に通知と_data_list_error_が出る」 |

### 5.97 切り替え時の更新範囲と同一性（TASK-470 2/2）

エンジンの操作（`step_engine`）がログの伸びたときに積む keyed list を絞り、切り替え・更新の後も根の要素が同一でフォーカスが保たれることを、複数のビューを宣言する試験用の教材で確かめた。

- dirty の規則: ビューの list 7 本は `dirty_list_fields` で絞る（ビューが変われば旧と新の 2 本、同じなら今のビューの 1 本）。選択のボタンの list は、Step かビューが変わったときだけ積む（宣言の集合は Step で決まるため）。それ以外の list は従来どおりすべて積む。行の中身が変わったかは再導出せず、ログが伸びたことで代用する（積みすぎる側に倒れ、束縛点として解決できるので全再描画にならない）
- 試験用の教材: `load-fixture:views`（試験の設定だけ）。st-1 は file_tree・card_list・check_list、st-2 は card_list・file_tree、st-3 は card_list を宣言する
- native の試験（`crates/web/tests/view_select.rs`）: 同じ Step・同じビューで 1 本だけ、種類が変わると旧と新の 2 本とボタン、Step が変わっても種類が同じなら 1 本とボタン、絞り込みの後の描画の一致、`load-fixture:views` の復号と Step の移動ごとの dirty
- ブラウザの試験（`tools/browser/view-switch.test.mjs`）: V1 ボタンの順と表示、V2 実クリックで 3 種を 2 往復、V3 入力欄にフォーカスを置いたままの切り替え・更新・移動、V4 再描画 101 回で選択の属性の差 0 回、N1 部品を項目の外に置く対照（内側では同一、外側では根の子が置き換わる）、P1 代役の部品での前後の切り替え
- 既存の試験の更新: `list-parent-attrs` は、選ばれていないビューの list を更新で再同期しない側に直した（後付けの属性が残ることを確かめる）

```text
実行日: 2026-10-17
元にしたコミット: 1484623-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART5-3.mutation crates/web/tests/mutations/ART12-3.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART5-3 | ビューの list を絞らず 7 本すべてを積む | DETECTED | `view_select` の 4 件（同じ Step・同じビューの 1 本だけの試験を含む） |
| ART12-3 | Step が変わっても種類が同じなら選択のボタンを積まない | DETECTED | `view_select` の「step_が変わっても種類が同じなら_その_list_とボタンだけが_dirty_になる」 |

- `make test-browser`: 381 件が通過（2026-10-17）。上の 7 件と `list-parent-attrs` の更新を含む
- 手元の確認: `select_changed` を常に偽にしても V3 は通った。ブラウザの本試験は、ボタンの積み漏れを検出できない（native の試験と ART12-3 が固定する）。理由は未調査
- 限界: 製品の画面には前後の切り替え（ART-2 の口）が無いため、P1 は代役の部品で代えた。製品の根は N1 の間も同一であることを同じ測り方で確かめた。「部品を項目の外に置く」状態を製品の画面に作る口は、登録簿の契約に例外を作るため設けていない。親の `aria-label`・ボタンの `aria-pressed` は契約の版上げが先に要る

### 5.98 タスクツリーの変種 (b)（TASK-474 1/2）

タスクツリーの (b) の本体（`task_tree`）を作り、代役の部品（`view_fixture`）の `(task_tree, b)` だけがこの本体を通るようにした。項目の数と印の数が前後で期待値と一致すること、状態を記号と文字の両方で示すことを確かめた。製品の画面への結線は、世界状態からタスクの属性を読む口と前後の切り替えの口が決まる後続の作業に残す。

- 項目の形: 根の `li` が木の属性と印を持ち、子は印の記号・印の文字・ラベル・詳細・状態のバッジの 5 つに固定する。バッジの子は記号と文字の 2 つ。状態・印・前後で形を変えないため、keyed list は同じ key の項目を in-place に更新できる
- 状態の語彙: `open`・`progress`・`done` は spec に定めが無く、試験データ由来の仮置きである。それ以外の値は `Unknown`（`?`・`不明`）に倒し、生の値は描画に出さない
- 構成: 木の位置は描画 1 回につき `shown_tree_attrs` を 1 回呼ぶ O(n) の計算にした。50 件の再描画 16ms の測定は TASK-485 の担当で、本節では測っていない
- `fandhe-frontend-headless-ui` の直接依存は足さなかった。`tree_view::item` は `aria-selected` を無条件に出し `data-value` に行の ID を出す。`marker::icon` は `aria-hidden` を出す。どちらも a11y 版 2 の閉じた一覧に無い。依存の採否は TASK-473 と合わせて決める
- native の試験（`crates/web/tests/task_tree_contract.rs`）: 項目数（前 5・後 6）と印の数（変更 3・追加 1・削除 1・印なし 1）、木の属性と `aria-expanded`（前 1・後 2）、親の `role=tree` の条件、状態のバッジ、状態の文字と JSON の対応、未知の状態、項目の形の不変、a11y 契約、エスケープ、印の宿主
- ブラウザの試験（`tools/browser/task-tree.test.mjs`）: P1 検査器の deep で違反 0・保持が前の行数に等しい、P2 前後の各状態の数え上げ、C1 状態の文字を空にする負の対照

```text
実行日: 2026-10-17
元にしたコミット: a4bb59d-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/A11Y22-1.mutation crates/web/tests/mutations/ART9-3.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| A11Y22-1 | 完了の状態の文字を空にする | DETECTED | `task_tree_contract` の 2 件 |
| ART9-3 | 親をツリーでない種類として作る | DETECTED | `task_tree_contract` の「木の属性は全項目にあり_親は_role_tree」ほか |

- ブラウザ: `task-tree`・`artifact-mark-toggle`・`artifact-view-check`・`artifact-view-controls`・`view-switch` の 62 件が通過（2026-10-17。`make test-browser` の全体ではなく、この 5 本を対象にした）

### 5.99 ファイルツリーの変種 (b)（TASK-473 1/2）

左の成果物エリアのファイルツリーを、変種 (b) で描く本体（`crates/web/src/file_tree_view.rs`）を作り、代役の部品（`view_fixture`）のファイルツリー (b) をこれに回した。変種 (a) は 5.100 で足した。

- 受入: 項目数（前 10・後 11）と変化の印の数（後は追加 1・削除 1・変更 2・印なし 7）が期待値と一致し、前後の切り替えで行も項目の内側も置き換わらない
- 判断 1: `headless-ui` の `tree_view` からは `item_text`（ラベル）だけを採る。`item`・`branch` は `aria-selected` を必ず出し（a11y が違反にし、呼び出し側から消せない）、`*_indicator` の `aria-hidden` と `branch_content` の `role=group` は閉じた一覧に無く、根が `div` で `ul` の子として不正であり、`branch` は開いた行すべてに `aria-expanded` を付け、`data-value` に教材由来の識別子が入る。採る部品を広げるには、spec の更新と a11y の契約の版上げが先に要る
- 判断 2: `fandhe-frontend-headless-ui` を web の直接の依存にした（0.69.2。`Cargo.lock` の解決済みの版のまま。`frontend_features` の件数は 5）。wasm-full は再エクスポートしないため
- 判断 3: 製品の `view()` へは結線しない。製品には前後の切り替えの口が無く、世界状態から前後の行を作る規則（前の取り直し）が未決で、親の属性を替えると `list_parent_attrs` の期待が変わるため
- 判断 4: 項目の形は前後で変えない。補足の `span` は空でも出し、ラベルはディレクトリでもファイルでも `item_text` にそろえる（種類は `data-kind`）。`aria-expanded`・`aria-setsize` の変化は属性だけで、子の列は変わらない
- native の試験（`crates/web/tests/file_tree_view_contract.rs`）: 項目数と key の並び、印の数、a11y の閉じた一覧、木の属性（手で数えた位置）、禁じた属性が 0 件、前後の形の不変、補足が空でも出ること、エスケープ、0 件・非選択で `role=tree` が付かないこと、200 行の構成（時間は測らない。測定は TASK-485）
- ブラウザの試験（`tools/browser/file-tree-view.test.mjs`）: FT1 検査器で前後 2 往復（行・内側の保持が前の行数に等しい）、FT2 前後の木の属性と項目数と印の数、FT3 外部リクエスト・ページのエラー・ダイアログが 0 件、FT4 (a) の対照

```text
実行日: 2026-10-17
元にしたコミット: a4bb59d-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART1-1.mutation crates/web/tests/mutations/ART1-2.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART1-1 | 項目に木の属性の組を付けない | DETECTED | `file_tree_view_contract` の「木の属性は手で数えた位置と一致する」 |
| ART1-2 | 補足が空のとき補足の `span` を省く | DETECTED | `file_tree_view_contract` の「補足が空でも_span_は出る」 |

- ブラウザの試験: `file-tree-view`・`artifact-view-check`・`artifact-view-controls`・`artifact-mark-toggle`・`view-switch`・`tree-attrs`・`list-parent-attrs` の 71 件が通過（2026-10-17。`make test-browser` の全件ではなく、関係する 7 ファイルを試験の設定の web で流した）
- 範囲外: 製品の結線、指せる行の属性（TASK-480）、変種 (a)（#377）、親の `aria-label`、`item`・`branch` の採用、200 件の再描画時間（TASK-485）、補足の見た目（TASK-490）

### 5.100 タスクツリーの変種 (a)（TASK-474 2/2）

タスクツリーの (a) を `task_tree` に足し、代役の部品（`view_fixture`）の `(task_tree, a)` もこの本体を通るようにした。(a) はチェックボックス（完了をチェック済みで示す）・状態のバッジ・深さのクラスで描き、木の意味（`role=tree`・`treeitem`）を持たない。製品の画面への結線は 5.98 と同じく後続の作業に残す。

- 項目の形: `li` の中に印の宿主の `div`（深さのクラス）があり、子は印の記号・印の文字・チェック・ラベル・詳細・状態のバッジの 6 つに固定する。チェックは子が文字ノード 1 つの `span` で、完了だけ `✓`、他は空文字。未知の状態もチェックなし（完了と主張しない）
- `fandhe-frontend-wireframe-ui` の `checkbox` は使わなかった。チェックの有無でグリフの子が増減して形が変わること、グリフの `aria-hidden` が a11y 版 2 の閉じた一覧に無いこと、依存が未導入で `wireframe_css()` の連結も未実装であることによる。採否は TASK-473 の headless-ui と合わせて決める
- native の試験（`crates/web/tests/task_tree_contract.rs` の `a_` で始まる 6 件）: 項目数と印の数、木の意味が無いこと、チェックの数（前 1・後 3）と未知の状態、状態のバッジと文字の数、項目の形の不変、印の宿主と深さのクラス、a11y 契約・エスケープ・`app.css` の `.tk-check`
- ブラウザの試験（`tools/browser/task-tree.test.mjs`）: P1a 検査器の deep で違反 0、P2a 前後の各状態の数え上げ（チェックの数は試料の JSON から Node 側で数えた完了の数）、C2 `data-checked` を反転する負の対照

```text
実行日: 2026-10-17
元にしたコミット: a6b6e58-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/A11Y22-1.mutation crates/web/tests/mutations/ART9-3.mutation crates/web/tests/mutations/A11Y22-2.mutation crates/web/tests/mutations/ART9-4.mutation crates/web/tests/mutations/ART9-5.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| A11Y22-1 | 完了の状態の文字を空にする（再実行。既存の `find` が一意のまま） | DETECTED | `task_tree_contract` の 3 件 |
| ART9-3 | 親をツリーでない種類として作る（再実行） | DETECTED | `task_tree_contract` の 2 件 |
| A11Y22-2 | 未着手の状態の文字を空にする | DETECTED | `task_tree_contract` の 3 件 |
| ART9-4 | (a) のチェックを常に外す | DETECTED | `task_tree_contract` の「a_完了の項目だけチェック済みで_未知の状態はチェックなし」 |
| ART9-5 | (a) のチェックの子をチェック済みのときだけ入れ子の span にする | DETECTED | `task_tree_contract` の「a_項目の形は状態と印と前後で変わらない」 |

- ブラウザ: `make test-browser` の全体（387 件）が通過（2026-10-17。CI には未組み込み）

### 5.101 ファイルツリーの変種 (a)（TASK-473 2/2）

ファイルツリーの変種 (a) を、`wireframe-ui` の `rich_text` で描く本体（`crates/web/src/file_tree_view.rs` の `file_tree_item_a` など）を作り、代役の部品（`view_fixture`）のファイルツリー (a) をこれに回した。(b) の関数は変えていない（ART1-1・ART1-2 の `find:` 行がそのまま当たる）。

- 受入: (a) の項目数（前 10・後 11）と変化の印の数（後は追加 1・削除 1・変更 2・印なし 7）が期待値と一致し、前後の切り替えで行も項目の内側も置き換わらない。外部へのリクエスト・ページのエラー・ダイアログは 0 件
- 判断 1: 外枠は `marked_item(.., Variant::A, ..)` の `li` と内側の `div`（印の宿主）で、中身は `rich_text`（ラベル・先頭の記号・末尾の補足）である。木の属性（`role=treeitem`・`aria-level` など）は付けない
- 判断 2: 末尾の補足の `span` は `detail` が空でも必ず出す。省くと前後で子の列が変わり、keyed list の in-place 更新が崩れる。先頭の `span` と太字は行の種類だけで決め、前後や印では変えない
- 判断 3: 先頭に `wireframe_ui::icon::*` は使わない。`svg` に `aria-hidden` を必ず出し、a11y の契約（版 2）の違反になる。記号は自作の `span` の文字（ディレクトリは `▾`、ファイルは空）にした。採るには契約の版 3 と spec の更新が先に要る
- 判断 4: `fandhe-frontend-wireframe-ui` を web の直接の依存にした（0.52.0。依存は core 0.4.3 だけで採用中の版と同じ。`frontend_features` の件数は 6）
- 判断 5: `wireframe_css()` は呼ばず、`app.css` へも連結しない。固定のライト配色の CSS の配信は TASK-490 の担当で、受入基準は構造だけである
- 判断 6: 製品の `view()` へは結線しない（5.99 の判断 3 と同じ）
- native の試験（`crates/web/tests/file_tree_view_a_contract.rs`）: 項目数と key の並び、印の数、`data-mark` が `li` に無く内側の `div` にだけあること、a11y の閉じた一覧と `role`・`aria-`・`svg` が 0 件、`rich_text` と太字・記号の対応、前後の形の不変、補足が空でも先頭・末尾の `span` が出ること、エスケープ、0 件・非選択で `role` が付かないこと、200 行の構成（時間は測らない。測定は TASK-485）
- ブラウザの試験（`tools/browser/file-tree-view.test.mjs`）: FT5 検査器で (a) の前後 2 往復、FT6 (a) の前後の項目数・`rich_text`・印の宿主と数・禁じた要素が 0 件、FT7 外部リクエスト・ページのエラー・ダイアログが 0 件

```text
実行日: 2026-10-17
元にしたコミット: 01a8771-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART1-1.mutation crates/web/tests/mutations/ART1-2.mutation crates/web/tests/mutations/ART1-3.mutation crates/web/tests/mutations/ART1-4.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART1-1 | (b) の項目に木の属性の組を付けない | DETECTED | `file_tree_view_contract` の「木の属性は手で数えた位置と一致する」 |
| ART1-2 | (b) の補足が空のとき補足の `span` を省く | DETECTED | `file_tree_view_contract` の「補足が空でも_span_は出る」 |
| ART1-3 | (a) の補足が空のとき末尾を渡さない | DETECTED | `file_tree_view_a_contract` の「補足が空でも末尾の_span_と先頭の_span_は出る」 |
| ART1-4 | (a) の項目を変種 B の根で作る | DETECTED | `file_tree_view_a_contract` の「data_mark_は_li_には無く内側の_div_にだけある」 |

- ブラウザの試験: `file-tree-view`・`artifact-view-check`・`view-switch`・`tree-attrs`・`artifact-mark-toggle` の 59 件が通過（2026-10-17。`make test-browser` の全件ではなく、関係するファイルを試験の設定の web で流した。CI には未組み込み）
- 範囲外: `wireframe_ui::icon::*` の採用（a11y の契約の版 3 が先）、`wireframe_css()` の配信と配色（TASK-490）、製品の結線、指せる行の属性（TASK-480）、200 件の再描画時間（TASK-485）

### 5.102 差分の変種 (b)（TASK-475 1/2）

左の成果物エリアの差分を、変種 (b) で描く本体（`crates/web/src/diff_view.rs`）を作り、代役の部品（`view_fixture`）の `(diff, b)` をこれに回した。(a) は `wireframe-ui` の `text` が空白を畳みインデントを失う（PoC-8 の G-4）ため、(b) は自作の `pre`・行番号・記号の列で描く。変種 (a) の観察は 5.103（TASK-475 2/2）に分けた。製品の画面への結線は 5.98 と同じく後続の作業に残す。

- 受入: 項目数（前 7・後 9）、コードのインデントが保たれること、行番号の列（旧・新）と記号の列があること、行の印（層 1）と差分の記号（層 2）が別の要素に出ること
- 項目の形: 根の `li` の子は、層 1 の記号・文字、旧の行番号、新の行番号、層 2 の差分の記号、`pre` の 6 つに固定する。hunk の行も同じ 6 つで、行番号と記号は空文字になる。`tags` は出さない
- 2 層の印: 層 1 は前後の切り替えで付く `+`・`-`・`~`（`mark-glyph`）、層 2 は `state` から読んだ差分そのものの `+`・`-`（`diff-sign`）。未知の `state` は `?` にし、追加・削除と主張しない
- 行番号: `旧:新` を `:` で分け、各側が空または ASCII の数字だけ（最大 9 桁）のときだけ出す。外れる側は空文字にし、生の値を出さない
- `class` と `role`: `marked_item` は (b) で深さがあると根に `class` を足すため、`depth` は `None` で渡して `class` の重複を避けた。a11y 版 2 は表・行の `role` と `aria-hidden` を持たないため、列は `span` と CSS だけで作る
- native の試験（`crates/web/tests/diff_view_contract.rs`）: 項目数と key、層 1 の印の数、層 1 と層 2 が別の要素であること、記号と `state` の対応、`pre` のタグの数とインデント、行番号、形の崩れた行番号と未知の `state` の fail-closed、前後の形の不変と子の数、a11y と `role`・`aria-` が 0 件、エスケープ、非選択で空、500 行の構成（時間は測らない。測定は TASK-485）、`app.css` の 4 クラス
- ブラウザの試験（`tools/browser/diff-view.test.mjs`）: DF1 検査器で前後 2 往復、DF2 前後の列・`pre` の空白の扱い・文字・2 層の印、DF3 外部リクエスト・ページのエラー・ダイアログが 0 件、DF4 `white-space: normal` にするとインデントの差が消える対照

```text
実行日: 2026-10-17
元にしたコミット: 2b83a7f-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART8-1.mutation crates/web/tests/mutations/ART8-2.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART8-1 | コードを `pre` でない要素で描く | DETECTED | `diff_view_contract` の「コードは_pre_で_インデントの空白が文字に残る」「同じ_key_の項目の形は前後で変わらず_子は_6_つ」 |
| ART8-2 | 差分の `-` の列を消す | DETECTED | `diff_view_contract` の「層1と層2は別の要素に出る」「差分の記号は_state_と対応し_集計は手で数えた値と一致する」 |

- ブラウザの試験: `diff-view`・`artifact-view-check`・`artifact-view-controls`・`artifact-mark-toggle`・`view-switch` の 63 件が通過（2026-10-17。`make test-browser` の全件ではなく、関係する 5 ファイルを試験の設定の web で流した）
- 範囲外: 製品の結線、指せる行の属性（TASK-480）、色とコントラスト（TASK-490）、500 行の再描画の測定（TASK-485）、約 1,000 行を超える場合の見せ方（TASK-484）、親の `aria-label`、変種 (a)（TASK-475 2/2）

### 5.103 差分の変種 (a)（対照。TASK-475 2/2）

差分の変種 (a)（`wireframe-ui` の `text` でコードを描く）を、対照としてだけ固定した。製品の契約（差分は (b) だけ。`ViewKind::allows`・`VariantPolicy`）は変えない。(a) へは代役の部品（`view_fixture`）の別の action `fixture_contrast`（payload は完全一致の `diff:a` だけ）からだけ入り、`fixture_select` の `diff:a` は拒否されたままである。

- 受入: (a) が 14 通りの検査器の枠（前後 2 往復の項目数・key・印の 3 手段・同一性）を通ること、(a) でインデントが失われる観察を固定すること、同じページの (b) では保たれる正の対照、契約の拒否が不変であること
- 項目の形: (b) とコードの 1 列だけが違う。`data-testid="diff-code"` の `span` の中に `span.fw-wire-text` を 1 つ置く。印の宿主は内側の `div`（`li` には付かない）
- 観察の中身: 実画面（`wireframe_css()` は未配信）ではコードの先頭の 4 つの空白が 1 つに畳まれ、直前の列の後ろに空白 1 つ分（約 8px）が残るだけになる。(b) の 4 つ分（約 31px）より大きく小さい。上流の `TEXT_CSS` は `white-space: nowrap` で、これも行頭の空白を畳む
- native の試験: `view_fixture` の単体 3 件（対照の口の復号・(a) の描画・対照の後も `fixture_select` が拒否）と `crates/web/tests/diff_view_a_contract.rs`（項目数と key、印の宿主、`pre` が無く `fw-wire-text` が項目数と同じ、2 層の印、行番号の fail-closed、形の不変、a11y、エスケープ、`TEXT_CSS` の記録）
- ブラウザの試験（`tools/browser/diff-view.test.mjs`）: DF5 対照の口の (a) が検査器で違反 0、DF6 (a) の前後でインデントが畳まれ `pre` が無く `role` が 0・(b) では保たれる、DF7 対照の後も `fixture_select` の `diff:a` が `rejected` で状態が変わらない

```text
実行日: 2026-10-17
元にしたコミット: a54e6e5-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART8-1.mutation crates/web/tests/mutations/ART8-2.mutation crates/web/tests/mutations/ART8-3.mutation crates/web/tests/mutations/ART8-4.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART8-1 | コードを `pre` でない要素で描く（(b)） | DETECTED | `diff_view_contract`（5.102 と同じ 2 件） |
| ART8-2 | 差分の `-` の列を消す（(b)） | DETECTED | `diff_view_contract`（5.102 と同じ 2 件） |
| ART8-3 | (a) のコードを `wire_text` を通さず描く | DETECTED | `diff_view_a_contract` の「a_コードは_wire_text_で_pre_は無い」 |
| ART8-4 | (a) の項目を (b) の印の付け先で作る | DETECTED | `diff_view_a_contract` の「a_data_mark_は_li_には無く内側の_div_にだけある」 |

- ブラウザの試験: `diff-view`・`artifact-view-check`・`artifact-view-controls`・`artifact-mark-toggle`・`view-switch` の 66 件が通過（2026-10-17。関係する 5 ファイルを試験の設定の web で流した）。`make check-delivery` は配信物に試験用の口が無いことを確認した
- 範囲外: (a) を製品から外すかの決定（TASK-465）、`wireframe_css()` の配信と配色（TASK-490）、文書 (a) の対照（TASK-476）、製品の結線、指せる行の属性（TASK-480）、測定（TASK-485）、ブラウザの試験の CI への組み込み（必須チェックと ruleset の変更が要る）

### 5.104 文書のビューの変種 (b)（TASK-476 1/2）

文書の行の中身を本文の木（`contract::doc::Block`）に結び、変種 (b) の項目として描いた（`document_view`）。木の描画は TASK-149・TASK-150 の `doc_view` を再利用し、ブロック 1 件の入口 `render_block_node` だけを足した。製品の契約（文書は (b) だけ。`ViewKind::allows`・`VariantPolicy`）は変えない。

- 受入: 見出し・表・一覧・引用・コードの構造が保たれること、実行時に Markdown を解釈するコードが 0 件であること
- 実行時に Markdown を解釈しない構造: 木は pack の `markdown::parse`（ビルド時）が作り、web は `Block` を受けて描くだけである。代役の部品は `doc-before.md`・`doc-after.md` を木にした JSON の写しを持ち（単体試験が `markdown::parse` の結果との一致を確かめる）、前後は `align_blocks` で `-` と `+` の行にする。pack は web の dev-dependency（試験専用）だけで、通常の依存に無い
- 項目の形: 根の子は記号・文字（変化の印）と `data-testid="doc-block"` の包みの 3 つで、包みの中にブロックの要素が 1 つ入る。`depth` は渡さず、`role`・`aria-*`・指せる行の属性は付けない。`data-block` の値は閉じた集合の静的な文字列である
- サンプルに無い構造: `doc-before.md`・`doc-after.md` には囲みのコードと区切り線が無い（spec の写しでバイト一致のため書き換えない）。ブラウザの試験は見出し・段落・番号つき一覧・番号なし一覧・表・引用・行内のコードを、囲みのコード（空白の保持・言語名の許可）と区切り線は native の契約試験の手作りの行で確かめる
- native の試験: `crates/web/tests/document_view_contract.rs`（項目数と key、印の件数、構造の要素の数、項目の形、`data-block`、囲みのコード、区切り線、エスケープ、リンク・画像の許可、深さの上限、a11y、80 ブロックの構成）、`crates/web/tests/no_runtime_markdown.rs`（manifest の通常の依存・`Cargo.lock` の推移的な依存・ソースのコード行）、`view_fixture` の単体 2 件
- ブラウザの試験（`tools/browser/doc-view.test.mjs`）: DV1 検査器で違反 0・項目数 `[10,13,10,13,10]`、DV2 構造の要素の数と表のセルの文字が Markdown と一致、DV3 外部通信・エラー・ダイアログ 0 件、DV4 構造を文字だけにすると検査が失敗する対照
- 試作の教材パック（`bbs-tutorial`）の読み取りは、この repo に取り込まれていない（`testdata/README.md`）ため行っていない

```text
実行日: 2026-10-17
元にしたコミット: 56d1ba0-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART7-1.mutation crates/web/tests/mutations/ART7-2.mutation crates/web/tests/mutations/ART7-3.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART7-1 | 文書に変種 (a) を許す | DETECTED | `artifact_view_binding_contract` の 2 件 |
| ART7-2 | 包みの中身を木の描画でなく種類名の文字だけにする | DETECTED | `document_view_contract` の構造の数・80 ブロックの構成ほか 6 件 |
| ART7-3 | 印の宿主を項目自身から内側の `div` に変える | DETECTED | `document_view_contract` の「全項目の根の子は記号_文字_包みの3つで_包みはブロックを1つ持つ」 |

- ブラウザの試験: `doc-view`・`artifact-view-check`・`artifact-view-controls`・`artifact-mark-toggle`・`view-switch`・`diff-view` の 70 件が通過（2026-10-17。試験の設定の web で流した）
- 範囲外: `-` と `+` の組を主張する専用の試験と文書 (a) の対照は 5.105 で扱った（TASK-476 2/2）。製品の結線と世界状態から `Vec<Block>` を読む規則、指せる行の属性（TASK-480）、色（TASK-490）、測定（TASK-485）、ブラウザの試験の CI への組み込み。検査器の `DANGER_SELECTOR` は `img` をすべて違反と数えるが `doc_view` は相対の `src` の画像を正当に `img` にする。サンプルに画像が無いため当たらないが、製品の結線の時に食い違う

### 5.105 文書のビューの `-` と `+` の組と変種 (a)（対照。TASK-476 2/2）

(b) で `-` と `+` の組が出ることを試験で主張し、変種 (a)（`wireframe-ui` の `text`・`paragraph` で平文の行に落とす方式）を対照として固定した。製品の契約（文書は (b) だけ。`ViewKind::allows`・`VariantPolicy`）は変えない。(a) は代役の部品の対照の口 `fixture_contrast`（payload `document:a`）からだけ描ける。

- 受入: 前後の切り替えで変更のあったブロックが `-` と `+` の組で出ること。(a) では表・引用・一覧が構造を失うこと
- 項目の形（(a)）: 根の `li` の子は内側の `div` 1 つで、印はその `div` が持つ（差分の (a) と同じ）。`div` の子は記号・文字・`doc-block` の包みの順で、包みの中は見出しと表の見出し行が `text`（大・太字）、それ以外が `paragraph` の行になる。リンクの宛先・画像の URL は捨てる。入れ子は `MAX_RENDER_DEPTH` で打ち切り、省略の文言の行にする
- 観察の中身: サンプルの前は見出しの行 5・段落の行 10、後は見出しの行 6・段落の行 20（Markdown から手で数えた）。構造の要素（`h1`〜`h6`・`ol`・`ul`・`table`・`blockquote`・`pre`・`code`・`a`・`img`・`hr`）は 0 件で、表は `項目 | 内容` のように縦棒で連結された文字になる。`wireframe_css()` は配信していないため、(a) の印の宿主が内側の `div` であることから `static/app.css` の取り消し線は当たらない（見た目は TASK-490）
- native の試験: `crates/web/tests/document_view_a_contract.rs`（項目数と key、項目の形、構造の喪失、行の数、文字の保持、印の件数、同じ key の形の不変、エスケープ、囲みのコードと区切り線、深さの上限、a11y、80 ブロックの構成）、`document_view_contract.rs` の組の隣接（3 組・種類 list/table/list・記号 `-`/`+`）、`view_fixture` の単体（対照の口の受理と拒否、(a) の後の構造の要素 0、対照の後も `fixture_select` の `document:a` を拒否）
- ブラウザの試験（`tools/browser/doc-view.test.mjs`）: DV5 `-`/`+` の組（(b)。2 往復と、追加の印を消すと検査が失敗する対照）、DV6 (a) が検査器の枠で違反 0、DV7 (a) で構造の要素が全て 0・行の数と文字が残り、(b) へ戻すと構造が戻る、DV8 対照の後も `document:a` の選択は拒否される

```text
実行日: 2026-10-17
元にしたコミット: 3abaecf-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART7-1.mutation crates/web/tests/mutations/ART7-2.mutation crates/web/tests/mutations/ART7-3.mutation crates/web/tests/mutations/ART7-4.mutation crates/web/tests/mutations/ART7-5.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART7-1 | 文書に変種 (a) を許す | DETECTED | `artifact_view_binding_contract` の 2 件 |
| ART7-2 | 包みの中身を木の描画でなく種類名の文字だけにする | DETECTED | `document_view_contract` の構造の数・80 ブロックの構成ほか 6 件 |
| ART7-3 | 印の宿主を項目自身から内側の `div` に変える | DETECTED | `document_view_contract` の「全項目の根の子は記号_文字_包みの3つで_包みはブロックを1つ持つ」 |
| ART7-4 | (a) の包みの中身を平文の行でなく木の描画にする | DETECTED | `document_view_a_contract` の構造の要素・行の数ほか 5 件 |
| ART7-5 | (a) の項目の印の付け先を項目自身にする | DETECTED | `document_view_a_contract` の「項目の形は li の子が内側の div 1 つで印の宿主はその div」 |

- ブラウザの試験: `doc-view`・`diff-view`・`artifact-view-check`・`artifact-view-controls`・`artifact-mark-toggle`・`view-switch` の 74 件が通過（2026-10-17。試験の設定の web で流した）。`make test-browser` の全件は未検証
- 範囲外: 80 ブロックのブラウザでの描画の測定（TASK-485）、(a) を製品から外すか残すかの決定（TASK-465）、`wireframe_css()` の配信と配色・(a) の取り消し線（TASK-490）、製品の結線と指せる行の属性（TASK-480）、本文を出す部品（TASK-489）、検査器の `DANGER_SELECTOR` が `img` をすべて違反と数える件、試作の教材パック（`bbs-tutorial`）の読み取り、`make test-browser` の CI への組み込み（必須チェックと ruleset の変更が要るため、要対応事項として報告のみ）

### 5.106 カードの変種 (a)（TASK-477 1/2）

左の成果物エリアのカードを、変種 (a) で描く本体（`crates/web/src/card_view.rs`）を作り、代役の部品（`view_fixture`）の `(card_list, a)` をこれに回した。(a) は `wireframe-ui` の `card_basic`（ラベル・補足・状態の `tag`）と、ラベルを連結した `tag` 1 個で描く。変種 (b)（`article`・`ul`・`marker`）と 20 件の規模は TASK-477 2/2（5.107）に分けた。

- 受入: (a) の項目数（前 3・後 4）と変化の印の数（後は追加 1・削除 1・変更 2・印なし 0）が期待値と一致し、前後の切り替えで行も項目の内側も置き換わらない。部品の型・識別子・綴りに Issue・PR などの語が 0 件（`artifact_view_generic_names` が `card_view.rs` を走査する）
- 判断 1: ラベル（`tags`）は区切り「・」で連結した 1 個の `tag` にし、空でも必ず出す。1 件ごとに `tag` にすると件数の違う前後で子の数が変わり、keyed list の in-place 更新の前提（5.95）と `artifact-mark-toggle` の `deep` が崩れる
- 判断 2: `card_basic` の `secondary`・`trailing` は常に `Some` で渡す。`None` だと要素自体が消え、前後で子の列が変わる
- 判断 3: `leading` は `None` に固定する。`wireframe_ui::icon::*` は `svg` に `aria-hidden` を必ず出し、a11y の契約（版 2）の違反になる（5.101 の判断 3 と同じ）
- 判断 4: 状態は `tag` の文字で示し、色に頼らない。語彙（下書き・オープン・マージ済み・クローズ・不明）は spec に定めが無く、試験データと PoC-8 由来の仮置きである。完全一致で読み、語彙の外は「不明」にする（fail-closed）。強調（`Primary`）はオープンのときだけで、class が変わるだけで形は変えない
- 判断 5（ART-17）: 状態やラベルが変わった行は、行単位の「変更」の印だけで示す。欄ごとの印や差の一覧は持たない
- 判断 6: `wireframe_css()` は呼ばず、`app.css` へも連結しない（TASK-490）
- 判断 7: 製品の `view()` へは結線しない（5.99 の判断 3 と同じ）
- native の試験（`crates/web/tests/card_view_a_contract.rs`）: 項目数と key の並び、印の数、`data-mark` が `li` に無く内側の `div` にだけあること、部品の数、状態の文字と強調、ラベルの連結、前後の形の不変（比較 3 件）、補足・ラベルが空でも部品が出ること、語彙の外の状態、欄ごとの印が無いこと、a11y の閉じた一覧と `role`・`aria-`・`svg` が 0 件、エスケープ、0 件・非選択で `role` が付かないこと
- ブラウザの試験（`tools/browser/card-view.test.mjs`）: CV1 検査器で (a) の前後 2 往復、CV2 前後の項目数・部品の数・印の宿主と数・禁じた要素が 0 件、CV3 外部リクエスト・ページのエラー・ダイアログが 0 件

```text
実行日: 2026-10-17
元にしたコミット: 1b5274b-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART17-1.mutation crates/web/tests/mutations/ART17-2.mutation crates/web/tests/mutations/ART17-3.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART17-1 | 補足が空のとき `secondary` を渡さない | DETECTED | `card_view_a_contract` の「補足とラベルが空でも形の部品は出る」 |
| ART17-2 | 語彙の外の状態をオープンへ寄せる | DETECTED | `card_view_a_contract` の「語彙の外の状態は不明になり_既知の状態へ寄せない」 |
| ART17-3 | (a) の項目を変種 B の根で作る | DETECTED | `card_view_a_contract` の「data_mark_は_li_には無く内側の_div_にだけある」 |

- ブラウザの試験: `card-view`・`artifact-view-check`・`artifact-mark-toggle`・`artifact-view-controls`・`view-switch` の 62 件が通過（2026-10-17。試験の設定の web で流した。`make test-browser` の全件と `list-parent-attrs`（配信用の web が別に要る）は未検証。CI には未組み込み）
- 範囲外: 変種 (b)・20 件の規模・2 変種の検査器の通し（TASK-477 2/2）、`wireframe_css()` の配信と配色（TASK-490）、製品の結線、指せる行の属性（TASK-480）、再描画の時間（TASK-485）、値の注入の 24 通り（TASK-482）、`make test-browser` の CI への組み込み（必須チェックと ruleset の変更が要るため、要対応事項として報告のみ）

### 5.107 カードの変種 (b) と 20 件の規模（TASK-477 2/2）

カードの変種 (b)（自作の `article` と `ul`、`headless-ui` の `marker`）を `card_view.rs` に足し、代役の部品（`view_fixture`）の `(card_list, b)` をこれに回した。あわせて 20 件の規模のデータで、検査器から (a)・(b) の 2 変種を流した。

- 受入: (b) の項目数（前 3・後 4）と印の数（後は追加 1・削除 1・変更 2・印なし 0）が期待値と一致し、印は項目の `li` 自身にある。前後の切り替えで行も項目の内側も置き換わらない。20 件では (a)・(b) とも項目 20、保持 20。部品の型・識別子・綴りに Issue・PR などの語が 0 件（`artifact_view_generic_names` が `card_view.rs` と新しい testid の綴りを走査する）
- 判断 1: 項目の根は `li` で、`article` はその内側に置く。親の list は `ul` で、`ul` の直下の `article` は HTML として不正であり、検査器は (b) の印の宿主が項目自身であることを求める。見出し要素は使わない（20 件で見出しが 20 個増え、ページの見出しの構成を変えるため。契約に定めも無い）
- 判断 2（spec からの逸脱）: ラベルの `ul` は `li` を 1 個に固定し、文字は「・」で連結する。ラベル 1 件ごとの `li` にすると、`pr1`（前 1 件・後 2 件）で子孫の数が前後で変わり、検査器の `deep` が違反になる。`ul` の自作は満たすが、項目 1 件ごとの `li` ではない。spec 側の確認が要る
- 判断 3: `marker` は `root`（`div`）と `content`（`span`）だけを使う。`icon` は `aria-hidden` を固定で出し、a11y の契約（版 2）の閉じた一覧に無い。色調は `CardState::tone` の閉じた `match`（下書き = warning、オープン・不明 = neutral、マージ済み = info、クローズ = danger）で、spec に定めの無い仮置きである。状態は文字でも示し、色に頼らない
- 判断 4: 状態の語彙・完全一致・fail-closed、印の扱い（ART-17）、`wireframe_css()` を呼ばないこと、製品の `view()` へ結線しないことは (a) と同じ（5.106）
- 判断 5: 20 件のデータは `testdata/`（spec の写しで SHA-256 固定）に置けないため、`crates/web/tests/data/artifact_view_scale/cards-20.json` に置く（出所は同じ場所の `README.md`）。`view_fixture` に action `fixture_data`（payload は `cards:sample`・`cards:20` だけ）を足し、検査器の `runViewCase` に任意の `rows` を足した。`view_fixture` は `test-hooks` の下だけで、配信物には入らない
- 注意: 20 件のデータは追加・削除とラベルの件数の変化を含まない。判断 2 の形の不変は、sample の `pr1` で確かめる
- native の試験（`crates/web/tests/card_view_b_contract.rs`）: 項目数と key の並び、印の数、`data-mark` が `li` 自身だけにあること、`article`・`marker`・`ul`・`li` の数、状態の文字と色調（5 状態）、ラベルの連結、補足が空でも `p` が出ること、前後の形の不変（`pr1` を含む比較 3 件）、欄ごとの印が無いこと、a11y の閉じた一覧と `role`・`aria-`・`svg`・`icon` が 0 件、エスケープ、0 件・非選択で `role` が付かないこと、20 件の構成。`view_fixture` の単体試験: (b) が本体を通ること、`fixture_data` の受理と拒否、20 件への切り替えと戻し、非選択中の切り替えで dirty を積まないこと
- ブラウザの試験（`tools/browser/card-view.test.mjs`）: CV4 (b) の前後 2 往復、CV5 前後の部品の数と印の宿主と数、CV6 外部リクエスト・ページのエラー・ダイアログが 0 件、CV7 a -> b -> a の選び直し、CV8 20 件の写しが `generate.mjs` の出力と一致、CV9 20 件を (a)・(b) で流す、CV10 20 件に切り替えずに 20 件の `rows` で流すと項目数の違反が出る（`rows` の口の対照）。`artifact-view-controls` の N3b は、(b) の本体では最初の `span` が `marker` になるため、選択子を `[data-testid="card-label"]` に直した（試験の意図は変えない）

```text
実行日: 2026-10-17
元にしたコミット: bf3b033-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART17-1.mutation 〜 ART17-6.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART17-4 | (b) の項目を変種 A の根で作る | DETECTED | `card_view_b_contract` の「data_mark_は_li_自身にあり内側の要素には無い」 |
| ART17-5 | ラベルを 1 件ごとの `li` にする | DETECTED | `card_view_b_contract` の「同じ_key_の項目の形は前後で変わらない_ラベルの件数が変わる行を含む」ほか 2 件 |
| ART17-6 | オープンと語彙の外の色調を情報へ変える | DETECTED | `card_view_b_contract` の「状態の文字と色調は固定の語彙に対応する」 |

- 変異を置いていないもの: `fixture_data` の差し替え。変異の仕組みは統合試験の対象だけを指定でき、`view_fixture` の単体試験（lib）は指せないため、単体試験だけで確かめた
- ブラウザの試験: `card-view`（CV1〜CV10）・`artifact-view-check`・`artifact-mark-toggle`・`artifact-view-controls`・`view-switch` の 70 件が通過（2026-10-17。試験の設定の web で流した。`make test-browser` の全件は未検証。CI には未組み込み）
- 範囲外: 再描画の時間（TASK-485）、`wireframe_css()` の配信と配色（TASK-490）、製品の結線、指せる行の属性（TASK-480）、値の注入の 24 通り（TASK-482）、`make test-browser` の CI への組み込み（必須チェックと ruleset の変更が要るため、要対応事項として報告のみ）

### 5.108 チェックの一覧の変種 (a)・(b) と 20 件の規模（TASK-478）

チェックの一覧を (a)（`wireframe-ui` の `rich_text`・`tag`）と (b)（`li` と `headless-ui` の `marker`）の両方で描く本体（`crates/web/src/check_view.rs`）を作り、代役の部品（`view_fixture`）の `(check_list, a)`・`(check_list, b)` をこれに回した。あわせて 20 件の規模のデータで、検査器から 2 変種を流した。

- 受入: 項目数（前 5・後 6）と印の数（後は追加 1・削除 1・変更 3・印なし 1）が期待値と一致し、状態が記号つきの文字（`✓ 成功`・`✗ 失敗` など）で出る。前後の切り替えで行も項目の内側も置き換わらない。20 件では (a)・(b) とも項目 20、保持 20、後の印は変更 20（補足が全行で変わるため）。部品の型・識別子・綴りに特定の教材の語が 0 件（`artifact_view_generic_names` が `check_view.rs` と記号・文字・testid の綴りを走査する）
- 判断 1（spec からの逸脱）: (b) に `role=list`・`role=listitem` を付けない。a11y の契約（版 2）の `ROLE_VALUES` に無く、付けると `undefined-role` になる。契約の版上げは spec が先に要るため、本作業では行わず、`li`（`ul` の暗黙の listitem）と `marker` で描く。契約の版 3 に足すかは spec 側の確認事項である
- 判断 2: 状態は `CheckState` の閉じた語彙（`success`・`failure`・`running`・`pending`・`skipped`）を完全一致で読み、一覧の外は `Unknown`（`? 不明`）にする（fail-closed）。記号・文字・色調は spec に定めの無い PoC-8 の試作由来の仮置きである。注意: 実行中と待機中は記号が同じ `…` で文字だけが違う。省略の `–`（U+2013）は印の `-`（U+002D）と見た目が近い（コードポイントは違い、試験で印の記号と一致しないことを確かめた）。試作の `data-state`（教材由来の文字列を属性へ入れる）は写さない
- 判断 3: (a) の先頭は `wireframe_ui::icon::*` を使わず、自作の `span.icon` に記号の文字を入れる（`icon::*` は `aria-hidden` を必ず出し a11y の契約の違反になる）。先頭の `span` はどの状態でも出し、状態で子の数を変えない。強調は失敗のときだけ
- 判断 4: (b) は `li` 自身が印の宿主で、子はラベルの `span`・`marker` のバッジ・補足の `span` の 3 つに固定する。`marker::icon` は呼ばない（`aria-hidden` を固定で出すため）
- 判断 5: 製品の `view()` へは結線しない（カードと同じ。TASK-470・TASK-347 の後続）
- 判断 6: 20 件のデータは `crates/web/tests/data/artifact_view_scale/checks-20.json` に置く（出所は同じ場所の `README.md`）。`view_fixture` の `fixture_data` を、差し替えるビューの種類を持つ形（`Data(ViewKind, DataSet)`）に一般化し、payload `checks:sample`・`checks:20` を足した。`cards:*` の振る舞いは変えない
- native の試験（`crates/web/tests/check_view_a_contract.rs`・`check_view_b_contract.rs`）: 項目数と key の並び、印の数、印の宿主（(a) は内側の `div`、(b) は `li` 自身）、部品の数、6 状態の記号・文字・色調、語彙の外、形の不変（前後で比べる 5 件）、補足が空でも要素が出ること、a11y の閉じた一覧と `role`・`aria-`・`svg`・`data-part="icon"` が 0 件、エスケープ、0 件・非選択で `role` が付かないこと、20 件の構成。`view_fixture` の単体試験: (a)・(b) が本体を通ること、`checks:*` の受理と拒否、20 件への切り替えと戻し、非選択中の切り替えで dirty を積まないこと
- ブラウザの試験（`tools/browser/check-view.test.mjs`）: CK1〜CK3 は (a)、CK4〜CK6 は (b)（前後 2 往復・部品の数と印の宿主と数・外部リクエスト 0 件）、CK7 a -> b -> a の選び直し、CK8 20 件の写しが `generate.mjs` の出力と一致、CK9 20 件を (a)・(b) で流す、CK10 20 件に切り替えずに 20 件の `rows` で流すと項目数の違反が出る

```text
実行日: 2026-10-17
元にしたコミット: baae6ee-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART1-5.mutation 〜 ART1-10.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART1-5 | (a) の先頭の記号を出さない | DETECTED | `check_view_a_contract` の「どの状態でも先頭の記号と補足の要素が出る」ほか 3 件 |
| ART1-6 | (a) の項目を変種 B の根で作る | DETECTED | `check_view_a_contract` の「data_mark_は_li_には無く内側の_div_にだけある」 |
| ART1-7 | 語彙の外の状態を成功へ寄せる | DETECTED | `check_view_a_contract` の「語彙の外の状態は不明になり_既知の状態へ寄せない」 |
| ART1-8 | (b) の項目を変種 A の根で作る | DETECTED | `check_view_b_contract` の「data_mark_は_li_自身にだけあり_内側には無い」 |
| ART1-9 | 失敗のバッジの文字から記号を落とす | DETECTED | `check_view_b_contract` の「状態は記号つきの文字と色調で出る」ほか 1 件 |
| ART1-10 | 失敗の色調を中立にする | DETECTED | `check_view_b_contract` の「状態は記号つきの文字と色調で出る」 |

- 変異を置いていないもの: `fixture_data` の差し替え（5.107 と同じ理由で、単体試験だけで確かめた）
- ブラウザの試験: `check-view`（CK1〜CK10）・`artifact-view-check`・`artifact-mark-toggle`・`artifact-view-controls`・`view-switch`・`card-view` の 81 件が通過（2026-10-17。試験の設定の web で流した。`make test-browser` の全件は未検証。CI には未組み込み）
- 範囲外: 契約の版 3（`role=list`・`listitem`）と状態の語彙・記号・色調の spec 側の確認、製品の結線、再描画の時間（TASK-485）、`wireframe_css()` の配信と配色（TASK-490）、指せる行の属性（TASK-480）、値の注入の 24 通り（TASK-482）、`make test-browser` の CI への組み込み（必須チェックと ruleset の変更が要るため、要対応事項として報告のみ）

### 5.109 アプリのデモのワイヤーの変種 (a)（TASK-479 1/2）

アプリのデモのワイヤーを (a)（`wireframe-ui` の `text`・`tag`・`rich_text` と `input`・`button`・`card_basic`）で描く本体（`crates/web/src/app_demo_view.rs`）を作り、代役の部品（`view_fixture`）の `(app_demo, a)` をこれに回した。(b) は TASK-479 2/2 で、今は汎用の描き方のままである。

- 受入: 項目数（前 8・後 9）と印の数（後は追加 1・削除 0・変更 3・印なし 5）が期待値と一致し、部品が行の種類ごとに出る。前後の切り替えで行も項目の内側も置き換わらない。部品の型・識別子・綴りに特定の教材の語が 0 件（`artifact_view_generic_names` が `app_demo_view.rs` と状態の文字を走査する）
- 判断 1（試作からの逸脱）: `Active`・`Disabled` は常に `false` で渡す。試作は行データから決めるが、`wireframe-ui` 0.52.0 はこの 2 つを真のときだけ `data-active`・`data-disabled` として出すため、同じ key の項目の属性名の集合が前後で変わり、形の不変（5.95）に反する。状態は class（`Primary`）と文字（`（選択中）`・`（無効）`・`（不明）`）で示す
- 判断 2: 状態は `WireState` の閉じた語彙（空・`active`・`disabled`）を完全一致で読み、一覧の外は `Unknown`（fail-closed）。語彙は spec に定めの無い試験データと PoC-8 由来の仮置きである。`data-state` は作らない
- 判断 3: 種類の外の行（`RowKind` の他の 8 種）は、ラベルを文字として出すだけにする。`wireframe_ui::icon::*` は `aria-hidden` を出すため使わない。`role`・`aria-` は 0 件で、TASK-366 の契約の `tablist` の数は変えない
- 判断 4: タブの横並びは `app.css` の `[data-view="app_demo"] > li[data-kind="tab"] { display: inline-block; }` の 1 規則で補う。領域ごとの配置（ART-23）と配色（TASK-490）は扱わない。製品の `view()` へは結線しない
- native の試験（`crates/web/tests/app_demo_view_a_contract.rs`）: 項目数と key、印の数と宿主、部品の数、形の不変（比較 8 件）、`data-active`・`data-disabled` が 0 件、状態の語彙と文字、欄の文字、種類の外、a11y の閉じた一覧、エスケープ、`role` が付かないこと、`app.css` の規則。`view_fixture` の単体試験: (a) が本体を通ること
- ブラウザの試験（`tools/browser/app-demo-view.test.mjs`）: AD1 前後 2 往復の検査器、AD2 部品の数と印の宿主と数、AD3 タブの横並び、AD4 外部リクエスト 0 件

```text
実行日: 2026-10-17
元にしたコミット: a5665ea-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART1-11.mutation 〜 ART1-13.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART1-11 | 欄の `Active` を行データで切り替える | DETECTED | `app_demo_view_a_contract` の「同じ_key_の項目の形は前後で変わらない」ほか 1 件 |
| ART1-12 | ボタンの `Disabled` を状態から決める | DETECTED | 同上 |
| ART1-13 | (a) の項目を変種 B の根で作る | DETECTED | `app_demo_view_a_contract` の「data_mark_は_li_には無く内側の_div_にだけある」 |

- ブラウザの試験: `app-demo-view`・`artifact-view-check`・`artifact-mark-toggle`・`artifact-view-controls`・`view-switch`・`card-view`・`check-view` の 85 件が通過（2026-10-17。試験の設定の web で流した。`make test-browser` の全件は未検証。CI には未組み込み）
- 範囲外: (b)（`headless-ui` の `field`・`role=tab`・`tablist`）は TASK-479 2/2、指せる行の属性は TASK-480、模擬ログインの中身は TASK-492、配色と `wireframe_css()` は TASK-490、領域ごとの配置は ART-23、製品の結線、`make test-browser` の CI への組み込み（必須チェックと ruleset の変更が要るため、要対応事項として報告のみ）

### 5.110 アプリのデモのワイヤーの変種 (b) と tablist の属性（TASK-479 2/2）

アプリのデモのワイヤーを (b)（`headless-ui` の `field` による読み取り専用の欄と、自作の `span`・`button`）で描く本体を `crates/web/src/app_demo_view.rs` に足し、代役の部品（`view_fixture`）の `(app_demo, b)` をこれに回した。これでアプリのデモの (a)・(b) とも本体を通る。

- 受入: 項目数（前 8・後 9）と印の数（後は追加 1・削除 0・変更 3・印なし 5）が期待値と一致する。`tablist` の属性は TASK-366 の契約（版 2）の適合試験と食い違わない（ページの `tablist` は `pane-tabs` の 1 個、`tab` は `pane-tab` の 2 個のまま）。9 行の規模は試験データ `appdemo.json` の後の状態がちょうど 9 行であるため、新しい規模のデータは作らない
- 判断 1（spec からの逸脱。spec 側の確認事項）: (b) は `role`・`aria-*` を 1 つも出さない。spec は自作の `role=tab`・`role=listitem` と親の `role=tablist` を挙げるが、契約（版 2）では、親の `tablist` は `check_complete` の数（ちょうど 1）を崩し、`role=tab` は `pane-tab` の testid・予約 id・`aria-controls` を要し、`aria-selected` は tab 以外で違反、`listitem` は閉じた一覧に無い。契約を変えるには spec を先に直して `A11Y_CONTRACT_VERSION` を上げる必要があるため、ここでは変えない。意味は `ul`・`li`・`button` の暗黙の役割で保つ。試作も PoC-12 の A5 でアプリのデモの親を `tablist` から外した。契約の版 3 に `tab`・`listitem` を足すかは spec 側の確認事項である。「`tablist` の属性を契約に合わせる」は、ページの `tablist` を 1 個のままにして適合試験が通ることと読んだ
- 判断 2: 選択中・無効は文字（`（選択中）`・`（無効）`・`（不明）`）で示し、`aria-selected`・`disabled` は出さない。`disabled` を状態から出すと、試験データの `a-add`（前 `disabled`・後 空）で同じ key の項目の属性名の集合が前後で変わる。見出しは `h1`〜`h6` を使わず `p` にする（ページの見出しの構成を変えないため）
- 判断 3: 欄の `id` は `app-demo-field-{行の並びの添字}` で、行の識別子（信頼できない値）から作らない。予約 id（`pane-tab-*`・`pane-*`）と衝突しない。`value` は空でも必ず出す
- 判断 4（信頼境界の例外）: 教材由来の文字列が文字のノードの外に出るのは欄の `input` の `value`（補足）だけである。属性名は固定、`render` の既定のエスケープを通る、URL・イベントハンドラの属性ではない、の 3 点から許容し、エスケープの試験で確かめる
- 判断 5: 操作対象の大きさ（A11Y-3・ART-10）。ブラウザで測ると `button` は 94x24、`input` は 185x21 で、`input` の高さが 24 未満だった（2026-10-17）。`app.css` に `[data-view="app_demo"] button, [data-view="app_demo"] input { min-height: 24px; min-width: 24px; }` の 1 規則を足した（色は足さない。TASK-490）
- native の試験（`crates/web/tests/app_demo_view_b_contract.rs`）: 項目数と key、印の数と宿主（`li` 自身）、部品の数、形の不変（比較 8 件）、`disabled`・`aria-`・`role=`・`svg`・見出しが 0 件、状態の文字、欄の `id`・`for`・`value`、種類の外、a11y の閉じた一覧、`tablist` 1 個・`tab` 2 個の数え上げと適合試験、負の対照（親の `tablist`・`span` の `role=tab` で違反）、エスケープ（`value` を含む）、`app.css` の規則。`view_fixture` の単体試験: (b) が本体を通ること
- ブラウザの試験（`tools/browser/app-demo-view.test.mjs`）: AD5 (b) の 2 往復の検査器、AD6 部品の数と印の宿主と数、AD7 ページの `tablist` と一覧の中の `role`、AD8 同じ `input` の `value` の更新と読み取り専用、AD9 タブの横並びと操作対象の大きさ、AD10 選び直しと外部リクエスト 0 件

```text
実行日: 2026-10-17
元にしたコミット: a22dc63-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART1-14.mutation 〜 ART1-16.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART1-14 | (b) の項目を変種 A の根で作る | DETECTED | `app_demo_view_b_contract` の「data_mark_は_li_自身にだけあり内側には無い」 |
| ART1-15 | ボタンの `disabled` を状態から出す | DETECTED | 「同じ_key_の項目の形は前後で変わらない」ほか 1 件 |
| ART1-16 | タブの `span` に `role=tab` を足す | DETECTED | 「a11y_の閉じた一覧に前後とも違反しない」ほか 2 件 |

- ブラウザの試験: `app-demo-view`・`artifact-view-check`・`artifact-view-controls`・`artifact-mark-toggle`・`view-switch`・`card-view`・`check-view` の 91 件が通過（2026-10-17。試験の設定の web で流した。`make test-browser` の全件は未検証。CI には未組み込み）
- 範囲外: 指せる行の属性は TASK-480、模擬ログインの中身は TASK-492、配色と `wireframe_css()` は TASK-490、領域ごとの配置は ART-23、axe の対応づけは TASK-483、再描画の時間は TASK-485、値の注入の 24 通りは TASK-482、製品の結線、`make test-browser` の CI への組み込み（必須チェックと ruleset の変更が要るため、要対応事項として報告のみ）。(b) の `button`・`input` はフォーカスできるが押しても何も起きない（行を指す操作は TASK-480 との関係を後続で確かめる）

### 5.111 指せる行の共通の作りと属性（TASK-480 1/2）

7 種のビューの行が、同じ作り（`artifact_mark::pointed_item`）で指せる行の 4 属性（`data-action="ui"`・`data-payload`・`data-row`・`tabindex="0"`）を付けられるようにした。宛先は呼び出し側が `PointOf`（行の識別子から `PointTarget` を引く口）で注入し、ビューは指せるかどうかを判断しない。契約（`artifact_view.rs`。版 1）は変えず、属性の綴りと順は `PointAttrs::pairs()` だけが持つ。

- 受入: 7 種のビューの全変種（差分・文書の (a) は対照の口）で、宛先つきの行がすべて 4 属性を根の `li` に組で持つ。宛先の無い行は 4 属性を 1 つも持たない。内側の要素（(a) の印の宿主の `div` など）には付かない。既存の試験・検査がすべて通る
- 判断 1: 指せるかどうかは `RowId` だけの関数で、`Phase` には依らない。同じ key の項目の属性名の集合が前後で変わらず、keyed list の in-place 更新の前提（ART3-4 の延長）が保たれる。後の状態で前の中身を残して出す削除の行も、宛先があれば指せる
- 判断 2: `PointTarget::new` が拒否する行（payload が 4,096 バイトを超える・空の view）は `point_all` が `None` に落とし、前後とも表示だけで出す。行は落とさず panic しない（fail-closed）
- 判断 3: 製品で指せる行を決める規則（Step の `ui_action` の `targets` との照合か、導出したすべての行を指せるか）は後続の結線（TASK-470・TASK-347 の後続）で決める。本 Issue は注入の口と、代役の部品の規則だけを置く。代役の部品は、ビューの識別子を合成値（`fixture-<ビューの種類>`）にし、データの先頭の行だけを表示だけ、残りを指せる行にする（各ビューに負の対照を 1 行ずつ持つため）
- 判断 4（spec 側の確認事項）: 文書の行の識別子は「文書の経路 + `#` + 位置」で、これを `Input::Ui.path` にどう対応させるかは、`artifact_doc_rows` の doc では TASK-476 の担当とされているが、TASK-476 の成果物では決まっていない。本 Issue は規則を固定せず、呼び出し側の注入に任せる
- 判断 5: 操作対象の大きさ（A11Y-3）。`data-action` を持つ行は `a11y::TARGET_SIZE_SELECTOR` の集合に入るため、`app.css` に `[data-action="ui"] { min-height: 24px; min-width: 24px; }` の 1 規則を足した（色は足さない。TASK-490）
- 判断 6: 木の (b) は `role=treeitem` の行ごとに `tabindex="0"` が付く。roving tabindex ではないが、a11y の契約（版 2）は tab 以外の静的な `tabindex` を検査しない。木のキー操作は TASK-494・A11Y-18 の範囲として記録する。アプリのデモの (b) では、`button`・`input`（読み取り専用）が指せる `li` の中でもフォーカスを受け、1 行に 2 つのタブの止まりになる。本 Issue では変えない
- native の試験（`crates/web/tests/point_rows_contract.rs`）: 7 種 × 許される変種 × 前後で、宛先つきの行の 4 属性が組・順（action, payload, row, tabindex）・根にだけあること、payload が宛先に戻ること、宛先の無い行は 0 件、宛先が全く無ければ 0 件、同じ key の項目の指せるかどうかが前後で変わらないこと、一覧全体の a11y の違反が 0（木の (b) の `role=treeitem` を含む）、危険な文字を含む view・row のエスケープと復号の往復、payload の超過・空の view が表示だけになること。`artifact_mark_contract` に `pointed_item(target=None)` が `marked_item` と同じ出力になる試験を足した。`view_fixture` の単体試験: 先頭の行以外が 4 属性を持つこと、`decode_action("ui", ..)` が `None`（クリックの受け付けは TASK-480 2/2）
- ブラウザの試験（`tools/browser/point-rows.test.mjs`）: 14 通り（7 種 × 変種、対照を含む）× 前後で、4 属性の組・先頭の行は 0 件・内側に `data-action` が無い・`focus()` と Tab でフォーカスを受ける・寸法が 24x24 以上・外部リクエストなどの安全性の計数が 0。クリックの効果は確かめない（TASK-480 2/2）

```text
実行日: 2026-10-17
元にしたコミット: 26618b7-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART11-2.mutation 〜 ART11-4.mutation（ART11-1 も再実行）
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART11-1 | 属性の組から `tabindex` を落とす（再実行） | DETECTED | `artifact_view_binding_contract` の「属性の組は_4_つで順が固定_値はエスケープされる」 |
| ART11-2 | `pointed_item` が 4 属性を足さない | DETECTED | `point_rows_contract` の「宛先のある行は_4_属性を組で持ち_宛先の無い行は_1_つも持たない」ほか 2 件 |
| ART11-3 | カード (b) が宛先を捨てる | DETECTED | 同上ほか 1 件 |
| ART11-4 | アプリのデモ (b) が宛先を捨てる（PoC の B4 の退行） | DETECTED | 同上 |

- ブラウザの試験: `point-rows`・`card-view`・`app-demo-view`・`check-view`・`task-tree`・`file-tree-view`・`diff-view`・`doc-view`・`view-switch`・`artifact-view-check`・`artifact-view-controls`・`artifact-mark-toggle`・`list-parent-attrs` の 136 件が通過（2026-10-17。試験の設定の web で流した。`make test-browser` の全件は未検証。CI には未組み込み）
- 範囲外: クリックで指す操作の受け付け・名札の行 `screen/top`・クリックのブラウザ試験は TASK-480 2/2、Enter・Space は TASK-358・TASK-375、製品の `view()` への結線と製品で指せる行を決める規則は TASK-470・TASK-347 の後続、木の roving tabindex とアプリのデモの二重のタブの止まりは A11Y-18・A11Y-20・TASK-494、フォーカスの見た目と配色は TASK-490、`make test-browser` の CI への組み込み（必須チェックと ruleset の変更が要るため、要対応事項として報告のみ）

### 5.112 クリックで指す操作の受け付けとアプリのデモの名札の行（TASK-480 2/2）

指せる行のクリック（`data-action="ui"`・`data-payload`）を、host の `EngineHost::decode_action` が `PointTarget::decode` を通して `Op::Input(Input::Ui { view, path })` に変えて受け付けるようにした。代役の部品（`view_fixture`）も同じ復号器を通し、今の画面で指せる行だけを記録する。アプリのデモの名札の行（世界状態の経路 `screen/top`）は、名札のデータ（`appdemo-nameplate.json`）で指せる行になる。契約（`artifact_view.rs`。版 1）は変えない。

- 受入: 7 種のビューの宛先つきの行をクリックすると、指す操作として受け付けられる。宛先の無い行（表示だけの行）をクリックしても何も受け付けない。アプリのデモの名札の行 `screen/top` が指せる行になる
- 判断 1: host は判定しない。`ui` の payload を復号して `Op::Input` に渡すだけで、宛先が今の Step に合うかの照合と判定は core（`select_judge`）に任せる（`tests/no_progress.rs` の方針のまま）。復号できない payload は `HostAction` にならず、`HelloView` にも流さない（fail-closed）
- 判断 2: 配信する設定に入るのは host の `ui` の受け付けだけである。代役の部品・名札のデータ・`debug_view_fixture_state` の `pointed`・`point_count` は `test-hooks` の下だけにあり、`make check-delivery` で配信物に入らないことを確かめた
- 判断 3: 代役の部品は、`view()` が属性を付ける関数（`fixture_point`）と同じ関数で受理を決める。見えていない行（前の状態で出ない追加の行など）・表示だけの行・別のビューの宛先・存在しない行の偽造の payload は、状態を変えない（`data-payload` はページ上で書き換えられる、信頼できない値のため）。受理しても見た目は変えず、dirty を積まない
- 判断 4: 宛先の規則はデータごとに持つ（`PointRule`）。既定は「先頭の行だけ表示だけ」で、名札のデータは `screen/top` だけを指せる行にする。spec の ch-run の B4（`screen/top` が 1 件）と同じ形で、名札を自然な並び（top、left、right）のまま置き、`screen/left`・`screen/right` を表示だけの負の対照にするため
- 判断 5: 内側の独立した対話要素（アプリのデモ (b) の `button`・`input`、タスクツリー (a) のチェックボックスなど）の上のクリックは、上流が行の action として届けない（観察: PC4 でアプリのデモ (b) の内側のボタンの上のクリックは受け付けた件数が 0 のまま）。A11Y-20・TASK-494 の二重のタブの止まりと同じ範囲として、本 Issue では変えない
- 判断 6: 製品の `view()` への結線と、製品で指せる行を決める規則は後続（TASK-470・TASK-347 の後続）のままである。したがって、製品の画面で `screen/top` を指せるようにはしていない。本 Issue は、世界状態の `screen/top` から行・属性・入力までの往復を、本 Issue の範囲の部品をつないで native の試験で固定した
- native の試験: `crates/web/tests/point_click_contract.rs`（host の復号、危険な文字を含む view・row の往復、復号できない payload が `None`、`ui` 以外の既存の分岐が変わらないこと、8 Step の教材をクリックの `ui` で完走した状態が `Op` を直接送った状態と一致すること、誤った経路・別のビューの宛先で Step が達成にならないこと、世界状態の名札が (a)・(b) の行・4 属性・`Input::Ui` へ往復すること）。`view_fixture` の単体試験: 復号器の通りに受け付けること、表示だけの行・出ていない行・偽造・別のビューの宛先を無視すること、名札のデータで `screen/top` だけが指せる行になり元に戻せること、データの切り替えが閉じた payload だけを受けること
- ブラウザの試験（`tools/browser/point-click.test.mjs`）: PC1・PC2 は 14 通り（7 種 × 変種、対照を含む）× 前後で、実際のマウスで指せる行を 1 件クリックすると指した行が `{ view, row }` になり件数が 1 増え、表示だけの行のクリックでは変わらないこと。PC3 は名札の (a)・(b) の前後で、`screen/top` だけが `data-action="ui"`・`tabindex="0"` を持ち、クリックで受け付けられ、`screen/left`・`screen/right` は 4 属性を持たずクリックで変わらないこと。PC4 は内側のボタンの観察、PC5 は外部リクエストなどの安全性の計数が 0。クリックの点は、行の action に解決され内側に独立した対話要素が無い点をページ内で選び、見つからなければ失敗にする

```text
実行日: 2026-10-17
元にしたコミット: ebea79a-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/ART11-5.mutation ART11-6.mutation ART11-4.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| ART11-4 | アプリのデモ (b) が宛先を捨てる（再実行） | DETECTED | `point_rows_contract` の「宛先のある行は_4_属性を組で持ち_宛先の無い行は_1_つも持たない」 |
| ART11-5 | host が `ui` を受け付けない | DETECTED | `point_click_contract` の「正規の_payload_は_ui_の入力になる」ほか 4 件 |
| ART11-6 | 復号器が行の空を受ける | DETECTED | `point_click_contract` の「復号できない_payload_は何も受け付けない」 |

- 実行の記録（2026-10-17）: `make fmt`・`make lint`・`make test`（2,110 件が通過、失敗 0）・`make check`・`make check-deps`・`make check-dom-reads`・`make check-generic-words`・`make build-web` と `make check-delivery` が成功。ブラウザの試験は、試験の設定の web で `point-click`（18 件）と `point-rows`・`app-demo-view`・`artifact-view-check`・`artifact-view-controls`・`artifact-mark-toggle`・`view-switch`（計 83 件）が通過。`make test-browser` の全件は未検証。CI には未組み込み
- 変異の定義の置き場: 代役の部品（`pub(crate)`）の受理の条件は統合テストから届かないため、変異は host と復号器に置いた。受理の条件は `view_fixture` の単体試験とブラウザの PC2・PC3 が確かめる
- 範囲外: 製品の `view()` への結線と製品で指せる行を決める規則は TASK-470・TASK-347 の後続、Enter・Space は TASK-358・TASK-375、木の roving tabindex とアプリのデモの二重のタブの止まりは A11Y-18・A11Y-20・TASK-494、フォーカスの見た目と配色は TASK-490、文書の行の ID と `Input::Ui.path` の対応は spec 側の確認事項、`reload_points_host.rs` の `ui` の手を画面の経路へ切り替えること（`Counts` の固定値が変わるため触っていない）、`make test-browser` の CI への組み込み（必須チェックと ruleset の変更が要るため、要対応事項として報告のみ）

### 5.113 左の行の Enter・Space の結線（TASK-358 1/2）

指せる行（`li` に 4 属性が組で付いた行）で Enter・Space を押すと、その行への `click()` になり、マウスの click と同じ経路（上流の委譲 → `decode_action` → host）で受け付けられるようにした。`glue::wire_row_keys` が root の keydown を受け、`key_route::row_key_effect` の結果に従って `preventDefault` と `click()` を行う。D-42 により受け持ちは glue に置き、属性化も上流の口も採らない。

- 受入: 指せる行にフォーカスして Enter・Space を押すと、click と同じ入力になる
- 判断 1: `pointable` は属性を読まず、keydown の target が `HtmlLiElement` かで決める（`web-sys` の feature を 1 つ足しただけで、版と `Cargo.lock` は動かさない）。`tabindex` を持つ `li` は指せる行だけで、宛先の無い行はフォーカスを受けず keydown の target にならない。内側のボタン・入力欄の上では target がその要素になり、glue は何もしない（二重の click を防ぐ）。宛先を受け付けるかの判断は、従来どおり下流（`PointTarget::decode`・core・`is_pointable_now`）が持つ
- 判断 2: `wire_enter_submit` とは別の関数にし、`with_app` を呼ばない。借用を持ったまま `click()` すると、上流の同期の `update` が黙って失敗するため。代役の部品の root にも同じ関数を付ける（`debug_view_fixture_mount`。`test-hooks` の下だけ。失敗したら Runtime を保持しない）
- 判断 3: 要素の取得（`query_selector`・`root()`）は増やさない。click の先はイベントの target そのものである。`MAX_ELEMENT_LOOKUPS`・`PARTS_CONTRACT_VERSION` は変えない
- 判断 4: `no_hidden_triggers` の `glue.rs` の `.click()` の規則を「ちょうど 2 件。直前の錨が `SUBMIT_SELECTOR` の送信ボタン 1 件と、`click_row`（`row_key_effect` の結果）の行 1 件」に改めた。隠しトリガの経路が増えないことは保つ
- 代償: 合成の keydown を宛先の無い `li` へ送ると Space の `preventDefault` と `click()` が起きる。ただし `click()` は上流の解決で何も届けない。スクリプトは `click()` を直接呼べるため、新しい経路にはならない
- native の試験: `crates/web/tests/row_keys_wiring.rs`（`wire_row_keys` の本体に `row_key_effect(`・`HtmlLiElement`・`.click()`・`prevent_default` があり `with_app(` が無いこと、`mount_host` と `debug_view_fixture_mount` が呼ぶこと。欠落の負例つき）、`no_hidden_triggers.rs`（click の規則の改訂と負例: 3 件目・錨の無い click・送信の click だけが残る場合）
- ブラウザの試験（`tools/browser/point-keys.test.mjs`、20 件）: PK1 は 14 通り（7 種 × 変種、対照を含む）× 前後で、フォーカスした指せる行の Enter・Space が行を指し件数が 1 増え、`defaultPrevented` が真でスクロールしないこと。PK2 は click・Enter・Space の状態の変化の一致。PK3 は押しっぱなしで 1 回だけ受け付け、リピート中も `preventDefault` されること。PK4 は Shift+Enter・Control+Enter・Alt+Space・Meta+Enter が何もしないこと。PK5 は名札のデータで `screen/top` だけが受け付けられ、`screen/left`・`screen/right` はフォーカスを受けず Enter でも件数が変わらないこと。PK6 は内側のボタンの Enter の観察（結果は受け付けた件数 0 のまま。期待としては固定しない）。各試験の末尾で外部リクエストなどの安全性の計数が 0
- 負の対照: `wire_row_keys` の `row.click()` を一時的に外して試験の設定で再ビルドし、`point-keys` を単独で流すと 20 件中 18 件（PK1・PK2・PK3・PK5）が失敗し、PK4・PK6 だけが通った（変更は戻した）。結線が実際に効いていることの確認である

```text
実行日: 2026-10-17
元にしたコミット: 2546f64-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/FEND13-1.mutation crates/web/tests/mutations/FEND13-2.mutation FEND7-2 FEND14-1 FEND20-1 FEND4-1
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| FEND13-1 | 左の行の `row.click()` を外す | DETECTED | `no_hidden_triggers` の「実ソースは違反なし」 |
| FEND13-2 | `mount_host` が `wire_row_keys` を呼ばない | DETECTED | `row_keys_wiring` の「実ソースは違反なし」 |
| FEND7-2・FEND14-1・FEND20-1・FEND4-1 | 既存の変異（再実行） | DETECTED | 従来と同じ試験 |

- 実行の記録（2026-10-17）: `make fmt`・`make lint`・`make test`（2,113 件が通過、失敗 0）・`make check`・`make check-deps`・`make check-dom-reads`・`make check-generic-words`・`make check-toolchain`・`make build-web` と `make check-delivery` が成功。ブラウザの試験は、`point-keys`（20 件）、`point-click`・`point-rows`・`app-demo-view`、`enter-submit`・`key-invariants`・`field-space`・`submit-disabled`（38 件）を個別に流して通過。`make test-browser` の全件は未検証。CI には未組み込み
- 限界: 製品の画面（`#app`）にはまだ指せる行が無く、本結線は潜在的である（5.114 で最小の結線を足した）。修飾キー付きの行の Enter・Space は A11Y-18 が定めておらず、保守側（何もしない）に倒した。spec 側での確定が要る。変換中（IME）の行の Enter は native の `row_key_effect` の試験が固定し、ブラウザ試験では扱わない
- 範囲外: キーボードだけで確認を完走する試験は TASK-358 2/2（#399）、A11Y-18 の試験は TASK-375、製品の `view()` への結線と製品で指せる行を決める規則は TASK-470・TASK-347 の後続、木の roving tabindex とアプリのデモの二重のタブの止まりは A11Y-20・TASK-494、フォーカスの見た目は TASK-490、`make test-browser` の CI への組み込み（必須チェックと ruleset の変更が要るため、要対応事項として報告のみ）

### 5.114 キーボードだけで確認を完走する試験と製品の指せる行（TASK-358 2/2）

Tab・Shift+Tab・文字の入力・Enter・Space だけで、確認（`point_at`）を含む Step を達成まで進められることを、製品の画面（`#app`）で確かめる。A11Y-18 の流れ（Tab で行にフォーカスし、Enter・Space が click と同じ扱いで確認が成立する）に合う。

- 受入: キーボードだけで確認を含む Step を達成まで進められ、結果が A11Y-18 の期待と食い違わない
- 判断 1: 製品の `ArtifactPart` には指せる行が無く、そのままでは完走を確かめられないため、最小の結線を足した。選ばれたビューの宣言 ID が引ける行は、すべて `(宣言 ID, 世界状態の経路)` を宛先とする指せる行にする。宛先の受理と判定は従来どおり core（`select_judge`）に任せ、正解の行だけを指せる行にはしない（`rj-miss-point` の意味を残す）。宣言 ID が無い場合と `PointTarget::new` が拒否する行は表示だけに倒す
- 判断 2: 宛先の view は教材の宣言 ID（min-3step では `v-files`）で、種類の綴り（`file_tree`）ではない。core は宣言 ID との完全一致で照合する
- 判断 3: この規則は仮置きである。確定は TASK-470・TASK-347 の後続と TASK-739（#1069）。7 種のビューの本体を製品の `view()` に結線することも範囲外のまま
- native の試験: `crates/web/tests/keyboard_confirm_host.rs`（3 件。確認待ちの行が 4 属性を組で持ち payload が `v-files` と `notes/draft` に復号される・その payload を `decode_action("ui", ..)` に渡すと st-1 が達成になり st-2 へ進む・宣言 ID が無い場合は 4 属性が 1 つも付かない）
- ブラウザの試験（`tools/browser/keyboard-confirm.test.mjs`、CSP あり・なしで 10 件）: KC1 は入力欄へ Tab で移り `/make-note first-note` を入力して Enter、行へ Tab で移って Enter で達成になる。KC2 は Space でも同じ流れでスクロールしない。KC3 は Shift+Tab の逆順でも同じ行に着く。KC4 は達成の後の Enter・Space で達成が 1 回のままで現在の Step が戻らない。KC5 は Shift+Enter では達成にならず、続く Enter で達成になる。操作は `page.keyboard` だけで、信頼されたマウスのイベントが 0 件であることを数える。Tab の回数は観察（入力欄まで 2 回、行まで 3 回）で、固定しない
- 変異: FEND13-3（`ArtifactPart` が指せる属性を付けない）

```text
実行日: 2026-10-17
元にしたコミット: 713f895-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/FEND13-3.mutation crates/web/tests/mutations/FEND13-1.mutation crates/web/tests/mutations/FEND13-2.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| FEND13-3 | `ArtifactPart` が指せる属性を付けない | DETECTED | `keyboard_confirm_host` の「確認待ちの行は_4_属性を組で持ち_宣言_id_の宛先になる」ほか 1 件 |
| FEND13-1・FEND13-2 | 既存の変異（再実行） | DETECTED | 従来と同じ試験 |

- 実行の記録（2026-10-17）: `make fmt`・`make lint`・`make test`（2,116 件が通過、失敗 0）・`make check`・`make check-deps`・`make check-dom-reads`・`make check-generic-words`・`make build-web` と `make check-delivery` が成功。ブラウザの試験は、試験の設定の web で `keyboard-confirm`（10 件）を含む 18 ファイル（151 件。`point-keys`・`point-click`・`point-rows`・`view-switch`・`three-views`・`list-parent-attrs`・`stop-move-ignore`・`reload-points`・`tab-cycle`・`shift-tab`・`enter-submit`・`key-invariants`・`submit-disabled`・`choice-answer`・`approval-flow`・`xss-cases`・`log-lines-xss`）が通過。`make test-browser` の全件は未検証。CI には未組み込み
- 負の対照: `ArtifactPart` の結線を一時的に外して試験の設定で再ビルドすると、`keyboard-confirm` の 10 件がすべて失敗した（変更は戻した）
- 限界: 完走は確認の Step の達成までである。st-2（`choice`）の画面の口は TASK-450 の担当で未実装のため、st-3 以降と 6 Step の CH-1 全体は扱わない。外れた行を指す経路（`rj-miss-point`）は native の既存の試験の範囲。修飾キー付きの Enter・Space は A11Y-18 に定めが無く、保守側（何もしない）のまま。行の操作対象の大きさとフォーカスの見た目は TASK-368・TASK-490。人によるキーボードだけの完走は JUDGE-20 の担当で、本試験は人の評価に数えない
- 範囲外: A11Y-18 の試験そのものは TASK-375（#1008）、木の roving tabindex と二重のタブの止まりは A11Y-20・TASK-494、`step-status` の表示は 5.116（TASK-435 2/2）、`make test-browser` の CI への組み込み（必須チェックと ruleset の変更が要るため、要対応事項として報告のみ）

### 5.115 Step の状態表示（受付済み・確認を省く Step。TASK-435 1/2）

入力が受け付けられた後に右の操作エリアの `step-status` が「受付済み（確認待ち）」になり、確認の前に「達成」が出ないこと、確認を省く Step（`confirm: none`。定義で明示したものだけ）では受付と同時に「達成」が出ることを確かめる。TERM-31 に合う。3 種の確認ごとのブラウザ試験と既存の試験にある「`step-status` は TASK-435 の担当」という注記の除去は TASK-435 2/2（#402、5.116 を使う）の担当で、ここでは触れない。

- 受入: 送信の後に「受付済み（確認待ち）」が出る。確認の前に「達成」が出る例が 0 件。確認を省く Step は、明示した場合に限り、受付と同時に「達成」が出る
- 判断 1（導出）: `crates/web/src/step_status.rs` の `step_status` が `Session` を読むだけで決める。現在の Step の状態が `Accepted` なら受付済み（処理中でも同じ）、`Achieved` なら達成、それ以外はログを末尾から読み、狭い許可リストを読み飛ばして最初に当たったイベントが `Achieved` で、状態の上でも同じ Step が `Achieved` のときだけ達成とする。理由は、core が達成と同じ操作の中で `next` へ遷移して現在の Step を移すため、現在の Step の状態だけでは確認を省く Step と確認の直後の「達成」が一瞬も見えないこと。未読込・未知のイベントは語なしに倒す。表示は「達成」を独自に作らず、確認前の達成が 0 件であることは core（EVT-3・EVT-5）の保証を写す
- 判断 2: `step-status` に属性を足さない（契約の語彙を増やさないため）。契約の版は上げない。`Entered`・`NotStarted`・`Skipped` と章の完了の語は未決のまま語なし（空の文字）にする
- 判断 3: `Reloaded` は読み飛ばさない（`run_status` と同じ。再読み込みの直後は語なしになる）。`offers` は「確認待ち = `Accepted` かつ処理中でない」と読むが、あちらは確認の口を出してよいかの判断で、表示は処理中でも受付済みとする
- native の試験: `crates/web/tests/step_status_contract.rs`（10 件。未読込・開始直後は空、送信の後は進行の各操作の後も受付済みで `Achieved` が 0 件、外れた指し先は受付済みのまま、正しい指し先で達成、確認を省く Step は 1 回の操作で受付と達成がそろい達成になる、確認を `approve` に置き換えた対照では受付済みのまま、承認の確認、移動の直後は空、束縛点は語が変わった操作でだけ dirty）
- ブラウザの試験（`tools/browser/step-status.test.mjs`、3 件）: 開始直後は空で `stateJson` の `stepStatus` は null。送信の後に受付済みになり、5 段の進行の後も待っても受付済みのままで、`MutationObserver` の記録に「達成」が 0 件。`go_to` と選択で達成になり、記録に「受付済み」が 0 件。配信設定には試験の口が無く空。外部リクエスト・エラー・CSP 違反は 0 件
- 変異: TERM31-1（受付済みを語なしへ倒す）・TERM31-2（受付済みを達成として返す）・TERM31-3（ログの末尾の読み取りを外す）。TASK-435 2/2 が足す場合は TERM31-4 から使う

```text
実行日: 2026-10-17
元にしたコミット: bdb4057-dirty
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/TERM31-1.mutation crates/web/tests/mutations/TERM31-2.mutation crates/web/tests/mutations/TERM31-3.mutation crates/web/tests/mutations/TERM33-1.mutation
```

| ID | 変異 | 結果 | 検出した試験 |
|---|---|---|---|
| TERM31-1 | 受付済みを語なしへ倒す | DETECTED | `step_status_contract` の「送信の後は受付済みで_進行の各操作の後も達成にならない」ほか 4 件 |
| TERM31-2 | 受付済みを達成として返す | DETECTED | 同上ほか 4 件 |
| TERM31-3 | ログの末尾の読み取りを外す | DETECTED | 「確認を省く_step_は受付と同時に達成になる」ほか 2 件 |
| TERM33-1 | 既存の変異（再実行） | DETECTED | 従来と同じ試験 |

- 実行の記録（2026-10-17）: `make lint`・`make test`（2,126 件が通過、失敗 0）・`make check`・`make check-deps`・`make check-dom-reads`・`make check-generic-words` が成功。`host_contract` の登録簿の束縛点の件数を 16 から 17 に直した。ブラウザの試験は、試験の設定の web で `step-status`（3 件）に加え `run-status`・`confirm-burst`・`three-views`・`keyboard-confirm`・`reload-points`・`list-parent-attrs`・`stop-redo`・`choice-answer`・`approval-flow`（35 件）が通過。`make check-delivery` と `make test-browser` の全件は未検証（配信設定の web に試験の口が無いことは `step-status` の試験で確認した）。CI には未組み込み
- 限界: `processing` を持つ確認省略の Step は、進行の間「受付済み（確認待ち）」が出て、最後の進行で「達成」になる。語は契約の版 1 の固定語なので、確認を省く Step でも「確認待ち」の文言は変わらない。停止（`Control::Stop`）は core の Step の状態を戻さないため、停止の直後も受付済みのままになる（「達成」にはならない）
- 範囲外: 3 種の確認ごとのブラウザ試験と注記の除去（#402）、`Entered` などの語の追加（spec の TERM-31 を先に直し契約の版を上げる必要がある）、配置の確定（TASK-517・TASK-438）、ライブの欄（TASK-366・TASK-370）、`make test-browser` の CI への組み込み（必須チェックと ruleset の変更が要るため要対応事項として報告のみ）

### 5.116 確認の 3 種の操作の後の達成の表示（TASK-435 2/2）

指す（`point_at`）・答える（`answer`）・承認する（`approve`）の 3 種の確認ごとに、送信 → 「受付済み（確認待ち）」→ 確認の操作 → 「達成」を実ブラウザで確かめる。TERM-31 に合う。実装の変更は無く、5.115 の導出と結線の受入を、確認の種類ごとに実際の UI の操作で確かめる試験である。

- 受入: 確認の操作の後に「達成」が出る試験が 3 種で通る。3 種のいずれでも、確認の操作の前に「達成」が DOM に出た例が 0 件（`MutationObserver` の記録で確かめる）
- 判断 1: 入力の送信は `raw-trigger` へ `Op` の JSON を送って決定的にし、確認の操作は Playwright の実クリックで行う。確認の操作の直前に記録へ印を入れ、印より前に受付済みが 1 件以上・達成が 0 件、印より後に達成が出ることを確かめる（観察器が動いていることの対照を兼ねる）
- 判断 2: 答えると承認では、誤った操作（誤った選択肢・対象なしの「直す」）の負の側を含める。拒否になっても語は受付済みのまま（承認の最初の拒否では空）で、「達成」は出ない。st-1 だけを扱い、st-2 以降の通しは `approval-flow` の担当とする
- 判断 3: 語は DOM の `textContent` と `stateJson` の `engine.stepStatus`・`achieved` の件数・`current` を突き合わせる
- 試験（`tools/browser/confirm-status.test.mjs`、3 種 × CSP あり・なしの 6 件）: CS-POINT（mount の教材の st-1。左の行 notes/draft のクリック）、CS-ANSWER（`load-fixture:answer`。誤った案の後に正しい案）、CS-APPROVE（`load-fixture:approval`。対象なしの直す、対象つきの直す、はい）。各ケースの最後に外部リクエスト・エラー・CSP 違反・ダイアログ・危険な要素が 0 件であることを確かめる
- 変異: 足さない。導出は変えず、TERM31-1〜3 と native の契約試験が既に覆う。変異注入（`scripts/check-mutations.sh`）はブラウザ試験を対象にしない
- 既存の試験の注記: `choice-answer`・`approval-flow`・`keyboard-confirm`・`step-status` の「`step-status` は TASK-435 の担当」を、本節の試験への参照に直した（assertion は変えていない）

```text
実行日: 2026-10-17
元にしたコミット: eec5fe0（作業ツリーに本変更）
コマンド: tools/build-web.sh（TEST_HOOKS=1 と配信設定）の後、FANDHE_WEB_DIR=dist/web FANDHE_WEB_TEST_DIR=dist/web-test node --test tools/browser/confirm-status.test.mjs
結果: 6 件が通過、失敗 0
```

- 限界: `processing` を持つ確認省略の Step の文言、停止の直後の表示は 5.115 の限界のまま。確認の後に次の Step へ移っても語が「達成」のままであることまでは確かめるが、次の Step の受付後の語は本試験の対象外
- 範囲外: `Entered` などの語の追加（spec の TERM-31 を先に直し契約の版を上げる必要がある）、配置の確定（TASK-517・TASK-438）、ライブの欄（TASK-366・TASK-370）、st-2（`choice`）の画面の口（TASK-450）、`make test-browser` の CI への組み込み（必須チェックと ruleset の変更が要るため要対応事項として報告のみ）

### 5.117 CH-0〜CH-3 の通しと CH-1 のキーボードだけの完走（TASK-450 3/3）

CH-0〜CH-3 の形の 20 Step を 1 つのページで最後まで自動操作で進め、CH-1 の 6 Step をキーボードだけで完走する。TERM-23 の残りに合う。5 方式（コマンド・複数行の自然文・選択肢・画面操作・承認）と 4 種の確認（指す・答える・承認する・なし）を画面から操作し、通しの途中で安全の計数がすべて 0 件であることを確かめる。

- 解釈: 教材 repo が無く（2026-10-17 に `gh repo list Fandhe-AI` で確認。TASK-642 が未完了）、core にも章の完了・遷移先の入口の写しの作り直し・規則の `when` の評価が無い。このため、CH-0〜CH-3 の形（章ごとの Step 数・入力方式・確認の種類・拒否・問い返し・補完・処理中）を写した試験用の教材（`crates/web/tests/data/ch0_3/ir.json`。`pack.id` は `ch0-3-run`。作り方は同じ場所の `README.md`）を、無条件の `next` だけで線形につないで通す。試作の 73 項目は目標にせず、項目数は「N/N 項目」と記録する
- 受入: (1) 試験の設定の画面を `?fixture=ch0-3` で開き、CSP あり・なしの両方で `main/st-3-2` が `achieved` になるまで進む。`raw-trigger` の click と `load-fixture:` は 0 回 (2) 5 方式と 4 種の確認を 1 回以上ずつ操作 (3) CH-1 の 6 Step を、`waitMounted` の後は `page.keyboard` だけで完走し、信頼されたマウスのイベントは 0 件 (4) ページのエラー・配信元以外への通信・ダイアログ・危険な要素・CSP 違反が 0 件 (5) 本節の記録
- 判断 1: 教材は 20 Step（CH-0 が 4、CH-1 が 6、CH-2 が 8、CH-3 が 2）。各 Step は `views` を 1 件だけ宣言し、指す対象の行は Step に入る前か同じ Step の受付の効果で作る（`on_enter`・`starts[].effects` は core が適用しないため使わない。`reload_points_ch2` の st-7 がテスト専用の口に頼った原因はここにある）
- 判断 2: 開始状態の補助。`test_fixture::session_of` が、契約の試料の開始状態の節点（`main/st-1`）を教材の `starts[0].node` に合わせる。既存の 3 つの教材は先頭が `main/st-1` で結果は変わらない（単体試験で固定）。`Session::from_state` の呼び出しは 1 箇所のまま（`tests/no_progress.rs`）
- 判断 3: 規則の `when` を使わない。`ch1-pack.json` の st-1-4 の取り消しの規則は `when: {clarifying: …}` を持つが、core は `when` を評価しないため、`when` なしで書き直した。Esc の取り消しは問い返しの後に押す順で確かめる
- 判断 4: キーボードの区間は `page.evaluate` を観察（フォーカス・状態の読み取り）だけに使う。目的の要素へは Tab を押して `document.activeElement` が着くまで繰り返し、回数は上限（300）だけ置いて固定しない。拒否の後は下書きが残るため、次の入力の前に全選択して置き換える。マウスの click は `detail` が 1 以上のものだけ数える（Enter・Space によるボタンの click も信頼されたイベントになるため）
- 試験: native `crates/web/tests/ch0_3_run_host.rs`（3 件。模範の操作列 47 手を、判定の種類・現在の Step・状態つきで流す。警告・無視・省略による達成が 0 件で、確認のある Step の達成の直前の判定が「確認」であること。5 方式と 4 種の確認が教材と手の両方にあること）。ブラウザ `tools/browser/ch0-3-run.test.mjs`（通し × CSP あり・なしの 2 件が各 57/57 項目、キーボード × CSP あり・なしの 2 件が各 34/34 項目）。`test_fixture` の単体試験に 3 件（クエリの完全一致・鍵と 20 Step と先頭の Step・既定の教材の開始状態が変わらないこと）
- 操作の範囲: 拒否（誤答・綴り違い・順序違い・対象なしの直す・早すぎるはい・外れた行）、問い返し、Tab の補完、Shift+Enter の改行、Esc の取り消し、処理中の完了待ち（4 進行と 2 進行）、選択肢の Enter と Space。各方式に入った直後と確認待ちで、選択肢・承認・入力欄・送信の数を表のとおりに数える

試作の 73 項目との対応（試作の通しの実行元 `03-poc/_postfreeze-fixes/tools/ch-run.mjs` は本リポジトリから読めず、項目ごとの突き合わせは未確認）:

| 区分 | 内容 | 理由 |
|---|---|---|
| 形を変えて再現した | 4 + 6 + 8 + 2 = 20 Step の通し、5 方式・4 種の確認、拒否・問い返し・補完・複数行・Esc の取り消し・処理中の完了待ち、CH-1 のキーボードだけの完走 | 教材は試作の文面ではなく、形を写した試験用の教材 |
| 再現していない | 章の完了と「次の章へ進む」の表示 | core が章の完了（`ChapterCompleted`・`completed`）をまだ扱わず（`confirm.rs` の「扱わないこと」。TASK-233 以降）、製品の画面にも口が無い |
| 再現していない | 遷移先の `on_enter` の名札の行、入口の写しの作り直し | core が遷移で入口の写しを作り直さない（`go_to.rs`・`checkpoint.rs` だけが代入する） |
| 再現していない | 規則の `when`（`clarifying` など）、条件つきの辺 | core が評価しない（`accept.rs` の `NoConditions`） |
| 再現していない | チェックポイントの復帰、本文と do の表示 | 製品の画面にボタンも表示も無い |
| 再現していない | 処理中の Esc による停止 | 先頭以外の Step では入口の写しが無く、停止が警告で無視される（`stop.rs` の規則 2）。通しにもキーボードの区間にも入れない |

担当のタスク番号は `docs/spec/05-tasks.md` で確かめていないため「未確認」とする。

**教材 repo の CH-0〜CH-3 での流し直し: 未実施。** 理由は、教材 repo が未作成で TASK-642 が未完了なこと（2026-10-17 に確認）。教材 repo ができた後に、本試験の手順（`?fixture=ch0-3` の教材を本物に差し替える）で流し直す。

変異（`crates/web/tests/mutations/TERM23-5.mutation` を新規。TERM23-1〜4 は TASK-450 1/3・2/3 の変異で、本節で再実行して記録する。1/3・2/3 の節は足さない）:

| ID | 対象 | 結果 |
|---|---|---|
| TERM23-1 | `offers.rs`: 入力方式 `choice` の選択肢を入力待ちでなく確認待ちで出す | DETECTED |
| TERM23-2 | `offers.rs`: 壊れた宣言で空に倒さず口を出す | DETECTED |
| TERM23-3 | `hello.rs`: 入力欄なしの設定を表示に反映しない | DETECTED |
| TERM23-4 | `host.rs`: 入力欄なしの教材でも下書き・送信・補完を受け付ける | DETECTED |
| TERM23-5 | `host.rs`: 対象つきの「直す」のクリックを「はい」として送る（`ch0_3_run_host` が検出） | DETECTED |

```text
実行日: 2026-10-17
元にしたコミット: cae5e2f（作業ツリーに本変更）
コマンド（変異）: scripts/check-mutations.sh crates/web/tests/mutations/TERM23-1.mutation 〜 TERM23-5.mutation
コマンド（native）: cargo test -p fandhe-workshop-web --features test-hooks --test ch0_3_run_host（3 件が通過）
コマンド（ブラウザ）: TEST_HOOKS=1 tools/build-web.sh の出力を FANDHE_WEB_TEST_DIR、TEST_HOOKS なしの出力を FANDHE_WEB_DIR にして
  node --test tools/browser/ch0-3-run.test.mjs ほか input-ports・input-field-hidden・keyboard-confirm・reload-points・choice-answer・approval-flow・esc-outcomes・candidates
結果: 9 ファイル 64 件が通過、失敗 0（ch0-3-run は 4 件）
負の対照: `test_fixture.rs` の `?fixture=ch0-3` の分岐を外して再ビルドすると、ch0-3-run の 4 件がすべて失敗する（既定の教材で始まり、最初の待ちが時間切れ）
配信物: make build-web → make check-delivery が RESULT: ok。配信物の wasm に ch0-3-run の文字列は無い
```

- 未検証: `make test-browser` の全体（9 ファイルだけ流した）
- 限界: 試験用の教材であり、本物の CH-0〜CH-3 の挙動は確かめていない。処理中の停止は通しに入れていない。Esc は入力欄にフォーカスがある場面だけ（ボタンにフォーカスがあるときの Esc は `main` に届かない。TASK-450 2/3 の対象外の事項）
- 範囲外: 教材 repo での流し直し（TASK-642 の後）、章の完了・入口の写しの作り直し・`when` の評価・チェックポイントの復帰の口、先頭以外の Step で処理中に停止が無視されること（全周の通し TASK-92 の前に解消が要る）、`make test-browser` の CI への組み込み（必須チェックと ruleset の変更が要るため要対応事項として報告のみ）

### 5.118 外観の閉じた集合と E-PARSE（TASK-53 1/2）

`pack.appearance.theme` が 4 値（`standard`・`mono`・`high_contrast`・`reading`）に限られ、名前にない値と `theme` 以外の欄が `E-PARSE` になることを確かめる（PACK-21）。契約の読み取り（`appearance_contract`）、`compile`・`validate`・CLI・TOML の診断（pack の `appearance`）、IR から `data-theme` までの通し（web の `theme_shared`）を試験する。語彙にスタイルを指定する欄が 0 件であることも確かめる。

- PACK21-1: 未知の欄を拒否する分岐を無効化する
- PACK21-2: `compile` が外観の診断を捨てる

```text
実行日: 2026-10-18
元にしたコミット: 2313656（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/contract/tests/mutations/PACK21-1.mutation crates/pack/tests/mutations/PACK21-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| PACK21-1 | crates/contract/src/appearance.rs: theme 以外の欄を拒否する分岐を無効化する | unknown_keys_are_rejected | unknown_keys_are_rejected | DETECTED |
| PACK21-2 | crates/pack/src/compile.rs: compile が外観の診断を捨てる | bad_appearance_is_one_parse_error_without_values | toml_form, bad_appearance_is_one_parse_error_without_values, cli_reports_and_accepts | DETECTED |

### 5.119 左エリアの設定と E-LEFT-* の 3 種（TASK-53 2/2）

`pack.left_pane` の 2 項目（`world_list` 2 値・`view_tabs` 3 値）が閉じた集合であること、名前にない値・型違い・未知の欄が `E-PARSE` になること、既定から変えたときだけ `E-LEFT-MULTI-VIEW`・`E-LEFT-TARGET-VIEW`・`E-LEFT-TARGET-ROW` が期待のファイル・項目で報告されることを確かめる（PACK-22）。契約の読み取り（`left_pane_contract`）、`compile`・`validate`・TOML・CLI の診断（pack の `left_pane`）、web が同じ型を使うこと（`left_pane_shared`）を試験する。

- PACK22-1: `view_tabs` の名前にない値を拒否せず既定へ倒す
- PACK22-2: `compile` が `left_pane` の `E-PARSE` を捨てる
- PACK22-3: `E-LEFT-TARGET-ROW` の「どの `put`・`replace` にも現れない」判定を無効化する

```text
実行日: 2026-10-18
元にしたコミット: 51a5578（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/contract/tests/mutations/PACK22-1.mutation crates/pack/tests/mutations/PACK22-2.mutation crates/pack/tests/mutations/PACK22-3.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| PACK22-1 | crates/contract/src/left_pane.rs: view_tabs の名前にない値を拒否せず既定へ倒す | unknown_values_are_rejected | unknown_values_are_rejected | DETECTED |
| PACK22-2 | crates/pack/src/compile.rs: compile が left_pane の E-PARSE を捨てる | bad_left_pane_is_one_parse_error_without_values | bad_left_pane_is_one_parse_error_without_values | DETECTED |
| PACK22-3 | crates/pack/src/left_pane.rs: E-LEFT-TARGET-ROW の判定を無効化する | target_row_is_checked_even_with_world_list_hidden_only | diagnostics_do_not_leak_values, target_row_is_checked_even_with_world_list_hidden_only, cli_reports_all_three_codes_and_accepts_the_fixed_pack, parameterised_targets_and_puts | DETECTED |

### 5.120 存在しない要素への remove・set_attr の検査（TASK-58 2/2）

`remove`・`set_attr` の経路を、教材のどの `put`・`replace` も置かないとき、`validate` が `E-REF-ELEMENT` を期待のファイル・項目で報告することを確かめる（PACK-30・D-43）。欠陥パック `elements/{a,b}/M01〜M03` の検出、置かれる経路（`put`・`replace`・`${}` を含む `put`・開始点の `put`）の対照、診断が経路以外の値を写さないことを試験する（`missing_elements`）。遮蔽（先の規則が後の規則を隠す）は検査に足さない限界として `limits/{a,b}/L03` に固定した（`static_limits`）。

- PACK30-1: `E-REF-ELEMENT` の「どの `put`・`replace` も置かない」判定を常に偽にする

```text
実行日: 2026-10-18
元にしたコミット: 30bb88f（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/PACK30-1.mutation crates/pack/tests/mutations/PACK22-3.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| PACK30-1 | crates/pack/src/element_check.rs: E-REF-ELEMENT の判定を常に偽にする | pack30_missing_element_defects_reported_at_expected_location | pack30_missing_element_defects_reported_at_expected_location, pack30_unsafe_path_and_values_are_not_echoed, pack30_start_effects_are_checked_and_put_by_start_places | DETECTED |

`placed` の共有化（`left_pane.rs` から `placed.rs` へ移した）の後も、`PACK22-3` が DETECTED であることを同じ実行で確かめた。

### 5.121 宣言より新しい語彙を使う教材の検査（TASK-59 1/2）

`pack.vocab` を書いた教材が、宣言より新しい版の拡張点（EP-9〜EP-12）を使うと、`validate` が `E-VOCAB-MISMATCH` を拡張点ごとに 1 件報告することを確かめる（PACK-31・VER-14・D-43）。宣言が一致する教材と `pack.vocab` の省略は報告しないこと、読めない宣言が値を写さない `E-PARSE` になることを試験する（`vocab_declaration`）。

- PACK31-1: 宣言と必要な版の比較を常に偽にする

```text
実行日: 2026-10-18
元にしたコミット: 69ed2fe（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/PACK31-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| PACK31-1 | crates/pack/src/vocab_check.rs: 宣言と必要な版の比較を常に偽にする | pack31_ep9_with_vocab_below_1_2_is_reported | pack31_ep9_with_vocab_below_1_2_is_reported, pack31_ep9_in_module_start_is_reported, pack31_newer_vocab_than_declared_is_reported, pack31_one_diagnostic_per_extension_point, pack31_spec_pack_declaring_1_0_but_using_requires_is_reported | DETECTED |

### 5.122 確認の選択肢の label の版の検査と誤検出 0 件（TASK-59 2/2）

`confirm.options[].label`（語彙 1.3）を使う教材が `pack.vocab` に 1.3 未満を宣言すると、`validate` が `E-VOCAB-MISMATCH` を 1 件報告することを確かめる（PACK-31・VER-14・D-43）。入力方式 `choice` の `options[].label`（v1 の語）を取り違えないこと、宣言と使用が一致する教材（`testdata/` の教材と `confirm-label-pack`）で誤検出が 0 件であることも試験する（`vocab_declaration`）。

- PACK31-2: 確認の選択肢の `label` を使用と数える判定を反転させる
- PACK31-3: `confirm` の代わりに `input` を見る（入力方式 `choice` の `label` を取り違える）

```text
実行日: 2026-10-18
元にしたコミット: 5b4fffd（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/PACK31-2.mutation（PACK31-3 も同様）
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| PACK31-2 | crates/pack/src/vocab_check.rs: 確認の選択肢の label を使用と数える判定を反転させる | pack31_label_with_vocab_below_1_3_is_reported | pack31_label_reported_once_per_pack_and_orders_after_ep12, pack31_label_in_module_step_is_reported, pack31_confirm_label_pack_positive_control, pack31_label_with_vocab_below_1_3_is_reported, pack31_label_matching_or_omitted_or_null_is_not_reported | DETECTED |
| PACK31-3 | crates/pack/src/vocab_check.rs: confirm の代わりに input を見る | pack31_input_choice_label_is_not_confirm_label | pack31_label_in_module_step_is_reported, pack31_label_reported_once_per_pack_and_orders_after_ep12, pack31_label_with_vocab_below_1_3_is_reported, pack31_input_choice_label_is_not_confirm_label, pack31_matching_declarations_are_not_reported, pack31_correct_packs_have_no_false_positives | DETECTED |

### 5.123 行の途中の改行の連結（TASK-61）

Markdown の本文で、前後がともに CJK（かな・漢字・全角記号）の行の途中の改行を連結し、英数字・混在・インラインの境界の改行は `"\n"` のまま残すことを確かめる（PACK-27・D-51）。fixture-pack の本文を実際の経路で compile して、連結と非連結の対照も試験する（`soft_break_join`）。

- PACK27-1: 連結の判定を常に偽にする
- PACK27-2: 連結の判定を常に真にする
- PACK27-3: 直後の文字の検査を外す

```text
実行日: 2026-10-18
元にしたコミット: b5a4a61（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/PACK27-1.mutation（PACK27-2・PACK27-3 も同様）
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| PACK27-1 | crates/pack/src/markdown.rs: 連結の判定を常に偽にする | pack27_japanese_soft_break_is_joined | pack27_joins_inside_emphasis_list_and_heading, pack27_japanese_soft_break_is_joined, pack27_compiled_pack_joins_cjk_and_keeps_ascii | DETECTED |
| PACK27-2 | crates/pack/src/markdown.rs: 連結の判定を常に真にする | pack27_ascii_soft_break_is_kept | pack27_mixed_soft_break_is_kept, pack27_ascii_soft_break_is_kept, pack27_inline_boundary_is_kept, pack27_compiled_pack_joins_cjk_and_keeps_ascii | DETECTED |
| PACK27-3 | crates/pack/src/markdown.rs: 直後の文字の検査を外す | pack27_mixed_soft_break_is_kept | pack27_mixed_soft_break_is_kept | DETECTED |

### 5.124 文書ビューの本文の語（TASK-66 1/2）

文書のビューの宣言 `views.<ID>.body: shown` が IR に載り、本文の木がそのビューの行の要素（`fixture`）から引けること、`replace` で `fixture` を替えると本文が替わること、省略と `hidden` では本文が出ないこと、不正な値が値を写さない `E-PARSE` で `compile` が IR を出さないこと、語彙 1.3 の宣言で使うと `E-VOCAB-MISMATCH`（EP-13）になることを確かめる（PACK-38・D-50・D-51）。契約の読み取りと `body_doc` の `None` に倒れる場合も試験する（`view_body_contract`・`view_body`）。

- PACK38-1: 未知の綴りの拒否を外す（`visible` などが黙って `hidden` として通る）
- PACK38-2: `views.body` を使用と数える判定を常に偽にする

```text
実行日: 2026-10-18
元にしたコミット: 0fa7146（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/PACK38-1.mutation（PACK38-2 も同様）
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| PACK38-1 | crates/contract/src/view_body.rs: 未知の綴りの拒否を外す | unknown_spelling_is_refused_exactly | unknown_spelling_is_refused_exactly | DETECTED |
| PACK38-2 | crates/pack/src/vocab_check.rs: views.body を使用と数える判定を常に偽にする | vocab_declaration_older_than_1_4_is_a_mismatch | vocab_declaration_older_than_1_4_is_a_mismatch | DETECTED |

### 5.125 選んだ値の差し込みの限界の観察（TASK-66 2/2）

受講者が選んだ値を差し込む語は足さないと決まった（D-50、PACK-39）。この決定の下での現状を、`crates/pack/tests/chosen_value_gap.rs` の `pack39_` の 10 件で固定する。差し込みらしい欄が `on_enter` で `E-PARSE` になること、規則の `outcome.effects` では読み込みは通るが実行時に入力ごと拒否されること、受講者の値を指す枠が `E-SLOT-UNKNOWN` になること、章の `${名前}` が文字のまま世界状態に残ること、列挙・引数の条件のない規則・選択ごとの固定値の属性の回避が書けること、列挙にない値が拒否され世界状態が変わらないことを確かめる。

変異は置かない。Rust のソースを変えず、新しく守る実装の分岐が無いためである（PACK-44 の試験と同じ扱い）。限界の記録は `docs/known-gaps.md`。

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
| `fandhe-frontend-headless-ui` | 0.69.2（直接の依存。`tree_view::item_text` を使う。TASK-473。`marker::root`・`content` も使う。TASK-477・TASK-478） |
| `fandhe-frontend-wireframe-ui` | 0.52.0（直接の依存。`rich_text` を使う。TASK-473。`tag` も使う。TASK-477・TASK-478） |

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
