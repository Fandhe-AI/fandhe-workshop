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
- 同じ入力列の 2 回適用の決定性（TASK-218 1/3、EVT-22・EVT-16、P-5）は `crates/core/tests/determinism.rs`、時刻・乱数・大域状態の源が
  core・contract に無いことの静的検査は `determinism_sources.rs`、記録と比較器は `common/trace.rs` にある。実測は 5.277
- 表示文字列の差し替えからの独立（TASK-218 2/3、EVT-2・P-7）は `crates/core/tests/text_independence.rs`、差し替えの部品は
  `common/text_table.rs` にある。元・表 A・表 B の 3 つの IR に同じ入力列を流し、`Judged` の次の行動を文面の ID に写して比べる。実測は 5.281
- 文言ではなく ID を変えると差が出る負の対照（TASK-218 3/3、EVT-2・P-7）は `crates/core/tests/id_change_control.rs` にある。拒否の文面の ID だけを付け替えた IR で、5.281 と同じ比べ方が差を検出することを確かめる。実測は 5.282
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
- 履歴が追記だけで増える性質（TASK-188 1/2、EXT-5・D-53）は `crates/core/tests/history_append_only.rs` にある。前後 2 つの包みについて、
  頭の欄（`format`・`history_format`・`pack_id`・`start`・4 軸の `versions`）が変わらないこと、`ops` が確定した部分を書き換えないこと
  （末尾の未確定の `Draft` だけは置き換わってよい。今の記録規則のまま）、`snapshot.log` の接頭辞が保たれること、
  `snapshot.checkpoints` の既存の項目が変わらず `next_checkpoint` が減らないことを確かめる。`run`・`entry`・`drafts`・`hints_opened` は
  列ではなく現在の状態で上書きされるため対象にしない。記録 1 段ごと・離れた 2 点・書き出し→読み込みを通した形に加え、時点 t の包みを
  `boot_session` で復元して本番の保存の入口と続きの記録でも確かめる。固定・誘導つき・再読み込みを混ぜた生成と起動をまたぐ形の 6 本を各 1,000 通り流し、
  固定の列（各種類の増え方を網羅）と検査器の自己確認を足した 8 本の実測は dev プロファイルで約 6.5 秒（2026-10-19、`cargo test` の試験の所要時間）。
  変異は 5.141 節
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
| `id` | 必須 | 変異の ID。ファイル名（`.mutation` を除く）と同じ値にする。使える文字は `^[A-Za-z0-9_-]+$`。正は `crates/*/tests/mutations/*.mutation` の定義で、本表では列挙しない（規則は表の下） |
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
- ID の正は `crates/<crate>/tests/mutations/<ID>.mutation` の定義そのものである。どのタスクがどの ID を足したかは、第 5 節の各小見出しと記録の表の ID 列に残る
- 本節で ID を列挙しない理由: 並列にマージされる PR が同じ 1 行へ追記し、漏れと衝突が起きたため（Issue #1560。2026-10-19 時点の定義は 425 件）
- 新しい ID は `<ビヘイビア ID からハイフンを除いたもの>-<連番>` で付ける（例: `SAVE-19` → `SAVE19-6`）。連番は、同じ crate の同じ族の既存の最大の次を使う
- 既存の例外は改名しない。足場の時期の `<英大文字 1 字><連番>`（`S1`・`M1` など）と、`JUDGE1-C1` のような英字つき枝番である
- ID はファイル名なので、同じ crate の中では重ならない。crate をまたぐ重複は避ける。既存の `VER7-1`（contract と core）は、第 5 節の記録が参照するため改名しない
- ファイル名と `id` の一致は、今は検査していない（2026-10-19 に全 425 件の一致を確かめた）

### 3.3 新しい変異を足す手順

1. 壊したい 1 か所を決め、`find`（ちょうど 1 回現れる文字列）と `replace` を書く。ID は 3.2 節の規則で付け、ファイル名を ID と同じにする
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
- 問い返しの後の欄は、2026-10-21 に空にすると確定した（D-67。JUDGE-27・TERM-22）。この試験では固定しない（拒否のときだけ残す）。固定する試験は 5.198 にある
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
- 問い返しのブラウザ試験は未対応。試験の設定の mount は `min-3step` の IR を埋め込み、その `clarifications` は空で、`open_history` は IR を固定するため、問い返しを起こせない。別の Issue で扱う（2026-10-21 に `?fixture=ch0-3` の教材で問い返しを起こせるようになり、問い返しの後の欄の試験を 5.198 に足した。この記述は当時のもの）

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

TASK-448 1/2（TERM-28）で、`approval_offers` の候補の元を規則の対象から `revise_candidates` に替えた。TERM27-2 の `find`・`replace` は新しい filter の行
（`.filter(|c| !c.id.trim().is_empty())`）に合わせて更新した。TERM28-1 は対象つきの「直す」の文字を表示名でなく ID にする変異、
TERM28-2 は表示名つきの候補をボタンにしない変異で、どちらも native の `crates/web/tests/approval_buttons.rs` が検出する。

```text
実行日: 2026-10-23
元にしたコミット: fc777b1-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/TERM27-2.mutation crates/web/tests/mutations/TERM28-1.mutation crates/web/tests/mutations/TERM28-2.mutation crates/web/tests/mutations/TERM29-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| TERM27-2 | crates/web/src/offers.rs: 対象つきのボタンを出さない（find を更新） | 入力の段は三つと対象の数だけのボタンが出る | 左の 1 件に加え、重複・太い形・拒否・エスケープの複数件 | DETECTED | - |
| TERM28-1 | crates/web/src/offers.rs: 対象つきの直すの文字を ID にする | 候補の数だけ対象つきのボタンが出て_文字は表示名で_id_は属性だけに出る | 左の 1 件に加え、表示名の逃がしの 1 件 | DETECTED | - |
| TERM28-2 | crates/web/src/offers.rs: 表示名つきの候補をボタンにしない | 候補の数だけ対象つきのボタンが出て_文字は表示名で_id_は属性だけに出る | 左の 1 件に加え、逃がし・誤った候補の押下の 2 件 | DETECTED | - |
| TERM29-1 | crates/web/src/offers.rs: 候補が 0 件の承認の Step（再承認の Step）でボタンを出さない | 差し戻しの後は再承認の_step_で三つが出て_はいで進み_直すといいえは拒否になる | 同左（1 件） | DETECTED | - |

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
契約試験 `key_decide_contract` が検出する。候補が開く状態は TASK-440 が結ぶまで画面から作れないため、ブラウザ試験（`tools/browser/shift-tab.test.mjs`）は候補なしの行（`preventDefault` されない・入力欄から履歴の読み込みの部品へ移る・送信ボタンから入力欄へ戻る）だけを確かめる。

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
- 例外は 2 種類で、理由をスクリプトに書いた。字句の完全一致の許可は `thread_local`（Rust 標準のマクロ）と `SlashCount`（区切り文字 `/` の個数）。件数を固定した例外は `crates/core/src/command_class.rs` の `skill` が 3 件（コマンドの種類 `skill` と付録 A の `skill` の実際の衝突。増減すると失敗する）と、`crates/core/src/secret_scan.rs` の `github` が 5 件・`gh` が 2 件（秘密情報の形の ID・名前・接頭辞が付録 A の語に当たる。ID は spec の試験データ `secret-shapes.json` の `expect` と同じ値にする必要がある。TASK-164 1/3。5.177）
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
- 列の追加（Enter の 11 行にも付与した）: `web`（`delivery` か `test`）・`open`（押す前の一覧の開閉。前提として照合する）・`key`（捕捉する `KeyboardEvent.key`）・`focus`（押した後の `document.activeElement` の `data-testid`。`body` は `BODY`、履歴の読み込みのファイル入力は `IMPORT-FILE`）。ログの行は「押す前の行 + `lines` = 押した後の行」で照合する。欄の値が変わる行は `waitForFunction` で待つ（Tab の補完は `set_timeout(0)` を 2 段通る）
- Enter の 11 行の `focus` は全行 `draft-input` で、事前の見込みと一致した

| id | web | 一覧（前提） | キー | 下書き | preventDefault | ログの行 | 欄の値 | フォーカス先 |
|---|---|---|---|---|---|---|---|---|
| TAB-TEXT | 配信 | 閉 | Tab | `x` | なし | なし | `x` | `submit-input` |
| TAB-EMPTY | 配信 | 閉 | Tab | 空 | なし | なし | 空 | `submit-input` |
| SHIFT-TAB-TEXT | 配信 | 閉 | Shift+Tab | `x` | なし | なし | `x` | `IMPORT-FILE` |
| SHIFT-TAB-EMPTY | 配信 | 閉 | Shift+Tab | 空 | なし | なし | 空 | `IMPORT-FILE` |
| TAB-OPEN | 試験 | 開 | Tab | `/he` | あり | なし | `/help` | `draft-input` |
| SHIFT-TAB-OPEN | 試験 | 開 | Shift+Tab | `/he` | なし | なし | `/he` | `keydown-probe` |
| TAB-NOMATCH | 試験 | 閉 | Tab | `/zzz` | なし | なし | `/zzz` | `submit-input` |
| TAB-EXACT | 試験 | 閉 | Tab | `/help` | なし | なし | `/help` | `submit-input` |

現状で観察した値は、事前の見込み（`decide` と既存の試験）と全行で一致した。食い違いは無い。SHIFT-TAB-OPEN のフォーカス先 `keydown-probe` は、試験の設定にだけある入力欄の前の要素で、配信設定では、入力欄の前にある履歴の操作（TASK-531 1/2）の最後の部品（読み込みのファイル入力。表では `IMPORT-FILE`）へ移る（SHIFT-TAB-TEXT）。この PR より前は `BODY` だった。

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
- web の結線: ホストは `RecordedSession` を通して操作を記録する（`apply_engine`・`submit_engine`・入力欄の下書きの変更）。起動の前に `persist::mount_with_history` が保存先を開いて保存済みの値を読み、`EngineHost::boot` の後にマウントする（`mount_app` は `Promise` を返し、`app.js` が `await` する）。離れる直前（`pagehide`）に `history_to_save` が値を返すときだけ保存先へ書く。保存の遅延のタイマーは、この時点では置かなかった（TASK-318 で `glue` に置いた。5.170）。保存済みの値を読めない・開けないときは保存を止める（fail-closed）
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
| 再現していない | 章の完了イベント（`ChapterCompleted`・`completed`） | core がまだ扱わない（`confirm.rs` の「扱わないこと」。TASK-233 以降）。「次の章へ進む」の表示と移動は状態から導いて画面に出し `Control::GoTo` で移る（5.224・5.227） |
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

### 5.126 観察の記録の試験（TASK-79 2/2）

試作の `browser-test-poc7.mjs` に当たる「観察の記録」を `tools/browser/safety-observe.test.mjs`（node:test、7 本）として `make test-browser` に加え、`browser test (Chromium)` の関門に含めた。試作は `finally` の `process.exit(0)` が例外を覆い、失敗しても終了コードが 0 だった。ここは node:test なので例外は必ず失敗になる。

観察値（件数・段・時間・有無）は assert せず、`t.diagnostic()` の `[節名] ...` の行（CI のログ）に残す。assert するのは基盤だけである。環境変数・マウント・test-hooks の口・ページの例外 0 件・各節の空振り（読んだ件数 0 でない、送った回数が 49 × 2 など）。環境変数の未設定は skip にせず失敗にする。秘密情報らしい値は test の中で部品から組み立て、形の ID（S-01〜S-10）と有無の件数だけを出す。

| 節 | 今のビルドでの観察 | 育てる先（未実装の部分） |
|---|---|---|
| data | 5 つの `.sha256` を `node:crypto` で照合（16 件、不一致 0。追補 3 を 5 行に作り直したため試作の 17 件ではない） | なし |
| xss | 64 件の入り口別の件数。E5 の 49 件を配信設定の入力欄から CSP あり・なしで送った計数 | E1 は 5.175（TASK-144。CSP あり・なし）で済み。E2〜E4・E6 の画面の経路は TASK-145・146・147 |
| oversize | IR 20MB・履歴 10MB のちょうど・1 バイト超を `checkJsonLoad` へ。入力 20,000・20,001 文字 | 10 倍の水準は TASK-154（#855）・TASK-155 |
| nest | 10,000 段の入れ子と 1,000,000 要素の配列（IR・履歴の両方） | 同上 |
| cycle | 巡回する IR 4 本を関門へ | 画面での起動口は無い（D-68。実行時の教材の取得が入った後の課題）。遷移の上限は TASK-160（#444） |
| csp | 両ビルドの `index.html` の CSP の meta とスクリプトの数。CSP あり・なしのマウント後の計数 | 配置の検査は TASK-151（#538〜#540） |
| secret | 10 形を入力欄から送り、警告の要素の有無（0/10） | 警告の挙動は TASK-164（#541〜#544） |

実行の記録（2026-10-19、base 23a5245）: 単体で 7 本が pass、`make test-browser` 全体は 532 本が pass（終了コード 0、85 秒）。配信設定の E5 は送信の回数だけを assert する（配信設定は教材なしで、右のログの受付の行は 0 件増える。受付の行の観察は `log-lines-xss.test.mjs` の担当）。`FANDHE_WEB_TEST_DIR` を未設定にして流すと終了コード 1 になる（例外が 0 に化けない）。

限界: 各節は今のビルドで観察できる範囲に限る。10 倍の水準は CI のメモリと時間のため入れていない。Chrome の原本での実行は 5.128（BUILD-20・TASK-94）にある。

### 5.127 YAML 系 crate の見直し（TASK-73）

結論は、D-58（2026-10-12）と PACK-47 に従い、別の crate には移らないことである。`yaml_serde` 0.10.7 を維持し、版を固定する。`Cargo.lock` に変更は無い。代わりの crate は探していない。

固定は `crates/pack/tests/yaml_dependency.rs` の `pack47_` の 3 件で確かめる。ルートの `Cargo.toml` の宣言が exact 指定の `=0.10.7` であること（`"0.10.7"` は暗黙の caret 要件で `cargo update` により動く）、`Cargo.lock` の解決が 1 版・0.10.7・crates.io の registry であること、pack crate が版を上書きせず workspace の宣言を使うことである。版を上げるときは、試験の定数 `ADOPTED_YAML_SERDE` とこの節を同時に直す。

基準値（実行日 2026-10-19、元にしたコミット 23a5245。`cargo test --locked -p fandhe-workshop-pack` の passed の件数）。

| 系統 | 試験 | passed |
|---|---|---|
| PACK-3 | `format_b` の `pack3_` | 5 |
| PACK-4 | `format_b` の `pack4_` | 9 |
| PACK-6 | `compare` の `pack6_` | 7 |
| 参考 | `ir_format` の `ir2_golden_bytes_match` | 1 |
| 参考 | `yaml_features` の `pack5_` | 9 |
| 参考 | `radix_literals` の `pack5_` | 7 |

golden の IR 5 ファイル（`crates/pack/tests/data/golden/`）の SHA-256 は次のとおり（`min-3step-pack` と `min-3step-pack-b` は同一）。

```text
ch1-pack.ir.json           3c61f683e32266f6aff1c56e87d5eb030a07d53e74f1a3758fd981132c79077c
fixture-pack.ir.json       ac48968b0897222653067a9a73dd1d47c6723046a60c45b11d1f4cef893c54bc
ime-pack.ir.json           9a179699101fd13a74c8c1feb8a3b01c446be6e95402237b66394d6cd47d8929
min-3step-pack-b.ir.json   7f29c18a1bddb4e1720f0d68ce3efc9d3fe43c86e17c6fda6ec45cdd192bd9a4
min-3step-pack.ir.json     7f29c18a1bddb4e1720f0d68ce3efc9d3fe43c86e17c6fda6ec45cdd192bd9a4
```

2026-10-22 追記（PACK-19、#1708）: `fixture-pack` の途中の Step `a` に 1 択の `choice` と `confirm: none` を足したため、上の `fixture-pack.ir.json` は compile の出力と一致しなくなった。この凍結物は書き換えず、0fa7146 のエンジンで作り直した 2 本を `crates/pack/tests/data/ir_before_ep13/` に別名で足した。compile の出力との比較（`ir_format` の `ir2_golden_bytes_match`、`ext9_ir_identity`、`content_hash`）はこの 2 本を使う。旧い IR の実例としての読み込みの試験（`ir_compat`・`ir_old_engine`）は、従来の `golden/fixture-pack.ir.json` を読み続ける。

```text
fixture-pack-choice.ir.json    3a9fa6e83f5468c48e4fcb8709039f2dd6ba1ac5bb72e7a7b227a99ba02f639a
no-tree-pack-choice.ir.json    e721d6146b01c8914fc9b37c18ed40741c358f11c3efa14791e2f7a229f69314
```

`content_hash` の期待値 `FIXTURE_PACK_HASH` は `6cad339caadef18fe3747fefdd6432bbfa8bd646dcf94fbfd83d5b5ebc87116b` に求め直した。`fixture-pack-choice.ir.json` から本文の木（`body_doc`・`body_levels_doc`）を除き、compact の JSON にして Python の `hashlib.sha256` で独立に計算した。同じ手順で従来の golden が従来の期待値 `8837847d…` になることを先に確かめた。

懸念（コードと試験に残っている事実）。

- 所有者が 1 人で 2026-01 に公開された。継続は未確認（spec に記録された調査時点の情報）
- アンカー・エイリアス・マージキーを透過に展開し、タグを黙って解決する。対処として `saphyr-parser` で先に拒否している（`yaml_strict.rs`・`yaml_features.rs`）
- 10 進でない整数を黙って数に読む。対処として拒否している（`radix_literals.rs`）
- 複数文書のとき位置を返さない（`format_b.rs` の `pack4_expect_multiple_documents_are_rejected_without_position`）
- 重複キーの検出は別の走査で行っている

未確認の事項は、crates.io の現在の保守状況・所有者の数・取り下げの有無・既知の脆弱性である。本タスクでは調べていない。依存の棚卸し（BUILD-25、TASK-99 = #439）で確かめる。棚卸しは初回公開の前とリリースごとに行う。

移ることになった場合の手順（合格条件は PACK-47 に残っている）。

1. 入れ替える前に、上の件数を控える
2. 入れ替えた後、同じコマンドで同じ件数が通ることを確かめる
3. `ir2_golden_bytes_match` が通ることを確かめる。入れ替える前のビルドの `fandhe-pack compile` の標準出力と、入れ替えた後の出力を `cmp` でバイト比較する。形式 A／B の同値は `fandhe-pack compare` で見る。`compare` は今のビルドだけで 2 つを比べるため、前後の比較には使えない
4. `ADOPTED_YAML_SERDE` とこの節を直し、決定は TASK-18 に戻す

- PACK47-1: `Cargo.toml` の `yaml_serde` の宣言を exact 指定から暗黙の caret 要件（`"0.10.7"`）に緩める

```text
実行日: 2026-10-19
元にしたコミット: 23a5245（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/PACK47-1.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| PACK47-1 | Cargo.toml: yaml_serde の宣言を caret 要件に緩める | pack47_workspace_declares_adopted_yaml_serde_version | pack47_workspace_declares_adopted_yaml_serde_version | DETECTED |

### 5.128 Chrome の原本でのブラウザテスト（TASK-94）

BUILD-20 の原本（Google Chrome の最新の安定版）で、`make test-browser` を x86_64 の上で流し、全件が通ることを確かめた。それまでの証拠は Chromium の写し（BUILD-4・CI の `browser test (Chromium)`）だけだった。

起動の口は TASK-28 で実装済みである（`FANDHE_BROWSER_CHANNEL`。値は `chromium` か `chrome`）。仕様は「原本は `channel: "chrome"` を指定し、写しは 1 行を外す」と書くが、実装は既定が同梱の Chromium で、原本は環境変数で選ぶ。向きが逆である。起動の失敗はどちらの channel でも黙って Chromium へ切り替わらない（fail-closed）。そのため、`chrome` を指定して全件が通ったこと自体が、Chrome で流れた証拠になる。診断の行（`harness.test.mjs` の `browser: channel=... version=...`）が channel と版を残す。

実行の記録（2026-10-19、base 0ce9b62、作業ツリーに本変更）。

| 項目 | 内容 |
|---|---|
| ホスト | x86_64 の Linux |
| 実行の場所 | 使い捨てのコンテナ。イメージ `mcr.microsoft.com/playwright:v1.63.0-noble`（digest `sha256:eff16c30e6f3f4af0a03fa4b706120d5e9b0891c344a27d64559aff5900a4a27`）。ソースは読み取り専用で mount し、コンテナの中の写しでビルドした |
| 道具 | rustc 1.98.1、wasm-bindgen CLI 0.2.128（CI と同じ musl のプリビルトを SHA256 で照合）、Node v24.20.0、Playwright 1.63.0 |
| Chrome の導入 | `npx playwright install chrome` |
| 原本の Chrome | Google Chrome 155.0.8059.84 |
| 同梱の Chromium | 153.0.8010.12 |

| 実行 | コマンド | tests | pass | fail | 終了コード | 秒 |
|---|---|---|---|---|---|---|
| 原本（コンテナ） | `FANDHE_BROWSER_CHANNEL=chrome make test-browser` | 535 | 535 | 0 | 0 | 105 |
| 写し（同じコンテナ） | `make test-browser` | 535 | 535 | 0 | 0 | 86 |
| 写し（ホスト） | `make test-browser` | 535 | 535 | 0 | 0 | 125 |

件数は 5.126 の 532 本より 3 本多い。その後に足された試験と、本タスクで足した channel と版の診断の試験 1 本を含む（内訳は確かめていない）。原本と写しで件数は同じである。

対照: Chrome を導入する前のコンテナで `FANDHE_BROWSER_CHANNEL=chrome node --test tools/browser/harness.test.mjs` を流すと、11 本すべてが `Chromium distribution 'chrome' is not found` で失敗し、終了コードは 1 だった（Chromium へ切り替わらない）。`FANDHE_BROWSER_CHANNEL=firefox make test-browser` は終了コード 2 だった。

限界:

- 確かめたのはコンテナ（x86_64・Ubuntu noble）であり、CI の runner ではない
- Chrome の最新版は日々変わる。この記録は日付つきの 1 点にすぎない
- Chrome の `.deb` の SHA は固定していない（最新版を対象にする D-9 の性質による）。そのため CI には入れず、使い捨てのコンテナで流した
- macOS と Linux Arm64 では確かめていない。Linux Arm64 には Chrome の安定版が無い（PoC-11 F-2）

#### Chromium で流す扱い

Chrome の安定版が入らない環境（Linux Arm64 など）では、変数を付けずに既定の同梱 Chromium で流す。D-9 に従い、その結果は参考の確認とする。正は x86_64 の Chrome の最新版での記録とする。CI の関門は今の Chromium のままである。自動で Chromium へ切り替える仕組みは足していない。手順は README の「Chrome の原本と Chromium の扱い」に書いた。

#### 04 の BUILD-20 への反映案

`docs/spec/**` は編集しないため、spec 側で直す案をここに残す。仕様の文は転記せず、ID と項目名で指す（未承認）。

- 前提: 実装では、原本は `FANDHE_BROWSER_CHANNEL=chrome` で選び、既定は同梱の Chromium である。「9 本のスクリプトが `channel: "chrome"` を持つ」という試作の前提を、これに置き換える。許可しない値と Chrome が無い場合は、どちらも失敗になる
- 操作: x86_64 の上のコンテナで `FANDHE_BROWSER_CHANNEL=chrome make test-browser` を流す
- 期待: 原本が全件合格する。Chrome の安定版が無い環境では、変数なしの Chromium で流し、結果は参考の確認とする（D-9）
- 根拠・証拠の種類: 「なし」を、本節の記録（自動ブラウザ。Chrome の原本。x86_64 のコンテナ）に置き換える
- 状態と検討中の理由: 扱いの決定が採用されたら「確定」にする案。検討中の理由に挙げた案のうち「`channel` を環境変数で切り替える」を採った。「x86_64 の runner の CI で原本を流す」は人間の判断待ちとして残す

人間の判断が要る事項（未承認）。

1. Chromium で流す扱いの決定の採否
2. Chrome の原本を CI の関門にするか。する場合は、`ci.yml` にジョブを足し、`ci-complete` の `needs` と ruleset の必須チェックを同時に更新する。ruleset は実装者の権限の外にある。提案は、関門は Chromium のままにして、Playwright の版上げとリリースの前に原本を流し直して記録を更新すること
3. 原本を流し直す手段（スクリプトや `workflow_dispatch` のワークフロー）を足すか

範囲外: `tools/measure/browser.mjs` は channel を受けずに `chromium.launch()` を呼んでおり、測定は Chromium に固定されている。

### 5.129 教材パックと Markdown の大きさの上限（TASK-159 1/2）

`source::load` の事前走査が、教材パック全体（20,971,520 バイト）と Markdown 1 ファイル（524,288 バイト）の上限を、中身を読まずに判定することを確かめる（SAFE-22・D-25）。ちょうどの大きさは読め、1 バイト超過と 200MB・5MB は `E-LIMIT-PACK-SIZE`・`E-LIMIT-MD-SIZE` で 1 秒未満に拒否される。巨大なファイルは疎ファイルで作る。Markdown は、どの Step も参照しない `.md` を使い、本文の読み込み時の判定ではなく事前走査だけが捉える形にする。`cli` に `compile` の終了コードと標準出力の確認を 1 件足した。深さとシンボリックリンクは #442、3 コマンドの入口への適用は 5.151 節（TASK-410）にある。

- SAFE22-1: Markdown の判定を `>=` にする（ちょうど 512KB を断る）
- SAFE22-2: Markdown の事前走査の判定を外す
- SAFE22-3: パック全体の判定を `>=` にする（ちょうど 20MB を断る）
- SAFE22-4: パック全体の事前走査の判定を外す

```text
実行日: 2026-10-19
元にしたコミット: c4c73ef（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/SAFE22-1.mutation（SAFE22-2〜4 も同様）
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| SAFE22-1 | crates/pack/src/source.rs: Markdown の判定を >= にする（ちょうど 512KB を断る） | markdown_exactly_at_limit_loads_even_when_unreferenced | markdown_exactly_at_limit_loads_even_when_unreferenced | DETECTED |
| SAFE22-2 | crates/pack/src/source.rs: Markdown の事前走査の判定を外す | unreferenced_markdown_one_byte_over_is_rejected_by_prescan, markdown_5mb_is_rejected_quickly_without_reading | markdown_5mb_is_rejected_quickly_without_reading, unreferenced_markdown_one_byte_over_is_rejected_by_prescan | DETECTED |
| SAFE22-3 | crates/pack/src/source.rs: パック全体の判定を >= にする（ちょうど 20MB を断る） | pack_total_exactly_at_limit_loads | pack_total_exactly_at_limit_loads | DETECTED |
| SAFE22-4 | crates/pack/src/source.rs: パック全体の事前走査の判定を外す | pack_total_one_byte_over_is_rejected_without_reading, pack_total_200mb_is_rejected_quickly | pack_total_200mb_is_rejected_quickly, pack_total_one_byte_over_is_rejected_without_reading | DETECTED |

### 5.130 依存の棚卸し（TASK-99）

BUILD-25・D-58 の依存の棚卸しの手順を実装した。スクリプトは `tools/inventory-deps.mjs`（`make inventory-deps`）、手順と結果の記録は `docs/dependency-inventory.md` である。

回帰テストは `make test-inventory-deps`（`tools/inventory-deps.test.mjs`、node:test）。loopback の stub サーバに試料を置いて流し、実 crates.io には通信しない。

| 項目 | 内容 |
|---|---|
| 実行日・元にしたコミット | 2026-10-19、base 2761fba（作業ツリーに本変更） |
| コマンド | `make test-inventory-deps` |
| 結果 | tests 14、pass 14、fail 0 |
| 主張の内容 | 正常・取り下げ済み・team だけの所有者・直接/推移・index の 404・5xx・壊れた JSON・checksum の不一致・リダイレクト・タイムアウト・不正な crate 名と許可外の取得先・crates.io 以外の source・古い例外・Markdown のエスケープ・JSON の鍵・実 repo の `Cargo.lock` の件数 |

実際の棚卸しを 2026-10-19 に 1 回流した（`make inventory-deps`。終了コード 0）。66 件の registry の crate（直接 16 件）のうち、取り下げ済みは 0 件、所有者 1 人は 32 件（うち例外 1 件の `yaml_serde`）だった。全件の表と D-58 との机上の照らし合わせは `docs/dependency-inventory.md` に記録した。5.127 が未確認としていた `yaml_serde` の保守の状況・所有者の数・取り下げの有無は、同文書で確かめた（所有者は user 1 人、取り下げなし、crates.io 上の最終更新日は 2026-08-18）。

実装で計画から変えた点が 1 つある。所有者 1 人の crate を終了コード 1 の条件にする案だったが、実測で 66 件中 32 件が該当したため、記録と要約への表示に留め、終了コードは変えないことにした。

未確認・限界:

- 既知の脆弱性の走査（`cargo audit`・RustSec）は扱っていない
- `miniserve` 0.35.0 とその推移的な依存は、L2 の配布物が未導入のため対象外である
- CI では実際の棚卸しを流していない（通信が要り、数分かかる。必須チェックの変更は人間の判断）
- 所有者 1 人で直接の依存に当たる `fandhe-frontend-*` の 6 件を D-58 が個別に判断した記録は確かめていない
- 取得先（crates.io の API・sparse index）の応答は日々変わる。記録は日付つきの 1 点である

### 5.131 ディレクトリの深さとシンボリックリンクの上限（TASK-159 2/2）

`source::load` の事前走査が、ディレクトリの入れ子の上限とシンボリックリンクを、中身を読まずに判定することを確かめる（SAFE-43・D-44）。根を 0 段として 32 段は通し、33 段は `E-LIMIT-DEPTH` で拒否する。シンボリックリンクは辿らず `E-LIMIT-SYMLINK` で拒否する。拒否は 1 秒未満で終わり、`workshop.yaml` を壊しても `E-PARSE` は出ず、33 段目より下やリンクの先の `SECRET` は診断に現れない。リンクの種類は、パック外のディレクトリ・宛先なし・パックの根への循環・`chapters` 自体・`workshop.yaml`・巨大ファイルの 6 通りを試す。`cli` に `compile` の終了コードと標準出力の確認を 2 件（`i12`・`i13`）足した。ファイルへのリンクを本文として参照する場合は、既存の `compile.rs` の `neg_symlink_is_rejected_without_following` が扱う。パックの根自体がリンクの場合の扱いは spec に定めが無く、本節では扱わない。`validate`・`test` を含む 3 コマンドの入口への適用は 5.151 節（TASK-410）にある。

- SAFE43-1: 深さの判定を `<=` にする（33 段を通してしまう）
- SAFE43-2: 深さの判定を 1 段厳しくする（ちょうど 32 段を断る）
- SAFE43-3: シンボリックリンクの判定を外す
- SAFE43-4: リンクを辿って見る（`symlink_metadata` を `metadata` にする）

```text
実行日: 2026-10-19
元にしたコミット: 2e30fa1（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/SAFE43-1.mutation（SAFE43-2〜4 も同様）
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| SAFE43-1 | crates/pack/src/source.rs: 深さの判定を <= にする（33 段を通してしまう） | dir_33_levels_is_rejected_without_reading | dir_33_levels_is_rejected_without_reading | DETECTED |
| SAFE43-2 | crates/pack/src/source.rs: 深さの判定を 1 段厳しくする（ちょうど 32 段を断る） | dir_exactly_32_levels_loads | dir_33_levels_is_rejected_without_reading, dir_exactly_32_levels_loads | DETECTED |
| SAFE43-3 | crates/pack/src/source.rs: シンボリックリンクの判定を外す | dangling_symlink_is_rejected_as_symlink, dir_symlink_to_outside_is_rejected_without_following | symlink_named_workshop_yaml_is_rejected_before_parse, symlink_to_huge_file_is_rejected_quickly_without_reading, symlink_cycle_to_pack_root_is_rejected_quickly, dangling_symlink_is_rejected_as_symlink, chapters_dir_itself_as_symlink_is_rejected, dir_symlink_to_outside_is_rejected_without_following | DETECTED |
| SAFE43-4 | crates/pack/src/source.rs: リンクを辿って見る（symlink_metadata を metadata にする） | dangling_symlink_is_rejected_as_symlink, dir_symlink_to_outside_is_rejected_without_following | dangling_symlink_is_rejected_as_symlink, symlink_named_workshop_yaml_is_rejected_before_parse, dir_symlink_to_outside_is_rejected_without_following, chapters_dir_itself_as_symlink_is_rejected, symlink_to_huge_file_is_rejected_quickly_without_reading, symlink_cycle_to_pack_root_is_rejected_quickly | DETECTED |

### 5.132 版の違う履歴 15 件と期待の表（TASK-175 1/2）

版の違う履歴 15 件を、実施の前に固定した期待の表（`testdata/version-histories/expectations.json`）と突き合わせる（VER-2）。履歴は現行のエンジンで試験の中で作り、`load_history`（段 1〜8）で開く。結果の種類・理由コード・違いの軸が表と合うことを見て、食い違いを「期待より慎重な側」と「それ以外」に分けて数え、どちらも 0 件を求める。表の ID・種ごとの件数（1/2/2/2/4/3/1）・結果ごとの件数（1/6/5/3）は試験のコードにも持ち、表だけの書き換えを検出する。比較器の自己確認として、期待を 1 つ書き換えると食い違いが出ることも確かめる。写しの一致と `Session` を作らないことの確認は 5.134（#449。TASK-175 2/2）で行う。本節は確かめていない。

- VER2-1: 教材形式が古い履歴の判定を外す（V-iv-4 が適用し直しまで進み、危険な側の食い違いになる）
- VER2-2: エンジンの主版の違いを開かないに倒す（V-v-3 が慎重な側の食い違いになる）

```text
実行日: 2026-10-19
元にしたコミット: e6fde05（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/VER2-1.mutation（VER2-2 も同様）
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| VER2-1 | crates/core/src/history_open.rs: 教材形式が古い履歴の判定を外す（V-iv-4 が適用し直しまで進み、危険な側の食い違いになる） | fifteen_histories_match_the_fixed_expectations | fifteen_histories_match_the_fixed_expectations | DETECTED |
| VER2-2 | crates/core/src/history_open.rs: エンジンの主版の違いを開かないに倒す（V-v-3 が慎重な側の食い違いになる） | fifteen_histories_match_the_fixed_expectations | fifteen_histories_match_the_fixed_expectations | DETECTED |

### 5.133 遷移 1,000 回の上限（TASK-160 1/3）

1 回の操作で起きる内部の遷移（`next`・`use` の `enter`・`final` の `exit_to`）を `transit` モジュールがループでたどり、着地の回数が `MAX_TRANSITIONS_PER_INPUT`（1,000）を超える 1,001 回目で止めることを確かめる（SAFE-23・D-25）。止めたときは固定文の `Warning` を 1 件足し、`halt` が現在の Step・処理中・入口の写しを空にする（内側の fail-closed。後続の 5.147 で、`apply` が直後に操作の前の状態へ戻すようになった）。再帰を使わないので、スタックは溢れない。見出しとまとめ表示（SAFE-34、#445）は 5.135、止めた後の戻り方（SAFE-35、#446）は 5.147 で扱う。

試験は 2 層に置いた。

- native（`crates/core/tests/transition_limit.rs`）: 巡回する IR 4 本（`testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir/`。読み込みを通る）のうち、`cycle-enter-self`・`cycle-enter-2` は `Control::GoTo` で巡回の入口へ入り、`cycle-exit-2` は `main/f1` への自然文 1 回で、`ModuleEntered`／`ModuleExited` がちょうど 1,000 件・`Warning` が 1 件・現在の Step が空になる。`cycle-d07-flat` は `s-d07` の節点から `Input::Ui` を 2 回送り、上限の `Warning` が 0 件で `ModuleEntered` が少数に収まる。巡回しない `enter` の連鎖（入口 → `use` k 個 → 末尾。着地は k + 1 回）と `exit_to` の連鎖（c0 → c1 … ck → 末尾）は、k = 999 で通り（`Warning` 0 件）、k = 1,000 で止まる。256 KB のスタックのスレッドで巡回を流しても溢れない。連鎖の途中の `enter` の先が IR に無いときは、固定文の `Warning` が 1 件で panic しない
- ブラウザ（`tools/browser/transition-limit.test.mjs`）: `load-fixture:cycle-*`・`load-fixture:chain-1000`・`chain-1001`（試験の設定だけ。配信物には入らない）で教材を差し替えて同じ主張を確かめる。タブが落ちないこと（`crashed` が偽）とページ内のエラーが 0 件であることを、各場合で確かめる。上限で止めた警告は、エンジンの書き出しの `warnings`（固定文）から数える（`go_to` で飛んだ教材では効果の適用の警告が別に出るため、種類 `warning` の件数では数えない）

開始（`Control::Begin`）は未実装（TASK-217）なので、SAFE-23 の「開始する」の代わりに `go_to` で巡回の入口へ入った。開始時の連鎖は TASK-217 が同じ歩行器を通す。web の試験用の起動（`start_state_at`）は、開始の節点が `main` の外にあるとき、切り身の鍵を節点の根に合わせる（合わせないと開始の Step の状態を引けず、入力が受け付けられない）。

所要時間は assert せず、diagnostic に残した（2026-10-19、debug ビルドの native で 8 件合計 0.01 秒、ブラウザで巡回 1 本あたり約 40〜50 ミリ秒）。

- SAFE23-1: 上限の比較を `>=` にする（1,000 回ちょうどの着地が止まる）
- SAFE23-3: 上限を 1 つ緩める（1,001 回目が通る）

上限の判定そのものを外す変異は、検査が終わらなくなるため作っていない。

```text
実行日: 2026-10-19
元にしたコミット: 8098b2b（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAFE23-1.mutation（SAFE23-3 も同様）
```

### 5.134 版の違う履歴 15 件のブラウザでの確認（TASK-175 2/2）

版の違う履歴 15 件を実際のブラウザで開き、5.132 の期待の表（`testdata/version-histories/expectations.json`。Node が直接読む）と突き合わせる（VER-2）。履歴と開く側の教材は、native の試験と同じ作り方を `emit_histories_for_browser`（`#[ignore]`）が書き出す（`scripts/test-browser.sh` だけが `--ignored` で呼び、15 件がそろわなければ失敗にする）。開く側の教材・エンジンの版・語彙の上限は、試験の設定の口（`raw` の `load-ir:`・`opening-override:`）で作る。配信の設定の判定の経路は変えず、口は配信物に入らない。

確かめたこと（`tools/browser/version-histories.test.mjs`。CSP あり・なしの 2 通り）:

- 結果の種類・理由コード・違いの軸が表と合う。食い違いは「期待より慎重な側」と「それ以外」に分けて数え、どちらも 0 件
- 開く 7 件（変換なし 1・変換して開く 6）で、開いた直後の状態（`lastImportStateJson()`）が履歴ファイルの `snapshot` と完全に一致する（7/7）
- 開かない 8 件（表示だけ 5・開かない 3）で、実行環境の差し替えが 0 回（`installCount`）。状態・保存領域・URL も変わらない（8/8）
- 理由が空の例が 0 件（開かない 8 件は空でない文字列、開く 7 件は `null`）
- 外部への要求・CSP 違反・ページのエラー・dialog・popup が 0 件
- 比較器の自己確認（期待を 1 つ書き換えると食い違いが出て、慎重さで分類される）

集計（2026-10-19。CSP あり・なしとも同じ）: 慎重な側 0・それ以外 0・写しの一致 7/7・差し替え 0/8・変化なし 8/8・空の理由 0/8。

native の固定（`crates/web/tests/version_histories_host.rs`。feature `test-hooks`）は、上書きが無ければ配信と同じ判定になること、エンジンの上書きで V-v-1 が変換（軸 `engine`）・V-v-3 が表示だけ（`engine_major`）になること、語彙の上限の上書きで開かない（`vocab_newer`）になること、開かない履歴で差し替えの回数が増えないこと、読めない上書き・教材は何もしないことを固定する。ブラウザ試験は変異注入（`scripts/check-mutations.sh`。`cargo test` だけを流す）の対象外のため、変異はこの native の試験に掛ける。

- VER2-3: 開く側のエンジンの版の上書きを無視する（V-v-1・V-v-3 が期待と食い違う）
- VER2-4: 変換して開いたときの違いの軸を常に空で返す（軸の照合が失敗する）

```text
実行日: 2026-10-19
元にしたコミット: 770091e（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/VER2-3.mutation（VER2-4 も同様）
ブラウザ試験: make test-browser（539 件が通過）
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| SAFE23-1 | crates/core/src/transit.rs: 上限の比較を以上にする（1,000 回ちょうどの着地が止まる） | enter_chain_boundary, exit_chain_boundary | cycle_enter_self_stops_at_limit, cycle_exit_2_stops_at_limit, cycle_enter_2_stops_at_limit, enter_chain_boundary, exit_chain_boundary | DETECTED |
| SAFE23-3 | crates/core/src/transit.rs: 上限を 1 つ緩める（1,001 回目が通る） | enter_chain_boundary, exit_chain_boundary | cycle_enter_self_stops_at_limit, cycle_enter_2_stops_at_limit, cycle_exit_2_stops_at_limit, enter_chain_boundary, exit_chain_boundary | DETECTED |
| VER2-3 | crates/web/src/host.rs: 開く側のエンジンの版の上書きを無視する | an_engine_override_turns_the_judgment_into_a_conversion_with_the_engine_axis, a_major_engine_override_is_view_only_and_does_not_replace_the_session | an_engine_override_turns_the_judgment_into_a_conversion_with_the_engine_axis, a_major_engine_override_is_view_only_and_does_not_replace_the_session | DETECTED |
| VER2-4 | crates/web/src/host.rs: 変換して開いたときの違いの軸を常に空で返す | an_engine_override_turns_the_judgment_into_a_conversion_with_the_engine_axis | an_engine_override_turns_the_judgment_into_a_conversion_with_the_engine_axis | DETECTED |

範囲外: 実体ごとの状態・切り身とフラグの分離・遷移先の `on_enter` の効果・`ChapterCompleted`（TASK-233 以降。#564〜#566 と重なるため、`transit` を通して拡張する前提で調整が要る）、開始での連鎖（TASK-217）、戻り方（#446）。見出しと専用の `Event`（#445）は 5.135 で扱った。

### 5.135 遷移の上限で止めた理由の見出しとまとめ表示（TASK-160 2/3）

遷移 1,000 回の上限（5.133）で止めたとき、受講者に理由が見えるようにする（SAFE-34・D-45）。止める前は、作成者向けの `Warning` が残るだけで、web はそれを行にしないため、受講者の画面には何も出なかった。試作（PoC-7）では見出しが「教材の定義の誤り」になり、案内の欄が空で、同じ行が 1,000 行並んだ。

実装した 4 点は次のとおり。

1. 専用の見出しの行: 右のログに `data-kind="limit"` の行を 1 件出す。見出し（`line-heading`）は「実行時の上限で止めました」、本文（`line-body`）は上限の回数（`1,000`）を含み、教材の定義の誤りではなく実行時の上限であることを述べる
2. 案内の欄: ログの通知欄 `log-notice`（`role="status"`）に、理由の文を出す
3. 専用の `Event`: `Event::TransitionLimitReached { at, landings }`（`CONTRACT_VERSION` 6）。文字列の欄を持たない
4. まとめ表示: 個々の着地（`ModuleEntered`・`ModuleExited`・`Transitioned`）は行にせず、止めた 1 行にまとめる

決めたこと（決定 D-45 に文言と細目の記録が無いため、エンジンの側で仮に置いた。記録として残す）。

- `Warning` は残し、その直後に専用の `Event` を 1 件足す。置き換えない。SAFE-23（Must）と TASK-160 の完了条件が「理由が `Warning` として 1 件残る」と書いており、5.133 の試験もその前提だからである。`Warning` は作成者向けの診断として残り、受講者向けの表示は専用の `Event` だけから作る
- まとめ表示の読み: 現在の web では、`ModuleEntered` 等は `line_from_event` が `None` を返すため、DOM にはもともと 1,000 行並ばない。ここでは「止めた 1 行だけを出し、個々の着地は行にしない」ことを契約として定め、`tests/log_lines_contract.rs` で固定した。後続のタスクが着地を行にする場合は、まとめの規則を一緒に持ち込む必要がある
- 案内の欄は、領域「案内とログ」の通知欄 `log-notice` と読んだ。`a11y` 契約が予約済みで、どこにも描いていなかった。`input-notice` は入力の上限の案内（`paste-notice`・`input-limit`）が使っているため採らない
- 行の種類 `limit` と部品 `line-heading` を足し、`TERMINAL_AREA_CONTRACT_VERSION` を 2 から 3 にした（`LineKind` は閉じた一覧で、黙って変えられない）。`reloaded`（版 2）と同じく、`LIVE_EXCLUDED_LINE_KINDS` には入れない
- 文言は仮置きで、`log_lines`（`LIMIT_HEADING`・`LIMIT_BODY_*`）と `limit_notice`（`LIMIT_NOTICE_*`）の定数 1 か所に置いた。固定文と、着地の回数を 3 桁ごとに区切った数だけで作り、教材・受講者の値を含めない（SAFE-11）。仕様の側での確定は spec repo の作業になる
- 通知欄は、ログを末尾から読み、`Changed`・`Ignored`・`Warning`・`Reloaded` を読み飛ばして、最初に当たったイベントが `TransitionLimitReached` のときだけ文を出す（`run_status` と同じ狭い許可リスト）。止めた後は操作の前の Step へ戻り（5.147）、戻った Step で次の入力が受け付けられると消える
- 状態表示（`run-status`）は現行どおり「待機中」に倒れる。`RunStatus` は 3 値の閉じた契約で、上限の停止を「停止」にするかは本件の範囲外とした

試験は 3 層に置いた。

- native の core（`crates/core/tests/transition_limit.rs`）: 巡回 3 本と連鎖の境界で、止めたときに `TransitionLimitReached` がちょうど 1 件で、`Warning` の直後・イベント列の末尾にあり、`landings` が 1,000 であること。止まらない場合（`cycle-d07-flat`・1,000 回ちょうどの連鎖・連鎖の途中の `enter` の先が無い場合）は 0 件であること。`api_types.rs` は直列化の形と、余分なキーの拒否を確かめる。`projection_derive.rs` は 1 件につき 1 行になることを確かめる
- native の web（`crates/web/tests/log_lines_contract.rs`・`limit_notice.rs`・`terminal_area_contract.rs`）: 行の構造と固定の文言、数の区切り、1,000 件の着地と `Warning` から行になるのが上限の 1 行だけであること、`EngineHost` で巡回の IR を流したときの `limit` の行・通知欄（DOM と束縛値の一致、`role="status"`、子の要素なし、束縛点が dirty になる）、上限に達しない場合は行も通知も出ないこと
- ブラウザ（`tools/browser/transition-limit.test.mjs`）: 巡回 3 本と `chain-1001` で、`limit` の行がちょうど 1 件（見出し・本文の回数）、その操作で増えた右のログの行が 5 行以下、`log-notice` の文・`role`・子の要素 0 件、エンジンの書き出しの `limits`（`landings` が 1,000 の 1 件）を確かめる。止めた後の確認は 5.147 に置き換えた。`cycle-d07` と `chain-1000` では行 0 件・通知欄が空・`limits` が 0 件の対照を確かめる。5.133 の主張（警告 1 件・`module_*` が 1,000 件・crash 0・ページ内エラー 0・安全性の計数）は残した。文言の期待値は試験の冒頭の定数に置き、正は Rust 側の定数である

- SAFE34-1: 専用の `Event` の着地の回数を 0 にする
- SAFE34-2: 専用の `Event` を行にしない
- SAFE34-3: 通知欄の導出が常に空を返す
- SAFE34-4: 個々の着地（`ModuleEntered`）を行にする

あわせて、`SAFE23-2.mutation` の置換対象が `transit.rs` の 2 か所に一致して検査が止まっていたため、`halt` の行に説明のコメントを付けて一意にした（置換する内容は変えていない）。

```text
実行日: 2026-10-19
元にしたコミット: 606b235（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAFE34-1.mutation（SAFE34-2〜4・SAFE23-1〜3 も同様）
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| SAFE34-1 | crates/core/src/transit.rs: 専用の `Event` の着地の回数を 0 にする | cycle_enter_self_stops_at_limit, enter_chain_boundary | cycle_enter_2_stops_at_limit, cycle_exit_2_stops_at_limit, cycle_enter_self_stops_at_limit, enter_chain_boundary, exit_chain_boundary | DETECTED |
| SAFE34-2 | crates/web/src/log_lines.rs: 専用の `Event` を行にしない | 上限で止めた理由は_limit_行になり_見出しと本文が固定の文言で出る, 個々の着地と遷移は行にならず_まとめは上限の_1_行だけである | 同左 | DETECTED |
| SAFE34-3 | crates/web/src/limit_notice.rs: 通知欄の導出が常に空を返す | 巡回で上限に達すると_limit_行が1件と通知欄の理由が出て_着地は行にならない | 同左, 上限に達した操作で通知欄の束縛点が_dirty_になる | DETECTED |
| SAFE34-4 | crates/web/src/log_lines.rs: 個々の着地を行にする | 個々の着地と遷移は行にならず_まとめは上限の_1_行だけである | 同左 | DETECTED |
| SAFE23-1〜3 | crates/core/src/transit.rs（5.133 の表のとおり） | 5.133 のとおり | 5.133 のとおり | DETECTED（本変更の後に再実行して確認） |

範囲外: 止めた後の戻り方（SAFE-35、#446。5.147 で扱った）、開始での連鎖（TASK-217）、実体ごとの状態・`on_enter`（TASK-233 以降）、spec の `screen-terminal-area.md` の行の種類の閉じた一覧への `limit`・`line-heading` の追記と文言の確定（spec repo 側の作業）、上限の停止での状態表示（`run-status`）の扱い。

### 5.136 版の線引きの決定との照合（TASK-177 1/2）

結論: `open_history`（`crates/core/src/history_open.rs`）の判定は、D-52 の 6・7 と D-59 の 2 の決定と一致しており、判定の変更は無い。決定は試作と同じため、04（VER-2〜VER-7）の期待の更新も生じない。照合の表は 5.61c にあり、ここでは重複させない。本件は、決定で採らなかった案を表す変異を足して固定する。

| 決定の項目 | 固定している試験 | 変異 |
|---|---|---|
| D-52 の 6: 主版の違いは向き・位置を問わず表示だけ（採らなかった案 (b)「適用し直して一致すれば主版違いも変換」を退ける） | `ver6_content_major_with_removed_step_is_view_only_at_any_position`・`ver6_content_major_boundaries` | `VER6-1`〜`VER6-3` |
| D-52 の 6: 副版以下は変換して開く（採らなかった案 (a)「教材内容の版が違えば常に表示だけ」を退ける） | `ver4_*`・`ver5_step_added_by_minor_keeps_early_history_and_diverges_after` | `VER18-1`（本件で追加） |
| D-52 の 7・D-59 の 2: `ir_format` が古ければ表示だけ | `ver7_format_older_is_view_only` | `VER7-2` |
| D-59 の 2: `ir_format` が新しければ開かない（D-52 の文言どおりの「表示だけ」と読まない） | `ver7_format_newer_is_refused`・`history_ir_format_newer_than_opening_pack_is_refused` | `VER7-1`・`VER7-8`・`VER18-2`（本件で追加） |

D-59 の 4 のとおり、`format_newer` の向きの理由は Claude Code の判断で、ユーザーの理由としては記録されていない。

```text
実行日: 2026-10-19
元にしたコミット: 7528a9d（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/VER18-1.mutation crates/core/tests/mutations/VER18-2.mutation（退行確認として VER6-1〜VER6-3・VER7-1・VER7-2・VER7-8 も同時に実行）
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| VER18-1 | crates/core/src/history_open.rs: 内容の版が違えば常に表示だけにする | ver4_content_patch_only_is_converted | ver4_content_minor_with_changed_text_is_converted, ver4_content_patch_only_is_converted, ver4_converted_state_is_saved_with_the_opening_versions, replay_mismatch_is_separated_from_diverged, ver5_step_added_by_minor_keeps_early_history_and_diverges_after, ver7_format_older_is_view_only, ver6_content_major_boundaries, ver6_content_major_with_removed_step_is_view_only_at_any_position | DETECTED |
| VER18-2 | crates/core/src/history_open.rs: 新しい `ir_format` も表示だけにする | ver7_format_newer_is_refused | history_ir_format_newer_than_opening_pack_is_refused, refused_reasons_are_non_empty_and_classified, ver7_format_newer_is_refused, ver7_stage6_order_follows_history_load_order | DETECTED |
| VER6-1〜VER6-3・VER7-1・VER7-2・VER7-8 | 同左（退行確認） | 各変異の定義のとおり | 各変異の定義のとおり | 全件 DETECTED |

範囲外: 15 件の試験を決定どおりの期待で通す作業（TASK-177 2/2、#452）。

### 5.137 版の違う履歴 15 件の期待と決定の照合（TASK-177 2/2）

結論: 15 件の期待は D-52 の 6・7 と D-59 の 2 の決定どおりで、`expectations.json` の変更は無い。決定は試作と同じため、04（VER-2〜VER-7）の期待の更新も生じない（5.136 を参照）。本件は、15 行を 1 行ずつ決定の条項に結び付けた照合の記録と、決定で採らなかった読みで期待を書くと食い違うことの自己確認を足す。

| ID | 決定の条項 | 期待（結果 / コード / 軸） |
|---|---|---|
| V-0 | 版の違い無し。適用し直して一致 | open / - / - |
| V-i-1・V-i-2 | D-52 の 6: 副版以下の違いは適用し直して一致すれば変換 | converted / - / content, content_hash |
| V-ii-1 | D-52 の 6: 副版で、適用し直しが写しと一致しない | view_only / replay_diverged / - |
| V-ii-2 | D-52 の 6: 副版で、足した Step より手前のため一致 | converted / - / content, content_hash |
| V-iii-1・V-iii-2 | D-52 の 6: 主版の違いは位置を問わず表示だけ | view_only / content_major / - |
| V-iv-1 | 語彙が古い側は副版以下と同じく変換 | converted / - / vocab, content_hash |
| V-iv-2 | 語彙が新しければ開かない | refused / vocab_newer / - |
| V-iv-3 | D-59 の 2: `ir_format` が新しければ開かない | refused / format_newer / - |
| V-iv-4 | D-52 の 7・D-59 の 2: `ir_format` が古ければ表示だけ | view_only / format_older / - |
| V-v-1 | エンジンの副版の違いは変換 | converted / - / engine |
| V-v-2 | エンジンが新しければ開かない | refused / engine_newer / - |
| V-v-3 | エンジンの主版の違いは表示だけ | view_only / engine_major / - |
| V-ref | 参照元の違いは変換 | converted / - / refs |

自己確認の試験 `rejected_readings_of_the_decision_mismatch_the_fifteen`（`crates/core/tests/version_histories.rs`）は、表の行の写しの期待を不採用の読みに書き換え、実際の結果と食い違うことを確かめる。書き換える前の行は一致することも同じ試験の中で見る。

| 不採用の読み | 書き換えた行 | 食い違いの分類 |
|---|---|---|
| 案 (a)「教材内容の版が違えば常に表示だけ」 | V-i-1・V-i-2・V-ii-2 を view_only / content_major | それ以外（実際は開く） |
| 案 (b)「主版の違いでも、適用し直して一致すれば変換」 | V-iii-2 を converted | 慎重な側（実際は表示だけ） |
| D-52 の文言どおり「`ir_format` が違えば常に表示だけ」 | V-iv-3 を view_only / format_older | 慎重な側（実際は開かない） |

判定の側の不採用案は 5.136 の `VER18-1`・`VER18-2` が固定しており、本件では変異を足さない。表の JSON は同じ行が複数あり、変異の `find` が一意に当たらないためである。

```text
実行日: 2026-10-19
元にしたコミット: 97605c5（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/VER2-1.mutation crates/core/tests/mutations/VER2-2.mutation crates/core/tests/mutations/VER18-1.mutation crates/core/tests/mutations/VER18-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| VER2-1 | crates/core/src/history_open.rs: 教材形式が古い履歴の判定を外す | fifteen_histories_match_the_fixed_expectations | fifteen_histories_match_the_fixed_expectations | DETECTED |
| VER2-2 | crates/core/src/history_open.rs: エンジンの主版の違いを開かないに倒す | fifteen_histories_match_the_fixed_expectations | fifteen_histories_match_the_fixed_expectations | DETECTED |
| VER18-1 | crates/core/src/history_open.rs: 教材内容の版が違えば常に表示だけにする | ver4_content_patch_only_is_converted | 5.136 と同じ 8 件 | DETECTED |
| VER18-2 | crates/core/src/history_open.rs: 新しい `ir_format` も表示だけにする | ver7_format_newer_is_refused | 5.136 と同じ 4 件 | DETECTED |

範囲外: spec の VER-18 の「証拠の種類」は `ir_format` が古い履歴を「実装前で、確かめていない」としているが、実装は既にあり V-iv-4・`VER2-1`・`VER7-2` が固定している。spec 側で直す事項であり、本件では編集しない。

### 5.138 主版を上げる規約と `pack.vocab` の規約の机上の照合（TASK-179）

結論: VER-11 の完了条件 3 点が `docs/pack-authoring.md` の節に書かれた。主版の上げ忘れを警告する検査は D-52 により採らず、試験は足さない。

| 完了条件 | 節 | 根拠 |
|---|---|---|
| 主版を上げる条件 | 「主版を上げる条件（`pack.version`）」 | VER-11・D-52 |
| `pack.vocab` を書く規約 | 「`pack.vocab` の書き方」 | VER-11・VER-14・PACK-31 |
| 本文だけの修正が判定に現れない点 | 既存の 2 節と、上の節からの参照 | VER-12・D-51 |

文書の主張と実装の照合は次のとおり。

| 主張 | 照合先 |
|---|---|
| 主版の違いは位置によらず表示だけ（`content_major`） | `fifteen_histories_match_the_fixed_expectations`（V-iii-1・V-iii-2）・`ver6_*` |
| 副版で足した Step より先は `replay_diverged` | 同上（V-ii-1）・`ver5_*` |
| `pack.vocab` の省略は `1.0` で記録 | `t2_default_vocab_record`（`VocabVersion::DEFAULT`） |
| 読めない `pack.version` は記録を作らない | `t6_fail_closed` |
| 語彙の上限は 1.4、`validate` は上限超えを報告しない | `crates/pack/src/vocab.rs` の `LATEST`、`vocab_check.rs`（`declared < needed` だけを見る） |
| `validate` は `pack.version` の形を検査しない | `crates/pack/src` に該当の検査が無い（grep 0 件） |

```text
実行日: 2026-10-19
元にしたコミット: 6bfdef0（作業ツリーに本変更）
```

範囲外: spec の VER-11 の「作成者向けの文書に規約がまだ無い」の記述は、spec 側で直す。

### 5.139 旧新 IR の読み込みの互換（TASK-181 1/2）

結論: 04 の VER-13 の 3 つの場合を、D-51・D-52・D-59 の決定に合わせて native の試験で固定した（`crates/pack/tests/ir_old_engine.rs`）。旧いエンジンの実物はこの repo に無いため、試験の中に「版 1 だけを読み、確認の選択肢の `label` を知らない旧いエンジン」の模型を置いた。模型であり実物ではない。製品コードの変更は無い。

| 場合 | 決定の下での期待 | 断る仕組み | 試験 |
|---|---|---|---|
| 1. 旧い IR（木も `label` も無い）を新しいエンジンが読む | 変換せず `ir_format` 1 のまま読め、`answer` の確認も読める（`label` は無く、表示名は ID） | 加算だけ | `ver13_old_ir_answers_are_read_by_new_engine_without_label`（バイト一致は `ir_compat.rs`） |
| 2. `label` を持つ IR を旧いエンジンが読む | 断る。`label` は v1.3 の加算で `ir_format` は 1 のまま（PACK-24）のため、版ではなく型で断る。理由に `label` の値を含めない | 旧い `AnswerOption` の `deny_unknown_fields` | `ver13_label_stays_v1_and_new_engine_reads_it`・`ver13_label_ir_is_refused_by_old_engine_without_echo` |
| 3a. 本文の木を使う IR を旧いエンジンが読む | 木を使うと `ir_format` 2 になり（D-51・D-52）、版 1 だけを読む旧いエンジンは版の理由で断る（型の不一致にしない） | 版の先読み | `ver13_tree_ir_requires_v2_and_old_engine_refuses_by_version` |
| 3b. 木を使わない教材を旧いエンジンが読む | 最小の `ir_format`（1）のままで読める（D-52） | 加算だけ | `ver13_pack_without_trees_stays_v1_and_old_engine_reads_it` |
| 今後の欄 | 新しい欄は `ir_format` を上げる。`Value` で通す位置（`confirm`）の欄も、版の先読みが先に断る | 版の先読み | `ver13_future_field_inside_confirm_is_refused_by_version_gate` |

対照として、模型が版 1 の golden を読めること（`ver13_old_engine_model_reads_v1_goldens`）を確かめ、模型が正しい理由で断っていることの前提にした。

限界: 製品の `load_ir` は `confirm` を読み込み時に型付けしない。版 1 のまま `answer` の選択肢に未知の欄を足した IR は読み込みを通り、確認の判定時に不正な宣言として扱われる。`ver13_future_field_in_answer_option_is_refused_at_judging_not_loading` はこの現状の記録で、望ましい挙動の主張ではない。読み込み時に型付けするかは別に判断する（`crates/core/src/ir_load.rs`）。

#### 04 の VER-13 への反映案

【現状】の「本文の木だけを足した IR は、旧いエンジンでも読める」は、`Ir`・`Node` が未知の欄を許し `ir_format` が 1 のままだった試作の性質である。D-51・D-52 の下では「木を使う IR は `ir_format` 2 になり、版 1 だけを読む旧いエンジンは版の理由で断る。木を使わない教材は版 1 のままで読める」に置き換わる。証拠の種類には本件の自動テスト（native）を足す。旧いエンジンで実際に開く確認と、表示・保存済みの履歴の不変は TASK-181 の 2/2（#456）が残るため、検討中のままとする案である。spec の変更は spec repo 側で行い、本 repo では編集しない。

```text
実行日: 2026-10-19
元にしたコミット: 223308a（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/VER13-1.mutation crates/pack/tests/mutations/VER13-2.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| VER13-1 | crates/contract/src/ir.rs: 本文の木だけを使う IR の必要な版を 1 と計算する | ver13_tree_ir_requires_v2_and_old_engine_refuses_by_version | ver13_tree_ir_requires_v2_and_old_engine_refuses_by_version | DETECTED |
| VER13-2 | crates/contract/src/confirm_spec.rs: `AnswerOption` が未知の欄を受け入れる | ver13_future_field_in_answer_option_is_refused_at_judging_not_loading | ver13_future_field_in_answer_option_is_refused_at_judging_not_loading | DETECTED |

VER13-2 は `deny_unknown_fields` の属性の行が複数あり一意に当たらないため、番兵の欄を足して「未知の欄を受け入れる」状態を表した。

範囲外: 旧いエンジンで実際に開く確認と表示・履歴の不変（#456）、読み込み時の `confirm` の型付け。

### 5.140 旧いエンジンの断りの表示と履歴不変（TASK-181 2/2）

結論: 旧いエンジンが新しい教材（IR）を読めないとき、理由つきで断ることと、保存済みの履歴が書き換わらないことを、自動ブラウザ（`tools/browser/ir-refusal.test.mjs`、CSP あり・なし）と native（`crates/web/tests/ir_refusal_host.rs`）で固定した。5.139 と同じく、ブラウザで動く今のエンジンを「より新しい教材に対する旧いエンジン」とみなす。旧版のビルドの実物は repo に無い。

断られる IR は、既定の教材（`min-3step`）と同じ `pack.id` で作る（同じ教材の新しい版が旧いエンジンに届いた場面）。試験の設定の `?fixture=` の完全一致で選ぶ。

| 場面 | IR | 理由コード | 試験 |
|---|---|---|---|
| 版が新しい | `format_version`・`pack.ir_format` が 3 で、未知の欄を持つ（D-51） | `format_newer` | 画面の T1、native の `future_format_ir_is_refused_as_format_newer` |
| 版を上げずに欄だけが足された混在 | 版は 1 のまま、未知の欄を持つ（`Ir` は未知の欄を拒否する） | `malformed` | 画面の T2、native の `future_field_ir_is_refused_as_malformed` |

確かめたこと:

- 断られる画面は、通知 `ir-notice`（`role=status`）に「教材を読み込めませんでした。（コード）」だけを出す。入力欄・ログ・部品は描かない。未知の欄の値（番兵の文字列）は、画面・コンソール・写しの JSON のどこにも出ない（SAFE-11・SAFE-40）
- 先に既定の画面で 1 手を送って保存させ、保存先（localStorage）に履歴があることを確かめてから、断られる画面へ同じ文脈のまま移る。断っている間も、断った画面の再読み込み（pagehide）の後も、保存先の全体が 1 文字も変わらない。履歴の値は出力に出さず、一致かどうかだけを比べる
- 断った後に既定の画面へ戻ると、保存済みの履歴が `restored` として開ける（陽性の対照）
- 起動後に `load-ir:` で読めない IR を送っても、状態・ログの長さ・差し替えの回数（`installCount`）・URL・保存先が変わらず、写しの `irRefusal` だけが立つ
- 断る仕組み: 起動で断ったホストは session を持たず、`history_key` が `None` になる。保存先を開かず、`pagehide` の書き出しも付けない（native の `refused_host_has_no_history_key_and_saves_nothing`）

製品のコードの変更（最小）: `IrLoadError::reason_code`（core。固定の ASCII で ID・入力値を含めない）、`ir_notice`（通知文の仮置き）、`EngineHost::refused`・`ir_refusal`。配信する設定の `for_mount` は `refused` を呼ばず、IR の取得は従来どおり後続のタスクの担当で、配信物の振る舞いは変えていない。

限界:

- 模型であり実物ではない。実在する旧版のビルドで開く確認は TASK-183、版つき URL は TASK-125・TASK-178 の担当である
- 旧いエンジンと新しい教材が混ざる場面（キャッシュされた旧いエンジンの配信物と新しい教材）は、キャッシュや版つき URL ではなく、試験用のクエリで模している
- `label` を持つ IR を旧いエンジンが断る場合は、今のエンジンが `label` を知っているためブラウザでは再現できない（5.139 の限界と同じく、`confirm` の型の確定は判定の時点である）。本文の木だけを足した IR も、版 3 に相当する場面が無いため扱わない（5.139 に記録）
- 文言とコードは仮置きである。`format_unsupported`・`format_inconsistent` は【推論】による名前で、最終の名前と受講者向けの文面・画面の枠は TASK-545（SHELL-31）・D-57、通知の口の契約は TASK-511 が決める

#### 04 の VER-13 への反映案

証拠の種類に、本件の自動ブラウザ（理由の表示と、保存済みの履歴が書き換わらないこと）を足す案である。「旧いエンジンで実際に開く確認」は TASK-183 が残るため、状態は検討中のままとする。spec の変更は spec repo 側で行い、本 repo では編集しない。

```text
実行日: 2026-10-19
元にしたコミット: 01ac7b0（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/VER13-3.mutation crates/web/tests/mutations/VER13-4.mutation crates/web/tests/mutations/VER13-5.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| VER13-3 | crates/core/src/ir_load.rs: 新しい版を `format_newer` ではなく `format_unsupported` に写す | future_format_ir_is_refused_as_format_newer, refused_host_shows_the_notice_with_the_code_only | 同左（加えて refused_ir_at_runtime_keeps_the_session） | DETECTED |
| VER13-4 | crates/web/src/host.rs: 起動で断ったホストが理由コードを控えない | refused_host_shows_the_notice_with_the_code_only | 同左 | DETECTED |
| VER13-5 | crates/web/src/host.rs: 起動後に断った IR の理由コードを控えない | refused_ir_at_runtime_keeps_the_session | 同左 | DETECTED |

集計: 3 件すべて DETECTED。native は `ir_refusal_host` が 6 件、ブラウザは 8 件（2 つの IR と対照と起動後の拒否を、CSP あり・なしで流す）で通った（2026-10-19）。

範囲外: 実物の旧版での確認（TASK-183）、製品の「教材 IR を読めないときの画面」（TASK-545）、通知の口の契約（TASK-511）。

### 5.141 履歴が追記だけで増える性質（TASK-188 1/2）

結論: TASK-186 の決定 D-53（2026-10-12）に合わせ、今の `ops` と写しの列の欄が追記だけで増えることを性質テスト（`crates/core/tests/history_append_only.rs`）で固定した。同期の試験は作っていない（D-5）。履歴の形式の文書は `docs/history-format.md`（TASK-188 2/2）に書いた。

性質の定義（前後 2 つの包み prev → next）:

| 番号 | 内容 |
|---|---|
| 1 | `format`・`history_format`・`pack_id`・`start`・`versions`（4 軸）が等しい |
| 2 | `ops` は、n を prev の件数として、next の件数が n 以上、先頭の n - 1 件が等しく、n 件目は等しいか両方が `Draft` |
| 3 | prev の `snapshot.log` が next の接頭辞になっている |
| 4 | prev の各チェックポイントが next に同じ値で残り、`next_checkpoint` が減らない |

確かめたこと:

- 記録 1 段（下書き・操作・再読み込み）ごとの包みを直前の包みと比べる。ケースごとに引いた時点 t の包みと最後の包みも比べ、書き出し→読み込みを通した包みで一致と性質を確かめる
- 時点 t の包みを書き出して `boot_session` で復元し、起動直後の `envelope_for_save()`（`Reload` と `Reloaded` が足される）と、その後の続きの記録の各包みが、時点 t の包みに対して性質を満たす
- 固定の列で、下書きの置き換え・送信の後の下書き（畳まれない）・送信・エンジンの操作・再読み込み・保存・ログが伸びた後の復帰を各 1 回以上起こす
- 検査器の自己確認: 1 か所ずつ壊した 15 通りの組が違反になり、末尾の下書きだけを別の下書きに変えた組と件が増えた組は違反にならない。違反の文言に下書き・送信の本文を含めない（A09）

判断: 今の記録規則は連続する下書きを最後の 1 件に畳むため、`ops` を「末尾の未確定の `Draft` 以外を書き換えない列」と定義した。記録規則は変えていない（安全側）。写しの全体は単調ではないため、単調と主張するのは `log`・`checkpoints`・`next_checkpoint` と頭の欄だけである。

#### 04 の EXT-5 への反映案

「今の `ops` を追記だけで増える列として保証する」の証拠に、本件の性質テストを足す案である。保証の読みは「末尾の未確定の `Draft` だけが置き換わる」を明記する。spec の変更は spec repo 側で行い、本 repo では編集しない。

```text
実行日: 2026-10-19
元にしたコミット: 3de72fb（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/EXT5-1.mutation crates/core/tests/mutations/EXT5-2.mutation crates/core/tests/mutations/EXT5-3.mutation crates/core/tests/mutations/EXT5-4.mutation
```

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| EXT5-1 | crates/core/src/ops_record.rs: 確定済みの Submit を下書きで書き換える | scripted_sequence_covers_each_kind_of_growth, property_append_only_fixed, property_append_only_guided | 同左（加えて検査器の自己確認、再読み込み・起動をまたぐ 4 本） | DETECTED |
| EXT5-2 | crates/core/src/ops_record.rs: 再読み込みの記録を先頭へ差し込む | property_append_only_with_reload_fixed, property_append_only_with_reload_guided, property_append_only_across_boot_fixed | 同左（加えて固定の列、自己確認、誘導つきの起動） | DETECTED |
| EXT5-3 | crates/core/src/checkpoint.rs: 復帰で log を切る | scripted_sequence_covers_each_kind_of_growth | 同左（加えて自己確認と性質テスト 6 本） | DETECTED |
| EXT5-4 | crates/core/src/checkpoint.rs: 保存のたびに同じ ID へ書く | scripted_sequence_covers_each_kind_of_growth | 同左（加えて自己確認と性質テスト 6 本） | DETECTED |

集計: 4 件すべて DETECTED（2026-10-19）。

限界:

- 同期の試験は 0 件である（D-5。初回は同期を実装しない）
- `run`・`entry`・`drafts`・`hints_opened` は対象外である。世代の単調な増加は既存の P 系の性質の範囲である
- 起動後の続きは、進行の宛先の履歴（`seen`・`last_tick`）を引き継がずに流す

### 5.142 旧い教材と旧い履歴が新しい版で開く試験（TASK-190 1/2）

結論: EXT-9（D-52・D-53）の「開く」の側を、凍結した成果物で固定した（`crates/pack/tests/old_pack_history_open.rs`）。EP-13（語彙 1.4）を含む今のエンジンで、旧形式の凍結物である版 1 の IR 4 本が読め、凍結した履歴が変換なしで開く。受け口ごとの版は持たない（D-53）ことも、履歴の `versions` の欄の集合で固定した。新しい項目を使わない教材の IR のバイト一致は #463（TASK-190 2/2）が受け持つ。

「新しい版のエンジン」は、EP-13 を含む今のエンジンである。凍結物は今のエンジン（コミット 6fc670a）で生成した旧形式の試料であり、EP-13 の追加前のエンジンが書いた成果物ではない（履歴の語彙 1.0 は教材の宣言に由来し、旧エンジンの証拠にならない）。証明する範囲は、旧形式の履歴・IR を今のエンジンが開くことまでである。次の受け口の追加は、既存の試験用の口（`OpeningCaps::with_vocab_max`・`record_versions_with`）で模す。旧履歴の凍結物の作り方は `crates/pack/tests/data/old_histories_1/README.md`。

| 試験 | 内容 | 結果 |
|---|---|---|
| `ext9_premise_engine_is_newer_than_frozen_artifacts` | 凍結物の語彙は今の上限より古く、版 1 の IR は語彙の宣言が無い | 通る |
| `ext9_frozen_v1_irs_open_as_sessions` | 版 1 の golden 4 本が読め、開始点から実行環境を作れる | 通る |
| `ext9_frozen_history_opens_with_its_v1_ir` | 凍結した履歴が `Open`（写しと一致）。起動では `Restored` で再読み込みが 1 件足される | 通る |
| `ext9_frozen_history_against_recompiled_v2_ir_is_view_only` | 今 compile し直した版 2 の IR では `ViewOnly(format_older)`（D-52 の既知の境界） | 通る |
| `ext9_frozen_history_opens_when_pack_redeclares_newer_vocab` | 教材が語彙 1.4 を宣言し直すと `Converted`（`vocab` の軸を含み、`ir_format`・`engine` は含まない） | 通る |
| `ext9_frozen_history_opens_in_a_next_engine_with_one_more_extension_point` | 語彙の上限 1.5・エンジンの副版 +1 を模すと `Converted`（違いは `engine` だけ） | 通る |
| `ext9_contract_empty_history_sample_opens_in_current_engine` | 契約の試料 `empty` が `Open`、起動では `Fresh` | 通る |
| `ext9_stale_contract_samples_are_closed_for_a_non_version_reason` | `two-rounds`・`draft-rejected` は `replay_mismatch`（段 8）で開かない。版の理由ではない | 通る |
| `ext9_history_versions_have_no_per_extension_point_version` | `versions` の鍵は 6 つだけ。受け口ごとの欄を足した包みは `malformed` で断られ、理由に欄の名前を含めない | 通る |

限界:

- 旧いエンジンの実物（過去の版のビルド）は repo に無い。次の受け口の追加は模擬である
- 版 1 の履歴は、今 compile し直した版 2 の IR では続きから再開できず表示だけになる（D-52 は変換の規則を持たない）。教材の配信が版 2 に替わると起きるため、04 の EXT-9 の注意点として扱う
- `two-rounds`・`draft-rejected` の写しは、再読み込みの意味の実装前に作られ、受け口とは別の理由で開かない。写しの更新は契約の試料の版上げに当たり、本件の範囲外
- 試作の写しの形の `testdata/save-restore-versions/**/h-*.json` は今の型で読めず、対象外

#### 04 の EXT-9 への反映案

証拠の種類に本件の試験（旧い教材と旧い履歴が、受け口を足した後のエンジンで開くこと）を足し、「検討中の理由」のうち受け口を足した後に旧い教材と履歴を読む自動テストが無い点を解く案である。バイト一致は #463 が揃うまで残す。上の限界（版 1 の履歴と版 2 の IR）は注意点として書く。spec の変更は spec repo 側で行い、本 repo では編集しない。

```text
実行日: 2026-10-19
元にしたコミット: 6fc670a（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/EXT9-1.mutation crates/pack/tests/mutations/EXT9-2.mutation
```

| ID | 対象 | 期待した失敗 | 結果 |
|---|---|---|---|
| EXT9-1 | crates/core/src/history_open.rs: 履歴の語彙が上限より古いときに断る向きの取り違え | ext9_frozen_history_opens_with_its_v1_ir | DETECTED |
| EXT9-2 | crates/contract/src/ir.rs: 版 1 の IR を読まない | ext9_frozen_v1_irs_open_as_sessions | DETECTED |

集計: 2 件すべて DETECTED（2026-10-19）。

### 5.143 新しい項目を使わない教材パックの IR の同一性の試験（TASK-190 2/2）

結論: EXT-9（D-52・D-53）の「同一」の側を、追加の前のエンジンの成果物で固定した（`crates/pack/tests/ext9_ir_identity.rs`）。新しい項目（EP-13 の `views.<ID>.body`）を使わない教材 13 件の IR が、EP-13 の追加の前後でバイト単位で同じである。受け口ごとの版は持たない（D-53）ことも、IR の版の鍵の集合と、欄を足すと断られることで固定した。

「追加の前」は、EP-13 の追加コミット（ad4629c）の親 0fa7146 のエンジンである。`tests/data/golden/*.ir.json` の 5 本は、最後にそのコミットで書かれた（以後作り直していない）。golden の無い 7 教材と、本文を持たない最小パックの 1 件は、そのコミットのエンジンで出力して `tests/data/ir_before_ep13/` に凍結した（作り方は同ディレクトリの README。実行日 2026-10-19）。同じ旧いエンジンで golden の 5 本がバイト一致することも、凍結時に確かめた。

| 試験 | 内容 | 結果 |
|---|---|---|
| `ext9_premise_frozen_set_does_not_use_new_items` | 対象の全教材で、どのビューにも `body` が無く、`pack.vocab` が 1.4 未満または無い | 通る |
| `ext9_golden_packs_are_byte_identical_across_ep13` | golden の 5 教材を今のエンジンで出力し、golden とバイト一致 | 通る |
| `ext9_other_packs_are_byte_identical_across_ep13` | golden の無い 7 教材を出力し、`ir_before_ep13/` とバイト一致 | 通る |
| `ext9_pack_without_trees_stays_v1_and_identical_across_ep13` | 本文も練習データも持たない教材は版 1 のままで、凍結物とバイト一致 | 通る |
| `ext9_using_the_new_item_changes_only_its_own_key`（対照） | `document-body-pack` から `body` を省くと、違うのは `/views/v-doc/body` だけで、版は同じ | 通る |
| `ext9_ir_has_no_per_extension_point_version` | 版を表す鍵は `format_version`・`pack.ir_format`・`pack.version`・`pack.vocab` だけ。受け口ごとの版の欄は、教材の宣言では `E-PARSE`、IR の読み込みでは `Json` で断られ、値も鍵も写さない | 通る |

限界:

- 受け口の追加の実例は EP-13 の 1 件だけで、次の受け口の追加は試験できない。受け口が増えたら、凍結物の世代を足す運用は未決である
- golden の出所（0fa7146 の出力であること）はコミットの履歴に頼る。試験は履歴を見ない
- 対象は `tests/data` と `testdata` の教材 13 件である。`testdata/` の他の教材は含めていない

#### 04 の EXT-9 への反映案

証拠の種類に本件の試験を足す。5.142（1/2）と合わせて、「検討中の理由」のうち受け口の追加についての自動テストが無い点が、開く側・同一の側の両面で解ける案である。spec の変更は spec repo 側で行い、本 repo では編集しない。

```text
実行日: 2026-10-19
元にしたコミット: 1097f1f（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/EXT9-3.mutation crates/pack/tests/mutations/EXT9-4.mutation
```

| ID | 対象 | 期待した失敗 | 結果 |
|---|---|---|---|
| EXT9-3 | crates/contract/src/ir.rs: 受け口の欄の既定値が使わない教材の出力に漏れる | ext9_golden_packs_are_byte_identical_across_ep13 | DETECTED |
| EXT9-4 | crates/contract/src/ir.rs: 使わない教材の版まで上がる | ext9_pack_without_trees_stays_v1_and_identical_across_ep13 | DETECTED |

集計: 2 件すべて DETECTED（2026-10-19）。

### 5.144 CLI のコマンド一覧・引数・終了コードの契約（TASK-399 1/2）

`fandhe-pack` の外部仕様（`docs/cli.md` 版 1。コード側は `crates/pack/src/cli_contract.rs`）の適合試験 `crates/pack/tests/cli_contract.rs`（実行ファイルの挙動）と `cli_contract_doc.rs`（文書とコードの一致）に対する変異注入である。

AUTHOR1-1 はコマンドの一覧から `metrics` を落とし、AUTHOR1-2 は使い方の誤りの終了コードを 1 にし、AUTHOR1-3 は `compile` の引数の形から `-o` を落とす。

変異注入は、文書を一時ディレクトリへ写さない。そのため文書を読む `cli_contract_doc.rs` は変異の対象に含めず、手元で表・使い方のブロックを 1 文字ずらして失敗することを確かめる運用とする。

```text
実行日: 2026-10-19
元にしたコミット: fe74c4e（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/AUTHOR1-1.mutation crates/pack/tests/mutations/AUTHOR1-2.mutation crates/pack/tests/mutations/AUTHOR1-3.mutation
```

| ID | 対象 | 期待した失敗 | 結果 |
|---|---|---|---|
| AUTHOR1-1 | crates/pack/src/cli_contract.rs: コマンドの一覧から metrics が落ちる | commands_are_the_six_in_contract_order | DETECTED |
| AUTHOR1-2 | crates/pack/src/cli_contract.rs: 使い方の誤りの終了コードが診断ありの 1 と同じになる | author1_no_args_prints_usage_and_exits_2, exit_codes_are_fixed_values | DETECTED |
| AUTHOR1-3 | crates/pack/src/cli_contract.rs: compile の引数の形から -o が落ちる | usage_declares_compile_output_option | DETECTED |

集計: 3 件すべて DETECTED（2026-10-19）。

### 5.145 診断 1 行・項目・最終行・出し先の契約（TASK-399 2/2）

`docs/cli.md` 版 2 の 6〜9 節（コード側は `crates/pack/src/cli_contract.rs` と `crates/pack/src/diag.rs`）の適合試験 `crates/pack/tests/cli_contract.rs`（実行ファイルと型）に対する変異注入である。

AUTHOR2-1 は空の項目を拒否する検査を外し、AUTHOR2-2 は診断の行から項目の欄を落とし、AUTHOR2-3 は読み込めないときの最終行を `OK:` にし、AUTHOR2-4 はファイル・項目の欄の `: ` を直さなくする。

文書を読む `cli_contract_doc.rs` は、5.144 と同じく変異の対象に含めない。手元で 8 節の例の 1 文字をずらして失敗することを確かめる運用とする。

```text
実行日: 2026-10-19
元にしたコミット: df2cde5（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/AUTHOR2-1.mutation crates/pack/tests/mutations/AUTHOR2-2.mutation crates/pack/tests/mutations/AUTHOR2-3.mutation crates/pack/tests/mutations/AUTHOR2-4.mutation
```

| ID | 対象 | 期待した失敗 | 結果 |
|---|---|---|---|
| AUTHOR2-1 | crates/pack/src/diag.rs: 項目が空の診断を作れてしまう | diagnostic_requires_file_and_item | DETECTED |
| AUTHOR2-2 | crates/pack/src/diag.rs: 診断の行から項目の欄が落ちる | author2_diagnostic_lines_have_four_fields | DETECTED |
| AUTHOR2-3 | crates/pack/src/cli_contract.rs: 読み込めないときの最終行が OK: になる | author3_unreadable_final_line | DETECTED |
| AUTHOR2-4 | crates/pack/src/diag.rs: ファイル・項目の欄の区切りの `: ` を直さなくなる | diagnostic_separator_in_fields_is_escaped | DETECTED |

集計: 4 件すべて DETECTED（2026-10-19）。

### 5.146 引数の形の検査とコマンドの振り分け（TASK-401）

`docs/cli.md` 版 2 の 3 節の引数の形の検査と、6 つのコマンドの振り分け（コード側は `crates/pack/src/cli.rs`）に対する変異注入である。試験は `crates/pack/tests/cli_contract.rs`（形の誤りが読み込みの前に 2 で止まること）と `crates/pack/tests/cli.rs`（形が正しい呼び出しが各コマンドの handler に届くこと）にある。

AUTHOR1-4 は 1 引数のコマンドの余分な引数の検査を外し、AUTHOR1-5 は `vocab` の振り分けを `metrics` の handler に差し替え、AUTHOR1-6 は `-` で始まる引数を位置引数として拒否する判定を外す。

```text
実行日: 2026-10-19
元にしたコミット: 2f8070b（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/AUTHOR1-4.mutation crates/pack/tests/mutations/AUTHOR1-5.mutation crates/pack/tests/mutations/AUTHOR1-6.mutation
```

| ID | 対象 | 期待した失敗 | 結果 |
|---|---|---|---|
| AUTHOR1-4 | crates/pack/src/cli.rs: 1 引数のコマンドの余分な引数の検査が外れる | argument_shape_errors_are_reported_before_loading | DETECTED |
| AUTHOR1-5 | crates/pack/src/cli.rs: vocab の振り分けが metrics の handler になる | i10_each_command_is_dispatched_to_its_handler | DETECTED |
| AUTHOR1-6 | crates/pack/src/cli.rs: - で始まる引数を位置引数として拒否する判定が外れる | argument_shape_errors_are_reported_before_loading | DETECTED |

集計: 3 件すべて DETECTED（2026-10-19）。

### 5.147 上限で止めた後の戻り方（TASK-160 3/3）

遷移 1,000 回の上限（5.133）で止めた後、受講者が先へ進めるようにする（SAFE-35・D-45「止めた後は直前の Step に戻す」）。それまでは `halt` が現在の Step を空にしたため、以降の入力はすべて無視され、受講者は止まったまま残った。

決めたこと（D-45 に細目の記録が無いため、自動運転でエンジンの側が安全側に倒して置いた。spec 側の確定は別の作業）。

- 「直前の Step」は、上限に当たった操作を受ける前に受講者がいた Step と読む。巡回の途中の最後の着地点へ戻すと、同じ巡回にすぐ入り直すだけになるため採らない
- 戻る契機は「止めたのと同じ操作の中で、エンジンが戻す」。明示の「戻る」操作（新しい `Control`）は足さない。web には受講者が `GoTo`・`Restore` を出す UI が無く、明示の操作にすると本番の受講者は戻れず、`CONTRACT_VERSION` の版上げと画面の部品（spec 先行）も要るため
- 戻す単位は、操作の前の `run` 全体と状態の入口の写しの欄（`SessionState.entry`）。`current` だけを戻すと、止めた操作が達成にした Step が `Achieved` のまま残り、次の入力が `Idle` になって受け付けられない（`cycle-exit-2`）。世界状態も戻るので、止めた操作の `Changed` は 0 件になる
- 戻した後も変える欄は 2 つ。`generation` は操作の後の値を残す（減らさない。`go_to` の `Moved.new_generation` と一致させる）。`busy` は必ず空にする（進行の経路で、同じ `TickAddr` が再び有効になり、止める・戻すを無限に繰り返すのを防ぐ fail-closed）
- 操作の前に現在の Step が無かったときは戻す先が無いので、`halt` の空のまま止める。`enter` の先が IR に無い場合（`UNKNOWN_TARGET`）は上限ではないので戻さない
- 実装は `session::apply` が操作の前の `run`・`entry` を控え、イベント列に `TransitionLimitReached` があれば `transit::return_after_limit` で戻す。控える費用は、操作 1 件につき `world` 以外の分の複製が増えるだけ
- 新しいイベントは足さず、`CONTRACT_VERSION` は 6 のまま。通知欄の文言は「実行を止めて直前の Step に戻しました」に変えた（仮置き）。通知は戻った Step での次の入力まで残る
- 止めた操作が出した `Accepted`・`Achieved` などはログに残る（ログは追記だけ）。復習は `Projection` が、操作の先頭（`InputReceived`・`Progress`・`Moved`）で控えた復習と印の元へ `TransitionLimitReached` で戻して、Step の状態と揃える。ログ行は消さない
- `Projection` の控えは、復習と印の元の写しではなく取り消し差分（undo log）の長さで持つ。復習と印の元を変える箇所は、変える前の値を差分へ 1 件ずつ積み、上限で止めたときは控えた長さまで差分を逆に当てる。控えの取得は O(1)、変更 1 件の費用はその変更で変わる値の大きさ、戻す費用は控えの後の差分の数に比例し、操作ごとに復習全体・印の元全体を複製しない（Step を多く通る履歴でも累積の複製が 2 乗にならない）。差分は生きている控えのうち最も古いものより前を捨てる。保存の時点の復習は main と同じく現在の復習と `Arc` を共有する（保存の後の最初の変更で 1 回だけ複製する）
- 進行（処理中）の途中から止めた場合は、操作の前（受付済み・処理中）ではなく入口の写し（処理を始めた受付の直前の状態）へ戻す。受付済みのまま処理中だけを消すと、入力も進行も受け付けられず、確認 none の Step が達成に進めないため。復習と印は、処理中（進行の途中）から止めたとき、進行でも移動でも、実際に処理を始めた受付（`Accepted`）の時点へ戻す。処理中に拒否された再送（`InputReceived` と `Judged` だけ）では入口の控えを更新しない
- 入口の写しは状態に書かない。受付のたびに `SessionState.entry` へ写すと、処理中の履歴の写し（`snapshot.entry`）が null でなくなり、#1606 で凍結した `min-3step-v1-progress.json` が、版が同じなのに適用し直しの照合で開けなくなる（VER-2・VER-3。`first_difference` は `entry` も比べる）。`SessionState.entry` の埋め方は main と同じ（通常の操作では空。写しを持つチェックポイントへの復帰でだけ入る）にし、履歴の形式・版・凍結物は変えない
- 写しが要る場面（処理中の停止と、処理中の上限停止の後の戻り）でだけ、`entry_derive` が操作の前のログから受付の時点を導く。状態に現在の Step の写しがあればそれを優先する。起点はいまの `run` とログの末尾で、末尾から 1 件ずつ前へ見る。`Changed` は変更前の値を控え（`apply` が操作 1 件・経路ごとに 1 件、変更前は操作前の値で記録するので、新しい方から当てると受付の直前の世界状態に一意に戻る）、処理中に起きても実行状態を変えない出来事（`Progress`・`Flag`・`Warning`・`Ignored`・拒否された再送の `InputReceived`・`Judged`、`Refused`・`CheckpointSaved`・`Reloaded`）は読み飛ばす。`Restored` に当たったら、そのチェックポイントの `run` と保存の時点（`log_len`）へ起点を移す（チェックポイントの保存と復帰は写しを導かず、この移動で処理中の保存へ復帰した後も受付の時点へ戻れる）。現在の Step の `Accepted` に当たったら、控えた変更前の値を当て、Step を `Entered`・`busy` を空にした `run` を写しとする
- 受付の時点を一意に決められないとき（ログの先頭まで受付が無い、処理中の区間では起きない出来事（Step への入り・移動・停止・達成・上限での停止・別の Step の受付・未知のもの）に当たる、`Restored` の写しが無い・`log_len` が食い違う・前を指さない・同じ Step の処理中でない）は導かない。停止は従来どおり `Warning` だけで状態を変えず、上限で止めた後は操作の前の `run` から処理中だけを空にして戻す（現在の Step は残るが、受付済みのまま入力で進めない場合がある）。どちらも取り消しをしない側である
- 計算量は、1 回の導出でログの長さに比例する（位置は単調に減り、`Restored` の移動も必ず前へ進む）。導出は処理中の停止と処理中の上限停止の操作でだけ、1 操作につき高々 1 回行い、受付・進行のたびには行わないので、操作の列を適用し直しても 2 乗にならない。ログの長さは履歴の読み込み口の上限で抑えられる
- フラグ（`flags`・`global_flags`）と、選択の入力で選んだ選択肢（`choices`）は戻さない。`Event::Flag` は変更前の値を持たず、選択肢の上書きは変更前を残さないため、ログから一意に決められない。どちらも受け付け直すと同じ値を置き直す（冪等）ので、同じ規則での効果は二重にならない。これを導けない扱い（取り消しをしない）にすると、フラグを立てる処理中の Step で解決済みの P1（入口の写しが無いと受け付け直せない）が再発するため、戻さないことを選んで試験で固定した（`busy_limit_keeps_flags_set_by_the_stopped_operation`）
- 処理中に保存して復帰した後も、復習と印の戻し先は入口の受付の時点に揃える。core の復帰は保存時の `busy` を戻し、受付の時点を `Restored` から保存の時点へ起点を移して導くので、`Projection` も `CheckpointSaved` の時点の処理中と入口の控え（受付から保存までの復習の取り消し差分。量は受付の後の変更の数に比例する）を控え、`Restored` の `log_len` に対応する控えを戻す（保存の時点の復習へ差分を逆に当てて入口の時点の復習を作る）。持ち主の根の項目だけを控えから取り、他の根の項目と印の元は復帰の時点のもの（印の元は空）にする。保存の位置を指さない `log_len` では、処理中と入口の控えを無しに倒す。`Projection` の欄は外から見えず、保存形式・公開 API・`CONTRACT_VERSION` は変えない
- 停止と上限の後の戻り方を変えたので、変更前のビルドで「受付→処理中の停止→保存」のように保存した履歴は、版が同じまま適用し直しの照合で開けなくなる。エンジンは未配信で版が `0.0.0` のまま（`ENGINE_VERSION` は `Cargo` の版）なので、旧い挙動で適用し直す経路は足さない。挙動を変えたときに版を上げる運用は、履歴の保存と版の確認の作業（#1058・#1059）で扱う

試験は 3 層に置いた。

- native の core（`transition_limit.rs`・`stop_restore.rs`、入口の写しの導出）: `apply` の受付から処理中になり（状態の入口の写しは空のまま）、最後の進行で止めたとき、ログから導いた受付の時点へ戻って世界状態の変化が原因 `Stop` の `Changed` で取り消され、受け付け直して再び止まって戻れること（`busy_limit_rewinds_to_accept_derived_from_log`）。処理中に保存・先へ進めて復帰してから停止すると、`Restored` から起点を移して受付の前へ戻ること（`busy_restore_then_stop_returns_to_accept`）。写しの `log_len` が食い違うとき（`mismatched_checkpoint_is_not_used_to_derive_entry`）と、受付と現在の間に Step への入りがあるとき（`entry_is_not_derived_across_a_step_entry`）は導かず、停止は `Warning` だけ、上限の後は受付済みのまま処理中だけを空にすること。フラグは戻さないこと（`busy_limit_keeps_flags_set_by_the_stopped_operation`）。サンプルの写しを消しても、導いた写しで写しがあるときと同じ停止の結果になること（`stop_without_stored_entry_derives_the_same_entry_from_log`）。ログにも受付が無ければ `Warning` だけであること（`stop_without_entry_or_accept_in_log_warns_only`・`stop_with_other_step_entry_falls_back_to_log`）。凍結した旧い履歴が開くことは `crates/pack/tests/old_pack_history_open.rs` の `ext9_*` が確かめる
- native の core（`transition_limit.rs`）: 巡回 3 本と連鎖の境界で、止めた後に現在の Step・入口の写し・`run`（世代は `go_to` のとき +1）が操作の前と一致し、`Changed` が 0 件であること。戻った Step への入力でイベントの先頭が `InputReceived` になること。`cycle-exit-2` は次の入力で `Accepted` が出て、再び止まって戻ること。進行の経路（`Tick`）で止めたとき `busy` が空で、同じ宛先の再送は `Ignored` になり再び歩かないこと。止めて戻った後の保存が働き、同じ操作の列が同じ状態になること。受付で x を置き、最初の進行で x を消し、処理中に保存・復帰して最後の進行で止めたとき、`Changed` が 0 件で Step が入口（Entered）に戻り、復習と印に x が残らないこと（`busy_restore_then_limit_rewinds_review_to_entry`）
- native の core（`projection_derive.rs`）: 同じ場面をイベント列で組み、復習・印が入口の時点へ戻り、増分と一括が一致すること（`limit_rewind_after_restore_while_busy_returns_to_entry`）。処理中でない時点の保存へ復帰した後の移動の上限では、復帰前の受付の控えへ戻らないこと（対照、`limit_after_restore_to_idle_checkpoint_keeps_restored_review`）
- native の web（`limit_notice.rs`）: 戻った Step での入力で `InputReceived` が 1 件増え、通知欄（DOM と束縛値）が空になり、上限の行は 1 件のまま
- ブラウザ（`transition-limit.test.mjs`）: 巡回 3 本と `chain-1001` で、止めた直後の `current` が操作の前の Step で、続く入力で `input_received` が 1 件増えること。`cycle-exit-2`・`chain-1001` は `accepted` が 1 件増え、上限の行が 2 件・通知が出る。`cycle-enter-*` は通知が消える

- SAFE35-1: 止めた後に戻す処理を飛ばす
- SAFE35-2: 戻すときに `busy` を空にしない
- SAFE35-3: `run` を戻さず `current` だけ戻す
- SAFE35-4: 処理中から戻すときに世代を進めない（復帰前の進行の宛先が、再入力で同じ Step が処理中になった後に再び有効になる）
- SAFE35-5: 入力の受信のたびに入口の控えを取る（拒否された再送で上書きする）
- SAFE35-6: 処理中の移動で入口の控えへ戻さない
- SAFE35-7: 最後の進行で処理中を解かない（処理完了後の移動の上限で入口の控えへ戻る）
- SAFE35-8: 復帰で処理中と入口の控えを保存の時点へ戻さない（処理中の保存へ復帰した後の上限で、復習が進行の直前に残る）
- SAFE35-9: 導いた入口の写しで、受付から現在までの世界状態の変化を戻さない
- SAFE35-10: `Restored` から保存の時点へ起点を移さない
- SAFE35-11: 復帰の写しの `log_len` と `Restored` の食い違いを見ない
- SAFE35-12: 処理中の停止が状態の写しだけを見て、ログから導かない
- SAFE35-13: 処理中の区間で起きない出来事を読み飛ばす（区間の外の古い受付から導く）

`SAFE23-2`（`halt` の `current = None` の削除）は、`apply` が直後に戻すため、効果が試験から見えない等価変異になった。廃止し、内側の fail-closed は SAFE35 の変異と、操作の前に現在の Step が無い場合の既存の挙動で担保する。

```text
実行日: 2026-10-19
元にしたコミット: 1de3532（作業ツリーに本変更。入口の写しをログから導く作りに変えた後に SAFE35-1〜13 を一括で再実行した）。SAFE35-5〜8 は、控えを取り消し差分に変えた後に 4a000b1 の作業ツリーで再実行した
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/SAFE35-1.mutation … crates/core/tests/mutations/SAFE35-13.mutation（13 件を 1 回で渡す）
```

| ID | 対象 | 期待した失敗 | 結果 |
|---|---|---|---|
| SAFE35-1 | crates/core/src/session.rs: 止めた後に戻さない | cycle_enter_self_stops_at_limit, cycle_exit_2_stops_at_limit, exit_chain_boundary | DETECTED |
| SAFE35-2 | crates/core/src/transit.rs: `busy` を空にしない | busy_limit_without_entry_copy_keeps_current_step | DETECTED |
| SAFE35-3 | crates/core/src/transit.rs: `current` だけ戻す | cycle_exit_2_stops_at_limit, cycle_exit_2_accepts_again_and_returns_again | DETECTED |
| SAFE35-4 | crates/core/src/transit.rs: 処理中から戻すときに世代を進めない | tick_path_returns_without_busy_and_does_not_loop, busy_limit_without_entry_copy_keeps_current_step | DETECTED |
| SAFE35-5 | crates/core/src/projection.rs: 受信のたびに入口の控えを取る | limit_rewind_ignores_rejected_resend_during_busy | DETECTED |
| SAFE35-6 | crates/core/src/projection.rs: 処理中の移動で入口の控えへ戻さない | limit_rewind_on_move_while_busy_returns_to_entry | DETECTED |
| SAFE35-7 | crates/core/src/projection.rs: 最後の進行で処理中を解かない | limit_on_move_after_last_progress_returns_to_before_the_move | DETECTED |
| SAFE35-8 | crates/core/src/projection.rs: 復帰で処理中と入口の控えを戻さない | limit_rewind_after_restore_while_busy_returns_to_entry | DETECTED |
| SAFE35-9 | crates/core/src/entry_derive.rs: 導いた写しで世界状態の変化を戻さない | busy_limit_rewinds_to_accept_derived_from_log, busy_restore_then_stop_returns_to_accept | DETECTED |
| SAFE35-10 | crates/core/src/entry_derive.rs: `Restored` から保存の時点へ起点を移さない | busy_restore_then_stop_returns_to_accept, busy_restore_then_limit_rewinds_review_to_entry | DETECTED |
| SAFE35-11 | crates/core/src/entry_derive.rs: 復帰の写しの `log_len` の食い違いを見ない | mismatched_checkpoint_is_not_used_to_derive_entry | DETECTED |
| SAFE35-12 | crates/core/src/stop.rs: 停止が状態の写しだけを見てログから導かない | stop_without_stored_entry_derives_the_same_entry_from_log, stop_with_other_step_entry_falls_back_to_log | DETECTED |
| SAFE35-13 | crates/core/src/entry_derive.rs: 処理中の区間で起きない出来事を読み飛ばす | entry_is_not_derived_across_a_step_entry | DETECTED |

集計: 13 件すべて DETECTED（2026-10-19）。控えを取り消し差分に変えた後の SAFE35-5〜8 の再実行も 4 件すべて DETECTED（2026-10-19。置換対象の行は変えていない。他の 9 件は `projection.rs` を対象にせず、再実行していない）。SAFE23-1・3、SAFE34-1 は 7528a9d の作業ツリーで DETECTED を確かめた後、本変更では再実行していない（`transit.rs` の変更は `return_after_limit` の写しの取り方だけで、これらの置換対象と試験は変えていない）。

範囲外: spec の SAFE-35 の文言の更新、フラグ・選んだ選択肢を受付の時点へ戻すこと（ログに変更前の値が無い。条件の語彙の評価器を結線するとき（TASK-267）に見直す）、明示の戻る・チェックポイントへ戻る操作と画面の部品（SAVE-31 が先）、止めた操作の `Accepted`・`Achieved` がログに残ることによる食い違い、通知文言の確定、開始での連鎖（TASK-217）。

### 5.148 compile の出力とバイト単位の決定性（TASK-407）

`fandhe-pack compile <pack-dir> [-o <file>]` の `-o` の書き出しを実装し、標準出力と `-o` のファイルが同じバイト列であること、複数回・別ディレクトリの写し・列挙順違いでも IR が同一であること、末尾の形を、実行ファイルを起動する結合試験（`crates/pack/tests/compile_output.rs` の `author8_` 8 件）で固定した（AUTHOR-8）。契約は `docs/cli.md` 版 4 の 10 節。

- 書き出しは compile が診断なしで終わった後にだけ行い、同じディレクトリの一時ファイルへ書いて `rename` で置き換える（途中の失敗で IR が残らない）。書き出せないときは `error:` の 1 行と終了コード 2
- 権限で書けない場合の試験は、root で動く CI で偽陽性になるため入れていない

変異注入（実行日: 2026-10-19、元にしたコミット: 209ff01 に本変更を載せた作業ツリー、コマンド: `scripts/check-mutations.sh crates/pack/tests/mutations/AUTHOR8-*.mutation`）。

| ID | 対象 | 期待した失敗 | 結果 |
|---|---|---|---|
| AUTHOR8-1 | crates/pack/src/cli.rs: `-o` のファイルの末尾を 1 バイト落とす | author8_stdout_and_output_file_are_byte_identical | DETECTED |
| AUTHOR8-2 | crates/pack/src/cli.rs: `-o` のときも標準出力へ IR を書く | author8_stdout_and_output_file_are_byte_identical | DETECTED |
| AUTHOR8-3 | crates/pack/src/cli.rs: 書き出しの失敗を成功として返す | author8_unwritable_output_exits_2 | DETECTED |

集計: 3 件すべて DETECTED（2026-10-19）。「失敗時に一時ファイルを消さない」変異は、試験の失敗経路（親が無い・ディレクトリを指す）が一時ファイルを作る前に失敗するため等価になり、採らなかった。

### 5.149 `metrics` の数え方と出力（TASK-402）

`fandhe-pack metrics <pack-dir>`（`crates/pack/src/metrics.rs`、出力の形は `docs/cli.md` 版 4 の 11 節）に対する試験と変異注入である。数え方は PoC-3 の試作に合わせた近似（実効行数は空行と `#` 行を除く。深さは YAML がインデント / 2 + 1 + 一覧の印 + 行内の入れ子、TOML が表の見出しの段数 + 1 + 行内の入れ子）で、試作と違い、上限・リンク・読めないファイルは診断にして表を出さない（fail-closed）。

期待値の出し方。

- `crates/pack/tests/metrics.rs` の期待値は、同じ規則を Rust 以外（python）で別に書いた小さなスクリプトで、最小の教材パック 2 形式（`testdata/engine-event-consistency/test-data/min-3step-pack`・`crates/pack/tests/data/min-3step-pack-b`）と `testdata/condition-vocab-pack` に流して得た。Rust の出力から写していない。スクリプトは repo に入れていない
- 掲示板教材そのものは repo に無く（D-11。`testdata/README.md`）、spec は private なので自動試験には入れていない。ローカルで、spec の試作パック（形式 A の 8 ファイル）に `metrics` を流し、PoC-3 のログの形式 A のブロックと `diff` で比べた。ヘッダ・8 行・`TOTAL` の全行がバイト単位で一致した（2026-10-19。TOML の深さの規則まで含めて試作と同じ数え方である）。形式 B はログが章 4 の時点で、パックが章 16 に増えているため比べていない
- `AUTHOR1-5` の置換は、`run_metrics` が出力先を受けるようになったため書き直した（意味は変えていない。`vocab` の実装後は期待する失敗の試験が `i11_vocab_is_dispatched_to_vocab` になった）

AUTHOR1-7 は実効行数がコメント行を除かず、AUTHOR1-8 は `TOTAL` の深さが最大でなく和になり、AUTHOR1-9 は `workshop.*` が無くても表を出し、AUTHOR1-10 は `file` 欄の `,` を直さず 5 欄が崩れる。

```text
実行日: 2026-10-19
元にしたコミット: 209ff01（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/AUTHOR1-4.mutation … AUTHOR1-10.mutation（AUTHOR1-4〜6 の再実行を含む 7 件を 1 回で渡す）
```

| ID | 対象 | 期待した失敗 | 結果 |
|---|---|---|---|
| AUTHOR1-4 | crates/pack/src/cli.rs: 1 引数のコマンドの余分な引数の検査が外れる | argument_shape_errors_are_reported_before_loading | DETECTED |
| AUTHOR1-5 | crates/pack/src/cli.rs: vocab の振り分けが metrics の handler になる | i11_vocab_is_dispatched_to_vocab | DETECTED |
| AUTHOR1-6 | crates/pack/src/cli.rs: - で始まる引数を位置引数として拒否する判定が外れる | argument_shape_errors_are_reported_before_loading | DETECTED |
| AUTHOR1-7 | crates/pack/src/metrics.rs: 実効行数がコメント行を除かない | me1_min_pack_format_a | DETECTED |
| AUTHOR1-8 | crates/pack/src/metrics.rs: TOTAL の深さが最大でなく和になる | me1_min_pack_format_a | DETECTED |
| AUTHOR1-9 | crates/pack/src/metrics.rs: workshop.* が無くても表を出す | me5_missing_dir_and_missing_workshop_are_diagnostics | DETECTED |
| AUTHOR1-10 | crates/pack/src/cli_contract.rs: file 欄の , を直さず 5 欄が崩れる | me9_odd_file_names_keep_one_row_of_five_fields | DETECTED |

集計: 7 件すべて DETECTED（2026-10-19）。

範囲外: 実効行数・深さの厳密化（YAML の複数行の文字列・フロー記法。別の決定が要る）、掲示板教材の自動試験（D-11・TASK-14 の公開可否に依る）、形式 B の掲示板教材の照合（比べるログが無い）。

### 5.150 vocab の数え上げと出力（TASK-408）

`fandhe-pack vocab <pack-dir>` を実装し、拡張点を使った Step の表・（i）（ii）・拡張点ごとの行・教材パック全体の行（EP-10〜EP-12）を、実行ファイルを起動する結合試験（`crates/pack/tests/vocab_command.rs` の `author9_` 8 件）で固定した（AUTHOR-9）。契約は `docs/cli.md` 版 5 の 12 節。

- 試験用の教材は `crates/pack/tests/data/vocab-pack`（架空。語彙の付録 A の語を使わない）。EP-1〜EP-8・EP-13 を 1 つずつ・併用・使わない・`use`・呼ばれるモジュール・呼ばれないモジュールの形で置く
- 関門は `source::load` と `compile::compile` で、診断があれば表を出さず終了コード 1 で診断だけを出す。`validate` 専用の検査は関門に入れない
- 教材パック全体の行は、欄の有無ではなく読んだ値が既定と違うかで判定する（`vocab_check` の「欄がある」とは規則が違う）。`author9_explicit_default_values_emit_nothing` が固定する
- EP-9 と `confirm.options[].label` は、この時点では数えなかった（TASK-409）。数えるようにした記録は 5.154
- ID の `,`・`"`・制御文字・先頭の `#` は `\u{XXXX}` に直し、偽の欄や偽の集計行を作れないようにする（`author9_hostile_ids_are_escaped`）
- `fandhe-pack vocab` の振り分けは `tests/cli.rs` の `i11_vocab_is_dispatched_to_vocab` が見る。これに合わせて AUTHOR1-5 の変異の期待する失敗の試験を直した

変異注入（実行日: 2026-10-19、元にしたコミット: eb5468e に本変更を載せた作業ツリー、コマンド: `scripts/check-mutations.sh crates/pack/tests/mutations/AUTHOR9-*.mutation crates/pack/tests/mutations/AUTHOR1-5.mutation`）。

| ID | 対象 | 期待した失敗 | 結果 |
|---|---|---|---|
| AUTHOR9-1 | crates/pack/src/vocab_report.rs: EP-10 の既定の判定が欄の有無になる | author9_explicit_default_values_emit_nothing | DETECTED |
| AUTHOR9-2 | crates/pack/src/vocab_report.rs: `use` の Step が通常の Step として数えられる | author9_table_and_summary | DETECTED |
| AUTHOR9-3 | crates/pack/src/vocab_report.rs: 拡張点ごとの行が ID の文字列順になる | author9_table_and_summary | DETECTED |
| AUTHOR9-4 | crates/pack/src/cli_contract.rs: ID のエスケープが外れる | author9_hostile_ids_are_escaped | DETECTED |
| AUTHOR9-5 | crates/pack/src/vocab_report.rs: `compile` の関門が外れる | author9_compile_failure_prints_diagnostics_only | DETECTED |
| AUTHOR9-6 | crates/pack/src/vocab_report.rs: `processing` の要素の `effects` 配下の `when` が走査されない | author9_processing_effects_when_is_counted | DETECTED |
| AUTHOR1-5 | crates/pack/src/cli.rs: `vocab` の振り分けが `metrics` の handler になる | i11_vocab_is_dispatched_to_vocab | DETECTED |

集計: 7 件すべて DETECTED（2026-10-19）。`processing[].effects[].when` の `clarifying` を数える試験 `author9_processing_effects_when_is_counted` と AUTHOR9-6 は、PR のレビュー指摘（走査の欠落）で足した。

### 5.151 3 つのコマンドの入口での上限の検査（TASK-410）

上限の検査が、`validate`・`test`・`compile` が共通に通る読み込みの前に働くことを固定した（AUTHOR-10。上限の値は SAFE-22・SAFE-43）。試験は `crates/pack/tests/limits_commands.rs` の `author10_` 11 件と、`tests/cli.rs` の `i14`・`i15` である。

- 入口は `source::load`・`validate::validate_dir`・`compile::compile_dir` の 3 つ。パック全体・Markdown・深さ・シンボリックリンクの超過は、どの入口でも上限の診断だけを返し、`workshop.yaml` を壊しても `E-PARSE` が出ず、`SECRET` も診断に現れない
- 複数の上限を同時に超えた場合は、上限の診断の組だけが出る（`author10_multiple_limits_listed_and_nothing_read`）
- `body`・`body_levels` が指す `.md` 以外の本文が 524,288 バイトを超えたら、診断コードは素の `E-LIMIT` になる。大きさは読む前に metadata で判定する。`.md` の本文は従来どおり `E-LIMIT-MD-SIZE`。`.md` の判定は事前走査と同じく大文字小文字を区別する
- CLI の層は `compile` だけを確かめた（`i14`・`i15`）

未確認: `validate`・`test` の CLI 本体は未実装（TASK-403・TASK-404）のため、CLI から起動したときの終了コードと診断の行は、それらの実装後に同じ教材パックで確かめる。後続の実装が `source::load` を通ることが前提である。

変異注入（実行日: 2026-10-19、元にしたコミット: 897b5ac に本変更を載せた作業ツリー、コマンド: `scripts/check-mutations.sh crates/pack/tests/mutations/AUTHOR10-1.mutation crates/pack/tests/mutations/AUTHOR10-2.mutation`）。

| ID | 対象 | 期待した失敗 | 結果 |
|---|---|---|---|
| AUTHOR10-1 | crates/pack/src/source.rs: `.md` 以外の本文を `.md` として扱う | author10_non_md_body_over_is_e_limit | DETECTED |
| AUTHOR10-2 | crates/pack/src/source.rs: 事前走査の結果を捨てて読み込みへ進む | author10_multiple_limits_listed_and_nothing_read, author10_pack_size_over_on_every_entry | DETECTED |

### 5.152 診断コードの一覧と実装の一致の検査（TASK-400 1/2）

`docs/diagnostics.md` の 3 つの区分（2 節の本表、4 節の参照のみ、5 節の本表外）と、実装の `DiagCode::ALL` の一致を固定した（AUTHOR-11）。試験は `crates/pack/tests/diagnostics_doc.rs` の `author11_` 7 件と、`crates/pack/src/diag.rs` の `d9_all_covers_every_variant` である。`rust-test` ジョブの `cargo test --workspace` で実行されるため、CI の変更は無い。

- 区分は見出しの番号（`## 2.` など）で探した節から取り出す。コードの一覧も件数も試験に書かない。文書が無い・節が無い・本表の 1 列目がコードの形でない場合は失敗する
- `author11_doc_and_impl_codes_match`: 3 区分の和集合と `DiagCode::ALL` の集合が一致する。食い違いは「一覧にだけある」「実装にだけある」の形でコード名を示す
- `author11_buckets_are_disjoint_and_unique`: 本表の重複と、区分どうしの重複が無い
- `author11_recorded_counts_match`: 2 節の「一覧の件数」、3 節の「本表は N 件」と、実装の列挙との対応の記述（全体・本表・本表外・上限超過）が、実際の件数と一致する
- `author11_limit_codes_follow_contract`: contract の `PackLimitCode::ALL` が参照のみの区分と `DiagCode::ALL` にあり、参照のみの区分にあって `PackLimitCode` に無いのは素の `E-LIMIT` だけである
- 自己確認 3 件（`author11_detects_*`）: 文書の写しの文字列を書き換えて、「一覧にだけある」「実装にだけある」と区分の重複が検出されることを確かめる
- `d9_all_covers_every_variant`: 全変種を網羅する `match` の番号が `ALL` と過不足なく一致する。`ALL` への入れ忘れを検出する

変異注入は使わない。`scripts/check-mutations.sh` は `docs/` を写さないため、文書を読む試験は元の状態で失敗し、終了コード 2 になる（5.144 と同じ制約）。代わりに、手元で次を行い、行ったあとに元へ戻した。

実行日: 2026-10-20、元にしたコミット: 47162a5 に本変更を載せた作業ツリー。

| 変更 | 期待した失敗 | 結果 |
|---|---|---|
| `docs/diagnostics.md` の本表から `E-REF-ELEMENT` の行を消す | `author11_doc_and_impl_codes_match` が「実装にだけある: E-REF-ELEMENT」を示す | 検出 |
| `diag.rs` の `DiagCode::RefElement` の `as_str` を `E-REF-ELEM` にする | 同じ試験が「一覧にだけある: E-REF-ELEMENT」「実装にだけある: E-REF-ELEM」を示す | 検出 |
| `diag.rs` の `ALL` から `DiagCode::Parse` を消し、`d5` の件数を合わせる | `d9_all_covers_every_variant` が失敗する | 検出 |

TASK-400 2/2（#474）は節の番号を変えず、`diagnostics_doc.rs` を変えていない。2026-10-20 に `author11_*` 7 件が通ることを確かめた。

### 5.153 版上げ PR の受け入れ確認の模擬（TASK-101 1/2）

実行日: 2026-10-20、元にしたコミット: 47162a5 に本変更を載せた作業ツリー。

- `make test-check-frontend-bump`: 一時の repo で版上げを模擬した 29 件がすべて通る。版上げの検出（`Cargo.lock` の版だけの変更・ルートの feature の変更・無関係な変更・base の欠落）、版上げ PR での期待値・`tools/check-deps.sh` の `EXC_N`（1 から 2）・関門自体の変更の拒否、件数が 1 件減る・増える場合の失敗、ANSI つきの一致、skip の条件、`cargo metadata` の `default-features` 有効・`action-keydown` 欠落・壊れた JSON、誤用を含む
- 実物での確認: `make update-test-counts` で期待値を作り（Rust 2,543 件・ignored 4 件・240 束、ブラウザ 553 件）、実際の `make test` の出力を使って、試験 1 件を ignored にした写しの log が `counts rust` で終了コード 1 になり、元の log は 0 になることを確かめた（変更ファイルの一覧を `--changed-files` で渡して PR を模擬した）
- 未確認: 実際の GitHub Actions 上の動作（初回の CI で確かめる）。`make check-fork-ci` は `ruby` が無い環境のためローカルでは未実行

### 5.154 vocab の EP-9 の数え上げ（TASK-409）

`fandhe-pack vocab` が EP-9（開始条件）を数えるようにした（AUTHOR-17、D-64）。契約は `docs/cli.md` 版 6 の 12 節。

- 判定は `vocab_check::start_uses_ep9`（`requires` または `unmet`）に寄せ、`validate` の使用の判定と `vocab` の数え上げが食い違わないようにした
- 付く行は開始点の `node` の Step の行。同じ Step は 1 回。モジュールの開始点はそのモジュールの行。`use` の Step を指す開始点は数えない（`use` 行は拡張点を持たない契約。限界として `docs/cli.md` 13 節に記す）。`label` は数えない
- 結合試験は `vocab_command.rs` の `author17_` 7 件。旧 `author9_ep9_and_label_not_counted` は置き換えた

変異注入（実行日: 2026-10-20、元にしたコミット: cfa60c8 に本変更を載せた作業ツリー、コマンド: `scripts/check-mutations.sh crates/pack/tests/mutations/AUTHOR17-1.mutation crates/pack/tests/mutations/AUTHOR17-2.mutation` と `AUTHOR9-*.mutation`）。

| ID | 対象 | 期待した失敗 | 結果 |
|---|---|---|---|
| AUTHOR17-1 | crates/pack/src/vocab_report.rs: EP-9 の数え上げが働かない | author17_ep9_counted_on_start_node_row | DETECTED |
| AUTHOR17-2 | crates/pack/src/vocab_report.rs: EP-9 が scope の先頭の Step の行に付く | author17_ep9_attributed_to_target_step_not_first | DETECTED |
| AUTHOR9-1〜AUTHOR9-6 | crates/pack/src/vocab_report.rs ほか（既存） | 各 `author9_` の試験 | 全件 DETECTED（`find:` が崩れていないことの確認） |

### 5.155 版上げ PR の WASM の gzip +10% の判定（TASK-101 2/2）

実行日: 2026-10-20
元にしたコミット: bfc039b（origin/main）に本変更を加えた作業ツリー

- `make test-check-frontend-bump`: 57 件すべて pass（1/2 の 29 件に、`wasm-gzip` の 27 件と `tools/build-web.sh` の変更を `guard` が拒むケース 1 件を追加）。+10% ちょうどは合格、1 byte 超過は失敗（模擬の版上げ PR で基準を外すと失敗する）、減少・同じは合格、基点の不一致・ログの欠落や重複・base の値が 0・toolchain と `wasm-bindgen` の版の違いは終了コード 2
- 実物の確認（手元）: `CI_EVENT_NAME=pull_request scripts/check-frontend-bump.sh --base HEAD^1 --assume-bump wasm-gzip` で base と head の両方を実際にビルドした。両側とも 353359 bytes、差分 0 で合格。実行後に一時の worktree と作業ツリーの変更は残らなかった
- 未確認: GitHub Actions 上の版上げ PR での実際のビルド（本変更は版上げ PR ではないため、CI では skip の log を出して 0 で終わる見込み）。初回の版上げで確かめる。CI での所要時間の増加（版上げ PR のときだけ release ビルドが 2 回増える）も初回の実測で確かめる

### 5.156 配信用・試験用の 2 設定のビルド検査（TASK-78 1/2）

実行日: 2026-10-20
元にしたコミット: 70af5ce（origin/main）に本変更を加えた作業ツリー

`scripts/test-build-web.sh`（`make test-build-web`）に節 (e) を足し、CI の `browser-test` ジョブに step を 1 つ足した。ジョブ・`needs`・ruleset は変えていない。

- 使う教材: `testdata/engine-event-consistency/test-data/min-3step-pack`（`fandhe-pack compile` で診断 0 件）。`scripts/build-pack-bin.sh` で解決した `fandhe-pack` の標準出力を `IR_FILE` に渡す（`docs/cli.md` 10 節。画面に配る IR は compile の出力そのもの）
- (e) の検査: `release` を明示し `TEST_HOOKS` を空にした配信する設定のビルドが通ること、ログに `config: delivery / profile: release` があること、`ir/main.json` が compile の出力とバイト一致すること、`format_version`（数値）と `pack.id`（文字列）を持つこと、`mount_app` があり `debug_*` の書き出しが無いこと
- (c) に追加: 試験の設定のログに `config: test / profile: release` があること
- `tools/check-delivery.sh` は `wasm-opt` を適用したときだけ流す。未適用（CI の `SKIP_WASM_OPT=1`）の WASM には core の名前（`debug_struct` 等）が残り、バイト列の `debug_` の検査が誤検出するため。2026-10-20 に `SKIP_WASM_OPT=1` のビルドで確認した。適用したビルドでは通る
- 実行結果: `make test-build-web`（`wasm-opt` 適用）と `SKIP_WASM_OPT=1 make test-build-web` の両方がすべて ok。後者は約 47 秒
- 負の対照（手元で一時的に試し、コミットしていない）: compile の出力を仮の JSON `{"ir":"sample"}` に差し替えると「IR の形」の検査が失敗する
- 限界: web はまだ `ir/main.json` を読まない（読み込みは TASK-109・TASK-121）ため、「IR が入る」は配信物のファイルの水準で確かめる。WASM の `gzip -9` の関門と基点の記録は TASK-78 2/2
- 未確認: CI 上での所要時間の増分（初回の実行後に測る）

### 5.157 WASM の大きさの関門（TASK-78 2/2）

実行日: 2026-10-20
元にしたコミット: b83741d（origin/main）に本変更を加えた作業ツリー

`scripts/check-wasm-size.sh`（`make check-wasm-size`）を足し、CI の `browser-test` ジョブに step「Check WASM size (1MB gate)」を足した。ジョブ・`needs`・ruleset は変えていない。

- 判定: 配信する設定（`delivery`・`release`・機能の追加なし）の WASM の `gzip -9` が 1,048,576 bytes（DLV-27。R-33 の提案値）以下なら 0、超えたら 1。上限はスクリプトの定数で、環境変数・引数では変えられない。様式の欠落・試験の設定・`wasm_gzip` の欠落や重複・複数の JSON 値は 2（fail-closed）。`dirty` は失敗にせず記録する
- 出力: 標準エラーに 1 行（値・commit・dirty・`wasm-opt` の状態・設定）。標準出力に、判定に使った結果の JSON へ `gate {limit_bytes, verdict}` を足したものを出す。CI のログに基点つきの値が残る
- `wasm-opt`: CI に入れない（5.155 と同じ判断。供給網の面を増やさない）ため、CI の step は `SKIP_WASM_OPT=1` で測る。基点に `wasm_opt.applied: false` が残る。未適用の値は適用後より大きい見込みで、上限以下なら適用後も以下になる側の判定だが、`-Os` が `gzip` の大きさを増やさない保証は無い（経験則）。受入基準の「`wasm-opt` 適用後」とは測る対象が違う
- `make test-check-wasm-size`: 20 件すべて pass（ちょうど 1,048,576 は合格・1 byte 超過は失敗・試験の設定と release でない設定・`wasm_gzip` の欠落と重複・基点の欠落・連結した JSON・壊れた入力・誤用は 2）
- 実物の確認（手元。2026-10-20）: `SKIP_WASM_OPT=1 make check-wasm-size` は 353,359 bytes で合格（`wasm-opt` 未適用。約 19 秒）。`wasm-opt` を適用した `make check-wasm-size` は 298,392 bytes（`wasm-opt version 116`）で合格。いずれも `dirty: true`（本変更が未コミットのため）
- 未確認: CI 上での所要時間の増分と、GitHub Actions 上の実行（初回の PR の実行後に測る）。`actionlint` は手元で通した
- 範囲外: CI への binaryen の導入と適用後の値での判定、`scripts/check-frontend-bump.sh` の `GUARDED_FILES` への関門の追加。`ci.yml` 自体の書き換えは本関門では防げず、ai-review とレビュースレッドの解決必須で補う。spec の人間の承認点（CI の関門を加えること）は PR で承認を求める

### 5.158 更新の dirty field と束縛点・keyed list の対応（TASK-348 1/3）

実行日: 2026-10-20
元にしたコミット: 0edd8b3（origin/main）に本変更を加えた作業ツリー

`crates/web/tests/dirty_binding_points.rs` を足した（FEND-3）。上流の `Runtime::apply_update_for_dirty` は、mount 時の束縛点の表にも DOM の `data-bind-list` にも無い field が dirty に 1 件でもあると、`root` の全子ノードを作り直す。この条件を、`view()` の目印（`collect_binding_specs` の束縛点と `data-bind-list`）で模して確かめる。各回で、(1) dirty の field が mount 時の目印に含まれる、(2) 更新後の目印の集合が mount 時と同じ、(3) `unresolved_binding_specs` が空、の 3 点を見る。`src/` と依存は変えていない。

- 試験 1: 33 回の更新（入力の受付 10・進行と停止と受け付け直し 11・保存 1・ビューの選択 3・確認と選択 2・移動と復帰と範囲外 5・候補の開き直し 1）。出力は `updates=33 with_dirty=28`。dirty になった field は 16 種で、固定した 11 種（`input_notice`・`input_candidates`・`learner_log`・`engine_log_rows`・`engine_log_consumed`・`run_status_word`・`run_status_state`・`artifact_consumed`・`review_items`・`review_consumed`・`offer_choices`）をすべて含む
- 試験 2: mount 時に描かれる全 field が一度は dirty になる。除外は `NEEDS_OTHER_FIXTURES` の 5 件で、`log_notice`（遷移 1,000 回の上限の教材は試験の設定だけにある）と、33 回の列の教材が宣言していない 4 種のビューの list（`view_app_demo`・`view_check_list`・`view_document`・`view_task_tree`）
- 試験 3: 承認を持つ教材で `decide` と `revise-target` を送る（33 回には数えない）
- 試験 4: 入力欄なしの教材（PACK-20）。候補の list が無く、下書き・送信・補完が dirty を積まず、他の更新は試験 1 と同じ条件を満たす

コマンド: scripts/check-mutations.sh crates/web/tests/mutations/FEND3-1.mutation crates/web/tests/mutations/FEND3-2.mutation

| ID | 対象 | 結果 |
|---|---|---|
| FEND3-1 | crates/web/src/engine_parts.rs: 復習の消費済みの件数の束縛点の field 名を登録簿と違う綴りにする | DETECTED（試験 1〜4 が失敗） |
| FEND3-2 | crates/web/src/host.rs: ビューの選択で目印の無い field 名を dirty に積む | DETECTED（試験 1・2 が失敗） |

限界: native の試験は上流の判定条件を `view()` の目印で模したもので、実 DOM での要素の同一性・フォーカスと入力途中の値の保持（B-7）、`rerender` の負の対照（B-7n）は後続のブラウザ試験が確かめる。`scripts/expected-test-counts.txt` は更新していない。

### 5.159 root の外の Esc の受け持ちと「ちょうど 1 回」（TASK-353 1/2）

実行日: 2026-10-20
元にしたコミット: cfd7755（origin/main）に本変更を加えた作業ツリー

root（`#app`）の外の Esc は、glue が `document` の keydown で受けて Component に届ける（FEND-8・D-42。`glue::wire_outside_esc`）。root の中の Esc は上流の属性（`ESC_KEYDOWN_ATTRS`）が届けるため、target が root の中なら glue は届けない（二重配送を作らない。FEND-6）。判定は native で試せる純関数 `key_route::outside_root_esc` が持ち、上流の `action_from_keydown` に同じ属性を与えて照合する（キー・修飾キー 4 つ・変換中の除外が root の中と同じ関数でそろう）。

| 場面 | 期待 |
|---|---|
| 処理中でない × フォーカスなし（`body`） | `escCount` +1。`input_received` と `judged`（拒否。`not_in_this_step`）が各 1 件・世代は不変・`stopped` は 0 件 |
| 処理中でない × root の外のボタン | 同上 |
| 処理中 × フォーカスなし（`body`） | `escCount` +1。世代 +1・`run-status` は `stopped`・`stopped` が 1 件・`input_received`・`judged`・`changed` は 0 件 |
| 処理中 × root の外のボタン | 同上 |

- 試験: `tools/browser/esc-once.test.mjs`（`scripts/test-browser.sh` に追加。CSP あり・なし）。場面の表 `SCENES` に行を足せば TASK-353 2/2（入力欄・root の中のボタン）が受け継げる。Esc の後に 2 フレームと 80ms 待ち、遅れて届く 2 回目が無いことを見る。対照として `Shift+Escape`・変換中の合成イベント・`Enter` が 0 件であることも見る（シミュレーターであり、実 IME ではない）。root の外のボタンは試験のページにだけ作る要素で、配信物には無い
- 計画との差（実測で確かめた）: 計画は「処理中の停止では `changed` が 1 件」としたが、実測では 0 件だった。停止は `run` を入口の写しへ戻すだけで世界状態を戻さないため、`changed` は出ない。期待を 0 件に直した
- `esc-attr.test.mjs`: 最後の試験を「root の外（`body`）の Esc は今は届かない」から「ちょうど 1 回届く（2 回押して 2）」に改めた
- 契約の表（`tests/glue_contract.rs`）: `DOM_CALLS` に `document`・`contains`（いずれも `KeydownReceive`）、`NON_DOM_CALLS` に `ok_or_else` を足した。`GlueOp` の変種は増やしていない（7 件のまま。`PARTS_CONTRACT_VERSION` は 1）。`document` に `keydown` 以外のリスナーを足すと検出する負例と、`keydown` の正例を足した。要素の取得（`.root(`・`.query_selector(`）の箇所は増やしていない（`root` の clone を closure に持ち込む）
- `web-sys` の feature に `Document`・`Node` を足した（`Cargo.lock` は変わらない）
- 変異 `FEND8-1`（`outside_root_esc` の root の中の早期 `None` を外す）: `scripts/check-mutations.sh crates/web/tests/mutations/FEND8-1.mutation` で DETECTED（失敗した試験は `root_の中の_esc_は二重に届けない`）
- 実行した検査（手元。2026-10-20）: `make lint`・`make test`・`make check-deps`・`make check-dom-reads` は通った。ブラウザ試験は `esc-once`（3 回続けて pass）・`esc-attr`・`esc-outcomes`・`esc-key`・`keydown-action`・`key-invariants`・`stop-redo`・`candidates`・`point-keys` が pass（CSP あり・なしを含む）
- 未検証: `make test-browser` の全体と `make check-mutations` の全件、CI 上の実行（初回の PR の実行後に確かめる）。`scripts/expected-test-counts.txt` は更新していない（別の PR で行う）
- 範囲外・申し送り: 入力欄・root の中のボタンの 4 場面は TASK-353 2/2。root の中でも `main` の外にある部品（`workshop-parts` の中のボタン）にフォーカスがあるときの Esc は、上流の属性にも本件の glue（root の外だけ）にも当たらず 0 回になる見込みで、本件では直していない

### 5.159b 入力欄・root の中のボタンの Esc と「ちょうど 1 回」（TASK-353 2/2）

実行日: 2026-10-20
元にしたコミット: 4c2f7bc（origin/main）に本変更を加えた作業ツリー

5.159 の場面の表 `SCENES` を 10 行に広げた（処理中か否か × 5 つのフォーカス先）。CSP あり・なしで計 20 件と、対照 2 件。

| フォーカス先 | 場所 | 期待 |
|---|---|---|
| フォーカスなし（`body`）・root の外のボタン | 5.159 のとおり | 5.159 のとおり |
| 入力欄（`draft-input`） | `main` の中 | `escCount` +1。処理中でなければ取り消しの入力（`input_received`・`judged` 各 1 件）、処理中なら停止（`stopped` 1 件） |
| root の中のボタン（停止ボタン `stop-btn`） | `main` の中 | 同上 |
| 左の成果物の行（`notes/draft`） | `#app` の中・`main` の外（`workshop-parts`） | 同上 |

- 仕様の「8 つの場面」は body・root の外のボタン・入力欄・root の中のボタンの 4 × 2。`main` の外の部品の 2 行は、5.159 の申し送りの穴を確かめる追加の行である
- 処理中でない × 入力欄は、Esc の前に下書きが空で候補の一覧が閉じていることを assert する（候補が開いていると Esc は閉じるだけになるため）。処理中でない × 部品は、`/make-note first-note` を送って進行が終わる（`run-status` が処理中でない）のを待ち、行が出てからフォーカスする。Esc の結果は拒否（`not_in_this_step`）で、場面ごとに期待を変える必要は無かった
- 実測した穴 1（5.159 の申し送りの確認）: 修正前は `main` の外の部品（`workshop-parts`）の Esc が 0 回（`escCount` の増分 0）。上流の属性が `main` にしか無く、glue は root の外だけを受け持つため。`host.rs` の `view()` が `workshop-root` に `ESC_KEYDOWN_ATTRS` を足して塞いだ（`main` の属性は残す。入れ子でも上流は `closest` の最も近い 1 つにだけ届け、`main` の中の 3 場面は 1 回のまま）。教材を読めずに起動した場合の通知の root には付けない
- 実測した穴 2（穴 1 を塞いだ直後に出た）: 処理中の部品の行で Esc を押すと 2 回届いた（停止の後に 2 回目の入力が `ignored` で残る）。上流の配送が先に停止して画面を作り直すと、押された行が DOM から外れ、泡立ちの段で受ける glue の `contains` が偽になって root の外として届けていた。glue の `document` のリスナーを捕捉の段（`add_event_listener_with_callback_and_bool`）に変えて、画面が作り直される前に target を判定する。契約の表（`glue_contract.rs`）の許可済みの呼び出し名の範囲に収まり、変更は不要だった
- 受け持ちを変えても同じ結果であること: 新しい切り替えの仕組みは作っていない。既存の場面（body・root の外のボタン）と `main` の中の場面が受け持ちの変更の前後で同じ結果であることを `esc-once` の表で見た。比較として `esc-attr`・`esc-outcomes`・`esc-key`・`keydown-action`・`stop-redo`・`point-keys`・`keyboard-confirm` と TASK-351 の 5 ファイル（`key-invariants`・`tab-cycle`・`tab-complete`・`shift-tab`・`candidates`）を流した。`scripts/test-browser.sh` の全体（575 件）が pass した（2026-10-20）。`esc-once` は 3 回続けて 22 件 pass
- 試験: `crates/web/tests/host_contract.rs` の `workshop_root_は_esc_の_keydown_属性を持つ`（`workshop-root` と `hello-root` が Esc の属性を持つ）、`crates/web/tests/ir_refusal_host.rs`（通知の root は持たない）
- 変異 `FEND6-1`（`workshop-root` への `ESC_KEYDOWN_ATTRS` の追加を外す）: `scripts/check-mutations.sh crates/web/tests/mutations/FEND6-1.mutation` で DETECTED。`FEND8-1` も DETECTED のまま。捕捉の段への変更を戻す変異は、ブラウザ試験（`esc-once` の処理中 × 部品）が検出する
- 実行した検査（手元。2026-10-20）: `make check`・`make lint`・`make test`・`make check-deps`・`make check-dom-reads` は通った
- 未検証: `make check-mutations` の全件、CI 上の実行。`scripts/expected-test-counts.txt` は更新していない（別の PR で行う）。5.160 は別の PR（#1631）が使うため、本節は 5.159b とした
- 範囲外: `main` の Esc の属性を `workshop-root` に一本化する整理、変換中の root の中の Esc（TASK-354）、押しっぱなしの扱い（FEND-18・TASK-364）

### 5.160 要素の同一性とフォーカスの保持（TASK-348 2/3）

実行日: 2026-10-20
元にしたコミット: cfd7755（origin/main）に本変更を加えた作業ツリー

`tools/browser/element-identity.test.mjs` を足した（FEND-3。PoC-1 の B-7 相当）。試験の設定の画面で、入力の受付・進行・保存・確認の順に操作し、上流の構造フォールバック（root の子の全置換）が起きないことを実際の DOM で確かめる。`src/` と依存は変えていない。

- 操作の列: 入力欄にフォーカスして `/make-note first-note` を打ち Enter で送信 → 実タイマーの進行 5 回を上限つきのポーリングで待つ → `save` → `/make-no` を途中まで打つ → 確認の Op（`ui` の `v-files`・`notes/draft`）→ 選択（`opt-memo`）→ もう一度 `save`。入力欄にフォーカスした後は、フォーカスを動かす操作をしない（`raw-trigger` は JS の `click()`）
- 確かめる項目: (1) `#app` の子の取り除きが 0 回（`MutationObserver` の計数。読むときに未配送の記録も数える）、(2) 参照を保持した 18 要素が `===` かつ `isConnected`、(3) `document.activeElement` が入力欄、(4) 入力欄の値と `__fandheTest.draft()` が `/make-no`、(5) 空振りでない（`log-list` が 1 件・ログの行と `engine.logLen` の増加・進行 5 回・確認と選択のログの増加）、(6) CSP 違反・ページ内エラー・コンソールのエラーが 0 件
- 保持する 18 要素（`#app` の中でちょうど 1 件。PoC の id は製品に無いため `data-testid` で選ぶ）: `workshop-root`・`hello-root`・`workshop-parts`・`draft-input`・`submit-input`・`stop-btn`・`input-notice`・`part-log`・`part-log-consumed`・`run-status`・`step-status`・`part-artifact-view`、リストの親 6 本（`log-list`・`part-log-list`・`part-review-list`・`candidate-list`・`view-select`・`view-list[data-view=file_tree]`）

手元の確認（コミットしない。2026-10-20）:

| 確認 | 内容 | 結果 |
|---|---|---|
| (a) 対照 | 検査の直前に `__fandheTest.rerender()` を呼ぶ | 失敗（root の子の取り除きが 1 回） |
| (b) 変異 | `crates/web/src/hello.rs` の送信で、目印の無い field 名を dirty に積む | 失敗（root の子の取り除きが 1 回） |

通常の試験は 3 回続けて通過した。

限界: 負の対照（`rerender` で保持した要素が残らないことの試験）は TASK-348 3/3 が担当する（5.161 で足した）。ブラウザ試験には変異注入（`.mutation`）の仕組みが無いため、(a)(b) は手元の確認に留める。配信設定の画面での変種は入れていない（`raw-trigger` が無く、入力・Enter・実タイマーの進行だけになるため）。`scripts/expected-test-counts.txt` は更新していない（件数の照合は版上げ PR でだけ働く）。

### 5.161 全再描画の負の対照（TASK-348 3/3）

実行日: 2026-10-20
元にしたコミット: 7b130ca（origin/main）に本変更を加えた作業ツリー

`tools/browser/element-identity.test.mjs` に 2 件目の `B-7n` を足した（FEND-3。PoC-1 の B-7n 相当）。試験用の口の全再描画（`__fandheTest.rerender()`）を起こすと、参照を保持した 6 要素が 1 つも残らず、入力欄のフォーカスが失われることを確かめる。B-7 の検査器（`changedRefs`・`readSwaps`・フォーカスの判定）が実際に差を検出できることの裏づけである。`src/` と依存は変えていない。

- 操作の列: 入力欄にフォーカスして `/make-no` を打つ（送信しない）→ 作り直しの前の前提（外れた要素 0 件・取り除き 0 回・フォーカスは入力欄）を assert → `rerender()`（`true` を assert）→ 2 フレーム待つ
- 6 要素（B-7 の `PINNED` の部分集合。PoC との対応）: `workshop-root`（app-root）・`draft-input`・`submit-input`（submit-btn）・`stop-btn`・`view-list[data-view=file_tree]`（world-list）・`part-log-list`（log-list）
- 確かめる項目: 取り除きが 1 回以上、`changedRefs` が 6 件（残った要素が 0 件。PoC の `< 6` より強い）、`document.activeElement` が元の入力欄でも新しい入力欄でもない、CSP 違反・ページ内エラー・コンソールのエラーが 0 件。入力途中の値の喪失は assert しない（`ime-input` の I-7 が、作り直しの後も欄の値が下書きと一致すると assert しているため）

手元の確認（コミットしない。2026-10-20）:

| 確認 | 内容 | 結果 |
|---|---|---|
| (a) 対照の対照 | `rerender()` の呼び出しを外した写しで流す | 失敗（取り除きが 0 回で「取り除きが起きた」の assert が落ちる） |

2 件（B-7・B-7n）は 3 回続けて通過した。

限界: ブラウザ試験には変異注入（`.mutation`）の仕組みが無いため、(a) は手元の確認に留める。`scripts/expected-test-counts.txt` は更新していない（件数の照合は版上げ PR でだけ働く）。`make test-browser` の全体は未検証（本試験のファイルだけを流した）。

### 5.162 置き換えの前後の WASM の gzip の大きさ（TASK-361）

FEND-16 と TASK-361 は、配信する設定の WASM の `gzip -9` の大きさについて、「上流の口への置き換え」の前後で増加が +10% 以内であること、大きさが 1,048,576 bytes（1MB。DLV-27・R-33 の提案値）以下であることを求める。ただし置き換えは起きていない。D-42 は案 (c)（glue を維持する）を選んでおり（FEND-7・FEND-8。5.92）、比べる対象を 5.92 と同じに読み替える。

- 「前」は `669ab6e`（TASK-352 に着手する前の基準。5.86b）、「後」は `bddd086`（TASK-352 の 3 件の完了後）
- `669ab6e..bddd086` には TASK-319（保存・復元）と TASK-468 の変更も入っている。TASK-352 自身の増分を分けるため、中間の 3 点（`2be0b3f`＝TASK-352 1/3 の直前、`85d03e6`＝1/3、`4835336`＝2/3 の直前）も測った
- 1MB の判定が今の本体でも成り立つかを示すため、測定時点の `origin/main`（`487571f`）も測った

結論: 1MB 以下の判定は全点で満たす。前後の +10% 以内の判定は満たさない（`wasm-opt` 適用で +45.9%）。ただし増分はすべて `669ab6e` から `2be0b3f` までの区間（TASK-319 の保存・復元の結線）で生じており、TASK-352 の 3 件が足した大きさは 0 bytes である（4 点とも WASM が同じ大きさ）。値は調整せず、そのまま記録する。

```text
実行日: 2026-10-20
元にしたコミット: 487571f（origin/main）。測った点ごとに detached の worktree へ取り出して測定
コマンド: 各点で tools/measure/wasm-size.sh（wasm-opt 適用）と SKIP_WASM_OPT=1 tools/measure/wasm-size.sh（未適用）。CARGO_TARGET_DIR は全点で共有
```

基点（全点で共通。`docs/measurement.md` の 4 項目）。

| 項目 | 値 |
|---|---|
| commit | 表の各点（すべて `dirty: false`） |
| `wasm-opt` | 適用の系列は `wasm-opt version 116 (version_116)`、未適用の系列は `SKIP_WASM_OPT=1` |
| 機能の範囲 | `config: delivery`・`profile: release`・`cargo_features: []`（`test-hooks` なし） |
| 圧縮 | `gzip -9 -c`（`gzip 1.14`） |
| toolchain | `rustc 1.98.1 (48a229cea 2026-09-01)`・`wasm-bindgen 0.2.128`（全点で `Cargo.lock` も同じ版） |

表 1 `wasm-opt` 適用の系列（判定の本線。単位は bytes）。

| 時点 | commit | `wasm_raw` | `wasm_gzip` | 前の点からの差分 | 千分率 |
|---|---|---|---|---|---|
| 前 | `669ab6e` | 482,170 | 192,305 | - | - |
| TASK-352 の直前 | `2be0b3f` | 744,357 | 280,585 | +88,280 | +459.1‰ |
| 1/3 | `85d03e6` | 744,357 | 280,585 | 0 | 0‰ |
| 2/3 の直前 | `4835336` | 744,357 | 280,585 | 0 | 0‰ |
| 後 | `bddd086` | 744,357 | 280,585 | 0 | 0‰ |
| 測定時点の本体 | `487571f` | 789,783 | 299,329 | +18,744 | +66.8‰ |

表 2 `SKIP_WASM_OPT=1` の系列（CI の関門と同じ基点の参考値。単位は bytes）。

| 時点 | commit | `wasm_raw` | `wasm_gzip` | 前の点からの差分 | 千分率 |
|---|---|---|---|---|---|
| 前 | `669ab6e` | 1,048,722 | 230,629 | - | - |
| TASK-352 の直前 | `2be0b3f` | 1,599,048 | 333,766 | +103,137 | +447.2‰ |
| 1/3 | `85d03e6` | 1,599,048 | 333,766 | 0 | 0‰ |
| 2/3 の直前 | `4835336` | 1,599,048 | 333,766 | 0 | 0‰ |
| 後 | `bddd086` | 1,599,048 | 333,766 | 0 | 0‰ |
| 測定時点の本体 | `487571f` | 1,683,744 | 353,889 | +20,123 | +60.3‰ |

判定（適用の系列。`after * 100 <= before * 110` は `check-frontend-bump.sh` の `wasm-gzip` と同じ式）。

| 条件 | 比べる値 | 結果 |
|---|---|---|
| A1 前後の +10% 以内 | `669ab6e` 192,305 → `bddd086` 280,585（+45.9%。28,058,500 > 21,153,550） | 満たさない |
| A1（未適用の系列） | 230,629 → 333,766（+44.7%） | 満たさない |
| TASK-352 自身の増分（1/3 と 2/3） | `2be0b3f` → `85d03e6`、`4835336` → `bddd086` とも 0 bytes | 増加なし |
| A2 `wasm_gzip` が 1,048,576 以下 | 最大は適用 299,329・未適用 353,889（いずれも `487571f`） | 全点で満たす（余裕は適用で約 749,000 bytes） |

読み方。

- 増分の原因は置き換えではない。`669ab6e..2be0b3f` の依存の差は `js-sys` の追加だけで、`fandhe-frontend-*` の版は変わっていない。保存先（localStorage・IndexedDB）と書き出しの結線を入れた TASK-319 の変更とみられる（推測。区間内の変更ごとの寄与は分けていない）
- `wasm-opt` を適用すると `wasm_raw` は約 5 割減るが、`wasm_gzip` は約 15〜17% しか減らない。未適用との差は小さく、適用の有無は +10% の判定の結論を変えない
- 「前」を TASK-352 の直前（`2be0b3f`）にとれば、前後の増加は 0% で A1 を満たす。ただし FEND-16 の「前」は置き換えの前の基準であり、読み替えは 5.92 に合わせて `669ab6e` とした。どちらを正とするかは spec 側の判断に委ねる
- 参考値は比較に使わない。382,192 bytes は PoC-1 の追試（`fandhe-frontend` 0.46）の試作の基点で、本リポジトリの依存（`fandhe-frontend-app 0.2.6` など）・commit・機能の範囲が違う。5.157 の 353,359 bytes・298,392 bytes は `dirty: true` で別の commit の値である

限界と残る作業。

- 初回公開の前に基点をそろえて測り直す作業は TASK-126（DLV-27）が受け持つ。本節は一回の測定の記録で、関門ではない
- CI の関門は `SKIP_WASM_OPT=1` の基点（5.155・5.157）で判定する。CI に `wasm-opt` は入れていない
- 手元の `wasm-opt`（version 116）の版は `docs/toolchain.md` で固定していない。適用の系列は再現に同じ版が要る
- 前後の +10% を満たさない事実は、spec の FEND-16 の文言（置き換えの前後）と D-42 の食い違いを含めて spec 側で扱う。置き換えが将来起きる場合は、本節と同じ手順で測り直す
- `make test-measure-wasm-size` は流していない（未検証）

### 5.163 1 リストを約 1,000 項目で畳む（TASK-360 1/2）

実行日: 2026-10-20
元にしたコミット: 9f2dbb2（origin/main）に本変更を加えた作業ツリー

FEND-22・D-46 の案 3（Step ごとに分け、古い Step を 1 行の要約に畳む）を、右のログ（`LogPart`）と復習（`ReviewPart`）へ入れた。畳みの計画は `list_policy::plan_fold`（純粋関数）、要約行は `list_policy::fold_node` が作る。D-46 が実装に委ねた細目は、次のとおり決めた。

| 細目 | 決めたこと |
|---|---|
| グループ | ログは表示される行のうち `(世代, Step)` が同じで連続する範囲。復習は項目 1 件を 1 グループ。章の単位は作らず、集約行で代える |
| 畳む順序 | (1) 最新を除く 2 行以上のグループを、古い順に Step の要約 1 行にする。(2) なお超えるなら最古から順に集約行 1 行へまとめる。(3) なお超えるなら最新のグループの先頭を「この Step の前の N 行」の 1 行にする |
| 保証 | 総数が 1,000 以下なら畳まない。超えるときは出る行が常に 1,000 以下（4,096 件の安全弁は残る） |
| 決定性 | 計画はグループの長さの列と目安だけで決まる。履歴を持たず、増分と一括の導出が一致する |
| キー | 要約 `s{先頭のイベント}`・集約 `fold-older`・先頭の畳み `fold-head`。データ行のキーは変えない（残る行の要素を保つ） |
| 要約行 | 文言は固定で数は 3 桁区切りの 10 進だけ。属性は `data-fold`・`data-fold-rows`（Step の要約だけ検証済みの `data-step`）。`data-event` は付けない |
| 開き方 | 未実装（`docs/known-gaps.md` の FEND-22） |

- native: `crates/web/tests/list_fold.rs` の 9 件（恒等・1,001 での畳み・形ごとの上限・古い側だけが畳まれる・追記・擬似乱数 1,000 通り・長さ 0 と巨大値で panic しない・要約行の文言と属性）。`list_limit.rs` のホストの試験は、表示前の行が 4,096 を超えるまで入力を続け、畳みで 1,000 項目以下・通知なし・dirty の解決を確かめる形に書き直した（4,097 件の `Err` の経路は単位の試験が守る）
- ブラウザ: `tools/browser/list-fold.test.mjs`（`raw-trigger` から拒否される入力を送り続け、50 回ごとに 1,000 項目以下・通知 0 件・要約行に `data-event` なし・畳みの発生を確かめ、最後に親と枠の同一性・安全性の観察 0 件を確かめる。所要約 4 秒）。復習の 1,000 件超は IR から作れないため native の試験で守る（`crates/web/tests/review_fold.rs` の 2 件。達成済みの Step を 1,001 件ログに積んだホストを `ReviewPart` まで描き、要約を含む項目数が 1,000 以下・`older` の要約 1 行と畳んだ件数・最新の項目の保持・残る行の連続・通知なし・束縛点 1 つを確かめる。1,000 件ちょうどでは畳まない）
- 変異: `FEND22-2`（計画が畳まない）・`FEND22-3`（ログがグループの長さを渡さない）・`FEND22-4`（先頭を畳んだ後も畳んだ行を出す）が DETECTED。`FEND15-4` は host の試験を書き直したため、`expect-fail` から host の試験を外した
- 既知: `FEND15-2`・`FEND15-3` の `find` が base の `engine_parts.rs`（`keyed_view_or_notice`）と一致せず、`check-mutations.sh` が終了コード 2 で止まる（本変更の前から。未対応）

未実施: 左のビュー（7 本の view list。TASK-484）、4,096 件の打ち切りと状態の文字（TASK-360 2/2）、再描画の傾きの測定（TASK-232・TASK-454）。上流への報告は、D-46 に従い行わない。`scripts/expected-test-counts.txt` は更新していない。

### 5.164 状態を変える経路が update だけであることの検査（TASK-362 1/2）

FEND-20 は、glue（web-sys）がキー入力・タイマー・変換中の印・保存の通知を受けても、状態を変える経路は `Component::update` だけであることを求める。従来の `crates/web/tests/state_paths.rs` は `glue.rs` だけを読んでいた。本節で、`lib.rs` の `cfg(target_arch = "wasm32")` つきの `mod` 宣言から求めた層全体（`glue`・`persist`・`leave_flush`・`storage_idb`・`storage_local`・`history_download`・`test_hooks`）に広げた。DOM 操作の許可一覧と glue の行数・web-sys の feature 数の記録は 2/2（TASK-362）が受け持つ。

置き場: spec の変更対象は `tools/check-deps.sh` と CI だが、`check-deps.sh` と `state_paths.rs` は「DOM 読み取りは前者、状態経路は後者」と分担を決めている。同じ規則を二か所に持たないため、本検査は `state_paths.rs` に置いた。`make test` を通して CI の `rust-test` ジョブで流れる。層の列挙が失敗・空の場合、`host.rs` から `&mut self` のメソッドを列挙できない場合は、失敗にする（fail-closed）。

| 規則 | 内容 |
|---|---|
| R1 | 層のどのファイルにも `.update(`・`::update(`・`component_mut`・`.borrow_mut().as_mut()`・`apply_with_view_transition`・`::hydrate(`・`.hydrate(` が無い（`hydrate` は DOM の属性から状態を復元し、`update` を通らない） |
| R2 | `RUNTIME`・`Runtime<EngineHost>` は glue の外に無い。glue の外の `.dispatch_action(` は `test_hooks::debug_view_fixture_dispatch` の 1 件、`.component()` は `debug_view_fixture_state` の 1 件、`Runtime::mount(` は `glue::mount_host` と `debug_view_fixture_mount` の各 1 件だけ（件数は増減とも失敗） |
| R3 | `host.rs` の `&mut self` のメソッド（実測 2026-10-20 で 19 名）は、層の中では許可表の呼び出しだけを許す（`persist::boot_and_mount` の `boot` 2 件・`block_saving` 1 件、`glue::mount_host` の `set_tick_request`・`set_tick_reset`・`set_draft_sync_request` 各 1 件）。許可した呼び出しは mount より前に位置する。呼び出しは `.method(` と UFCS 形式の `::method(` の両方を数える |
| R4 | `host.rs` に内部可変性の綴り（`std::cell`・`RefCell`・`Cell<`・`UnsafeCell`・`OnceCell`・`Mutex`・`RwLock`・`Atomic`）が無い。`with_app` の `&EngineHost` で状態を変えられない前提を固定する |

限界:

- `EngineHost` が持つ他の crate・他のモジュールの型の内部可変性を推移的にたどる検査はしない。2026-10-20 時点で core・contract の `src` に内部可変性は 0 件である（人が確認した事実で、機械の検査ではない）
- 名前の衝突（層のコードが別の型の同名メソッドを呼ぶ）は検出側に倒す。誤検出が出たら、許可表に理由つきで足す
- 文字列の照合のため、マクロ展開や別名の `use` を介した呼び出しは見ない

変異注入の記録（`FEND20-2`〜`FEND20-6` は新規。`G1`・`G2`・`FEND4-1` は範囲を広げた後も検出することの再確認）。

```text
実行日: 2026-10-20
元にしたコミット: 50a74a0（未コミットの差分つき）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/{FEND20-2,FEND20-3,FEND20-4,FEND20-5,FEND20-6,G1,G2,FEND4-1}.mutation
```

| ID | 入れた誤り | 規則 | 結果 |
|---|---|---|---|
| FEND20-2 | `persist.rs` の `mount_host` の直後に `host.boot(None);` | R3（位置） | DETECTED |
| FEND20-3 | `persist.rs` に許可表に無い `host.open_history(&[]);` | R3（許可表） | DETECTED |
| FEND20-4 | `test_hooks.rs` の読み取りの口に `rt.dispatch_action("x", "");` | R2 | DETECTED |
| FEND20-5 | `leave_flush.rs` に `Runtime::hydrate(..)` | R1 | DETECTED |
| FEND20-6 | `persist.rs` に UFCS 形式の `EngineHost::open_history(&mut host, &[]);` | R3（許可表・UFCS） | DETECTED |
| G1 | glue の `component()` を 2 箇所にする | glue 固有 | DETECTED |
| G2 | glue に状態を変える呼び出しを足す | glue 固有 | DETECTED |
| FEND4-1 | 配送を `set_timeout` を通さず直接届ける | glue 固有 | DETECTED |

本番コード（`crates/web/src/**`）は変えていない。`scripts/expected-test-counts.txt` は更新していない（件数の照合は版上げ PR でだけ働く）。

### 5.165 4,096 件の打ち切りを安全弁にする（TASK-360 2/2）

実行日: 2026-10-20。元にしたコミット: fa795da。

| 項目 | 決定 |
|---|---|
| 置き場 | `list_policy::keyed_view_or_notice` の前段（製品の全 keyed list の入口）。`keyed_view` は変えない |
| 表示 | 先頭の 4,095 件と、最後の状態の行 1 件（合計 4,096 項目）。文言は「上限超過: 先頭 4,095 件だけ表示（全 N 件）」 |
| 属性 | 親と状態の行に `data-list-limit="<全件数>"`。状態の行に `data-event` は付けない。キーは `list-limit` |
| 件数以外の `Err` | 従来の通知（`data-list-error="limit"`・`data-bind-list` を保つ） |
| `PARTS_CONTRACT_VERSION` | 上げない（方針は変わらず、実現だけが変わる） |

試験: `crates/web/tests/list_limit.rs`（4,097・10,000 件の打ち切り、再構築、キー重複の通知）、`crates/web/tests/view_select.rs`（4,095・4,096・4,097 経路）。
変異 `FEND15-4`・`FEND22-1`・`FEND22-5`・`FEND22-6` は 4 件とも DETECTED（`FEND15-2`・`FEND15-3` は base からの `find` の不一致で、ファイルを指定して流した）。
`FEND15-4`・`FEND22-1` の `expect-fail` は、本節で試験名を改めた。
未実施: ブラウザ試験（`?fixture=list-limit` の開始状態は未実装）。左のビューの畳みは TASK-484。

### 5.166 DOM 操作の許可一覧の層全体への検査と、glue の行数・web-sys の feature の記録（TASK-362 2/2）

FEND-20 は、glue（web-sys）が DOM に対して行う操作を 7 種に限り、glue のコード行と web-sys の feature の数を記録して、増えたら理由を求める。従来の `crates/web/tests/glue_contract.rs` は `glue.rs` だけを読んでいた。本節で、許可一覧の検査を層全体（5.164 と同じ 7 モジュール）に広げ、行数と feature の増加を止める試験を足した。状態を変える経路の検査は 5.164（1/2）が受け持つ。

置き場: spec の変更対象は `tools/check-deps.sh` と CI だが、5.164 と同じく、DOM の値の読み取りは `check-deps.sh`（`make check-dom-reads`）、それ以外の静的検査は `crates/web/tests/` の native 試験、と分担を保った。呼び出し名の表は試験の側にだけ置く（`src/` の走査対象に DOM 読み取りの綴りを持ち込まないため）。試験は `make test` を通して CI の `rust-test` ジョブで流れる。`glue_ops.rs`・`GlueOp`・`PARTS_CONTRACT_VERSION` は変えていない（契約の変更は spec が先）。spec の人間の承認点「CI のガードの変更」と、置き場が spec の指定と違うことは、自動運転のため承認を得ていない。PR で要確認とする。

#### 層全体の許可一覧（`glue_contract.rs`）

| 規則 | 内容 |
|---|---|
| L1 | 構造を変える操作（`set_inner_html`・`append_child`・`create_element` など）は、層のどのモジュールにも 0 件 |
| L2 | glue 以外のモジュールは、モジュールごとの許可表（`MODULE_RULES`）と、層で共有する DOM ではない呼び出し名（`LAYER_NON_DOM_CALLS`・`NON_DOM_CALLS`）に無い呼び出し名を持たない。glue は従来の 7 種の厳密な表のまま（緩めない） |
| L3 | リスナー登録の第 1 引数は文字列リテラルで、モジュールの許可イベントにある。glue は 5 種（TASK-283 で `beforeinput` を足した。5.189）、`leave_flush` は `pagehide` だけ、他は 0 種（リスナー登録の呼び出し自体も許さない） |
| L4 | glue 固有の DOM 操作（`query_selector`・`set_attribute`・`click`・`set_value`・`prevent_default` など）は、他のモジュールの許可表に書けない。例外は別の意味で使う `window`・`root`・`contains`・`key` だけ。glue の外の `.query_selector(` は 0 件、`.root(` は `test_hooks` の 1 件だけ |
| L5 | 層の列挙が空、glue が無い、許可表に無いモジュールがある（新しい wasm32 専用モジュールが増えた）、層に無いモジュールの行が表にある、のどれも失敗にする（fail-closed） |

| モジュール | 用途 |
|---|---|
| `glue` | 7 種（要素の取得・属性の書き込み・click・タイマー・keydown の受け取り・変換中の印・textarea の値の反映）。厳密な表 |
| `storage_idb`・`storage_local` | 保存先（IndexedDB・localStorage） |
| `history_download` | 書き出し（Blob と object URL） |
| `leave_flush` | 離脱時の保存（`pagehide` の通知を受けるだけ） |
| `persist` | 起動時の URL（`location.pathname`）から保存先を決めてマウントする |
| `test_hooks` | 試験用の口（配信物に入らない。それでも fail-closed の表を持たせる。配信物に無いことは `check-delivery.sh` が受け持つ） |

#### glue の規模（2026-10-20、`26520cb`）

数え方は 5.92 と同じ（コード行は空行と `//` で始まる行を除いた数）。関門にするのは `glue.rs` のコード行だけで、他は記録のみ。

| ファイル | `wc -l` | コード行 | 関門 |
|---|---|---|---|
| `glue.rs` | 680 | 452 | `glue_scale.rs` の `MAX_GLUE_CODE_LINES = 562`（2026-10-22 に TASK-112 2/2 の受信上限の公開 `ir_read_limit` で 557 から上げた。その前は  TASK-112 2/2 の `mount_app` の引数 `ir`（同梱の IR のバイト列。署名が複数行になる分）と TASK-531 1/2 の履歴の読み込みの配送 `import_history`・読み取りの上限 `history_read_limit`・書き出しの URL の口の結線で 531 から上げた。その前は 2026-10-22 に TASK-335 の試験用の口 `export_live`（読み取りだけ。`test-hooks` のときだけ）で 527 から上げた。その前は 2026-10-21 に TASK-517 2/2 のキーで選んだタブへのフォーカス移動（`on_pane_focus_request`）で 509 から上げた。5.203。その前は TASK-284 2/2 の拒否の後の下書きの全選択の予約（`on_draft_select_request`・`sync_draft` の `select`）で 491 から上げた。5.200。その前は TASK-283 の `wire_paste_clip` で 452 から 491 へ上げた。この表の `wc -l`・コード行は 2026-10-20 の実測のまま。5.189。上げた経緯は以下のとおり。`<=` で照合。増えたら失敗。2026-10-20 に TASK-318 の遅延保存のタイマーで 401 から上げ、2026-10-21 に TASK-164 3/3 の `secret_gate` で 449 から上げた。5.170・5.183） |
| `key_route.rs` | 553 | 394 | なし |
| `test_hooks.rs` | 524 | 376 | なし |
| `storage_idb.rs` | 319 | 236 | なし |
| `persist.rs` | 153 | 98 | なし |
| `storage_local.rs` | 83 | 64 | なし |
| `glue_ops.rs` | 97 | 42 | なし |
| `history_download.rs` | 59 | 30 | なし |
| `leave_flush.rs` | 53 | 27 | なし |

5.92 の `bddd086` は glue.rs が 500 行・コード 324 行だった。今は 401 行で、増分は TASK-343 以降のキー処理・タイマー・root の外の Esc などによる。spec の「349 行」は決定時点の PoC の値で、数え方と範囲が今と違うため目標にしない（5.92）。今の実測を上限に固定し、増えたときだけ失敗にして理由を求める。上限を上げる PR は、`MAX_GLUE_CODE_LINES` と本節の表を同時に更新し、PR 本文に理由を書く。

#### web-sys の feature（2026-10-20、`26520cb`）

ルート `Cargo.toml` の `[workspace.dependencies]` に 25 個ある（2026-10-21 に TASK-283 が `EventInit` を足して 24 個から増えた。5.189。下の表の個数は 24 個の時点のまま）。`frontend_features.rs` の `WEB_SYS_FEATURES`（用途つき）と中身まで完全に一致することを確かめる（個数だけでなく入れ替えにも気づく）。`default-features = false` と、`crates/web/Cargo.toml` の web-sys が `workspace = true` だけで feature を足していないことも確かめる。

| 用途 | 個数 | feature |
|---|---|---|
| キー入力・textarea・左の行・root の外の Esc | 11 | `Document`・`Node`・`Element`・`Event`・`EventTarget`・`HtmlElement`・`HtmlTextAreaElement`・`HtmlLiElement`・`InputEvent`・`KeyboardEvent`・`Window` |
| 保存先（localStorage・IndexedDB） | 10 | `Location`・`Storage`・`DomException`・`IdbFactory`・`IdbOpenDbRequest`・`IdbRequest`・`IdbDatabase`・`IdbTransaction`・`IdbTransactionMode`・`IdbObjectStore` |
| 書き出し | 3 | `Blob`・`BlobPropertyBag`・`Url` |

5.92 の 21 個からの増分 3 個は `Document`・`Node`・`HtmlLiElement` で、root の外の Esc（TASK-353）と左の行の識別が入れた。決定時点の「9 個」より多いが、保存先・書き出しなどの用途が後から加わったためで、今の値を基点にする。

限界:

- 文字列の照合のため、マクロ展開・`use` の別名を介した呼び出しは見ない
- 名前の衝突（DOM でない同名の呼び出し）は検出側に倒す。誤検出が出たら、許可表に理由つきで足す
- 行数の関門は `glue.rs` だけで、`key_route.rs` など分割した先の増加は止めない（記録のみ）

変異注入の記録（`FEND20-7`〜`FEND20-10` は新規。`FEND20-1`・`G1`・`G2` は範囲を広げた後も検出することの再確認）。

```text
実行日: 2026-10-20
元にしたコミット: 26520cb（未コミットの差分つき）
コマンド: scripts/check-mutations.sh crates/web/tests/mutations/{FEND20-7,FEND20-8,FEND20-9,FEND20-10,FEND20-1,G1,G2}.mutation
```

| ID | 入れた誤り | 規則 | 結果 |
|---|---|---|---|
| FEND20-7 | `persist.rs` に `el.set_inner_html("")` | L1・L2（glue 以外） | DETECTED |
| FEND20-8 | `leave_flush.rs` のリスナーのイベントを `click` にする | L3 | DETECTED |
| FEND20-9 | `glue.rs` のコメント 1 行をコードの行にする（上限を超える） | 行数の上限 | DETECTED |
| FEND20-10 | ルートの web-sys の feature に `HtmlInputElement` を足す | feature の一覧 | DETECTED |
| FEND20-1 | glue に `el.set_inner_html("")` | glue 固有・L1 | DETECTED |
| G1 | glue の `component()` を 2 箇所にする | glue 固有 | DETECTED |
| G2 | glue に状態を変える呼び出しを足す | glue 固有 | DETECTED |

本番コード（`crates/web/src/**`）は変えていない。`scripts/expected-test-counts.txt` は更新していない（件数の照合は版上げ PR でだけ働く）。節番号は、並行する PR（#1637）が 5.164 の次を使う見込みのため、5.166 とした。

### 5.167 版上げの手順書の机上の照合（TASK-363）

実行日: 2026-10-20、元にしたコミット: a1995f8 に本変更を載せた作業ツリー。

- 6.6.2 の対応表の (1)〜(5) の各行を、FEND-17 の期待（D-63 で確定した版）、`ci.yml` の step 名（逐語）、`scripts/check-frontend-bump.sh` の usage と `error:` の文言、6.6 の表と突き合わせた。手元のコマンドの make ターゲット・オプションも実在を確かめた
- 文書だけの変更で、試験は足していない。`scripts/`・`tools/`・`ci.yml` は変えていない
- 未確認: 実際の版上げ PR での運用。初回の版上げで、CI 上の動作と所要時間を本節に追記する

### 5.168 localStorage の容量超過で状態を壊さない（TASK-323 1/2。SAVE-10）

実行日: 2026-10-20、元にしたコミット: 04b82bb に本変更を載せた作業ツリー。

- 変更のたびに保存を試す経路を足した。`update` で状態を変えた（ログが伸びた）操作の末尾で 1 回だけ、`persist::save_now` が保留に積んで保存先へ書く。下書きだけの変更では試さない。遅延・畳み込みは無い（下書きだけの変更の遅延保存は 5.170）
- 失敗の種類は `HostModel::save_failure`（`StorageError`）に入れ、`save-notice`（`role="status"`、`log-notice` とは別の欄）に固定の文で出す。容量超過は「保存できませんでした（QuotaExceededError）」。他の 2 種の文は仮置きで、配置と確定は TASK-534・TASK-323 2/2 が持つ。操作は巻き戻さず、書けなかった値は保留に残る
- native 試験 `crates/web/tests/save_failure.rs`: 未操作・保存停止・下書きだけでは保存を試さない、容量超過で表示・dirty・状態の進行・既存の保存の不変、空けた後の次の変更で表示が消える
- ブラウザ試験 `tools/browser/save-quota.test.mjs`（S-17 相当。CSP あり・なし）。`indexedDB` を隠して localStorage だけの保存先にする（IndexedDB があれば切り替わり失敗は出ないため）。(1) 離れる前に保存される (2) 満杯で理由が出て保存値は不変・ログは伸びる (3) 空けた後の次の変更で表示が消える (4) 再読み込みで復元される。`make test-browser` は 580 件が通った
- 変異注入 `SAVE10-1`〜`SAVE10-4`: 4 件とも DETECTED（`scripts/check-mutations.sh`）。表示が消えない・変更で保存しない・文言違い・下書きでも保存する
- 既知の限界: 失敗の後、`HistoryStorage::write` は使用量を全走査し直すが、試験が外から別の鍵を消した場合は古い値のままになる。IndexedDB が無いと切り替えの経路を通らないため、判定には影響しない。IndexedDB へ切り替えた後の非同期の失敗の表示は TASK-323 2/2。操作ごとの書き込みによる再描画の測定（`make measure`）は未計測

### 5.169 IndexedDB の保存失敗で状態を壊さない（TASK-323 2/2。SAVE-11・SHELL-19）

実行日: 2026-10-20、元にしたコミット: 6c6d214 に本変更を載せた作業ツリー。5.168 の「IndexedDB へ切り替えた後の非同期の失敗の表示は TASK-323 2/2」は、本節で解消した。

- 経路: IndexedDB の確定（`Done`）のコールバックからホストへは触れない（`state_paths`）ため、`persist` が保存先の失敗の種類を読み取り専用の probe（`async_save_failure`）にして `EngineHost::set_save_failure_probe` で mount の前に 1 回だけ結ぶ。ホストは `update` の末尾で毎回、「同期の保存の失敗、無ければ probe の値」を求め直し、前と違うときだけ `save-notice` を dirty に積む。結んだ時点でも 1 回引く
- `HistoryStorage::last_async_error` は、後の put の確定で `None` に戻す（戻さないと表示が消えない）。delete の失敗は従来どおり記録だけ
- 全件の読み取りの失敗（Issue のコメントで引き継いだ D-52 の巻き戻り）: IndexedDB を開けたが全件を読めなかったときは、接続を閉じたうえで `HistoryStorage::unreadable` にする。`read`・`write` は `Unavailable` を返し、`persist::boot_and_mount` の既存の経路で保存が止まる。どちらの保存先の既存の値も変えない。probe も `Unavailable` を返す。この fail-closed の採用は自動運転での判断で、人の承認は無い（採らなかった案は、localStorage に失敗の印を残して次の起動で照合する方式）
- 限界 1: 非同期の失敗は、失敗した操作の時点ではなく次の `update`（打鍵・送信・進行）で表示される。操作の時点で出すには、action の契約（FEND-4・版 1）に action を足す必要があり、spec を先に変える必要がある。本タスクでは行わない
- 限界 2: IndexedDB を開けない（`error`・`blocked`）間に localStorage だけへ書いたセッションでは、再読み込みで D-52 により旧い IndexedDB の値が勝つ危険が残る。本タスクで直したのは「開けたが読めない」場合だけ
- 限界 3: 実際の容量超過（`QuotaExceededError`）の再現（CDP の `Storage.overrideQuotaForOrigin`）は試していない（未検証。TASK-534 2/2 で試したが再現できなかった。5.233）。ブラウザ試験は put の直後に `transaction.abort()` を呼ぶ包みで中断を起こし、種類は `Failed` になる。`QuotaExceeded` の文は native 試験で確かめた
- native 試験 `crates/web/tests/save_failure_idb.rs`: 確定の失敗で次の `update` に理由が出る・状態は進む・IndexedDB の既存の値は不変・続く間は dirty に積まない、種類ごとの文、後の確定で表示が消える、localStorage の写しは確定の失敗で消えない、全件を読めない保存先は最初から理由を出し `read`・`write` を断る
- ブラウザ試験 `tools/browser/save-idb-failure.test.mjs`（CSP あり・なし）: localStorage を閾値の手前まで埋めて 1 手目を IndexedDB へ向け、put を中断させた 2 手目の後に理由が出て IndexedDB の値が不変、中断を外した 3 手目で更新・表示が消え、再読み込みで復元される。`getAll` を失敗させて起動すると、起動の直後から理由が出て、操作しても既存の localStorage の値は変わらない。`make test-browser` は 584 件が通った（5.168 の 580 件に 4 件）
- 変異注入 `SAVE11-1`〜`SAVE11-3`: 3 件とも DETECTED（`scripts/check-mutations.sh`。SAVE10-1 も DETECTED のまま）。put の成功で失敗を消さない・`update` の末尾で求め直さない・全件を読めない印を持たない
- 範囲外: 文言（`Unavailable`・`Failed`）の確定と保存停止の専用の通知は TASK-534 の持ち分。SAVE-11・SHELL-19 を確定にするかは spec 側で扱う

### 5.170 保存の遅延の測定（TASK-318。SAVE-7・PoC-6 の S-16）

実行日: 2026-10-20、元にしたコミット: `04b82bb` に本変更を載せた作業ツリー（dirty）。

受講者の操作で状態が変わってから、履歴が保存先へ書き込まれる（localStorage は `setItem` の完了、IndexedDB は transaction の `complete`）までの時間を測る。測るには変更のたびの保存の経路が要るため、本タスクで次を本番の経路に結線した。

- 変更の通知: `EngineHost::set_save_request`。`update` の末尾で、履歴（op の列・下書きの記録）が実際に変わったときだけ 1 回通知する（`history_changed` の印。変化の無い `update`・起動では通知しない。未操作の再読み込みは保存しない。SAVE-28）
- 遅延の判定: `save_schedule`（純関数。native の単体試験 6 件）。遅延（debounce）200ms・最大の待ち 500ms は PoC の実装値で、実装の値であり期待にしない。連続する変更は最後の 1 回に畳み、最初の変更から最大の待ちを超えない。古いタイマーは `clearTimeout` でなく token で捨てる
- タイマーと書き出し: タイマーの予約は `glue`（L4 の許可表で `set_timeout` は glue だけ）、書き出しは `persist::save_deferred`（`history_to_save` が `None` なら書かない。fail-closed。書けなかった値は保留に残る）。`pagehide` の書き出しはこれまでどおり残る
- 試験用の口: `debug_autosave_state`・`debug_history_save_len`（読み取りのみ。値・鍵は返さない。配信物には入らない。`make check-delivery` で確認）

#### 測定の方式

`tools/browser/save-latency.test.mjs`。時刻はページ側の `performance.now()`。書き込みの時刻は context の初期化スクリプトが `Storage.prototype.setItem`・`IDBObjectStore.prototype.put` を包んで記録する（時刻と長さだけを持ち、値・鍵は持たない）。変更の時刻 t0 は、入力欄へ値を入れて `input` を発火する操作（`SetDraft`）と同じ evaluate の中で、操作の直前に取る。

- 条件: {localStorage, IndexedDB} × {小さい履歴 2〜9KB, 大きい履歴 85〜156KB}（UTF-8 のバイト数）。各条件で暖機を 1 回捨て、計測 10 回。各回の前に保存が落ち着くのを待つ
- IndexedDB は、localStorage を閾値の手前まで無関係の鍵で埋めて開き直し、最初の本番の保存で切り替える（暖機で捨てる）
- 履歴は受け付けられない入力の送信を繰り返して伸ばす（小さい: 1 回ずつ、大きい: 25 回ずつ）
- 値: 合計 = 書き込みの完了 - t0。待ち＋書き込み = 合計 - debounce（遅延を除いた値）。書き込みだけ = 完了 - 入口
- 統計: `tools/measure/browser.mjs` の `median`・`p95`（nearest-rank。10 回では最大値に等しい）
- assert: 4 条件の p95 が 1,000ms 以内、1 回の変更で書き込みがちょうど 1 件・`saves` が 1 増える（本番の遅延保存であること）、履歴の大きさが条件の範囲に入ること、外部への要求・ページの例外が 0 件。遅延の長さそのものは assert しない
- 対照 2 件: 変更しなければ 1.5 秒待っても履歴の鍵への書き込みが 0 件／50ms おきの 5 回の変更が 1 回の書き込みに畳まれ、最初の変更から 1 秒以内に書かれる

#### 結果（2026-10-20）

```text
基点: commit 04b82bb（dirty。本変更を含む）・wasm-opt なし（SKIP_WASM_OPT=1）・機能の範囲 test-hooks（試験の設定）
      圧縮 tools/serve.mjs の gzip なし（127.0.0.1 の配信。回線の制限なし）・CPU の減速なし
ブラウザ: Chromium 153.0.8010.12（headless）・Playwright 1.63.0・Linux x86_64（Core i7-13700K・10 論理コア・RAM 27GB）
```

| 条件 | 履歴（UTF-8） | 合計の中央値（ms） | 合計の p95（ms） | 待ち＋書き込みの p95（ms） | 書き込みだけの p95（ms） | 判定（1,000ms 以内） |
|---|---|---|---|---|---|---|
| localStorage・小 | 2,235 B | 203.9 | 204.7 | 4.7 | 0.1 | 通過 |
| localStorage・大 | 90,156 B | 205.3 | 206.1 | 6.1 | 0.2 | 通過 |
| IndexedDB・小 | 2,235 B | 204.9 | 205.9 | 5.9 | 0.7 | 通過 |
| IndexedDB・大 | 90,156 B | 204.7 | 206.6 | 6.6 | 0.5 | 通過 |

合計はほぼ debounce（200ms）で、待ち＋書き込みは数 ms である。実測値は `make test-browser` の `[save-latency]` の diagnostic に JSON で出る（履歴の中身と鍵は出さない）。

#### 限界・未確認

- 基準端末 A（`docs/measurement.md`）では測っていない。上の端末は自動運転の開発機で、基準端末 A ではない。基準端末 A での値の追記は人が行う（未実施）
- Chromium だけで測った。Chrome の原本・Firefox・Safari は測っていない
- 大きい履歴は 90,156 B（範囲 85〜156KB の下端に近い）。上端（156KB）は測っていない。範囲内であることは各回に assert している
- CSP ありだけで流す（時間は CSP の meta に依らない見込みだが、なしでは測っていない）
- 履歴の大きさは、下書きの記録が同じ下書きを置き換えるため、10 回の間で変わらない（書き込みの文字列の長さは 2,075・80,156 の UTF-16 の単位）
- `make measure` の束には入れていない。束（`measure-run/v1`）は `results` を 3 件に固定しており、4 件目は様式の変更になるため（`docs/measurement.md` §7）
- SAVE-9（IndexedDB で離れる直前の変更が落ちうる）は本タスクの対象外。遅延保存で落ちる窓は狭まるが、仕様の決定は spec 側

#### 既存の試験への影響

- 変更のたびに遅延保存が走るため、`tools/browser/tick-dispatch.test.mjs` の「`setTimeout` の遅延が進行の間隔だけ」を、進行の予約（debounce より長い値）だけを数える形に直した（遅延保存の遅延が進行の間隔より短いことも確かめる。assert の意味は保つ）
- `make test-browser` は 584 件すべて通過（`save-latency` の 6 件を含む）。`ir-refusal`・`version-histories`・`reload-fresh`・`reload-points`・`storage-leave`・`storage-select`・`element-identity` は変更なしで通る
- `glue.rs` のコード行が 401 から 431 に増えた（遅延保存のタイマーと試験用の読み取り）。`glue_scale.rs` の上限と 5.166 の表を同時に更新した（5.166 が定める手続き。PR 本文に理由を書く）
- 続けて 431 から 445 に増えた（遅延保存の結果と予約の無効化の配送。`Scheduled` は `action.rs` に置いて glue の増加を抑えた）。上限と 5.166 の表を同時に更新した。遅延保存の失敗・成功は私的な action `save-result` でホストへ届け、保存通知（SAVE-10）を更新する。即時の保存は予約済みの遅延保存を無効にする（`SaveDebounce::cancel`）
- さらに 445 から 449 に増えた（遅延保存の結果に保存世代 `epoch` を付けて配送し、古い結果をホストが捨てるための `save_epoch` の取得と引数の受け渡し）。上限と 5.166 の表を同時に更新した。PR 本文に理由を書く
- `state_paths.rs` の `INIT_ALLOWED` に `set_save_request`、`glue_contract.rs` の許可表に新しい呼び出し名（DOM ではない名前）を足した。web-sys の feature は増やしていない（24 個のまま）
- `scripts/expected-test-counts.txt` は更新していない（5.166 と同じ。件数の照合は版上げ PR でだけ働く）

#### 変異注入

| ID | 入れた誤り | 期待した失敗 | 結果 |
|---|---|---|---|
| SAVE7-1 | 履歴が変わっても保存の通知を出さない（`crates/web/src/host.rs`） | `save_request` の「下書きの変更ごとに 1 回通知する」 | DETECTED（`scripts/check-mutations.sh crates/web/tests/mutations/SAVE7-1.mutation`。送信の試験も失敗した） |

### 5.171 保存の直前に起動番号を照合して古いタブを止める（TASK-324 1/2。SAVE-12・D-56）

実行日: 2026-10-20、元にしたコミット: b8b2fc9 に本変更を載せた作業ツリー。5.170 は別の open PR（TASK-318）が使う見込みのため空けた。

- 経路: ページを開くたびに起動番号を補助の鍵（meta）へ書き、保存の直前に meta が自分の値と違えば書かない。`crates/web/src/boot_guard.rs`（`BootGuard`・`Guarded`）が持ち、`persist::mount_with_history` が鍵の決定の直後（保存先を開く前）に 1 回だけ取得する。書き出しは `persist::flush_guarded` が唯一の関所で、`save_now` と `pagehide` のリスナーはここを通る。古いタブの `write` は `Ok` を返して捨てる（`Err` にすると保存失敗の表示が出るため。古いタブの表示は 5.178 で実装）
- 自動運転での判断（人の承認は無い）: (a) contract は版上げせず、meta は web が持つ。(b) meta の鍵は `fandhe-workshop/meta/boot/<配信の区間>/<教材の ID>/<開始点の ID>` で、教材ごとに分ける（全体で 1 つにすると別の教材のタブ同士が互いを古くする）。`HistoryKey::parse` が拒むため `keys()`・SAVE-32 の削除の対象に入らない。(c) 値は `<n>:<tag>`（`tag` は 8 桁の 16 進）。タブ間の読み書きは原子的でなく、同時の起動が同じ `n` を書きうるため、最後に書いた 1 つだけを一致させる。(d) 取得の書き込みの失敗・照合で読めない・取得前は、いずれも書かない（fail-closed）
- native 試験 `crates/web/tests/boot_guard.rs`: 古いタブの書き込みが届かず新しいタブは届く（陽性の対照）、古いタブの保留の書き出しが上書きしない、照合が無ければ上書きされる（対照）、古いタブは戻らない、別の鍵のタブは互いを古くしない、取得・照合の失敗で書かない、起動番号の加算と壊れた値・飽和、同じ `n` で `tag` が違う同時の起動、IndexedDB へ切り替えた保存先でも `put` を発行しない、meta の鍵が一覧・削除の対象外、`flush` の呼び出しが `flush_guarded` の 1 か所だけ。照合の窓の限界（`限界_` の 3 件。2026-10-21 に追加）: 照合の読み取りの直後に別のタブの取得を割り込ませ、古いタブの 1 回分が届くこと・後のタブの保存が上書きして残ること・後のタブが未操作なら古いタブの 1 回分が残ることを固定する。偽の保存領域にはタブ間の遅れが無く、書いた直後の再照合が効く。再照合が古い値を読む（効かない）場合は、割り込みの後の 1 回だけ前の値を返す kv で模し、次の保存の照合で止まることを固定する。実ブラウザで同じタスクの中の再照合がどちらになるかは未確認
- ブラウザ試験 `tools/browser/stale-tab.test.mjs`（PoC-6 S-8 相当。CSP あり・なし）: 2 枚のタブで、A の 1 手が保存され、B の 1 手で保存値が変わり（陽性の対照）、A の次の 1 手はメモリ上だけ進んで保存値は B のまま、A は古いタブになり、A を `pagehide` つきで閉じても保存値は変わらない。`make test-browser` は 586 件が通った（5.169 の 584 件に 2 件）
- 変異注入 `SAVE12-1`〜`SAVE12-3`: 3 件とも DETECTED（`scripts/check-mutations.sh`）。照合を外す・起動番号を増やさない・包みが照合せずに委ねる。当初案の「取得の書き込みの失敗を無視する」変異は、照合の読み取りが二重に止めるため SURVIVED となり、採らなかった。SAVE12-1・SAVE12-3 の find は実装（`Some(v) if v == mine => Ok(true),`・`if !self.guard.verify()? {`）に合わせて直し、2026-10-20 に実際に流して 3 件の DETECTED を確かめた
- 既知の限界: localStorage が使えず IndexedDB だけが使える環境では、起動番号を確かめられないため保存が止まる。SAVE-32 の教材ごとの削除は meta の鍵を消さない（値は小さい）。meta の鍵は `HistoryStorage` の使用量の見積もりにわずかに含まれる。実ブラウザでの同時の起動の競合（`tag` の効果）は試していない（未検証）。照合と書き込みはタブ間で原子的でなく、照合の直後に別のタブが取得すると古いタブが 1 回だけ書きうる（`docs/known-gaps.md` の SAVE-12）。競合の窓はブラウザ試験の対象外（`stale-tab.test.mjs` と #605 は逐次の場面だけを確かめる）
- 範囲外: 新しいタブの操作が再読み込みで戻ることの試験は TASK-324 2/2（5.177 で済み）。古いタブに出す「保存されていない」表示は 5.178 で実装。合流の規則（D-56）は採らない

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

件数の基準値は `scripts/expected-test-counts.txt` に固定する（束ごとの passed・ignored と合計。更新日と元にしたコミットは先頭の注釈）。
この文書に件数の表を二重に持たない。2026-10-14 の 42 件の表は古くなったため、2026-10-20 に参照へ置き換えた。
更新は `make update-test-counts`（遅い）で、版を上げる前に別の PR で行う。

#### 6.3.1 期待値の更新の方針（2026-10-23。#1764）

試験を足す・減らす PR が期待値を書き換えると、並行する PR の分と合計の行がずれて衝突する（#593 / PR #1751 の事例）。
照合は版上げ PR か期待値を変えた PR に限られる（`scripts/check-frontend-bump.sh` の冒頭）ため、更新の担い手を次に絞る。

| 決めたこと | 内容 |
|---|---|
| 試験を足す・減らす PR | 期待値ファイルを変えない（既定）。件数の照合はその PR では skip され、実際の件数は log に出る（6.6 の判定表の「いいえ／いいえ」）。5.x の記録には「期待値は更新していない」と書く |
| 期待値を変える PR | 期待値ファイルだけを変える専用の PR に限る。機能・試験の変更と混ぜない |
| いつ | (a) 版上げの前（6.6.2「事前の PR」の 1）。(b) 期待値の追従を目的に立てた Issue があるとき |
| 誰が | (a) は版上げの Issue の担当。(b) はその Issue の担当 |
| 作り方 | 最新の `origin/main` から作ったブランチで `make update-test-counts` を流す。行を手で足し引きしない（先頭の注釈の元のコミットと実測が対応しなくなるため） |
| main が進んだとき | マージ前に main へ試験が入り CI の `counts` が「test counts differ」で失敗したら、main を取り込んで `make update-test-counts` を流し直す |
| 版上げ PR | 期待値を変えない（`guard` が拒む。6.6） |

### 6.4 node:test の件数

`tools/browser/*.test.mjs`（`make test-browser`）の件数は、同じ `scripts/expected-test-counts.txt` の `browser|total|...` の行に固定する。
`tools/serve.test.mjs`（`make test-serve`）と `tools/measure/browser.test.mjs` は、版上げの受け入れ確認の対象にしていない。
`make test-keydown-control` は、feature を外した写しで `keydown-action.test.mjs` が `ACTION-KEYDOWN-NOT-DISPATCHED` で失敗することを確かめ、成立した（2026-10-14）。

### 6.5 未計測のもの

- CI での実行結果。wasm32 のビルドとブラウザの試験は CI に組み込まれていない（上表はローカルの計測）
- `tools/measure/` の shell を伴う試験（`make test-measure`・`make test-measure-wasm-size`）の件数

### 6.6 版上げ PR の受け入れ確認（FEND-17・D-63・TASK-101）

`fandhe-frontend` の版上げ PR は、受け入れ確認 (1)〜(5) をすべて CI の確認にし、1 つでも外れたら通さない（D-63。2026-10-20）。
本リポジトリでは、既存のジョブへの step の追加で実現する。ジョブの集合・名前・`ci-complete` の `needs`・ruleset の必須チェックは変わらない。

| 確認 | 守る場所 | 備考 |
|---|---|---|
| (1) ビルド・clippy・fmt | `rust-lint`（`make lint`）・`rust-test`（`make test`）・`browser-test`（`make test-browser` が wasm32 の配信設定と `test-hooks` をビルド） | 全 PR で流れる |
| (2) 既存の試験の件数 | `rust-test` の `counts rust`・`browser-test` の `counts browser`（`scripts/check-frontend-bump.sh`）。期待値は `scripts/expected-test-counts.txt` | 下の判定表。TASK-101 1/2 |
| (3) WASM の gzip +10% | `browser-test` の `wasm-gzip`（`scripts/check-frontend-bump.sh`）。版上げ PR のときだけ base と HEAD をビルドして比べる | 下の 6.6.1。TASK-101 2/2 |
| (4) `check-deps.sh` の例外の数 | `rust-lint` の `make check-dom-reads`（`EXC_N=1`。増減とも失敗）と、`guard` による `tools/check-deps.sh` の固定 | 版上げ PR で例外の定義を緩めて通すことを禁じる |
| (5) `wasm-full` の feature の明示 | `crates/web/tests/frontend_features.rs`（`rust-test`）と、`guard` の `cargo metadata` の確認 | `guard` は版上げ PR かどうかに関係なく毎回流す |

版上げ PR の見分け方: base（`HEAD^1`。PR の merge commit の第 1 親）と `HEAD` で、`Cargo.lock` の `fandhe-frontend-*` の `name`・`version`・`source`・`checksum`、ルートと `crates/web` の `Cargo.toml` の `fandhe-frontend-*` の行のどれかが違えば版上げ PR とする。

件数の照合は、PR のイベント（`pull_request`）のときだけ行う。

| 版上げ PR | 期待値ファイルを変えた | 結果 |
|---|---|---|
| はい | はい | 失敗。版上げ PR の中で期待値を書き換えて試験の減少を隠すことを禁じる |
| はい | いいえ | 件数を照合する。差があれば失敗 |
| いいえ | はい | 件数を照合する（期待値を更新する PR の値を CI で確かめる） |
| いいえ | いいえ | skip（実際の件数を log に出す） |

判定できない場合（base が無い・差分の取得の失敗・`test result` の行が 0 件・TAP の集計の欠落・失敗した試験がある）は、終了コード 2 で失敗する（fail-closed）。
版上げ PR で変えてはいけないファイルは、期待値ファイル・`tools/check-deps.sh`・`scripts/check-frontend-bump.sh`・`tools/build-web.sh`（(3) の測定の手順を守る）である。
限界: `ci.yml` 自体の書き換えは防げない（版上げで `wasm-bindgen` の版を合わせる正当な変更があるため）。ai-review とレビュースレッドの解決必須で補う。
同名の結合試験が crate をまたいで重複する束は合算して数える。
期待値は main の上では「最後に更新した基準値」であり、版を上げる前に別の PR で更新する。手順書は 6.6.2 に置く（TASK-363）。

#### 6.6.1 (3) WASM の gzip +10% の判定（TASK-101 2/2）

`scripts/check-frontend-bump.sh wasm-gzip` が、版上げ PR のときだけ、同じ runner・同じジョブの中で base（`HEAD^1`）と `HEAD` を一時の worktree に取り出して `tools/build-web.sh release` でビルドし、配信用 WASM の `gzip -9` の大きさを比べる。
`head * 100 <= base * 110` なら合格（ちょうど +10% は合格）、超えたら終了コード 1 で失敗する。base・head の sha・大きさ・差分・千分率を log に出す。

| 決めたこと | 内容と理由 |
|---|---|
| 基点の取り方 | repo に値を記録せず、同じジョブで base と head の両方を測る。記録した値は版上げ以外の PR で古くなり、手元（`wasm-opt` あり）と CI（なし）で基点が食い違うため（`docs/measurement.md` の 6 章が禁じる比較） |
| `wasm-opt` | CI へ binaryen を入れない（供給網の面と SHA256 の固定が増えるため）。両側とも `SKIP_WASM_OPT=1` を明示し、`wasm-opt` なしの基点で比べる。両側の `wasm-opt:` の行が違えば判定不能 |
| 基点を揃えられない場合 | `rust-toolchain.toml` か `Cargo.lock` の `wasm-bindgen` の版が base と違えば、終了コード 2 で失敗し、別の PR で先に上げるよう案内する（CI は 1 つの版の CLI しか持たない） |
| fail-closed | ビルドの失敗・ログの行の欠落や重複・配信の設定でない・ログの値と自分で測った値の不一致・base の値が 0 以下は、すべて終了コード 2 |
| skip | `pull_request` でないとき、版上げ PR でないときだけ。後者はビルドしない |
| 回帰テスト用の口 | `--base-build-log`・`--head-build-log`（ビルドを省いてログの解析と判定だけを行う）。`--assume-bump` は版上げの検出を飛ばして測る（手元の確認用）。CI はどれも渡さない |

限界: `ci.yml` 自体と `Cargo.toml` の `[profile.release]` の書き換えは防げない（後者は版上げで `Cargo.toml` を変えるのが正当なため）。ai-review とレビュースレッドの解決必須で補う。
`wasm-opt` を適用した基点での比較は将来の検討事項である。

#### 6.6.2 版上げの手順書（TASK-363）

D-63（2026-10-20）により、(1)〜(5) は版上げ PR の CI の確認である。本節の手順書は CI の代わりにしない。
補うのは次の 3 つだけである。

- (a) 版を上げるかどうかの判断（人が決める）
- (b) (3) の基点と (2) の期待値を、版上げ PR の前に揃える手順
- (c) CI が外れたときの対処

##### 判断（人が決める）

- 版を上げるかどうか（Issue を立てて着手するか）は人が決める。エージェントは材料を揃えるところまでを行う
- 材料: 上流の変更点（破壊的変更・セキュリティ修正・必要な機能）、`make inventory-deps` による取り下げと所有者の確認（`docs/dependency-inventory.md`）、新しい版が要求する `wasm-bindgen` と Rust toolchain の版、6 crate の版の組（6.1）
- 閾値（+10%・例外の数・件数）は FEND-17 の契約である。版上げ PR の中で緩めない。変える場合は spec 側の決定が先に要る

##### 事前の PR（版上げ PR より前に、別の PR で main に入れる）

1. **(2) の期待値**: `scripts/expected-test-counts.txt` の先頭の注釈（更新日・元のコミット）が古い場合は、`make update-test-counts` で更新する。遅く、ブラウザ試験も流れる（`npm ci` と `npx playwright install chromium` が前提）。期待値だけを変える PR にする（方針は 6.3.1）。この PR では CI が件数を照合する（6.6 の判定表の「いいえ／はい」）
2. **(3) の基点**: 新しい版が新しい `wasm-bindgen` か Rust toolchain を要求する場合は、先に上げる（`docs/toolchain.md` の第 5 節。`web-sys`・`js-sys` の版も揃える。`ci.yml` の CLI の版と SHA256、`rust-toolchain.toml` も同じ PR で上げ、`make check-toolchain` を通す）。揃えないと `wasm-gzip` が終了コード 2 で止まる
3. **守るファイル**: `tools/check-deps.sh`・`tools/build-web.sh`・`scripts/check-frontend-bump.sh`・期待値ファイルを変える必要があれば、別の PR で先に入れる（`guard` が版上げ PR での変更を拒む）

(3) の基点は repo に記録しない。CI が同じジョブで base（`HEAD^1`）と `HEAD` をビルドして比べるため（6.6.1）。手元の値（`wasm-opt` あり）と CI の値（`SKIP_WASM_OPT=1`）は基点が違うので、比べない。

##### 版上げ PR の作り方

1. ルートの `Cargo.toml` の `[workspace.dependencies]` で `fandhe-frontend-*` の版を上げる。直接の依存の版は `Cargo.lock` の解決済みの版と同じにする
2. 宣言を保つ。`wasm-full` は `default-features = false` と `features = ["action-keydown"]`（(5)）、`wasm-client` は `default-features = false`
3. `cargo update -p <crate> --precise <版>` で `Cargo.lock` を更新し、`--locked` でビルドが通ることを確かめる
4. 同じ PR で `crates/web/tests/frontend_features.rs` の `ADOPTED` と 6.1 の表を更新する
5. 守るファイルの変更と無関係な変更を混ぜない。`fandhe-frontend-*` の行に触れると版上げ PR として検出される（6.6）

##### push 前の手元の確認（CI の先取りであり、代わりではない）

`<log>`・`<tap>` は repo の外の一時パスにする。`origin/main` を base とする。

| 確認 | 手元のコマンド |
|---|---|
| (1) | `make lint`・`make test`・`make test-browser` |
| (2) Rust | `make test 2>&1 \| tee <log>` のあと `CI_EVENT_NAME=pull_request scripts/check-frontend-bump.sh --base origin/main counts rust <log>` |
| (2) ブラウザ | `FANDHE_TEST_TAP_OUT=<tap> make test-browser` のあと `CI_EVENT_NAME=pull_request scripts/check-frontend-bump.sh --base origin/main counts browser <tap>` |
| (3) | `CI_EVENT_NAME=pull_request SKIP_WASM_OPT=1 scripts/check-frontend-bump.sh --base origin/main wasm-gzip`（base と head を 2 回ビルドするため遅い） |
| (4) | `make check-dom-reads` |
| (5) | `CI_EVENT_NAME=pull_request scripts/check-frontend-bump.sh --base origin/main guard`（`make check-frontend-bump` は feature の明示だけを確かめる） |
| 参考（(1)〜(5) の外） | `make check-wasm-size`（1 MB の関門。TASK-78） |

##### CI が外れたときの対処

| 外れたもの | 対処 |
|---|---|
| (1) ビルド・lint・試験の失敗 | 版上げ PR の中で上流の API の変化に合わせて直す。`#[allow]` は理由つきでだけ使う（`coding-rust.md`） |
| (2) 終了コード 1「test counts differ」 | 試験が増減した原因を版上げの中で突き止める。期待値を版上げ PR で書き換えない（`guard` でも失敗する）。期待値が古いだけなら、別の PR で `make update-test-counts` を入れ、版上げ PR を base に追従させる |
| (2) 終了コード 2 | 失敗した試験・集計の欠落を直す |
| (3) 終了コード 1「grew ... more than +10%」 | 増えた原因（feature の取り込み・上流の変化）を調べる。feature を絞るか、版を見送る（人が決める）。閾値は変えない |
| (3) 終了コード 2 | toolchain・`wasm-bindgen` が base と違う場合は、事前の PR の 2 を先に入れる。base の欠落などの CI 側の前提の欠落は CI の不具合として扱う |
| (4) `EXC_N` の不一致 | DOM を読むコードを直す。`tools/check-deps.sh` を版上げ PR で変えると `guard` が拒む |
| (5) 「must be declared with default-features = false ...」、または `frontend_features.rs` の失敗 | 宣言を戻す。`ADOPTED` と `Cargo.lock` を揃える |
| `guard`「must not change <file>」 | その変更を別の PR に分ける |

禁止: `ci.yml` の step を外す・`if:` で飛ばして通すこと、`--no-verify`。`ci.yml` の書き換えは関門で防げない（6.6 の限界）ため、ai-review とレビュースレッドの解決必須で補う。

##### 対応表（机上の照合。2026-10-20、元にしたコミット: a1995f8）

| | FEND-17 の期待 | 版上げ PR での CI の確認（TASK-101。ジョブ・step） | 手元の先取り | 外れたときの対処 |
|---|---|---|---|---|
| (1) | ビルド・clippy・fmt が通る | `rust-lint`（`make lint`）・`rust-test`・`browser-test` | 手元の表の (1) | 「(1)」の行 |
| (2) | 既存の試験の件数が減らない（期待値と一致） | `rust-test` の `Check Rust test counts (version bump gate)`・`browser-test` の `Check browser test counts (version bump gate)` | (2) Rust・(2) ブラウザ | 「(2)」の 2 行 |
| (3) | 配信用 WASM の gzip の増加が +10% 以内 | `browser-test` の `Check WASM gzip growth (version bump gate)` | (3) | 「(3)」の 2 行 |
| (4) | `check-deps.sh` の例外の数が増えない | `rust-lint` の `make check-dom-reads` と `make check-frontend-bump`（`guard`） | (4) | 「(4)」の行と `guard` の行 |
| (5) | `wasm-full` の feature の明示 | `rust-test` の `frontend_features.rs` と `rust-lint` の `make check-frontend-bump`（`guard`） | (5) | 「(5)」の行 |

### 5.172 章ごとの保存先の鍵で別の章の履歴が混ざらない（TASK-326 1/2。SAVE-14・DLV-8）

実行日: 2026-10-20、元にしたコミット: `a7c3363` に本変更を載せた作業ツリー（dirty）。

試作の F-1（同じオリジンで同じ教材の別の章のリンクを順に開くと、先に保存した別の章の履歴が出た）が、章ごとの保存先の鍵で起きないことを確かめる。鍵の規則（`HistoryKey::from_ir`。`pack.id` と `starts[0].id` だけを読む）と、本番の経路への結線（`EngineHost::history_key`・`persist::mount_with_history`）は実装済みのため、本タスクは試験と対照だけを足し、本番のコードは変えていない。

#### 章の教材（試験の設定だけ）

`src/test_fixture.rs` の `chapter_ir_bytes`。8 Step の教材（`tests/data/reload_points_ch2/ir.json`）の `pack.id` を `chapter-links` に置き換えたものを章 A（`starts` は元のまま。開始点 `s-1`）、さらに `starts` を `s-4`（`main/st-4`。選択肢 1 手で受付になる Step）の 1 件に置き換えたものを章 B とする。新しい試験データのファイルは作らない。`?fixture=chapter-a`・`?fixture=chapter-b` の完全一致だけが選ぶ。2 本は `pack.id` が同じで、開始点・内容のハッシュ・鍵が違う（単体試験が確かめる）。

#### native の試験と変異

- `crates/web/tests/chapter_keys_host.rs`（2 件）: 章 A を保存してから章 B を開くと章 B は `Fresh` で章 B の開始の Step から始まり、章 A の値は変わらず、章 B も保存でき（鍵がちょうど 2 本）、それぞれの鍵で開き直すと `Restored` で保存した時点の Step に戻る。別の教材の鍵は 2 本のどちらとも違い、開いても `Fresh` で鍵が増えない
- 変異（`scripts/check-mutations.sh` の結果は 2 件とも DETECTED）

| ID | 置き換え | 失敗した試験 |
|---|---|---|
| `SAVE14-4` | 鍵が `starts[0].id` を無視して `pack.id` だけになる | `chapter_keys_host` の 2 件 |
| `SAVE14-5` | 教材にも章にも依らない固定の鍵（試作の修正前に当たる） | `chapter_keys_host` の 2 件 |

#### ブラウザ試験

`tools/browser/chapter-keys.test.mjs`（CSP あり・なし。2 件）。1 つの browser context で、章 A を 1 手進める → 同じタブで章 B（`fresh`・章 B の開始・`reloaded` の行 0 件・章 A の値の長さ不変）→ 章 B を 1 手進める（履歴の鍵がちょうど 2 本）→ 章 A・章 B へ戻る（`restored`・保存した Step）→ 各章で再読み込み（同じ）→ 別の教材（既定。`fresh`・鍵が 3 本目）を確かめる。章 B を開いた直後の判定が外れたときは固定の印 `CHAPTER-HISTORY-MIXED` を失敗の文に含める。出力に保存の値・鍵の文字列・下書きを出さない（有無・長さ・本数・比較の結果だけ）。

#### 修正前の鍵との対照（ブラウザの層）

`make test-chapter-key-control`（`scripts/test-chapter-key-control.sh`。遅く、CI・フック・`make check` には入れない）。`mktemp -d` の写しで `host.rs` の `history_key` の式を固定の鍵（`SAVE14-5` と同じ）に置き換え、`TEST_HOOKS=1` でビルドして上の試験を流す。作業ツリーは書き換えない。

- 結果（2026-10-20）: 終了コード 0（対照が成立）。写しでは試験が `CHAPTER-HISTORY-MIXED` の印で失敗した。置き換えた鍵では章 A の履歴が章 B の起動で開かれ、章 B は新しい状態で始まらない（章 B の `bootKind`・現在の Step の観測値は記録していない）
- 試験の口が求める鍵（`storageKey`）は本番の `history_key` を通らないため、対照では画面が実際に書いた鍵を見て章 B の判定へ進めるようにしてある

#### 件数・確認（2026-10-20）

- `make lint`・`make test`（2,653 件・失敗 0）・`make check-deps`・`make check-dom-reads`・`make check-generic-words`・`make check`・`make check-delivery`（`make build-web` の後）は通った
- `make test-browser`: 592 件中 591 件が通り、1 件（`safety.test.mjs` の「対照: タブの異常終了（chrome://crash）が数えられる」）が失敗した。本変更が触れない試験で、実行環境の Chromium の異常終了の扱いによる可能性がある。base での再現は未確認（未検証）。`chapter-keys` の 2 件は通った
- `scripts/expected-test-counts.txt` は変えていない（CI の件数の照合は版上げ PR か期待値の変更のときだけ働く）

#### 範囲外

- 教材の版・内容が変わっても鍵が変わらないことの画面の確認と、IR が読めないときの鍵 `unreadable/<IR の名前>`（TASK-326 2/2）
- 配信先の区間による別の配信先との衝突（SAVE-15。TASK-327）
- 起動時の変換の告知の表示（未実装。本試験は告知の行が出ないことだけを見る）

### 5.173 教材の版・内容が変わっても鍵が変わらず、読めない IR で保存先を書き換えない（TASK-326 2/2。SAVE-14・DLV-8）

実行日: 2026-10-20、元にしたコミット: `ec7d025` に本変更を載せた作業ツリー（dirty）。

保存先の鍵は教材の ID と最初の開始点の ID だけで決まり、教材の版・内容に依らない（試作 F-1）。契約の層（`HistoryKey::from_ir`・`HistoryKey::unreadable`・`SAVE14-1`〜`SAVE14-3`・`SAVE15-2`）は固定済みのため、本タスクは web の層（ホスト・保存の経路）と実際の画面で、次の 2 点を確かめる。

1. 教材の版・内容が変わっても鍵が変わらない（同じ鍵で保存済みの履歴が読まれ、新しい鍵が増えない）
2. 読めない IR で保存済みの履歴を書き換えない。IR が読めないときの鍵が教材の鍵の範囲（`pack_prefix`・`belongs_to`）に入らない

#### 本番のコードを変えない理由

5.172 と同じく、試験・試験の設定・文書だけを足し、本番のコード（`host.rs`・`persist.rs`）は変えていない。

- 配信する設定には「IR の名前」が無い（D-68 で場所は固定の `./ir/main.json` になり、URL から名前は生まれない）。読めない IR の鍵 `unreadable/<IR の名前>` は、本番の経路ではまだ作れない
- 今の経路は、仕様の鍵より厳しい fail-closed である。`EngineHost::refused` のとき `history_key` は `None` で、`persist::mount_with_history` は保存先を開かず、`pagehide` の書き出しも付けない。このため「読めない IR で保存済みの履歴を書き換えない」は、鍵を経由するより厳しく満たされる
- 読めない IR に鍵を与えて通常の mount の経路へ流すと、断った画面で保存先への書き込み（起動番号の記録等）が起きうる。5.140 の「保存先が不変」を壊すため、結線は実行時に `./ir/main.json` を取得する処理が入るときに決める（TASK-168 では結線しない）

#### 教材の変種（試験の設定だけ）

`src/test_fixture.rs` の `Chapter::ANext`。章 A と `pack.id`・`starts` が同じで、`pack.version` を `0.1.0`（主版は 0 のまま）、題名を別の文字列にしたもの。内容のハッシュは章 A と違い、鍵は同じ（単体試験が確かめる）。`?fixture=chapter-a-next` の完全一致だけが選ぶ。新しい試験データのファイルと、新しい試験用の口は足さない。

#### native の試験と変異

- `crates/web/tests/key_stability_host.rs`（4 件）: (a) 版・題名を変えた教材（同じ主版）が同じ鍵で `Restored` になり、保存しても鍵が 1 本のまま。(b) 主版を変えた教材は同じ鍵で読んだうえで `NotOpened` になり、`history_to_save` が `None` で保存先は 1 文字も変わらない。(c) 読めない IR のホストは `history_key` が `None`・`boot` が `NoSession`・保存する値が無く、保存先は不変。(d) `HistoryKey::unreadable` の鍵は教材の接頭辞で始まらず `belongs_to` が偽で、保存先に置いても教材の履歴の起動は変わらない
- 変異（`scripts/check-mutations.sh` の結果は DETECTED）。既存の `SAVE14-4`・`SAVE14-5` も DETECTED のまま

| ID | 置き換え | 失敗した試験 |
|---|---|---|
| `SAVE14-6` | web の鍵に教材の版（`pack.version`）を混ぜる | `key_stability_host` の (a)・(b) |

#### ブラウザ試験

`tools/browser/key-stability.test.mjs`（CSP あり・なし。2 件）。1 つの browser context で、章 A を 1 手進める（履歴の鍵が 1 本）→ 版・題名を変えた教材へ移る（鍵は 1 本のまま・`restored`・Step と各 Step の状態が同じ）→ もう 1 手進める（鍵は 1 本のまま）→ 再読み込み（同じ）→ 読めない IR（`?fixture=future-ir-format`）へ移る（断った画面のマウント後の localStorage の全体が、断っている間と `pagehide` の後で一致し、鍵は増えない）→ 章 A へ戻る（`restored`）を確かめる。版が変わって鍵が変わったときは固定の印 `KEY-CHANGED-BY-VERSION` を失敗の文に含める。出力に保存の値・鍵の文字列・下書きを出さない。`chapter-keys` は 5.172 の対照（`test-chapter-key-control`）が前提とするため、手順は足さず別のファイルにした。

#### 件数・確認（2026-10-20）

- `make lint`・`make test`（2,657 件・失敗 0）・`make check-deps`・`make check-dom-reads`・`make check-generic-words`・`make check`・`make check-delivery`（`make build-web` の後）は通った
- ブラウザ試験は、`TEST_HOOKS=1` の web を作り `key-stability`・`chapter-keys`・`ir-refusal` の 3 ファイル（計 10 件）を単独で流し、すべて通った。`make test-browser` の全体は流していない（未検証）
- `make test-chapter-key-control` は流していない（未実施。本変更は `chapter-keys` に触れない）
- `scripts/expected-test-counts.txt` は変えていない

#### 限界

- 読めない IR の鍵 `unreadable/<IR の名前>` を本番の経路で使うのは、実行時に `./ir/main.json` を取得する処理が入るときに結線を決めてから（D-68。TASK-168 では結線しない）。それまでは保存先を開かない fail-closed で満たす
- `load_ir` は `starts: []` を受ける。このとき `HistoryKey::from_ir` は `None` で、ホストは保存なしで mount する（契約の doc comment は `unreadable` へ倒すと書くが、web は倒していない）。作成側は `validate` が `E-STRUCTURE` で拒否する（PACK-25）ため、配られる IR では起きにくい。名前が無い点は同じで、実行時の扱いは先送りする

#### 範囲外

- 配信先の区間による別の配信先との衝突（SAVE-15。TASK-327）
- 起動時の変換の告知の表示（未実装。本試験は告知の行が出ないことだけを見る）

### 5.174 保存領域が失われたときの案内（TASK-340。SAVE-29・D-57）

元にしたコミット: `ffaf5f3`（実行日 2026-10-20）。受講者が保存領域を消す・ブラウザが自動で削除する・保存できない状態で開く場面では、再び開くと新しい状態で始まる。そのとき履歴を失わない手段として、書き出しを案内する通知欄 `export-notice`（`role="status"`。`LogPart` の末尾の束縛点 `export_notice`）を足した。

#### 起点の規則（仮置き）

- 初回の保存時（`FirstSave`）: 起動が `Fresh`（保存済みの履歴が無い・読めない）だったページで、最初に状態を変える操作の `update` の末尾に出す。保存を試す時点と同じで、保存を止めた状態（シークレットなど）でも出す（成功を起点にすると、失う場面で案内が出なくなるため）。復元（`Restored`）・下書きだけの変更・2 回目以降の操作では出さない
- 章の終わり（`ChapterEnd`）: その操作で伸びたログに、`main` の根の最後の Step（`is_final`）の `Achieved`、または `ChapterCompleted` が現れたとき。起動時に復元したログの分は見ない。学習モジュールの最後の Step は呼び出し元へ戻るため数えない。同じ操作で両方が起きたときは `ChapterEnd`
- 案内は表示の状態で、`ops`・保存先に書かない（新しい保存の鍵も作らない）。次の起点・履歴の差し替え（`open_history`・`install_recorded`）で置き換わり、再読み込みでは出ない

#### 未決の点

- 文面・回数・消える時機は未決（D-57 は「実装のタスクで案を出し、ユーザーが決める」）。本実装は仮置きで、変えるときは `crates/web/src/export_notice.rs` の定数と規則だけを直す。試験は文そのものでなく「空か・種類で違うか」を固定する
- 書き出しの操作（リンク・ボタン）は TASK-531、通知欄の配置は TASK-534 が持つ。本タスクは文だけを出す

#### native の試験と変異

- `crates/web/tests/export_notice.rs`（7 件）: 初回の案内と 2 手目で動かないこと・dirty、復元では出ないこと、保存を試せない起動でも出ること、章の終わり、途中の Step の達成では章の終わりにならないこと、復元した章の終わりは起動の直後に出ず履歴の差し替えで消えること、文に教材の値が含まれないこと
- 変異（`scripts/check-mutations.sh` の結果はすべて DETECTED）

| ID | 置き換え | 失敗した試験 |
|---|---|---|
| `SAVE29-1` | 初回の案内を立てない | `export_notice` の初回・保存を試せない起動 |
| `SAVE29-2` | 復元した起動でも初回の案内を出す（`fresh_start` の条件を外す） | 復元の試験 |
| `SAVE29-3` | 最後の Step でなくても章の終わりにする（`is_final` の判定を外す） | 途中の Step の達成の試験 |
| `SAVE29-4` | 初回の案内の文を空にする | 初回の案内の試験 |

#### ブラウザ試験

`tools/browser/storage-lost.test.mjs`（CSP あり・なし。8 件）。(A) 1 手を送って保存し、CDP の `Storage.clearDataForOrigin` で保存領域を消して開き直すと、`fresh`・世代 1・`reloaded` 0 件・最初の Step・鍵なしで始まり、案内は操作の前は空・最初の操作で再び出る。(C) `localStorage` の取得が例外・`indexedDB` なしの状態でも `fresh` で始まり、最初の操作で案内が出て、再読み込みしても `fresh`・世代 1。(D) `load-fixture:approval` を最後まで進めると章の終わりの案内が出る（途中は空）。(E) 対照として、消さずに再読み込みして `restored` になった後は操作しても案内が出ない。いずれも CSP 違反・pageerror・console error は 0 件。

#### 件数・確認（2026-10-20）

- `make lint`・`make test` は通った。`storage-lost` の 8 件は `TEST_HOOKS=1` の web を作って単独で流し、すべて通った。`make test-browser` の全体は流していない（未検証）
- `scripts/expected-test-counts.txt` は変えていない（件数の照合は版上げ PR のときだけ）

#### 限界

- (B) ブラウザの自動削除（eviction）は本物を起こせない。保存済みの履歴が消える点で同じ状態になる CDP の消去で代える（`storageTypes: "all"` は IndexedDB も含む）
- (C) では保存を止めるため `save-notice` は出ない（保存を試さないため。SAVE-10・SHELL-18 の範囲）
- `ChapterCompleted` は core がまだ出さない（TASK-233 以降）ため、`main` の最後の Step の達成で代える。TASK-339 が章の終わりの判定を別に定めたら一本化する
- `load-fixture`（試験の設定だけ）は `install_recorded` を通るため初回の印を落とす。(D) の途中で初回の案内が出ないのはこのため（本番の経路には無い）

#### 範囲外

- 書き出しの可視の操作と案内からの導線（TASK-531）・通知欄の配置（TASK-534）・案内の確定（ユーザー）
- 受講者による履歴の削除（SAVE-32・TASK-341）

### 5.175 Step の本文への注入 49 件（TASK-144。SAFE-1）

元にしたコミット: `fe45d81`（実行日 2026-10-20）。教材パックの Markdown（Step の本文）に注入用の試験入力を入れても、画面で何も実行されず、生の HTML が文字として出ることを確かめる試験 `tools/browser/pack-body-xss.test.mjs` を足した（1/2。#536）。試験 B は CSP あり・なしの両方で 49 件を流す（CSP なしは 2/2。#537。下の「CSP なし」）。

#### 何を確かめるか

データの正は `xss-cases.md` の「入り口に E1 を含む」49 件（`xss-cases.mjs` の `e1Payloads`。件数が違えば例外）。1 件ごとに `min-3step-pack` を一時ディレクトリへ写し、`st-1.md` をペイロードで置き換えて実物の `fandhe-pack compile` を流す（終了コード 0・IR 版 2・`st-1` の `body_doc` を確かめる。拒否は飛ばさず失敗にする）。

| 試験 | 内容 |
|---|---|
| A | 試験の設定と配信する設定の `index.html` の CSP の meta が、各 1 件で同一 |
| B | CSP あり・なしの各 49 件ごとに新しい context で `loadIr` → `installCount` の増加と `current` が `st-1` を確かめ、本文の木を関門と描画に通した HTML を容器へ入れ、計数の全区分が 0。生の HTML（`text` の節で `<` を含むもの）が `textContent` に含まれ（U+0000 は除く。確かめた数は 12 件以上）、`img` が 0 件で、`a[href]` は同一オリジンか http(s) だけ |
| C | B で得た 49 件の HTML を、試験の口の無い配信する設定のページへ入れても全区分 0 |
| D | 対照。描画を通さない生の入力（XSS-01・XSS-07）は、数え方が 0 でなく数える |

#### 注入の口と代え方

1/2・2/2（#536・#537）の時点では、配信する設定が IR を読まず本文も枠に出していなかったため、試験の設定の `loadIr` と、`checkDoc` → `renderDoc` の文字列を `#app` の外の容器へ入れる経路で代えていた。この口は TASK-525 2/2（#712）で、配信する設定の画面の実際の本文の区画を見る形へ差し替えた（下の「本文の区画への差し替え」）。

#### 観察（通るのが正しい件）

- XSS-29: `/a` の相対リンクは同一オリジンの `a` になる
- XSS-45: 全角の `：` の形は相対として `a` になり、基点の下の同一オリジンに解決される
- XSS-47: U+0000 は構文解析で落ちるため、除いて比べる
- XSS-20・26: 外部・スキーム相対の画像は `img` にならず代替の文字になる

#### 限界

- 旧版の限界（枠の本文の区画が無く配信する設定が IR を読まない）は、下の「本文の区画への差し替え」で解消した
- 49 件は網羅ではない

#### 件数・確認（2026-10-20）

- `pack-body-xss` の 4 件と `safety-observe` の 7 件は、`TEST_HOOKS` ありなしの 2 つの web を作って単独で流し、すべて通った（確かめた生の HTML の文字は 66 箇所）
- 手元の変異: 描画した HTML の `&lt;` を `<` に戻して容器へ入れると、B・C が失敗した（戻して再び通ることも確認）
- `make test-browser` の全体は流していない（未検証）。`scripts/expected-test-counts.txt` は変えていない

#### CSP なし（TASK-144 2/2。2026-10-20）

元にしたコミット: `ef137ee`。試験 B のループを `[true, false]` にし、`index.html` の CSP の meta を除いた一時の写し（`withCspMode`）で同じ 49 件を流す。CSP に頼らず、描画の経路（`checkDoc` → `renderDoc`）だけで全区分が 0 になることを確かめる。受け入れ条件の語と計数の対応は、実行 = `hits`、外部通信 = `external`、ダイアログ = `dialogs`・`popups`、危険な要素 = `danger`、`on*` 属性 = `onAttrs`、`style` 属性 = `styleAttrs`、`http(s)` の出現 = `extHttp`・`nonHttp`、ページ内エラー = `pageErrors`・`crashed`・`scanError` である。

- 単独の実行: A・B（CSP あり）・B（CSP なし）・C・D の 5 件がすべて通った。確かめた生の HTML の文字は、B の 2 回とも 66 箇所
- 手元の変異（描画した HTML の `&lt;` を `<` に戻して容器へ入れる）: B（CSP あり）・B（CSP なし）・C が失敗した（B は XSS-01 で生の HTML が文字として出ていないことを検出）。戻して再び通ることを確認した。実行の計数そのものが CSP なしで数えられることは、試験 D（対照）が担う
- ブラウザ試験の件数は 1 件増える。`scripts/expected-test-counts.txt` は変えていない（先行の #1648 と同じ扱い。版上げの前に `make update-test-counts` で更新する）
- `make test-browser` の全体は流していない（未検証）。CI の `browser-test` ジョブで確かめる

#### 本文の区画への差し替え（TASK-525 2/2。#712。2026-10-23）

元にしたコミット: `e973589`。代わりの口 `showStepBody` をやめ、配信する設定の web の一時の写しに `<meta name="fandhe-ir" content="1">` と `ir/main.json`（件ごとに実物の `fandhe-pack compile` で作った IR）を足して開き、`?start=` なしで `starts` の先頭（st-1）が開く画面の本文の区画（`#app [data-testid="step-body"]`）を観察する形にした。試験側は区画へ何も入れない。写しは `mkdtemp` の一時ディレクトリで、前提（`</head>` が 1 個・`fandhe-ir` と `fandhe-test-hooks` の meta が 0 個・CSP の meta が 1 個）が崩れれば失敗にする。件ごとにサーバを起動し直す（`tools/serve.mjs` が gzip の結果をサーバごとに持つため）。

| 試験 | 内容 |
|---|---|
| A | 変更なし |
| B | CSP あり・なしの各 49 件で、試験の口が無いこと・`ir-notice` が 0 件・題が IR の Step の題と一致・区画がちょうど 1 個を確かめ、全区分 0・`hits` 0・通信が配信物の構成要素と `ir/main.json` 1 件（200）だけ（`pageRequestProblems`）、生の HTML が文字（66 箇所）、`img` が 0 件、外部の画像が `data-image-blocked` の代わりの文字になること（件数が IR の画像の節の数と一致し、`alt` が文字として出る）、`data-body-level` が `A` |
| C | 試験の設定の `loadIr` で入れた画面の区画の HTML が、B（CSP あり）の配信する設定の画面と同じ（U+0000 は除いて比べる）で、全区分 0 |
| D | 対照。実際の区画へ生の入力（XSS-01・XSS-07）を入れると、数え方が 0 でなく数える |

- 確かめた数（CSP あり・なしとも）: 生の HTML の文字 66 箇所、代わりの文字になった外部の画像 3 件（XSS-20・26 を含む。下限は 2）
- `ir/main.json` の取得は、IR の読み取りが上限で `reader.cancel()` を呼ぶため、応答 200 のまま `net::ERR_ABORTED` として `failed` に載ることがある。この 1 形（`ir/main.json` の ERR_ABORTED）だけを除いて通信を判定する。本文は描画済みである（訂正: 原因の調査は 5.268。上限での `cancel()` は IR が小さいため呼ばれず、この説明は成り立たない。原因は未確定）
- U+0000（XSS-47）は、試験の設定と配信する設定の経路で区画の `innerHTML` に残る・落ちるの差が出る。B は `textContent` から除いて比べ、C は HTML から除いて比べる
- 手元の変異（空振りの防止）: 代替の文字の件数の期待を 1 ずらすと B（2 件）・C が失敗した（戻して再び通ることも確認）
- 限界: 実行時の水準は常に A（`Control::SetGuidance` が未実装。TASK-294）で、B・C の本文の 49 件の確認は行っていない。49 件は網羅ではない。試験の作成者と実装の作成者が同じ AI である
- 件数: ブラウザ試験の件数は変わらない（5 件）。`scripts/expected-test-counts.txt` は変えていない。`make test-browser` の全体は流していない（未検証）。CI の `browser-test` で確かめる

### 5.176 CSP の meta の配置と内容の検査（TASK-151 1/2。SAFE-13）

元にしたコミット: `fe45d81`（実行日 2026-10-20）。`<meta http-equiv>` の CSP は、その要素より後に現れた内容にしか効かない。そのため配信する `index.html` で、CSP の meta が `<meta charset>` の直後にあり、内容が仕様の文字列と完全一致することを、失敗で止める検査として足した。修正前は `crates/web/static/index.html` で viewport の meta が charset と CSP の間にあったため、CSP を charset の直後へ移した（内容の文字列は変えていない）。

#### 判定の規則（fail-closed）

`tools/browser/harness.mjs` の `inspectCspMeta(html)`（正の文字列は `DELIVERY_CSP`。1 か所だけに置く）が次をすべて満たすときだけ合格にする。

- `<head>` と `</head>` がちょうど 1 つずつ
- charset の meta と CSP の meta が、どちらも head の中にちょうど 1 つずつ
- charset の meta の終わりから CSP の meta の始まりまでが空白だけ（「直後」を厳しい側に解釈した。コメントや他の要素を挟むと不合格）
- CSP の meta の `content`（二重引用符）が `DELIVERY_CSP` と完全一致（正規化しない。指令の並び替え・空白の違いも不合格）
- head より前は doctype・`<html>` の開始タグ・空白だけで、head の開始の直後は空白だけを挟んで charset の meta が来る（空白は HTML の空白〔タブ・LF・FF・CR・空白〕に限る）。ブラウザは head に置けない要素や文字が来た時点で head を暗黙に閉じ、後続の meta が body に入って CSP が効かなくなる。開始・終了タグの文字列上の位置だけでは所属を判定できないため、HTML パーサを持ち込まずに構造を限って暗黙の終了を拒否した（`title` など head に置ける要素も charset より前には置かせない厳しい側。2026-10-21 のレビュー指摘で追加）
- 解析できない形は例外にせず問題として返す
- HTML コメント・`script`・`style`・`title`・`textarea`・`noscript` の中身にある meta は数えない（ブラウザでは効かないため。noscript は scripting 有効のブラウザでテキスト扱いになる。同じ長さの空白に置き換えてから解析する。2026-10-20 のレビュー指摘で追加）

#### 試験

`tools/browser/csp-placement.test.mjs`（34 件。ブラウザ不要。`make test-browser` の列挙に加え、CI の `browser-test` ジョブが流す）。配信設定・試験の設定の `index.html`（`FANDHE_WEB_DIR`・`FANDHE_WEB_TEST_DIR`）が合格すること、および実物を加工した写しの対照 30 種（viewport を挟む・`'wasm-unsafe-eval'` を外す・`img-src *`・meta を消す・meta を 2 つにする・charset より前に置く・`http-equiv` を `data-http-equiv` にする・`http-equiv` の値の前後に空白を足す・`http-equiv` の直前の区切りを NBSP にする・`http-equiv`・`charset` の属性名の前に `<` を付ける・`content` を `data-content` にする・コメントを挟む・charset と CSP をまとめてコメントで囲む・CSP だけをコメントで囲む・`script` の中身に置く・charset と CSP をまとめて `template` で囲む・charset と CSP をまとめて `noscript` で囲む・CSP だけを `noscript` で囲む・CSP だけを入れ子の `template` で囲む・head の開始の直後に `div`・文字・NBSP を置く・head の開始と charset の間に `title` を置く・head より前に `body` を始める・head より前に文字を置く・head を空にして閉じ直す・head の開始タグの属性値に後続を飲み込ませる・html の開始タグの属性値に head を飲み込ませる・head より前にコメントを置く）が期待の理由で不合格になることを確かめる。環境変数が無ければ skip せず失敗にする。

#### 件数・確認（2026-10-20）

- 配信・試験の両設定を `SKIP_WASM_OPT=1 tools/build-web.sh debug` で作り、`csp-placement` の 11 件が通った。環境変数なしでは 10 件が失敗する（skip しない）。`harness`・`safety-observe` の 18 件も通った。`make check`・`make test` は通った
- `make test-browser` の全体は流していない（未検証）。`scripts/expected-test-counts.txt` は変えていない
- 2026-10-21（暗黙の head の終了の拒否）: 両設定を `SKIP_WASM_OPT=1 tools/build-web.sh` で作り、`csp-placement` の 27 件と `harness` の 11 件が通った。環境変数なしでは 26 件が失敗する（skip しない）。修正前の `harness.mjs` に戻すと、追加した対照 7 種のうち 6 種が合格してしまう（失敗する）ことを確かめた（head を空にして閉じ直す形は修正前も `head element count` で不合格）。`scripts/expected-test-counts.txt` のブラウザの合計は、CI の実測 628 件（`924b943`）に追加の 7 件を足した 635 件にした
- 2026-10-21（`http-equiv` の前後の空白）: ブラウザは列挙属性の前後の空白を除かず、`" Content-Security-Policy "` は CSP として働かないため、判定から `trim()` を外し ASCII の大文字小文字だけを無視する完全一致にした。対照を 1 種足し、`csp-placement` は 28 件、ブラウザの合計は 636 件にした
- 2026-10-21（タグ・属性の区切り）: ブラウザはタグ名の終わりと属性の区切りを HTML の空白（タブ・LF・FF・CR・空白）だけで判定し、NBSP や垂直タブは名前の一部になる。解析の正規表現の `\s` と `\b` を HTML の空白に置き換え、`http-equiv` の直前の区切りを NBSP にした対照を 1 種足した（修正前は合格してしまう）。`csp-placement` は 29 件、ブラウザの合計は 637 件にした
- 2026-10-21（正の形との完全一致）: 属性の解析の抜け道（`<http-equiv=` のように読めない文字を飛ばして途中から属性名と読む等）に判定を依存させないため、charset と CSP の meta は要素の文字列を正の形（`<meta charset="utf-8">`・`<meta http-equiv="Content-Security-Policy" content="…">`）と完全一致で比べるようにした（属性の順序の入れ替えなど、ブラウザでは有効な形も拒否する厳しい側の規則）。対照を 2 種足し、`csp-placement` は 31 件にした。main の取り込みで増えた試験（`boot_guard` の 21 件・ブラウザの 2 件）を含め、件数の期待値は Rust 2688 件・ブラウザ 641 件にした
- 2026-10-21（開始タグの完全一致）: head の開始タグを `[^>]*>` で切ると、`<head data-x='>` のように引用符つきの属性値の中の `>` で切れ、後続の charset・CSP が属性値に飲み込まれた形（ブラウザでは meta が生成されない）を合格させていた。head より前（doctype・html の開始タグ）と head の開始タグは、原文を正の形（`<!doctype html>`・`<html lang="ja">`・`<head>`。間は HTML の空白だけ）と完全一致で比べるようにした（head より前のコメントも拒否する厳しい側の規則）。対照を 3 種足し、修正前の `harness.mjs` では 3 種とも合格してしまうことを確かめた。`csp-placement` は 34 件、ブラウザの合計は 644 件にした

#### 限界・範囲外

- `tools/check-delivery.sh` には組み込まない（CI に入っておらず、二重定義を避けるため）
- 誘導での完走・「移動」での全 Step・CSP あり・なしの対照での通信と違反の計数・インライン不使用の検査の CI 化は TASK-151 2/2（#540。5.180）
- 要確認: CI のガードの追加は TASK-151 の人間の承認点。既存の `browser-test` ジョブに試験を 1 つ足しただけで、ジョブ・ruleset・必須チェックは変えていない

### 5.177 新しいタブの操作が再読み込みで戻る（TASK-324 2/2。SAVE-12・PoC-6 の S-8 相当）

実行日: 2026-10-21、元にしたコミット: 9b8fd10 に本変更を載せた作業ツリー。5.176 は別の open PR（TASK-151 1/2）が使うため空けた。

- 変えたもの: ブラウザ試験 `tools/browser/stale-tab.test.mjs`（5.171）の既存の 2 件（CSP あり・なし）の手順を広げた。試験の件数は変わらない。本番のコードは変えていない
- 手順: (1) A の 1 手が保存される。(2) B を開いて 1 手を送り、左の成果物の行（`notes/draft`）で確認して `main/st-2` へ進める。保存値が変わる（陽性の対照）。(3) A の 1 手はメモリ上だけ進み（Step は `main/st-1` のまま）、保存値は B のまま、A は古いタブになる。(4) B を再読み込みすると `restored` で `main/st-2` が戻り、各スライスの状態・増分の射影・世界の一致が通り、B は新しいタブのまま。(5) 再読み込みの後も A の 1 手は保存されない。(6) 再読み込みの後の B の選択（`main/st-3` へ）は保存される（陽性の対照）。(7) A を `pagehide` つきで閉じても保存値は変わらない。(8) B をもう一度再読み込みすると `main/st-3` が戻る
- 判別の前提: 5.171 の手順は B が 1 手送るだけで、2 手目は拒否でログが伸びるだけだったため、復元された状態を A のメモリ上の状態とも復元の失敗とも区別できなかった。B に Step を変える操作（確認・選択）をさせ、A が B と違う Step にいることを試験の中で assert する。陰性の対照として、一時の写しで B の確認を外すと「A が B と違う Step にいる」の assert で CSP あり・なしの両方が失敗することを手元で確かめた（コミットしない）
- 揺れの対策: 保存値の変化は即時の読み取りでなく `waitForFunction` で待ち、再読み込みの後は処理待ちが無くなってから基準を取り直す（`pagehide` で書き直されうるため）。assert は緩めていない。単体で 5 回流して毎回 2 件とも通った
- 出力: 失敗の文には、保存値・鍵・起動番号・ログの本文を出さない（長さと簡易の digest・真偽・Step の uid だけ）
- 変異注入は足さない: `crates/web/tests/mutations/` の形式は cargo の試験を対象にし、ブラウザ試験を対象にできない。照合を外す変異は SAVE12-1〜3（5.171）が native 試験で検出する
- 範囲外: 古いタブに出す「保存されていない」表示（SHELL-23）、合流の規則（SAVE-13）、照合と書き込みの間の競合の窓、実ブラウザでの同時の起動、古いタブ A 自身の再読み込み

### 5.178 古いタブの画面に「保存されていない」表示を出す（TASK-325 1/2。SAVE-13・SHELL-23・D-56）

実行日: 2026-10-21、元にしたコミット: aaca452 に本変更を載せた作業ツリー。5.176 は別の open PR が使う見込みのため空けた。

- 経路: 古いタブの `Guarded::write` は `Ok` を返して捨てるため、契約（`StorageError`）は変えず、SAVE-11 の probe と同じ形の読み取り専用の口 `StaleProbe` を `host.rs` に足した。`persist::boot_and_mount` が `persist::stale_tab`（`BootGuard::is_stale` の真偽だけ。保存領域は読まない）を結び、`EngineHost::update` の末尾（保存の試みの後）で `refresh_stale` が引く。真なら `HostModel::stale_tab` を立て、`stale_notice` を dirty に積む。一度立てたら戻さず、再び積まない。表示は新しい通知欄 `stale-notice`（`stale_notice.rs`、`engine_parts` の `F_LOG[8]`、`role="status"`）で、保存失敗の `save-notice` とは別の欄にした（あちらは保存に成功すると消えるが、こちらは消えない）
- 自動運転での判断（人の承認は無い）: (a) probe の形を選び契約を変えない。(b) `save-notice` と別の通知欄にする。(c) 一度出したら消さない。(d) 文面は仮置き（最終の文面は TASK-549、配置は TASK-534 が決める）。教材・鍵・起動番号の値を含めない固定の文だけ
- native 試験 `crates/web/tests/stale_notice.rs`（5 件）: 追い越された古いタブだけが同じ `update` で通知を出し `save-notice` は空のまま・新しいタブは空（陽性の対照）、出た通知は消えず dirty に再び積まれない、probe を結ばないホストは出ない、文に教材の値が含まれない、照合の無い保存先では出ない（対照）。既存の `host_contract`（field 数 21）・`dirty_binding_points`・`glue_contract`・`state_paths` を追従させた
- ブラウザ試験 `tools/browser/stale-tab.test.mjs`（CSP あり・なし）に通知の確認を足した。A は追い越される前と B の 1 手の前は空、追い越された後の 1 手で空でなくなり、再読み込みの後の A の操作でも消えない。B は常に空。文面は固定せず「空でない」だけを確かめる。試験の件数は変わらない（`make test-browser` は 610 件が通った）
- 変異注入 `SAVE13-1`〜`SAVE13-3`: 3 件とも DETECTED。`update` の末尾の `refresh_stale` を外す・通知の文を常に空にする・一度立てた後も dirty に積み直す
- 既知の限界: 通知が出るのは古いタブが保存を試みたとき（操作の後）で、別のタブが開いた瞬間には出ない（`storage` イベントは使わない。照合は保存の直前だけ）。保存を止めている間・保存する値が無い間（未操作・上限超過）は照合しないので出ない。書いた直後の再照合でも stale が立つため、その 1 回の書き込みが届いた場面でも「以後は保存されない」通知が出る（文面は今後の保存について述べる）
- 範囲外: 通知の最終の文面・配置（TASK-549・TASK-534）、別のタブが開いた瞬間に知らせること、合流の規則（D-56 で採らないと決定。TASK-325 2/2）、`export-notice` が `terminal_area::is_defined_testid` に無い既存の不整合

### 5.179 交互 10 回ずつの操作の後も新しいタブの履歴が開く（TASK-325 2/2。SAVE-13・D-56）

実行日: 2026-10-21、元にしたコミット: 74d84e3 に本変更を載せた作業ツリー。

- 条件の判定: Issue #608 は「TASK-311 の決定が合流の規則を採った場合」に限る。決定 D-56（2026-10-12）は合流の規則を採らず、古いタブは保存しない（SAVE-12）にとどめて表示を出す（TASK-325 1/2）と決めた。よって合流は実装せず、本番のコード（`crates/*/src/**`）は変えない。自動運転での判断（人の承認は無い）であり、#608・#606 を不要として閉じるかの最終判断は人に残す
- 代わりに、受け入れ条件の場面（2 つのタブで交互に 10 回ずつ操作して再読み込みする）を、採用した規則の下で確かめた
  - ブラウザ試験 `tools/browser/stale-tab.test.mjs`（CSP あり・なしで 2 件増）: 10 ラウンド、各ラウンドで古いタブ A の 1 手の後に保存値が直前の値のままであること、新しいタブ B の 1 手の後に保存値が変わること（陽性の対照）を確かめる。最後のラウンドで B だけ確認まで進めて st-2 にし、A（st-1）と Step を分ける。その後 A が古いタブで通知が出ること、B は出ないことを確かめ、B の再読み込みで `bootKind` が `restored` かつ写しと一致すること、A の再読み込みでも B の履歴が復元されること（A のメモリ上の操作は合流されない）を確かめる。5 回流して毎回 4 件とも通った
  - native 試験 `crates/web/tests/boot_guard.rs` の `交互に_10_回ずつ書いても保存先には新しいタブの最後の書き込みだけが残る`: 同じ交互の書き込みを偽の localStorage で確かめる
- 「`replay_mismatch` 0 件」の表し方: 起動で開けなかった理由は `stateJson` に出ない（`BootOutcome::NotOpened` の理由は捨てている）ため、`bootKind === 'restored'`（開けなければ `not_opened`）と、復元した Step・スライス・増分の射影・世界の一致で表した。理由を出す口は足していない
- 範囲外: 合流の規則そのもの、spec 側の SAVE-13・SHELL-23 の状態の更新、起動時の理由を `stateJson` に出す口、IndexedDB の保存先でのブラウザ試験（native 試験が照合を確かめる）、`scripts/expected-test-counts.txt` の更新（別 PR で `make update-test-counts`）

### 5.180 インラインを使わない画面の検査と CSP あり・なしの計数（TASK-151 2/2。SAFE-13・SAFE-15）

実行日: 2026-10-21、元にしたコミット: 3fd422b に本変更を載せた作業ツリー。5.176 の範囲外としていた 3 点を、既存の `browser-test` ジョブ（`make test-browser`）の中の試験として足した。本番のコード（`crates/**`）は変えていない。

#### 試験

- `tools/browser/inline-free.test.mjs`（SAFE-15。24 件。ブラウザを使うのは 1 件。harness の追加分は別に数える）: `harness.mjs` の `inspectInlineFree` が、配信・試験の両設定の `index.html`・`app.css` で、`script` は `type="module" src="./app.js"` の 1 つだけ（インラインの script 0）、`style` 属性・`on*` 属性・`style` 要素 0、`href`・`src` 等が `data:`・`javascript:`・`blob:`・外部の `http(s):`・`//` でない、favicon の `link` が `./icon.svg` の 1 つ、`app.css` に `url(`・`@import`・`expression(` が無い、試験用の meta が配信する設定に無い、を満たすことを確かめる（fail-closed。問題の一覧を返す）。`app.js`・`icon.svg` が配信元から直接取れる（200・`javascript`・`image/svg+xml`）ことも確かめる。実物の写しを加工した対照 14 種（インラインの script・src つきの script への中身の同居・`style` 属性・`on*` 属性・`style` 要素・`data:` の favicon〔PoC-7 の修正前〕・外部の script・外部の画像・プロトコル相対の画像・script の `src` の差し替え・favicon の link の削除・`app.css` の `url(data:)`・外部の `@import`・配信する設定への試験用の meta）が、期待の理由と件数で不合格になる。ブラウザの対照として、配信する設定の写しの body に `data:` の画像を 1 つ足すと、CSP ありでは違反がちょうど 1 件（`img-src`）、CSP なしでは 0 件、加工前の実物では両方 0 件になる
- `tools/browser/csp-runs.test.mjs`（SAFE-13。6 件）: 場面ごとに CSP あり → なしの順で流し、各回と各 Step で、配信元以外への通信 0・CSP の違反 0・コンソールの CSP 関連の出力 0（`harness.mjs` の `cspConsoleMessages`。`rec` に `consoleWarnings` を足した）・ページ内エラー 0・失敗した要求 0・HTTP 400 以上 0（いずれも favicon を除く）・安全の計数（`scanDom` を含む）がすべて 0 であることを主張する。場面の終わりに、CSP の有無で配信元への要求の種類（`requestKinds`。相対パス・種類・状態の組）が等しいことも比べる
  - D1（配信する設定）: 最小の画面の起動と入力欄への入力・送信。`window.__fandheTest` が無い
  - T1（試験の設定）: 最小教材 `min-3step` の 3 Step を実クリックと実入力で達成まで進める
  - T2: CH-1 の 6 Step を含む区間（`st-0-1` から `st-2-1`）をキーボードだけで進め、Step の到達ごとに走査する。誘導は `ch0-3-journey.mjs`（`ch0-3-run.test.mjs` から移した補助。試験の名前・件数・判定は変えていない）の `journey` に `onStep` の口を足して共有した
  - T3: 掲示板教材の代わりの試験用の教材の全 Step を「移動」（`raw-trigger` へ `go_to`）で順に開き、各 Step で走査する。`ch0-3` の 20 Step と `reload-points` の 8 Step の 2 件。Step の一覧は教材の IR から読み、個数を固定する
  - T4: 表示部品 7 種 × 変種 (a)・(b) の 14 通りすべてを `artifact-view-check.mjs` の `CASES` から mount し、前後の両方で走査する。文書 (a)・差分 (a) は対照の口から描く。部品が増えれば `CASES` に入り、自動で対象になる
- `harness.test.mjs` に `cspConsoleMessages` と `requestKinds` の純関数の対照を 2 件足した（CSP の文言だけを拾う・余計な要求が差に現れる・除外は引数で明示した分だけ）

#### 確認（2026-10-21）

- 配信・試験の両設定を `SKIP_WASM_OPT=1 tools/build-web.sh` で作り、`inline-free` と `harness` の 36 件、`csp-runs` の 6 件、`ch0-3-run` の 4 件がすべて通った。追加分の実行時間は `csp-runs` 約 13 秒、`inline-free` 約 3 秒で、`browser-test` ジョブの timeout（60 分）に対して無視できる。環境変数なしでは `csp-runs`・`inline-free` の 25 件が失敗する（skip しない）
- T3 の巡回でエンジンの警告は 0 件だった（件数は診断の行に出すだけで assert しない）
- `make test-browser` の全体は流していない（未検証）。`scripts/expected-test-counts.txt` は変えていない（CI の実測と完全一致が要るため、並列の PR とずれる危険を避けた。更新は別の機会に `make update-test-counts`）

#### 限界・範囲外

- 配信する設定は実行時に教材を読み込まない（`EngineHost::for_mount` は session なしで、`crates/web/src` に `ir/main.json` を取得する処理が無い）ため、配信する設定での誘導の完走・「移動」は確かめていない。D1 は起動と入力欄への入力・送信までである。実行時の教材の読み込みが入った後に、同じ場面を配信する設定へ広げる
- 掲示板教材は repo に無い（D-11。公開可否は TASK-14）。「全 Step を移動で開く」は、CH-2 の形を写した試験用の教材で代えた。本物の掲示板教材では未確認である
- favicon は要求の種類の比較から外した（ヘッドレスでの取得が揺れるため）。代わりに静的な検査と直接の取得で確かめる
- 静的な検査は `index.html`・`app.css` が対象である。Rust が実行時に描く DOM は `scanDom`（`style` 属性・`on*` 属性・危険な要素・URL）が各場面で数える
- CH-0〜CH-3 の全 Step の CSP 計数つきの完走は TASK-152 の範囲
- `tools/check-delivery.sh` には組み込まない（5.176 と同じ判断）
- 要確認: CI のガード（インライン不使用と CSP の計数を失敗で止める検査）の追加は TASK-151 の人間の承認点で、承認は未取得である。既存の `browser-test` ジョブに試験を足しただけで、ジョブ・ruleset・必須チェックは変えていない

### 5.181 秘密情報らしい文字列の検出と保存の直前の警告（TASK-164 1/3。SAFE-27・SAFE-29）

保存の直前に、固定の 10 形（S-01〜S-10）の秘密情報らしい文字列を検出して警告する。警告には形の名前だけを出し、値は出さない。対照 6 件（N-01〜N-06）では出さない。検出した値は保存先へ積まない（D-44 の失敗側。一度書かれた秘密情報は保存先に残り、元に戻せないため）。

#### 経路

- 検出（core の `secret_scan`）: 接頭辞と文字種・長さで判定する純関数。正規表現の crate は足していない。JSON は値として読み直してから、文字列の値だけを調べる（キーは見ない）。`Finding` は形と 32 ビットの指紋だけを持ち、値を持たない。指紋は TASK-164 3/3 が確認済みの照合に使う
- 3 つの保存の入口が同じ判定（web の `secret_notice::secret_hold`）を使う。(1) 状態を変える操作の直後の即時保存（`EngineHost::save_after_change`）は、検出したら saver を呼ばず、警告を出す。(2) 下書きの遅延保存（`persist::save_deferred`）は、積まず、保存世代と形だけを `secret-hold` の action でホストへ配って警告を出す（世代が進んでいれば古い検出は捨てる）。(3) 離れる直前の書き出しは、積まないだけで、警告は出さない（離れる最中のため）
- 警告の解除: 即時の経路は、次の保存で検出に掛からなければ空にする。遅延の経路は、書けた保存の `save-result`（成功）が空にする。ただし履歴の `ops` に値が残る間は、以後の保存も同じ値で止まる。値を履歴から外す選択は TASK-164 3/3 が持つ
- 表示は `secret-notice`（`role="status"`）。配置・文言は仮置きで、確定は TASK-534 が持つ。`A11Y_CONTRACT_VERSION` と `CONTRACT_VERSION` は上げていない

#### 試験

| 試験 | 内容 |
|---|---|
| `crates/core/tests/secret_scan.rs`（12 件） | 部品から組み立てた 10 形がすべて期待の ID で検出される（入れ子の JSON の中でも）。名前つきの形（S-05）は名前の大文字・混在でも検出する。対照 6 件は 0 件。語の途中の接頭辞・キーだけにある値は検出しない。任意の文字列・切り詰めた値で panic しない（proptest）。接頭辞の反復（`-eyJ`・`xoxb-`・BEGIN 行など 9 形の入力、各 20 万回）が線形時間で終わる（5 秒の上限） |
| `crates/web/tests/secret_notice.rs`（6 件） | 10 形の入力で saver が 0 回・警告が期待の形・描画の文に名前が出て値が出ない。対照 6 件は警告が空で saver が 1 回。警告の後の保存も止まる。`secret-hold` は世代つきの完全一致の payload だけを受け、世代が古い検出は捨てる |
| `tools/browser/secret-warning.test.mjs`（3 件） | 10 形を下書き・送信の両方の経路で入れ、入力の直後は警告が空・遅延の後に名前が出る・`engine.secretWarning` が期待の形・履歴の鍵への書き込みが 0 件。対照 6 件は警告が空で書き込みが 1 件以上。値は `secret-values.mjs` が部品から実行時に組み立てる |
| 変異 `SAFE27-1`〜`SAFE27-6`（core 4 件・web 2 件） | 3 部の規則を落とす・長さの下限を 0 にする・キーも調べる・語の境界を外す・検出しても saver を呼ぶ・警告の文を空にする。すべて DETECTED（2026-10-20） |

#### 確認（2026-10-20）

- `make lint`・`make test`・`make check`・`make check-deps`・`make check-dom-reads`・`make check-generic-words`・`make test-check-generic-words`（49 件）は通過した。`make test-browser` は 611 件がすべて通過した。`make check-mutations` は SAFE27 の 6 件だけを流し、DETECTED だった（全件は流していない）
- 試験の出力に完成した値が無いこと: `make test-browser` の出力と core・web の native の試験の出力を一時ファイルへ保存し、10 形の値が含まれる件数を数えた。0 件だった（確認の道具は repo に置かない。確認後に削除した）
- 警告が出るまでの時間（入力から表示。下書きの経路、Chromium の同梱版、CSP あり）は 203〜219ms だった。遅延保存の待ち（実装の値）を含み、期待にしない
- `make build-web`・`make check-delivery` は流していない（未検証。wasm の export は足していない）。`scripts/expected-test-counts.txt` は変えていない（版上げの前に `make update-test-counts` で更新する）

#### 限界

- 規則は固定の 10 形に合わせたもので、一般の秘密情報の検出率を示さない。形の追加は TASK-165 が持つ（5.184 で 5 形を足した）
- 履歴ファイルの読み込み（インポート）の内容は調べない
- 保存の前は、入力欄と画面のログに値が出る（PoC の限界 (2)）。画面からの消去は TASK-164 3/3 が持つ
- 検査の費用は履歴の大きさに比例し、即時の保存のたびに 1 回走る。PoC の native の release で 9MB あたり約 88ms。WASM での時間は未計測
- 警告の間に保存先が変わらないことの IndexedDB を含む確認・書き出しの停止・放置したときの扱い（SAFE-44）は 5.182 で確かめた。選択のボタンは 5.183（TASK-164 3/3）で足した

### 5.182 警告中の保存と書き出しの停止、放置したときの扱い（TASK-164 2/3。SAFE-27・SAFE-28・SAFE-44・D-44）

警告の間は、保存先（localStorage・IndexedDB の両方）に書かず、受講中の履歴の書き出しも止める。選ばずに放置して閉じたときも書かない。D-44（2026-10-12）の決定は「選ぶまで書かず、その間の進行は保存されない（保存の失敗側に倒す）」で、SAFE-44 は確定済みである。

#### 経路

- 受講中の履歴を書き出す唯一の入口として `EngineHost::export_live`（戻りは `history_export::ExportGate`）を置いた。(1) 書き出す履歴が無い・包みを組み立てられない・書き出せない・UTF-8 として読めないときは `Empty`、(2) 警告の間は `Held`、(3) 遅延保存の検出が届く前の下書きだけの値も、関門自身が `secret_hold` で走査して `Held`（fail-closed）、(4) それ以外は `Ready`。`Held` は形の ID だけを持ち、値を持たない。読み取りだけで、警告・dirty は変えない
- `save_blocked` と読み込みの上限（`history_limit::check_save`）は見ない。書き出しは保存先を上書きせず、保存できない場面ほど要るため（SAVE-29）。上限を超える書き出しの扱いは TASK-531 に残す
- 書き出しの画面は未実装（TASK-531）。TASK-531 はリンクを描く前に `export_live` を呼び、`Held` なら出さない。試験用の状態 JSON に `exportGate`（ready・held・empty）と `exportBytes`（`Ready` のときだけ長さ）を足した。中身は入れない。wasm の export は足していない
- 離れる直前の書き出し（`persist::install_save_on_leave`）は 1/3 のとおり、見つかれば積まない。コードの分岐は変えていない。警告の状態は保存しないため、開き直した後は直前の無害な保存から復元され、警告は出ない
- TASK-164 3/3 は、確認済みの値（指紋）を `secret_hold` と `export_live` の両方で除外する前提である

#### 試験

| 試験 | 内容 |
|---|---|
| `crates/web/tests/secret_hold_export.rs`（6 件） | 10 形の送信で `export_live` が期待の形の `Held` で saver が 0 回。下書きだけの値は、遅延の検出が届く前でも関門が走査して `Held`。対照 6 件は `Ready` でバイト列が保存する値と一致。警告の後の進行も保存・書き出しされない。未操作は `Empty`。`Held` の表示に値が含まれない |
| `tools/browser/secret-hold.test.mjs`（8 件） | (A) 10 形で `exportGate` が held・`exportBytes` が null、対照は ready で長さが正。(B) localStorage・IndexedDB の両方で、無害な 1 手が書かれる陽性の対照の後、秘密情報の送信と続く 1 手で保存先の digest と履歴の鍵への書き込みの記録が増えない。(C) 警告のまま閉じて同じ context で開き直すと、保存先は基準のまま・復元した状態は秘密情報の前・警告なし・保存先のどこにも値が無い（下書きだけの経路と、送信して続けて進めた経路を、保存先ごとに。保存の無い状態から始めると鍵が無く新しく始まる） |
| 変異 `SAFE44-1`・`SAFE44-2` | 警告の間でも書き出しを通す・関門自身の走査を外す。どちらも DETECTED（2026-10-21）。警告の判定と走査は二重の守りで、片方を外しても止まるため、前者は警告の分岐の戻り値を変える。離れる直前の分岐は wasm のみで native の試験では検出できないため変異にせず、(C) の陽性の対照（下書きの経路で閉じても基準のまま）で担保する |

#### 確認（2026-10-21）

- `make lint`・`make test`・`make check`・`make check-deps`・`make check-dom-reads`・`make check-generic-words` は通過した。`make test-browser` は 689 件がすべて通過した。`make check-mutations` は SAFE44 の 2 件と SAFE27-4・SAFE27-5 だけを流し、すべて DETECTED だった（全件は流していない）
- 試験の出力に完成した値が無いこと: `make test-browser` と `make test` の出力を一時ファイルへ保存し、10 形の値が含まれる件数を数えた。0 件だった（確認の道具は repo に置かない。確認後に削除した）
- `make build-web`・`make check-delivery` は流していない（未検証。wasm の export は足していない）。`scripts/expected-test-counts.txt` は変えていない（版上げの前に `make update-test-counts` で更新する）

#### 限界

- 書き出しの画面は TASK-531、選択のボタンと確認済みの値の扱いは TASK-164 3/3 が持つ
- 警告を出したまま閉じると、その間の進行（警告の後の操作）は保存されない。D-44 で決めて受け入れた限界で、`docs/known-gaps.md` に記録した
- 試験用の状態 JSON の `exportGate` は、呼ぶたびに履歴の全体を走査する（試験の設定だけ）

### 5.183 警告の選択「このまま保存する」と「この入力を残さない」（TASK-164 3/3。SAFE-28・D-44）

警告（`secret-notice`）に 2 つの選択のボタンを付けた。「このまま保存する」は見つかった値を確認済みとして保存し、書き出しを通す。「この入力を残さない」は保存せずに読み直し、値が保存先・入力欄・画面のどこにも残らないようにする（送信の後に選んでも同じ）。文言と配置は仮置きで、確定は TASK-534 が持つ。

#### 経路

- 判定を `EngineHost::secret_gate` の 1 か所に集めた。確認済みの検出（形と 32 ビットの指紋の組。値は持たない）を除いた形を返す。即時の保存（`save_after_change`）・書き出し（`export_live`）はホストの中で、遅延保存と離れる直前の保存は `glue::secret_gate` を通して呼ぶ。ホストを借りられないときは、確認済みを除かない `secret_hold`（厳しい側）に倒す
- 「このまま保存する」（`secret-keep`）: 警告が無いときは何もしない（古いクリック）。今の履歴の未確認の検出の形が警告の形と違うとき（警告の後に別の形が入った）は、確認済みに加えず警告を新しい形へ置き換えて終える。一致すれば確認済みに加え、予約済みの遅延保存を無効にして保存する（下書きだけの検出から来た警告も保存する）
- 「この入力を残さない」（`secret-discard`）: 警告が無いときは何もしない。保存先を `set_saved_reader`（`persist::boot_and_mount` が結ぶ）で読み、`restore_from_saved`（`boot` から切り出した起動の分岐。条件は `boot_session` に任せる）で実行環境を作り直す。入力欄は `HelloView::reset_for_reload` で下書き・送信済みの一覧・案内を空にし、`<textarea>` へ書き戻す。確認済みと予約済みの遅延保存を取り下げ、保存先へは書かない。読めなかったときは新しく始めて保存を止める。ページの再読み込みにしないのは、ブラウザが `<textarea>` の値を復元しうるため
- ボタンは `F_LOG` の末尾に足した keyed list `secret_choice`（`secret-choice`）に置き、警告の有無が変わるときだけ dirty に積む（全再描画にしない）。文字・属性は固定の定数だけ
- 確認済みの集合は保存しない。起動時に保存先の中身から種を入れない（旧いエンジンが書いた値や読み込んだ履歴の値を黙って確認済みにしないため）。履歴の差し替え・読み直しで空にする。試験用の状態 JSON には `secretAcknowledged`（件数だけ）を足した

#### 試験

| 試験 | 内容 |
|---|---|
| `crates/web/tests/secret_choice.rs`（14 件） | 10 形の保存と書き出し・下書きだけの検出からの保存・警告が無いときの無視・別の形を確認済みにしない・確認済みの値が続く保存で再警告されず新しい値には警告が出る・10 形の読み直しで値が保存先・入力欄・画面・記録した状態に残らない（送信の後と下書きだけ）・保存済みの履歴が無い／読めないときの読み直し・読み直しが確認済みを空にする・ボタンが警告のあるときだけ出る・対照ではボタンが出ない・action 名の完全一致 |
| `crates/web/src/hello.rs` の単体試験（1 件） | 読み直しの初期化が下書き・一覧・案内を空にし通し番号を戻さない |
| `tools/browser/secret-choice.test.mjs`（10 件） | (A) 10 形で保存され `exportGate` が ready（localStorage）、IndexedDB・下書きだけの経路でも保存先に届く。(B)(C) 読み直しで保存先が基準のまま・履歴の鍵への書き込みが増えない・入力欄が空・画面の文字と HTML と保存先に値が無い（送信の後と下書きだけ。localStorage・IndexedDB）。(D) 読み直しの後に閉じて開き直しても値が無い。(E) 確認済みは再読み込みで失われ、次の変更で再び警告が出る。(F) 対照 6 件でボタンが出ない |
| 変異 `SAFE28-1`〜`SAFE28-4` | 確認済みに加えない・入力欄の初期化を外す・判定で確認済みを除かない・保存先からの読み直しを外す。すべて DETECTED（2026-10-21） |

`glue::secret_gate` を通る遅延保存と離れる直前の除外は wasm だけの経路で native では検出できないため変異にせず、ブラウザ試験 (A) の下書きだけの経路と (E) で担保する。

#### 確認（2026-10-21）

- `make lint`・`make test`（`--features test-hooks` の web も）・`make check`・`make check-deps`・`make check-dom-reads`・`make check-generic-words` は通過した。`make test-browser` は 699 件がすべて通過した（689 件から `secret-choice` の 10 件が増えた）。変異は SAFE28 の 4 件と SAFE27-4・SAFE27-5・SAFE44-1・SAFE44-2 を流し、すべて DETECTED だった（全件は流していない）
- 試験の出力に完成した値が無いこと: `make test`・`make test-browser` の出力を一時ファイルへ保存し、10 形の値（JSON のエスケープ形を含む）が含まれる件数を数えた。0 件だった（確認の道具は repo に置かない）
- `make build-web`・`make check-delivery` は流していない（未検証。配信する設定に試験用の口は増やしていない）。`scripts/expected-test-counts.txt` は変えていない（版上げの前に `make update-test-counts` で更新する）
- `glue.rs` のコード行が 449 から 452 に増えた（`secret_gate` の 3 行）。`glue_scale.rs` の上限と 5.166 の表を同時に更新した。`state_paths.rs` の `INIT_ALLOWED` に `set_saved_reader`、`glue_contract.rs` の許可表に DOM ではない呼び出し名を足した

#### 限界

- 確認済みは再読み込みで失われる（保存しない）。選ばずに閉じればその間の進行は保存されない（D-44）。`docs/known-gaps.md` に記録した
- 同じ形の別の値を警告の後に足して「このまま保存する」を選ぶと、足した値も確認済みになる（指紋を action に載せないため区別できない）。形が違う値は区別する
- 読み直した状態は、復元として再読み込みの記録が 1 件付く。離れる直前に、値を含まないその状態が保存される（未操作の再読み込みと同じ扱い）
- 古いタブで「この入力を残さない」を選ぶと、保存先の最新の内容から作り直される（実際の再読み込みと同じ）

### 5.184 秘密情報の検出の形の追加（TASK-165 1/2。SAFE-30・D-45）

固定の 10 形に、決定 D-45（2026-10-12）に基づいて 5 形（S-11〜S-15）を足し、15 形にした。追加した形でも、保存の直前に警告が出て保存と書き出しが止まる。検出の範囲の限界は `docs/known-gaps.md` の SAFE-30 に書いた。

追加の 5 形の選定は、D-45 が「実装のタスクで案を出し、ユーザーが決める」としたものに対し、本変更が実装の案として出した（本変更の時点では、自動運転のためユーザーの確定を得ていなかった）。

追記（2026-10-21）: ユーザーが、この 5 形の選定を確定した（形の一覧は下の表のまま変えていない）。確定は、spec の SAFE-30 と、spec のリポジトリの決定記録 D-66 に記録する。

#### 追加した形

選定の基準は、固定の接頭辞と文字種・長さで判定できること、既存の形と接頭辞が重ならないこと、開発の道具と API の資格情報であること、接頭辞だけの説明文では検出しないこと。名前は接頭辞の形で表した（語彙の付録 A の語を避けるため）。

| 試験 ID | 形の ID | 接頭辞 | 続く文字種 | 長さの下限 |
|---|---|---|---|---|
| S-11 | `sk_ant_api_key` | `sk-ant-` | 英数字・`_`・`-` | 40 |
| S-12 | `sk_proj_api_key` | `sk-proj-`・`sk-svcacct-`・`sk-admin-` | 英数字・`_`・`-` | 40 |
| S-13 | `gitlab_token` | `glpat-` | 英数字・`_`・`-` | 20 |
| S-14 | `npm_token` | `npm_` | 英数字 | 36 |
| S-15 | `pypi_token` | `pypi-AgEIcHlwaS5vcmc` | 英数字・`_`・`-` | 50 |

見送った形は、旧い接頭辞なしの長いキー（接頭辞が短く誤検出しやすい）・Slack の Webhook の URL（URL の形で別の規則が要る）・SendGrid（区切りが 2 つある）・OAuth のクライアントシークレット・Discord と Twilio（少数に絞るため）・一般の高エントロピー文字列（誤検出が多い）である。

#### 試験

| 試験 | 内容 |
|---|---|
| `crates/core/tests/secret_scan.rs`（16 件） | 追加 5 形が期待の ID だけで検出される（入れ子の JSON の中も）。`sk-proj-` の 3 つの接頭辞がそれぞれ検出される。全 15 形が自分の ID だけで検出される。追加の対照 4 件（N-07〜N-10）は 0 件。語の途中の接頭辞は追加の形も検出しない。追加の接頭辞の反復（各 20 万回）が線形時間で終わる |
| `crates/web/tests/secret_notice.rs`（8 件） | 追加 5 形で saver が 0 回・警告が期待の形・文に名前が出て値が出ない。追加の対照 4 件は警告が空で saver が 1 回 |
| 変異 `SAFE30-1`〜`SAFE30-3` | sk-ant- の規則を落とす・npm 風の長さの下限を 0 にする・`sk-admin-` の接頭辞を落とす。すべて DETECTED（2026-10-21）。SAFE27-1・2・3・6 も DETECTED。全件は流していない |

試験データは `testdata/secret-shapes-added/added-shapes.json`（この repo で作った。部品に分け、完成した値は置かない）。既存のローダ（`shapes()`・`controls()`）は 10 形・6 件のまま残し、ブラウザ試験の前提を変えていない。

#### 確認（2026-10-21）

- `make lint`・`make test`・`make check`・`make check-deps`・`make check-dom-reads`・`make check-generic-words`・`make test-check-generic-words`（49 件）・web の `--features test-hooks` の `secret_notice` は通過した
- `make build-web`・`make check-delivery` は流していない（未検証。wasm の export と配信する設定は変えていない）。`scripts/expected-test-counts.txt` は変えていない（版上げの前に `make update-test-counts` で更新する）

#### 限界

- 一般の検出率は保証しない。見逃した形・過剰な検出の件数の測定と、履歴ファイルの読み込み前の警告（D-45 で見送り）の扱いは 5.185 に記録した
- 追加の 5 形の選定は、2026-10-21 にユーザーが確定した（D-66）。一般の検出率を保証しないことは変わらない

### 5.185 秘密情報の検出の追加 5 形の見逃しと誤検出の測定、履歴の読み込みの扱い（TASK-165 2/2。SAFE-30・D-45）

5.184 で足した 5 形（S-11〜S-15）について、見逃しと誤検出（過剰な検出）の件数を、実装者が作った試料に対して測り、記録する。あわせて、履歴ファイルの読み込みの前に警告が出ない現状（決定 D-45 が検査の拡大を見送ったもの）を、試験で固定する。検出の規則（`crates/core/src/secret_scan.rs`）は変えていない。測定の結果に合わせて規則を直すと、追加の 5 形の確定とは別の新しい決定になるためである（5 形の選定は、2026-10-21 にユーザーが確定した。D-66）。

#### 測定の方法

試料は `testdata/secret-shapes-added/measure-corpus.json` の 44 件（この repo で作った。接頭辞は必ず部品に分け、完成した値は置かない）。分類は次のとおりである。

| 分類 | 真値 | 内容 |
|---|---|---|
| `realistic` | 秘密情報 | 各形の実在の長さに近い値 |
| `context` | 秘密情報 | 代入・引用符・URL の引数・日本語の句読点など、前後の文脈つきの値 |
| `known_miss` | 秘密情報 | 検出しないと分かっているもの（英数字に接した接頭辞・接頭辞の大文字化・下限より 1 文字短い値・ゼロ幅文字または改行で分けた値） |
| `prefix_only` | 秘密情報でない | 接頭辞だけの説明文・環境変数名・40 桁の 16 進数・UUID・PyPI の URL |
| `known_fp` | 秘密情報でない | 検出してしまうと分かっているもの（接頭辞のあとに下限以上のケバブケースの語や長い識別子が続く） |

数え方は次のとおりである。陽性は、期待の形が `secret_warning` に出れば「検出」、出なければ「見逃し」とする。陰性は、15 形のどれかが出れば「誤検出」とする。

#### 結果（2026-10-21。元にしたコミットは 52cb30e の上の本変更）

| 形 | 陽性 | 検出 | 見逃し | 陰性 | 誤検出 |
|---|---|---|---|---|---|
| `sk_ant_api_key` | 8 | 3 | 5 | 2 | 1 |
| `sk_proj_api_key` | 6 | 3 | 3 | 2 | 1 |
| `gitlab_token` | 7 | 3 | 4 | 2 | 1 |
| `npm_token` | 6 | 3 | 3 | 4 | 1 |
| `pypi_token` | 6 | 3 | 3 | 1 | 0 |
| 合計 | 33 | 15 | 18 | 11 | 4 |

この件数は、実装者が「見逃す例」を意図して入れた試料を含む集計である。一般の検出率でも、実際の利用での誤検出の率でもない。見逃しの試料は、検出しないと分かっている種類を各形に置いた結果であり、率を読み取るものではない。

見逃した試料の ID（値は書かない）:

- 直前に英数字がある接頭辞: M-04・M-14・M-23・M-32・M-42
- 接頭辞の大文字化: M-05・M-16・M-24・M-33・M-43
- 下限より 1 文字短い値: M-06・M-15・M-22・M-31・M-41
- ゼロ幅文字または改行で分けた値: M-07・M-08・M-25

誤検出した試料の ID: M-10（`sk-ant-` のあとの長い slug）・M-18（`sk-proj-` のあとの CSS のクラス名風の語）・M-27（`glpat-` のあとの文書の見出し風の語）・M-37（`npm_` のあとの 36 文字以上の英小文字の語）。接頭辞だけの説明文・環境変数名・16 進数・UUID・URL の試料（M-09・M-17・M-26・M-34〜M-36・M-44）は誤検出しなかった。

#### 試験

| 試験 | 内容 |
|---|---|
| `crates/core/tests/secret_scan.rs` の `added_shapes_measure_matches_the_recorded_counts` | 試料の形ごとの件数が、試料の `expected` と完全に一致する。分類と実測が食い違う試料が無い。JSON の値に入れても結果が同じ。失敗の文には試料の ID と件数だけを出す。完全一致にしたのは、規則を変えたときに記録した件数の見直しを強いるため |
| `tools/browser/secret-warning.test.mjs` | (E) 追加 5 形を下書き・送信の 2 経路で確かめる。(F) 追加の対照 N-07〜N-10 は警告が出ず履歴が書かれる。(G) 試料 44 件を下書きの経路に流し、形ごとの件数が試料の `expected`（core と同じ表）と一致する。診断には ID・分類・警告の有無だけを出し、値は出さない |
| `crates/web/tests/history_import.rs` の `opening_a_history_does_not_warn_and_the_first_save_after_it_does` | 追加の形の値を入力に含む履歴を開いても、開いた直後は警告が空で saver も呼ばれない。続けて最初の保存を起こすと、saver は 0 回のまま警告に期待の形が入る |

#### 履歴の読み込みの扱い（D-45 で見送り）

受け入れ条件の「履歴の読み込みの内容を検査する場合は、読み込む前に警告が出る」は、D-45 が検査の拡大を見送ったため、採らない側である。現状の挙動は次のとおりで、上の `history_import` の試験が固定する。

- `EngineHost::open_history` は `load_session` に判定を任せ、秘密情報の検査をしない。読み込みの前にも直後にも警告は出ない
- 読み込んだ後は、保存の入口（即時・遅延・離れる直前）と書き出しが、どれも書く直前に保存の直前の関門を通る。この関門は履歴の全体を検査するため、読み込んだ履歴に値が含まれていれば、最初の保存または書き出しの直前に警告が出て、保存が止まる
- 読み込みの口は、試験の設定の `static/test-hooks.js` からしか呼べない。可視の画面での読み込みは TASK-531 が持つ

#### 確認（2026-10-21）

- `cargo test -p fandhe-workshop-core --test secret_scan`（17 件）・`cargo test -p fandhe-workshop-web --test history_import`（9 件）は通過した
- ブラウザは、`TEST_HOOKS=1` でビルドした web に `secret-warning.test.mjs` を単独で流し、7 件が通過した（(G) の件数は core と一致）
- `make lint`・`make test`・`make check`・`make check-deps`・`make check-generic-words`・`make check-dom-reads` は通過した。`make test-browser` の全体は流していない（未検証。CI の `browser-test` に任せる）。`scripts/expected-test-counts.txt` は変えていない（版上げの前に `make update-test-counts` で更新する）

#### 限界

- 試料は実装者が作ったもので、件数は検出率ではない。試料の選び方で件数は変わる
- 追加の 5 形の選定は、2026-10-21 にユーザーが確定した（D-66。規則と件数は本節の記録のまま）
- 測定の結果で見つかった誤検出・見逃しを減らす規則の調整は、確定の後も範囲外の作業で、直していない（直すと新しい決定になる）
- 履歴の読み込みの内容の検査は、D-45 のとおり足していない

### 5.186 秘密情報の検査を含む保存の時間を WASM で測る（TASK-166。SAFE-31・R-20・D-45）

実行日: 2026-10-21、元にしたコミット: `197559a` に本変更を載せた作業ツリー（dirty）。

履歴が 10MB に近いとき、保存のたびに行う秘密情報の検査を含めて、変更から保存の完了までが R-20 の 1 秒以内に収まるかを WASM で測った。D-45 は「WASM で測って確かめる。収まらない場合に限り、入力の瞬間の検査を検討する」とした。PoC-7 の値（native の release で約 88ms・9MB）は WASM の値ではない。

結論: 収まった。検査の方法は変えていない（`core::secret_scan` と保存の経路は変更なし）。

#### 費用の構成

保存の直前は、次の順に 1 回ずつ走る。仕様の記述にある「書き出しを 2 回」とは形が違う。

1. `history_to_save`: 包みを `export_bytes` で直列化し、`history_limit::check_save` を通す。10,485,760 B を超えると `None` で、保存は黙って起きない
2. `secret_gate` → `secret_hold_except` → `scan_json_text`: その文字列を `serde_json::Value` へ読み直し（木の確保）、文字列の値ごとに `scan_text` を走らせる
3. 書き込み（localStorage の `setItem` か IndexedDB の `put`）

直列化と読み直しはそれぞれ 1 回で、書き出しを 2 回行ってはいない。

#### 測定の方式

`tools/browser/secret-scan-time.test.mjs`。時刻はページ側の `performance.now()`。書き込みの記録器は 5.170 と同じ（時刻と長さだけを持つ）。

- 履歴: Rust が作る合成の包み（`test_fixture::large_loadable_envelope`）。契約の試料（2 周分）の `ops` の後ろに、受け付けられない送信（ASCII 8,000 文字）を 620 件足し、適用し直して包み直す。受け付けられない入力は状態の写しにも残るため、本文は履歴の中に 2 回入る。直列化後の大きさは 10,130,514 B（9.5MiB〜10MiB の範囲を単体試験とブラウザ試験が縛る）。UI から送信を重ねて伸ばさないのは、送信のたびに履歴の全体を再直列化・再検査・再書き込みして、準備の費用が二次になるため
- 取り込み: `largeHistoryText(kind)` と `importText` を同じ evaluate の中で呼び、10MB を CDP に通さない。取り込めなければ失敗にする
- 種類: `plain`（接頭辞を含まない文）・`dense`（接頭辞の候補が密に現れるが検出に掛からない。走査の最悪側）・`marker`（対照。中ほどの 1 件に検出に掛かる値を置く。値は部品から実行時に組み立て、完成した値をソースに置かない）
- 試験用の口（`test-hooks` の feature のときだけ。読み取りのみ）: `debug_large_history_text`・`debug_secret_gate_len`（直列化と関門を本番と同じ関数で 1 回走らせ、`{"bytes","shapes"}` だけを返す）
- 暖機を 1 回捨てる。10MB の ASCII は localStorage の閾値（UTF-16 で 4,194,301）を超えるため、保存先は IndexedDB になる（`autosaveState().kind === 'idb'` を assert）。localStorage は 10MB を持てず、測っていない
- 値: (a) 直列化 = `historySaveLen`、(b) 関門 = `secretGateLen`（直列化 + 読み直し + 走査）、(b)-(a) = 検査の費用の見積り、(c) 書き込みだけ、(d1) 遅延保存の変更から書き込みの完了まで（debounce を引いた値も出す）、(d2) 即時の保存（受け付けられない短い入力の送信）から書き込みの完了まで。各 10 回
- assert: (d1)(d2) の p95 が 1,000ms 以内、1 回の変更で書き込みが 1 件、`saves` が増える、`secretHolds` が増えない、`shapes` が 0、大きさが範囲に入る、外部への要求・ページの例外が 0 件
- 対照: `marker` を取り込み、下書きを 1 回変える。警告（`secret-notice`）が出て、履歴の鍵への書き込みが 0 件で、`secretHolds` が 1 増える。関門が 10MB の中ほどまで調べていること（関門を空にしても時間の試験が通ってしまわないこと）を示す

#### 結果（2026-10-21）

```text
基点: commit 197559a（dirty。本変更を含む）・wasm-opt なし（SKIP_WASM_OPT=1）・機能の範囲 test-hooks（試験の設定）
      圧縮 tools/serve.mjs の gzip なし（127.0.0.1 の配信。回線の制限なし）・CPU の減速なし
ブラウザ: Chromium 153.0.8010.12（headless）・Playwright 1.63.0・Linux x86_64（Core i7-13700K・10 論理コア・RAM 27GB）
```

| 条件 | 履歴（UTF-8） | (a) 直列化 p95（ms） | (b) 関門 p95（ms） | (b)-(a) 中央値（ms） | (c) 書き込み p95（ms） | (d1) 遅延保存 p95（ms） | (d1)-debounce p95（ms） | (d2) 即時 p95（ms） | 判定（1,000ms 以内） |
|---|---|---|---|---|---|---|---|---|---|
| plain | 10,130,514 B | 8.9 | 54.1 | 45.9 | 22.3 | 313.6 | 113.6 | 116.9 | 通過 |
| dense | 10,130,514 B | 10.7 | 72.2 | 61.9 | 26.6 | 334.0 | 134.0 | 138.7 | 通過 |

- 検査の費用は 10MB で約 46〜62ms。変更から保存までの大半は debounce（200ms）と IndexedDB の書き込み・transaction の完了で、1 秒の基準に対して 3 倍以上の余裕がある
- 即時の保存でも書き込みは 1 件だった（予約済みの遅延保存は取り消される）。(d2) の書き込みの件数は両条件で 1〜1
- 対照（marker）: 検出 1 形、警告の要素が出るまで 285.4ms、履歴の鍵への書き込みは 0 件、`secretHolds` は 1 増えた
- 実測値は `make test-browser` の `[secret-scan-time]` の diagnostic に JSON で出る（履歴の中身・鍵・値は出さない）

#### 限界・未確認

- 基準端末 A（`docs/measurement.md`）では測っていない。上の端末は自動運転の開発機で、基準端末 A ではない。基準端末 A・低速な端末での値の追記は人が行う（未実施）。余裕が 3 倍程度のため、端末が大きく遅いと基準を割りうる
- Chromium だけで測った。Firefox・Safari は測っていない
- IndexedDB だけを測った。localStorage は 10MB の履歴を持てない（閾値で IndexedDB へ切り替わる）
- 履歴は合成で、実際の受講の履歴ではない。`dense` は接頭辞が密な最悪側に寄せたが、実在の入力の分布を代表しない
- wasm-opt なし・CPU の減速なし。CI の runner は開発機より遅いため、CI の `browser-test` の値は別に確認する（未実施）
- 検査が保存の直前で、それまで画面とログに値が出る点（SAFE-31 の未決の論点）は扱っていない
- `make measure` の束には入れていない（5.170 と同じ。束は `results` を 3 件に固定する）

#### 既存の試験への影響

- `scripts/test-browser.sh` の明示の一覧に `secret-scan-time.test.mjs` を足した。`scripts/expected-test-counts.txt` は更新していない（版上げの前に `make update-test-counts` で更新する）
- `test_fixture.rs` の単体試験を 3 件足した（大きさの範囲と上限の内側、`plain`・`dense` の検出 0 件と `marker` の 1 形、種類の名前の完全一致）
- 依存の crate・`glue.rs` は変えていない

### 5.187 拒否された入力が入力欄に残る（TASK-271。JUDGE-5・TERM-17）

JUDGE-5 と TERM-17（Must）は、拒否された入力が入力欄に残る（消えない）ことを求める。保持の実装は `crates/web/src/host.rs` の `submit` の `keeps_draft`（TASK-438）にある。本節は、その証拠を拒否の 7 分類すべてへ広げる試験の記録である。本番のコードは変えていない。

- 実行日: 2026-10-21。元にしたコミット: ed16219（dirty。本変更を含む）
- コマンド: `cargo test --locked -p fandhe-workshop-web --test submit_wiring`・`scripts/check-mutations.sh crates/web/tests/mutations/JUDGE5-4.mutation crates/web/tests/mutations/TERM1-1.mutation`・`make test-browser`（740 件が通過）

#### 方式

- native: `submit_wiring.rs` の `全拒否の分類で_拒否の後の下書きは送った本文のまま_書き戻しは要求されない`。分類ごとに新しいホストを作り、ログ末尾の拒否の分類・下書きが送った本文とバイト単位で同じこと・書き戻しの要求が増えないことを確かめる。`RejectCategory::ALL` の 7 語すべてが流れたことも assert する
- ブラウザ: `tools/browser/reject-keeps-draft.test.mjs`。入力欄へ本文を入れ、Enter と送信ボタンの click の両方で送る。拒否の後に、欄の値（DOM）と状態の下書き（`__fandheTest.draft()`）がどちらも本文のままであること、安全性の計数が 0 であることを CSP あり・なしで確かめる。`reject-lines` は入力欄を通らない経路で、JUDGE-5 の証拠にはならないため別に足した

#### 入力欄の本文と分類

`min-3step` の開始点 `s-1`（Step の入力は `make-note`）で実測した。試験は毎回、得た分類を assert する。

| 分類 | 入力欄の本文 | 元の error_inputs |
|---|---|---|
| `unknown_command` | `/nothing` | `e-unknown` |
| `wrong_order` | `/publish-note` | `e-wrong-order` |
| `missing_arg` | `/make-note` | `e-missing-arg` |
| `bad_arg_shape` | `/make-note Bad_Name` | `e-bad-shape` |
| `not_supported` | `/sync-cloud` | `e-not-supported` |
| `not_in_this_step` | `/help` | `e-not-in-step`（`choice` のため置き換え） |
| `not_in_this_step` | 前後の空白・空行を含む自然文の複数行 | 追加（trim されないことの確認） |
| `busy` | 受付の `/make-note first-note` の後、処理中に `/make-note second-note` | `e-busy`（`choice` のため置き換え） |

`e-busy` と `e-not-in-step` は `choice` の入力で入力欄から送れないため、入力欄の本文に置き換えた。対応表に無い分類は例外にし、得た分類の集合が error_inputs の 7 分類と一致することも最後に assert する。busy は、`run-status` が busy になるのを待ってから送る（固定の sleep は使わない）。処理が終わった後にも、欄が書き換わっていないことを確かめる。

#### 変異の結果

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| JUDGE5-4 | `host.rs`: 拒否の後も下書きを空にする | `全拒否の分類で_拒否の後の下書きは送った本文のまま_書き戻しは要求されない` | 上の試験と既存の `拒否された送信は記録され_下書きは残り_書き戻しは要求されない` | DETECTED |
| TERM1-1 | `host.rs`: 受付の後に下書きを空にしない | `拒否されない送信は本文つきで記録され_下書きが空になり_書き戻しが要求される` | 同じ試験ほか 3 件（新しい試験も含む） | DETECTED |

#### 限界・未確認

- ブラウザの試験への変異の対照（変異を入れた写しで `TEST_HOOKS=1` をビルドして失敗を見る）は行っていない（未実施）。変異は native の層で検出している
- Chromium だけ。`min-3step` の教材だけで確かめた。Firefox・Safari・実 IME は扱わない
- 拒否の後の選択状態・フォーカスの位置・問い返しの後の欄は扱わない（TASK-284・JUDGE-27・TERM-22 の範囲。問い返しの後は空・拒否の後は選択状態とする扱いは 2026-10-21 に確定した。D-67）。問い返しを持つ教材が試験の設定の画面に無いことも同じ限界である（5.3.1d）
- busy は処理中の窓（5 段 × 300ms）に依存する。窓を外れた場合は分類が busy にならず失敗するため、黙って通ることはない

#### 既存の試験への影響

- `scripts/test-browser.sh` の明示の一覧に `reject-keeps-draft.test.mjs` を足した。`scripts/expected-test-counts.txt` は更新していない（5.186 と同じ。版上げの前に `make update-test-counts` で更新する）
- `host.rs`・`glue.rs`・`testdata/**` は変えていない

### 5.188 拒否の後に状態が変わらないこと（TASK-270。JUDGE-5）

拒否された入力（`Judged` が拒否）が、Step を受付済みにせず、世界状態も変えないことを、`reject_invariant`（新規の統合試験）が 1,000 通りの入力列で確かめる。拒否のたびに、戻りのイベントがちょうど `InputReceived` と拒否の `Judged` の 2 件であること、`run` と `log` 以外の全欄が操作の前と一致すること、`log` が追記だけであること、操作の前に受付済みでなかった Step が受付済みにならないこと、次の行動が空でないことを確かめる。

実装は、`accept` の拒否の分岐が `state.run` を書く前に返る構造（変更なし）と、`session::apply` の関門 `reject_guard` である。関門は、拒否を含む結果が状態を変えた、または余分なイベントを含むときに、`run`・`entry` を操作前へ戻し、イベントを `InputReceived`・`Judged` だけにして固定文の `Warning` を 1 件足す。黙って戻すと誤りが状態から消えて試験が検出できないため、見える形にした。正しい実装では発火しない。関門そのものの行は、変異注入が統合試験だけを対象にするため記録していない。単体の試験（`reject_guard` の `tests`）で振る舞いを確かめた。

#### 性質テスト（2026-10-21）

3 本とも 7 分類（`RejectCategory::ALL`）のすべてが 1 件以上出た（assert する。件数は種に依存するため assert しない）。

| 試験 | IR・入力 | ケース数 | 種 | UnknownCommand | WrongOrder | MissingArg | BadArgShape | Busy | NotInThisStep | NotSupported |
|---|---|---|---|---|---|---|---|---|---|---|
| rejections_keep_state_on_random_sequences | min-3step・固定の操作列 | 1,000 | 既定 | 940 | 1076 | 630 | 639 | 1770 | 17469 | 937 |
| rejections_keep_state_on_guided_sequences | min-3step・誘導つきの操作列 | 1,000 | 既定 | 382 | 524 | 96 | 62 | 4830 | 9911 | 338 |
| rejections_keep_state_from_any_status | judgment-ir・Step と状態を指定した入力列 | 1,000 | 既定 | 184 | 829 | 17 | 27 | 5205 | 4145 | 157 |

#### 変異注入

```text
実行日: 2026-10-21
元にしたコミット: ed16219-dirty（本変更を含む作業ツリー）
コマンド: scripts/check-mutations.sh crates/core/tests/mutations/JUDGE5-4.mutation crates/core/tests/mutations/JUDGE5-5.mutation crates/core/tests/mutations/JUDGE5-6.mutation
```

| ID | 対象 | 期待した失敗 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|
| JUDGE5-4 | crates/core/src/accept.rs: 処理中・達成済みの拒否で処理中を解く | rejections_keep_state_on_random_sequences, rejections_keep_state_from_any_status（実際は 3 本とも失敗） | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (0, 0, true, [Command("/help")]) |
| JUDGE5-5 | crates/core/src/accept.rs: 入力の段の拒否で Step を受付済みにする | 3 本すべて | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: [Cmd(1)] |
| JUDGE5-6 | crates/core/src/accept.rs: 確認の段の拒否でフラグを立てる | rejections_keep_state_on_random_sequences, rejections_keep_state_from_any_status（実際は 3 本とも失敗） | DETECTED | cases=1000 seed=46414e4448452d50524f50544553542d534545442d7631000000000000000001; minimal input: (0, 2, false, [Command("/help")]) |

#### 限界・未確認

- 関門は `run` と `entry` だけを戻す。`checkpoints`・`next_checkpoint`・`drafts`・`hints_opened` は `Op::Input` の経路が書かない欄で、戻さず、性質テストの一致の検査だけで守る
- 関門の行そのものへの変異は、統合試験だけを対象にする `check-mutations.sh` では記録できない
- `scripts/expected-test-counts.txt` は更新していない（版上げの前に `make update-test-counts` で更新する）

### 5.189 上限を超える貼り付けの切り捨て位置（TASK-283。JUDGE-24・TERM-16・D-60）

入力欄は 1 回の入力を 20,000 コードポイントまでとする（5.3 系。TASK-158・TASK-280）。従来は値全体の末尾で切っていたため、上限に近い下書きの途中へ長い文字列を貼ると、貼った分でなく入力済みの末尾が消えた（PoC-2 の F-9）。D-60（2026-10-15）は「貼った分だけを切る。`beforeinput` で挿入の前に止める。全体の末尾で切る処理は安全網として残す。ブラウザの自動試験への合格を採用の条件とする」と決めた。本節は、その実装と試験の記録である。

- 実行日: 2026-10-21。元にしたコミット: 13b7156（dirty。本変更を含む）
- コマンド: `cargo test --locked -p fandhe-workshop-web --test input_limit`・`scripts/check-mutations.sh crates/web/tests/mutations/JUDGE24-1.mutation`（JUDGE24-2 も同様）・`make test-browser`（通過。`input-limit.test.mjs` の JUDGE-24 の 7 件を含む）

#### 決めたこと（2026-10-21 に spec の D-65 として記録された）

| 論点 | 決定 |
|---|---|
| 切る位置 | 貼った文字列の先頭から、入る分（上限 − 選択の外の文字数）だけ残し、残りを捨てる（D-60） |
| 介入する条件 | `insertFromPaste`・変換中でない・下書きの `<textarea>`・欄と状態が揃っている・選択の終わりより後ろに既存の文字がある・合計が上限を超える、のすべてを満たすとき。選択の終わりが末尾なら末尾で切っても結果が同じなので、従来の安全網に任せる |
| キャレット | 残した貼り付けの直後（選択の始まり + 残した分の UTF-16 の長さ）。D-60 は位置を決めていないため本実装で決めた。spec の観察（末尾へ移る）とは違う |
| 案内 | 切ったときも JUDGE-23 の案内を出す。1 文字消すと消える |
| 対象外 | `insertFromDrop`・`insertText`（上限ちょうどでの途中への 1 文字）・`insertReplacementText`。安全網（末尾で切る）が受け持つ |

#### 実装

- 純粋関数 `input_limit::clip_paste`（DOM に触れない）。範囲外・逆順・サロゲートペアの途中・選択の外だけで上限を超える場合は `None`（介入せず安全網に倒す）。改行は LF として数える。はみ出した先頭 1 文字を値の末尾に置き、状態側の `clamp_draft` がちょうどその 1 文字を切って超過の印を立てる（超過の判定を 1 か所に保つため）
- glue の `wire_paste_clip`（`beforeinput`）。計画があれば `preventDefault`・欄へ値を書く・泡立つ `input` を合成・キャレットを置く。合成の `input` は上流の通常の入力の経路で `SetDraft` を届け、glue は状態を直接変えない。欄の今の値は読まず、状態の下書きから取る（`.value()` の例外は 1 件のまま）
- 変えたガード: `glue_contract.rs` の許可イベントに `beforeinput` を足し（4 種から 5 種）、DOM の呼び出し名（`input_type`・`data`・`selection_start`・`selection_end`・`set_selection_range`・`dispatch_event`・`new_with_event_init_dict`・`set_bubbles`）を `TextareaSync` として足した。`glue_scale.rs` の上限を 452 から 491 に、web-sys の feature に `EventInit` を足した（24 個から 25 個）。`GlueOp` の変種は増やさないが、操作範囲が従来の値の書き戻しを超えるため `PARTS_CONTRACT_VERSION` を 1 から 2 に上げた（codex の P1 指摘への対応）。許される操作の範囲（読むのは利用者の入力である選択の位置と貼り付けた文字列で、描画の結果ではない境界を含む）と契約の版 2 は、spec の FEND-20 と D-65 に反映した（2026-10-21）

#### 確認した事実（Chromium。Playwright の `ControlOrMeta+V`）

`beforeinput` は `inputType === 'insertFromPaste'` で発火し、`data` に貼り付けた平文が入る。`data_transfer` を使う必要はなかった（web-sys の `DataTransfer` は足していない）。

#### 試験

| 種別 | 試験 | 内容 |
|---|---|---|
| native | `input_limit.rs` の `judge24_` 11 件 | 収まる貼り付けは介入しない・途中の超過で後ろが残る・上限ちょうど・選択の置き換え・後ろが空・サロゲートペア・既存側のペアの途中・CRLF/CR・不正な位置・選択の外だけで超過・200,000 文字 |
| ブラウザ | J-1（CSP あり・なし） | 19,990 文字の 10,000 の位置へ 20 文字を貼ると、10 文字だけ入り末尾の既存の文字が残り、案内が出て、キャレットは 10,010 |
| ブラウザ | J-2 | 上限ちょうどで途中に貼っても値は変わらず、案内が出て、キャレットは残り、Backspace で案内が消える |
| ブラウザ | J-3 | 選択した 5 文字を置き換える貼り付けは、入る量が 5 文字増える |
| ブラウザ | J-4 | サロゲートペアを境界で切り、`isWellFormed()` が真 |
| ブラウザ | J-5（対照） | 末尾への超過の貼り付けは従来と同じ。収まる途中への貼り付けは既定の挿入で案内なし |
| ブラウザ | 試験の設定 | 状態の下書き・超過の印が欄の値と一致する |

#### 変異の結果（2026-10-21）

| ID | 対象 | 結果 |
|---|---|---|
| JUDGE24-1 | `input_limit.rs`: 介入の条件を反転する | DETECTED |
| JUDGE24-2 | `input_limit.rs`: キャレットを貼り付けの直後からずらす | DETECTED |

ブラウザの試験への変異の対照は行っていない（未実施）。

#### 既知の限界

- 取り消しは対象外（2026-10-21 決定。#1667）。`wire_paste_clip` は既定の挿入を止め、`set_value` で値を書いて合成の `input` を発火させるため、Ctrl+Z で貼り付けの前に戻らない。2 回押しても戻らず、取り消しの履歴がまるごと消える。詳細と理由は `docs/known-gaps.md` の JUDGE-24
- `document.execCommand('insertText')` で書く案を Chromium の試作で 3 通り試した（2026-10-21）。(1) 末尾にはみ出し 1 文字を置く: 取り消しは効かない。(2) 残す分だけを挿入する: Ctrl+Z 1 回で値と選択範囲が戻り、やり直しも効くが、超過の印（案内と `inputOverLimit`）が立たず J-1〜J-5 のうち 6 件が失敗する。(3) 2 回に分けて挿入する: 取り消しは効かず、`input` が 2 件になる
- 両立しないのは、超過の印を、末尾に置いたはみ出し 1 文字を `clamp_draft` が切る書き戻し（`sync_draft` の `set_value`）で立てており、この書き戻しが取り消しの履歴を消すためである。欄の値を通らずに印を立てる新しい経路（`RuntimeAction` など）は、判定が 2 か所に分かれ、glue に新しい種類の操作が加わり（FEND-20）、部品の契約の版上げと spec の変更が要るため、範囲に見合わないと判断した
- 上限ちょうどの欄への貼り付けは何も挿入せず、その後の Ctrl+Z はその前の操作を取り消す（観察）。末尾への超過の貼り付けと上限での打鍵も `set_value` を通るため、同じく取り消しの履歴が消えると推測する（推論・未確認）
- `insertFromDrop`・上限ちょうどでの途中への打鍵（`insertText`）は、末尾の既存の文字が消えうる（JUDGE-23 の K-1 は末尾でしか確かめていない）
- Chromium だけ。Firefox・Safari・実 IME は扱わない。取り消しの試作も Chromium（Playwright・Linux）だけで確かめた

#### spec への反映（D-65）

JUDGE-24・TERM-16 の証拠への `beforeinput` の自動ブラウザ試験の合格の追記、「検討中」から「確定」への変更、キャレットの位置の決定は、spec の D-65（2026-10-21）で記録した。`docs/spec` の参照は D-65 を含む版に上げた。

#### 既存の試験への影響

`scripts/expected-test-counts.txt` は更新していない（5.186 と同じ。版上げの前に `make update-test-counts` で更新する）。`scripts/test-browser.sh` は変えていない（`input-limit.test.mjs` は既に一覧にある）。

### 5.190 推定した OS ごとの修飾キーの表示（TASK-285 1/2。JUDGE-28・D-48）

案内文に出す修飾キー（Cmd／Ctrl）を、ブラウザの User-Agent からの推定で出し分ける機構の試験である（2026-10-21）。受講者が選ぶ欄と選んだときの切り替えは TASK-285 2/2（#588）が足した（5.191）。

- 方式: `static/app.js` が `navigator.userAgent` を `mount_app(root_id, user_agent)` の第 2 引数で渡す。Rust 側（`persist::mount_with_history`）が core の `learner_os::estimate` で `LearnerOs` を求め、`EngineHost::set_learner_os` で入力欄の表示（`key_hint` の束縛点）に渡す。Rust 側はブラウザの値を読まず、web-sys の feature は増えない（24 個のまま）。`glue.rs` のコード行も増えない。OS は表示だけの値で、`Session`・履歴には入れない
- 推定の規則: `Windows` を含めば `Windows`、`Macintosh` か `Mac OS` を含めば `Mac`（iPhone・iPad の UA を含む）、それ以外は `Unknown`。`Unknown` は修飾キーを出さない（Ctrl に倒さない）。UA-CH（`userAgentData`）は扱わない
- 表示: 入力欄の下の `[data-testid="key-hint"]`。Mac は「貼り付け: Cmd+V」、Windows は「貼り付け: Ctrl+V」、`Unknown` は 1/2 の時点では空（2/2 で 5.191 の促す文言に変えた）。入力欄なしの教材では出さない。`role`・`aria-*` は付けない

| 試験 | 内容 |
|---|---|
| `crates/core/tests/learner_os.rs` | Windows の Chrome・Edge、macOS の Chrome・Safari、iPhone、Linux・Android・ChromeOS・空文字・無関係な文字列、64 KiB の文字列と制御文字・非 ASCII（落ちない） |
| `crates/web/tests/modifier_key_hint.rs` | 修飾キーの語と案内文、ホストへ渡した OS が束縛値と描画に届くこと、dirty を立てないこと、`role`・`aria-*` が無いこと |
| `hello.rs` の単体試験 | OS ごとの文言と、入力欄なしで 0 件 |
| `tools/browser/modifier-key-hint.test.mjs` | `openPage` の `userAgent` で Windows・macOS・Linux の UA を与え、CSP あり・なしで案内の文字が合うこと、`?fixture=no-input` で 0 件、安全性の計数が 0 |

変異注入（実行日 2026-10-21、元にしたコミット 075246a の作業ツリー）:

| ID | 対象 | 結果 |
|---|---|---|
| JUDGE28-1 | `learner_os.rs`: Windows の判定の分岐を外す | DETECTED（`windows_の_ua_は_windows` ほか） |
| JUDGE28-2 | `modifier_key.rs`: Mac の語を Ctrl に取り違える | DETECTED（`修飾キーの語は_os_ごとに合う`） |

限界・未実施:

- 推定が外れたときに受講者が選択欄で直せるかの確認、Windows の実機・人の目での表示の確認は未実施（自動試験は UA の差し替えで macOS・Windows・Linux を模擬するだけである）
- 受講者が選ぶ欄、`Unknown` のときに選ぶよう促す文言、mount 後の切り替え（`key_hint` の dirty）は TASK-285 2/2（#588）で実装済み（5.191）
- 操作プロファイルの説明への適用と OS によらない操作の試験は TASK-392、右の案内文への結線の確認は TASK-456
- `dirty_binding_points.rs` の「一度は dirty になる」の検査から除いていた `key_hint` は、#588 で外した（5.191）
- `scripts/expected-test-counts.txt` は更新していない（版上げの前に `make update-test-counts` で更新する）

### 5.191 受講者が選ぶ欄による修飾キーの表示の切り替え（TASK-285 2/2。JUDGE-28・D-48）

5.190 の推定に、受講者が OS を選ぶ欄を併用する（2026-10-21、元にしたコミット 8855857）。推定が外れたときに受講者が直せるようにするためで、両方の併記・Ctrl での統一は採らない。

- 方式: 入力欄の下、送信ボタンの直後（Tab の順は入力欄、送信、選ぶ欄、停止。入力欄と送信の間に挟まない）に `<select data-testid="os-select" data-action-change="os">` を `<label>` の中に置く。上流の `wire_events` が `change` を `select.value` を payload にして dispatch する。web crate は DOM の値を読まない（`check-dom-reads` に影響しない）。`Cargo.toml` の feature・web-sys の feature・`glue.rs` は変えない
- 復号: 名前 `os` と payload が `mac`・`windows` に完全一致するときだけ `HelloAction::SetOs` にする。空・未知・大文字違いは `None`（何もしない）。値と OS の対応は `modifier_key.rs` の `os_choice_value`・`parse_os_choice` の 1 か所に置く
- 更新: 値が変わったときだけ `learner_os` を更新し `key_hint` を dirty にする。同じ値の選び直しは dirty を積まない。option の `selected` は推定に合うものにだけ静的に付け、束縛にはしない（目印の集合が mount 時から変わらない）
- 履歴・保存: OS は `Session`・履歴・保存先に入れず、保存の経路も動かさない。選んだ値は保存しない（再読み込みでは推定に戻る）
- `Unknown` の文言: 「OS を選ぶと、貼り付けのキーが表示されます」。Cmd・Ctrl は出さない（`modifier_label(Unknown)` は `None` のまま）
- 入力欄なしの教材: 欄を出さず、`os` の action を受けても状態を変えない（fail-closed）
- 寸法: `select` は操作の対象の最小寸法の判定に掛かるため、`static/app.css` で 24px の最小寸法を与える

| 試験 | 内容 |
|---|---|
| `crates/web/tests/modifier_key_hint.rs` | 選択の復号（`mac`・`windows` だけ）、選ぶと `key_hint` だけが dirty で値が切り替わること、同じ値と不正な値は何も変えないこと、`selected` が推定と選択に合うこと、欄に `role`・`aria-*` が無いこと、履歴・下書き・世代が変わらないこと |
| `crates/web/tests/dirty_binding_points.rs` | `run_os()` の列で `key_hint` が dirty になり、各回で束縛点への対応・目印の集合の不変を確かめる。「一度は dirty になる」の除外から `key_hint` を外した |
| `crates/web/tests/input_field_hidden.rs` | 入力欄なしで `key-hint`・`os-select` が 0 件、`os` の action が状態を変えない |
| `tools/browser/modifier-key-hint.test.mjs` | PoC-2 の OS-1〜OS-5 に相当。UA ごとの欄の値、選ぶと案内が変わること、案内と欄が同じノードのままであること、24x24 以上・`role`・`aria-*` なし・`label` の中、入力欄なしで 0 件（CSP あり・なし） |

変異注入（実行日 2026-10-21、元にしたコミット 8855857 の作業ツリー）:

| ID | 対象 | 結果 |
|---|---|---|
| JUDGE28-3 | `modifier_key.rs`: 選ぶ欄の `mac` を Windows に取り違える | DETECTED（`選ぶ欄の値の復号は_mac_と_windows_の完全一致だけを受ける` ほか） |
| JUDGE28-4 | `hello.rs`: 選んだときの `key_hint` の dirty を外す | DETECTED（`選ぶと_key_hint_だけが_dirty_になり_値が切り替わる`） |

限界・未実施:

- 推定が外れたときに受講者が選択欄で直せるかの人の確認は未実施（TASK-393・TASK-460）。自動試験は機構だけを確かめる
- Windows の実機・人の目での表示の確認は未実施（TASK-20 の計画・TASK-286・TASK-395）。自動試験は UA の差し替えによる模擬である
- 選んだ値は保存しない（再読み込みで推定に戻る）。安全側の既定である
- 操作プロファイルの説明への適用は TASK-392、右の案内文への結線の確認は TASK-456、欄の最終的な置き場は枠の担当。UA-CH による推定の強化は扱わない
- `scripts/expected-test-counts.txt` は更新していない（版上げの前に `make update-test-counts` で更新する）

### 5.192 サブパス配信の構成と相対パスの参照（TASK-106 1/3。DLV-1・DLV-2）

配信物（配信する設定の `dist/web`）の構成要素を 1 か所（`tools/browser/delivery.mjs` の `DELIVERY_COMPONENTS`）で宣言し、資源の参照がすべて相対パスで構成要素だけを指すことと、1 つのサブパスで開いたページの取得資源が構成要素だけであることを確かめる。後続の TASK-106 2/3（章への直接リンク 10 件）・3/3（名前の違う 2 か所のサブパスとルート配信）はこのモジュールを使う。

構成要素:

| 要素 | Content-Type | 備考 |
|---|---|---|
| `index.html` | `text/html; charset=utf-8` | サブパスの `/` で要求されたものも入口として扱う |
| `app.css` | `text/css; charset=utf-8` | |
| `icon.svg` | `image/svg+xml` | ヘッドレスでは取得が揺れるため、ページの要求の有無は問わず直接取得で確かめる |
| `app.js` | `text/javascript; charset=utf-8` | |
| `wasm/fandhe_workshop_web.js` | `text/javascript; charset=utf-8` | wasm-bindgen の glue |
| `wasm/fandhe_workshop_web_bg.wasm` | `application/wasm` | |
| `ir/main.json` | `application/json` | `IR_FILE` 指定時だけ存在（必須ではない） |

静的検査（`inspectDeliveryRefs`。ブラウザ不要・fail-closed）:

- ファイル集合は必須の構成要素をすべて含み、構成要素以外を含まない。通常ファイル以外は問題にする
- 参照の抽出: `index.html` の `href`・`src`、JS の静的 import・`export from`・動的 `import("…")`・`new URL("…", import.meta.url)`・`fetch("…")`、CSS の `url(…)`・`@import`
- 参照の規則: `./` 始まりか、スキームも `/` も持たない名前だけを許す。先頭 `/`・`//`・スキーム・`..`・`\` は問題。解決先が構成要素で実在することも確かめる
- 既知の例外 1 件: `app.js` の `./test-hooks.js` の動的 import は、`index.html` に `fandhe-test-hooks` の meta が無い（配信する設定）ときは取得されない経路として許す。件数が 1 でない・meta がある場合は問題にする
- 空振り防止の錨: index → `./app.css`・`./icon.svg`・`./app.js`、`app.js` → `./wasm/fandhe_workshop_web.js`、glue → `fandhe_workshop_web_bg.wasm` が抽出できなければ問題にする

glue の確認（2026-10-21、配信する設定のビルドの実物）: `wasm/fandhe_workshop_web.js` は `new URL('fandhe_workshop_web_bg.wasm', import.meta.url)` で本体を解決し、絶対パス・外部 URL・文字列リテラルの `fetch` を持たない（`fetch(module_or_path)` は変数）。このため `index.html`・`app.js`・`app.css`・`tools/build-web.sh`・`tools/serve.mjs` は変更していない。

試験（`tools/browser/delivery-subpath.test.mjs`。11 件）:

- 静的検査が 0 件であること
- 負の対照 8 件（配信物の写しを 1 か所ずつ壊す）: ルート絶対・外部・スキーム相対・root の外へ出る参照、構成要素以外のファイル、必須の構成要素の欠落、参照先の無い相対参照、試験用の meta。期待した理由の問題が出ること
- サブパス `/fandhe-delivery/` で開いたページの全要求が構成要素で、status 200・Content-Type が表どおり・gzip で、external・失敗・CSP 違反・ページ内エラーが 0 件であること
- `fandhe-pack compile` の出力を `ir/main.json` に置いた写しで、静的検査が 0 件、ページ内からの `./ir/main.json`・`./icon.svg` の相対の解決と取得がサブパスの下に閉じること

限界・未実施:

- web は実行時に IR を取得しないため、「ページが IR を取得する」ことは確かめていない（TASK-109・TASK-121 の後に取得資源の集合へ含める）。`ir/main.json` が `IR_FILE` とバイト一致で入ることは `scripts/test-build-web.sh` の (b)(e) が保証する
- 直接リンク 10 件は TASK-106 2/3（5.194）、名前の違う 2 か所のサブパスとルート配信は 5.196（3/3）
- `scripts/expected-test-counts.txt` は更新していない（版上げの前に `make update-test-counts` で更新する）

### 5.193 枠の区画と操作の名前の契約（TASK-511 1/2。SHELL-1・SHELL-10・SHELL-17）

画面の枠の区画・左右の取り付け点・枠が受け取る操作の名前と宛先の形を、版 1 の型として固定した（2026-10-21、元にしたコミット f26e94b）。置き場は `crates/web/src/shell.rs`（版は `shell::SHELL_CONTRACT_VERSION`）である。

- 方式: 識別子は予約の接頭辞 `shell-` に集め、`a11y` の `live-`・`terminal_area` の `line-` と同じく衝突を機械的に防ぐ。`tablist`・tab・tabpanel の id は `a11y` が持つため再定義せず、`PaneMount::pane_tab` で参照として結ぶ
- 上部の区画のうちヒント・チェックポイントは確定（`SectionStatus::Fixed`）、教材名・位置・「模擬」は名前と識別子だけの予約（`Reserved`。0 か 1 個、中身は固定しない）。決定の後に版 2 が足す
- action は `export-history`（空）・`import-history`（本文そのもの。検査は `open_history` に任せる）・`discard-and-start`（空）・`dismiss-notice`（`[a-z][a-z0-9-]*`、32 バイト以下）。`ShellAction::decode` は正規形だけを受け、`Debug` は読み込みの本文を出さない（SAFE-27）

| 試験 | 内容 |
|---|---|
| `crates/web/tests/shell_contract.rs` | 版と値の固定、`parse`・`from_name` の往復、`shell-` 接頭辞と内部の非重複、`a11y`・`terminal_area` の識別子・`aria-label` との非重複（受け入れ条件）、タブとの全単射、既存の action の名前との非衝突、decode と encode の往復、payload の拒否、`Debug` が本文を出さないこと、検討中の項目が版 1 に無いこと |

変異注入（実行日 2026-10-21、元にしたコミット f26e94b の作業ツリー。`scripts/check-mutations.sh` の出力）:

| ID | 対象 | 結果 |
|---|---|---|
| SHELL1-1 | `shell.rs`: ヒントの識別子を `a11y` の tablist の値にする | DETECTED（`a11y_と_terminal_area_の識別子と重複しない` ほか） |
| SHELL10-1 | `shell.rs`: 「次の章へ進む」の接頭辞を崩す | DETECTED（`次の章へ進むの文字の形` ほか） |
| SHELL17-1 | `shell.rs`: 捨てて始めるが空でない payload を受理する | DETECTED（`書き出しと捨てて始めるは空でない_payload_を拒否する`） |

限界・未実施:

- 通知の種類・通知の値・`view()` に定義にない区画や識別子が出ると失敗する適合の検査器は 2/2（#690）が持つ
- 予約の区画の中身は TASK-512・TASK-516 の決定の後に版 2 が足す。TASK-516 の本文との照合は spec を参照した範囲では矛盾を見つけていないが、決定前のため未確定である
- 読み込みでファイルの本文を得る glue の仕組みは `glue_ops` の許される操作に無く、TASK-531 が持つ（`parts::PARTS_CONTRACT_VERSION` の版上げが要る可能性がある）。ホストの `decode_action` への結線もしていない
- 「次の章へ進む」の action の名前と宛先は TASK-526 が決める

### 5.194 章への直接リンク 10 件のサブパス配信（TASK-106 2/3。DLV-1）

直接リンク 10 件を、新しい文脈（保存領域が空）で順に開き、1 ページが取得する資源が構成要素だけであることを確かめる。リンクと判定は `tools/browser/delivery.mjs`（`DIRECT_LINKS`・`DIRECT_LINK_FORMS`・`directLinkUrl`・`pageRequestProblems`）に置く。判定は `base` を引数で受けるので、3/3 の別のサブパス・ルート配信の試験もそのまま使える。

リンク 10 件（出所: PoC-10 が実行より前に固定した表。spec の `03-poc/static-delivery-localhost/test-data/direct-links.md`。エンジンは現時点でリンクを解釈しないため、開始点は不透明な識別子として扱う）:

| 章 | 開始点 |
|---|---|
| CH-0 | `s-0-1`・`s-0-2b` |
| CH-1 | `s-1-1`・`s-1-4`・`s-1-5`・`s-1-6` |
| CH-2 | `s-2-1`・`s-2-3` |
| CH-3 | `s-3-1`・`s-3-2` |

リンクの形は DLV-9 の案 (a)・(b) の 2 つを両方流す（どちらもサーバ側の経路を要さない）。`query` は `<base>?start=<開始点>`、`hash` は `<base>#/<章>/<開始点>`。形は 2026-10-21 に案 (a) の `query` に確定した（D-68）。採らなかった `hash` を `DIRECT_LINK_FORMS` から外すのは TASK-109（#527）で行う。開始点を 10 以上持つ IR は repo に無いため、TASK-109・TASK-110 で差し替える。

判定（`pageRequestProblems`。fail-closed。0 件が合格）:

- すべての要求が構成要素に写る。status 200・Content-Type が表どおり・gzip
- 同じ構成要素への要求が 2 回以上ない。必須の要求（`index.html`・`app.js`・`app.css`・glue・wasm）がそろう（`icon.svg` は揺れるため必須にしない）
- external・失敗・HTTP エラー・CSP 違反・ページ内エラー・console error・dialog・popup・クラッシュが 0 件。console warning は CSP に限らず 0 件
- 入口の応答が 200 で、開いた URL のまま（転送・書き換えなし）。10 ページの要求の集合（`requestKinds`）が同じ

試験（`tools/browser/delivery-direct-links.test.mjs`。15 件。サブパス `/fandhe-direct-links/`。配信物の写しに `fandhe-pack compile` の出力を `ir/main.json` として置く）:

- 正 2 件（`query`・`hash`）: 10 件ずつ開いて上の判定を満たす
- 負の対照（判定）11 件: 基準の記録が 0 件、構成要素の外の要求・404・Content-Type の違い・gzip なし・external・重複・必須の欠落・CSP 以外の警告・ページ内エラー・空の記録がそれぞれ期待の理由で検出される
- 負の対照（実ブラウザ）2 件: `index.html` の `./app.css` をルート絶対にした写しの検出、パスの形のリンク（DLV-9 の案 (d)。未採用）で入口が 404 になり検出されること

実測（2026-10-21、Chromium、配信する設定。`SKIP_WASM_OPT=1`）: 正の試験は 1 件あたり約 5.5 秒。console warning は 0 件だった。

限界・未実施:

- 配信する設定の `app.js` はクエリもハッシュも読まず、10 件とも既定の教材で開く。確かめたのは配信の側の性質（静的な解決・取得資源・エラー 0 件）だけである。章の開始点の題と状態（DLV-7）は TASK-110、実装は TASK-109 の範囲。リンクの形は 2026-10-21 に `?start=` に確定した（D-68）
- web は実行時に IR を取得しないため、「ちょうど 7 件（IR を含む）」は確かめていない。TASK-109・TASK-121 の後に取得資源へ含める
- 名前の違う 2 か所のサブパスとルート配信は 5.196（3/3）で確かめた
- `scripts/expected-test-counts.txt` は更新していない（版上げの前に `make update-test-counts` で更新する）

### 5.195 通知の口の契約（TASK-511 2/2。SHELL-16・SHELL-17・SHELL-18・SHELL-20・SHELL-31）

通知の種類 5 つ・通知の値 4 つ・通知欄の識別子・適合の検査器を、`shell` に版 1 のまま足した（2026-10-21、元にしたコミット bd475af）。版は上げない（1/2 の方針）。

- 方式: 通知欄の識別子 `shell-notice` は `shell` が持つ（`shell-` の接頭辞は `shell` の持ち分という 1/2 の境界を守る）。`a11y` は `shell::NOTICE_AREA_TESTID` を参照し、`save-notice`・`export-notice`・`stale-notice`・`secret-notice` と同じ任意の通知欄として扱う（`role="status"` を必須にし、`check_complete` では 2 個以上でないことだけを求める）。必須の 3 つは変えないため `A11Y_CONTRACT_VERSION` は 2 のままとした。`a11y.rs` 冒頭の「枠の通知欄を 4 つ目として足す場合は版上げ」は、必須の通知欄 `StatusNotice::ALL` を指すと読んだ
- 理由コードは版 1 では不透明な文字列（ASCII の英字で始まり、英数字・`_`・`-` の 1〜64 バイト）で、TASK-310 の `LoadReason` には結ばない。結ぶのは TASK-516。`LoadReason::ALL` の全件が形として通ることだけを試験で確かめる
- `dismiss-notice` の `decode` は正規形だけを見る 1/2 の方針を変えず、種類への結びは `NoticeKind::parse`（ホスト側）に置く
- 検査器は `view()` に定義にない区画・通知の種類・文面の識別子・操作の置き方が出ると違反を全件集める。生 HTML・通知欄の外の通知・種類に許されない操作・読み込みの本文の DOM への配置も違反にする。違反の文面に文字ノードの中身は含めない

机上の照合（spec の期待と定義の対応）:

| 要件 | 定義 |
|---|---|
| SHELL-1 | 区画・取り付け点の識別子は `shell-` に集まり、未定義は `undefined-testid`。左右はちょうど 1 個（`check_complete`） |
| SHELL-10 | 「次の章へ進む」の識別子と文字の形（1/2。変更なし） |
| SHELL-16 | `ImportRefused`。理由は必須、操作は `DismissNotice` |
| SHELL-17 | `CorruptHistory`。理由は必須、操作は `DiscardAndStart` |
| SHELL-18 | `SaveFailed`。理由は必須、操作なし（保存に成功すると消える） |
| SHELL-20 | `VersionConverted`。理由は任意、操作は `DismissNotice` |
| SHELL-31 | `PackUnloadable`。理由は必須、操作なし |

| 試験 | 内容 |
|---|---|
| `crates/web/tests/shell_contract.rs` | 通知の識別子・種類・文面の識別子の値の固定、`parse` の往復、`dismiss-notice` の受理、許す操作の表、理由の規則、`Notice::new` の 5 つのエラー、理由コードの正規形と `LoadReason::ALL` が形として通ること、属性と宛先、検討中の項目が無いこと |
| `crates/web/tests/shell_notice_sample.rs` | ダミーの通知 5 種を載せた枠が `shell::check_complete`・`a11y::check_no_undefined` の両方を通ること、危険な文字列のエスケープ、負の見本（未定義の区画・種類・文面の識別子、許されない操作、payload の食い違い、理由の欠落と不正、通知欄の外、重複、文面の部品の中身、取り付け点の重複と欠落、生 HTML、操作の置き方）、`role` なしで `a11y` が `notice-role` で失敗すること、違反の文面に文字ノードを含めないこと |
| `crates/web/tests/a11y_contract.rs` | `shell-notice` が役割つきで通り、役割なしで `notice-role`、2 個で `check_complete` が失敗すること |

変異注入（実行日 2026-10-21、元にしたコミット bd475af の作業ツリー。`scripts/check-mutations.sh` に SHELL の定義を渡した出力）:

| ID | 対象 | 結果 |
|---|---|---|
| SHELL1-2 | `shell.rs`: 検査器が未定義の `shell-` の識別子を違反にしない | DETECTED（`未定義の区画は失敗する`） |
| SHELL16-1 | `shell.rs`: 読み込みの拒否の理由を任意にする | DETECTED（`理由の規則` ほか） |
| SHELL17-2 | `shell.rs`: 壊れた履歴の通知から `DiscardAndStart` を外す | DETECTED（`許す操作の表の固定` ほか） |
| SHELL18-1 | `shell.rs`: 保存の失敗の値を正規形でなくする | DETECTED（`通知の識別子と種類の値の固定` ほか） |
| SHELL31-1 | `shell.rs`: 検査器が未定義の種類を違反にしない | DETECTED（`未定義の通知の種類は失敗する`） |

既存の SHELL1-1・SHELL10-1・SHELL17-1 も引き続き DETECTED である。`make check-mutations` の全件実行は、本件と無関係の `crates/core/tests/mutations/EVT12-11.mutation` の `find` が `projection.rs` に無く、定義の不正（終了コード 2）で止まるため未実施である。

限界・未実施:

- ホストの `decode_action` への `ShellAction` の結線と、通知欄の実際の描画・配置は画面のタスク（TASK-517・TASK-531〜TASK-534・TASK-545）が持つ。既存の `save-notice` を枠の通知欄へ寄せるかは TASK-534 が決める
- 通知の文面と次の行動（SHELL-36）は TASK-516・TASK-549 が持つ。理由コードを列挙に結ぶのは TASK-516
- 観察: `host.rs` の `ir-notice` は `role="status"` を持つが `a11y` の通知欄の一覧に無く、この画面に `a11y::check_no_undefined` を掛けると `status-on-non-notice` になる見込みである（未検証）。SHELL-31 を枠の通知 `pack-unloadable` へ移すのは TASK-545 の範囲

### 5.196 名前の違うサブパス 2 か所とルート配信（TASK-106 3/3。DLV-2）

同じ成果物（1 つの写しのディレクトリ）を 3 通りの配信で順に配り、どれにも同じ判定を流す。ビルド時に base を書き換えないこと（DLV-2）は、配信の前後で写しの全ファイルの sha256 が一致することで確かめる。判定と直接リンクは `tools/browser/delivery.mjs`（5.192・5.194）をそのまま使う。

| 名前 | base | 意図 |
|---|---|---|
| A | `/fandhe-bases-a/` | サブパス 1 つめ。他の試験の base と重ならない |
| B | `/another-repo-name/` | サブパス 2 つめ。A と長さ・文字が違う 1 段の名前（GitHub Pages のプロジェクトサイトを模す） |
| root | `/` | ルート配信 |

DLV-2 の組 0 の確認 6 項目（csp・pageErrors・consoleErrors・external・failed・httpErrors が 0 件）は `pageRequestProblems` が含む。

試験（`tools/browser/delivery-bases.test.mjs`。4 件。配信物の写しに `fandhe-pack compile` の出力を `ir/main.json` として置く）:

- base が互いに違い、A・B がルートでないこと（1 件）
- 正（1 件）: 静的検査が 0 件。3 通りの base それぞれで、入口（`./ir/main.json`・`./icon.svg` が base の下に解決され 200、`window.__fandheTest` が無い）と直接リンク 10 件×2 形（query・hash）が `pageRequestProblems` で 0 件。全ページの要求の集合（base からの相対）が 1 通りで、配信の前後で写しの sha256 が一致する
- 負の対照（実ブラウザ。入口の判定を [A, B, root] の順に並べて期待と比べる。静的検査が `root-absolute` を出すことも確かめる）2 件
  - `./app.css` を `/app.css` にした写し: [不合格, 不合格, 合格]。ルート配信だけではルート絶対の参照を見逃す
  - `./app.css` を `/fandhe-bases-a/app.css` にした写し（base を書き換えたときの形）: [合格, 不合格, 不合格]。サブパス 1 か所だけでは base に固定した参照を見逃す

実測（2026-10-21、Chromium、配信する設定。`SKIP_WASM_OPT=1`）: 正の試験は約 35 秒、負の対照は 1 件あたり約 3.2 秒。ルート配信でも `/favicon.ico` の要求は起きず、除外は足していない。

限界・未実施:

- web は実行時に IR を取得しないため、「ちょうど 7 件（IR を含む）」は確かめていない。TASK-109・TASK-121 の後に取得資源へ含める
- 直接リンクは配信の側の性質しか確かめない。章の開始点で開くこと（DLV-7）は TASK-110
- `scripts/expected-test-counts.txt` は更新していない（版上げの前に `make update-test-counts` で更新する）

### 5.197 幅 960px 以上の 2 列の骨格（TASK-517 1/2。SHELL-1）

ホストの `view()` を、`tablist`・2 つの tabpanel・取り付け点（`shell-left`・`shell-right`）の枠で組み、幅 960px 以上で成果物（左）と操作（右）を横に並べて `tablist` を隠す。実装は `crates/web/src/shell_frame.rs`、見た目は `crates/web/static/app.css` の節で、部品は `ArtifactView` だけを左、他を右に置く（`Review`・`Checkpoints` の置き場は暫定）。

- Rust の試験: `crates/web/tests/shell_frame_host.rs`（9 件）。既定のホスト・教材を読んだホスト（Step を進めた各段）・入力欄なしの教材のホストの `view()` が `shell::check_complete`・`shell::check_no_undefined`・`a11y::check_no_undefined` を通ること、取り付け点が 1 個ずつで部品が正しい側にあること、tab が `PaneTab::ALL` の順で選択が「操作」だけであること、各 tabpanel が対応する取り付け点を持つこと、`pane_of` の表、CSS の `@media (min-width: 960px)` の規則を固定する
- ブラウザの試験: `tools/browser/shell-columns.test.mjs`。幅 1280px と 960px で 2 領域が横に並び（上端が一致・左の右端が右の左端以下）、`pane-tabs` が `display: none`、横スクロールが無く、CSP 違反・ページ内エラーが 0 件。対照の 959px では `pane-tabs` が見え、2 領域は横に並ばない
- 変異: SHELL1-3（成果物を右へ振り分ける）・SHELL1-4（tab の順を逆にする）・SHELL1-5（境目を 1024px にする）。`FEND4-3` は `host.rs` の該当行が変わったため `find` を新しい行に合わせた。実行日 2026-10-21、元にしたコミット 11cc0c8 に本件の変更を載せた状態で、4 件とも DETECTED だった（`scripts/check-mutations.sh` に 4 件を指定）
- `make test-browser` の全件（794 件）が通った（2026-10-21）

限界・申し送り:

- 960px 未満の挙動は TASK-517 2/2（#699。5.203）が持つ。本件の時点では `tablist` が見えるが押しても何も起きず、2 領域は縦に積まれていた。片方だけを表示する CSS・クリックと矢印・Home・End の切り替え・4 つの幅の通しの試験は 2/2 が足す。選択を描き直すときは `keyed_list` を使う見込みである（束縛では aria・`tabindex` を書けない）
- ライブの欄（`live-response`・`live-left`）は tabpanel の外（root の直下）に置く（a11y の `live-in-tabpanel`）。TASK-450 が足す。a11y の `check_complete` をホストの view に掛けるのは、領域の `aria-label` が揃ってからである
- `shell-left` にだけ `aria-label="成果物"` を付けた。右の「案内とログ」「コマンドの入力」は TERM（TASK-431・450）の担当である

### 5.198 問い返しの後の下書きが連結されない（TASK-284 1/2。JUDGE-27・TERM-22。D-67）

試作（PoC-13 の AI 代理の準備）で、`st-1-4` の引数なしの `/new-idea`（問い返し）の後に全選択せず `/advance-phase` と打つと、残った下書きにつながって `/new-idea/advance-phase` になり、順序違い（`wrong_order`）ではなく `unknown_command` で拒否された。D-67（2026-10-21）で、問い返しの後は欄を空にすると確定した。

本番の `host.rs` の `submit` は、すでに問い返しを拒否に数えず（`events_summary`）、問い返しの後に下書きを空にして書き戻しを要求している。この動作を固定する試験が無かったため、本件は動作を変えず、入力欄を通る経路の試験と変異を足した。

証拠の種類: 自動ブラウザ・シミュレーター（AI の代理と同じ操作の再現。人の評価ではない）。

| 試験 | 内容 |
|---|---|
| `crates/web/tests/clarify_draft.rs`（3 件） | `ch1-pack` の `main/st-1-4` で、画面から届く `draft`・`submit` の action を流す。入力欄の値は「いまの下書き + 打った文字」と模す。問い返しの後に下書きが空で書き戻しが 1 回・拒否が 0 件、続けて打った入力の本文が `/advance-phase` ちょうどで分類が `wrong_order`（`unknown_command` でない）、その拒否の後に下書きが残り書き戻しが増えないこと |
| `tools/browser/clarify-draft.test.mjs`（4 件） | `?fixture=ch0-3` で `main/st-1-4` まで `page.keyboard` だけで進み、全選択せずに `/new-idea` → `/advance-phase` と打つ。送信は Enter と送信ボタン（Tab で移って Enter）、CSP あり・なし。問い返しの後に欄の値と状態の下書きが空・拒否の行が増えない・Step が進まない、続けた入力が `wrong_order`、拒否の後に欄と状態の下書きが残る。`raw-trigger` の click 0 回・安全の計数 0・エンジンの警告と無視 0 件 |
| `tools/browser/ch0-3-journey.mjs` | 台帳の項目「問い返しの後の入力欄が空」を 1 つ足した（`ch0-3-run`・`csp-runs` の両方に効く）。`journey` が `main/st-1-4` でも止まれるようにした |

変異（実行日 2026-10-21、元にしたコミット 1f8b74e に本件の変更を載せた状態）:

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| JUDGE27-1 | `host.rs` の `events_summary`: 問い返しも下書きを残す判定に数える | `問い返しの後は下書きが空になり_書き戻しが要求される` | 同左・`問い返しの後の拒否では下書きが残り_書き戻しは増えない`・`問い返しの後に続けて打った入力は順序違いの拒否になり_未知のコマンドにならない` | DETECTED |
| JUDGE5-4 | `host.rs`: 拒否の後も下書きを空にする（既存） | `全拒否の分類で…` | 既存の 2 件 | DETECTED |

手元の対照（コミットしない）: 上の置換を入れて配信の設定外の試験用ビルドを作り、`clarify-draft` の 4 件と `ch0-3-run` の 4 件が失敗した。`clarify-draft` は問い返しの後に欄が空にならず待ちが時間切れになり、`ch0-3-run` は台帳の項目「問い返しの後の入力欄が空」で失敗した。置換を戻すと全件が通った。

確かめないこと・限界:

- 拒否の後に下書きを選択状態にして次の入力で置き換える処理は 5.200（TASK-284 2/2）で足した。`ch0-3-journey.mjs` のドライバの全選択（`replaceDraft`）の見直しの結果も 5.200 に書いた
- フォーカスの位置、実 IME、人の確認は扱わない
- 書き戻しは 0 ミリ秒のタイマーで非同期のため、それより速く打った場合の取りこぼしは観察も固定もしていない
- `scripts/expected-test-counts.txt` は更新していない（件数の照合は版上げ PR だけで働く。更新は別の PR で `make update-test-counts`）

### 5.199 開始点を指す直接リンク `?start=`（TASK-109 1/2。DLV-9・SAFE-7・SAFE-33・D-68）

直接リンクは IR を 1 つ（固定の `./ir/main.json`）に保ち、URL の `start` を IR の `starts` の ID と完全一致で照らして開始点を選ぶ（D-68。2026-10-21）。一致すればその開始点で開き、一致しなければ開かずに固定のコード（`start_unknown`・`start_missing`）だけの通知を出す。値は画面・タイトル・コンソール・保存先に出さない。履歴の鍵の開始点は URL の `start` が指す開始点に当たる（SAVE-14 の読み。指定なしは先頭の開始点なので既存の鍵は変わらない）。

実装: `static/app.js` が `start` だけを読み（`new URLSearchParams(location.search).getAll("start")`。2 件以上は一致し得ない値に倒して断らせる）、`mount_app` の第 3 引数で渡す。照合は純関数 `crates/web/src/start_select.rs`（バイト単位の完全一致のみ。前方一致・大文字小文字の無視・正規化・復号をしない）、断ったホストは `EngineHost::refused_start`（session なし・鍵なし・保存先を開かない）、選んだ開始点は `EngineHost::with_session_at` で記録・鍵・起動の分岐に通る。通知の文言（`ir_notice.rs`）は仮置きである。

- Rust の試験: `start_select.rs` の単体試験（指定なし・一致・不一致 14 形・開始点なし・コードに入力値が無い）、`test_fixture.rs` の単体試験（`start=` の対だけを取り除いた完全一致の選択と既存の否定が変わらないこと・開始点ごとの節点・不一致の拒否・鍵）、`crates/web/tests/start_link_host.rs`（3 件。通知がコードだけ・断ったホストが鍵を持たず保存済みの履歴を開かない・選んだ開始点が鍵に入り指定なしの鍵が従来と同じ）、`tick_interval.rs`（`app.js` の URL の読み取りをファイル・件数・形・読むキーで限定する例外を足した）
- ブラウザの試験: `tools/browser/direct-link-start.test.mjs`（試験の設定。CSP あり・なしの 4 件）。開始点 3 件の教材（`?fixture=multi-start`）を、指定なし（先頭で新しく始まる）・各開始点（クエリの順序違いを含む）・一致しない値 10 形（未知の ID・`../` を含む 2 形・大文字違い・前方一致・末尾の空白・空・重複・センチネル・5,000 文字）で開く。一致で現在の Step が変わり、`s-1` の履歴は `start=s-4` で開かれず `start=s-1` で復元される。不一致は通知が 1 件・入力欄とログが 0 件・`irRefusal` が `start_unknown`・センチネルが DOM・タイトル・コンソールに 0 件・保存先の全体が断っている間も再読み込みの前後も不変・断った後も指定なしで履歴が開く・配信元以外への通信が 0 件・CSP 違反・ページ内エラーが 0 件
- 配信する設定の `delivery-direct-links`・`delivery-bases` は、採らなかった hash の形を外して query の 1 形で流す（5.194・5.196 の「2 形」の記録は当時のもの）

限界・申し送り:

- 配信する設定の web は実行時に IR を取得しないため、`start` を受け取っても照合する IR が無く、画面は今までどおり（session なし）である。配信する設定で直接リンクが開始点で開くことは、`./ir/main.json` の実行時の取得と開始（`Control::Begin`・開始点からの初期状態の構築。TASK-217・TASK-294）が入った後でなければ確かめられない。本件では取得を実装していない
- 試験の設定の開始は、契約の試料の開始状態の節点を `starts` の節点に合わせて作る（`test_fixture::session_at`。`Session::from_state` の例外の 1 か所）。開始点の `effects` は適用しない
- 開始点を増やしても IR の合計とハッシュが変わらない試験は TASK-109 2/2（#529）、章の開始点の再読み込み・往復（DLV-7・DLV-8）は TASK-110、`?ir=` を読まないことの試験は 5.222（TASK-168）

### 5.200 拒否の後の下書きが選択状態になり連結されない（TASK-284 2/2。JUDGE-27・TERM-22。D-67）

D-67（2026-10-21）で、拒否の後は拒否された下書きを入力欄に残し（JUDGE-5）、下書きの全体を選択状態にして、次に打った文字で置き換える（末尾に続けて入力されない）と確定した。選択の範囲は下書きの全体と読む。問い返しの後に空にする前半は 5.198（1/2）で固定済みで、本件は後半を実装した。

実装:

- `host.rs`: 純関数 `selects_draft`（拒否のときだけ真）、通知先 `set_draft_select_request`、選択の予約の印 `draft_select_pending`。`submit` が拒否で下書きを残したときに印を立てて通知する。印は、下書きが変わる action（`draft`・`complete`）と次の送信で下りる。進行の tick など下書きを変えない `update` では下りない
- `glue.rs`: `on_draft_select_request` が `set_timeout(0)` で `sync_draft(root, true)` を予約する。`sync_draft` は印が立っているときだけ `set_selection_range(0, u32::MAX)` を呼ぶ（終端は欄の長さへ丸められるので長さを数えない）。変換中は選ばない。`field.value()` の読みは 1 か所のまま（`make check-dom-reads`）
- 打鍵が 0 ミリ秒のタイマーより先に届いた場合は、`draft` の action で印が下りるため全選択しない（打った文字を消さない。そのとき文字は下書きに続く）

証拠の種類: 自動ブラウザ・シミュレーター（人の評価ではない）。

| 試験 | 内容 |
|---|---|
| `crates/web/tests/reject_draft_select.rs`（5 件） | 拒否の後に選択の要求が 1 回・書き戻しが 0 回・印が真、問い返しの後は要求が 0 回で次の拒否で 1 回、打鍵で印が下りる、下書きを変えない操作で印が残る、通知先が無くても落ちない。受け付けの後は選ぶ下書きが無く、`st-1-4` に受け付けになる入力が無いため確かめない |
| `tools/browser/reject-draft-select.test.mjs`（4 件） | `?fixture=ch0-3` の `main/st-1-4` で、`/new-idea` → `/advance-phase`（拒否）の後に選択が欄の全体になり、全選択せずに `/new-idea` と打つと欄の値と状態の下書きがちょうど打った文字列になり、Enter で送ると受け取った入力が打った文字列ちょうどになる。送信は Enter と送信ボタン（Tab で移って Enter）、CSP あり・なし。いずれも 35 項目が通った |
| `tools/browser/reject-keeps-draft.test.mjs` | `assertKept` に、選択が下書きの全体であることを足した（`page.fill` の後のキャレットは末尾なので、選択が無ければ失敗する）。busy の分類は処理の終了後も見る |
| `tools/browser/ch0-3-journey.mjs` | 台帳の項目「main/st-1-4 拒否の後の下書きが全体の選択状態」を足した（`ch0-3-run`・`csp-runs` の両方に効く） |

`replaceDraft`（Ctrl/Cmd+A）の見直し: 受け付けの後の空欄など他の場面でも無害で、ドライバの操作を変えると通しの他の段に影響するため、残した。選択の確認は `reject-draft-select` が全選択を使わずに受け持つ。

変異（実行日 2026-10-21。元にしたコミット 298e9d7 に本件の変更を載せた状態）:

| ID | 対象 | 期待した失敗 | 結果 |
|---|---|---|---|
| JUDGE27-2 | `host.rs` の `submit`: 拒否の後に下書きを選択状態にしない | `拒否の後は選択の要求が出て_下書きは送った本文のまま残り_書き戻しは要求されない` | DETECTED（5 件が失敗） |
| JUDGE27-1・JUDGE5-4 | 既存 | 既存の試験 | DETECTED のまま |

手元の対照: ブラウザ試験側の対照（`select = false` を入れた試験用ビルドで `reject-draft-select` などが失敗すること）は未検証。

glue の行数: `MAX_GLUE_CODE_LINES` を 491 から 509 に上げた（5.166 の表にも記録）。`INIT_ALLOWED` に `set_draft_select_request` を足し、`glue_contract.rs` の許可に `set_draft_select_request`・`draft_select_pending` を足した。

確かめないこと・限界:

- 送信ボタンの click で拒否された場合のフォーカスの位置。選択は欄に残るが、フォーカスはボタンにある（D-67 にフォーカスの記述が無い）。試験の観察では、Tab で欄へ戻っても選択は保たれ、置き換えが成立した（Chromium）
- 実 IME・Safari・Firefox での `setSelectionRange` の挙動（CI は Chromium だけ）
- 0 ミリ秒のタイマーより速い打鍵（全選択を止める側に倒すが、そのとき打った文字は下書きに続く）
- 接頭辞を読めずに残した下書き（拒否ではない）は選ばない
- `scripts/expected-test-counts.txt` は更新していない（別の PR で `make update-test-counts`）。人の確認は扱わない

### 5.201 開始点の数を増やしても IR の合計と内容のハッシュが変わらない（TASK-109 2/2。DLV-9・D-68）

5.199 の直接リンクは IR を 1 つ（`ir/main.json`）に保つ。旧方式（PoC-10 の `make-start-irs.mjs`）は開始点ごとに `starts` の順を変えた写しを `ir/start-<id>.json` として配り、開始点の数だけ IR が複製され（20 開始点で約 1.8MB）、写しごとに内容のハッシュが別になった（F-1。別の章のリンクで「変換して開きました」が出た）。新方式のこの利点が崩れないことを回帰から守る。

定義: `content_hash`（`crates/contract/src/content_hash.rs`。版 1）は本文の木だけを除いた IR の SHA-256 で、`starts` を含む。開始点を足した教材（別の IR）のハッシュは変わるのが正しい。確かめるのは「同じ IR を別の開始点から開いたとき同じ」であり、「バイト数がまったく増えない」ではない（`starts` の項目の分は増える）。

- ブラウザの試験: `tools/browser/start-count-invariance.test.mjs`（5 件）
  - (P1) 配信する設定。`min-3step-pack` の開始点 1 件と、`chapter.toml` に固定の文字列で 2 件足した 3 件の教材を compile し、それぞれ `ir/main.json` に置いた配信物の写しを作る。どちらも `inspectDeliveryRefs` が 0 件、IR のファイルが `ir/main.json` の 1 つだけ、`starts` の数が 1 と 3（空振り防止）、`starts` を除いた IR が一致、3 件の IR の大きさが 1 件の 2 倍未満（かつ増える）。指定なしと各 `?start=` を新しい文脈で開き、`pageRequestProblems` が 0 件、要求の種類が全ページ・両配信物で同じ（開始点ごとの資源が無い）
  - (P1 対照) 3 件の配信物に旧方式の写し `ir/start-<id>.json`（`starts` の先頭を入れ替えたもの）を足すと、IR のファイルが 3 件増え、合計が 2 倍以上になり、静的な検査が「構成要素でないファイル」を 3 件出す
  - (P2) 試験の設定（CSP あり・なしの 2 件）。開始点 3 件の 1 つの IR（`?fixture=multi-start`）を `start=s-1`・`s-4`・`s-7` で、新しい文脈ごとに開く。現在の節点が開始点の節点（空振り防止）で、1 手進めて保存させた履歴の `versions.content_hash` が 3 件とも同じ小文字 16 進 64 文字、`start` の欄・鍵は開始点ごとに違う
  - (P2 対照) 開始点ごとに別の IR を配る `?fixture=chapter-a`・`?fixture=chapter-b` では `content_hash` が割れる（測り方に感度があることを示す）
- Rust の試験: `crates/web/src/test_fixture.rs` の単体試験 2 件。同じ IR は指定なしと 3 開始点のどれで開いても `content_hash` が同じで、開いた開始点と節点の組が 3 通りある。旧方式を模した `starts` の先頭の入れ替えの写しは、`content_hash` が互いに違う
- 出力の規則: 失敗の文・診断に IR の中身・鍵・ハッシュの値・履歴の中身を出さない（件数・真偽・記号名だけ）。一時ディレクトリは必ず消す

限界・申し送り:

- 配信する設定は実行時に IR を取得しないため、ハッシュは試験の設定の保存先でだけ確かめる。配信する設定の `ir/main.json` は `tools/build-web.sh` がそのまま入れる（`scripts/test-build-web.sh` が確かめる）ので、P1 は写しに置いて静的に検査する。配信する設定での開始点つきの取得とハッシュの照合は、TASK-217・TASK-294 の後
- 開始点の数が違う 2 つの IR のハッシュの一致は確かめない（設計上違う）

### 5.202 承認の差し戻しの再承認を Step を分けて書く（TASK-62 1/2。PACK-32・D-69）

1 Step の `approval` で承認 → 差し戻し → 再承認を表す語は足さないと決まった（D-69、2026-10-21）。この決定の下で、差し戻しの Step と再承認の Step を分けた教材が書けることを、`crates/pack/tests/approval_revise_split.rs` の `pack32_` の 4 件と実パック `crates/pack/tests/data/revise-split-pack` で固定する。

- `validate` が診断 0 件で通ること。`pack.vocab` は `1.0` のままで、`E-VOCAB-MISMATCH` が出ない
- `compile` した IR の承認の `input` が `{"kind":"approval"}` だけで、差し戻しの Step の `next` が再承認の Step を指すこと
- IR を core の `Session` で動かし、差し戻しの Step での承認・却下の拒否、差し戻しでの達成と再承認の Step への移動、再承認の Step での差し戻し・却下の拒否、承認での達成と終端への移動、案の `state` が「差し戻し」から「承認済み」へ変わることを確かめる
- 1 Step で再承認を表すつもりの欄（`reapprove: true`）を承認の `input` に足すと、`validate` と `compile` が `E-PARSE` になること（語を足していないことの対照）

変異は置かない。Rust のソースを変えず、新しく守る実装の分岐が無いためである（5.125 と同じ扱い）。`next` の `when` による枝分かれは、core が実行時に評価しないため範囲外とした。

### 5.202b 開始条件の評価と片方だけの構造の誤り（TASK-51。PACK-18・EP-9）

開始点の `requires`（開始条件）と `unmet`（満たさないときの文面の ID）について、片方だけの記述を `E-STRUCTURE` にし（pack）、状態に触れる前の評価を関門 `start_gate::check_start` として core に置いた。評価を core に置くのは、呼び出し元（`Control::Begin`・`Refused` の記録。TASK-217・TASK-240）が core にあるためである。

- core の試験: `crates/core/tests/start_gate.rs`（8 件）。条件なしと満たす条件で開始してよいこと、満たさない条件で `StartUnmet`・文面の ID・空でない次の行動が返り、呼ぶ前後の `SessionState` のバイト列が一致すること、無い開始 ID・無い Step の実体が `UnknownStart` になること、評価できない条件と文面の無い `unmet` でも断ること、モジュールの開始点が実体の根で評価されることを固定する
- pack の試験: `crates/pack/tests/start_requires.rs`（7 件）。`requires` だけ・`unmet` だけ・`requires: null` と `unmet` の組・モジュールの開始点が、`compile` と `validate` の両方で 1 件の `E-STRUCTURE`（file は定義ファイル、item は `start:<ID>`、値を写さない）になること、両方ある・両方無いが通ること、`testdata/condition-vocab-pack` と `testdata/module-reuse-rounds/test-data/pack` の変換した IR で関門が断ってから開始してよくなること
- 既存の試験の修正: 片方だけの開始点を使っていた `validate_refs`・`text_slots`・`condition_vocabulary`・`vocab_command` の入力を両方そろえた。`vocab_command` の `author17_unmet_only_counts_as_ep9` は、`compile` の関門で到達しなくなったため `author17_one_sided_ep9_is_refused_by_compile_gate` に置き換えた
- 変異: PACK18-1（`requires` だけの開始点を見逃す）・PACK18-2（開始条件が偽でも開始してよいとする）。実行日 2026-10-21、元にしたコミット 1f8b74e に本件の変更を載せた状態で、2 件とも DETECTED だった（`scripts/check-mutations.sh` に 2 件を指定）

限界・申し送り:

- `requires: null` は読み込みの型で省略と区別できないため、省略と同じ扱いにした
- `Control::Begin` への結線・`Refused` の記録・web の画面は後続のタスクである（PACK-18 は web への結線を未検証としている）

### 5.203 幅 960px 未満のタブの切り替え（TASK-517 2/2。SHELL-1・A11Y-5）

幅 960px 未満（959px・360px）で、選んだタブの領域だけを表示し、クリックと矢印（左右）・Home・End で切り替える。実装は `crates/web/src/shell_frame.rs`（`tablist` を keyed list `pane_tabs` にし、親の `data-pane` に選択を写す）・`crates/web/src/host.rs`（`select-pane`・`pane-key` の 2 action と、表示の状態 `pane`）・`crates/web/static/app.css`（`@media not all and (min-width: 960px)` で選んでいない tabpanel を `display: none`）である。

要点:

- 選択は表示の状態で、`ops`・履歴・保存先には書かない。再読み込みで既定（操作）に戻り、Step が変わっても捨てない。切り替えは保存を起こさない
- `aria-selected`・`tabindex` は束縛では書けない（a11y 版 2）ので、`tablist` を keyed list にした。項目の root を `button[role=tab]` そのものにして、子の作り直しでフォーカスが失われないようにした。tabpanel と `.sh-panes` は静的なまま描き直さない（要素の同一性。FEND-3）
- キーの action の `data-keys` に Esc を含める。keydown の属性は最も近い 1 つにしか届かず、含めないと tab にフォーカスがある Esc が root の Esc に届かず 0 回になる（FEND-6）。`data-keydown-prevent-default` は Esc にも掛かるが、ボタンの上の Esc に既定の動作は無い。Home・End はこれが無いとページを先頭・末尾へ動かす
- 境目は 960px 以上の規則と補い合う `not all and (min-width: 960px)` にした（`max-width: 959px` だと小数の幅に穴が空く）。`hidden`・`aria-hidden` は 960px 以上で両方を見せる必要があるため使わない
- 受け付けない payload は `None`（fail-closed）。`select-pane` は `PaneTab::id` の完全一致だけ、`pane-key` は矢印・Home・End・Escape だけを受ける

試験:

- Rust: `crates/web/tests/shell_frame_host.rs`（16 件。本件で 7 件追加）。keyed list の親・`data-pane`・キーの属性・項目の root、選択と dirty（`pane_tabs` だけ。同じ値では空）、`a11y::tab_key_target` に沿った巡回、`pane_tabs` が `data-bind-list` にあること、`decode_action` の許可と拒否、切り替えが Session に触れないこと、CSS の規則。`dirty_binding_points.rs` は `pane_tabs` を dirty にする列（`run_pane`）を足し、`no_hidden_triggers.rs` は `shell_frame.rs` の `data-payload` の使用を許可の表に足した
- ブラウザ: `tools/browser/shell-tabs.test.mjs`（9 件）は 959px・360px で、tablist の表示・tab の大きさ（24px 以上）・既定は操作だけが見えること、クリック、矢印・Home・End（`keydown` の `defaultPrevented`）、Alt+ArrowRight で変わらない対照、試験の設定で tab にフォーカスがある Esc が `escCount` でちょうど 1 回であること、959px→960px→959px の境目を確かめる。`tools/browser/shell-widths.test.mjs`（4 件）は 1280px・960px・959px・360px で CH-0 から CH-1 の 6 Step（`main/st-1-1`〜`st-1-6`）を通して `main/st-2-1` に入るまで進める。960px 未満では操作の前に要るタブをクリックし、959px・360px で 6 回切り替えた。台帳は 4 幅とも 30/30。`shell-columns.test.mjs` の 959px の対照は、隠れた側の矩形が 0 になって空振りで通る「縦に積む」の判定をやめ、見える領域がちょうど 1 つであることに直した
- 変異: SHELL1-4（`find` を新しい行に合わせた）・SHELL1-6（矢印・Home・End が修飾キーつきとして扱われる）・SHELL1-7（Escape を root の Esc に結ばない）・SHELL1-8（操作を選んだときに成果物の領域が隠れない）。実行日 2026-10-21、元にしたコミット 1f8b74e に本件の変更を載せた状態で、4 件とも DETECTED だった（`scripts/check-mutations.sh` に 4 件を指定）
- `make test-browser`: 807 件のうち 805 件が通った（2026-10-21）。失敗 2 件の内訳は、`tick-dispatch`（`data-payload` を持つ要素を指せる行だけに限っていた検査に `select-pane` を許可して直した。修正後に単独で通過）と、`secret-scan-time` の dense（10MB の保存の p95。単独で再実行すると通過し、p95 は 287ms で、全件を流している間の負荷による揺れと見ている。推測）である。修正後の全件の流し直しは未実施である。`esc-once` の 1 件も 1 回目の全件実行で失敗したが、単独の再実行では 22 件とも通った（再現せず）

限界・申し送り:

- キーで切り替えると、選んだタブへフォーカスも移す（ホストの要求の通知先を glue が受け、描画の後に `focus()` する。`glue_contract` の呼び出しの表に `focus` を `Click` として足した。分類の整理は `docs/known-gaps.md` A11Y-5）。ブラウザ試験が、移ったことと Enter で選択が戻らないことを確かめる
- 既定のタブと Step をまたぐ保持は仮置きである（SHELL-2 が未決）。変化があった側の印は SHELL-5 の担当である
- CH-1 から始める直接リンクが無い（TASK-109）ため、CH-1 の 6 Step は CH-0 から通して確かめた
- 360px の横スクロールと 24px 未満の操作対象は TASK-518・TASK-368 の担当で、assert せず `shell-widths` の diagnostic に件数だけを残す（2026-10-21 の実測は、横スクロール 0px・24px 未満の対象 1 件。4 幅とも同じ）
- 960px 以上で tabpanel の役割が 2 つ残ることの読み上げは A11Y-21 の担当である。右の領域の `aria-label` とライブの欄（tabpanel の外に置く）は TASK-450 の担当である

### 5.204 直接リンク 10 件と再読み込みの状態（TASK-110 1/2。DLV-7・D-68）

DLV-7 は、新しいブラウザの文脈で章 X の直接リンクを開くと章 X の開始点の Step と状態で開き、再読み込みしても同じ章・同じ状態になることを求める。PoC-10 は決定の前の形（`?ir=` と開始点ごとの IR）で 10 件を確かめただけで、D-68 の `?start=` の形では未確認だった。これを試験の設定（`TEST_HOOKS=1`）の画面で確かめる。

- 教材: `?fixture=ch0-3-links`（`crates/web/src/test_fixture.rs` の `ch03_links_ir_bytes`）。20 Step の通しの教材の `starts` を 10 件に書き換え、`pack.id` を `ch0-3-links` にした IR を実行時に作る。`ch0_3/ir.json` は変えない。10 件の ID と節点は `CH03_LINK_POINTS` で、ブラウザ試験のリンクの表（`delivery.mjs` の `DIRECT_LINKS`）と揃える
- ブラウザの試験: `tools/browser/direct-link-reload.test.mjs`（CSP あり・なしで各 3 件）
  - (T1) 10 件それぞれを新しい文脈で開く。保存先が空であること、現在の節点が開始点の節点で、その単位が `DIRECT_LINKS` の章と一致すること、状態が入った直後（`entered`・導出の状態なし・状態の語が空・処理中でない・通知なし）であること、再読み込みの後も同じ状態で、操作が無いので保存先が空のままであること、配信元以外への通信・ページ内エラー・CSP 違反・コンソールのエラーが 0 件であること
  - (T2) リンク 1 を開き、選択肢 `opt-right` を実クリックして「受付済み（確認待ち）」にし、保存を待ってから再読み込みする。履歴が復元され、同じ節点・同じ状態・同じ Step ごとの状態・同じ状態の語で開く
  - (対照) 10 件の期待の節点が互いに異なり、`start` なしで開いた節点は 10 件のうち 9 件と違う（`start` を無視する誤りなら 9 件が落ちる）。開始点が 1 件の `?fixture=ch0-3` に `start=s-2-1` を付けると `start_unknown` で断られる
- Rust の試験: `test_fixture.rs` の単体試験。`?fixture=ch0-3-links` の選択（`start` の併用と余分なキー）と、10 件それぞれで開く節点・鍵が 10 通りに分かれること・未知の ID と 1 件の教材の断り
- 期待の題・章: 実行時に `ch0_3/ir.json` から節点の単位と題を引き、リンク 1 の題（`画面の右側を選ぶ`）を固定の文字列の錨にして読み取りの空振りを防ぐ
- 出力の規則: 失敗の文にリンクの番号・真偽・件数だけを出し、鍵・履歴の中身を出さない

実行の記録（2026-10-21）: ローカルで Chromium により 6 件が通った。`start` を無視する改変での落ち方の手元の確認は行っていない（各リンクの現在の節点を 1 件ずつ照合するため、無視すれば 9 件が落ちる設計である。確認は未実施）。

限界・申し送り:

- 配信する設定は実行時に IR を取得しないため、DLV-7 は試験の設定でだけ確かめた。配信する設定での確認は TASK-217・TASK-294 の後
- Step の題は DOM に出ていない（TASK-525）ため、現在の節点と IR の題の対応で判定した。TASK-525 の後に DOM の題の照合を足す
- 開始点の `effects` は適用しない（5.199 と同じ）。1 操作の後の再読み込みはリンク 1 だけ（PoC-10 の固定の手順と同じ）
- 章 A の履歴が保存済みのときの章 B のリンク（DLV-8）と修正前の症状（F-1）の対照は TASK-110 2/2（5.206 を参照）。本件の `?fixture=ch0-3-links` を使える
- 節番号は並行する PR とマージ順で衝突しうる（5.204 は作業時点で未使用の番号）

### 5.204b 変換して開いたときの版の違いの通知（TASK-535 1/2・2/2。SHELL-20）

版の違いを変換して開いたときに、枠の通知欄（`shell-notice`）へ通知を 1 件出す。実装は `crates/web/src/shell_notice.rs`（通知欄の描画。keyed list `shell_notices`）・`crates/web/src/host.rs`（表示の状態 `version_notice`、`dismiss-notice` の action）である。判定は core の `history_open` のままで、web は条件を複製しない。

経路:

- `install_recorded`（実行環境の差し替え）で通知を一度消し、変換して開いたときだけ立て直す。立てるのは履歴ファイルの読み込み（`open_history` が `Converted`）と起動時の復元（`boot_session` の `Restored` が `notes` を持つ）である。配信する設定で変換して開くのは起動時の復元だけである
- 開かなかったとき（表示だけ・開かない）は通知にも状態にも触れない（SAVE-19）。前に変換して開いた実行環境は残るので、通知も残る
- 閉じる操作（`dismiss-notice`、payload は `version-converted`）は表示の真偽だけを変える。正規形でない payload・既知でない種類・他の枠の操作（`export-history`・`import-history`・`discard-and-start`）は受けない。描いていない種類は何も変えない
- 通知は root の先頭（tabpanel の外）に置く。幅 960px 未満でも見える。親は常に 1 個で、項目の出し入れだけで表すので root は全置換にならない
- 画面に出すのは固定の文と固定の識別子だけで、履歴由来の文字列（版・参照元）は出さない（SAFE-40）

試験:

- Rust: `crates/web/tests/version_notice.rs`（8 件）。同じ版・変換して開く・変換の後の同じ版・開かない場合に触れない・起動時の復元 4 通り・閉じる操作の復号と効果・束縛の目印・履歴由来の文字列を出さない。`dirty_binding_points.rs` は `shell_notices` を他の教材が要る field に足し、`no_hidden_triggers.rs` は `shell_notice.rs` の `data-payload` の使用を許可の表に足した。単体の試験は `shell_notice.rs`（2 件）
- ブラウザ: `tools/browser/version-notice.test.mjs`（4 件。CSP あり・なし）。V-i-1 で通知が 1 件出て文が一致し、幅 360px で成果物のタブを選んでも見え、閉じると 0 件になり root が同じ要素のままであること、V-0（変換なし）・V-iii-1（表示だけ）で 0 件であること。`make test-browser` は 2026-10-21 に全件通った（失敗 0）

操作の後の保存（TASK-535 2/2。#730。SHELL-20・VER-4。PoC-6 の S-13 相当）:

- 保存の値の `versions` は、`history_to_save`（`RecordedSession::envelope_for_save` → `build_envelope`）がいま載っている IR から求め直す。変換して開いた後の保存は、履歴を書いた側の版を引き継がず、開いた側の版で書かれる。実装は変えず、試験で固定した
- Rust: `crates/web/tests/version_notice.rs` に 3 件を足した（計 12 件）。履歴ファイルの読み込みで変換した後の操作の保存の `versions` が開いた側の `record_versions` と一致し、元の履歴の版とは違い、`ops` が続きに増え、その値が新しい版では変換なしで・古い版では変換で開く。起動時の復元で変換した場合も同じ。対照として同じ版で開くと同じ版のまま書かれる
- ブラウザ: `tools/browser/version-notice.test.mjs` に 6 件を足した（計 10 件。CSP あり・なし）。V-i-1・V-ii-2 は、読み込みでは保存先へ書かれず（`storageRead` が空・localStorage の鍵が 0 件）、開いた直後の Step が履歴の `snapshot` と同じで、操作の後に保存された `versions.content` が開いた側の教材の `pack.version` と一致し、履歴の `content`・`content_hash` と違い、その値を新しいページで開くと `open` で通知が 0 件である（hash の求め方は JS に持ち込まない）。起動時の復元は、開ける包みの `content_hash` だけを書き換えて保存先へ置き、再読み込みで `restored` と通知 1 件になり、操作の後の `versions` が書き換える前の値と一致する。値は `versions` だけを読み、本文・`ops`・`snapshot` は出さない
- 実行の結果（2026-10-21）: `cargo test --locked -p fandhe-workshop-web --test version_notice` は 12 件が通った。`tools/browser/version-notice.test.mjs` を `scripts/test-browser.sh` と同じ準備で単独に流し、10 件が通った。`make test-browser` の全件は手元では流していない（CI の `browser-test` に任せる）。`scripts/expected-test-counts.txt` は更新した（元にしたコミット dac8dcb。TASK-535 とは無関係な束の増減も実測のまま含む）。2026-10-22 に main を取り込んだときは、main の値（元にしたコミット 04fae88）を基準にし、Rust の行はマージ後の `make test` の実測に合わせ（`version_notice` は 12 件。main の後続の変更の分も含む）、ブラウザの合計は main の 837 件に本変更の 6 件を足した 843 件とした（ブラウザ試験は手元で流していない）
- 対照の自己確認（保存の IR を開いた履歴の版に変えて落ちることの確認）は未実施である。試験が正しい理由で落ちることの確認は残っており、別の機会に行う

限界・申し送り:

- 文面（`VERSION_CONVERTED_NOTICE`）と置き場は仮置きである。最終の文面・配置・見た目は TASK-549・TASK-534 が持つ
- 内容以外の軸（エンジン・語彙・参照元）だけの変換でも通知を出す。SHELL-20 が直接は決めていない判断である（推論）
- 他の通知の種類の移設と書き出し・読み込みの結線は TASK-516・TASK-531 などの担当である
- 開く側のエンジンの上書き（`openingOverride`。V-iv-1 など）を使う行は 2/2 の対象外である。保存の `engine` は実物の版になり、上書きした版にはならない（試験の口の限界。配信する設定には上書きが無い）
- 変異注入の定義は足していない

### 5.205 承認の「直す」の誤った対象の候補と表示名（TASK-62 2/2。PACK-42・D-69）

対象: 承認の `input.revise`（語彙 1.5。`input.revise.candidates`・`input.revise.candidates[].label`）。`crates/pack/tests/approval_revise_candidates.rs`（`pack42_`）と、実パック `crates/pack/tests/data/revise-candidates-pack`。契約の型は `crates/contract/tests/input_spec_contract.rs`（`input_spec_contract_version_is_3`（5.209 で 2 から 3 に更新））。

確かめること:

- 候補 3 件（表示名つき 2 件・省略 1 件）の宣言が `validate`・`compile` を診断 0 件で通り、IR の `input` に宣言の順・値のまま出ること
- 空の候補・空白だけの `id`・`id` の重複・空白だけの `label` が `E-STRUCTURE`、未知の欄・型の違いが `E-PARSE` になること
- 規則の `target` が候補に無いと `E-STRUCTURE`、候補に入れた対象は通ること。診断に候補・`label`・`target` の値が出ないこと
- `pack.vocab` が 1.4 で `revise` を使うと `E-VOCAB-MISMATCH` が 1 件、省略では出ないこと。`choice` の `revise` は語彙の語に数えないこと
- core の `Session` で、正しい対象の「直す」だけが受付になり、誤った候補・宣言に無い対象・対象なしは受付にならず Step が進まないこと（拒否の文面・分類は判定の持ち分で固定しない）
- モジュールの Step の規則の `target` に `${...}` を使うと、置換前の値では誤検出せず、展開後の値を `compile` が検査すること

変異（`crates/pack/tests/mutations/`）。2026-10-21 に 3 件を個別に流し、すべて DETECTED だった（全件の `make check-mutations` は流していない）。

| ID | 誤り | 検出する試験 |
|---|---|---|
| PACK42-1 | 候補の ID の重複の検査を外す | `revise_check_reports_each_violation` |
| PACK42-2 | `input.revise` を使用と数える判定を常に偽にする | `pack42_older_vocab_declaration_is_a_mismatch` |
| PACK42-3 | 規則の `target` と候補の突き合わせを外す | `pack42_rule_target_must_be_a_declared_candidate` |

語彙の上限は 1.5 になった（5.1xx の「上限は 1.4」は当時の記録で、書き換えない）。版の上限を前提にする試験（`history_open.rs` の `ver7_`・`vocabulary_terms.rs`）は 1.5 に追従した。

### 5.206 章 A と章 B の往復で保存済みの履歴を壊さない（TASK-110 2/2。DLV-8・SAVE-14・D-68）

DLV-8 は、同じオリジンで章 A を進めた後に章 B のリンクを開いても章 A の履歴が消えず、「変換して開きました」の通知も出ず、章 A のリンクへ戻ると続きが出ることを求める。D-68 のリンクは `?start=<開始点 ID>` で IR は 1 つであり、保存先の鍵は教材の ID と開始点の ID で決まる（SAVE-14。`host.rs` の `history_key`）。このため章ごとに履歴が分かれるはずである。試験の設定（`TEST_HOOKS=1`）の画面で、5.204 と同じ `?fixture=ch0-3-links` を使って確かめる。章 A はリンク 1（`s-0-1`）、章 B はリンク 7（`s-2-1`）。

- ブラウザの試験: `tools/browser/direct-link-roundtrip.test.mjs`（CSP あり・なしで各 2 件）。1 つの browser context（同じ保存領域）で進める
  - (T1) 章 A を実クリック 2 手（`opt-right`、行 `cards/notice`）で、どの開始点でもない `main/st-0-2a` まで進め、保存の鍵が章 A の 1 本だけであることを確かめる。次に、章 B のリンクを開く。`fresh`・開始点の節点・`reloaded` の行 0 件・章 A の値の SHA-256 が不変、の 1 つの式 `mixed` が偽で、世代が 1・処理中でなく・通知が出ず（契約の属性・通知の枠・本文の語）・鍵が章 A の 1 本のままであること。再読み込みと、他の 8 件のリンクでも同じ。章 A のリンクへ戻ると `restored` で `main/st-0-2a`・同じ Step ごとの状態・増分と世界の一致で開き、再読み込みしても同じ。章 A を次の 1 手進めても鍵は章 A の 1 本のまま。章 B を 1 手進めると鍵がちょうど 2 本になる。配信元以外への通信・ページ内エラー・CSP 違反・コンソールのエラーは 0 件
  - (対照) 章 A の値を章 B の鍵の位置へ写し（鍵が章で分かれない修正前の症状の再現）、章 B のリンクを開くと `mixed` が真になる。失敗の文には固定の印 `DIRECT-LINK-HISTORY-MIXED` を含める
- 出力の規則: 失敗の文にリンクの番号・節点の ID・真偽・件数だけを出し、鍵・履歴の値・下書きを出さない。要約（SHA-256）は page の中で計算し node へ出さない

実行の記録（2026-10-21）: ローカルで Chromium により 4 件が通った。回帰として `chapter-keys`・`direct-link-reload`・`direct-link-start`・`key-stability` の 14 件も通った。鍵を固定の文字列にした配信物で T1 が落ちることの確認は未実施（対照の写しで `mixed` の感度を確かめた）。

限界・申し送り:

- 起動時の「変換して開きました」の通知は host がまだ描かない（TASK-535 1/2）。通知の 0 件の判定は描画された後に効き、それまでは `fresh`（変換して開く経路は `restored` の一種）が実質の判定である
- 配信する設定は実行時に IR を取得しないため、試験の設定でだけ確かめた。配信する設定での確認は TASK-217・TASK-294 の後
- 開始点の `effects` は空で、前の章の世界状態を持たない（5.199 と同じ）
- 対照は修正前の配信物の再ビルドではなく、保存先の写しによる再現である
- `scripts/test-chapter-key-control.sh` の置換前の文字列が #1681 以降の `host.rs` と合わず、対照が動かない。本件の範囲外として別に扱う
- 節番号は並行する PR とマージ順で衝突しうる（5.206 は作業時点で未使用の番号）

### 5.207 複数の対象を 1 つずつ分けた確認の正誤（TASK-63 1/2。PACK-33・D-69）

決定（D-69、2026-10-21）は「`answer` に複数の問いを持たせる語は足さず、対象ごとに Step を分けて書く」である。`crates/pack/tests/split_answer.rs`（実パックは `crates/pack/tests/data/split-answer-pack`。3 枚のカードを 3 Step に分けた形で、`pack.vocab` は `'1.0'`）が、次の 5 件を固定する。

| 試験 | 確かめること |
|---|---|
| `pack33_split_pack_validates_without_newer_vocab` | `validate_dir` が 0 件で、`compile_dir` が成功すること |
| `pack33_cli_compile_and_vocab_accept_split_pack` | `fandhe-pack compile` が終了コード 0・標準エラー出力が空で、`vocab` の 3 Step の拡張点の欄が空・(ii) が 0 であること |
| `pack33_each_card_has_its_own_verdict` | カードごとに `judge_confirm` が正答のちょうど 1 つだけ `Confirmed` を返し、他は `rj-wrong-kind` の誤答になること。3 枚の正答は互いに違う |
| `pack33_session_records_per_card_correctness` | `Session` で 3 枚を順に通し、`derive_records` が 3 件で最初の回答が正答・誤答・正答になること。誤答の後の再選択は正答の記録になる |
| `pack33_merged_step_yields_a_single_record` | 対照。まとめた 1 つの `answer` の Step も検査を通るが、記録は 1 件しかできないこと（PACK-33 の「決定の前の現状の観察」） |

- CLI の `fandhe-pack validate` は TASK-403（#647）まで未実装（終了コード 2）のため、`validate_dir` と、同じ読み込みを通る `compile`・`vocab` の CLI で代えた。TASK-403 の結線後に置き換えてよい
- `Event` から `CheckObservation` への写像は TASK-304 の担当で未実装のため、試験の中の `choose` が代用する（`crates/core/tests/confirm_record_boundary.rs` と同じ規則）
- 変異は置かない。Rust のソースを変えず、新しく守る実装の分岐が無いためである

### 5.208 入力のない確認だけの Step と 1 択の `choice`（TASK-63 2/2。PACK-35・PACK-19・D-69）

決定（D-69、2026-10-21）は「入力のない確認だけの Step を表す語は足さず、選択肢が 1 つだけの `choice` で表す」である。受講者の操作が 1 回増えることは限界として受け入れる（TERM-30）。`crates/pack/tests/read_only_step.rs`（実パックは `crates/pack/tests/data/read-only-step-pack`。`pack.vocab` は `'1.0'`）が、次の 7 件を固定する。

| 試験 | 確かめること |
|---|---|
| `pack35_one_choice_pack_validates_without_newer_vocab` | `validate_dir` が 0 件で、`compile_dir` が成功すること |
| `pack35_cli_compile_and_vocab_accept_one_choice_pack` | `fandhe-pack compile` が終了コード 0・標準エラー出力が空で、`vocab` の拡張点の欄が空・(ii) が 0 であること |
| `pack35_one_extra_operation_then_answer` | `Session` で、1 択の選択 1 回は受付で止まり、誤答は受付のまま、正答で達成になること。`derive_records` が 1 件で、最初の回答が誤答、再選択が正答 |
| `pack35_inputless_confirm_step_is_structure_error` | 入力が無く `confirm` だけを持つ終端でない Step が、`validate_dir`・`compile_dir` の両方で `E-STRUCTURE` を 1 件返すこと。選択肢の ID・拒否文の ID が診断に出ない |
| `pack35_inputless_confirm_in_module_is_structure_error` | モジュールの定義の同じ形（`confirm` が `${` を含む場合を含む）が、実体の数によらず定義ごとに 1 件になること |
| `pack35_cli_compile_rejects_inputless_confirm_step` | CLI の `compile` が終了コード 1・診断を標準出力・標準エラー出力が空であること |
| `pack35_final_step_without_input_is_allowed` | 終端の Step は、入力が無くても `confirm` を持てること（PACK-19 の除外） |

- CLI の `fandhe-pack validate` は TASK-403（#647）まで未実装のため、`validate_dir` と、同じ読み込みを通る `compile`・`vocab` の CLI で代えた
- 範囲外: PACK-19 の「両方無い」のうち確認も持たない途中の Step の拒否（既存の教材・試験が依存するため。`docs/known-gaps.md`）。読むだけの Step の画面（TERM-30）
- 2026-10-22 追記（#1709）: 確認も持たない途中の Step の拒否を実装した。記録は 5.226

変異 PACK35-1（入力のない確認だけの Step の拒否を外す）の確認。

実行日: 2026-10-21
元にしたコミット: 30c1ff0-dirty
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/PACK35-1.mutation

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 |
|---|---|---|---|---|
| PACK35-1 | crates/pack/src/expand.rs: 入力のない確認だけの Step の拒否を外す | pack35_inputless_confirm_step_is_structure_error | pack35_inputless_confirm_step_is_structure_error, pack35_inputless_confirm_in_module_is_structure_error, pack35_cli_compile_rejects_inputless_confirm_step | DETECTED |

### 5.209 保存・書き出しを Step の入力として宣言する語（TASK-64 1/2。PACK-34・D-69）

対象: 入力方式 `env_action`（語彙 1.5。`env_action`・`input.action`・`input.action=save`・`input.action=export`）。契約の型は `crates/contract/tests/input_spec_contract.rs`（`env_action_`・`input_spec_contract_version_is_3`）、宣言の検査は `crates/pack/tests/env_action_input.rs`（実パックは `crates/pack/tests/data/env-action-pack`）、実行時の受付は `crates/core/tests/env_action_save.rs`（IR は `crates/core/tests/data/env_action_1/ir.json`。教材の `compile` とバイト一致することを pack 側の `pack34_core_test_ir_matches_the_pack` が確かめる）。

確かめること:

- 保存の Step が `validate`・`compile` を診断 0 件で通り、IR の `input` に宣言のまま出ること。`input_field: hidden` の教材でも診断が出ないこと
- `action` の欠け・未知の値・型の違い・未知の欄が `E-PARSE`、`processing` を持つ Step が `E-STRUCTURE` になること。診断に宣言の値が出ないこと
- `pack.vocab` が 1.4 で `env_action` を使うと `E-VOCAB-MISMATCH` が 1 件、省略では出ないこと
- core で、保存の成功が保存の Step を受付にし、確認を省く宣言なら達成して次の Step へ進むこと（`CheckpointSaved` → `Accepted` → `Achieved` → `Transitioned` → `Entered`）。世代が変わらないこと
- 確認の宣言が `none` でないと `Accepted` で止まり、`none` の理由が空白だけなら `Warning` だけで状態が変わらないこと
- 書き出しの Step・`env_action` の無い Step・宣言が読めない Step・処理中の Step・保存に失敗したとき（ID のあふれ）は、保存で受付にならないこと
- 受講者のコマンド・選択・取り消しの入力が `env_action` の Step を受付にせず、状態が変わらないこと
- 同じ操作列から同じログと状態になり、そのログから `Projection` を導けること

実装の過程で `Projection::take_starts` に `CheckpointSaved` を操作の先頭として加えた（`InputReceived` を伴わない `Accepted` が保存の直後に続くため）。上限で止めた操作の巻き戻しの範囲を保存の操作の分だけにするための変更で、保存の後に遷移の上限で止まる場合の試験は足していない（未検証）。

変異（`crates/pack/tests/mutations/`・`crates/core/tests/mutations/`）。2026-10-21 に 3 件を個別に流し、すべて DETECTED だった（全件の `make check-mutations` は流していない）。

| ID | 誤り | 検出する試験 |
|---|---|---|
| PACK34-1 | `env_action` の宣言を型で読む検査を飛ばす | `pack34_unreadable_declarations_are_parse_errors` |
| PACK34-2 | 保存の成功の判定を外し、保存に失敗したときも受付にする | `pack34_failed_save_does_not_accept` |
| PACK34-3 | `action` の照合を外す（書き出しの Step も保存で達成になる） | `pack34_export_step_is_not_achieved_by_save` |

範囲外: 書き出しの成功を達成に結ぶ実行時の経路（`docs/known-gaps.md` の PACK-34）。保存・書き出しのボタンの画面。

### 5.210 誤答の拒否文で前後の切り替えを案内する形（TASK-64 2/2。PACK-36・D-69）

決定（D-69、2026-10-21）は「誤答の拒否に前後比較のビューを結ぶ語は足さず、拒否文（`message`・`next_action`）で前後の切り替え（ART-2）を案内する。比較を自動で見せないことは限界として受け入れる」である。`crates/pack/tests/wrong_answer_compare.rs`（実パックは `crates/pack/tests/data/wrong-answer-compare-pack`。`pack.vocab` は `'1.0'`）が、次の 5 件を固定する。

| 試験 | 確かめること |
|---|---|
| `pack36_guide_pack_validates_without_newer_vocab` | `validate_dir` が 0 件で、`compile_dir` が成功すること |
| `pack36_cli_compile_and_vocab_accept_guide_pack` | `fandhe-pack compile` が終了コード 0・標準エラー出力が空で、`vocab` の (ii) が 0 であること |
| `pack36_wrong_answer_resolves_to_guide_text` | 誤答が `rj-unchanged` の誤答になり、拒否文の次の行動が前後の切り替えの案内であること。正答は `Confirmed` |
| `pack36_session_shows_guide_and_stays_on_step` | `Session` で誤答を送ると受付のまま達成が増えず、判定が拒否で、差し込み後の文面が案内であること。誤答で出るイベントは判定と入力の記録だけで、比較を自動で見せないこと。続く正答で達成になること |
| `pack36_binding_field_on_answer_is_rejected` | 対照。`confirm.answer` に比較のビューを結ぶ欄を足した写しは `E-PARSE` になり、IR を出さないこと（語を足していないこと） |

- CLI の `fandhe-pack validate` は TASK-403（#647）まで未実装（終了コード 2）のため、`validate_dir` と、同じ読み込みを通る `compile`・`vocab` の CLI で代えた
- 画面での前後の切り替えの見え方（ART-19）はブラウザで確かめていない。画面のタスクの持ち分である
- 変異は置かない。Rust のソースを変えず、新しく守る実装の分岐が無いためである
- `rejections.<id>` に未知の欄を足したときの挙動は、試験では固定していない（未確認）

### 5.211 確認 `answer` の選択肢の `label` を必須にする検査（TASK-65 1/2。PACK-37・D-67）

決定（D-67、2026-10-21）は「`label` を必須にし、無い選択肢は検証で診断として止める」である。`crates/pack/tests/answer_label_required.rs` は、`label` の無い（欄が無い・`null`）選択肢を持つ教材パックを `validate_dir` に通し、`E-STRUCTURE`（文面は `validate::ANSWER_LABEL_REQUIRED`）が Step ごとに 1 件、終了コード（`diag::exit_code`）が 1 になることを固定する。`validate` だけの検査で、`compile` は検査しない。

| 試験 | 確かめること |
|---|---|
| `pack37_missing_label_is_reported_with_exit_code_1` | 選択肢の 1 つに `label` が無い教材が、1 件の `E-STRUCTURE`（定義ファイルと `step:<id>`）・終了コード 1 になること |
| `pack37_null_label_is_reported` | `label: null` も同じ |
| `pack37_all_labels_validate_clean` | 対照。全選択肢に `label` があれば 0 件・終了コード 0 |
| `pack37_one_diagnostic_per_step` | 1 つの Step で複数の選択肢が欠けても 1 件。2 つの Step で欠ければ 2 件 |
| `pack37_format_a` | 形式 A（TOML）でも検出すること |
| `pack37_module_step_is_reported_at_the_module_file` | モジュールの Step の欠けが、モジュールのファイルに付くこと |
| `pack37_choice_label_is_not_confused` | 入力方式 `choice` の `label` を取り違えないこと |
| `pack37_compile_does_not_check` | 同じ教材の `compile_dir` が成功し、`validate` だけが止めること |
| `pack37_message_has_no_values` | 秘密情報に似た選択肢の ID・`label` が診断に写らないこと |

既存の試験への影響（2026-10-21 に `cargo test -p fandhe-workshop-pack` で洗い出した）と扱い。

- 編集してよい教材パック（`testdata/condition-vocab-pack`、`testdata/pack-defects/base/format-b-v1_1` と上書き 3 件、`crates/pack/tests/data/vocab-pack`・`read-only-step-pack`・`split-answer-pack`・`wrong-answer-compare-pack`）と、試験のコード中のインラインの宣言は、`label` を足した
- 編集できない教材パック（spec の写し、IR の凍結物を持つ `fill-pack`）は、試験の共通部の `without_answer_label_required` で、この 1 種の診断だけを除いて見る。除いた件数が 0 でないことも確かめる
- 語彙の版 `1.0` を宣言して `answer` を使っていた 4 つの教材（`read-only-step-pack`・`split-answer-pack`・`wrong-answer-compare-pack` と `split_answer.rs` の対照）は、`label` が語彙 1.3 の語のため `1.3` の宣言に変えた。1.3 より前を宣言した教材は `answer` を使えなくなる（`docs/pack-authoring.md`）
- `vocab_declaration.rs` の `pack31_correct_packs_have_no_false_positives` は、確認の `label` を持つようになった 2 つの教材を、入力方式 `choice` の `label` だけを持つ前提の一覧から外した（残る 3 件で取り違えの確認は続く）
- `vocab_command.rs` の `author17_label_not_counted` は、土台の教材が `label` を持つようになったため、足す向きから外す向きに変えた
- `metrics.rs` の `me3_` は、`condition-vocab-pack` に `label` を足して増えたバイト数に合わせた

変異（`crates/pack/tests/mutations/`）。

| ID | 誤り | 検出する試験 |
|---|---|---|
| PACK37-1 | `validate` が `label` の欠けを検査する呼び出しを外す | `pack37_missing_label_is_reported_with_exit_code_1` |

2026-10-21 に呼び出しを手で外して確かめ、`answer_label_required` の 9 件のうち 7 件が失敗し、戻すと全件通った（`make check-mutations` は流していない）。

範囲外: `fandhe-pack validate` の CLI からの終了コードの確認（本体は TASK-403。ここでは `diag::exit_code` で代えた）。`compile` だけを通した IR では `label` の無い選択肢が ID のまま出る（`docs/pack-authoring.md` に明記した）。置換後に `label` が空になる場合は扱わない。

TASK-65 2/2（#509）の扱い: 任意のままにして教材側ですべての選択肢に `label` を付ける案は、D-67（2026-10-21）が採らなかった。そのため、教材側の規約と、全 Step の画面で選択肢に ID の文字列が出る件数を数える試験は、この repo では足さない。結論: 変更なし（Rust・試験・試験の件数のいずれも変えない）。

- `label` の無い教材パックは `validate` が止めるため、検査を通った教材パックで選択肢が ID のまま画面に出る経路は無い（PACK-37・TERM-25）。例外は `compile` だけを通した IR で、`docs/pack-authoring.md` に記載済みである
- `st-1-4`・`st-2-7` の画面で表示名が出ることの確認は、TERM-25（画面のタスク TASK-446）と INTRO-18（教材のタスク）が担う。教材パックは教材 repo にあり、この repo の `testdata/` の spec の写しは D-67 の前の形（`label` 無し）のまま残す

### 5.212 自然文の内容で規則を分ける書き方の拒否（TASK-68。PACK-41・D-43）

決定（D-43、2026-10-12）は「判定語（部分一致・正規表現など）を語彙に足さず、内容で規則を分ける書き方は `validate` が拒否する」である。2026-10-21 の確認で、現状は `on.text` に `{contains: ...}` を書いても `validate`・`compile` とも 0 件で通り、実行時は `on.text == true` だけを見るため内容の条件が無視されると分かった。そのため `input.rs` に `E-PARSE` の検査を足した（診断コードは増やさない）。`crates/pack/tests/text_rule_content.rs` が `validate_dir` と `compile_dir` の両方で固定する。

| 試験 | 確かめること |
|---|---|
| `pack41_content_value_in_on_text_is_rejected` | 真偽値でない `on.text`（mapping・文字列・数値・null）が `E-PARSE` 1 件で、値が出ないこと |
| `pack41_unknown_key_beside_text_is_rejected` | `text: true` と並ぶ未知のキー（`contains` など）が `E-PARSE` で、キー名だけが出ること |
| `pack41_unknown_key_on_text_step_is_rejected` | `text` の Step で `text` キーのない `on` の未知のキーと、mapping でない `on` |
| `pack41_unsafe_key_name_is_not_echoed` | 識別子として安全でないキー名を写さないこと |
| `pack41_plain_text_rules_pass` | 対照。`text: true`・`false`・`cancel`・既知のキーの組み合わせと 5 方式の教材が通ること |
| `pack41_other_steps_are_not_checked` | `text` でない Step で `text` キーのない `on` は本検査の対象外であること |
| `pack41_module_text_rule_is_checked` | モジュールの `text` の Step の規則が、モジュールのファイルに付いて拒否されること |
| `pack41_format_a_is_rejected_too` | 形式 A（TOML）でも同じ |
| `pack41_when_content_condition_is_rejected_by_compile` | `when: {contains: ...}` を `compile` が `E-PARSE` で拒否すること |
| `pack41_rule_choice_ignores_words`（core の `text_judge`） | 語を含む文と含まない文で、選ばれる規則・分類（`free_text_unjudged`）が変わらないこと |

変異（`crates/pack/tests/mutations/`）。

```text
実行日: 2026-10-21
元にしたコミット: af94c08（作業ツリーに本変更）
コマンド: scripts/check-mutations.sh crates/pack/tests/mutations/PACK41-1.mutation crates/pack/tests/mutations/PACK41-2.mutation
```

| ID | 対象 | 期待した失敗 | 結果 |
|---|---|---|---|
| PACK41-1 | crates/pack/src/input.rs: 規則の `on` の未知のキーの拒否を外す | pack41_unknown_key_beside_text_is_rejected | DETECTED |
| PACK41-2 | crates/pack/src/input.rs: `on.text` が真偽値でないことの拒否を外す | pack41_content_value_in_on_text_is_rejected | DETECTED |

範囲外: `validate` が `when` の条件の語彙を検査しない全般の不足（TASK-42 の領域）。`text` でない Step の規則で `text` キーのない `on` の未知のキーは検査しない（core は黙って飛ばす）。規則の `on` の 9 キーの集合は core（private 定数）と pack（`RULE_ON_KEYS`）の二重管理である。spec の TASK-68 の「Rust の変更は無い」という見積もりは、現状が拒否していなかったため成り立たなかった（spec 側への差し戻し事項）。`fandhe-pack validate` の CLI からの終了コードの確認は TASK-403 の実装待ち。

### 5.213 案内レベルの語と本文・ヒントの出し分け（TASK-49 1/2。PACK-17）

水準 L で出す本文を C → B → A の順に選ぶこと（`guidance::body_for`）、ヒントを契約の `visible_hints` で選ぶこと、`A`・`B`・`C` 以外の水準の語を読み込みで `E-PARSE` にすることを確かめる。`body_levels` の鍵 `A` の `E-STRUCTURE`、`E-GUIDANCE-LEAK`、`E-HINT-HIDDEN` は 5.214（TASK-49 2/2）が確かめる。

| 試験 | 内容 |
|---|---|
| `crates/pack/tests/guidance_levels.rs` | 本文の倒れ方 4 種 × 3 水準、ヒント 4 種の水準ごとの選択、モジュールの 2 実体の一致、受け付ける水準の全組、拒否する値（`pack.guidance`・`guidance_levels`・`starts[].guidance`・`body_levels` の鍵・ヒントの `levels`）の項目と 1 件の `E-PARSE`、値を診断に写さないこと、形式 A（TOML）でも同じこと |
| `crates/pack/tests/defect_packs.rs` | D23 が `E-PARSE`・`chapters/ch-13/chapter.yaml`・`scope` で検出され、static の検出が 9/9 になること |
| `crates/pack/src/guidance.rs` の単体試験 | `body_for`・`read_guidance_levels`・ヒントの `levels` の形 |

変異注入（実行日 2026-10-21、元にしたコミット 11cc0c8 の作業ツリー。`scripts/check-mutations.sh crates/pack/tests/mutations/PACK17-1.mutation crates/pack/tests/mutations/PACK17-2.mutation crates/pack/tests/mutations/PACK17-3.mutation`）:

| ID | 対象 | 結果 |
|---|---|---|
| PACK17-1 | `guidance.rs`: 本文の探し方が `fallback_chain` に従わない | DETECTED（`body_is_chosen_c_then_b_then_a`） |
| PACK17-2 | `source.rs`: 章の水準の語の診断を捨てる | DETECTED（`start_guidance_rejects_unknown_levels` ほか） |
| PACK17-3 | `guidance.rs`: ヒントの `levels` の要素を検査しない | DETECTED（`hint_levels_rejects_bad_shapes`） |

限界・未実施:

- `Node.body_levels_doc` からの実行時の木の選択は `crates/core/src/body_level.rs`（5.247）、開始点の水準と周回の切り替えは TASK-294 が持つ
- ヒントの `levels` 以外の欄の形は検査しない

### 5.214 案内レベルの 3 つの診断（TASK-49 2/2。PACK-17）

`body_levels` の鍵 `A` を `E-STRUCTURE`（`validate` と `compile`）、隠すと宣言した語が水準 B・C の本文または `do` に出ることを `E-GUIDANCE-LEAK`（`validate` だけ）、ヒントを持つ Step で出ない水準があることを `E-HINT-HIDDEN`（`validate` だけ）にすることを確かめる。検査は `LoadedPack` だけを見る純関数で、本文は読み込み済みの `pack.bodies` から引く。診断に隠す語・本文・ヒントの ID は写さない。

| 試験 | 内容 |
|---|---|
| `crates/pack/tests/guidance_levels.rs` の `body_levels_key_a_is_a_structure_error` | 鍵 `A` は `load` を通り、`validate`・`compile` が `E-STRUCTURE`・`chapters/c1/chapter.yaml`・`step:a` を 1 件返すこと |
| 同 `guidance_leak_*` | 本文（コマンド名・`hide_words`、水準 B・C）、水準 A と規則の無い水準で出ないこと、開始点の `guidance` とモジュールの Step、定義の段の `do`（引数だけの `do` は見ない）、空の語を飛ばすこと、値を写さないこと、`compile` は通すこと |
| 同 `hint_hidden_*` | B・C だけのヒントで水準 A が 1 件、A だけで 2 件、全水準・両方持つ Step・ヒント無し・未定義のヒントだけの Step では出ないこと（後者は `E-REF-HINT`）、ID を写さないこと、`compile` は通すこと |
| `crates/pack/tests/defect_packs.rs` | D21・D22 が `chapters/ch-13/chapter.yaml`・`step:st-13-2` で検出され、語彙 v1.1 の静的な検出が 11/11 になること。`#[ignore]` の試験は無くなった |

変異注入（実行日 2026-10-21、元にしたコミット 7f46482 の作業ツリー。`scripts/check-mutations.sh crates/pack/tests/mutations/PACK17-4.mutation crates/pack/tests/mutations/PACK17-5.mutation crates/pack/tests/mutations/PACK17-6.mutation`）:

| ID | 対象 | 結果 |
|---|---|---|
| PACK17-4 | `validate.rs`: 構造の検査が鍵 `A` を検査しない | DETECTED（`body_levels_key_a_is_a_structure_error`） |
| PACK17-5 | `guidance.rs`: 漏れの検査が定義の段の `do` を見ない | DETECTED（`guidance_leak_in_do_is_reported_and_chapter_placeholder_is_literal`。2026-10-22 に試験名を改めた） |
| PACK17-6 | `guidance.rs`: ヒントの検査が水準 B しか判定しない | DETECTED（`hint_hidden_is_reported_per_level` ほか） |

誤検出の掃除（2026-10-21）: `testdata/module-reuse-rounds/test-data/pack`・`testdata/save-restore-versions/test-data/packs/*`・`testdata/pack-defects/base/format-b-v1_1`・`crates/pack/tests/data/vocab-pack`・`crates/pack/tests/data/fixture-pack` に `validate_dir` を流し、`E-GUIDANCE-LEAK`・`E-HINT-HIDDEN`・鍵 `A` の `E-STRUCTURE` が 0 件であることを確かめた（既存の確認 `answer` の `label` の `E-STRUCTURE` は別件）。

限界:

- 部分文字列の包含による近似で、語の境界は見ない。ヒントの文面・宣言の無い章の本文は調べない（GUIDE-5。TASK-38・TASK-71）。実体の引数で渡す `do` は 5.219（TASK-71 3/3）で置換後まで調べるようになった（2026-10-22。上の「誤検出の掃除」はそれより前の記録）
- `E-GUIDANCE-LEAK`・`E-HINT-HIDDEN` を `compile` の関門にするかは決めていない（今は `validate` だけ）

### 5.215 実体ごとの案内レベルの引数（TASK-70 1/2。PACK-45）

決定 D-55（2026-10-12）は、実体ごとの案内レベル `use.guidance` を語彙に足さないとした（GUIDE-13）。本文への引数の値の差し込み（GUIDE-15）とヒントの文面を実体ごとに変えることには決定が無い。このため #512 の受け入れ条件のうち「語彙を足す案」の側は D-55 が採っておらず、「足さない案では、現状の回避の限界の記録が文書にある」の側で満たす。語彙・診断コードは足さない。

| 試験 | 内容 |
|---|---|
| `crates/pack/tests/per_instance_guidance_gap.rs` の `pack45_use_guidance_is_rejected_by_compile` | `use` の `guidance` が `E-PARSE` ちょうど 1 件になること |
| 同 `pack45_use_guidance_is_rejected_by_validate` | `validate` も同じ `E-PARSE` を 1 件返すこと（修正前は 0 件で通した。`validate` が展開の `E-PARSE` を捨てていた） |
| 同 `pack45_use_guidance_value_is_not_echoed` | 値が診断に写らないこと |
| 同 `pack45_hint_ids_can_differ_per_instance_but_texts_are_not_substituted` | ヒントの ID を引数で渡すと実体ごとに別のヒントが水準別に出ること、定義の文面の `${}` が残ること、IR の実体が案内レベルの欄を持たないこと |

変更: `validate` の `check_module_expansion` が、読めない `use` の `E-PARSE`（`expand::INVALID_USE`）を残す。`compile` を通る教材の結果は変わらない。

変異注入（実行日 2026-10-21、元にしたコミット b68aec4 の作業ツリー。`scripts/check-mutations.sh crates/pack/tests/mutations/PACK45-1.mutation`）: PACK45-1（`validate.rs`: 読めない `use` の `E-PARSE` を捨てる）は DETECTED（`pack45_use_guidance_is_rejected_by_validate`）。

限界・範囲外:

- 置換後のヒント ID の `E-REF-HINT` の検査は無い。未定義の値を渡すとヒントが黙って消える。`E-HINT-HIDDEN` も `${}` のヒントを持つモジュールの Step を見ない（TASK-71 の領域）
- `validate` が `expand_use` のほかの `E-STRUCTURE` を捨てているかは未確認
- 本文の差し込みとヒントの文面の実体ごとの語は、決定が要る
- `do_levels` は #513（TASK-70 2/2）。5.216 に記録した

### 5.216 `do` を案内レベルごとに書き分ける欄 `do_levels`（TASK-70 2/2。PACK-50・GUIDE-14）

決定 D-55（2026-10-12）の項 4 は、`do` を水準別に書く語 `do_levels` を語彙に足すとした（引数化は続けない）。#513 は「語彙を足す案」の側で受け入れ条件を満たす。形は D-55 が決めていなかったため、次のとおり決めた。鍵は `B`・`C` だけで水準 A の文は `do`、`do` は必須、探す順は本文と同じ C → B → A、値は空にできない。語彙の版は v1.6、拡張点の番号なし、`ir_format` は 1 のまま（空なら書き出さない）。この形を spec の PACK-50・GUIDE-14 へ書き戻し、検討中から確定へ移す判断は spec の持ち分である（本 PR では docs/spec を変えない）。

| 試験 | 内容 |
|---|---|
| `crates/pack/tests/do_levels.rs` の `pack50_level_b_wrong_order_next_action_hides_skill_name` | 受け入れ条件。案内レベル B で順序違いの次の行動に skill の名前が出ず、B の文が入ること |
| 同 `pack50_level_a_keeps_do` | 負の対照。A では `do` が入ること |
| 同 `pack50_level_c_falls_back_to_b` | C は B の文に倒れ、C の文があれば C が入ること |
| 同 `pack50_ir_carries_do_levels_only_when_used` | 使う教材の IR にだけ `do_levels` が載ること |
| 同 `pack50_leak_is_clean_with_do_levels`・`pack50_leak_detects_hidden_word_in_do_levels_b`・`pack50_leak_check_reads_the_do_shown_at_that_level` | 漏れの検査がその水準で出る `do` を読み、`do_levels` だけの Step で本文の誤検出を出さないこと |
| 同 `pack50_key_a_is_structure`・`pack50_blank_value_is_structure`・`pack50_requires_do` | `E-STRUCTURE` がちょうど 1 件（`validate`・`compile` の両方） |
| 同 `pack50_unknown_key_is_parse`・`pack50_non_string_value_is_parse` | `E-PARSE` |
| 同 `pack50_vocab_mismatch_below_1_6` | `pack.vocab` が 1.6 未満なら `E-VOCAB-MISMATCH` 1 件、省略なら検査しない |
| 同 `pack50_module_arg_substitution`・`pack50_module_blank_arg_is_caught_after_expansion` | モジュールの引数が置換され、空になった水準の文が展開の後に止まること |
| 同 `pack50_values_are_not_echoed` | 値・鍵が診断に写らないこと |
| `crates/pack/tests/vocabulary_terms.rs` の `do_levels_is_v16_without_number`・`latest_is_v16` | 語彙の文書の v1.6 の行と表の一致 |
| `crates/core/src/text_fill.rs` の `step_do_at_falls_back_c_to_b_to_a`・`crates/contract/src/ir.rs` の `ir6_` | 選択の倒れ方・欄の往復と空のとき書き出さないこと |

変更: contract の `Step` に任意の欄 `do_levels`、core の `text_fill::step_do_at`（`slot_values` の `step_do` と漏れの検査が共用）、pack の `guidance::check_do_levels`（`validate`・`compile`）、`vocab` の v1.6、core の `VOCAB_MAX` を 1.6。使わない教材の IR はバイト単位で変わらない（golden は書き換えない）。

変異注入（実行日 2026-10-22、元にしたコミット 886d34f の作業ツリー。`scripts/check-mutations.sh crates/pack/tests/mutations/PACK50-<n>.mutation`）: PACK50-1（`slot_values` の `step_do` を `do` に戻す）・PACK50-2（`step_do_at` が自分の水準だけを見る）・PACK50-3（鍵 `A` の判定を外す）・PACK50-4（漏れの検査が `do` そのものを見る）はいずれも DETECTED。

限界・範囲外:

- 復元の後の `message` の再充填は水準 A になる（`docs/known-gaps.md` の PACK-50。TASK-307・TASK-308）
- 開始点の水準の結線は未実装（TASK-294）。試験は `RunState` を直接組む。画面の確認は対象外
- `${}` の引数で渡す `do`・`do_levels` は漏れの検査が見ない（PACK-52。TASK-71）
- `scripts/expected-test-counts.txt` は版上げ PR の関門の期待値で、本変更では更新しない

### 5.217 モジュールの先頭の Step を省く語が無いこと（TASK-71 1/3。PACK-51・D-43）

決定 D-43（2026-10-12）の項 7 は、モジュールの先頭の Step を省ける語を足さず、作成者の規約として受け入れるとした（PACK-51・MOD-17 (a)）。#515 の受け入れ条件は、「省けないことを診断にしてよい」の側で満たす。語彙・診断コード・コードは変えない。

| 試験 | 内容 |
|---|---|
| `crates/pack/tests/module_first_step_gap.rs` の `pack51_use_enters_the_first_step_even_when_args_route_around_steps` | testdata の 1 周目・2 周目（`setup: "false"`）とも `enter` が先頭の `st-3-2` であること。2 周目で通らない `st-4-1`・`st-5-1` も IR に残り、先頭の Step の `next` が引数の条件で分かれること |
| 同 `pack51_enter_is_the_first_step_for_a_small_module` | 小さなモジュールで、先頭の Step が後ろを飛ばす引数（`skip`）の真偽によらず `enter` が先頭であること |
| 同 `pack51_skip_like_use_field_is_rejected_by_compile` | `use` の `start`・`skip_first`・`from` が `E-PARSE` ちょうど 1 件になること |
| 同 `pack51_skip_like_use_field_is_rejected_by_validate` | `validate` も同じ `E-PARSE` を 1 件返すこと |
| 同 `pack51_step_level_skip_field_is_rejected` | Step に `skip` を書くと `E-PARSE` が 1 件になること |
| 同 `pack51_skip_like_values_are_not_echoed` | 値が診断に写らないこと |

変更: 製品のコードの変更は無い。

変異注入（実行日 2026-10-22、元にしたコミット 6d8627b の作業ツリー。`scripts/check-mutations.sh crates/pack/tests/mutations/PACK51-1.mutation`）: PACK51-1（`expand.rs`: `use` の入り口を 2 つ目の Step にする）は DETECTED（`pack51_enter_is_the_first_step_for_a_small_module`、`pack51_use_enters_the_first_step_even_when_args_route_around_steps`）。

限界・範囲外:

- 引数の型と範囲外の値の拒否は 5.218（#516。TASK-71 2/3）、`E-GUIDANCE-LEAK` の範囲は 5.219（#517。TASK-71 3/3）
- 飛ばした Step が実行時に未着手のまま残り、途中開始の「省略」と区別されること（MOD-17 (b) の実行時の側）は対象外
- 規約を spec の `04-behavior/pack-authoring.md` へ書き戻すことは spec の持ち分である（本 PR では docs/spec を変えない）
- `scripts/expected-test-counts.txt` は版上げ PR の関門の期待値で、本変更では更新しない

### 5.218 モジュールの引数の型と範囲外の値の拒否（TASK-71 2/3。PACK-51・MOD-17 (c)・D-43）

決定 D-43（2026-10-12）の項 7 は、引数の型と範囲外の値を `validate` が検査で拒否するとした。型と範囲を宣言する欄の形は仕様に無いため、`params` の隣の任意の欄 `param_types`（語彙 v1.7、拡張点の番号なし）として本変更で決めた。`params` の読み方（PACK-4）は変えない。

| 試験 | 内容 |
|---|---|
| `crates/pack/tests/module_arg_types.rs` の `pack51_bool_arg_out_of_range_is_rejected_by_compile` | `bool` に `"flase"` を渡すと `compile` が `E-STRUCTURE` ちょうど 1 件で止まること |
| 同 `pack51_bool_arg_out_of_range_is_rejected_by_validate` | `validate` も同じ `E-STRUCTURE` を 1 件返すこと（展開の診断の許可リストの回帰） |
| 同 `pack51_int_arg_outside_min_max_is_rejected` | `min`-1・`max`+1 は拒否、両端は通ること |
| 同 `pack51_int_arg_non_canonical_is_rejected` | `"01"`・`"+1"`・`"1.0"`・`"abc"`・空・`"-0"`・`i64` の溢れは拒否、`0`・負数・`i64` の端は通ること |
| 同 `pack51_enum_arg_not_in_values_is_rejected` | `values` に無い値・大文字小文字の違い・空は拒否、一致は通ること |
| 同 `pack51_in_range_args_compile_and_ir_matches_untyped` | 範囲内の値は `compile`・`validate` とも 0 件で、`param_types` を消した教材と IR がバイト単位で一致すること（負の対照） |
| 同 `pack51_untyped_params_are_not_checked` | 宣言しない引数は従来どおり任意の文字列で通ること |
| 同 `pack51_default_out_of_range_is_rejected_once` | 既定値が範囲外なら、呼び出し 0 回・2 回とも宣言の側の 1 件になること |
| 同 `pack51_declaration_errors_are_structure` | 鍵が `params` に無い・`enum` の `values` が無い／空／重複・型に合わない欄・`min` が `max` を超える、がそれぞれ `E-STRUCTURE` 1 件になること |
| 同 `pack51_declaration_unknown_type_or_field_is_parse` | 未知の型・欄・整数でない `min` が `E-PARSE` になること |
| 同 `pack51_nested_arg_is_checked_after_substitution` | 入れ子の `${名前}` を置換した後の値で判定されること |
| 同 `pack51_format_a_toml_param_types` | 形式 A（TOML）でも同じ判定になること |
| 同 `pack51_vocab_mismatch_below_1_7` | `pack.vocab` が 1.7 未満なら `E-VOCAB-MISMATCH` が 1 件、1.7 以上と省略では 0 件であること |
| 同 `pack51_arg_values_are_not_echoed` | 引数・既定値・`values` の値が診断に写らないこと |
| `crates/pack/tests/vocabulary_terms.rs` の `param_types_terms_are_v17_without_number` | 語彙 v1.7 の 8 語が 7 節にあり、拡張点の番号を持たないこと |
| `crates/pack/src/param_check.rs` の単体試験 | `value_fits` の境界と、診断の文面に値が入らないこと |

変更: `ScopeSrc` に `param_types` を足し、宣言の検査（`compile`・`validate`）と呼び出しの引数の検査（`expand_use`。`validate` の許可リストにも追加）を `param_check.rs` に置いた。語彙 v1.7（文書の版 7）、`VOCAB_MAX` 1.7、診断の一覧の版 12 を同期した。IR と `ir_format`（1）は変えない。

変異注入（実行日 2026-10-22、元にしたコミット 1b178a7 の作業ツリー。`scripts/check-mutations.sh crates/pack/tests/mutations/PACK51-2.mutation` から `PACK51-5.mutation`）: PACK51-2（`expand.rs`: 呼び出しの引数の範囲外の判定を外す）、PACK51-3（`validate.rs`: 許可リストから範囲外の引数を外す）、PACK51-4（`param_check.rs`: `int` の上限を含まない比較にする）、PACK51-5（`param_check.rs`: 既定値の検査を外す）は、いずれも DETECTED。

限界・範囲外:

- `condition.param` の `eq` の値が宣言した型・範囲に入るかの検査は対象外（`docs/known-gaps.md` の PACK-51）
- 決めた `param_types` の形を spec の PACK-51・MOD-17 と `04-behavior/pack-authoring.md` へ書き戻すことは spec の持ち分である（本変更では docs/spec を変えない）
- `E-GUIDANCE-LEAK` の範囲は 5.219（#517。TASK-71 3/3）、MOD-17 (b) の実行時の側はエンジンの担当
- `scripts/expected-test-counts.txt` は版上げ PR の関門の期待値で、本変更では更新しない

### 5.219 隠す語の検査の範囲（TASK-71 3/3。PACK-52・GUIDE-5・D-43）

決定 D-43（2026-10-12）と 04 の PACK-52・GUIDE-5 に従い、`E-GUIDANCE-LEAK` の範囲に足すのは (a) モジュールの実体の引数（`${名前}`）で渡す `do`・`do_levels` の置換後の文字列だけである。(b) ヒントの段 2・3 と (c) 開始点の `guidance` も `body_levels` も持たない章の本文は、検査の外に置き、作成者の規約として受け入れる（ヒントの段ごとに隠す語を宣言する欄も足さない）。#517 の受け入れ条件の文面は (a)(b)(c) の検出と読めるが、これは TASK-71 の完了条件にある、決定前の「範囲を広げる案では」の条件つきの文の写しである。完了条件の前半「04 の PACK-52 の文が自動テストで確かめられ、通る」を正とし、(a) の検出と、(b)(c) を報告しないことの観察を固定した。「誤検出 0 件」は (c) 型の本文で確かめる。#515（5.217）・#516（5.218）と同じく、決定で採った側で受け入れ条件を満たす。語彙・診断コード・IR・`ir_format` は変えず、`compile` の結果も変わらない。

検出は展開器（`expand.rs`）に `validate` 専用の任意の検査として足した。既定値の重ね合わせ・入れ子の引数・予算・巡回の停止を二重に持たないためである。置換前に `${` を含む `do`・`do_levels` の Step について、調べる水準（章の開始点の B・C ∪ `body_levels` か `do_levels` を持つ Step の B・C）ごとに、置換の前に語が無く置換の後に語が出る場合だけ 1 件にする。置換の前の字面にすでに語がある水準は、定義の側の検査が報告するので重ねない。同じ内容の診断は 1 件にまとめる。診断は固定の英語で、語・引数の値・本文を写さない。

| 試験 | 内容 |
|---|---|
| `crates/pack/tests/guidance_leak_scope.rs` の `pack52_a_argument_do_leak_is_reported` | 引数で渡した skill 名が水準 B で 1 件になり、モジュールの定義ファイルと Step で報告され、`compile` は通ること |
| 同 `pack52_a_default_param_leak_is_reported` | 引数を渡さず既定値が漏れる場合も 1 件になること |
| 同 `pack52_a_do_levels_argument_is_checked_at_that_level` | `do_levels` は、その水準で出る文だけが読まれること |
| 同 `pack52_a_nested_argument_is_substituted` | 入れ子の `use.args` を経由した値の漏れを検出すること |
| 同 `pack52_a_clean_argument_and_level_a_have_no_leak` | 安全な値・開始点が A か無しの章・規則の無い水準では 0 件であること（負の対照） |
| 同 `pack52_a_not_double_reported` | 定義の段の字面にすでに語がある `do` と、同じモジュールの 2 実体は 1 件であること |
| 同 `pack52_a_parameterized_module_name_is_not_suppressed` | 引数つきのモジュール名で呼ばれる Step は定義の段が辿らないため、置換の前の字面に語があっても置換後の診断が 1 件出ること（抑制は定義の段の対象だった Step・水準に限る） |
| 同 `pack52_b_hint_tiers_are_not_checked` | B・C に出るヒントの文面に skill 名があっても 0 件であること（D-43 の観察） |
| 同 `pack52_c_undeclared_chapter_body_has_no_false_positive` | 宣言の無い章の本文・`do` は 0 件で、開始点を `guidance: B` にすると検出されること（誤検出 0 件と、対象が宣言に結びつくことの対照） |
| 同 `pack52_values_are_not_echoed` | 引数・既定値・隠す語が診断に写らないこと |
| 同 `pack52_rounds_testdata_reports_known_true_leaks` | `module-reuse-rounds` の `validate` が、5 つの Step（`idea-cycle` の `st-2-1`、`issue-to-merge` の `st-4-1`・`st-5-1`・`st-6-1`・`st-7-1`）を水準 B・C の 2 件ずつ報告すること |

testdata の実測（2026-10-22）: 実装の前は `module-reuse-rounds` の `E-GUIDANCE-LEAK` が 0 件、実装の後は 10 件（上の 5 Step × B・C）になった。原因は R-5 が記録した真の漏れで、`ch-round1` が引数を渡さず、既定値の skill 名（`/new-idea`・`/init-claude` など）が、`body_levels` を宣言したモジュールの Step の水準 B・C に出る。`ch-round2` は skill 名を含まない値を渡しており、元ではない。計画で見込んだ 2 件（`st-6-1`・`st-7-1`）より多かったが、`ch-round1` も `body_levels` を持つ Step を水準 B・C で検査する定義（既存の対象の決め方 (i)）のためで、誤検出ではない。写しは編集できないため、「`validate` が 0 件」を見る `validate_refs.rs` の `correct_testdata_packs_have_no_false_positives` は、この 1 種の診断だけを `common::without_expanded_do_leak` で除き、除外が空振りしていないことを確かめる。

変異注入（実行日 2026-10-22、元にしたコミット 41f2e44 の作業ツリー。`scripts/check-mutations.sh crates/pack/tests/mutations/PACK52-1.mutation` から `PACK52-3.mutation` と `PACK17-5.mutation`）: PACK52-1（`expand.rs`: 置換後の漏れの検査を行わない）、PACK52-2（`validate.rs`: 許可リストから `GuidanceLeak` を外す）、PACK52-3（`guidance.rs`: 置換後の判定が水準ごとの `do` を見ない）、PACK17-5（試験名を改めた後の再実行）が、いずれも DETECTED。

限界・範囲外:

- 部分文字列の包含による近似で、語の境界は見ない。モジュールの本文の Markdown は引数で置換されない（PACK-45・GUIDE-15）ため、(a) の対象は `do`・`do_levels` だけである。展開が `E-LIMIT` で打ち切られた後ろは調べない（`E-LIMIT` が検査の未完了を示す）
- (b)(c) の規約と、PACK-52・GUIDE-5 の「現状の観察」の文（引数の `do` も報告されない）の更新、`04-behavior/pack-authoring.md` への書き戻しは spec の持ち分である（本変更では docs/spec を変えない）。#517 の受け入れ条件の文面の差し戻しも spec 側の持ち分である
- `module-reuse-rounds` の写し（と同形の写し）の真の漏れ（`ch-round1` の既定値）は、教材の側（spec repo の PoC データ）で直す
- `E-GUIDANCE-LEAK`・`E-HINT-HIDDEN` を `compile` の関門にするかは未決のまま
- `scripts/expected-test-counts.txt` は版上げ PR の関門の期待値で、本変更では更新しない

### 5.220 表示だけの履歴の動作（TASK-176 1/2。VER-8）

版の違う履歴の判定（TASK-174）で「表示だけ」になった履歴について、web の動作を入れた。表示だけの履歴を読み込んだとき（`open_history`）と、起動時の復元（`boot` の `NotOpened`）で、ホストが理由のコード（`LoadReason`）と読み込んだバイト列の控えを持つ。実行環境は作らない（`install_recorded` は呼ばず、VER-2・TASK-175 の「表示だけで `Session` を作った例 0 件」を保つ）。画面に出す状態は、起動時は初期状態、読み込み時は直前の状態のままである。

- 理由は通知欄 `view-only-notice`（`role=status`）に出す。文は固定の文言と理由のコードだけで、履歴の値を含めない（SAFE-40）。枠の通知（`shell` の `NoticeKind`、契約は版 1）は spec が先に変わらないと足せないため、`stale-notice` と同じ任意の欄にした。文面は仮置きで、最終の文面は TASK-549、配置は TASK-534 が決める
- 操作の関門は `update` の先頭にある。表示だけの間、`Engine`（指せる行・選択肢・承認のクリックと、保存・停止・進行の tick を含む raw の `Op`）、送信・Esc・停止・下書きの変更、補完を捨てる。下書きの変更を捨てたときは `<textarea>` の値を戻させる。表示だけを変える action と、モードから出る経路（試験の設定の読み込み・差し替え）は通す
- 保存は `history_to_save` が `None` を返して止める。`save_blocked` は消えない印で、読み込みで入ったモードが開ける履歴の読み込みで終わった後も保存が止まるため使わない。進行の宛先（`next_tick`）も `None` にし、予約済みのタイマーは無効にする
- 書き出し（`export_live`）は、読み込んだバイト列そのものを返す。作り直さない（SAVE-25 の実装側。正式な試験は TASK-335。5.231）。秘密情報の検査は通常の書き出しと同じく通す
- 開ける履歴の読み込み（`install_recorded`）で、モードと控えを捨てる。開かない（`Refused`）の動作と理由の表示は #733 の範囲で、従来の挙動を保つ

| 試験 | 内容 |
|---|---|
| `crates/web/tests/view_only.rs`（7 件） | 読み込みで理由が出て、7 種の操作（下書き・送信・Esc・停止ボタン・補完・raw の保存・raw の停止）の前後で状態・ログ・下書きが同じで保存の値と進行の宛先が出ないこと。対照として、開ける履歴では同じ操作が状態を変えること。書き出しが読み込んだバイト列と一致すること。起動時の復元でも同じこと。開かない履歴は表示だけにならないこと（#733 で変わる前提の回帰）。開ける履歴の読み込みでモードから出ること。通知の文に履歴の版が入らないこと |
| `tools/browser/view-only.test.mjs`（CSP あり・なしで 4 件） | 読み込み（V-iii-1）と起動時の復元で、`stateJson().engine.viewOnly` が理由のコードになり、通知欄にコードを含む文が出ること。打鍵・Enter・送信ボタン・Esc・raw の保存・raw の停止の後で状態・ログ・世代が変わらず入力欄が空のままであること。復元では保存先の値が書き換わらないこと。外部通信・CSP 違反・ページのエラーが 0 件 |
| `tools/browser/version-histories.test.mjs`・`history-versions.test.mjs` | 状態の比較から `viewOnly`・`exportGate`・`exportBytes` を外し、`viewOnly` が表示だけの行でだけ理由と一致することを明示で確かめるようにした |

実行結果（2026-10-22）: `cargo test --locked -p fandhe-workshop-web`（既定と `--features test-hooks`）は全件 pass。ブラウザ試験は `view-only`・`version-histories`・`history-versions`・`version-notice` の 4 ファイル 25 件が pass（配信設定と試験の設定を `SKIP_WASM_OPT=1` でビルドし、版の違う履歴 15 件を書き出して単独に実行した。全件の `make test-browser` は CI の `browser-test` に任せる）。

【推論】表示だけの間に履歴の写し（`snapshot`）を今の IR で描くことはしない。版の違う IR では Step が存在しない場合があり、誤って描く危険があるため。履歴の中身の表示は REVIEW-4/5（TASK-505）の範囲で、`docs/known-gaps.md` に記録した。`scripts/expected-test-counts.txt` は版上げ PR の関門の期待値で、本変更では更新しない。

### 5.221 開かない履歴の動作（TASK-176 2/2。VER-8・SHELL-16）

版の違う履歴の判定（TASK-174）で「開かない」になった履歴について、理由を画面に出すようにした。保存領域・`Session` の状態・ログ・入力欄の下書きは、5.220 と同じく変えない（読み込みは `install_recorded` を呼ばず、起動時は `save_blocked` が上書きを止める）。

- 理由は枠の通知欄（`shell-notice`）に `NoticeKind::ImportRefused`（契約の版 1。`reason_rule` は必須）で出す。項目は `data-notice-kind=import-refused` と `data-notice-reason`（`LoadReason` のコード）を持ち、文は固定の文言と理由のコードだけで作る（履歴由来の値を含めない。SAFE-40）。文面は仮置きで、最終の文面は TASK-549・TASK-534、次の行動（SHELL-36）は TASK-538 が決める
- 読み込み（`open_history`）は `Refused` をすべて通知する。理由の種類では絞らない。閉じる操作（`dismiss-notice`）で消え、状態は変わらない。次に開ける履歴・表示だけの履歴を読み込むと消える。表示だけの間に開かない履歴を読み込んでも、表示だけのまま・控えのバイト列も変えない
- 起動時の復元は、版の判定の段（`LoadReason::stage() == VersionJudgment`。`format_newer`・`vocab_newer`・`engine_newer`・`bad_version`）の拒否だけに通知を出す。それ以外の段（壊れた履歴）の案内は TASK-533（SHELL-17）の範囲で、従来どおり通知を出さない。【推論】`history_format_newer` は版に関わるが版の軸を持たないため、TASK-533 側に残した
- 起動時の通知には閉じる操作を置かない。起動時に開けなかった場合は `save_blocked` がセッションの終わりまで立ったままで、通知が「保存していない」ことの唯一の手がかりになるため。生の `dismiss-notice` が届いても閉じない（fail-closed）

| 試験 | 内容 |
|---|---|
| `crates/web/src/shell_notice.rs`（単体 6 件） | 項目の数・属性・閉じるボタンの有無・2 通知の並び・全 `LoadReason` で項目が作れること |
| `crates/web/tests/refused_history.rs`（11 件） | 読み込みで状態・ログ・下書き・導出した値・保存の値が不変で積む field が通知欄だけ。読み込みの後も操作が効く。閉じる操作。続けて開く履歴での消去。表示だけの間の読み込み。変換して開いた通知との併存。起動時の閉じられない通知と生の閉じる操作の無視。壊れた履歴で通知が出ない。履歴由来の値が出ない。読み込み先の無いホスト |
| `tools/browser/refused-history.test.mjs`（CSP あり・なしで 8 件） | 表の refused の 3 行（V-iv-2・V-iv-3・V-v-2）で、結果・理由が表と合い、実行環境の差し替え 0 回・状態・入力欄・保存先の値が不変で、通知が 1 件出て、操作が効き、閉じるボタンで消える。起動時の復元（`ir_format` を上げた履歴）で閉じるボタンの無い通知が出て、操作の後も保存先の値が書いた値のまま。外部通信・CSP 違反・ページのエラーが 0 件 |
| 既存の試験 | `history_import.rs` の不変の確認を「積む field は `shell_notices` だけ」に、`version_notice.rs` に併存の確認を足した。`view_only.rs` の旧い回帰は壊れた履歴の試験に改めた |

実行結果（2026-10-22）: `make lint`・`cargo test --locked -p fandhe-workshop-web --features test-hooks` は全件 pass。ブラウザ試験は `refused-history`・`view-only`・`version-notice`・`version-histories`・`history-versions`・`history-import` の 6 ファイル 36 件が pass（配信設定と試験の設定を `SKIP_WASM_OPT=1` でビルドし、版の違う履歴 15 件を書き出して単独に実行した）。全件の `make test-browser` は CI の `browser-test` に任せる（ローカルでは未実行）。`scripts/expected-test-counts.txt` は版上げ PR の関門の期待値で、本変更では更新しない。

### 5.222 「直す」の対象の候補と再承認の判定（TASK-265。JUDGE-11・D-69）

決定（D-69、2026-10-21）は、再承認を「承認（差し戻し）の Step」と「再承認の Step」に分けて書くこと（PACK-32）と、`approval.revise.candidates` で誤った対象の候補と表示名を宣言すること（PACK-42）である。語彙と検査は TASK-62 で入っており、本変更で core の判定を結んだ。

- 候補を宣言した Step の対象つきの「直す」は、対象が候補か規則の対象にあるときだけ宣言にあるものとする。無ければ `not_in_this_step` の分類による拒否で、対象なしの拒否の規則は当てない
- 対象が候補にあり規則の対象に無い（誤った候補）ときは、対象つきの規則に一致が無ければ、対象なしの「直す」の規則のうち拒否だけを上から採る（2 段目）。受付・問い返しは飛ばす。`when` は `Some(true)` のときだけ採る。拒否の規則が無ければ分類による拒否
- 候補を宣言した Step の対象なしの「直す」は、対象つきの規則が無くても拒否の規則だけを採る。候補を宣言しない Step は従来のまま
- `select_judge::revise_candidates` が、宣言の順の候補と表示名を返す。宣言が無ければ規則の対象を表示名なしで返す。`input` が無い・方式が `approval` でなければ空で、宣言が壊れていれば `InvalidDeclaration`（受付へ倒さない）
- 判断（推論）: D-69 と JUDGE-11 の文面は、誤った候補に対象なしの規則を当てるかを書かない。TASK-264 が試作の「対象なしの規則を任意の対象に当てる」照合を採らずに本タスクへ送ったこと、testdata の ST-2.6 の `rj-2-6-wrong-target` が誤った対象向けの文面であることから、拒否の規則に限って採った。拒否にしか倒れないため受付の安全性は変わらない

| 試験 | 内容 |
|---|---|
| `crates/core/tests/revise_candidates.rs`（16 件） | 候補が宣言の順で表示名つきで返ること。宣言が無ければ規則の対象が表示名なしで返ること。他の方式・入力なしで空。壊れた宣言（未知の欄・空の一覧・ID の重複・空白だけの `label`）が `InvalidDeclaration` で、どの入力も受付にならず、`Display` に値が入らないこと。正しい対象の受付。誤った候補が対象なしの拒否の規則で拒否され分類と次の行動が付くこと。拒否の規則が無い・`on_reject` の上書き・`when` が偽か評価不能の場合。矛盾した教材（受付・問い返しの規則が前にある）で受付にも問い返しにもならないこと。宣言に無い対象が分類による拒否になること。候補を宣言しない形が従来のままであること。コンパイル済みの形（`null` 込みの `on`）。差し戻し → 再承認を `Session` で通すこと（早すぎる承認・誤った候補は拒否で Step が変わらず、正しい対象で再承認の Step へ進み、承認で達成）。性質テスト（判定が決定的で、対象つきの「直す」の受付は完全一致する受付の規則があるときに限り、拒否に次の行動が付く） |
| 既存の `revise_target.rs`・`select_judge.rs`・`judge.rs`・pack の `approval_revise_candidates.rs`・`approval_revise_split.rs` | 変えずに通る |

変異注入（実行日 2026-10-22、元にしたコミット 59e64d1 の作業ツリー。`scripts/check-mutations.sh crates/core/tests/mutations/JUDGE11-1.mutation` から `JUDGE11-4.mutation` と `JUDGE10-1.mutation`・`JUDGE10-3.mutation`）:

| ID | 内容 | 結果 |
|---|---|---|
| JUDGE11-1 | 2 段目で拒否以外の規則を飛ばさない | DETECTED |
| JUDGE11-2 | 誤った候補への 2 段目を行わない | DETECTED |
| JUDGE11-3 | 宣言に無い対象を宣言にあるとみなす | DETECTED |
| JUDGE11-4 | 候補の宣言を無視して規則の対象へ倒す | DETECTED |
| JUDGE10-1・JUDGE10-3 | 既存の変異（`find` を新しい構造に合わせて JUDGE10-1 を直した） | DETECTED |

範囲外: 承認のボタンを `revise_candidates` から出し表示名で描くこと、`label` のエスケープ、差し戻し → 再承認の画面とブラウザ試験は #666（TASK-448）。`next` の `when` による枝分かれの実行時の評価は既存の別タスク。spec の JUDGE-11・PACK-42 の状態の書き戻しは spec repo の持ち分。`scripts/expected-test-counts.txt` は版上げ PR の関門の期待値で、本変更では更新しない。

### 5.223 `?ir=` を読まないことと開始点の許可（TASK-168。SAFE-33・D-68）

許可の方式は D-68 で決まった「IR は固定の `./ir/main.json`、`start` は IR の `starts` の ID と完全一致」である。URL の `ir` は読まず、読み込み先を作る入力にしない。Issue の「一覧にある値は開く・ない値は読み込まない」は次のように対応させた。

| 受け入れ条件 | D-68 での意味 | 確かめ方 |
|---|---|---|
| 一覧にある値は開く | `start` が `starts` の ID と完全一致なら開く（TASK-109 で実装済み） | `ir-location` (T)。`?ir=` を付けても教材と現在の Step がクエリなしと同じ |
| 一覧にない値（`../` を含む値）は読み込まれず理由が示される | `start` は固定のコード `start_unknown` で断る。`ir` は読み込み先を変えない | `ir-location` (T)(D)。理由用の `ir` の通知は作らない（SAFE-33 が求めていない） |
| 外部への通信が 0 件 | 配信元以外への要求と、配信元の別の JSON の取得が 0 件 | `ir-location` (D)。囮の JSON 3 件を置き、取得されないことを確かめる |

- native: `crates/web/tests/tick_interval.rs` に 2 件を足した（計 6 件）。`app.js` が `ir` のキーを読まないことと、本番の Rust（`test_hooks.rs`・`test_fixture.rs` を除く）が `./ir/` を組み立てないことを 0 件で固定する。字句が空振りしないことの確認つき
- ブラウザ: `tools/browser/ir-location.test.mjs`（3 件）。(D) 配信する設定で `?ir=` の 7 形（`../something`・`..%2Fsomething`・別名・`main`・絶対 URL・`//host`・`start` 併用）を開き、取得する資源の種類がクエリなしと同じで囮を取得せず、通信・CSP 違反・エラーが 0 件。(T) 試験の設定の CSP あり・なしで、許可される 4 形は教材が変わらず通知が出ない。断られる 3 形（`start` が不一致）は固定の文言とコードだけで、値は DOM・コンソールに出ず、保存先は再読み込みを挟んでも変わらない
- 実行結果（2026-10-22）: `cargo test -p fandhe-workshop-web --test tick_interval` は 6 件 pass。`ir-location` は 3 件 pass（配信設定と試験の設定を `SKIP_WASM_OPT=1` でビルドして単独に実行）。全件の `make test-browser` は CI の `browser-test` に任せる（ローカルでは未実行）
- 変異: `app.js` に `getAll("ir")` を足すと native の検査が失敗する（字句の空振り防止の試験で検出力を確認。実ファイルへの変異は未実施）。ブラウザ試験の囮の取得に対する変異は未実施。`scripts/check-mutations.sh` には足していない
- 件数の期待値 `scripts/expected-test-counts.txt` は版上げ PR の関門の値で、本変更では更新しない
- 限界: 配信する設定は実行時に IR を取得しないため、「一覧にある値は開く」は試験の設定でだけ確かめる。`?ir=` の値は無視され、理由は出ない。`?fixture=` と `?ir=` は併用しない（試験の設定の教材の選び方の制約）
- 範囲外: 配信する設定で `./ir/main.json` を実行時に取得して開始点で開く処理、読めない IR の鍵の本番の経路への結線、旧方式の `?ir=` のリンクへの案内

### 5.224 章の完了後の「次の章へ進む」を 1 つだけ出す（TASK-526 1/2。SHELL-10・INTRO-3）

章の最後の Step（`main` の根で `final`）が達成になった後に、次の章の最初の Step を指す開始点を `次の章へ進む: <Step の題>` として 1 つだけ出す。最後の章の後は出さない。出す条件は状態から導く（`crates/web/src/next_chapter.rs`）ので、再読み込み・履歴の復元の後でも同じ表示になる。開始点が IR に無い章へは出さない（D-67・SHELL-12 の fail-closed）。次の章の先頭の Step を指す開始点が無く、途中の Step を指す開始点だけがあるときも、章の冒頭を飛ばす入口になるので出さない。

- native: `crates/web/tests/next_chapter.rs`（7 件）。試験用の教材 `ch0_3` は章の境目を無条件の `next` でつなぐため、試験の中で各章の最後を `final` にした変種を作る。`#` つきの開始点と、先頭より前に置いた先頭でない開始点を混ぜ、選び方を確かめる
  - 導出: CH-0・CH-1・CH-2 の後に出て（入口は先頭の節点に一致する開始点で、`#` つきは選ばない）、CH-3 の後は出ない。達成の前・最後でない Step では出さない。次の章の開始点が IR に無ければ出さない。先頭の Step を指す開始点が無く途中の Step を指す開始点だけがあるときも出さない
  - host: CH-0 の手を画面から送れる手だけで進め、最後の手より前は 0 個・最後の達成の後はちょうど 1 個、`next_chapter` が dirty に積まれ、枠の適合と a11y の検査が通る。「移動」ボタンは 0 件
  - 既定の feature（`test-hooks` なし）と `--features test-hooks` の両方で通す。`raw-trigger` が無いことの確認は既定の feature だけで行う
- 変異: `SHELL10-2`（達成の条件を外す）・`SHELL10-3`（同じ章を指す）を `crates/web/tests/mutations/` に足した。手で当てて、それぞれ試験が失敗することを確かめた（2026-10-22）。`make check-mutations` の全件は未実行
- 既存の固定の更新: `no_hidden_triggers.rs` の `PAYLOAD_ALLOWED` に `next_chapter.rs`、`dirty_binding_points.rs` の除外と `history_import.rs` の期待に `next_chapter` を足した
- 実行結果（2026-10-22）: `cargo test --workspace` と `make lint` は通った。`make test-browser` は未実行（CI の `browser-test` に任せる。ブラウザ試験は 5.227 で追加）
- 限界（2/2 で結線し、5.227 に移した）: 1/2 では action を結線しないので、押しても状態・ログ・保存先は変わらない（`next-chapter` は `decode_action` に無く、未知の action として捨てられる）。押下の試験は状態が変わらないことだけを固定し、host に履歴の `ops` の読み口が無いため `ops` は照合していない。置き場（右のエリアの末尾）は仮置きである
- 範囲外: （押下の結線・ブラウザ用の fixture・自動ブラウザ試験は 5.227 で済んだ）試験の設定への「移動」ボタンの追加（#910・#1127）、範囲外の章を出さないことの画面での確認（5.228 で済んだ。TASK-527）、最終の配置（TASK-534・TASK-548）

### 5.225 Step ごとの上流の行（対応表の欄）を語彙に持たない代用の記録（TASK-72。PACK-46・D-70）

決定 D-70（2026-10-22）は、対応表の欄を語彙に持たないとした。対応表は読み込まれない教材 repo の別の文書に置き、教材テストで Step ID と突き合わせる。ヒントの 3 段目の括弧書きは、コマンドの外（ヒントの別の文）に置く教材側の約束で、語は足さない。#518 の受け入れ条件は「持たない案」の側（代用の記録が文書にあること）で満たす。語彙・診断コード・IR・CLI は変えない。

| 試験 | 内容 |
|---|---|
| `crates/pack/tests/upstream_map_gap.rs` の `pack46_step_upstream_field_is_rejected_by_compile` | Step に `upstream`・`upstream_ref`・`source` を書くと `compile` が `E-PARSE` ちょうど 1 件で止まること |
| 同 `pack46_step_upstream_field_is_rejected_by_validate` | `validate` も同じ `E-PARSE` を 1 件返すこと |
| 同 `pack46_upstream_field_value_is_not_echoed` | 値が診断に写らないこと |
| 同 `pack46_pack_refs_pass_and_are_copied_to_ir` | パック全体の `refs` が診断 0 件で通り IR に写ること。存在しない Step ID らしい値を足しても検査されないこと |
| 同 `pack46_vocab_table_yields_step_ids_for_external_map` | `vocab` の表から全 Step の `scope,step` が取れ、展開後の複製を含まないこと。別の文書の行を模した表との突き合わせで、表に無い Step ID の検出と、参照名ごとの影響する Step の決定的な列挙が組めること（教材テストの形の例示で、エンジンの機能ではない） |
| 同 `pack46_hint_bracket_outside_command_is_plain_text` | 括弧書きをコマンドの外に置いたヒントも、続けた形（約束の違反）も、診断 0 件で通り文面が IR に残ること（検査が無いことの記録） |

変更: 製品のコードの変更は無い。

変異注入: 置かない。製品のコードの変更が無く、Step の未知の欄の拒否（`deny_unknown_fields`）を 1 行で一意に緩める置換が無いためである。

限界・範囲外:

- 教材 repo の対応表の文書、Step ID の対応を検査する教材テスト、影響する Step を列挙するスクリプトは教材 repo の担当（TASK-720・TASK-730）
- 参照版の置き場の最終の形（TASK-717）。`refs` の形の検査は足さない
- spec への書き戻しと、`docs/spec` の submodule の更新（D-70 の取り込み）は本変更では行わない
- `scripts/expected-test-counts.txt` は版上げ PR の関門の期待値で、本変更では更新しない

### 5.226 入力も確認も持たない途中の Step の拒否（#1709。PACK-19・D-69）

`expand::check_use_steps` に `is_bare_step` の分岐を足し、入力も `use` も確認も無い終端でない Step を `validate`・`compile` の両方が `E-STRUCTURE` で拒否する。拒否の前に #1708 で試験データを直した。

| 試験（`crates/pack/tests/pack19_bare_step.rs`） | 確かめること |
|---|---|
| `pack19_bare_intermediate_step_is_structure_error` | 章の裸の Step が `validate`・`compile` で `step:a` の `E-STRUCTURE` 1 件 |
| `pack19_bare_step_in_module_is_reported_once` | モジュールの裸の Step は、実体が 2 つでも定義ごとに 1 件で、引数の値を写さない |
| `pack19_cli_compile_rejects_bare_step` | `compile` の CLI が終了コード 1・標準出力に `E-STRUCTURE`・標準エラー出力が空 |
| `pack19_controls_are_not_rejected` | 終端だけの章と、`input` を持つ途中の Step は拒否されない |
| `pack19_confirm_only_step_keeps_one_diagnostic` | 確認だけの Step は既存の文面の 1 件のまま |

- 範囲外: 画面の側の挙動、`fandhe-pack validate` の CLI（TASK-403）
- 変異 PACK19-1（`is_bare_step` に `&& false`）: 2026-10-22、`scripts/check-mutations.sh crates/pack/tests/mutations/PACK19-1.mutation`。結果は DETECTED（失敗した試験は上の 3 件）。既存の PACK35-1 も DETECTED のまま
- 件数の期待値 `scripts/expected-test-counts.txt` は版上げ PR の関門の値で、本変更では更新しない

### 5.227 「次の章へ進む」の押下と CH-0 から CH-3 の通し（TASK-526 2/2。SHELL-10・INTRO-3）

「次の章へ進む」の押下を `decode_action`（`next-chapter`。空の payload は復号しない）と `update` に結線した。payload は今の提示（`next_chapter`）の開始点の ID とバイト単位で完全一致したときだけ受け（`press_target`。SAFE-33）、移動先の節点は一致した開始点を IR の `starts` から引く。移動は core の `Control::GoTo`（TASK-213）で、世代が +1 になり、処理中が解け、移動先へ入り、`ops` に記録されて履歴の復元でも再現する。提示なし・処理中・不一致・表示だけの間は core を呼ばない（fail-closed）。

- native: `crates/web/tests/next_chapter.rs`（13 件）
  - 一致の押下で `main/st-1-1` に入り、世代が +1・処理中でなく、提示が消えて `next_chapter` が dirty になり、枠の適合と a11y の検査が通る
  - 不一致の 8 形（提示していない開始点・`#` つき・節点の uid そのもの・前後の空白・大文字違い・未知の ID）は状態を変えない。空は復号しない
  - 提示が無い 3 場面（途中の Step・達成の前・最後の章の後）と表示だけの間は、正しい ID でも何もしない
  - 通し: 変種の IR で、画面から送れる手と各章の後の 1 個のボタンだけで CH-0 から CH-3 まで進む。途中は 0 個、各章の完了後はちょうど 1 個、最後の章の後は 0 個
  - 記録と復元: 越えた後の `history_to_save` の `ops` に移動先が `main/st-1-1` の `go_to` が 1 件あり、同じ履歴を新しいホストで開くと `main/st-1-1` が再現する（5.224 の「`ops` を照合していない」を閉じた）
- ブラウザ: `tools/browser/next-chapter.test.mjs`（CSP あり・なしで 6 件）。試験の設定の `?fixture=ch0-3-chapters`（`test_fixture.rs` の `ch03_chapters_ir_bytes`。各章の最後を `final` にし開始点を章の先頭 4 件にした変種。`pack.id` は `ch0-3-chapters`）で開く
  - 通し（実クリック）: `journey` の `chaptered` の経路で、各章の完了後にボタンがちょうど 1 個・「移動」ボタンが 0 件で、押すと次の章の先頭へ入り世代が進み、20 Step が達成になる。各章の途中と最後の章の後は 0 個
  - キーボードだけ: CH-0 から CH-1 の `main/st-1-4` まで、信頼されたマウスのイベント 0 件で境目を越える
  - 再読み込み: 章を越えた後の再読み込みで同じ Step に戻り、ボタンは 0 件
  - どれも安全の計数・配信元の外への通信・エンジンの警告と無視が 0 件。`journey` の既定の経路は変えず、共有する `ch0-3-run`・`clarify-draft`・`shell-widths`・`direct-link-reload` は変わらず通る
- 変異: `SHELL10-4`（payload を今の提示と照らさない）を `crates/web/tests/mutations/` に足した。手で当てて「不一致の押下は状態を変えない」試験が失敗することを確かめた（2026-10-22）。`make check-mutations` の全件は未実行
- 実行結果（2026-10-22）: `cargo test -p fandhe-workshop-web` の該当試験・`test_fixture` の単体試験は通った。ブラウザは `next-chapter` 6 件と上の共有する 4 ファイル（18 件）が `SKIP_WASM_OPT=1` の試験の設定のビルドで通った。全件の `make test-browser` は CI の `browser-test` に任せる
- 限界: core に `Control::Begin`（TASK-217）が無いので `Control::GoTo` で移る。開始点の `effects` は適用せず、章をまたぐ継続では前の章の世界状態を引き継ぐ（試験用の教材の開始点は `effects` が空）。周回の開始ではない。章の完了は引き続き状態から導き、`ChapterCompleted` 等のイベントでは表さない
- 範囲外: `Begin` への置き換えと開始点の `effects` の適用（TASK-217）、2 周目の開始（TASK-254）、試験の設定への「移動」ボタンの追加（#910・#1127）、範囲外の章の画面での確認（5.228 で済んだ。TASK-527）、最終の配置（TASK-534・TASK-548）

### 5.228 範囲外の章へ進まないことの観察（TASK-527。SHELL-12・D-67）

決定 D-67（2026-10-21）は、試行や体験で一部の章だけを使うとき「IR を絞る」案を採った。配る IR の `starts` に範囲外の開始点を含めなければ、範囲外の章の最初の Step へ入る操作は画面に出ない（`next_chapter` は開始点が無いと `None`、`start_select` は `starts` に無い ID を `start_unknown` で断る）。本件は製品のコードを変えず、この性質を CH-3 を最後に使う設定で固定する。

試験の設定に 2 つの教材を足した（`crates/web/src/test_fixture.rs` の `ch03_trial_ir_bytes`）。`?fixture=ch0-3-trial` は範囲外の章 CH-4 のノード `main/st-4-1` を IR に残し、開始点を CH-0〜CH-3 の先頭 4 件だけにした IR（`pack.id` は `ch0-3-trial`）。対照の `?fixture=ch0-4-full` は CH-4 の開始点 `s-4-1` を足したもの（`ch0-4-full`）。

| 試験 | 内容 |
|---|---|
| `crates/web/tests/next_chapter.rs` の 4 件（範囲外の章のノードが_ir_にあっても…、対照_範囲外の章の開始点があれば…、絞った_ir_では範囲外の章への押下を…、絞った_ir_では範囲外の開始点の指定を…） | 絞った IR では CH-3 の後の提示が `None`・CH-0〜CH-2 の導線は残る。対照では `s-4-1` が出る。範囲外の ID・節点の uid を payload で送っても状態も導線も変わらない。`select_start` が `Unknown` で断る |
| `crates/web/src/test_fixture.rs` の単体試験 | 2 教材が読め、クエリは完全一致だけで選ばれ、絞った IR は CH-4 のノードを持つが開始点には含まれない |
| `tools/browser/out-of-range-chapter.test.mjs`（CSP あり・なしで 6 件） | 絞った IR で CH-0 から CH-3 の最後まで通し、完了直後・少し後・再読み込みの後に、次の章へ進む・範囲外の ID の要素・「移動」ボタン・範囲外の章の題が 0 件。`?start=s-4-1` は `start_unknown` で断り、値は DOM・コンソールに出ず保存先が変わらない。対照では CH-3 の後にちょうど 1 個出て押すと `main/st-4-1` に入り、`?start=s-4-1` もその節点で開く。安全の計数・外部通信・警告と無視は 0 件 |

変異: `SHELL12-1`（開始点が無いとき別の開始点で代える）を `crates/web/tests/mutations/` に足した。2026-10-22 に `scripts/check-mutations.sh` で DETECTED（失敗した試験は上の native の 4 件のうち 2 件と、既存の 2 件）。

実行結果（2026-10-22）: native は既定の feature と `--features test-hooks` の両方で `next_chapter` 17 件が通った。ブラウザは `SKIP_WASM_OPT=1` の試験の設定のビルドで `out-of-range-chapter` 6 件と、共有する `next-chapter`・`direct-link-start` が通った。全件の `make test-browser` は CI の `browser-test` に任せる（ローカルでは未実行）。

限界:

- 絞った IR は試験の中で手で作ったもので、`compile` で絞る指定は TASK-67（PACK-43）にある。他の章への `next` を残した IR は実行時には拒否されず、`compile` の `E-REF-STEP` に任せる
- 開始点の `effects` は適用しない（5.227 と同じ）。INTRO-36 の「CH-3 の完了を伝える文」は教材 repo の側にある

範囲外: TASK-67、spec の SHELL-12 の「証拠の種類」の書き戻し、INTRO-36 の台本・文、`Control::Begin`（TASK-217）。`scripts/expected-test-counts.txt` は版上げ PR の関門の期待値で、本変更では更新しない。

### 5.229 `compile` の章の範囲と他の章への `next`（TASK-67。PACK-43・SHELL-12・D-67）

決定 D-67（2026-10-21）は「IR を絞る」案を採った。画面の側は 5.228 で固定済みで、本件はその IR を `compile` で作る形式の面である。作り方は spec で未決のため、TASK-67 の変更対象に従い `compile` の引数（`--chapters <id>[,<id>...]`。`docs/cli.md` 版 7）にした。教材パック全体を検査してから、選んだ章の `main` の Step と、そこから `use` で呼ばれた実体（入れ子を含む）だけを残し、開始点は残した Step を指すものだけにする。他の章の Step を指す `next` は `validate`・`compile` とも `E-REF-STEP`（`docs/diagnostics.md` 版 15 の注）にする。

| 試験（`crates/pack/tests/compile_chapters.rs`） | 内容 |
|---|---|
| `pack43_cross_chapter_next_is_ref_step` | 他の章への `next` が `validate`・`compile` で `E-REF-STEP` 1 件（file・item 固定）。`compile` は終了コード 1 |
| `pack43_undefined_target_is_not_doubled` | どの章にも無い宛先は従来の 1 件のまま |
| `pack43_narrowed_ir_has_no_out_of_range_start` | 範囲外の開始点・章の node・入れ子の実体が 0 件。範囲内の `m1` の実体は残り、`load_ir` が読めて辺がすべて解決する |
| `pack43_all_chapters_is_byte_identical` | 全章の指定（順不同）が指定なしとバイト単位で同じ |
| `pack43_narrowing_is_deterministic` | 同じ指定で標準出力と `-o` が同じバイト列 |
| `pack43_format_version_recomputed` | 本文の木を持つ章を外すと版 2 から 1 に求め直す |
| `pack43_unknown_or_ambiguous_chapter_exits_2` | 未知・重複した章 ID で終了コード 2・`error:` 1 行・標準出力が空・`-o` のファイルに触れない |
| `pack43_chapters_without_start_is_structure` | 絞った後に開始点が 0 件なら `E-STRUCTURE`（終了コード 1） |
| `pack43_option_shape_errors_exit_2` | 値なし・空の要素・重複・2 回の指定・`-` 始まりは終了コード 2。`-o` と順不同で通る |
| `pack43_pack_diagnostics_are_unchanged_by_selection` | 範囲外の章の欠陥も、指定の有無で同じ診断になる |

- 既存の `validate_reach.rs` の試験データは章をまたぐ `next` に依存していたため、章の中で閉じる形に直した（期待は弱めていない）
- 変異: `PACK43-1`（開始点の絞り込みを外す）・`PACK43-2`（章をまたぐ `next` の検査を無効にする）を `crates/pack/tests/mutations/` に足し、2026-10-22 に `scripts/check-mutations.sh` で 2 件とも DETECTED。`make check-mutations` の全件は未実行
- 限界: 開始点（`starts[].node`）が他の章の Step を指すことは制限しない（spec は `next` だけを挙げる）。絞った IR で `E-LEFT-TARGET-ROW` の静的な近似は再実行しない。条件が範囲外の Step を参照する場合の評価は未確認。モジュールの `#` 開始点は、そのモジュールが範囲内の章から呼ばれるときだけ残る
- 範囲外: spec の SHELL-12・PACK-43・INTRO-36 の「証拠の種類」の書き戻し、教材 repo の試行用の配信設定への `--chapters` の記載、章の ID の重複の一般の診断化（`--chapters` の指定時だけ「曖昧」で止める）、到達性（`reach.rs`）を章ごとの名前空間に分けること。`scripts/expected-test-counts.txt` は更新しない

### 5.230 読み込めない・開けない履歴の次の行動（TASK-334。SAVE-22・D-57）

D-57（2026-10-12）は、読み込めない履歴に理由ごとの次の行動を 1 つ示すと決めたが、理由ごとの割り当ては「実装のタスクで案を出し、ユーザーが決める」とした。本件の割り当ては案であり、決定は得ていない。割り当ては contract の `load_next_action::next_action`（網羅 match）、文は web の `shell_notice::next_action_text`（仮置き）で、通知の文の理由コードの後ろに付ける。押せる操作にすること（TASK-538）と文面の確定（TASK-549・TASK-534）は含まない。

| 次の行動 | 理由コード（案） |
|---|---|
| 別のファイルを選ぶ | `too_large`・`too_deep`・`too_many_ops`・`truncated`・`not_json`・`missing_version`・`malformed`・`other_pack`・`bad_version`・`replay_mismatch` |
| 書き出し元の版の教材で開く | `history_format_newer`・`format_newer`・`vocab_newer`・`engine_newer`・`format_older`・`engine_major`・`content_major`・`start_missing`・`replay_diverged` |
| 新しく始める | どの理由にも割り当てない（読み込みの経路では今の進みを捨てる操作に誘わないため。起動時の壊れた履歴 SHELL-17 で使う余地として値だけを持つ） |

| 試験 | 内容 |
|---|---|
| `crates/contract/src/load_next_action.rs` の単体試験 | 19 理由が独立の表と一致する・判定順の全件を網羅する・段ごとの方針・往復と未知の拒否・`StartFresh` が 0 件 |
| `crates/web/src/shell_notice.rs` の単体試験 | 全 19 理由で文が理由コードと次の行動の文で終わる・3 つの文が空でなく互いに異なる |
| `crates/web/tests/refused_history.rs` | 読み込み・起動時の `format_newer` で直書きの文が出る。壊れたファイルで「別の履歴ファイルを選ぶ」の文が出て、状態・保存の値が変わらない |

- 実行: 2026-10-22 に contract・web（`--features test-hooks`）の native 試験が通ることを確認した。ブラウザ試験は未実行・未作成（`make test-browser` は CI に任せる）
- 範囲外: 押せる操作と shell 契約の版上げ（TASK-538）、文面の確定（TASK-549・TASK-534）、割り当ての決定と spec の SAVE-22・SHELL-36 への反映、表示だけで開いた履歴への提示（VER-8）、起動時の壊れた履歴の案内（TASK-533）、ブラウザでの理由ごとの確認。`scripts/expected-test-counts.txt` は更新しない

### 5.231 表示だけの履歴の保存領域と書き出し（TASK-335。SAVE-25）

TASK-176（5.220）は操作が状態・ログを変えないことを固定した。本件は保存の側を、ブラウザで実際の保存先と実際のダウンロードで確かめる。PoC-6 の S-14・S-15（1.2.0・2.0.0 で表示だけ）に当たる。

- 試験用の口（feature `test-hooks` のときだけ）: `debug_export_prepare_live`（`static/test-hooks.js` の `exportPrepareLive`）。受講中のホストの `export_live` が返すバイト列を、直列化し直さずに `blob:` URL にする。履歴の中身は返さず、書き出せないときは `{"ok":false,"gate":…}`。既存の `exportPrepare` の戻りには `gate: null` が加わった
- `tools/browser/view-only-storage.test.mjs`（6 件）

| 場面 | 保存先 | CSP | 確かめること |
|---|---|---|---|
| 起動時の復元で主版違い | localStorage | あり・なし | 入力・クリック・保存・停止の後、保存の待ちを過ぎても保存の試行・履歴の鍵への書き込みが 0 件。保存先の値が置いた値のまま。書き出したファイルが置いた値とバイト単位で同一。再読み込み（`pagehide`）の後も同じ |
| 同上 | IndexedDB（localStorage を閾値の手前まで埋めて選ばせる） | あり | 同上 |
| 読み込みで表示だけ（表の `view_only` の全 6 行） | localStorage | あり・なし | 保存先の全体の digest の一覧が読み込みの前後と離脱・開き直しの後で同じ。書き出しが読み込んだファイルと同一 |
| 対照: 開ける履歴 | localStorage | あり | 同じ操作で保存先の値と書き出しが変わる（比較が変化を検出できる） |

- 実行結果（2026-10-22）: 新しい試験 6 件と、`view-only`・`history-export`・`history-import`・`version-histories`・`secret-hold` を含む 28 件が通った。`make lint` は通った。`make test-browser` の全件は未実行（CI の `browser-test` に任せる）
- 自己確認: `export_live` が控えを返さず作り直す写しで 5 件が「書き出しの門が ready でない」で失敗した（2026-10-22）。`history_to_save` の表示だけの判定を外した写しは失敗しなかった。表示だけの間は履歴の記録が無く `envelope_for_save` が `None` になるため、その判定は二重の防御であり、保存先の不変は結果（書き込み 0 件・値の一致）で見ている
- 限界: Chromium だけ。IndexedDB への切り替え後の `pagehide` の書き込みは確定を保証しない（SAVE-9）。CSP なしは localStorage の場面だけ。書き出しの名前の教材 ID は空で、名前の検査は `history-export` が持つ
- 範囲外: 表示だけの動作の実装と状態・ログの不変（5.220）、見せ方（REVIEW-4/5・SHELL-20）。`scripts/expected-test-counts.txt` は更新しない

### 5.232 保存の失敗の枠の通知欄への表示（TASK-534 1/2。SHELL-18・SAVE-10）

- 目的: localStorage の容量超過で「保存できませんでした（QuotaExceededError）」が通知欄に出て、状態は進み既存の保存は変わらず、容量を空けた後の次の変更で表示が消えることを、実際の画面（S-17 相当）で確かめる。IndexedDB の失敗の確認（SHELL-19）は 2/2（#727）が持つ。
- 決定（案 A）: 右のログ部品の `save-notice` を外し、枠の通知欄 `shell-notice` に `NoticeKind::SaveFailed`（識別子 `save-failed`・文面 `msg-save-failed`・押せる操作なし）の項目として出す。案 B（両方に出す）は `role="status"` の欄 2 つが同じ失敗を読み上げるため、案 C（ログ部品に残す）は幅 960px 未満で成果物のタブを選ぶと見えず SHELL-18（Must）を満たさないため退けた。自動運転のため人の承認は得ていない。
- 理由コード（`data-notice-reason`）: 閉じた列挙 `StorageError` から固定の ASCII へ写す（`QuotaExceeded`→`quota_exceeded`・`Unavailable`→`storage_unavailable`・`Failed`→`write_failed`。`save_notice::save_failure_reason`）。文は `save_notice_text` のまま（`Unavailable`・`Failed` は仮置き）。
- 追従: native 試験 `save_failure`・`save_failure_idb`・`stale_notice`（`common/storage_fakes.rs` の `save_failed_item` で通知欄の項目を読む。試験名は変えない）、`dirty_binding_points`・`host_contract`（field 24 → 23）、ブラウザ試験 `save-quota`・`save-idb-failure`・`stale-tab`。`a11y::SAVE_NOTICE_TESTID` は任意の通知欄として残し、現在は 0 個。
- ブラウザ試験 `save-quota`: 通知欄がちょうど 1 個、容量超過で項目が 1 件（理由 `quota_exceeded`・文面 `msg-save-failed`・ボタン 0）、旧来の `save-notice` が 0 件、空けた後に項目 0 件。新しい場面として幅 360px で操作の面で失敗を起こし成果物のタブへ切り替えても項目が見える。
- 変異: `SHELL18-2`（保存の失敗の項目を足さない）を `save_failure` が検出する。`SAVE10-1`・`SAVE10-3`・`SAVE11-2`・`SAVE13-1` も引き続き検出した（2026-10-22）。
- 過去の記録（5.x の `save-notice` の記述）は、TASK-534 1/2 で `shell-notice` の `save-failed` 項目へ移した。
- 実行結果（2026-10-22）: `make lint`・`cargo test -p fandhe-workshop-web`・上の 3 本と `element-identity`・`secret-warning`・`storage-lost`（Chromium）が通った。`make test-browser` の全件・`version-notice`・`refused-history`・`csp-runs` は未実行（環境変数の前提が要るため CI の `browser-test` に任せる）。`scripts/expected-test-counts.txt` は更新しない。

### 5.233 IndexedDB の失敗の枠の通知欄への表示（TASK-534 2/2。SHELL-19・SAVE-11）

- 目的: IndexedDB の容量超過・書き込みの失敗でも、localStorage の失敗（5.232）と同じ表示が枠の通知欄に出て、メモリ上の状態は進み、既存の保存は変わらないことを確かめる。
- 観察（2026-10-22。Chromium 1243・Playwright の CDP）: 実際の容量超過は再現できなかった。1 手目が IndexedDB へ届いた後に `Storage.overrideQuotaForOrigin`（`quotaSize` 1 と 1,024）を送ると、`Storage.getUsageAndQuota` は `overrideActive: true`・quota 1,024・usage 73,728 を返す。それでもアプリの 2 手目の保存・ページの再読み込み後の保存・直接の `put`（10,000・1,000,000・8,000,000 文字）はすべて成功し、`transaction.abort` は起きなかった。この Chromium の IndexedDB は CDP の quota の上書きを書き込みの可否に使わない。
- 判断（推奨案 A。自動運転のため人の承認は得ていない）: 容量超過でない失敗（`Failed`）は同じ欄・同じ項目の形で出し、文は理由ごとのまま。容量超過の文・理由コードの完全一致は、容量超過を実際に起こせる手段が見つかるまで native 試験（`save_failure_idb`。偽の IndexedDB が `QuotaExceeded` を返す）に任せる。案 B（全失敗を容量超過の文に寄せる）は誤った理由を出すため、案 C（`DOMException` の合成）は試験が試験自身を試すことになるため退けた。
- 試験（`save-idb-failure`）: 既存の中断（`Failed`）の場面に表示の形の照合を足した（欄 1・項目 1・ボタン 0・旧来の `save-notice` 0・理由 `write_failed`・文面 `msg-save-failed`。`save-quota` と同じ形）。新しい場面として幅 360px で成果物のタブを選んでも項目が見える。
- 未確認: 実際の IndexedDB の容量超過で `storage_idb.rs` の `watch()` が `QuotaExceeded` に分類すること（`tx.error().name()`）。5.169 の限界 3 は解消していない。実機で容量が尽きる環境（小さな quota の別のブラウザ・OS の容量逼迫）での確認が要る。
- 対照: 変異は足していない（Rust は変更なし）。
- 実行結果（2026-10-22）: `save-idb-failure` 5 件（Chromium）が通った。`make test-browser` の全件は未実行（CI の `browser-test` に任せる）。`scripts/expected-test-counts.txt` は更新しない。

### 5.234 履歴の書き出しと読み込みの操作を枠に置く（TASK-531 1/2。SHELL-15）

TASK-330 が書き出しと読み込みの仕組みを、TASK-309 が枠の識別子と宛先の形を入れたが、受講者が押せる操作は画面に無かった（読み込みは試験の設定の `test-hooks.js` からだけ届いた）。本件は枠に操作を置き、`EngineHost::export_live`・`open_history` に結線する。同じ状態の詳しい照合と書き出しのバイト単位の一致は 2/2（#724。5.244）が持つ。

設計（仮置きの点は `docs/known-gaps.md`）:

- 書き出しは 2 段。「履歴を書き出す」（`shell-export`）でホストが `export_live` を通し、`Ready` のときだけリンク（`export-download`）を出す。`Held`・`Empty`・URL を作る口が無いときは、固定の文（`export-status`）だけを出す（fail-closed）。受講者の値は文に入れない
- 上流の属性の検査（`is_safe_url`）は `blob:` の `href` を落とす。リンクの URL は `data-export-url` に載せ、`static/app.js` が自分のオリジンの `blob:` だけを `a.href` へ写す（`setAttribute` は使わない）。この書き込みは `crates/web/tests/no_hidden_triggers.rs` が許す唯一の例外で、対象は書き出しのリンクの `href` だけ・値は `blob:${window.origin}/` で始まるものだけ・件数は 1 件に固定し、`blob:` の前置き検査が無い場合や件数が増減した場合は失敗する
- 書き出しの結果は 0 か 1 件の keyed list（`export_result`）。履歴（ログ・下書き）が変わる・履歴を開く・表示だけに入るとリンクを外し、URL を破棄する（drop で `revokeObjectURL`）。選択の切り替えでは外さない
- 読み込みは `shell-import`（`data-action="import-history"`、`data-payload` なし）の入れ物の中に `label` と `input[type=file]` を置く。入れ物は `display: contents` で、label・input は上流の対話要素の境界で止まるため、ファイル画面を開くクリックでは `import-history` が届かない（空の本文と区別できないため）。選んだファイルは `app.js` が `file.slice(0, 上限 + 1).arrayBuffer()` でバイト列のまま読み（文字列にすると不正な UTF-8 が置換されるため）、wasm の `import_history` で届ける。Rust は DOM の値を読まない。上限は `history_read_limit`（`MAX_HISTORY_BYTES + 1`）
- 表示だけの間の allowlist に 2 つの action を足した。書き出しは読み込んだバイト列そのものを返す
- 置き場所は root の直下で通知欄の後ろ・`tablist` の前（仮置き。TASK-534・TASK-548）

| 試験 | 内容 |
|---|---|
| `crates/web/tests/history_ops.rs`（13 件） | 完成した画面の `shell`・`a11y` の検査と操作が 1 つずつ。入れ物が `data-payload` を持たず中の部品が `data-action` を持たない。押すとリンクが 1 件・もう一度押すと作り直して前を破棄。状態を変える操作でリンクが消え選択の切り替えでは消えない。履歴が無い・警告の間・口が無いときは文だけ（URL の作成の失敗は「履歴が無い」と言わず、警告が解けると警告の間の文が消える）。書き出しを押しても同じ field を二重に積まない。読み込みの結線（壊れた本文・空は状態を変えず通知）。開いた履歴で前のリンクが外れる。復号は 2 つだけ受ける |
| `crates/web/src/history_ops.rs`・`history_export.rs` の単体試験 | 描画の形・固定の文・不正な UTF-8 の本文の変換（大きさ・入れ子の上限で断られるものは長さと括弧の並びを保ち、同じ理由で断られる） |
| `tools/browser/history-ops.test.mjs`（6 件） | 画面の操作だけで書き出し（`Download`）。もう一度押すとリンクが新しい URL で作り直され、`app.js` が写した `href` が `data-export-url` と一致し 2 つ目もダウンロードできる → 別のオリジンの input へ `setInputFiles` → 開いて同じログの長さ・状態。ファイル画面を選ばずに閉じても結果・通知が変わらない。書き換えた・壊れた・空のファイルは開かず状態・ログが変わらず通知が出る。先に選んだファイルの読み取りが後から終わっても後の選択の結果を差し替えない。読み取りの失敗では状態・保存先を変えず通知が 1 件出る。配信する設定でも書き出しの文が出る。外部への要求・CSP 違反・ページのエラー・ダイアログ・ポップアップが 0 件、URL に状態が載らない |

- 既存の試験の更新: `version_notice.rs`（書き出し・読み込みの復号は受ける）、`dirty_binding_points.rs`（`export_result` は書き出しの操作の後に立つ）、`glue_contract.rs`・`state_paths.rs`（新しい呼び出しの許可）、`glue_scale.rs`（上限 531 から 552。`import_history`・`history_read_limit`・URL の口の結線）
- 実行結果（2026-10-22）: `make lint`・`make test`・`--features test-hooks` の native 試験・`make check-deps`・`make check-dom-reads`・`make check`・`make build-web` と `make check-delivery` が通った。ブラウザ試験は新しい 6 件と、関連する `history-export`・`history-import`・`view-only`・`view-only-storage`・`refused-history`・`version-notice`・`version-histories`・`history-versions`・`secret-hold`・`secret-choice`・`inline-free`・`csp-runs`・`shell-columns`・`shell-tabs`・`shell-widths`・`element-identity`・`esc-once`・`smoke-hello`・`safety`・`csp-placement`・`next-chapter` を単独で流して通った。`make test-browser` の全件は未実行（CI の `browser-test` に任せる）。`make check-generic-words` は本件と無関係の `crates/pack/src/guidance.rs:606` で失敗する（既存）
- 限界: Chromium だけ。OS のファイル画面（macOS）は人の確認（TASK-342）、配布物は TASK-114 が持つ。配信する設定は IR を持たないため、開く経路の確認は試験の設定で行う（配信する設定では `app.js` から `import_history` までの例外・外部要求が 0 件のことだけを見る）
- 範囲外: 読めない履歴 7 種の理由の表示（TASK-532）、未送信の下書きの案内（TASK-537）、通知欄・枠の最終の配置（TASK-534・TASK-548）。`scripts/expected-test-counts.txt` は更新しない

### 5.235 L2 の配布物の作成スクリプト（TASK-112 1/2。DLV-13・DLV-18）

- 目的: D-58・D-71 の形（miniserve 0.35.0 と静的ファイル）で、人の操作が「展開・`./start.sh`・URL を開く」の 3 つになる zip を作る。`tools/build-l2.mjs` が `fandhe-workshop-l2/{start.sh, miniserve, site/}` を作り、`site/` は配信する設定の配信物そのものにする。
- 決定（自動運転のため人の承認は得ていない）: zip は Node の `zlib` で自前に書く（`zip` が前提の道具に無いため。時刻・順序・mode を固定して決定的にする）。miniserve は上流の Release のビルド済み実行ファイルを使い、sha256 を `tools/l2/miniserve.sha256` に固定して照合する（固定値を上書きする口は無い。2026-10-22 に Release の digest との一致を確認。実行ファイル自体の取得・実行は未実施）。`L2_PLATFORM=linux-x64` は模擬環境の試験（#532）のための追加で、仕様に明記は無い。
- 試験（`tools/build-l2.test.mjs`。22 件）: 木の完全一致・mode（`start.sh`・`miniserve` が 0755、他が 0644）・`site/` のバイト一致・決定性・`start.sh` の起動（`PATH=/usr/bin:/bin` の環境で URL を表示し、引数が `--index index.html -i 127.0.0.1 -p <PORT> site` と完全一致。不正な `PORT` は 2）・fail-closed（digest の不一致、試験用の口・`.d.ts`・シンボリックリンク・`ir/main.json` の欠落、誤用、出力先の不正で zip が作られない）・固定表の形。
- 未確認: Apple シリコンでの実行（上流のビルド済みが ad-hoc 署名だけで起動するか。人による起動は TASK-119）、実ビルドの配信物での通し、miniserve のライセンス文・推移的な依存の棚卸し（TASK-137）。
- 阻害要因（範囲外）: 配信する設定の web は実行時に `ir/main.json` を取得しないため、この配布物でまだ CH-0〜CH-1 は通せない。#532 と親 #530 の前に、IR を実行時に読む実装が要る（5.238 で最小限を入れて解消した）。
- 実行結果（2026-10-22）: `make test-build-l2` の 22 件が通った。`scripts/expected-test-counts.txt` は更新しない。

### 5.236 参照元の版を上げる手順書の形（TASK-182 2/2。VER-15・UMAP-13・BUILD-26）

- 目的: 手順書 `docs/upstream-bump-procedure.md` が、教材テストを通すこと・公開前に人が対応表を確かめて記録を残すこと・生成物を無検証で公開しないことを固定し、引用したパスが実在することを確かめる。
- 試験（`crates/pack/tests/upstream_bump_procedure_doc.rs`。5 件）: 1 版の欄 `procedure_has_version_block`・2 節の順 `procedure_has_sections_in_order`・3 固定の文 `procedure_states_acceptance_rules`・4 パスの実在 `procedure_paths_exist`・5 終了コードの表 `procedure_tool_exit_codes_match`。
- 変異注入: 置かない。文書の形の検査で、負の対照は固定の文を 1 つ消すと失敗することの手元の確認とした（2026-10-22 に確認し、元に戻した）。
- 限界: 字面の検査で、手順の意味の正しさは人が読んで確かめる。
- 範囲外: 実際の記録と最終の承認（人間の承認点）・道具の CI への組み込み・教材 repo の試験。
- 実行結果（2026-10-22）: 5 件が通った。`scripts/expected-test-counts.txt` は更新しない。

### 5.236b 影響する Step の洗い出し道具（TASK-182 1/2。VER-15・UMAP-5）

- 目的: 対応表（CSV 版 1）と変わった参照名・ファイルから、影響する Step を機械的に列挙する道具 `tools/upstream-impact.mjs` が、期待どおりの Step を決定的に出し、不正な入力を fail-closed で止めることを確かめる。
- 試験（`tools/upstream-impact.test.mjs`。`make test-upstream-impact`。`node:test`、試験用の対応表を内蔵）: 1 参照名の全体の変更と同名の Step の scope 区別・2 和集合と整列・3 `--changes` のファイル単位と `path` 空の行・4 併用・5 該当なし・6 入力の行順によらずバイト一致・7 `--vocab`（教材に無い Step・誤った scope で 1、全行が一覧にあれば 0、`use` 行と集計行の読み取り）・8 表に無い参照名・9 不正な入力 21 形が 2 で標準出力が空・10 自由記述の列と不正なセルの値を写さない・11 `--help`。ほかに純粋な関数の試験 1 件。
- 変異注入: 置かない。`crates/` の変更が無く、`scripts/check-mutations.sh` の対象外の Node の道具であり、負の対照は試験 7・9 が担う。
- 限界: 行番号では照合しない（版を上げると行がずれるため。`lines` 列は人が読む欄）。引用符つきの CSV は読めない。表の形は参照版の置き場（TASK-717）の決定で版 2 になりうる。
- 範囲外: 手順書（TASK-182 2/2。5.236 で追加）・CI への組み込み（人間の承認点）・教材 repo の実際の対応表と教材テスト・出典の検査。
- 実行結果（2026-10-22）: 12 件が通った。`scripts/expected-test-counts.txt` は更新しない。

### 5.237 枠の契約の版 2 の区画と識別子（TASK-516 1/2。SHELL-6・SHELL-8・SHELL-24・SHELL-30）

- 目的: 枠の契約（`shell`）を版 1 から版 2 に上げ、D-72（上部の「模擬」・教材名・位置）・D-54（早送りの切り替えの区画）・D-74（ブラウザの案内）の区画と識別子を足したことを確かめる。版 1 を使う画面が壊れないことも示す。再読み込み・書き出しの案内・履歴の削除・理由コードの束縛は TASK-516 2/2（#696）が持つ。
- 期待と定義の対応:

| 要件 | 決定 | 定義 |
|---|---|---|
| SHELL-6 | D-72 | `TopSection::Mock` が `Fixed` かつ `ExactlyOne`。文字は `MOCK_LABEL`。必須の検査は `check_complete_v2` |
| SHELL-8 | D-72 | `TopSection::Title`・`Position` が `Fixed`。位置は `StepPosition`（`1 <= m <= k`）と属性 `data-position-m`・`data-position-k`。上部の外の置き場は `top-section-outside-top` |
| SHELL-24 | D-54 | `ControlSection::FastForward`（`shell-fast-forward`）。入・切は `ops` に入れない（`OPS_RECORDS_FAST_FORWARD` が偽であることを参照） |
| SHELL-30 | D-74 | `NoticeKind::BrowserNotice`（`browser-notice`）・`MessageId::BrowserNotice`・`BROWSER_NOTICE_TEXT`。理由は任意、操作は `DismissNotice` だけ |

- 版 1 から版 2 の差分:
  - 足したもの: 上の定義、`check_complete_v2`、検査の規則 `top-section-outside-top`・`position-format`、`shell-fast-forward` の計数。
  - 値を変えたもの: `SHELL_CONTRACT_VERSION` が 2。`TopSection` の `status` が全件 `Fixed`、`Mock` の `cardinality` が `ExactlyOne`。`NoticeKind::ALL`・`MessageId::ALL` が 6 件（末尾に追加）。
  - 変えていないもの: 版 1 の識別子・action の名前・通知の種類・文面の識別子の値と順、`check_complete`・`check_no_undefined` の版 1 の規則、host・描画。
  - 版 1 の試験で直した 5 か所（`crates/web/tests/shell_contract.rs`）: (1) `版は_1` を版 1 の値が残ることの試験へ（版の値は `shell_contract_v2` が固定する）、(2) 区画の確定と個数、(3) 識別子の件数（`+ 1`）、(4) 通知の種類・文面の識別子の列と理由・操作の表に `BrowserNotice` を追加、(5) 「検討中の項目は版 1 に無い」から `shell-fast-forward` を外し、通知の種類の件数を 6 に。
- 版 1 を使う画面が壊れない根拠: 版 1 の値はすべて不変で `ALL` の先頭に残る。足した規則は、版 1 の画面が描かない識別子（`shell-title`・`shell-position`・`shell-mock`・`shell-fast-forward`・`browser-notice`）にだけ掛かる。`check_complete` は変えず、`shell-mock` の必須化は `check_complete_v2` に分けた。既存の利用者（`shell_frame_host`・`shell_notice_sample`・`a11y_contract` ほか）は変更なしで通った。
- 試験（`crates/web/tests/shell_contract_v2.rs`。14 件）: 版・版 1 の値の保存・区画の状態と個数・位置の範囲と属性・早送りの区画・ブラウザの案内の値と UA らしい理由の拒否・正の見本（4 つの検査器）・負の見本（模擬の欠落は版 2 だけが失敗、重複、上部の外、位置の形、許されない操作）。
- 変異注入（2026-10-22。元にしたコミット `b6da7c6` の作業ツリー）: SHELL6-1・SHELL8-1・SHELL24-1・SHELL30-1 と既存の SHELL1-1・SHELL1-2・SHELL10-1・SHELL16-1・SHELL17-1・SHELL17-2・SHELL18-1・SHELL31-1 の 12 件がすべて DETECTED。
- 限界・申し送り: 画面が `check_complete_v2` へ移るのは TASK-523 の実装の後。「模擬」・位置の文言と「Step の ID は出さない」は TASK-524、早送りの action・状態の文字・無効の表示は TASK-540、ブラウザの案内の「一度だけ」と旧版の基準は TASK-544。理由コードを TASK-310 の `LoadReason` に結ぶことは 2/2。ブラウザ試験は画面の描画を変えないため流していない。
- 実行結果（2026-10-22）: `make lint`・`make test`・`make check-deps`・`make check` が通った。`make check-generic-words` は本件と無関係の既存の 1 件（`crates/pack/src/guidance.rs:606`）で失敗する（範囲外）。`scripts/expected-test-counts.txt` は更新しない。

### 5.238 枠の契約の版 2 の通知・操作の口（TASK-516 2/2。SHELL-21・SHELL-22・SHELL-36・SHELL-38）

- 目的: 5.237 で上げた版 2 に、残りの決定（D-57・D-52・D-75）を反映し、版 1・版 2 の適合試験が通ることを確かめる。版は 2 のまま据え置く（版 1 の 2/2 が版を上げずに足したのと同じ形）。
- 8 件の期待と決定と定義の対応（食い違いは spec の `d850232` の SHELL-6・8・21・22・24・30・36・38 と D-52・D-54・D-57・D-72・D-74・D-75 に照らして数えた。2026-10-22）:

| 要件 | 決定 | 定義 | 食い違い |
|---|---|---|---|
| SHELL-6 | D-72 | 5.237 の定義を再掲。`TopSection::Mock`（`ExactlyOne`。必須の検査は `check_complete_v2`） | 0 件 |
| SHELL-8 | D-72 | 5.237 の定義を再掲。`Title`・`Position`・`StepPosition` | 0 件 |
| SHELL-21 | D-57 の 1 | 枠の通知には足さない。運び手は `terminal_area` の `LineKind::Reloaded`（`reloaded`。TASK-322）。`NoticeKind::parse("reload")`・`ShellOp::from_name("reload-notice")` が `None` であることを試験が固定する | 0 件 |
| SHELL-22 | D-52 | `NoticeKind::ExportDraft`（`export-draft`）・`MessageId::ExportDraft`。理由は任意、操作は `DismissNotice` だけ。文面は固定しない。SAVE-29 の書き出しの案内とは別物 | 0 件 |
| SHELL-24 | D-54 | 5.237 の定義を再掲。`ControlSection::FastForward` | 0 件 |
| SHELL-30 | D-74 | 5.237 の定義を再掲。`NoticeKind::BrowserNotice`・`BROWSER_NOTICE_TEXT` | 0 件 |
| SHELL-36 | D-57 の 2 | 属性 `data-notice-next-action`（値は `LoadNextAction` の 3 値）。`import-refused` だけが持ち、理由から `next_action` で導く（割り当ては TASK-334 の案のまま）。規則 `next-action-missing`・`next-action-mismatch`。理由コードを `LoadReason` に結ぶ（`NoticeKind::reason_binding`。規則 `reason-unbound`） | 0 件 |
| SHELL-38 | D-75 | 操作 `DeleteHistory`（`shell-delete`）・`ConfirmDelete`・`CancelDelete`（空の payload だけ）、通知の種類 `DeleteConfirm`（`delete-confirm`）。上部に置くと `delete-in-top`、確定・取り消しが通知の外にあると `op-outside-notice`、確認に両方が無いと `check_complete_v2` の `delete-confirm-ops`。文言は固定しない | 0 件 |

- 版 2 の 1/2 からの差分:
  - 足したもの: 上の定義、検査の規則 5 件（`reason-unbound`・`next-action-missing`・`next-action-mismatch`・`delete-in-top`・`delete-confirm-ops`）、`NoticeError::ReasonUnbound`、`Notice::next_action`。
  - 値を変えたもの: `ShellOp::ALL` が 7 件、`NoticeKind::ALL`・`MessageId::ALL` が 8 件（すべて末尾に追加）。
  - 変えていないもの: 版 1 と 1/2 の識別子・action の名前・種類・文面の識別子の値と順、`SHELL_CONTRACT_VERSION`（2）、host・描画の呼び出し。
- 版 1 と版 2 の試験で直した 9 か所: `shell_contract.rs` の (1) 識別子と名前の値の固定（先頭 4 件の一致へ）、(2) 識別子の件数（`+ 7`）、(3) 通知の識別子と種類の値の固定（先頭 6 件の一致へ）、(4) 理由の規則、(5) 許す操作の表の固定（新しい 2 種を追加）、(6) `notice_new_の_5_つのエラー`・`notice_の属性と宛先` の理由を `format_newer` へ（束縛で `ReasonUnbound` が先に出るため）、(7) 検討中の項目の否定の列から `delete-history`・`shell-delete` を外す、(8) 通知の種類の件数 8。`shell_notice_sample.rs` の未定義の種類を `stale-tab` へ。`shell_contract_v2.rs` の先頭 4 件の一致と、まだ無いものの列から `shell-delete` を外す。
- 版 1 を使う画面が壊れない根拠: 版 1 の値は不変で `ALL` の先頭に残る。host の開かない通知の理由は `LoadReason::as_str` なので束縛を通る。属性が 1 つ増えるだけで、`refused_history`・`version_notice`・`history_ops`・`shell_frame_host` を含む web の全試験が通った。
- 束縛しなかった種類: `save-failed` は値が `StorageError` の写し（`save_notice::save_failure_reason`）、`pack-unloadable` は core の `IrLoadError::reason_code` が作り、どちらも TASK-310 の拒否理由の列挙ではないため不透明のままにした。
- 試験（`shell_contract.rs` 26 件・`shell_contract_v2.rs` 23 件・`shell_notice_sample.rs` 18 件）: 2/2 で `shell_contract_v2.rs` に 9 件を追加した。削除の操作の値と往復・書き出しの案内と削除の確認の値・理由コードの束縛（19 種と列挙外の 3 値）・次の行動（19 種）・再読み込みを足さないこと・正の見本（4 つの検査器）・束縛と次の行動の負の見本・削除の置き場の負の見本・確認の欠落（版 2 だけが失敗）。
- 変異注入（2026-10-22。元にしたコミット `b99123d` の作業ツリー）: 新しい SHELL22-1・SHELL36-1・SHELL36-2・SHELL38-1・SHELL38-2 と、既存の SHELL6-1・8-1・16-1・17-1・17-2・18-1・18-2・24-1・30-1・31-1 の 15 件がすべて DETECTED。最初の実行で SHELL22-1 が SURVIVED だった（期待した試験が許す操作を見ていなかった）ため、その試験に許す操作の検査を足して DETECTED にした。ブラウザ試験を使う SHELL1-* と SHELL10-*・12 は描画を変えないため流していない（未検証）。
- 限界・申し送り: 次の行動の押せる結線は TASK-538、削除の操作と確認の画面と文言は TASK-539、書き出しの案内の画面と下書きの有無の口は該当の画面のタスク、再読み込みの二重の通知の確認は TASK-536、文面は TASK-549。D-57 の次の行動の割り当ては決定が出ていない案のまま。ブラウザ試験は描画の構造を変えず属性が 1 つ増えるだけなので流していない。
- 実行結果（2026-10-22）: `make lint`・`make test`・`make check-deps`・`make check` が通った。`make check-generic-words` は本件と無関係の既存の失敗（`crates/pack/src/guidance.rs:606`）があり、新しい失敗は増えていない（範囲外）。`scripts/expected-test-counts.txt` は更新しない（版上げ PR だけが照合する）。

### 5.239 Rust のない模擬環境での L2 の完走試験（TASK-112 2/2。DLV-13・D-58・D-71）

- 目的: L2 の配布物（zip）を `PATH=/usr/bin:/bin`（cargo・rustc・rustup・node が見えない）の模擬環境に展開し、`./start.sh` を起動して、自動ブラウザ（Chromium）で URL を開き、CH-0〜CH-1（`main/st-2-1` に入るまで）を最後まで進めることを 3 回確かめる。人の操作は「展開・`./start.sh`・URL を開く」の 3 つだけで、各回でページ内エラー・CSP の違反・コンソールの error と warning・2xx 以外の応答・配信元以外への要求が 0 件、同梱の IR が 200 で 1 回取得されることを求める。
- 前提の変更（自動運転のため人の承認は得ていない。Issue ツリーに担当が無い）: 配信する設定の web は実行時に IR を取得せず、教材が 1 つも開かなかった（`EngineHost::for_mount` が `new()` だけ。5.223・5.235 に既出）。本タスクで最小限を入れた。(1) `tools/build-web.sh` は配信する設定で `IR_FILE` があるときだけ、`index.html` の印 `<!--fandhe:ir-->` を `<meta name="fandhe-ir" content="1">` にする（ほかは印の行を消す）。(2) `static/app.js` はこの meta があるときだけ固定の `./ir/main.json` を取得してバイト列を `mount_app` の第 4 引数に渡す（失敗は空のバイト列で断りに倒す。URL の値から場所は作らない。SAFE-33）。(3) `start_select::open_bundled_ir` が `load_ir`（大きさ・深さ・版・参照の上限）を通し、`start` を `starts` と完全一致で照合し、開始点の節点に入った直後の状態を core の `Session::entered_at` で組み立てる（表示層は状態を組み立てない規則 `no_progress` のため core に置いた。`Control::Begin`（TASK-217・TASK-294）が未実装の間の暫定。試験の設定の `test_fixture::session_at` と同じ状態）。読めない・選べないときは `refused`・`refused_start` の断りの画面に倒す。meta が無い配信物は取得せず従来どおり（既存の配信系のブラウザ試験の結果を変えない）。`tick_interval.rs` の `ir` の字句の検査は、`app.js` の固定の `fetch("./ir/main.json")` がちょうど 1 件のときだけ許す。
- 試験の構成: (a) `scripts/test-l2-run.sh`（`make test-l2-run`。`L2_SERVER` 必須。遅く、`make test-browser` には入れず CI の `browser-test` が流す）が web を専用の target・`CARGO_PROFILE_RELEASE_STRIP=symbols`・`SKIP_WASM_OPT=1`・`IR_FILE=crates/web/tests/data/ch0_3/ir.json` でビルドし、`tools/build-l2.mjs`（`L2_PLATFORM` は `uname` から）で zip を作り、`tools/browser/l2-run.test.mjs` を流す。専用の target と strip は、`check-delivery.sh` が WASM のバイト列に `debug_` が無いことを求めるのに、wasm-opt を使わない CI のビルドには名前情報が残るため。(b) `l2-run.test.mjs`: 前提の検査（模擬環境の PATH に 4 つのコマンドが見えない。見えるコマンドを検出する対照つき。`command -v` は dash が先頭の 1 件しか見ないため 1 件ずつ呼ぶ）・3 回の完走（毎回新しい展開先・起動・browser context。環境は PATH・HOME・PORT・`FANDHE_NO_OPEN` だけ）。(c) `l2-journey.mjs`: `ch0-3-journey.mjs` の `journey` の操作の列を共有し（`mouseDriver` の実クリック・fill・Enter だけ。隠し要素は使わない）、状態の観察だけを DOM に差し替える（`journey` の `observe`）。確認待ちは「`step-status` が受付済み・idle・ログの最後の行の `data-step` が当の Step」、入力待ちは「`step-status` が達成・idle・最後の行が直前の Step（先頭はログ 0 行）」、拒否後は「最後の行が rejected か clarified で当の Step」。(d) `l2-journey-calibrate.test.mjs`（`make test-browser` に入る。2 件）: 試験の設定の `?fixture=ch0-3` に同じ誘導を DOM の観察で流し、各観察（22 回）で `stateJson` の現在の Step・状態と一致することを確かめる。1 Step 先の期待を DOM が示さない対照つき。
- 限界: 教材は本物の CH-0〜CH-1 の 50 操作でなく `ch0_3` の試験データ（20 Step のうち 11 Step 目まで）。Linux の musl 版の miniserve で模擬し、Apple シリコンでの起動は TASK-118・119。Chromium だけで流した。現在の Step の ID は DOM に出ないため、DOM の観察は IR の node の並びから直前の Step を引く（較正の試験が守る）。終点（`main/st-2-1`）の入力待ちは、直前の Step の達成の語とログに加えて入力の口（下書き・送信が各 1 個で選択肢が 0 個）も見て、遷移が欠落して直前の選択肢の Step で止まった状態を断る（`l2-journey-calibrate` の対照が守る。2026-10-22）。meta の無い配信物は IR を取得しない（SHELL-32・BUILD-11 の観察は TASK-545・546 で見直す）。開始時の効果・案内の水準は反映しない（TASK-217・294）。CI の `check-workflow-secrets`・`check-fork-ci` はこの環境に ruby が無く未実行。
- 受信量の上限（レビュー指摘の対応）: `app.js` は同梱の IR を `r.body` の reader で `ir_read_limit()`（`MAX_IR_BYTES + 1`。`start_select.rs`。定数を JS に二重に持たない）バイトまでだけ読み、超えたら `reader.cancel()` で中止して 1 つの `Uint8Array` にまとめる（Content-Length は使わず実際の受信量で数える）。大きさの断りは `load_ir` の `TooLarge` が出す。`tools/browser/ir-recv-limit.test.mjs`（`make test-browser` に入る。2 件）が、1 GiB の本体を流す配信側が最後まで送れず（送出量が上限 + 64 MiB 未満）断りの通知に倒れること、上限未満の本体は最後まで送られること（対照）を確かめる。2026-10-22 に Chromium で 2 件が通った。
- 範囲外: TASK-113（待ち受けと起動時間）、TASK-114・115（L1 と L2 の一致とオリジンの分離）、TASK-117（案内文）、TASK-118・119（人の起動）、TASK-137（ライセンス文）、本物の教材での通し。5.236 の番号が既存で重複している（直さない）。（#1740 で解消。5.268）
- 実行結果（2026-10-22。Linux x86_64・Chromium）: `L2_SERVER=<miniserve 0.35.0 の linux musl 版。sha256 は固定値と一致> make test-l2-run` で前提の 2 件と 3 回の完走が 3/3 通った（各回 34 項目・要求 6 件・約 2.4 秒）。`l2-journey-calibrate` の 2 件（54 項目）が通った。`scripts/expected-test-counts.txt` は更新しない。

### 5.240 章の飛ばし・選択を出さないことの観察（TASK-528。SHELL-11・D-73）

決定 D-73（2026-10-22）は、章を飛ばす・選ぶ操作を受講者の画面に出さないと決めた。章をまたぐ操作は SHELL-10 の「次の章へ進む」の 1 つだけで、特定の章から始める手段は直接リンク `?start=`（DLV-9。5.199・5.204 が証拠）である。本件は製品のコードを変えず、この観察を native とブラウザで固定する。

確かめること:

- 初期・各章の途中・各章の完了の直後・最後の章の後の全時点で、章を選ぶ部品（`select`・`option`、`role` が `listbox`・`menu`・`menuitem`・`option`・`combobox`）・開始点へのリンク・「移動」ボタンが 0 件である。OS を選ぶ欄（TERM-38）は章の選択ではないので除く
- 次の章へ進むの個数は 0 か 1 で、画面に現れる開始点の ID は提示中の 1 件（直後の章の先頭を指すもの）だけである。途中の Step を指す開始点・`#` つきの開始点・提示と関係のない開始点は出ない。飛ばさない
- 開始点が 6 件・10 件あっても同じである

| 試験 | 内容 |
|---|---|
| `crates/web/tests/next_chapter.rs` の末尾の 3 件（章の飛ばし_選択の部品を出さず…、開始点が章の数より多くても…、検出器の対照…） | CH-0 から CH-3 を通し、毎手で上の不変条件を確かめる。10 件の並びは `test_fixture::CH03_LINK_POINTS` と揃えて保つ |
| `tools/browser/chapter-select-absent.test.mjs`（CSP あり・なしで 6 件） | `?fixture=ch0-3-chapters` の実クリックの通しの全 Step と境目、`?fixture=ch0-3-links` の初期と途中の Step で検出器が 0 件。DOM へ足した部品を数える検出器の対照つき |

変異: `SHELL11-1`（次の章を飛ばして 2 つ先の章の入口を出す）を `crates/web/tests/mutations/` に足した。2026-10-22 に `scripts/check-mutations.sh` で DETECTED。

実行結果（2026-10-22）: native は `next_chapter` 20 件が通った。ブラウザは `SKIP_WASM_OPT=1` の試験の設定のビルドで `chapter-select-absent` 6 件が通った。全件の `make test-browser` は CI の `browser-test` に任せる（ローカルでは未実行）。

限界:

- 配信する設定は実行時に IR を取得しないので、配信する設定で章の境目まで進める試験は持たない（配信物に試験用の口が 0 件であることは SHELL-35・TASK-547 の担当）
- 代わりの手段（直接リンク）の証拠は 5.199・5.204 を引くだけで、本件では試さない

範囲外: spec の SHELL-11 の「証拠の種類」の書き戻し、開始点を選ぶ操作（SHELL-34）・講師の追いつき（SHELL-33。どちらも TASK-515）、`Control::Begin`（TASK-217）。`scripts/expected-test-counts.txt` は版上げ PR の関門の期待値で、本変更では更新しない。

### 5.241 「模擬」の常時表示（TASK-523。SHELL-6・D-72）

- 目的: 画面の枠の上部（`shell-top`）に「模擬」（`shell-mock`）を常に置き、どの Step・幅・通知の有無でも消えず、アクセシビリティツリーにも出ることを確かめる。`SHELL_CONTRACT_VERSION` は 2 のまま変えない（契約の追加ではなく、画面への結線）。
- 期待と定義の対応:

| 要件 | 決定 | 実装・試験 |
|---|---|---|
| SHELL-6 | D-72 | `shell_frame::top_bar`（定数 `MOCK_LABEL` を `text()` で描く）を `host::EngineHost::view` の root の先頭に置く。`check_complete_v2` が必須にする |

- 置き方: `role`・`aria-label` は付けない（a11y 契約の閉じた一覧を増やさない）。tabpanel の外・root の先頭の子なので 960px 未満のタブの切り替えで隠れない。静的なノードで要素の同一性（FEND-3）を変えない。TASK-524 は `top_bar` の `rest` に教材名・位置を渡して足す。
- 試験:
  - native（`crates/web/tests/shell_frame_host.rs`）: 画面の照合を `check_complete_v2` へ移し、`mock_label_is_always_shown`・`mock_label_stays_for_every_selected_pane` を追加。`ch0_3_run_host`（20 Step の各手の後）・`next_chapter`・`refused_history`・`history_ops`・`version_notice` も `check_complete_v2` で照合する。
  - ブラウザ（`tools/browser/shell-mock.test.mjs`）: 20 Step の通しと章の移動（CSP あり・なし）、幅 360px・959px・1280px のタブの切り替え、通知の表示中・閉じた後、再読み込みの後、配信する設定。判定は個数 1・文字・可視・tabpanel の外・`shell-top` の中・アクセシビリティツリー（`ariaSnapshot`）。対照として `display: none` を付けると可視とアクセシビリティツリーの判定が不合格になる。
- 変異注入（2026-10-22）: SHELL6-2（識別子が外れる）・SHELL6-3（root の子から外れる）を追加。SHELL6-1・SHELL6-2・SHELL6-3・SHELL1-1・SHELL1-2 が DETECTED。他の変異は未実行。
- 限界:
  - spec の検証方法は「CH-0〜CH-3 の全 22 Step」だが、試験用の教材 `ch0_3` は 20 Step（本物の CH-0〜CH-3 は TASK-642）。20 Step で確かめた。
  - 「模擬」は教材に依存しない静的なノードで構造上すべての画面に出るが、ブラウザでの証拠は CH-0〜CH-3 の形の教材だけである。
  - 教材を読めない画面（`ir-notice` だけの画面）には置かない。教材も枠も描かない画面で、`ir-refusal` の試験がこの構造を固定している。
  - `.sh-mock` は `display: inline-block` を持つため、`hidden` 属性では隠れない（対照は `display: none` で行う）。
- 実行結果（2026-10-22）: `make lint`・`make test`・`make check-deps`・`make check-dom-reads`・`make test-browser`（899 件）が通った。

### 5.241b L2 の待ち受けアドレスと起動の時間（TASK-113。DLV-14）

- 目的: L2 の `start.sh` が待ち受けるアドレスが `127.0.0.1` だけで、同じネットワークの他のホストから届かないことを試験で固定する。仕様（DLV-14）は「他のホストからの接続の可否は確かめていない」としていた。あわせて、起動（`spawn` から最初の 200 まで）とマウント（`goto` から `html[data-mounted="true"]` まで）の時間を記録に残す（機械の時間。人の時間ではない）。
- 試験の構成: `tools/browser/l2-run.test.mjs`（`make test-l2-run`。5.239 の 3 回の完走の中と後）に足した。`start.sh` の動作は変えていない（引数・終了コード・表示の文言は `make test-build-l2` が固定している）。
  - 各回: (1) `start.sh` の出力に `http://127.0.0.1:<port>/` があり、出力中の URL の host が `127.0.0.1` だけ。(2) 当のポートの LISTEN のソケットの集合が `127.0.0.1` だけ（Linux は `/proc/net/tcp`・`tcp6` の状態 `0A`、macOS は `/usr/sbin/lsof -nP -iTCP:<port> -sTCP:LISTEN`。ほかの OS は skip にせず失敗）。(3) 自分の非 loopback の IPv4（1 件以上を必須）と Linux の `127.0.0.2` への `net.connect` が `ECONNREFUSED` になる（時間切れ・到達不能は拒否と区別して失敗）。(4) `127.0.0.1` へは接続できる。(5) 起動とマウントの時間が有限の正の数。値は閾値で判定せず、各回の行と 3 回の中央値の行を出す。
  - 対照: 復号の対照（合成の行で `0.0.0.0`・`::`・`::1` を復号し、LISTEN 以外・別のポートを除く）。対照 A は `0.0.0.0` で待ち受ける別のサーバを同じ判定が検出し、非 loopback の IPv4 から接続できること。対照 B は展開先の写しの `start.sh` から `-i 127.0.0.1` をちょうど 1 か所外すと、`127.0.0.1` 以外で待ち受け、非 loopback の IPv4 から接続できること（`-i` が効いていることの確認。zip・repo のファイルは変えない）。
- 限界: 物理的に別のホストからの接続ではなく、同じ機械から自分の非 loopback のアドレスへ接続する模擬である。カーネルは `127.0.0.1` に bind したソケットへ非 loopback のアドレス宛ての接続を渡さないため、他のホストからの拒否と同じ判定になる（対照 A・B が「届くなら接続できる」ことを示す）。起動の時間の分解能は待ち間隔の 10 ms 程度。PoC-10 の中央値（起動 110〜152 ms・マウント 91〜110 ms）とは機械・ビルド・miniserve の版が違うため比べない（`docs/measurement.md` の規則）。2 つの時間は `make measure` の束には入れていない。macOS（`lsof`）の経路は Linux では流れず未検証（実機は TASK-118・119）。
- 04 の差分の案（spec repo 側の作業。ここでは編集しない）: DLV-14 の「証拠の種類」を、「待ち受けが `127.0.0.1` だけであること・同じ機械から自分の非 loopback のアドレスへの接続が拒否されることを自動試験で確かめた。物理的に別のホストからの確認は行っていない」に更新する。
- 範囲外: 物理的に別のホストからの接続、実機（Apple シリコン）での起動・時間（TASK-118・119）、案内文（TASK-117）、L1 と L2 の一致（TASK-114・115）、ライセンス文（TASK-137）。5.236・5.240 の番号の重複（既存）は直さない。（#1740 で解消。5.268）
- 実行結果（2026-10-22。Linux x86_64・Chromium・miniserve 0.35.0 musl。sha256 は固定値と一致）: `make test-l2-run` で 9 件が通った（各回 41 項目。非 loopback の IPv4 は 1 件・他のアドレスへの接続 2 件がすべて拒否）。起動の中央値 265 ms（364/152/265）・マウントの中央値 284 ms（284/253/337）。`scripts/expected-test-counts.txt` は更新しない（`make test-browser` の系統に入らない）。

### 5.242 上部の教材名・章の ID・Step の位置の表示（TASK-524。SHELL-8・D-72）

- 目的: 上部（`shell-top`）に教材名（`shell-title`）と「<章の ID>　Step M / K」（`shell-position`）を文字で出し、Step の ID は出さない。Step を進めると M が 1 増え、章が変わると章の ID と K が変わる。再読み込み・書き出し→読み込みの後も復元した Step の位置が出る。`SHELL_CONTRACT_VERSION` は 2 のまま（契約は変えず、画面への結線）。
- 期待と定義の対応:

| 要件 | 決定 | 実装・試験 |
|---|---|---|
| SHELL-8 | D-72 | `top_position::top_position`（IR と現在の uid から章の ID・M・K を導く）を `host::EngineHost::top_place` が使い、`shell_frame::top_place_list`（registry の外の keyed list。field は `top_place`）を `top_bar` の `rest` に渡す |

- 置き方: 項目は教材名（キー `title:<pack.id>`）と位置（キー `position:<章>:<M>:<K>`）の 0〜2 件。親の `div.sh-place` は常に描き、Step が変わると位置の項目だけが差し替わる（要素の同一性。FEND-3）。`update` の前後で値を比べて dirty に積み、`install_recorded`（履歴を開く・教材の差し替え）でも積む。値は文字ノードにだけ入れる。位置を導けないとき（教材なし・現在の Step なし・caller をたどれない・循環）は位置を出さない（`check_position` の違反を作らない）。モジュールの展開した Step は `Instance::caller` をたどって呼び出した章の Step に読み替える（上限は実体の数 + 1 回）。
- 文言の細目（D-72 が本タスクに任せた点）: 章の ID と位置は全角空白 1 つで区切り、「Step」と `/` の前後は半角空白（例: `ch-1　Step 3 / 6`）。表示だけのモードでも復元した Step の位置を出す。
- 試験:
  - native: `top_position` の単体 3 件、`crates/web/tests/top_position_host.rs`（全 20 Step の文字・属性を IR から独自に数えた期待値と照合・教材なしの画面・位置を変えない更新で dirty にしない・履歴を開くと dirty と復元した位置・モジュールの Step の読み替え）、`ch0_3_run_host`（各手の後で、位置が変わったら `top_place` が dirty）。
  - ブラウザ（`tools/browser/shell-position.test.mjs`）: 20 Step の通しで教材名・位置・属性を IR から独自に数えた期待値と照合し、CH-1 の M が 1〜6、K が 6、章ごとに章の ID と K が変わる（CSP あり・なし）。再読み込みの後と、書き出し→別の文脈での読み込みの後に `ch-1　Step 4 / 6`。幅 360px・959px で見えて横スクロールなし。配信する設定（IR なし）では位置が出ず枠が崩れない。対照として位置を `display: none` にすると判定が不合格になる。
- 変異注入（2026-10-22）: SHELL8-2（M が 0 始まり）・SHELL8-3（caller をたどらない）・SHELL8-4（dirty を積まない）を追加。この 3 件と SHELL6-2 が DETECTED。他の変異は未実行。
- 限界:
  - 章の ID は IR の `unit`（教材の ID の綴り）をそのまま出す。spec の例の `CH-1` と大小が違う（`docs/known-gaps.md`）。
  - 分岐のある章では、分岐の Step も K に数えるので M が 2 以上飛ぶことがある。`ir.nodes` は章の定義順に並ぶ（`crates/pack/src/expand.rs` の push の順）。
  - 確かめたのは試験用の教材 `ch0_3`（20 Step）と、モジュールを含む試験データの導出だけで、本物の CH-1 は教材 repo の担当（TASK-642）。
  - 配信する設定のブラウザ試験は、IR を同梱しない組み立てなので教材ありの表示を確かめない。
  - Step の題・本文は TASK-525、上部の区画の配置の最終の決定は TASK-534・TASK-548 の担当。
- 実行結果（2026-10-22）: `cargo test`（web の新しい試験）・`tools/browser/shell-position.test.mjs`・`element-identity`・`next-chapter`・`shell-mock`・`shell-widths`・`csp-runs` が通った。全体の `make` 系は本節の末尾の追記を参照（未実施の項目はそこに書く）。

### 5.243 L1 と L2 の 3 配信先 × 3 回の一致（TASK-114 1/2。DLV-15・D-58）

- 目的: 配信の経路（サーバの実装・base のパス・オリジン）が、教材の挙動と書き出しに影響しないことを自動ブラウザの試験で固定する。仕様（DLV-15）は、L1 と L2 の同じ入力列を 3 配信先 × 3 回の計 9 回流し、操作ごとの題と状態・最後の画面の状態 `snapshot.json`・書き出した履歴 `history.json` がバイト単位で一致すること（状態の差 0）を求める。PoC-10 では成立していたが、エンジン repo には試験が無かった。
- 3 配信先の読み替え: 決定 D-58（2026-10-12）は L2 の案 (i)（dist-server 系）を採らないと決めたため、PoC-10 の「L1・案 (i)・案 (ii)」は使えない。9 回（3 × 3）の形を保ち、repo にある経路で互いに違う 3 つにした。

  | 名前 | サーバ | base | 起動 |
  |---|---|---|---|
  | `L1-a` | `tools/serve.mjs`（gzip つき） | `/fandhe-workshop/` | 回ごとに新しいポート |
  | `L1-b` | 同上 | `/another-repo-name/`（DLV-2 の前例） | 回ごとに新しいポート |
  | `L2` | 展開した zip の `./start.sh`（miniserve 0.35.0。`PATH=/usr/bin:/bin`） | `/` | 回ごとに新しいポート・新しい展開先 |

- 試験の構成: `tools/browser/l1-l2-match.test.mjs`（`make test-l2-run` の 2 つ目の `node --test`。`make test-browser` には入れない）。共有の部品は、起動の手順を移した `l2-server.mjs`（`l2-run.test.mjs` も import する。挙動は変えていない）と、画面の直列化の `screen-snapshot.mjs`。誘導は 5.239 の `l2-journey.mjs`（DOM だけの観察・`mouseDriver`）を共有し、操作の列を二重に書いていない。`ch0-3-journey.mjs` は変えていない。
  - 前提の照合: 流す前に、zip を展開した `site/` と配信する木（`scripts/test-l2-run.sh` が `FANDHE_L2_SITE` で渡す）が、同じファイルの集合・同じ SHA-256 であることを確かめる（7 ファイル）。差がビルドの差でなく配信の差であることの保証。
  - 観察 (a): `journey` の `onStep` が呼ばれるたびに、Step の ID・状態・`step-status` の語・`run-status` の `data-state`・ログの行数と最後の行の `data-step`・`data-kind`・入力の口の個数を積む（18 件）。誘導の台帳の項目名と合否も比べる。
  - snapshot (b): 画面の根 `#app` の複製の HTML と、HTML に現れない値（下書き・OS の選択・フォーカス）を、欄の順が固定の JSON にする。書き出しのリンク（`export-download`）の blob: を値に持つ `href`・`data-export-url`・`data-key` だけを複製の上で取り除き、他の属性の値は現在の配信元とサブパスの部分だけを `<base>/` に置き換えてパスの違いを残す。直列化の後に `127.0.0.1`・`localhost`・`blob:`・http の URL が残っていれば、9 回の一致が壊れる前に理由つきで失敗にする。snapshot は書き出しの操作の前に取る。
  - history (c): 試験用の口を使わず、`shell-export` を押して現れた `export-download` のクリックを Playwright の `Download` で受けて読む。JSON として読め、`ops` が 1 件以上あることを確かめる。
  - 各回の計数（書き出し操作の後に評価する）: ページ内エラー・CSP の違反・コンソールの error と warning・2xx 以外の応答・配信元以外への要求・ダイアログ・ポップアップ・クラッシュがどれも 0 件で、同梱の IR が 200 で 1 回取得される（5.239 と同じ）。
  - 比較: 9 回の観察・snapshot・history のハッシュがそれぞれ 1 種類。不一致のときは配信先と回の名前・最初に違った欄のパス・ハッシュの先頭 12 桁だけを出す（値は出さない）。
- 対照: (1) 照合は、展開した写しの `index.html` に 1 バイト足すと「内容の違い 1 件」、余分なファイルを足すと「片方だけ 1 件」を検出する（zip・配信する木は変えない）。(2) 終点の手前（`main/st-1-4`。`journey` が止まれる Step は他に `main/st-2-1` だけ）で止めた回は、観察・snapshot・history の 3 つとも 9 回のハッシュと違う（直列化が空を返すような壊れ方でも 9 回の一致が通ってしまうのを防ぐ）。(3) 直列化は、書き出し後の画面で blob の `href`・`data-export-url`・`data-key` を替えても変わらず（リンクの要素は snapshot に残る）、ログの 1 行の文字を書き換えると変わる。(4) 自己検査は、オリジン依存の文字列が残った直列化を拒む。
- 限界: 教材は試験データ `ch0_3`（20 Step のうち CH-1 の終点 `main/st-2-1` まで）で、本物の CH-0〜CH-1 の 50 操作ではない。Step の題は画面に描かれていない（`shell-title` は未結線。TASK-524）ため、題の代わりに IR の並びから導いた Step の ID を使った。拒否・問い返しの直後は観察を積まず、台帳の「進まない」の合否で比べる（`journey` に口を足さなかった）。L2 は Linux の musl 版 miniserve での模擬で、Apple シリコンの実機は TASK-118・119。Chromium だけ。
- 04 の差分の案（spec repo 側の作業。ここでは編集しない。自動運転のため人の承認は得ていない）: DLV-15 の「L1 と L2 の 2 案で 3 配信先」を「L1（手元のサブパス 2 名）と L2（D-58 の案 (ii)）の 3 配信先」に、「証拠の種類」をエンジン repo の自動ブラウザの試験（`l1-l2-match`）に更新する。
- 範囲外: 履歴の `ops` に揺れる欄が無いことの検査と、教材が変わっても再生できる固定の入力列とその比較（TASK-114 2/2。5.245b で実施）、実機（TASK-118・119）。5.236・5.240・5.241 の番号の重複（既存）は直さない。（#1740 で解消。5.268）
- 実行結果（2026-10-22。Linux x86_64・Chromium・miniserve 0.35.0 musl。sha256 は固定値と一致）: `l1-l2-match` の 15 件が 2 回続けて通った（9 回すべてで観察 18 件・要求 6 件。ハッシュの先頭 12 桁は観察 `d5649c055eab`・snapshot `40bdf1b8e53f`・history `3d2e6001baef` の各 1 種類。所要 約 26 秒）。試作の段階で `L1-b` の 3 回目が 1 度だけ「配信元以外への要求または失敗した要求あり」で失敗したが、原因の特定前に診断の出力を足した後は再現していない（未解明。再発すれば出力に件数と、配信元以外・失敗した要求の path（query は除く）と失敗の種類が出る）。`scripts/expected-test-counts.txt` は更新しない（`make test-browser` の系統に入らない）。

### 5.244 別のオリジンでの読み込みと続きの操作（TASK-531 2/2。SHELL-15）

- 目的: 受講の途中で書き出した履歴を、別のオリジン・別の context（名前の違うサブパス配信とルート配信）で読み込むと、同じ Step・世界状態・復習・ログ・チェックポイントの一覧・画面になり、続きを操作でき、同じ入力列の書き出しがバイト単位で一致することを確かめる。1/2（5.234）の `history-ops` は足場で、包みは契約の試料だった。本件は journey の画面の操作で実際に受講した記録（ホストの `OpsRecorder` が記録した `ops`）を使う。`history-import`（TASK-330 2/2）は両オリジンが同じ base 名で、書き出しは試験用の口だった
- 設計: `tools/browser/history-continue.test.mjs`（試験の設定。`?fixture=ch0-3`）。書き出しは `shell-export` → `Download`、読み込みは `shell-import` の input への `setInputFiles` だけで行う。受講は `journey` で CH-0・CH-1 を通って `main/st-2-1` まで進める。チェックポイントは画面の口が無いため、`main/st-0-2b`・`main/st-1-4` に入った直後に `raw-trigger` へ `{control:'save'}` を送って 2 つ作る（本試験で唯一の試験用の口の使用）
- 比べるもの: 状態の層（`current`・`generation`・`runStatus`・`logLen`・`log`・`stateWithoutLog`（世界状態・チェックポイント・下書き）・`review`・`artifacts`・`worldMatchesState`・`incrementalMatches`）と、画面の層（`shell-title`・`shell-position`・ログの各行の `data-step`・`data-kind`・成果物の行・`step-status`・`run-status`・入力の口の数）。`lastImport`（読み込みの結果）と `installCount`（エンジンの組み込みの回数の観察。読み込みで 1 増える。状態ではない）は比較から除く。書き出しのリンクの `blob:` はオリジンごとに違うため画面の比較に入れない
- 試験（3 件）:

| 試験 | 内容 |
|---|---|
| 別のオリジン（サブパス 2 名）で同じ状態になり続きを操作できる | A で受講して書き出す → B は別の状態から始まることを確かめて読み込む → 状態・画面が一致し通知 0 件 → B で書き出し直すとバイト列が A と同一 → 両側で CH-2 の先頭の 4 手を同じ順で操作 → 状態・画面が一致 → 書き出しがバイト単位で一致し、続きの前のファイルとは違う（負の対照） |
| サブパスとルート配信で独立に受講しても書き出しが一致する | A（サブパス）と R（ルート）で同じ入力列を操作し、書き出しのバイト列とファイル名が一致 |
| サブパスで書き出してルート配信で読み込む | 状態が一致し、書き出し直したバイト列が元と同一 |

- 診断: 不一致のときは、長さ・`ops` の件数・最初に違う op の位置と種類名だけを出す。履歴の中身（入力した文字列・下書き）は試験の出力・失敗の文に出さない。外部への要求・CSP 違反・ページのエラー・ダイアログ・ポップアップが両オリジンで 0 件
- 実行結果（2026-10-22。Linux・Chromium。試験の設定のビルドで `FANDHE_WEB_TEST_DIR` だけ指定）: 3 件が 4 回続けて通った（揺れなし）。Rust は変更なし。`make test-browser` の全件は未実行（CI の `browser-test` に任せる）。`scripts/expected-test-counts.txt` は更新しない
- 限界: チェックポイントの一覧の画面での確認は TASK-529 2/2・TASK-548。配信する設定は教材を持たないため、教材を動かす確認は試験の設定で行う。L2 の配布物は TASK-114、macOS のファイル画面と公開サイトは TASK-342。Chromium だけ
- 範囲外: 未送信の下書きの案内（TASK-537）、読めない履歴の理由（TASK-532）、通知欄・枠の最終の配置（TASK-534・TASK-548）

### 5.245 処理中の再読み込みの告知の画面（TASK-536。SHELL-21・SAVE-6・D-57）

- 目的: 処理中の再読み込みの告知は、D-57 の決定どおり右のログの `reloaded` 行だけが運び、枠が二重に知らせないことを固定する。枠の契約の版 2（TASK-516 2/2）は再読み込みを枠の通知の種類にしない。本件は画面側の確認と記録で、本番コード（`crates/web/src`）は変えていない。
- 読み上げの定義: 「読み上げが 1 回だけ届く」を、次の観察で表す。(1) 「再読み込みしました」の文字が DOM に現れる場所は右のログの `reloaded` 行だけで、行の数は再読み込みの回数と同じ。(2) `role="status"`・`role="alert"`・`aria-live`（`off` を除く）の欄の中に、この文字の変化が 0 件（ページ内の `MutationObserver` で全件を記録）。枠の通知欄 `shell-notice` の項目・`data-notice-kind` は 0 件。(3) 行の種類 `reloaded` は応答の読み上げの単位（A11Y-6）に数える側にある（native 試験）。
- 設計: `tools/browser/reload-notice.test.mjs` の N-1（1 回）・N-2（2 回）に、完走の前と後の 2 点で上の (1)(2) の計数を足した。native 試験は `terminal_area_contract` の `consistent_with_a11y` に `reloaded` の assert を足した。変異は `SHELL21-1`（`LIVE_EXCLUDED_LINE_KINDS` に `reloaded` を足す）。
- 実行結果（2026-10-22。Linux・Chromium。試験の設定と配信設定のビルド）: `reload-notice` の 3 件が通った。`SHELL21-1` は DETECTED。`make test-browser` の全件は未実行（CI の `browser-test` に任せる）。`scripts/expected-test-counts.txt` は更新しない。
- 限界: 再読み込みはページ内の模擬の合図（5.74 と同じ）で、実際の `page.reload()` で処理中の継続を確かめる場面は未検証（保存の書き出しの段と復元後の進行の再開の確認が要る。TASK-319・TASK-320 の領域）。ライブの欄（`live-response`）は実画面にまだ描かれておらず、実 DOM の更新回数の観察は TASK-370、支援技術での読まれ方は TASK-381。本件の観察は読み上げそのものの証拠ではない。
- 範囲外: 行の言い回しの確定・通知の文面（TASK-549）。5.236・5.241 の番号の重複（既存）は直さない。（#1740 で解消。5.268）

### 5.245b 固定の入力列での比較と履歴の `ops` の揺れる欄の排除（TASK-114 2/2。DLV-15・D-58）

- 目的: DLV-15 の残り 2 点を自動ブラウザの試験で固定する。(1) 書き出した履歴の `ops` に、時刻など実行のたびに変わる欄が無いこと。(2) 事前に固定した入力列での比較。5.243 の入力列はコード（`ch0-3-journey.mjs` の `journey`）で、教材 `ch0_3` やコードが変わると比べる列も一緒に変わる。PoC-10 では、凍結した 50 操作の列が本番の教材に結びついていたため、教材の修正の束の後に再生できなくなり、「固定の列での比較は未実施」として残った。この穴を埋める。
- 「教材が変わっても再生できる」の読み（採用）: 固定の列は、凍結した IR の写しと組にし、IR の SHA-256 を列に書いて固定する。比較の実行では、配信する木の `ir/main.json` を凍結した写しへ差し替えて配る。`ch0_3` や本物の教材が後で変わっても同じ IR で再生でき、再生できなくなることが無い。配信される IR が写しと違うときは、途中で時間切れになる前に `ir-mismatch` の理由で最初に断る（fail-closed）。
  - 採らなかった案: (a) 教材に依らない汎用の列（選択肢の ID・行のパスが教材に依るため成り立たない）、(b) `ch0_3` の IR に SHA-256 だけで結びつける列（`ch0_3` が変わると PoC-10 と同じく再生できなくなる）、(c) 手で書いた JSON の列（`journey` と二重定義になる）。
  - 列は手で書かず、`journey` を記録用の包み（`recordingDriver`・`recordingObserver`）に通して導き、一度だけ凍結した。`journey` や `ch0_3` が後で変わっても追随させない（追随させないことが「固定」の意味）。
- 置き場: `crates/web/tests/data/fixed_sequence_ch0_1/`（`ir.json` は凍結時点の `ch0_3/ir.json` のバイト写し・`sequence.json`・`README.md`）。列の項目は 3 種（操作 `op`・確認 `expect`（Step・状態・口の数）・拒否と問い返しの後も進まない `still`）と、凍結時の `ops`（36 件）・`content_hash`・IR の SHA-256。画面の語は列に入れない（文面の変更で固定の列が壊れないため）。語は 9 回の相互の一致でだけ比べる。
- 試験の構成: `tools/browser/l1-l2-fixed.test.mjs`（`make test-l2-run` の 3 つ目の `node --test`。`make test-browser` には入れない）。部品は `fixed-sequence.mjs`（記録・読み込み・再生・`ops` の検出器）と、5.243 の試験から移した `l1-l2-runner.mjs`（配信先の起動・1 回の実行・木の照合。5.243 の試験は挙動を変えずに import に替えた）。`l2-server.mjs` の `startL2` に、展開した写しのファイルだけを起動の前に差し替える `overlay` を足した（zip・`dist/` は変えない。パスは `..` と絶対パスを断る）。
  - 前提: IR を差し替えた L1 の木と L2 の木が、同じファイルの集合で同じ SHA-256（7 ファイル）。
  - 固定の列を 3 配信先 × 3 回（9 回）流し、各回で (a) 各 `expect` の口の数、(b) 書き出した `ops` が凍結した `expected_ops` とバイト単位で一致、(c) `history.versions.content_hash` が凍結した値と一致（凍結した IR が実際に使われた証拠）、5.243 と同じ計数（ページ内エラー・CSP・コンソール・2xx 以外・配信元以外への要求・ダイアログ・ポップアップ・クラッシュ）が 0 件であることを求める。9 回の相互では、観察の列・`snapshot.json`・`history.json` がそれぞれ 1 種類。
  - 再生器は、`journey` の台帳でしか見ていなかった待ち（問い返しの後に下書きが空になる・拒否の後に下書きが全体の選択状態になる。JUDGE-27・TERM-22）を、教材に依らないエンジンの一般則として持つ。
- 履歴の `ops` の揺れる欄（R1）:
  - 構造の検査 `volatileOps`: `ops` を歩き、鍵が許可リスト（core の `Op`・`Input`・`Control`・`TickAddr` の serde の綴りと `testdata/contract/history/*.json` の綴り）にあること、数が 0 以上 2^31 未満の整数であること、文字列が ISO 8601 の日時の形でないことを求める。検出したものはパスだけを返す。凍結した期待・固定の列の 9 回・5.243 の 9 回のすべてに当てる。
  - 時計と乱数をずらした実行: `L1-a` で 2 回、`page.clock.setFixedTime` で 2001 年と 2088 年に固定し、`Math.random` を回ごとに違う種の決まった列に差し替えて流し、`history.json` が 9 回とバイト単位で一致することを求める。`setFixedTime` はタイマーを止めない方式で、遅延保存などの本来の挙動は崩れなかったため、`Date.now` をずらす方式への切り替えは不要だった。自己確認として、同じ差し替えの page で `Date.now()` が固定した年を返すことを確かめる。
  - 現れた `ops` の種類は、ログに件数だけを出す（`input`・`draft`・`submit`・`tick`）。
- 対照（空振りの防止）: (1) 配信する木の IR が別のバイト列でも、凍結した IR を差し替えれば固定の列を再生でき、(a)〜(c) が通る。(2) 差し替えなしで別の IR（末尾に改行を足したもの・JSON として読めない前半だけのもの）に当たると、page を開く前に `ir-mismatch` で断る（開いた browser context が 0 件。照合は `runOnce` の `precheck` で遷移・マウント待ちより前に行い、マウントの失敗・時間切れが先に来ない）。(3) 期待の `submit` を 1 つ変えると、比較は `$.<index>.submit` の形のパスを報告して不一致になる。(4) 検出器は、合成した `ops` の時刻の欄・時刻らしい値・未知の鍵・範囲外の数のそれぞれのパスを返し、正常な `ops` は返さない。(5) IR の取得の計数（`l1-l2-runner.mjs` の `irFetchCheck`。5.243 の試験と共有）は、IR の 200 の応答がちょうど 1 件であることを求め、失敗から除くのはその 1 件の要求自体が応答の後に `net::ERR_ABORTED` と通知される競合（CI の Chromium で完了の通知の代わりに届くことがあった）だけにする。合成の記録で、id の違う 200 の重複の取得とその中断・応答のない別の要求の中断・別のエラー種別・別の URL・200 以外の応答の中断は数えることを確かめる。
- 凍結の手順（一度だけ。CI では流さない）: `FANDHE_FREEZE_SEQUENCE=<crates/web/tests/data/ の下の既存のディレクトリ> make test-l2-run` を流すと、凍結の試験だけが動き、列と IR の写しを書き出し、書き出した後は必ず失敗で終わる（凍結の実行が合格に数えられないため）。既存の列は書き換えず、作り直すときは版つきの新しいディレクトリを足す。
- 限界: (1) 実行で現れなかった `ops` の種類（`reload`・`control` など）は、鍵の許可リストで型の上からしか保証できない（鍵の名前を型から列挙する native の試験は別件）。(2) 教材は試験データ `ch0_3`（CH-1 の終点 `main/st-2-1` まで）で、本物の CH-0〜CH-1 の 50 操作ではない（TASK-642）。(3) 再生器は、列が `journey` の止まれる Step までであることに依る。(4) 時計は `Date` だけで、`performance.now` は動かさない。Chromium・Linux の miniserve だけ。
- 04 の差分の案（spec repo 側の作業。ここでは編集しない。自動運転のため人の承認は得ていない）: DLV-15 の「証拠の種類」の「固定の列での比較は未実施」を「エンジン repo の `l1-l2-fixed` で実施（凍結した IR と組の列）」に更新する。
- 範囲外: オリジンの分離（TASK-115）、読み込んだ `ops` を適用し直した結果と写しの一致（TASK-174・TASK-310・TASK-315）、本物の教材での CH-0〜CH-1（TASK-642）、実機（TASK-118・119）。5.243 の観察に `shell-position` の文字を足すこと（5.243 のハッシュが変わるため別件）。
- 実行結果（2026-10-22。Linux x86_64・Chromium・miniserve 0.35.0 musl。sha256 は固定値と一致）: `l1-l2-fixed` の 17 件（PR のレビューで (2) に JSON として読めない IR の対照と (5) を足した後は 19 件）と `l1-l2-match` の 15 件が通った（固定の列は観察 22 件・要求 6 件。ハッシュの先頭 12 桁は観察 `4bc3804f4905`・snapshot `7bc8fe78b1ea`・history `3d2e6001baef` の各 1 種類。時計と乱数をずらした 2 回も history は同じ）。5.243 の 9 回の snapshot は、TASK-524 の画面の変更で 5.243 の記録（`40bdf1b8e53f`）から `7bc8fe78b1ea` に変わっており、history は同じ。`scripts/expected-test-counts.txt` は更新しない（`make test-browser` の系統に入らない）。

### 5.246 Chrome 以外のブラウザへの案内（TASK-544。SHELL-30・D-74）

- 目的: Chrome 以外（Edge・Safari・Firefox ほか）と旧版の Chrome で開いたときに、締め出さずに枠の通知欄へ案内を 1 件だけ出し、閉じられ、Chrome の最新版では出さないことを固定する。D-74 が TASK-544 に委ねた 2 点を決めた。
- 決定（旧版の基準）: Chrome の主版が 152 未満。試験で確かめた Chromium の主版（`docs/toolchain.md`。153）から 1 引いた値で、段階配布の最新の安定版に誤って出さない余裕である。実時間の最新版は、外部への通信を CSP と SAFE が禁じるため知れない。`crates/web/tests/browser_notice_pin.rs` が `docs/toolchain.md` の主版と `MIN_CHROME_MAJOR + 1` の一致を確かめ、版の固定を上げると落ちて見直しを促す。
- 決定（判定）: 括弧の中を除いた製品トークンがすべて許可リスト（Mozilla・AppleWebKit・Chrome・HeadlessChrome・Safari・Mobile）に入り、Chrome の主版がすべて基準以上のときだけ出さない。`HeadlessChrome` は試験の Chromium の既定の UA のため Chrome に数える。判定できないものは出す側に倒す（案内は閉じられる非破壊の表示）。全域の純関数で、UA は保持・出力・記録しない。
- 決定（一度だけ）: ページの中では起動時に 1 件で、再描画・操作で増えず、閉じると 0 件。ページをまたいでは、閉じた記録（localStorage の `fandhe-workshop/meta/browser-notice` = `dismissed`）が同じオリジンにあれば出さない。閉じずに再読み込みすれば再び出る。鍵は履歴の鍵の空間の外で、履歴の一覧・削除の対象にならない。保存領域が読めなければ出し、書けなければそのページでは閉じたままにする。
- 試験: native は `crates/web/src/browser_notice.rs`・`shell_notice.rs` の単体、`crates/web/tests/browser_notice.rs`（UA の表と壊れた入力）、`browser_notice_pin.rs`（基準と固定の版）、`browser_notice_host.rs`（表示・閉じる・記録・他の種類では閉じない・書けなくても閉じる）。ブラウザは `tools/browser/browser-notice.test.mjs`（19 件。CSP あり・なしで Edge・Safari・Firefox・旧版の Chrome に 1 件、Chrome 153 と既定の HeadlessChrome に 0 件、閉じる・再読み込み・新しい context、幅 360px、配信する設定）。変異は `SHELL30-2`（許可リストの検査）・`SHELL30-3`（基準のずれ）・`SHELL30-4`（閉じた記録の値）で、いずれも DETECTED。
- 実行結果（2026-10-22。Linux・Chromium）: 上の native 試験とブラウザ試験 19 件が通った。`modifier-key-hint`・`element-identity`・`csp-runs` は変更なしで通った。`make test-browser` の全件は未実行（CI の `browser-test` に任せる）。`scripts/expected-test-counts.txt` は更新しない。
- 確かめないこと: 実機の Edge・Safari・Firefox での確認。UA を Playwright で差し替えた代理である。
- 限界・範囲外: `docs/known-gaps.md` の SHELL-30。IR の拒否の画面では通知欄を描かないため案内も出ない（TASK-545・TASK-546）。

### 5.247 案内レベル別の本文の選択（TASK-292。GUIDE-1）

- 目的: 現在の案内レベルに合う本文の木を選び、その水準の版が無ければ濃い側（C → B → A、B → A）へ倒れることを固定する。
- 設計: core の `body_level::select_body`・`body_doc_at` が `Node::body_doc`・`body_levels_doc` を契約の `GuidanceLevel::fallback_chain` の順に探す純関数。見つかった版の水準を `SelectedBody::level` で返し、倒れたかを区別できる。`body_levels_doc` の鍵 `A` と `A`・`B`・`C` 以外の鍵は読まず、木を持たない版 1 の Node は `None`（パス文字列は実行時に解釈しない）。`CONTRACT_VERSION` は上げない。
- 試験: `crates/core/tests/body_level_select.rs`（水準 3 × 版の有無 4 の 12 通りの表、`tests/data/body_level_1/ir.json` の目印の語による水準の定義への適合〔A は skill 名を含む、B は作業場所を含み skill 名を含まない、C はどちらも含まない〕と倒れた先、鍵 `A`・未知の鍵・木なし・版 1 の fail-closed、任意の鍵の組での全域性）。`crates/pack/tests/body_level_runtime.rs`（コンパイルした IR の各 Node × 各水準で、静的検査の `body_for` と実行時の `select_body` が同じ版を選ぶこと）。
- 変異（2026-10-23。元にしたコミット a8c2215。`scripts/check-mutations.sh crates/core/tests/mutations/GUIDE1-1.mutation crates/core/tests/mutations/GUIDE1-2.mutation`）:

| ID | 内容 | 結果 |
|---|---|---|
| GUIDE1-1 | `body_level.rs`: 倒れを外す（自分の水準だけを探す） | DETECTED（`twelve_combinations_pick_the_expected_body_and_fall_toward_the_dense_side`） |
| GUIDE1-2 | `body_level.rs`: A の本文を `body_levels_doc` の鍵 `A` から受ける | DETECTED（`fail_closed_on_odd_irs`） |

- 確かめないこと: 水準 B・C の本文が教育上適切か（教材側の INTRO-40・ROUND2-7）。
- 限界・範囲外: 画面への結線は SHELL-7 のタスク。compile が成功する教材では水準の木は欠けない（本文ファイルが無ければ `E-REF-BODY`、埋め込みの予算超えは `E-LIMIT`）ため、欠けの倒れは手で作った IR で確かめた。版 1 の IR は本文を出さない。ヒントの出し分けは TASK-293、開始点の水準と切り替えは TASK-294。

### 5.248 案内レベル別のヒントの出し分け（TASK-293。GUIDE-2）

- 目的: 現在の Step と案内レベルで出せるヒントの一覧（core の `hint_select`）が水準ごとに `levels` の宣言どおりに変わり、pack の `E-HINT-HIDDEN` が報告する (定義ファイル, Step, 水準) の集合と、実行時に一覧が空になる組の集合が、モジュールの実体を含めて一致することを固定する。規則の正は契約の `visible_hints` の 1 か所で、検査と実行時は違う入力（展開の前の Step と定義元の表 / 展開の後の node と IR の表）から引く。
- 試験: `crates/core/tests/hint_select.rs`（水準で一覧が変わる・Session の水準に従う・開始前と未知の Step は空・契約の規則と一致・性質テスト）、`crates/pack/tests/hint_hidden_runtime.rs`（base・D22・モジュールの実体・模擬の教材 8 組で一致）。base のヒントは `levels` なしで全水準に出し、D22 は `[B, C]` で水準 A だけが空になる。
- 実行結果（2026-10-23。元にしたコミット a8c2215）。コマンド: `scripts/check-mutations.sh crates/core/tests/mutations/GUIDE2-1.mutation crates/pack/tests/mutations/GUIDE2-2.mutation`

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| GUIDE2-1 | `crates/core/src/hint_select.rs`: 現在の案内レベルを無視して常に水準 A を返す | the_list_changes_with_the_level | the_list_changes_with_the_level, property_list_matches_independent_expectation | DETECTED | cases=256、固定の既定の種、最小の入力 `[Some([1])]` |
| GUIDE2-2 | `crates/pack/src/guidance.rs`: 検査が水準 C を見落とす | agrees_on_synthetic_packs | agrees_on_synthetic_packs | DETECTED | - |

- 範囲外: `Control::OpenHint`・`SetGuidance` の結線（TASK-295・TASK-294）、本文の表示（TASK-530）。

### 5.249 学習モジュールの実体ごとの状態の読み出し（TASK-233 1/2。MOD-1）

- 目的: 同じ定義の Step が、`Session` の状態の上でも実体ごとに別の Step の実体として扱われ、片方の状態・フラグがもう片方へ漏れないことを固定する。
- 決定: 状態の型・直列化の形は変えず、読み取り専用の `instance` モジュールと `Session::instances`・`slice`・`step_status`・`steps_of` を足した。根の決め方は `instance::root_of` の 1 か所に寄せ、中身は従来の `StepUid::root` のままにした。呼び出しの Step の持ち主の規則（`Instance.caller`）は書き込みと読み出しが一緒に変わる必要があるため入れず、TASK-235・TASK-241 が `root_of` を差し替える。`CONTRACT_VERSION` は据え置いた。
- 試験: `crates/core/tests/module_instances.rs`（手元で組んだ 2 実体の IR。列挙・Step の実体の分離・移動後の非干渉・フラグの非漏洩・不正な根と uid への全域性）、`crates/pack/tests/module_instances_session.rs`（`module-reuse-rounds` の IR。実体の列挙、round1 と round2 の Step の実体が同数で uid が全部違うこと、片方へ入っても他方が `NotStarted` のままで切り身の鍵が混ざらないこと）。
- 実行結果（2026-10-23。Linux）: 上の native 試験 8 件が通った。既存の試験は無改変で通る（最終の確認は PR の CI）。
- 範囲外: 呼び出しの Step の持ち主の規則（TASK-235・TASK-241）、web の残りの `StepUid::root` の直呼び（`next_chapter.rs`・`start_select.rs`・`step_status.rs`・`glue.rs` と試験の部品。core と web の `offers.rs` は `root_of` に寄せ済み）の置き換え、実体数・Step の実体数・開始数の照合（TASK-233 2/2）。

### 5.250 隠す語の検査の 4 種類の欠陥（TASK-300 1/2。GUIDE-4）

- 目的: 隠す語の検査（`E-GUIDANCE-LEAK`）とその周辺の検査が、PoC-3 の欠陥 D20〜D23 に相当する 4 種類を期待のコード・ファイル・項目で検出することを固定する。
- 試験: `crates/pack/tests/guide4_defects.rs`。D20〜D23 は `testdata/pack-defects/v1_1/b/` を写さずに読み、4/4 を確かめる（D22 は GUIDE-2 の `E-HINT-HIDDEN`。GUIDE-4 の証拠ではない）。補助の欠陥 G01〜G03（`testdata/pack-defects/guide4/b/`）が、`do`・開始点の水準だけ・`body_levels` だけの 3 経路を単独で確かめる（4 種類の外）。漏れの検査は `validate` だけで、`compile` は通ること、診断に隠す語を写さないことも確かめる。
- 変異（2026-10-23。元にしたコミット 8532507。`scripts/check-mutations.sh crates/pack/tests/mutations/GUIDE4-{1,2,3}.mutation`）:

| ID | 内容 | 結果 |
|---|---|---|
| GUIDE4-1 | `guidance.rs`: `do` の漏れの判定を常に偽にする | DETECTED（`guide4_leak_in_do_detected`） |
| GUIDE4-2 | `guidance.rs`: 開始点の水準の経路を空にする | DETECTED（`guide4_leak_via_start_guidance_only_detected`） |
| GUIDE4-3 | `guidance.rs`: `body_levels` の経路を使わない | DETECTED（`guide4_leak_via_body_levels_only_detected`） |

- 限界: 教材の作成者と検査器の作成者が同じ AI である。語の一致は部分文字列の近似である。モジュールの経路は欠陥パックにせず `guidance_levels.rs` が担当する。CLI の `validate` / `test` は未実装（TASK-404）のため、ライブラリの `validate_dir` で確かめた。GUIDE-5 の限界 3 項目は 5.253 が担当。人の確認は得ていない。

### 5.251 ヒントを開く操作と上限での拒否（TASK-295。GUIDE-17）

- 目的: `Control::OpenHint` が、現在の Step で現在の案内レベルに出せる数 n まで 1 段ずつ開け、n を超える操作と現在の Step が無い操作を `Refused`（`NoHint`）で拒否し、成功・拒否のどちらでも世代と `run` を変えないことを固定する。段数は教材が決め、3 段に固定しない（D-55 で固定しないと決まった。5.254 で確かめる）。
- 設計: core の `hint_open::on_open_hint`（非公開）。Step が無いときは空の uid の印で拒否を記録する。案内レベルを下げて n が開いた段数を下回っても拒否し、段数は減らさない。処理中でも開ける。
- 試験: `crates/core/tests/hint_open.rs`（水準ごとに n 段まで・超過の拒否で世代と状態が不変・Step 無し・ヒントの無い Step と IR に無い Step・水準を下げた後・水準ごとの上限）と性質テスト 2 本（各 1,000 件。宣言の組 × 水準 × 回数、Step の移動・水準の変更・操作の混在列）。
- 実行結果（2026-10-23。元にしたコミット 05f8102）。コマンド: `scripts/check-mutations.sh crates/core/tests/mutations/GUIDE17-2.mutation crates/core/tests/mutations/GUIDE17-3.mutation crates/core/tests/mutations/GUIDE17-4.mutation`

| ID | 対象 | 期待した失敗 | 実際に失敗した試験 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|---|
| GUIDE17-2 | `hint_open.rs`: 上限の比較を緩め n+1 段目が開ける | refuses_beyond_n_without_changing_anything_but_the_log, property_open_up_to_visible_count | 左に加え the_limit_follows_the_current_level など 5 件 | DETECTED | cases=1000、固定の既定の種、最小の入力 `([], 0, 1)` |
| GUIDE17-3 | `hint_open.rs`: Step 無しの拒否を空の列にする | refuses_without_current_step | refuses_without_current_step, property_mixed_sequences_keep_the_rules | DETECTED | cases=1000、最小の入力 `(false, [Open])` |
| GUIDE17-4 | `hint_open.rs`: 開いた段数を記録しない | opens_one_by_one_up_to_n, property_open_up_to_visible_count | 6 件 | DETECTED | cases=1000、最小の入力 `([None], 0, 1)` |

- 確かめないこと: 画面での表示と開く操作（TASK-530）。
- 範囲外: 鍵を実体ごとにすることと周回をまたぐ合計（TASK-297）、`SetGuidance` の結線（TASK-294）、3 段への固定（TASK-296。D-55 で固定しないと決まった。5.254 で確かめる）。

### 5.252 実体の呼び出しの有向非巡回グラフと数の照合（TASK-233 2/2。MOD-1）

- 目的: 教材全体が、モジュールの実体を静的に並べた有向非巡回グラフとして IR に載ることと、実体数・Step の実体数・開始数が教材の定義と一致することを固定する。
- 決定: IR の形式は変えない。core の `instance` に読み取り専用の `instance_graph`（`InstanceGraph`・`InstanceGraphError`）を足し、呼び出しの実体の水準で有向非巡回（Kahn 法）と配線（`caller`・`scope`・`enter`・`exit_to`）を確かめ、根ごとの Step の実体・開始の数を返す。Step の `next` の後戻りはこのグラフに含めない。`load_ir` には結線しない（手で組んだ IR と 1/2 の試験の IR が読めなくなり、`IrLoadError` の契約に波及するため）。`CONTRACT_VERSION` は据え置いた。
- 試験: `crates/core/tests/instance_graph.rs`（入れ子を含む受理と数、巡回 3 形と巡回の下流を返さないこと・未解決の呼び出し元・未知の `scope`・`scope` の食い違い・`main` の欠落と重複・`enter`/`exit_to` の誤りの拒否、決定性）、`crates/pack/tests/module_instance_graph.rs`（`module-reuse-rounds` の定義ファイルから導いた期待値と、compile した IR・`instance_graph` の数が一致すること。基点 36・5・19、辺、同じモジュールの 2 実体の Step 数、`--chapters` で絞った IR の不変条件）。
- 変異注入: `MOD1-1`（巡回の判定を常に通す）。
- 実行結果（2026-10-23。Linux。元にしたコミット 169a154）: 上の native 試験 18 件が通った。`scripts/check-mutations.sh crates/core/tests/mutations/MOD1-1.mutation` は DETECTED（期待した 3 件が失敗）。

    | ID | 結果 |
    |---|---|
    | MOD1-1 | DETECTED |

- 範囲外: `load_ir` への結線（拒否にするかの判断と理由コード）、web の `top_position.rs` の caller の追跡の置き換え、呼び出しの Step の持ち主の規則（TASK-235・TASK-241）、3 周目の追加で実体が 7 になること（MOD-12、TASK-245）。

### 5.253 検査の範囲の限界 3 項目（TASK-300 2/2。GUIDE-5）

- 目的: 決定 D-43 の限界 3 項目を、欠陥パックで固定する。(a) 実体の引数で渡す `do` は展開後の文字列まで検査して検出する。(b) skill 名を含むヒントの段 2・3 は水準 B でも検査しない。(c) 開始点の `guidance` も `body_levels` も持たない章の本文は検査の対象外とする。(b)(c) は「検出しない」現状の観察である。
- 試験: `crates/pack/tests/guide5_limits.rs`。差し替えは `testdata/pack-defects/guide5/b/G04〜G06`（土台は `base/format-b-v1_1`）。G04 は展開後の段の診断がちょうど 1 件で、引数を安全な値にすると 0 件になる対照つき。G05 は診断 0 件に加え、IR で水準 B に段 2・3 が見えること、同じ語を `do` に置くと検出される対照つき。G06 は診断 0 件で、正の対照は `guide4/b/G02`。`guidance_leak_scope.rs` の `pack52_` は範囲そのものの確認で、本試験は欠陥パックでの受け入れ試験である。
- 変異（2026-10-23。元にしたコミット 3f5bbaf。`scripts/check-mutations.sh crates/pack/tests/mutations/GUIDE5-{1,2}.mutation`）:

| ID | 内容 | 結果 |
|---|---|---|
| GUIDE5-1 | `expand.rs`: 置換後の `do` の漏れの検査を行わない | DETECTED（`guide5_a_argument_do_leak_detected`） |
| GUIDE5-2 | `guidance.rs`: 宣言の無い章も水準 B で検査する | DETECTED（`guide5_c_undeclared_chapter_body_is_not_checked`） |

- (b) に変異は置いていない。ヒントの文面を検査する経路がそもそも無く、1 行の置換で「検査する」状態を作れないためである。
- 限界: 教材の作成者と検査器の作成者が同じ AI である。語の一致は部分文字列の近似である。CLI の `validate` / `test` は未実装（TASK-404）のため、ライブラリの `validate_dir` で確かめた。人の確認は得ていない。

### 5.254 ヒントの上限の規則（TASK-296。GUIDE-6・D-55）

- 目的: ヒントの上限が「現在の案内レベルで出せる数 n まで」であり、3 段に固定されていないことを、n が 3 でない Step の試験と 39 本のヒントの確認で固定する。
- 設計: D-55 で固定しないと決まっており、コードは変えない（`hint_open::on_open_hint` は `current_hints(..).len()` を上限にし、定数を持たない）。試験と変異で、3 段への固定と 3 段であることの要求を見分けられるようにした。
- 試験: `crates/core/tests/hint_limit_rule.rs`（1・2・4・5・7 段の Step で、ちょうど n 段まで開け n+1 段目が `NoHint` で世代が不変・4 段以上が 3 段で打ち切られない・3 段未満でも拒否されない・水準を下げても開いた段数が減らない・出せる数が 4 以上の場合を毎回含む性質テスト 1,000 件）と、`crates/pack/tests/hint_limit_39.rs`（合成の教材 `tests/data/hint-limit-pack` を `compile_dir` で IR にし、13 Step x 3 段 = 39 本と水準で段数が変わる 2 Step の計 19 件の確認を core の `apply` まで通す。診断は 0 件）。
- 実行結果（2026-10-23。実行時のコミット e9e2b78）。コマンド: `scripts/check-mutations.sh crates/core/tests/mutations/GUIDE6-1.mutation crates/core/tests/mutations/GUIDE6-2.mutation crates/pack/tests/mutations/GUIDE6-3.mutation`

| ID | 対象 | 期待した失敗 | 結果 | 性質テストの種・ケース数・最小の反例 |
|---|---|---|---|---|
| GUIDE6-1 | `hint_open.rs`: 上限を 3 段で打ち切る | four_or_more_visible_hints_are_not_capped_at_three, property_limit_is_visible_count_not_three | DETECTED | cases=1000、固定の既定の種、最小の入力 `([], 4, 0, 1)` |
| GUIDE6-2 | `hint_open.rs`: 3 段でない Step を拒否する | fewer_than_three_hints_are_not_refused_for_being_short, opens_exactly_n_for_counts_other_than_three | DETECTED | cases=1000、固定の既定の種、最小の入力 `([], 4, 0, 1)`（GUIDE6-1 と同じ） |
| GUIDE6-3 | `guidance.rs`: ヒントの `levels` を無視する | hint_checks_19_pass_through_compile_and_open | DETECTED | - |

- 確かめないこと: 画面（TASK-530）。試作の 33/33 のうち開始点の案内レベルの確認 14 件（`starts[].guidance` の反映が前提。TASK-294）。`fandhe-pack test` の計数（TASK-404・TASK-84）。実教材の 3 段の記述（TASK-634）。試作の教材は private で写していないため、ヒントの確認 19 件は同じ形の合成の教材で流した。33/33 を達成したとは言えない。
- 範囲外: 3 段の構造の確認（TASK-298）、鍵を実体ごとにすること（TASK-297）。

### 5.255 本文・ヒントへの引数の値の差し込みの限界（TASK-302。GUIDE-15）

- 目的: 本文の木（IR 版 2）とヒントの文面に引数の値が入らない現状を、観察として固定する。D-50（H-3 の差し込みを足さない）と D-55（実体ごとの案内レベルを足さない）の範囲に GUIDE-15 は含まれず、差し込み語を足す決定が無いため、実装の分岐は「限界の固定」を採った。差し込み語の採否は spec 側の決定待ちである。
- 試験: `crates/pack/tests/body_arg_gap.rs`（`guide15_` 7 件）。(1) 本文の `${名前}` が木に字面で残る。(2) 実体どうしで水準 A・B・C の `select_body` が等しい。(3) 本文の未定義の名前は compile が通り、同じ字面を `do` に書くと `E-STRUCTURE`。本文の `{{step_do}}` は充填されない。(4) ヒントの ID と文面が実体で共有される（`pack45_` の観察を `hints_at` まで通す）。(5) 対照として、`do` は実体ごとに置換され、拒否の文面の `{{step_do}}` が実体ごとの値で埋まる。(6) 差し込みらしい未知の欄は `compile`・`validate` とも `E-PARSE` で、値を写さない。(7) PoC-4 の `round1`・`round2` は、同じ Step の本文が全水準で等しく、題材の名前を含まず、`do` は実体で異なる。
- 変異（2026-10-23。元にしたコミット ecc1557。`scripts/check-mutations.sh crates/pack/tests/mutations/GUIDE15-1.mutation`）:

| ID | 内容 | 結果 |
|---|---|---|
| GUIDE15-1 | `expand.rs`: `do` を引数の置換の対象から外す | DETECTED（`guide15_do_and_rejection_text_receive_argument_values` ほか 2 件） |

- 本文側の変異（本文を置換するようにする）は置いていない。`bodies` が置換前の参照からファイルを読んで木を作るため、1 行の置換では作れない。
- 確かめないこと: 画面での本文・ヒントの表示（TASK-530・SHELL-7）、差し込み語の採否。
- 限界: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.256 呼び出しの Step の持ち主と次の周回の初期状態（TASK-235 1/2。MOD-3）

- 目的: 次の周回の根の Step がすべて `NotStarted` でフラグが空であり、開始の直後は現在の Step と呼び出しの Step だけが `Entered` になることを固定する。
- 決定: Step の状態・選択の持ち主を `instance::owner_of`（`Instance::caller` が当の Step の実体の根。無ければ `root_of`）に一本化し、フラグ・効果の根は `root_of`（実体 ID の根）のままにした。`transit`・`Session::entered_at`・`accept`・`confirm`・`env_input` の書き読みと、web の `step_status`・`next_chapter`・`offers` の読み取りを `owner_of` 経由にした。これで周回ごとの根に呼び出しの Step が含まれ（2 周目で 18 件）、章の最後が `use` の Step でも「次の章へ進む」が出る。`CONTRACT_VERSION` は据え置いた（`docs/engine-api.md` の改訂履歴）。
- 試験: `crates/core/tests/call_step_owner.rs`（手で組んだ IR での持ち主・フォールバック・`caller` 重複・切り身の鍵の不変条件）、`crates/pack/tests/round_slice_init.rs`（`module-reuse-rounds` で 18 件・呼び出しの Step 2 件、開始の前の `NotStarted`・フラグ空、開始の直後に `Entered` が 2 件だけ）。`module_instances_session.rs` は呼び出しの Step を名前の比較から除いた。`Control::Begin` は未実装のため、開始の直後は `GoTo` で代用した。
- 変異注入: `MOD3-1`（`owner_of` が `caller` を無視する）。2026-10-23 に `scripts/check-mutations.sh crates/core/tests/mutations/MOD3-1.mutation` は DETECTED（期待した 2 件を含む 4 件が失敗）。
- 持ち主の読み書きの追加（レビュー対応）: `apply_acceptance`（`accept_text`・`accept_command` が共用。引数に `ir` を足した）、`judge` の状態の読み、`tick` の受付済みの判定も `owner_of`（`status_of`）を通した。効果の根（フラグ）は `root_of` のまま。`call_step_owner.rs` に、コマンド入力・自然文の受付が呼び出しの Step の `Accepted` を呼ばれた実体の切り身に書くことの試験を足した。
- 条件の持ち主（レビュー対応）: `condition::eval_condition` に実体の一覧（`instances`）を渡し、`step`・`chose` の参照先を `instance::owner_in`（`owner_of` の実体）に揃えた。`call_step_owner.rs` ではなく `condition_eval.rs` の `step_and_chose_read_the_callee_slice_for_a_call_step` が、呼び出しの Step の `entered`・`achieved`・`not_started`・選択を呼ばれた実体の切り身から読むことと、実体の一覧が空の対照を確かめる。
- 範囲外: 2 周目の終了後に 1 周目の切り身が不変なこと（TASK-235 2/2）、`Control::Begin`（TASK-237 1/2）、`Checkpoint.owner` と `projection` の復帰の範囲（TASK-241）、根の定義（#1413）。

### 5.257 ヒントの段の構造の確認（TASK-298。GUIDE-8・D-55）

- 目的: 「ヒントは 3 段」を教材側の約束として扱い、段の数・答えの段・Step ごとの例外を教材テストが宣言として渡すと、宣言と違う Step が一覧で報告されることを固定する。エンジンは段の数を固定しない（D-55）。
- 設計の判断: 段は `hints` の並び順で数え、ヒントの定義の `level`・`text` は読まない（語彙の語ではなく、読むには拡張点の手続きが要る）。`validate` には結線せず、新しい診断コードも足さない（結線すると「3 段」がエンジンの規則になる）。定義に解決できない ID は段に数えない。`hints` が空の Step は対象外で、宣言が名指しした Step は段数 0 として確かめる。未定義の ID だけを持つ Step は参照を持つので対象にし、段数 0 として報告する。宣言の誤り（段数 0・答えの段が範囲外・欄の名前が空）は報告 0 件と区別するため `Err` にする。報告は `uid`・数・種類だけで、文面・答え・欄の値を写さない。
- 試験: `crates/pack/tests/hint_stages.rs` の `guide8_` 11 件と `crates/pack/src/hint_stages.rs` の単体試験 2 件。同じ教材を宣言 2・3・4 で確かめて結果が変わること（対照）、4 段の Step でも `validate`・`compile` が診断なしで通り `visible_hints` が 4 件を返すこと、欄の名前を別名にしても答えの照合が働くこと、未定義の ID だけを持つ Step が段数 0 として報告されること、モジュールの実体ごとに `uid`（`<モジュール>@<実体>` を含む）で報告され表示でも伏せないことを含む。
- 変異（2026-10-23。元にしたコミット 91e20b1。`scripts/check-mutations.sh crates/pack/tests/mutations/GUIDE8-{1,2,3}.mutation`）:

| ID | 内容 | 結果 |
|---|---|---|
| GUIDE8-1 | `hint_stages.rs`: 段数の比較を `!=` から `>` にする | DETECTED（`guide8_steps_not_matching_the_declared_count_are_reported`） |
| GUIDE8-2 | `hint_stages.rs`: 答えの照合を常に一致とみなす | DETECTED（`guide8_answer_text_contains_answer`） |
| GUIDE8-3 | `hint_stages.rs`: 未定義の ID も段に数える | DETECTED（`guide8_answer_stage_must_resolve`） |

- 限界: 段の中身（場所・道具の名前・完全なコマンド）の意味と教育上の適切さは確かめない（GUIDE-8 は検討中）。答えの照合は部分文字列の包含の近似である。`fandhe-pack test` は未実装（TASK-404）で、呼び出し元は試験だけである。プロトタイプの教材は private の submodule にあるため、一時ディレクトリの教材で確かめた。
- spec 側の差分の案（spec repo の作業。自動運転のため人の承認は得ていない）: GUIDE-8 の証拠の種類「自動テスト（native。構造のみ）」を本節に結びつける。INTRO-39 の「3 段が揃わない Step を教材テストが検出するかは記録に無い」を、宣言どおりの段数でない Step を報告できる、に更新する。

### 5.258 前の周回の切り身とフラグの分離（TASK-235 2/2。MOD-3）

- 目的: 2 周目を終えた後も、1 周目の切り身が 2 周目の開始の前と同じで、1 周目のフラグが 2 周目の実体の切り身で立たないことを固定する。
- 決定: エンジンのコードは変えず、試験だけで固定した。事前の調査で、現行の実装は 1 周目の切り身を変えないことを確かめている。
- 試験: `crates/pack/tests/round_slice_init.rs`。`finish_round2` が `pt-rounds` の `s-r2` の区間と同じ入力列で 2 周目を終える。
  - `first_round_slices_are_unchanged_after_the_second_round_ends`: 1 周目の各根の切り身が `==` で同じで、1 周目の全 Step が `Achieved` のまま、呼び出しの Step の状態が `main` に入らない。
  - `first_round_flags_are_not_set_in_second_round_instances`: 2 周目の `issue-to-merge` のフラグは `precheck_ok` だけ（`stopped` なし）、`idea-cycle` は `revised` だけ。1 周目の `stopped`・`precheck_ok`・`variant_miss` は立ったまま。2 周目の操作が出した `Event::Flag` の範囲がすべて 2 周目の根。1 周目の `variant_miss` と `precheck_ok` は、2 周目の受付が同じ名前を扱うため、根の取り違えを検出できる。
  - `second_round_end_matches_the_playthrough_asserts`: `pt-rounds` の `asserts` の写し。
- 限界: 後続の部品が未実装のため、`GoTo` を 3 か所で使う。(1) 開始（`Control::Begin` は TASK-237 1/2）、(2) `idea-cycle@round2` の `st-2-5` の手前から `st-2-6` へ（遷移先の `on_enter` の効果を適用しないため）、(3) `issue-to-merge@round2` の `st-7-2` から `st-7-4` へ（`wait` の Step で処理中が始まらないため）。2 周目の終了は、最後の Step と呼び出し元の達成で判定する（`ChapterCompleted` は未実装）。`st-7-4` の受付で `Warning` が出るが、表明しない。
- 変異注入: `MOD3-2`（`root_of` が周回の接尾を落として根を混ぜる）。2026-10-23 に `scripts/check-mutations.sh crates/pack/tests/mutations/MOD3-2.mutation` は DETECTED（期待した試験を含む 6 件が失敗）。
- 範囲外: `when` つきの `next` の評価・着地の `on_enter`・`wait` の自動開始、`Control::Begin`、3 つのハッシュの一致（TASK-237）、`Checkpoint.owner` の範囲（TASK-241）。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。

### 5.259 次の周回の開始 `Control::Begin`（TASK-237 1/2。MOD-3・MOD-5）

- 目的: 前の周回を終えた状態から、`Control::Begin` で新しい開始点の次の周回を始められることを固定する。
- 決定: `crates/core/src/begin.rs` を足した。開始条件（`start_gate::check_start`）は状態に触れる前に評価し、満たさない・ID が無いときは `Refused` 1 件で状態を変えない。成功時は世代 +1、`Begun`、開始点の `effects`（原因 `Start`）、開始の節点への着地の順に進む。
- 試験: `crates/core/tests/begin.rs`（`min-3step` で世代・`Begun` と `Entered` の順・不明な ID の不変・処理中と入口の写しの解除・下書きとヒントの引き継ぎ・3 つの入口の一致）、`crates/pack/tests/round_slice_init.rs`（`GoTo` を `Begin("s-r2")` に置き換えた既存 3 試験、`s-r2-fresh` の 13 経路が原因 `Start` で書かれること、開始条件を満たさない `s-r2` の拒否で状態が不変なこと）。
- 限界: 飛ばした Step の `Skipped`（TASK-217）、`start.guidance` による案内の水準（TASK-294）、`${名前}` の置換（TASK-239）、入口の写しの作り直し（TASK-242、本件は `go_to` と同じく `None`）は扱わない。3 つの世界状態のハッシュの一致は TASK-237 2/2（5.260）、拒否の完全な保証は TASK-240。変異注入は 5.260 で足した。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.260 世界状態と切り身のハッシュの一致（TASK-237 2/2。MOD-5）

- 目的: `Control::Begin` による次の周回の開始が、世界状態と 1 周目の切り身を変えないことを、ハッシュで固定する。
- 決定: エンジンのコードは変えず、試験と記録だけを足した。ハッシュの定義は `testdata/module-reuse-rounds/test-data/scenarios.md` の A 節（SHA-256。入力は `serde_json::to_string` の正準 JSON の UTF-8）に従う。SHA-256 は試験専用の小さな実装（`crates/pack/tests/common/sha256.rs`。依存は足さない）で、既知解（空文字列・`abc`・FIPS 180-4 の 2 ブロックの例）を `printf ... | sha256sum` と照合した。`testdata/` は spec repo の写しなので CSV は置かず、値は本節の表に記録する。
- 試験: `crates/pack/tests/round_slice_init.rs`（`mod5_*` の 4 件）。1 周目は `play_round1` が `pt-r1` の入力列で流す。
  - `mod5_sha256_known_answers`: 既知解 3 件。
  - `mod5_world_hash_is_unchanged_by_begin_after_round1`: `Begin("s-r2")` の前後（H_end_r1・H_start_r2）で世界状態のハッシュと `==` が一致し、`Begun` が 1 件、`Changed` が 0 件。
  - `mod5_fresh_start_world_differs_from_round1_end_only_by_unreachable_paths`: 新しいセッションの `Begin("s-r2-fresh")` の直後（H_fresh）は H_end_r1 と一致しない。1 周目の終了時の全経路は fresh に要素まで一致して含まれ、fresh にだけある経路がちょうど 2 件であることを表明する（下記）。
  - `mod5_round1_slice_hashes_match_between_round1_end_and_round2_end`: 1 周目の根 2 件の切り身のハッシュが、1 周目の終了時と 2 周目の終了時で一致する。
- 記録（測定日 2026-10-23・元にしたコミット 8080634。`cargo test --locked -p fandhe-workshop-pack --test round_slice_init -- --nocapture` の `STATE-HASH` 行。仕様の真値ではなく実測値）:

| point | description | sha256 |
|---|---|---|
| r1-end-world | 1 周目の終了時の世界状態（H_end_r1） | `4ffc7541768aa5077273cd6b3e722879ef2e345b817c764892d0195ee845d70c` |
| r2-begin-world | `Begin("s-r2")` の直後の世界状態（H_start_r2） | `4ffc7541768aa5077273cd6b3e722879ef2e345b817c764892d0195ee845d70c` |
| r2-fresh-world | 新しいセッションの `Begin("s-r2-fresh")` の直後（H_fresh） | `291b8d8926c085b838ee69b67f922df2f9eba1e6c5d454b789a26e626c1b7f95` |
| r1-slice-idea-cycle@round1-at-r1-end | `idea-cycle@round1` の切り身（1 周目の終了時） | `e7af71dc9062a668ca3e32181923e0415f8b00f2a68e502cd586b9041f98cc43` |
| r1-slice-idea-cycle@round1-at-r2-end | 同（2 周目の終了時） | `e7af71dc9062a668ca3e32181923e0415f8b00f2a68e502cd586b9041f98cc43` |
| r1-slice-issue-to-merge@round1-at-r1-end | `issue-to-merge@round1` の切り身（1 周目の終了時） | `c39fd66ad7e650ec12f00201ca1c8af8fce52eb840c0404cca95ca81853c4275` |
| r1-slice-issue-to-merge@round1-at-r2-end | 同（2 周目の終了時） | `c39fd66ad7e650ec12f00201ca1c8af8fce52eb840c0404cca95ca81853c4275` |

- H_fresh の差（未達）: 受け入れ条件の H_fresh == H_end_r1 は、本件では満たせない。差は fresh にだけある次の 2 経路で、いずれもエンジンの後続の部品が未実装のため 1 周目の流しが書けない。試験はこの差がちょうど 2 経路であることを固定し、部品が入って差が縮むと落ちる。そのときは等式の試験へ書き換える。
  - `ideas/bbs-with-login/03-poc/login-session/result.md`: `st-2-5` の着地の `on_enter` が書く（未実装。`play_round1` が `GoTo` で飛ばす）。
  - `prs/pr-1-a`: `st-7-2` の `on_enter` と `wait` の進行が書く（未実装）。
- 1 周目の `GoTo` による回避 3 か所: (a) `st-2-4` の後に `st-2-6` へ（着地の `on_enter` が未実装）、(b) `st-3-2` の後の `st-4-1` へ（`when` つき `next` が評価されない。続けて `/init-claude`・`/setup-repo-guards` を送る）、(c) `ans-none-explicit` の後に `st-7-3` へ（`wait` の Step で処理中が始まらない）。
- 変異注入: `MOD5-1`（`Begin` が世界状態を消す）・`MOD5-2`（`Begin` が切り身を消す）。2026-10-23 に `scripts/check-mutations.sh crates/pack/tests/mutations/MOD5-1.mutation crates/pack/tests/mutations/MOD5-2.mutation` はどちらも DETECTED（`MOD5-2` は既存の周回の試験 4 件も失敗）。
- 限界: `ChapterCompleted` が未実装のため、1 周目の終了は最後の入力の直後で判定する。`st-7-4` の受付で `Warning` が出るが、表明しない。
- 範囲外: 着地の `on_enter`・`wait` の処理中の開始・`when` つき `next` の評価（H_fresh の等式化に必要）、途中開始と `${名前}` の置換（TASK-239）、閉じた根の引き継ぎ（TASK-238）、拒否の完全な保証（TASK-240）。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.261 持ち主の根の切り身と位置だけを戻す復帰（TASK-241 1/2。MOD-9）

- 目的: 復帰が戻す切り身を持ち主の根（保存時の `Checkpoint::owner`。無ければ写しの現在の Step の根）の 1 つに絞り、位置（`current`・`busy`・`completed`）は写しのものにする。他の根の切り身は復帰の前のまま残す（X1 の否定）。
- 決定: `crates/core/src/checkpoint.rs` の `on_restore` を、`state.run` を土台に持ち主の根の切り身だけを写しで置き換える形にした。世界状態・`global_flags`・案内レベルは暫定で写しの値を入れ、絞り込みと試験は TASK-241 2/2（#575）に回す。根が 1 つの既存の試験は変更なしで通る。
- 試験:
  - `crates/core/tests/restore_owner_slice.rs`（5 件）: 持ち主の切り身だけが戻る・写しに無い切り身は取り除く・`owner` が `None` なら写しの現在の Step の根に倒す・下書きとヒントは変わらない・乱数の性質テスト（S-5 の切り身と位置の部分）。
  - `crates/pack/tests/round_slice_init.rs` の `mod9_*`（3 件）: 2 周目の `st-2-4` で保存し同じ周回の別の実体と 1 周目の切り身が巻き戻らない（(ii-a)）・保存の後に変えた 1 周目の切り身が巻き戻らない（(ii-b) の代替）・1 周目の実体で保存し 2 周目の進みが消えない（(ii-c)）。
- 変異注入: `MOD9-1`（復帰が全ての根の切り身を写しへ戻す）。2026-10-23 に `scripts/check-mutations.sh crates/pack/tests/mutations/MOD9-1.mutation` は DETECTED。X1・X5・X6 の正式な検出の記録は TASK-244 の持ち分。
- 限界: (ii-b) の `st-7-2` への移動と進行は、`wait` の Step で処理中が始まらない（5.258・5.260）ため行わず、1 周目の Step への `GoTo` に置き換えた。`issue-to-merge@round2` の `st-7-1` へも入力で届かず `GoTo` で飛ぶ。世界状態 `prs/pr-1-a` の表明は TASK-241 2/2。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.262 世界状態・案内レベル・呼び出しの Step の復帰範囲（TASK-241 2/2。MOD-9）

- 目的: 復帰が戻す範囲を、世界状態は「保存の後に持ち主が書いた経路」、案内レベルは写しの値、呼び出しの Step の状態は持ち主と一緒に絞る。`global_flags`・ヒントの使用回数・下書き・他の根の切り身は戻さない（X5・X6 の否定）。
- 決定:
  - `crates/core/src/checkpoint.rs` の `on_save`・`on_restore` に `&Ir` を渡し、持ち主を `instance::owner_of` で決める（呼び出しの Step で保存すると呼ばれた実体の根）。IR から読むのは実体の呼び出し元の表だけ。
  - 世界状態は、写しの `log_len` 以降のログの `Changed` のうち宛先の持ち主が復帰の持ち主と一致するものの経路だけを、写しの値へ戻す（写しに無ければ取り除く）。原因は問わない。
  - 帰結: (a) 保存の後に持ち主と他の根の両方が書いた経路は戻る。(b) 宛先の無い `Changed` は戻さない。(c) 根が 1 つの教材では全体を戻す従来の結果と一致する。
  - 写しの `log_len` がログの長さを超えるときは、固定文の `Warning` だけで状態を変えない（core の fail-closed）。`Projection` は同じ状況で写し無しに倒す（fail-soft）ので、扱いが異なる。保存から読んだ履歴は再生で照合されるためこの状態には届かず、届くのは手組みの状態だけ。
  - `Checkpoint`・`RunState`・`Event` の形は変えず、`checkpoint` モジュールは署名の対象外なので `CONTRACT_VERSION` は上げない。
- 試験:
  - `crates/core/tests/restore_owner_slice.rs`（+6 件）: 持ち主の経路だけ戻る・共有の経路は戻る・保存の前と宛先の無い `Changed` は戻さない・`global_flags` は直前のまま案内レベルは写し・`log_len` の範囲外で `Warning` のみ・世界状態の性質テスト（S-5）。
  - `crates/core/tests/restore_call_step_owner.rs`（2 件）: 呼び出しの Step で保存すると持ち主が呼ばれた実体の根・復帰で呼び出しの Step の状態が戻り `main` の切り身は残る。
  - `crates/pack/tests/round_slice_init.rs` の `mod9_*`（+3 件）: 2 周目の持ち主の経路は戻り 1 周目の宛先の経路と切り身のハッシュは変わらない・呼び出しの Step の状態が持ち主と一緒に戻る・1 周目の保存への復帰が 2 周目の開始の世界状態を戻さない。
  - `crates/core/tests/contract_conformance.rs` の `sample_restore_entry_matches_apply`: 復帰が保存の後のログを読むため、e19 の前のログに e01〜e18 のイベント列を入れる形にした（`testdata/contract/event-sequence.json` は変えない）。
- 変異注入: `MOD9-2`（復帰が世界状態を写し全体へ戻す。X5 相当）・`MOD9-3`（保存の持ち主を scope の根にする。X6 相当）。2026-10-23 に `scripts/check-mutations.sh` で `MOD9-1`・`MOD9-2`・`MOD9-3` はいずれも DETECTED。X1・X5・X6 の正式な検出の記録は TASK-244 の持ち分。
- 限界:
  - 1 周目の世界状態の書き込みは、`wait` の処理中が未実装で `prs/pr-1-a` が作られない（5.258・5.260）ため、1 周目の `st-2-1` への `GoTo` と入力による `ideas/bbs-with-login/README.md` の書き換えに置き換えた。
  - 現在の Step が呼び出しの Step そのものになる場面は、`GoTo` でも入力でも作れない（着地先は呼ばれた実体の最初の Step）。そのため保存の持ち主の規則（X6）は pack では届かず、core の手組みの状態で確かめた。pack の呼び出しの Step の試験は、持ち主の復帰で状態が一緒に戻ることの確認にとどまり、`MOD9-3` は検出しない。
  - 案内レベルを戻すことは、`Begin` が開始点の `guidance` を反映しない（TASK-294）ため core の試験だけで確かめた。
- 範囲外: `Projection` が復帰の復習の根を `root_of` で引くため、呼び出しの Step で保存したときに core の持ち主と食い違う（直すには `Restored` へ根を持たせる契約の版上げか、`Projection` へ実体の表を渡す変更が要る）。停止（`stop.rs`）と上限の戻しの世界状態と `entry_derive` の根の決め方は TASK-242。案内レベルの切り替えと開始点の反映は TASK-294。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.263 入口の写しとチェックポイントの構成（TASK-242 1/2。MOD-10）

- 目的: 入口の写しが現在の Step の分を 1 つだけ持ち、Step に入るたびに置き換わること、チェックポイントが 4 欄を持ち通し番号が `Session` で 1 つであることを確かめる。
- 決定:
  - 状態の写しは「無い」か「現在の Step のもの」の 1 つだけ（不変条件 I-1）。`transit` の着地（通常の Step・呼び出し元への戻り）で無効化し、新しい写しは受付の時点をログから導く（SAFE-35）。
  - 不採用: `Entered` のたびに `Some(EntryCopy)` を書く方式。SAFE-35 の決定を覆し、`snapshot.entry` と `checkpoints[*].entry` の形が変わって履歴の照合に響くため採らない。
  - 型・欄・署名は変えないので `CONTRACT_VERSION` は上げない。
- 試験: `crates/core/tests/entry_copy_checkpoint.rs`（9 件）。遷移後の I-1・遷移後の保存に古い写しが入らない・巡回で入り直した後の停止が 1 周目の世界状態を戻さない・保存の 4 欄・通し番号の単一性・性質テスト 4 戦略（I-1・通し番号の規則）。修正前の `transit.rs` では 5 件が失敗し、修正後に通ることを 2026-10-23 に確かめた。
- 変異注入: `MOD10-1`（着地の写しの無効化を外す）。2026-10-23 に `scripts/check-mutations.sh` で DETECTED。X2 の正式な記録は TASK-244 の持ち分。
- 限界: 呼び出し元への戻り（`exit_to`）の分岐は同じ関数を通すが、この試験では専用の場面を作っていない。
- 範囲外: 停止の世界状態を持ち主の経路だけ戻すこと・`keep_other_roots` の根の決め方（TASK-242 2/2。5.264 で実装）。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.264 実体単位の停止（TASK-242 2/2。MOD-10）

- 目的: 複数の実体があるとき、処理中の停止が持ち主の根の切り身・位置と、持ち主が写しの後に書いた世界状態の経路だけを戻し、他の実体の切り身・経路を変えないことを確かめる（PoC-4 の F-3・変異 X2）。
- 決定:
  - 起点は停止の直前の `run`。持ち主の根の切り身・`current`・`completed`・案内レベルと、`log[entry.log_len..]` の `Changed` のうち宛先の持ち主が停止の持ち主と一致する経路だけを写しの値へ戻す。他は直前のまま。`global_flags` は戻さない（復帰と同じ）。
  - 持ち主の根は `owner_of` で決める（保存・`Changed` の宛先の判定と同じ）。処理中の Step は呼び出しの Step にならないので `root_of` と同じ値になる。1/2 が託した `keep_other_roots` の根の決め方への答えである。
  - 戻す規則は `entry_derive::owner_touched_paths`・`revert_owner` に置き、停止・復帰（挙動は同一）・上限で止めた後の戻し（`RewoundToEntry`）が共有する。`keep_other_roots` は削除した。
  - 写しのログ位置がログの長さを超えるときは固定文の `Warning` だけで状態を変えない（fail-closed。信頼できない値）。上限の戻しでは `busy` だけ空にする側へ倒す。
  - 型・欄・署名は変えないので `CONTRACT_VERSION` は上げない。エンジンは未配信（`0.0.0`）のため、停止の意味の変更による履歴の版上げはしない。
- 試験: `crates/core/tests/stop_owner_slice.rs`（9 件）。他の根の切り身が変わらない・世界状態は持ち主の経路だけ戻る（ログ位置より前・宛先なしは戻さない）・位置と世代・`global_flags` と案内レベル・写しに無い切り身の除去・ログから導く写しでの複数実体・範囲外のログ位置・根が 1 つなら全体を戻す結果と一致・性質テスト（停止 1 回に絞った M-1）。`contract_conformance` の e06 の単独の照合は、経路を決めるログ（e02・e04・e05 のイベント）を添える形に直した。
- 変異注入: `MOD10-2`（世界状態を持ち主以外の経路まで戻す。X2 相当）。2026-10-23 に `scripts/check-mutations.sh` で DETECTED。X2 の正式な記録は TASK-244 の持ち分。
- 限界: pack の実教材では、`wait` の処理中が未実装（5.258）で処理中の Step に入れないため、停止の確認を行っていない。上限の戻しの複数の根の場面は専用の試験を作っていない（既存の `transition_limit` が通ることで確かめた）。
- 範囲外: 操作列全体の M-1（混ざり 0）の縦断は TASK-243、X2 の正式な検出記録は TASK-244。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.265 Step の題・do・本文の表示（TASK-525 1/2。SHELL-7）

- 目的: Step の題・「いまやること」（`do`）・本文が、章の最初の Step を含めて画面に出ること、本文が見出し・一覧・表の構造を保つこと、案内レベルに応じた版が出ることを確かめる。
- 期待と定義の対応: SHELL-7 → `step_body`（`crates/web/src/step_body.rs`）。題は `h2[step-title]`、`do` は `p[step-do]`（固定の見出し「いまやること」つき）、本文は `div[step-body][data-body-level]`。親は `section[step-text]` で常に描く（FEND-3）。`shell-` の識別子は使わない（shell 契約は管轄外）。
- 決定:
  - 案内レベルは `RunState.guidance_level` を使い、本文は `select_body`、`do` は `step_do_at` が選ぶ（C → B → A の倒れは core が持つ）。
  - 本文は `doc_view::render_doc` だけを通す。題・`do` は文字ノードにだけ入れ、`RawHtml` を使わない。
  - 置き場は右の領域の先頭（仮置き。TASK-534・TASK-548）。Step か水準が変わったときだけ `step_body` を dirty に積む。
- 試験: native は `crates/web/tests/step_body_host.rs`（8 件。20 Step の題と `do`・章の最初の Step・本文の構造と外部画像・記号の非解釈・A/B/C と倒れ・dirty・履歴を開く・教材なし）と、`ch0_3_run_host`（題が変わった手で dirty）・`history_import`（開いたときの dirty の一覧）。ブラウザは `tools/browser/step-body.test.mjs`（20 Step の DOM と IR の一致・親の同一性・実コンパイルした教材の本文の構造・幅 1280px と 360px・実行と通信が 0 件・hidden の対照）。CSP あり・なしの両方で流す。
- 変異注入: `SHELL7-1`（本文の水準を A に固定）・`SHELL7-2`（Step の変化で dirty を積まない）・`SHELL7-3`（`do` の項目を作らない）。2026-10-23 に `scripts/check-mutations.sh` で 3 件とも DETECTED。
- 限界: 実行時の水準は常に A（`Control::SetGuidance` が core で未実装。TASK-294）。B・C は native で状態を注入して確かめた。ブラウザでの B・C は未確認。`direct-link-reload` の DOM の題の照合（5.204）は足していない。
- 範囲外: 49 件の注入を CSP あり・なしで画面の本文の区画に対して流す確認は #712（TASK-525 2/2）で実施した（5.175 の「本文の区画への差し替え」）。
- 実行の結果（2026-10-23）: `make lint`・`make test`・`make check-deps`・`make check-dom-reads`・`make check` が通過。ブラウザは `step-body` の 4 件と、`element-identity`・`shell-widths`・`shell-position`・`shell-tabs`・`inline-free`・`csp-runs`・`next-chapter`・`history-ops`・`direct-link-start` が通過。`view-only` 系は環境変数 `FANDHE_VERSION_HISTORIES_DIR` を用意せず未実行（未検証）。`make test-browser` 全体は未実施。`make check-generic-words` は本件と無関係の `crates/pack/src/guidance.rs:606` で既に失敗している。

### 5.266 チェックポイントの保存と復帰の操作（TASK-529 1/2。SHELL-13）

- 目的: 上部の区画 `shell-checkpoint` に置いた保存・復帰のボタンの押下だけで、保存 → 先へ進む → 復帰が通り、左の成果物と右の位置が保存の時点に戻ることを確かめる。
- 期待と定義の対応: SHELL-13 → 実装は `crates/web/src/checkpoint_ops.rs` と `host.rs`、試験は下記。保存・復帰の規則（EVT-12・MOD-9）は core が持つ。
- 置き方:
  - 枠の契約（`shell` 版 2）は変えない。入れ物は契約済みの `shell-checkpoint`。ボタンの名前（action `save-checkpoint`・`restore-checkpoint`、`data-testid` `checkpoint-save`・`checkpoint-restore`）は `shell-` の外で定めた。
  - registry の外の keyed list（field `checkpoint_ops`）。項目の root はボタンそのもの。`PartKind::Checkpoints` の DisplayPart は登録しない。
  - 出す条件は、session があり、表示だけでなく、現在の Step があること（欠けたら 0 件）。処理中は止めない（EVT-12）。
  - 押下は DOM の payload を信じない。保存は提示に保存のボタンがあるときだけ、復帰は正規形の 10 進（先頭 `0` なし・10 桁以下）で、今の `state.checkpoints` の鍵と一致したときだけ core へ送る（SAFE-33 と同じ方針）。
- 試験:
  - native: `crates/web/tests/checkpoint_ops_host.rs`（10 件）。出す条件・保存・保存 → 進む → 復帰・改ざんの復号・存在しない ID・現在の Step なし・検査器・dirty・表示だけ。
  - ブラウザ: `tools/browser/checkpoint-restore.test.mjs`。実際のボタンの click で場面 A（保存 → 4 Step 進む → 復帰。位置は IR から独自に数える）・場面 B（payload の書き換え 6 形）・場面 D（幅 360px・959px・配信する設定で 0 件）。CSP あり・なしで流す。
- 変異: `SHELL13-1`（提示との照合を外す）・`SHELL13-2`（dirty を積まない）・`SHELL13-3`（先頭 `0` の拒否を外す）。2026-10-23 に `scripts/check-mutations.sh` で DETECTED。`SHELL6-3` の `find:` は base の時点で `host.rs` と一致していなかったので、同じ行を変える本件で直した（DETECTED を確認）。
- 件数の多い場合（FEND-22・D-46）: 復帰のボタンは新しい側から 500 件ずつのページに分け、`checkpoint-page` のボタン（`checkpoint-older`・`checkpoint-newer`）で全件に届く。1 リストは目安以内で打ち切らない。試験は `checkpoint_ops.rs` の 5,000 件の単体試験。
- 限界: 文言は最小。一覧としての表示と、再読み込み・書き出し・読み込みをまたぐ一致は TASK-529 2/2（5.267 で扱った）。章の終わりの自動保存と数の上限は TASK-339。表示だけのモードでの 0 件はブラウザでなく native の試験で確かめた。
- 実行結果（2026-10-23）: `make lint`・`make test`・`make check`・`make check-deps`・`make check-dom-reads`・`make test-build-web` は通過。ブラウザは本試験 6 件と、shell-position・shell-mock・shell-widths・shell-columns・shell-tabs・element-identity・next-chapter・csp-runs・inline-free・restore-cancel が通過。`view-only`・`history-continue` の一部は環境変数 `FANDHE_VERSION_HISTORIES_DIR` を単体実行で渡せず未確認。`make check-generic-words` は `crates/pack/src/guidance.rs:606` の既存の違反で失敗する（本件の変更と無関係）。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.267 チェックポイントの一覧と再読み込み・書き出しをまたぐ一致（TASK-529 2/2。SHELL-13）

- 目的: 復帰のボタンの一覧が、保存した時点の章と Step の位置を持ち、再読み込みと、書き出し → 別のオリジンでの読み込みの前後で同じになることを確かめる。
- 期待と定義の対応: SHELL-13 → `crates/web/src/checkpoint_ops.rs`（`CheckpointEntry`・`offer`・`restore_label`）。
- 決定:
  - 位置は `state.checkpoints[id].run.current` と IR から `top_position` で導く。今の Step・時刻・乱数・DOM は使わない。履歴の `ops` から作り直した状態でも同じ値になるので、一覧が前後で一致する。
  - 項目は復帰のボタンのまま（鍵 `restore:{id}`）。文字は `チェックポイント {id}（{章}　Step M / K）に戻る`、属性は `data-checkpoint-chapter`・`data-checkpoint-m`・`data-checkpoint-k`。位置が導けないときは項目を残し、位置だけを出さない（fail-closed）。
  - 時刻は持たない（読み込みの前後で文字が一致しなくなるため）。
- 試験:
  - native: `checkpoint_ops_host.rs` に 4 件（位置の文字と属性が IR から数えた値・先へ進んでも不変・実ボタンの保存が履歴に記録され書き出し → 読み込み → 起動で一致・同じ ID の内容が読み込みで替わる）と、`checkpoint_ops.rs` の単体 1 件。
  - ブラウザ: `tools/browser/checkpoint-list.test.mjs`。実ボタンだけで章をまたぐ 3 件を作り、再読み込み 2 回・書き出し → 名前の違うサブパスでの読み込み → 読み込んだ側の再読み込みで一覧が一致すること、両側で同じ復帰をした結果の一致、保存で 1 件増えると不一致になる負の対照、幅 360px の横スクロールなしを確かめる。CSP あり・なしで流す。
- 変異注入: `SHELL13-4`（位置を保存した時点でなく今の Step から導く）。2026-10-23 に `scripts/check-mutations.sh` で DETECTED。
- 限界: 一覧はページ 0（500 件以下）の範囲で比べた。ページ位置は表示の状態で、再読み込みで 0 に戻る。L2 の配布物（TASK-114）と macOS（TASK-342）は扱わない。`history-continue` の保存はなお `raw-trigger` を使う。
- 実行結果（2026-10-23）: `checkpoint-list` 3 件がブラウザで通過。`make test-browser` 全体は未実施（未検証）。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.268 試験の記録の節番号の整理と、href の検査・IR の取得の除外の残課題（#1740）

実行日: 2026-10-23。元にしたコミット: `13ecddd`。並列の PR が同じ節番号を取り合った結果、5.2xx に重複 5 件と順序の乱れが残っていた。第 5 節の冒頭の規則（既存の番号は変えず、後から来た節に枝番を付ける）に従って直した。

#### 節番号の整理

判定の基準は「その番号を見出しに付けたコミットが main にマージされた時刻」で、遅い方に `b` を付けた。例外は無い。

| 番号 | 先の節（番号のまま） | 枝番を付けた節 |
|---|---|---|
| 5.202 | TASK-62 1/2 | TASK-51 → 5.202b |
| 5.204 | TASK-110 1/2 | TASK-535 → 5.204b |
| 5.236 | TASK-182 2/2 | TASK-182 1/2 → 5.236b |
| 5.241 | TASK-523 | TASK-113 → 5.241b |
| 5.245 | TASK-536 | TASK-114 2/2 → 5.245b |

- 並べ替え: 5.190 以降の節を `(番号, 枝番)` の順に並べ直した。節の本文は移動しただけで変えていない。見出し 5 行の番号以外の行の多重集合は、並べ替えの前後で同じである（`sort` の比較で確認。差は見出し 5 行のみ）。
- 追従した参照: `CLAUDE.md`（`make test-l2-run` の 5.241b・5.245b、history-ops の 5.234）・`README.md`（同 5.241b・5.245b）・`tools/browser/l2-run.test.mjs`（5.241b）・本ファイルの TASK-114 1/2 の範囲外の行（5.245b）。重複を「直さない」と書いた 4 節の範囲外の行には、本件で解消した旨を足した（本文は消していない）。
- 範囲外として残した乱れ: 5.0xx〜5.1xx の順序（5.34 が 5.64 の後、5.60b・5.60c・5.87b の位置）、第 6 節が 5.171 と 5.172 の間にある構成、重複を自動で防ぐ検査が無いこと。

#### `STATIC_HREF_GUARD` を同じ経路の照合にした

`crates/web/tests/no_hidden_triggers.rs` の検査は、`blob:` の確認が本文のどこかにあることしか見ていなかった。確認の後ろで別の値を `href` へ書く改変を通すため、空白を除いた本文の `if(url.startsWith(blobPrefix))a.href=url;` がちょうど 1 件あり、唯一の `.href=` がその文の中にあることを求める形（`STATIC_HREF_GUARDED_WRITE`）に変えた。

- 対照（`blob_の確認と_href_の代入が離れた写しを検出する`）: 元の行を (a) 確認を空の分岐にする、(b) 確認を別の文にする、(c) 確認を否定する、(d) `|| true` を足す、(e) 別の値を代入する、の 5 変種に置き換えた写しをすべて検出した。従来の判定（確認がどこかにある・代入が 1 件）は 5 変種すべてを通すことを同じ試験の中で assert し、新しい検査だけが見分けていることを示した。実物の `app.js` は違反 0 件。
- 試験は 1 件増えた（`no_hidden_triggers` 9 → 10）。`scripts/expected-test-counts.txt` は変えていない（件数の照合は期待値のファイルを変えた PR か版上げ PR でだけ働く。更新は別の PR）。
- 限界: 文字列の隣接の照合であり、JS の構文は解析しない。`url` が `const` の束縛であることは `app.js` の読み取りで確認したが、検査はそれを見ていない。

#### IR の取得の `ERR_ABORTED` の除外

`tools/browser/l1-l2-runner.mjs` の `irFetchCheck` は、応答 200 の後に同じ要求が `net::ERR_ABORTED` と通知される競合を除いている。原因を調べた。

- 5.175 の説明（IR の読み取りが上限で `reader.cancel()` を呼ぶ）は成り立たない。`app.js` の `readBounded` が `cancel()` を呼ぶのは読んだ量が上限（`ir_read_limit()`。20,971,521 バイト）に達したときだけで、試験の IR は数十 KB（`crates/web/tests/data/ch0_3/ir.json` は 35,958 バイト）である。仮説 H1 はコードの読み取りで棄却した。実測（`cancel` の呼び出し回数）は取っていない。
- CI のログ（`browser-test`。3 回分）に `ERR_ABORTED` の行は無い。除外は黙って通すため、ログから頻度は分からない。
- CDP の計測（H2: 完了の通知の前に loader が解放される競合、H3: キャッシュ・preload との重複、H4: Playwright の対応付けの順序）は、この環境にビルド済みの配信物（`dist/web`）・`node_modules`・miniserve が無く、未実施（未検証）である。
- 結論: 原因は未確定。除外は外さない。残してよい理由は、(1) 同じ要求の id、(2) 応答が 200、(3) URL とエラーの種類が一致する 1 形だけを除く、(4) アプリが起動できたので本体は読み切っている、の 4 点で、重複の取得・応答のない中断・別の資源は従来どおり数える。
- 次の手順: ビルド済みの環境で、IR の要求について `Network.loadingFinished`・`loadingFailed`（`canceled`）と `Content-Length` の有無を、CPU 抑制 × 配信サーバ（`serve.mjs`・miniserve）× 50 回以上で記録する。
- `tools/browser/pack-body-xss.test.mjs` の除外は、URL とエラーの種類だけを見ていて `irFetchCheck` より緩い。厳しくそろえるのは、ブラウザ試験を流せない環境では確かめられないため、本件では変えていない（範囲外として報告）。

### 5.269 枠の通しの試験の骨格（TASK-548 1/2。SHELL-1・SHELL-7・SHELL-8・SHELL-10）

- 目的: 枠の外側の部品（ART の成果物のビュー・TERM のログと Step の題と do・上部の位置・「次の章へ進む」）を組み合わせた通しで、結線の破れを見つける。部品ごとの試験が通っていても、組み合わせで壊れる場面があるため。本件は骨格で、CH-0〜CH-3 の 20 Step を幅 1280px と 360px の両方で最後まで通す。チェックポイント・再読み込み・書き出し→読み込み・保存の失敗の表示は TASK-548 2/2（#740。5.270 で差し込んだ）が担当した。
- 期待と定義の対応: SHELL-1（左右の取り付け点に ART と TERM の部品が載る・2 列とタブ）・SHELL-7（Step の題と「いまやること」）・SHELL-8（上部の位置）・SHELL-10（「次の章へ進む」での章の移動）。
- 構成:
  - `tools/browser/shell-journey.mjs`: 誘導（`ch0-3-journey.mjs` の `journey` を `chaptered: true` で使う）と Step ごとの観察。2/2 は `hooks.afterStep(uid, status, ctx)` から差し込む（差し込み口はここ 1 か所）。
  - `tools/browser/shell-journey.test.mjs`: 幅 2 × CSP あり・なしの 4 件、幅をまたぐ一致 2 件。
  - `paneAware`（960px 未満で操作の前に要るタブを選ぶ）は `ch0-3-journey.mjs` へ移して `shell-widths` と共有した。「次の章へ進む」は操作の領域の末尾にあるので、操作のタブを選んでから押す。
- 解釈:
  - 「模範解答で通す」は、既存の `journey` の誘導を CH-0〜CH-3 の形の通しとして使うことと読んだ。誘導は途中に拒否・問い返しの入力を含むが、最後は正しい入力で進める。本物の教材（`pt-ch0-3`）の模範解答での完走は教材 repo の TASK-642 の担当で、本 repo の範囲外とした。
  - 左の変化は、IR で規則の effects に `put` または `set_attr` を持つ Step（規則の外の processing 側の effects は除く。実測の対象は 13 件）に限って要求した。effects の無い Step には要求しない。あわせて、`engine.artifacts` が変わったのに左の指紋が変わらない場面が 0 件であることを各 Step で確かめた。左の指紋は `shell-left` の中の `[data-row]` の path と文字の列で、差分の印（mark）は表示の状態で揺れるので含めない。
  - 期待値（位置・題・do・effects の有無）は IR から数え、画面の `data-*` を読み返して合格にしない。
- 確かめること:
  - 各 Step で: 取り付け点（左に成果物のビュー 1・右にログ 1 と題 1・逆側に 0）、題と do が IR のとおり、ログの項目が増える、左の変化、位置の文字が `<章 ID>　Step M / K` で shell-top の中・tabpanel の外にある、世界状態の一致（`worldMatchesState`・`incrementalMatches`）。
  - 360px では操作のタブで題が、成果物のタブでビューが見える（見えているかは幅でなく計算された display で判定）。1280px ではタブの切り替えが 0 回。
  - 最後に: 20 Step が達成で終点が `main/st-3-2`、章の移動が 3 回、エンジンの警告・無視が 0 件、ページ内のエラー・CSP 違反・配信元の外への通信・安全性の計数が 0 件、幅をまたいで最終の世界状態と各 Step の状態が一致する。
  - 対照: 成果物のビューを hidden にすると見え方の判定が、位置の文字を書き換えると位置の判定が不合格になる。
- 結果の記録の様式: 試験は 1 回ごとに標準出力へ次の 1 行を出す。入力した値や本文は出さず、名前・件数・合否だけを出す。

  ```text
  SHELL-JOURNEY width=<px> csp=<on|off> steps=<達成数>/20 crossings=<章の移動> left-changes=<合格>/<対象> right-changes=<合格>/<対象> title=<合格>/<対象> position=<合格>/<対象> tab-clicks=<回数> external=<件数> page-errors=<件数> ledger=<合格>/<項目>
  ```

  記録の表の列は「実行日・コミット・幅・CSP・上の各欄・合否」。2/2 が足す列は `checkpoint`・`reload`・`export-import`・`save-failure`（予約。2/2 で追加）。
- 実行結果（2026-10-23）: 4 件と一致の 2 件がブラウザ（Chromium）で通過。各行は `steps=20/20 crossings=3 left-changes=13/13 right-changes=19/19 title=20/20 position=20/20 external=0 page-errors=0 ledger=167/167`、`tab-clicks` は 1280px で 0、360px で 70。`shell-widths`・`next-chapter`・`ch0-3-run`・`shell-position` は通過。`csp-runs`・`step-body` は配信する設定の成果物（`FANDHE_WEB_DIR`）が無い環境のため一部が失敗し、未検証。`make test-browser` 全体は未実施（未検証）。
- 限界:
  - 「次の章へ進む」・Step の本文・書き出しの操作の置き場は、いまも仮置きである（`crates/web/src/host.rs` のコメントは最終の配置を TASK-534・TASK-548 としている）。本件は配置を決めず、仮置きのままでも 2 幅で通ることだけを確かめた。
  - 試験の件数の期待値（`scripts/expected-test-counts.txt`）は更新していない（5.268 の方針。別の PR で行う）。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.270 枠の通しに挟むチェックポイント・再読み込み・書き出し・保存の失敗（TASK-548 2/2。SHELL-1・SHELL-7・SHELL-10・SHELL-13・SHELL-15）

- 目的: 5.269 の通し（CH-0〜CH-3 の 20 Step）の途中に、部品ごとの試験では見つからない結線の破れが出やすい操作を挟む。復帰・再読み込み・別のオリジンでの読み込みの前後で、同じ Step・同じ世界状態・同じ画面になることを、章の移動と重ねて確かめる。
- 期待と定義の対応: SHELL-13（チェックポイントの保存と復帰・一覧）・SHELL-15（書き出しと読み込み。別のオリジンでの継続）・SHELL-1／SHELL-7／SHELL-10（通しの観察は 5.269 のまま）・SAVE-10（保存の失敗の表示）。
- 構成:
  - `tools/browser/shell-journey-trips.mjs`（新設。試験でない補助モジュール）: 観察（`snapshotOf`）・比較（`diffReload`・`diffRestore`・`diffImport`。違った欄の名前だけを返す）・操作（保存・復帰・再読み込み・書き出し・読み込み）・`makeTripHooks`。`runShellJourney` の `hooks.afterStep` から差し込む。`journey` と `ch0-3-journey.mjs` は変えていない。
  - `tools/browser/shell-journey.test.mjs`: 2 つ目の配信（`/another-repo-name/`。別のオリジン）を入れ子で起動し、`test()` の件数は 6 のまま（既存の 4 件の中に挟んだ。`scripts/expected-test-counts.txt` は変えていない）。timeout は 600000。
  - `tools/browser/shell-journey.mjs`: `recordLine` に 4 列を足した。
- 挟む場所（両方の幅・両方の CSP で同じ）:
  - `main/st-0-2b` 入った直後: チェックポイント 1 件目を保存（一覧が 1 件。章・M・K が IR から数えた値）。
  - `main/st-1-1` 入った直後（章をまたいだ直後）: 再読み込み。
  - `main/st-1-4` 入った直後: 2 件目を保存し観察を取る。
  - `main/st-2-1` 入った直後（2 回目の章の移動の直後）: 観察を取る→ 3 件目を保存→ 2 件目（前の章）へ復帰→ 3 件目へ復帰→ 再読み込み。一覧は 3 件のまま一致。
  - `main/st-2-2` 入った直後: 目印（sessionStorage）を立てて再読み込みし（IndexedDB が隠れる）、localStorage を履歴の鍵でない接頭辞（`pad/`）で満たす。受理後: `save-failed` の項目が 1 件・理由に `QuotaExceededError`・見える・受講の状態は進んでいる・保存値は区間の開始のまま。詰め物を外す。
  - `main/st-2-3` 入った直後: 通知が消え、保存値が更新された。目印を外して再読み込み。
  - `main/st-3-1` 入った直後: 画面の操作だけで書き出し→ 別のオリジンの新しい context で読み込み（読み込みの前は書き出し側と違うことを対照で確認）→ 判定 `open` → 書き出し側と同じ Step・状態・画面→ 読み込んだ側の再読み込みの後も同じ→ 読み込み側の通信・エラー・CSP 違反・ダイアログ・ポップアップ・安全性の計数が 0 件。
  - `main/st-3-2` 確認待ち: 再読み込み。通しの後（達成のまま）: 再読み込み。
- 比較の定義:
  - 再読み込み: 現在の Step・世代を除く状態・成果物・復習が等しい。世代はちょうど +1。エンジンのログは前のログの末尾に `reloaded` が 1 件増えたもの。画面（左の行・位置・題と do・入力の口・チェックポイントの一覧）が等しく、画面のログは前の列の末尾に 1 件増えたもの。
  - 復帰: 現在の Step・世界・slices・成果物・画面（左・位置・題と do・入力の口）が保存の時点と等しい。世代が進み、`restored` がちょうど 1 件増え、一覧は変わらない。
  - 書き出し→読み込み: エンジン全体が等しく、画面が等しい。
  - 比べない欄と根拠（黙って除かず記録する）: (a) 成果物の `mark` と左の行の印の記号・文字は、直前の世代との差で決まる表示の状態で、復帰では消える。`restore-cancel` と同じ扱い。(b) 読み込みの比較では `lastImport`・`installCount`（5.244 と同じ）に加え `bootKind` を除く。再読み込みを経た側は `restored`、読み込んだ側は `imported` になる起動の経緯の記録で、受講の状態ではない。
- 解釈: (7) 「保存の失敗の表示」は、通しの途中で localStorage の容量超過が起きたとき、枠の通知欄に失敗の通知が出て、受講の状態は進み、次の保存の成功で消え、再読み込みで最新の状態に戻ることと読んだ。IndexedDB の失敗の経路は `save-idb-failure` の担当で、本件は 1 経路に絞った。IndexedDB は容量超過を切り替えて成功させてしまうため、目印のある区間だけ `window.indexedDB` を隠した（区間の外の保存先は 5.269 と同じ）。失敗は `journey` 自身の次の操作で起こし、通しの操作は増やしていない。
- 対照: 別の Step の観察を 3 つの比較に渡すと「違う」と言う。区間の外では `save-failed` の項目が 0 件。読み込みの前の別のオリジンは書き出し側と違う。
- 記録の様式: 5.269 の行に、`position` の後ろ・`tab-clicks` の前に次の 4 列を足した。

  ```text
  checkpoint=<合格>/<対象> reload=<合格>/<対象> export-import=<合格>/<対象> save-failure=<合格>/<対象>
  ```

- 実行結果（2026-10-23。元のコミット 955030c）: 4 件と一致の 2 件がブラウザ（Chromium）で通過。4 行とも `steps=20/20 crossings=3 checkpoint=9/9 reload=6/6 export-import=5/5 save-failure=7/7 external=0 page-errors=0 ledger=195/195`（`tab-clicks` は 1280px で 0、360px で 69）。`make test-browser` 全体は未検証（CI の `browser-test` で確かめる）。
- 限界:
  - 「次の章へ進む」・Step の本文・書き出しの操作の置き場は仮置きのままである（`crates/web/src/host.rs` のコメントは最終の配置を TASK-534・TASK-548 としている）。本件は配置を決めず、仮置きのまま 2 幅で通ることを確かめた。
  - macOS のファイル画面・公開サイトでの書き出しと読み込み（TASK-342）、360px での使いやすさの人の評価（TASK-387）は範囲外である。
  - 試験の件数の期待値は更新していない（件数を変えない設計）。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.271 選択肢の表示名の全 Step の件数（TASK-446。TERM-25）

- 目的: `st-1-4`・`st-2-7` を含む全 Step で、選択肢のボタンに ID の文字が出る件数が 0 件であることを、画面で数えて確かめる。確認 `answer` の選択肢の `label` は D-67 で必須になった。5.211 の TASK-65 2/2 は `validate` が欠けを止めることを担い、TASK-445 は部品が `label` を見える文字にすることを担う。本件は、その結線が通しの全場面で保たれることを数える。
- 期待と定義の対応: TERM-25（選択肢は表示名で出る）。PACK-37（`label` の無い教材パックを断る）は `crates/pack/tests/answer_label_required.rs` の `pack37_*` が担い、本件では足していない。
- 構成:
  - `tools/browser/choice-labels.test.mjs`（新設。`scripts/test-browser.sh` に登録）: `?fixture=ch0-3` を CSP あり・なしで `journey` により最後まで通す。`onStep` で、IR から数えた場面（入力方式 `choice` の入力待ち 8 場面・確認 `answer` の確認待ち 7 場面。計 15）ごとに、ボタンの見える文字・`aria-label`・`title` と選択肢の一覧の ariaSnapshot に、その Step の選択肢の ID が出る件数を足す。期待値は IR から独自に数える（画面の `data-*` を読み返して合格にしない）。
  - `crates/web/src/test_fixture.rs`: 対照の教材 `?fixture=ch0-3-nolabel`（`test-hooks` のときだけ。確認 `answer` の選択肢から `label` を外す。入力方式 `choice` の `label` は残す。単体試験 1 件）。
- 対照: `label` を外した教材では、数え方が `answer` の選択肢の総数以上の ID を検出し、`st-1-4`・`st-2-7` のボタンの文字が ID の並びになる。これは D-67 の前の「ID のまま出る」現状の観察を兼ね、数え方が 0 件を黙って返していないことの確認になる。
- 解釈: 本物の教材（教材 repo の TASK-615・TASK-617・TASK-627）は本 repo に無い。教材が揃うまでは CH-0〜CH-3 の形の試験用の教材で部品の動作を確かめる。本物の教材の全 Step の確認は、教材が揃った後に、教材 repo の CI で `validate` を流すこと（TASK-84）と、本試験を本物の IR で流し直すことで行う。`testdata/` の spec の写しは D-67 の前の形（`label` 無し）のまま残す（5.211 と同じ方針）。
- 記録の様式（1 回ごとに 1 行。値・本文は出さない）:

  ```text
  CHOICE-LABELS fixture=<ch0-3|ch0-3-nolabel> csp=<on|off> scanned=<n>/<期待> buttons=<n> id-hits=<n> st-1-4=<ok|ng> st-2-7=<ok|ng> log-id-hits=<n> ledger=<合格>/<項目>
  ```

- 実行結果（2026-10-23。元のコミット 88260ef）: 3 件がブラウザ（Chromium）で通過。`ch0-3` は CSP あり・なしの両方で `scanned=15/15 buttons=26 id-hits=0 st-1-4=ok st-2-7=ok ledger=84/84`。対照 `ch0-3-nolabel` は `scanned=15/15 buttons=26 id-hits=28 st-1-4=ng st-2-7=ng ledger=76/76`。`make test-browser` 全体は未検証（CI の `browser-test` で確かめる）。
- 限界:
  - 操作のログの受付の行は、選んだ選択肢の ID を本文に出す（`crates/web/src/log_lines.rs`。`log-id-hits` は観察値で、assert していない）。`docs/pack-authoring.md` は「操作のログの『選択: …』に同じ表示名が出ます」と書いており、食い違う。ログの直しは host の挙動とログの契約に波及するため本件では直さず、範囲外として報告した。
  - `compile` だけを通した IR では、`label` の無い選択肢が ID のまま出る経路が残る（対照がその観察）。
  - 試験の件数の期待値は更新していない（5.268 の方針）。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.273 core の依存境界と、右の描画から左の状態への参照 0 の検査（TASK-206。EVT-1・EVT-23）

- 目的: 表示層が描画の結果を読んで状態を作る経路を持たないこと（EVT-1）と、core が DOM・表示基盤を知らないこと（EVT-23）を、機械検査で固定する。CI の配線（ジョブ・make ターゲット）は変えない。CI のガードの変更にあたるため、人のレビューが必要である（承認は得ていない）。
- 置き場の判断: spec の TASK-206 は変更対象を `tools/check-deps.sh` とするが、右と左の分離は `engine_parts.rs` の impl 単位の切り出しが要り、awk では脆い。そのため `make test`（CI の rust-test）で流れる Rust の静的試験 `crates/web/tests/pane_isolation.rs` に置いた。`tools/check-deps.sh` は冒頭の対応表だけを足し、論理は変えていない。
- 規則と検査の対応:

  | 規則 | 検査 |
  |---|---|
  | core・contract の normal 依存の閉包に `web-sys`・`js-sys`・`wasm-bindgen*`・`fandhe-frontend*` が無い（EVT-23） | `scripts/check-crate-deps.sh`（`make check-deps`）。回帰は `make test-check-deps` で、実物の `Cargo.toml` に 4 系統を足すと失敗し、戻すと通ることを確かめる。dev 依存は禁じない |
  | 状態を変える経路は `Session` の操作だけ | `crates/web/tests/no_progress.rs`（TASK-347）。「web は `Session` の操作と `Projection` を呼ぶ」の検査可能な読み方はこれである。web が使う core の経路の allowlist は、タスクのたびに一覧を直す負担が利点より大きいため作らない |
  | DOM の文字列の読み取りが 0 件 | `tools/check-deps.sh`（`make check-dom-reads`）と `state_paths.rs` |
  | R1: 右の描画が左の状態を読まない | `pane_isolation.rs`。右 8 ファイルの `World`・`Mark`・`.world(`・`selected_view`・`view_choice` などと左のモジュールへの経路が 0 件（共有の定数 `ACTION_ATTR`・`PAYLOAD_ATTR` だけ可） |
  | R2: 左が右に出した文字列を読まない | 同上。左 13 ファイルの `line_from_event`・`judged_text`・右のモジュールへの経路が 0 件 |
  | R3: 両側を組む `engine_parts.rs` | 同上。`LogPart` と右の補助関数に R1、`ArtifactPart`・`ReviewPart` と左の補助関数に R2 を impl・関数の単位で適用する。切り出せなければ失敗する |
  | R4: `host.rs` が左の状態を書く形 | 同上。書き込みの形と件数を固定し、複合代入（`+=` など）と `&mut` の借用は許可した 1 件を除いて 0 件とし、新しい形が増えると失敗する |

- DOM の読み取りの例外は 1 件で、`glue.rs` の `sync_draft` の `field.value()` である。上流の束縛が textarea の現在値を同期しないため、下書きと比べて違うときだけ書き戻す。読んだ値は状態に入れない。件数は `tools/check-deps.sh`（`EXC_N=1`）と `state_paths.rs` が固定している。規則の範囲外として、入力イベントの欄（`KeyboardEvent::key`・`InputEvent::data`・`selection_start` など）と保存先の `Storage::get_item` は表示の文字列ではないため禁止の一覧に入れない。
- 実行結果（2026-10-23。元のコミット 88260ef）: `cargo test -p fandhe-workshop-web --test pane_isolation` は 16 件通過（2026-10-23 の指摘対応で 14 件から増やし、後続の impl と inherent impl の対照を足して 16 件）。`scripts/check-mutations.sh` で `EVT1-1`〜`EVT1-4` がすべて DETECTED（右が `model.world()` を読む・左が `judged_text` を読む・右が `ViewKind` を取り込む・`host.rs` に書き込みの形を足す）。`make test-check-deps` は 19 件通過。
- 限界:
  - 識別子の境界で照合し、マクロ・文字列の中は見分けない。モジュール名は `crate::`・`super::` の経路の形だけを数え、同名の変数は数えない。
  - `hello.rs` は入力欄を持つ右の部品のため右に入れた。`a11y`・`list_policy`・`parts`・`profile`・`learner_text`・`doc_view`・`shell*` は両側の共有または枠のため対象外にした。
  - 試験の件数の期待値（`scripts/expected-test-counts.txt`）は更新していない（版上げ PR ではない）。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.272 問い返し・拒否の後の下書きが連結されないことの画面の結果（TASK-444。TERM-22・JUDGE-27・D-67）

- 目的: PoC-13 の AI 代理の準備で起きた連結（問い返しの後に `/advance-phase` と打つと `/new-idea/advance-phase` になり、順序違いでなく未知のコマンドで拒否される）が、画面で起きないことを確かめる。エンジンの対処は TASK-284（5.198・5.200）で、本件は結果を画面で見る側を受け持つ。
- 5.198・5.200 との差分:
  - (A) 判定は画面の DOM だけで行う。右のログの `received` の本文（`line-body`）・`rejected` の `data-category` と理由・次の行動・`clarified` の文面と、欄の値・選択である。状態のフックは `main/st-1-4` までの通しと安全の計数にだけ使う。
  - (B) 経路 `proxy` は AI 代理の道具（`proxyd.mjs`）の操作を写す。type は `locator.focus()` の後に `setSelectionRange(len, len)` でキャレットを末尾へ置いて `keyboard.type` し、Enter の前にも同じ focus を行う。全選択は使わない。経路 `enter`・`button` は `page.keyboard` だけである。
  - (C) 外した対照（下記）。
- 構成: `tools/browser/draft-no-concat-screen.test.mjs`（新設）。3 経路 × CSP あり・なしの 6 件。期待の文面は IR（`crates/web/tests/data/ch0_3/ir.json`）から読む。`scripts/test-browser.sh` の一覧に足した。
- 場面 1（問い返しの後。3 経路）: `received` の本文が `/new-idea` ちょうど・`clarified` の文面が IR の文面・拒否の行が増えない・欄が空。続けた `/advance-phase` で `received` の本文が `/advance-phase` ちょうど（連結されない）・拒否の行がちょうど 1 件増え `wrong_order`・理由と次の行動が IR の文面・未知のコマンドの分類と文面が 0 件・Step が進まない・欄の値が残る（TASK-443 と両立）。
- 場面 2（拒否の後）: `enter`・`button` は欄の全体が選択され、全選択せずに打った `/new-idea` が連結されず `clarified` になり、拒否の行が増えず欄が空になる。`proxy` は対照として、代理の道具がキャレットを末尾へ動かすため `received` の本文が `/advance-phase/new-idea` になり、問い返しにならないことを固定する。
- 待ち: ST-1.3 の複数行の送信の後、欄が空になるのは 0 ミリ秒のタイマーの書き戻しで非同期である。待たずに打つと前の下書きに続くため、最初の送信の前に欄が空になるのを待つ（2026-10-23 に手元で観察した。待たない版は 6 件中 1〜2 件が不安定だった）。
- 手元の対照（2026-10-23。元のコミット 88260ef。コミットしない）: `crates/web/src/host.rs` の `selects_draft` を常に偽にした試験用ビルドでは、`enter`・`button` の 4 件が失敗し（拒否の後に選択されず、打った文字が連結されて問い返しにならない）、`proxy` の 2 件は通った。置換は戻した。これは 5.200 が未検証と残したブラウザ側の対照にあたる。問い返しの後も下書きを残す置換（JUDGE27-1 と同じ）の対照は行っていない。
- 実行結果（2026-10-23）: `proxy` 37 項目・`enter`/`button` 40 項目、6 件ともブラウザ（Chromium）で 3 回続けて通過。
- 限界:
  - 実 IME・Safari・Firefox での選択の挙動、0 ミリ秒のタイマーより速い打鍵、送信ボタンの経路のフォーカスの位置、人の確認は確かめない。
  - 代理の道具の type は拒否の後に今も連結を起こす。道具の操作の性質で、エンジンの規則（D-67）の不足ではない。PoC で代理の道具を再び使うときは、拒否の後の入力で全選択を併用する。
  - 上部の位置の表示は読まず、Step が進まないことは状態のフック（`engine().current`）で確かめた。
  - 試験の件数の期待値（`scripts/expected-test-counts.txt`）は更新していない（別 PR で `make update-test-counts`）。
  - 試験の作成者と実装の作成者が同じ AI である。

### 5.273 途中開始で飛ばした Step の省略（TASK-217 1/2。EVT-21・MOD-6）

- 目的: 途中の Step から始めたとき、開始の節点より前の Step が「達成」ではなく「省略」と記録されることを確かめる。
- 期待と定義の対応: EVT-21（飛ばした Step に `Skipped` を記録する）・MOD-6（前の周回の切り身は変わらない）。
- 構成:
  - `crates/core/src/begin.rs`: `Control::Begin` が、`Begun` の後・開始点の `effects` の前に、飛ばした Step を IR の順で `Skipped` にし、`Event::Skipped` を 1 件ずつ出す。
  - `crates/core/tests/begin_skip.rs`: `min-3step` を 5 Step に増やした IR（メモリ上で組む）での 7 件。
  - `crates/pack/tests/start_skip.rs`: `module-reuse-rounds` の `issue-to-merge@round2#s-6-1` で `st-3-2`・`st-4-1`・`st-5-1` の 3 件が `Skipped`、`st-6-1` が `Entered`、ほかの実体の切り身が開始の前と等しい。
  - `crates/core/tests/mutations/EVT21-1`〜`EVT21-4`: 省略しない・達成にする・未着手の条件を外す・同じ実体の条件を外す。
- 解釈: 対象は、開始の節点と同じ `unit`・同じ `scope` で、呼び出しの Step（`use`）でなく、状態が `NotStarted` の節点とした（試作 `begin_at` に合わせた）。呼び出しの Step は持ち主が呼び出し先の実体になるため省略にしない。拒否された開始（不明な ID・開始条件・世代のあふれ・効果を読めない）は、すべての関門の後で省略するため状態を変えない。状態とイベントを同じ分岐で出し、`Session::step_status` と射影の食い違いを作らない。
- 本件と 2/2（#556）の境界: 本件は省略の規則と `Event::Skipped` の発行まで。`Skipped` の件数が飛ばした数とちょうど等しいこと、同じ開始点への再度の開始で重複しないこと、履歴の適用し直し・書き出し→読み込み・再読み込みの前後での保持、件数に向けた変異は 2/2 が担う（5.274 で埋めた）。
- 実行結果（2026-10-23）: `begin_skip` 7 件・`start_skip` 1 件が通過。
- 限界: 実体ごとの途中開始と開始点の `${名前}` の置換は TASK-239、web の起動を `Session::entered_at` から `Control::Begin` へ置き換えるのは TASK-254。歩行器の上限（`TransitionLimitReached`）で止まった操作は状態を操作の前へ戻し、`on_begin` がイベント列から `Skipped` を取り消すため、ログに `Begun` は残るが `Skipped` は残らない。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.274 省略の記録（Skipped）の件数と保持（TASK-217 2/2。EVT-21・MOD-6・SAVE-4）

- 目的: 途中開始で飛ばした Step 1 つにつき `Skipped` がちょうど 1 件記録され、その件数と状態が再読み込み・書き出し→読み込み・適用し直しの前後で変わらないことを確かめる。
- 期待と定義の対応: EVT-21（飛ばした Step に `Skipped` を記録する）・MOD-6。件数の確認は native の自動試験（5 Step の教材で 3 件、`module-reuse-rounds` の実測例で 3 件）。
- 構成:
  - `crates/core/tests/begin_skip_keep.rs`: 6 件。件数の三者一致（イベント・ログ・状態）、同じ開始点への再度の開始で重複しない、別の開始点を挟むと `NotStarted` だけが省略になる、開始の後に進めても達成にならず復習に省略 3 件が残る、書き出し→読み込み・適用し直し・起動の分岐での再読み込み（3 回重ねる）・復元の後の続きでの保持。
  - `crates/pack/tests/start_skip.rs`: 実測例（`issue-to-merge@round2#s-6-1`）の書き出し→読み込み・適用し直しで 3 件のまま。
  - `crates/core/tests/mutations/EVT21-5`〜`EVT21-7`: 再度の開始で再び省略する・状態を省略にしない・復習で省略の印を消す。
- 解釈: 保存の経路は本番と同じ関数（`envelope_for_save`・`export_bytes`・`load_history`・`boot_session`）を通した。適用し直しに `Skipped` 固有の分岐は無く、保持は決定性から導かれるため、適用し直しだけを狙う変異は作っていない。
- 実行結果（2026-10-23）: `begin_skip_keep` 6 件・`start_skip` 2 件が通過。`check-mutations.sh` で `EVT21-1`〜`EVT21-7` がすべて DETECTED。
- 限界: ブラウザでの再読み込みの後の確認は TASK-247、web の起動を `Control::Begin` へ置き換えるのは TASK-254。開始の前に保存したチェックポイントへ復帰すると、切り身の状態は戻るがログは追記だけなので `Skipped` は残り、ログの件数と状態の件数は一致しなくなる。これは実体単位の復帰（TASK-241・TASK-242）の領域として扱い、本件では試験しない。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.275 承認の「直す」の候補と表示名の画面（TASK-448 1/2。TERM-28・INTRO-29・JUDGE-11）

- 目的: 承認の Step の「直す」の対象ボタンが、誤った候補を含めて宣言順に、ID でなく表示名で出ることを画面で確かめる。D-69（2026-10-21）は、語彙に候補と表示名（`revise.candidates[].id/label`）を足す案を採った。前提の TASK-62（語彙）・TASK-265（core の `revise_candidates`）・TASK-447（承認の部品）は main にある。本件は部品を `revise_candidates` に結線した。
- 期待と定義の対応: TERM-28（誤った候補を含むボタンが表示名で出る）・INTRO-29（正しい対象を画面が示さない）・JUDGE-11（誤った候補は受付にならず拒否になる。判定は core）。
- 構成:
  - `crates/web/src/offers.rs`: 候補は core の `select_judge::revise_candidates` だけから得る。見える文字は `直す（対象: 表示名）`、ID は `data-revise-target`・`data-payload` の属性値だけ（宛先の照合は ID）。候補を宣言しない Step は表示名なし（ID が文字）になる。契約（`terminal_area`）は変えていない。
  - `crates/web/tests/approval_buttons.rs`: 候補の数・宣言順・文字と属性の分離・誤った候補の拒否・正しい対象の受付・エスケープ・検査に通らない宣言で 0 件・`load-fixture:approval-candidates`。
  - `crates/web/src/test_fixture.rs`・`host.rs`: `load-fixture:approval-candidates` と `?fixture=ch0-3-revise`（ST-2.6 に候補 3 件と開始点 `s-2-6` を足した変種。いずれも `test-hooks` のときだけ。配信する設定の成果物に名前が無いことを確認した）。
  - `tools/browser/approval-revise-labels.test.mjs`（新設。`scripts/test-browser.sh` に登録）: 承認の部品・ST-2.6・宣言なしの対照を CSP あり・なしで流す。期待値は試験の定数で独自に持つ。見える文字・`aria-label`・`title` に ID が出る件数 0、対象つきボタンの属性名の集合が全候補で同じ（正誤の印が無い）、誤った候補の実クリックで拒否の行が 1 件増えボタンが残る、正しい対象で確認の段の 3 件になる、`collectSafety` が 0 件。
  - 変異: TERM28-1・TERM28-2（5.3.1f の表）。
- 対照: 候補の宣言が無い `load-fixture:approval` では文字が ID になる（決定の前の現状の観察で、数え方が ID を検出できることの確認）。
- 記録の様式（値・本文は出さない）:

  ```text
  REVISE-LABELS step=<節点> log-id-hits=<n>
  ```

- 実行結果（2026-10-23。元のコミット fc777b1 に本変更を載せた作業ツリー）: 新しい試験 6 件を含め、`approval-buttons`・`approval-flow`・`confirm-status`・`input-ports`・`reload-points`・`ch0-3-run` の計 29 件がブラウザ（Chromium）で通過した。`make test-browser` 全体は未検証（CI の `browser-test` で確かめる）。`make check-generic-words` の失敗 1 件（`crates/pack/src/guidance.rs:606`）は本件の前からある。
- 限界:
  - 操作のログの行 `承認の操作: 直す（対象: <ID>）` は選んだ候補の ID を出したままである（`crates/web/src/log_lines.rs`。`log-id-hits` は観察値で assert していない）。ログの契約・L1/L2 の一致に波及するため本件では直さず、範囲外として報告した（5.271 と同じ種類）。
  - ST-2.6 は本物の教材（教材 repo の持ち分）ではなく、CH-0〜CH-3 の形の試験用の教材の変種で確かめた。開始点からの直接の起動は前の Step の効果を持たない（5.199 と同じ限界）。
  - 差し戻し → 再承認の画面の流れは TASK-448 2/2 が担う。試験の件数の期待値は更新していない（5.268 の方針）。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.276 差し戻しの後の再承認の画面（TASK-448 2/2。TERM-28・TERM-29・JUDGE-11・PACK-32）

- 目的: D-69（2026-10-21）の形、すなわち差し戻しの Step と再承認の Step を分けて `next` でつなぐ教材で、差し戻し → 再承認が承認の部品と ST-2.6 の画面で進むことを確かめる。判定は core（TASK-265 の `judge11_reapproval_is_judged_in_a_separate_step`）、教材の形は pack（TASK-62 の `approval_revise_split`）で済んでいる。本件は画面の結果を確かめる。製品コードは変えていない。
- 期待と定義の対応: TERM-29（再承認の Step で「はい」が進み、「直す」「いいえ」は拒否になる）・TERM-28（誤った候補は拒否）・JUDGE-11・PACK-32。
- 構成:
  - `crates/web/src/test_fixture.rs`・`host.rs`: `load-fixture:approval-reapprove`（min-3step の st-1 が候補 3 件の差し戻し、st-2 が候補なしの再承認）と `?fixture=ch0-3-reapprove`（ST-2.6 に候補 3 件と開始点 `s-2-6` を足し、直後に再承認の Step `main/st-2-6b` を挿入した変種）。いずれも `test-hooks` のときだけ。配信する設定の wasm に名前が無いことを確認した。単体試験で、完全一致でだけ選ばれることと IR の形を固定した。
  - `crates/web/tests/approval_buttons.rs`: 差し戻しの後は再承認の Step へ移り、3 件（対象つき 0 件）が出て、「直す」「いいえ」は拒否でボタンが残り、「はい」で次へ進む試験（feature なし）と、`load-fixture` 版の 1 件。
  - `tools/browser/approval-reapprove.test.mjs`（新設。`scripts/test-browser.sh` に登録）: 承認の部品と ST-2.6 を CSP あり・なしで流す（4 件）。段の判別は `stateJson().engine.current` で行う。ST-2.6 は確認の段（`confirm: approve`）を経て再承認の Step へ進み、そこで再読み込みしても同じ Step・同じボタンに戻ることも確かめる。最後の Step は承認のボタンが 0 件。`collectSafety` が 0 件。
  - 変異: TERM29-1（5.3.1f の表）。
- 対照: 再承認の Step で「直す」「いいえ」が拒否になることが、「はい」で進む Step との対照になる。変異 TERM29-1 は候補 0 件の承認の Step でボタンを出さなくし、native の試験が失敗する。
- 記録の様式（値・本文は出さない）:

  ```text
  REAPPROVE step=<節点> log-id-hits=<n>
  ```

- 実行結果（2026-10-23。元のコミット ecac279 に本変更を載せた作業ツリー）: 新しい試験 4 件がブラウザ（Chromium）で通過し、`approval-revise-labels`・`approval-buttons`・`approval-flow`・`ch0-3-run` の 14 件も通過した。native は `make test`・`cargo test -p fandhe-workshop-web --features test-hooks` が通過し、TERM29-1 は DETECTED。`make test-browser` 全体は未検証（CI の `browser-test` で確かめる）。`make check-generic-words` の失敗 1 件（`crates/pack/src/guidance.rs:606`）は本件の前からある。
- 限界:
  - 本物の CH-2 ではなく、CH-0〜CH-3 の形の試験用の教材の変種で確かめた。開始点からの直接の起動は前の Step の効果を持たない（5.199 と同じ）。
  - ST-4.1・ST-6.1 の再承認は TASK-672 で確かめる。
  - 操作のログの行 `承認の操作: 直す（対象: <ID>）` は ID を出したままである（5.275 と同じ。`log-id-hits` は観察値で assert していない）。
  - 試験の件数の期待値は更新していない（5.268 の方針）。
  - 節番号は、open PR（#1769・#1772）が 5.273〜5.275 を使う見込みのため 5.276 とした。重なれば push の前に空き番号へ移す。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.277 同じ入力列の 2 回適用の決定性（TASK-218 1/3。EVT-22・EVT-16）

- 目的: 同じ IR・同じ初期状態に同じ入力列を 2 回適用して、状態・イベント列・導出の食い違いが 0 であることを性質テストで確かめ、時刻・乱数に依存しないことを機械で裏付ける。表示文字列の差し替え（TASK-218 2/3）と ID の変更の負の対照（3/3）は本件では扱わない。
- 既存との差分: `op_sequence_replay.rs`（TASK-219 1/3）は `apply` と `Session::apply` の 2 経路の一致である。本件は同じ経路を 2 回流し、導出（`Projection` のログ行・印・復習）を比べ、`==` に加えて直列化したバイト列でも比べる（ハッシュ系の集合は `PartialEq` が順序を無視するため）。
- 構成:
  - `common/trace.rs`: 1 操作の記録（`StepTrace`）と比較器（`compare_traces`）。2/3・3/3 も使える共通部品。
  - `determinism.rs`: IR と初期状態を別々に読み、1 本目の完了後に別スレッドで 2 本目を流す。`apply` と `Session` の両経路で比べる。固定の生成・誘導つきの生成を各 1,000 通り（「1,000 通り×2」を固定と誘導の 2 本と読む）、決定的な 37 操作（`script_specs`。進行・古い進行の無視・停止・移動・保存・復帰を含む）、導出が空でないことの確認、比較器の自己確認 3 件（操作の欠落・初期状態の違い・導出だけの違いを検出する）。統計は出力のみで assert しない。
  - `determinism_sources.rs`: core・contract のソース（行コメントを除き識別子の境界で照合）に、`std::time`・`Instant`・`rand::`・`std::env`・`thread_local`・`Atomic*`・`OnceLock`・`HashMap`・`HashSet` などが 0 件であることを検査する。例外は `core/src/select_judge.rs` の `HashSet`（重複除去の所属判定だけ。出力順は `Vec` の順）の 1 行（2 出現）に固定する。両 crate の `[dependencies]` に時刻・乱数の代表的な crate が無いことも確かめ、合成の入力での検出と、コメントを無視することの自己確認を持つ。
- 実行結果（2026-10-23。元のコミット 5f55358 に本変更を載せた作業ツリー）: `determinism` の 7 件が通過し、2 本の性質テストは各 1,000 通り（約 3 万操作）で一致した。所要時間は dev プロファイルで約 33 秒（2 経路 × 2 回を流すため既存の約 4 倍）。`determinism_sources` の 3 件が通過した。
- 限界:
  - 推移的な依存の閉包は `make check-deps` の領分で、本件は直接の依存（`[dependencies]`・`[build-dependencies]`・`[target.*.dependencies]`。`package` の改名と `workspace = true` の workspace 側の定義を解決）だけを見る。
  - 静的検査は識別子の境界での照合で、マクロの展開や文字列の中身は見分けない。コメントの除去は文字列・raw 文字列・文字リテラルの中の `//` をコメントと見なさない（後続の実コードを隠さない回帰ケースあり）。
  - 実行時の非決定性（時刻で分岐する等）を 1 行の置換で表す変異は作っていない。変異定義（`mutations/`）は足していない。
  - `scripts/expected-test-counts.txt` は更新していない（版上げ PR ではない）。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.278 候補が開いているときの Esc の ST-1.4 の場面と案内（TASK-452 1/2。TERM-9・JUDGE-18・D-60）

- 目的: D-60（2026-10-15）は、候補の一覧が開いているときの Esc を「1 回目で一覧を閉じるだけ、2 回目で取り消す。この順序は案内文で教える」とした。現行の仮置きと同じで、実装の変更は無い。ST-1.4 の「答えずに Esc で取り消す」経路（問い返しの後に `/ne` と打ち、候補が開いた状態）を通しで確かめていなかったので、その観察を固定する。製品コードは変えていない。
- esc-outcomes（K-09、5.x）との差分: そちらは取り消しが拒否になる Step（`min-3step`）。本件は取り消しが受け付けられる ST-1.4 で、問い返しの後の経路である。案内の文面を画面で照合する。
- 構成:
  - `tools/browser/esc-guide-st14.test.mjs`（新設。`scripts/test-browser.sh` に登録）: `?fixture=ch0-3`（基準。案内はエンジンが足さず IR の文面だけが出る）と `?fixture=ch0-3-esc-guide`（案内文を順序を教える文面に差し替えた変種）を、CSP あり・なしで流す（4 件）。操作は `page.keyboard` だけ。
  - `crates/web/src/test_fixture.rs`: `?fixture=ch0-3-esc-guide`（`test-hooks` のときだけ。`pack.id` と `clarifications.cl-new-idea-name.how_to_answer` だけを差し替える）。単体試験で、完全一致でだけ選ばれることと、案内文以外が基準と同じことを固定した。
- 確かめる項目: 問い返しの行の問いと案内が IR の文面ちょうどで、文字ノード 1 つであること（A03）。`/ne` で候補が 1 件開き、案内の行が残ること。Esc の 1 回目で一覧が閉じ、ログ・世代・Step・拒否の行・欄の値が変わらないこと。2 回目で ST-1.4 が受付になり、確認待ちの口（2,0,1,1）と左の `cards/idea` が出て、拒否の行が増えないこと。観察値として受付でログが 5 増え、最後の判定が受付で、欄の値が残ること。続けて ST-1.5 へ進めること。`collectSafety` が 0 件。
- 対照（2026-10-23）: 変種の期待の文面を基準の文面に差し替えると、変種の 2 件が失敗した（基準の 2 件は通る）。実装側の「候補が開いていれば閉じるだけ」の分岐を外した試験用ビルドでの対照は行っていない（未検証）。
- 実行結果（2026-10-23。Chromium、試験の設定の web を直接ビルド）: 4 件が通り、3 回続けて通った。native は `fandhe-workshop-web` の単体試験が通過。`make test-browser` 全体は未検証（CI の `browser-test` で確かめる）。
- 限界:
  - 本物の ST-1.4 の案内文は未作成である。教材 repo と spec の持ち分で、TERM-9 の「検討中」は本 repo では変えない。変種の案内文は試験用の仮の文面である。
  - 変種の文面を JS と Rust に二重に持つ。コメントで結び、Rust の単体試験は文面そのものではなく差し替えの範囲を守る。
  - 実 IME・Safari・Firefox・人の確認は行っていない。
  - 試験の件数の期待値（`scripts/expected-test-counts.txt`）は更新していない（5.268 の方針）。
  - 節番号は、open PR が 5.277 を使う見込みのため 5.278 とした。重なれば push の前に空き番号へ移す。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.279 修飾キー付き Enter と貼り付けの切り捨ての ST-1.4 の画面の結果（TASK-452 2/2。TERM-14・TERM-16・JUDGE-22・JUDGE-24・D-60・D-65）

- 目的: D-60 は、Ctrl・Cmd（Meta）・Alt 付きの Enter を送信にも改行にもしないと決め、上限を超える貼り付けは貼った分だけを `beforeinput` で切るとした。D-65 は、切った後のキャレットを残した貼り付けの直後に置くとした。実装は TASK-282・TASK-283 が持ち、現行のまま採るため製品コードは変えていない。教材の Step の画面での結果（ログ・世代・Step・行・候補の一覧が変わらないこと、キャレットの位置が次の Backspace・打鍵の効く場所として決定どおりであること）を固定する。
- 既存の試験との差分: `enter-submit` と `input-limit`（JUDGE-24 の J-1〜J-5）は最小の画面で、欄の値・案内・`selectionStart` を見る。本件は `?fixture=ch0-3` の ST-1.4 で、ログ・世代・Step・問い返しや拒否の行の件数と候補の一覧を見る。
- 構成: `tools/browser/modifier-enter-paste-st14.test.mjs`（新設。`scripts/test-browser.sh` に登録）。場面 A（修飾キー付き Enter）と場面 B（貼り付けの切り捨て）を、CSP あり・なしで流す（4 件）。操作は `page.keyboard` だけで、10,000 文字目へのキャレットの配置だけ `setSelectionRange` を使う。
- 確かめる項目:
  - 場面 A: `/new-idea`（修飾なしなら問い返しになる下書き）で、Control・Alt・Meta・Shift との組・Control+Alt の 7 通りの Enter が欄の値・ログ・世代・Step・行を変えず、候補も開かない。対照として Shift+Enter は改行を入れ、修飾なしの Enter は問い返しになる。`/ne` で候補が開いているときも、補完も改行も送信もせず、ログ・世代・Step・行が変わらない。
  - 場面 B: 19,990 文字の下書きの 10,000 文字目へ 20 文字を貼ると、貼った分の 10 文字だけが入り、後ろの 9,990 文字が残る。TERM-15 の案内が出て、キャレットが 10,010 にあり、自動送信が無い。次の Backspace は貼った文字を消し（案内が消えキャレットが 10,009）、次の打鍵はその位置に入る。
  - 共通: `collectSafety` が 0 件、`raw-trigger` の click が 0 回、エンジンの警告・無視が 0 件。
- 観察で固定した値（2026-10-23。Chromium）: 修飾キー付き Enter の後も候補の一覧は開いたまま（D-60 は一覧の開閉を決めていない）。
- 対照（2026-10-23）:
  - `crates/contract/src/key_decide.rs` の Enter の修飾キーの分岐を無効にすると、場面 A の 2 件が失敗し、場面 B は通った。
  - `crates/web/src/input_limit.rs` の JUDGE24-1 の置換（`if after.is_empty()` を反転）を当てると、場面 B の 2 件が失敗し、場面 A は通った。
  - JUDGE22-3 の変異の定義（`crates/web/tests/mutations/JUDGE22-3.mutation`）の置換は、`KeydownEffect` に欄 `deliver_complete` が増えたため現在はコンパイルできない（本件では直していない）。
  - JUDGE24-2（キャレットを +1）の対照は行っていない（未検証）。
- 実行結果（2026-10-23。Chromium、試験の設定と配信する設定の web を直接ビルド）: 4 件が通り、3 回続けて通った。`enter-submit`・`input-limit`・`paste-notice`・`paste`・`esc-outcomes`・`esc-guide-st14` の 67 件も通った。`make test-browser` 全体は未検証（CI の `browser-test` で確かめる）。
- 限界:
  - 上限ちょうどの途中での通常の打鍵は、観察では全体の末尾が切られてキャレットが末尾へ移る（貼り付けと違い、貼った分だけを切る経路ではない）。D-60 の対象外のため固定していない。
  - Playwright の `keyboard.press` は、Shift 以外の修飾キーが押されている間は文字を送らない。OS ごとの実キーボードの差は D-48 の別タスクである。
  - 実 IME・Safari・Firefox・他アプリからの貼り付け（TERM-20）は確かめていない。
  - 試験の件数の期待値（`scripts/expected-test-counts.txt`）は更新していない（5.268 の方針）。
  - 節番号は、open PR #1774 が 5.277 を使うため 5.279 とした。重なれば push の前に空き番号へ移す。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.280 案内の修飾キーの表示の右のエリアでの結線と OS によらない 4 操作（TASK-456。TERM-38・JUDGE-28・D-48）

- 目的: 修飾キー（Cmd／Ctrl）の出し分けの機構は TASK-285 が実装済みである。本件は、その表示が枠（TASK-517 の 2 列とタブ）の右のエリア `shell-right` の中で働くことと、Enter 送信・Shift+Enter 改行・Tab 補完・Esc 停止の結果が OS によらず同じことを自動ブラウザ試験で固定する。製品コードは変えていない。
- 既存の試験との差分: `modifier-key-hint` は幅 1024px で文言・欄の値・同一ノードを見るが、右のエリアの中にあること、幅 360px のタブとの関係、4 操作の OS 間の一致は見ていない。
- 構成: `tools/browser/modifier-key-right-pane.test.mjs`（新設。`scripts/test-browser.sh` に登録）。CSP あり・なしで 6 件ずつ（12 件）。User-Agent は `openPage` の `userAgent` で差し替える。
- 確かめる項目:

| 場面 | 内容 |
|---|---|
| (A) 幅 1280px × Windows・macOS・Linux の UA | `key-hint`・`os-select` が各 1 件で `shell-right` の中にあり、`shell-left`・`shell-top` の中に無い。文言と欄の値が UA に合う。選ぶ欄で切り替えると文言が変わり、`shell-right`・案内・選ぶ欄が同じノードのまま、Step の本文が変わらない |
| (B) 幅 360px × macOS・Windows の UA | 既定の「操作」で案内が見え、「成果物」で隠れ（件数は 1 のまま）、戻すと見える。選び直した値と文言がタブの往復で保たれる |
| (C) 幅 1280px × macOS・Windows・Linux の UA と「macOS から windows へ選び直した後」 | 同じ列（Enter 送信・Shift+Enter・Tab 補完・Esc 停止）の観察が 4 条件で `deepEqual` し、条件ごとの期待（行が増える・下書きが `a\nb`・`/help`・停止で世代 +1 と停止の行 1 件）にも合う |

- 共通: `collectSafety` の 4 計数（CSP 違反・pageerror・console error・ダイアログ）が 0 件。失敗の文面に UA の全文を出さない。
- 観察で固定した値（2026-10-23。Chromium）: 試験の教材の最初の自由入力（`hello`）は拒否になり、行は増えるが下書きは選択状態で残る。そのため (C) は「下書きが空になる」ではなく「行が増える」で送信を確かめる。
- 対照（2026-10-23）: 右のエリアの選択子を `shell-left` に向けると (A) の 6 件が失敗した（(B) と (C) の 6 件は通った）。
- 実行結果（2026-10-23。Chromium、試験の設定と配信する設定の web を直接ビルド）: 12 件が通り、3 回続けて通った。`make test-browser` 全体は未検証（CI の `browser-test` で確かめる）。
- 限界・未実施:
  - **未実施（Windows 実機）**: Windows の実機・人の目での表示の確認は行っていない。User-Agent の差し替えによる模擬である（TASK-20・TASK-286・TASK-395）。
  - **未実施（人の確認）**: 推定が外れたときに受講者が選ぶ欄で表示を直せるかの人の確認は行っていない（TASK-393・TASK-460）。自動試験の合格は人の評価に数えない（D-34）。
  - 確かめた対象は `key-hint` と `os-select` だけである。ヒントの画面（SHELL-14 の `shell-hint`）は未描画で、結線の確認は実装するタスクで同じ試験に足す。Step の本文に作成者が書いた Cmd／Ctrl は切り替わらない（差し込みの仕組みは機構の側の判断）。
  - 範囲外の記録: 右のエリアの識別子の契約の一覧（`crates/web/src/terminal_area.rs`）に `key-hint`・`os-select` が無い。
  - 試験の件数の期待値（`scripts/expected-test-counts.txt`）は更新していない（5.268 の方針）。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.281 表示文字列の差し替えからの独立（TASK-218 2/3。EVT-2・EVT-22・P-7）

- 目的: 教材の表示文字列 31 個（教材の題 1・Step の題 3・選択肢の表示名 2・ヒント 5・拒否の文面と次の行動 10×2）を差し替え用の表 A・B に置き換えても、状態・イベント列・導出が変わらず、右の文字列だけが変わることを確かめる。製品コードは変えていない。
- 1/3（5.277）との差分: 1/3 は同じ IR への 2 回適用。本件は元・A・B の 3 つの IR に同じ入力列を流す。ID の変更の負の対照は 3/3。
- 構成:
  - `common/text_table.rs`（新設）: 表（`testdata/engine-event-consistency/test-data/text-table-{A,B}.json`。写しは変更していない）の読み込み、IR の表示文字列の収集と差し替え、構造の比較用の空にした写し、`Judged` の次の行動を文面の ID に写す `mask_display`。差し替えはキーの集合の一致と各キーが 1 か所に当たることを assert する（fail-closed）。
  - `text_independence.rs`（新設）7 件: 表の網羅（31 個・値が互いに素・元の文字列が残らない・空にした IR が一致）、差し替えの fail-closed、決定的な 37 操作の一致（純粋関数と `Session` の両経路）、空振りでないことの確認、固定・誘導つきの各 1,000 通り（純粋関数の経路のみ）、写した後も比較器が差を検出することの自己確認。
- `Judged.next_action` を ID に写して比べる理由: `Event::Judged` の拒否・問い返し・受付の注記は、差し込み後の次の行動（表示文字列）を保持する。spec の「イベント列の差 0」は `Judged` の導入（TASK-267）前の記述で、文字どおりには成り立たない。そこで次の行動だけを文面の ID に写し、他の欄は写さず比べる。表示文字列が他の経路で入り込めば不一致になり、記録を直列化した文字列に表 A・B の文字列が現れないことも確かめる。spec 側の記述の更新は spec repo の持ち分で、本 repo では変えない。
- 空振りでないことの確認: 写す前の記録は元と A・B で食い違う。`Judged` の次の行動・`filled_texts_of_log` の文面・復習の項目の Step の題・ヒントの本文・選択肢の表示名・教材の題が、それぞれの表の値に等しく 3 つで異なる。文面の ID の列は 3 つで同じである。
- 実行結果（2026-10-23。dev プロファイル）: 7 件が通過。所要時間は約 48 秒。統計は固定の生成が 33,131 操作・拒否などの判定 23,461 件、誘導つきが 29,324 操作・判定 16,143 件（出力のみで assert しない）。`make lint` が通った。
- 限界:
  - 復習（web の `ReviewPart`）は経路と数だけを描き表示文字列を持たない。core で観察できる「復習の文字列」は、復習の項目から引いた Step の題だけである。
  - EVT-2 の自動ブラウザの証拠（左の行が一致し右の文字列が変わる）は未作成（TASK-218 の変更対象外）。
  - 差し替えは 31 の場所だけで、それ以外の教材の文（確認の理由・本文ファイル）は対象外。`clarifications` は空の教材のため問い返しの文面は未検証。
  - 経路の一致は TASK-219 の持ち分のため、1,000 通りは純粋関数の経路だけを流す。
  - 変異定義（`mutations/`）は足していない。`scripts/expected-test-counts.txt` は更新していない（5.268 の方針）。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.282 文言ではなく ID を変えると差が出る負の対照（TASK-218 3/3。EVT-2・P-7）

- 目的: 5.281 の比べ方（`Judged` の次の行動を文面の ID に写して比べる）が、何でも一致にしてしまう鈍いものでないことを確かめる。拒否の文面の ID だけを付け替えた IR（表示文字列は同一）に同じ入力列を流すと、同じ比べ方で差が出る。製品コードは変えていない。
- 構成:
  - `common/text_table.rs`: `load_value`（JSON の IR を読む。`load_variant` はこれを呼ぶ）、`rejection_refs`、`rename_rejection_id`（`rejections` のキーと、全体の文字列値の完全一致を付け替える。存在しない ID・参照 0 件・新 ID の衝突・付け替え後に旧 ID が残る場合は止める）を追加。
  - `id_change_control.rs`（新設）6 件: 表示文字列が変わらないこと、元・A・B の各表で同じ比べ方が差を検出し理由が `events` か `log` を指すこと（純粋関数と `Session` の両経路）、差が ID の置き換えだけであること、`Judged` の ID が付け替えに追従すること、判定に流れない ID の付け替えは差にならないこと、付け替えの fail-closed。
- 対象の ID: 元の IR に 37 操作を流して `Judged` に現れた拒否の文面の ID を実行時に求め、1 件ずつ合成の ID（`zz-renamed-<連番>`）へ付け替える。
- 局所性の検査の理由: `load_ir` は拒否の参照の解決を検査しない。付け替えが不完全でも読め、エンジンが文面の未解決の経路に入って別の理由の差が出うる。そこで写す前の記録を直列化し、新 ID を旧 ID へ戻すと元とバイト列が一致することを確かめる。
- 実行結果（2026-10-23。dev プロファイル）: 6 件が通過、所要時間は 1 秒未満。手元の変異（付け替えの参照の置き換えを外す）で 5 件が失敗することを確認し、元に戻した。判定に流れない参照つきの ID は 1 件。
- 限界:
  - ヒント・選択肢・Step の ID は対象外（選択肢と Step の ID は入力列・初期状態が参照するため、対照として純粋にならない）。問い返しの文面の ID も、教材の `clarifications` が空のため対象外。
  - 性質テスト（1,000 通り）は足していない。変異定義（`mutations/`）は足していない。`scripts/expected-test-counts.txt` は更新していない（5.268 の方針）。
  - EVT-2 の自動ブラウザの証拠は対象外。

### 5.283 CH-4 から CH-9 の ST-9.1 までを自動ブラウザで通す（TASK-674 1/2。ROUND1-1・ART-24・SHELL-10）

- 目的: ROUND1-1 は、CH-4〜CH-9 の新しい章の画面（ボタン・行・指す操作）がブラウザで確かめられていないことを理由の 1 つにしている。配信する設定のビルドを自動ブラウザで開き、CH-4 の先頭から CH-9 の ST-9.1 の達成まで通し、ページ内エラー・「次の章へ進む」の個数・模擬ログインのパスワード欄の不在を数える（PoC の `ch-run` と同じ形）。
- **解釈（最大の限界）**: 教材 repo の本物の CH-4〜CH-9（TASK-672）はまだ無い。そのため 5.117 の `ch0-3-run` と同じく、章ごとの入力方式・確認の種類・拒否・処理中・ビューの種類の**形**だけを写した試験用の教材（`crates/web/tests/data/ch4_9/ir.json`。17 Step・6 章。`pack.id` は `ch4-9-run`。出所と作り方は同 `README.md`）で通した。受け入れ条件は「形」について満たすもので、**本物の教材での流し直しは未実施**である。AI の代理の確認であり、人の評価には数えない（D-34）。
- 構成:
  - `crates/web/tests/data/ch4_9/ir.json`: 各章の最後を `final`、開始点を章の先頭 6 件（CH-10 の開始点なし）にした教材。`test_fixture.rs` が `?fixture=ch4-9`（完全一致だけ）で選ぶ。
  - `crates/web/tests/ch4_9_run_host.rs`（native）: 画面から送れる手だけで 17 Step を進め、各手の判定・現在の Step・状態と「次の章へ進む」の提示の数（次が章の移動の手のときだけ 1、他は 0）を確かめる。処理中の送信が `busy` で拒否されることは、ブラウザでは時間の窓に頼るため、ここで進行を止めたまま確かめる。
  - `tools/browser/ch4-9-journey.mjs`（誘導。`journey49`・`passwordFields`・2 つの観察）と `tools/browser/ch4-9-run.test.mjs`（試験。`scripts/test-browser.sh` に登録）。
- 確かめる項目:

| 試験 | 内容 |
|---|---|
| A（試験の設定。CSP あり・なし） | 実クリックと実キーだけ（`raw-trigger`・`load-fixture:` は 0 回）で 17 Step を CH-9 の達成まで通す。警告・無視・省略が 0 件、全 Step が達成、安全の計数・ページ内エラー・コンソールの error・CSP 警告・配信元以外への通信が 0 件。「次の章へ進む」は CH-4〜CH-8 の各章の最後の達成の後にちょうど 1 個（文字は「次の章へ進む: 」＋次の章の先頭の題、「移動」ボタンは 0 件）で 5 回押し、章の途中と ST-9.1 の達成の後は 0 個。全 Step の画面でパスワード欄が 0 件。最後の位置の表示が CH-9 |
| B（配信する設定。CSP あり・なし。**本体**） | `FANDHE_WEB_DIR` の一時の写しに `fandhe-ir` の meta と `ir/main.json`（`ir.json` のバイト列）を足し、`__fandheTest` が無いこと（試験用の口がないこと）を確かめたうえで、DOM だけの観察で A と同じ通しを通す。復習の行で 17 Step が達成・省略 0 件、`ir/main.json` が 200 で 1 回取得、上の 0 件の項目が同じ |
| C（較正） | B と同じ DOM の観察が、各時点で試験の設定の `stateJson` の現在の Step・状態と一致し、同じ時点で次の Step と同じ Step の別の状態を示さない（B の空振りの防止） |
| D（検出器の対照） | 模擬ログインの Step の画面に、`type=password`・`autocomplete=current-password`・「パスワード」のラベルつき・`type=PASSWORD` の欄を 1 件ずつ足すと検出数が 1 になり、除くと 0 に戻る |

- 「次の章へ進む」が 5 回である根拠: 出る条件は、現在が `main` の `final` の Step で達成・処理中でなく、次の章の先頭を指す開始点が `starts` にあること（`next_chapter.rs`）。CH-9 の後は次の章の開始点を置かない（試行の IR から範囲外の開始点を除く D-67。2 周目の開始は TASK-254）ので 0 個とした。
- パスワード欄の検査について: 製品の左の成果物は、どのビューの種類でも世界状態の経路を行として描くだけで、`app_demo` の入力欄つきのワイヤー（`app_demo_view.rs`）は製品の `view()` に結線されていない（TASK-470・TASK-347 の後続）。このため「パスワード欄 0 件」は、結線されるまでは構造上ほぼ自明に通る。検出器が働くことは D で別に示した。結線された後も同じ検査が働くよう、全 Step の観察で数える。
- 観察で固定した値（2026-10-23。Chromium）: ログに「達成」の行は出ず、受付は `received`、拒否は `rejected` の行になる。`step-status` は入力待ちで空（章の先頭）か直前の達成の語が残る（章の途中）。DOM の観察はこの値に合わせた（`inspectDom49` の注記）。
- 実行結果（2026-10-23。Chromium 153.0.8010.12、試験の設定と配信する設定の web を `SKIP_WASM_OPT=1` で直接ビルド）: A 92 項目×2、B 84 項目×2、C 225 項目、D 87 項目がすべて通った（全体で約 17 秒）。native の `ch4_9_run_host` 4 件と `test_fixture` の単体 1 件も通った。
- 確かめないこと・限界:
  - **未実施（本物の教材）**: TASK-672 の本物の CH-4〜CH-9 での流し直し。教材 repo ができた後の作業である。分岐の Step 化・止まる場面の主経路への置き方・実際の保存と書き出しの Step・教材の文面は写していない。
  - 配信元以外への通信の正式な計数と記録は TASK-674 2/2 の範囲である（→ 5.285）。ここでは各試験の台帳に「0 件」の 1 項目を置いただけで、`csp-runs` には登録していない（`journey49` の `onStep` の口と `?fixture=ch4-9` を引き渡す）。
  - 全周のキーボード完走は TASK-92 の範囲である。本試験は実クリックの通しだけを確かめる。
  - 試験の件数の期待値（`scripts/expected-test-counts.txt`）は更新していない（5.268 の方針）。
  - `make test-browser` 全体は未検証（CI の `browser-test` で確かめる）。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.284 再描画 1 回の速さ（ログ 100〜119 項目。TASK-223 1/2。EVT-24）

- 目的: 最小教材でログが 100〜119 項目の状態にして、再描画 1 回（隠し要素の click 1 回の同期的な処理）の 95 パーセンタイルが 16ms 以内かを、CPU の減速なしで測る。Rust は変えていない。
- 方式（`tools/measure/log-redraw.sh`、`tools/measure/browser.mjs --kind log-redraw`。条件の詳細は `docs/measurement.md` §4）: 20 試行 × 10 操作の計 200 値。操作は拒否される入力（`/make-note Bad_Name`）で、1 回でログが 2 件増える。`{control:'save'}` はログが 1 件ずつ増えるが画面の行に出ないため使わなかった（事前の確認で、click の後に `li` が増えないことを見た）。
- 同期性の確認: 全操作で、ログの長さが窓の中で 2 件増え、`li` が click から戻った時点で 2 件増え、上限の通知が無いことを確かめた。
- 結果（2026-10-23。開発機。Chromium 153.0.8010.12・Playwright。`SKIP_WASM_OPT=1`。試験の設定。実装の作業ツリー上の測定で `dirty: true`、基点の commit は 2dbb601）:

| 測定 | 値 | 判定 |
|---|---|---|
| `log_redraw_median` | 2,100 us | |
| `log_redraw_p95` | 2,400 us | 16,000 us 以内 |
| `log_redraw_max` | 3,800 us | |

- 基点: commit 2dbb601（作業ツリーに本件の変更あり）・`wasm-opt` 未適用（`SKIP_WASM_OPT=1`）・機能の範囲 test（`test-hooks`）・圧縮は `tools/serve.mjs` の gzip レベル 9。教材は `min-3step` 0.0.1（Step 3 件、IR の SHA-256 は `5b653cce...6961`）。端末は Linux x86_64（13th Gen Intel Core i7-13700K、論理コア 10、メモリ約 29GB）。
- 最小教材の値で、教材全体を代表しない。`performance.now()` の分解能は約 0.1ms で、値はこの粒度に丸まる。
- 限界・未確認:
  - **未実施（基準端末 A）**: 上の端末は自動運転の開発機で、基準端末 A（Apple シリコンの macOS）ではない。受け入れ条件の「基準端末 A」は未達である。追記は人が行う。手順: macOS（Apple シリコン）で `npm ci && npx playwright install chromium` の後、`FANDHE_BROWSER_CHANNEL=chrome tools/measure/log-redraw.sh | jq .`（`baseline.device.apple_silicon_macos` が `true` になる）。
  - Chromium の headless だけで測った。Chrome の原本・Windows（TASK-224）は未測定。
  - 「入力から描画まで」は #563（TASK-223 2/2）で扱う。ログの長さによる傾き（TASK-232）は対象外。
  - `make measure` の束・`scripts/expected-test-counts.txt` には入れていない。回帰は `make test-measure`。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.285 CH-4〜CH-9 の通しでの配信元以外への通信の計数（TASK-674 2/2。ROUND1-1・SAFE-13）

- 目的: CH-4〜CH-9 の run で配信元以外への通信が 0 件であることを、TASK-151 の道具（`harness.mjs` の `openPage`・`collectSafety`・`cspConsoleMessages`・`requestKinds`、`csp-runs.test.mjs` の `runScene`・`assertClean`）で本タスク自身が数える。TASK-152 の結果には頼らない。
- 構成（`tools/browser/csp-runs.test.mjs` に 2 場面を登録。`scripts/test-browser.sh` は登録済みのため変更なし）:
  - T5（試験の設定。`?fixture=ch4-9`）: `journey49` で 17 Step を実クリックと実キーで通す。各 Step の観察ごとに走査（`assertClean`）し、章ごとに配信元以外への通信・CSP の違反・WebSocket の接続の増分を表にして 0 を主張する。
  - D2（配信する設定）: `withDeliveryCopy`（`ch4-9-journey.mjs` へ移設。5.283 の B と共有）の写しに `ir/main.json` を置き、DOM だけの観察で同じ通しを行う。復習の行で 17 Step の達成・省略 0、`ir/main.json` が 200 で 1 回を確かめる。
  - 両場面とも CSP あり → なしの順で流し、配信元への要求の種類が同じであることと、要求を観察できたこと（空振りでないこと）を `runScene` が確かめる。
  - `runScene` に `dir`・`prepareDir`・`setup`・`failedFilter` の口を足し、`assertClean` に失敗の除外の口を足した。既存の D1・T1〜T4 の挙動は変えない。
  - 失敗の除外は 1 形だけ。IR の取得が 200 の後に同じ要求だけ `net::ERR_ABORTED` になる競合（5.268）で、`l1-l2-runner.mjs` の `irFetchCheck` をそのまま使う。外部通信は除外の対象にしない。
- 実行結果（2026-10-23。基点は `origin/main` の `7eafc45`。Chromium は Playwright 同梱。web は `SKIP_WASM_OPT=1` で試験の設定・配信する設定を別々に作成）:

| 場面 | CSP | CH-4 | CH-5 | CH-6 | CH-7 | CH-8 | CH-9 | 配信元への要求 | 走査 |
|---|---|---|---|---|---|---|---|---|---|
| T5 | あり | 0 | 0 | 0 | 0 | 0 | 0 | 6 件 | 33 回 |
| T5 | なし | 0 | 0 | 0 | 0 | 0 | 0 | 6 件 | 33 回 |
| D2 | あり | 0 | 0 | 0 | 0 | 0 | 0 | 6 件 | 33 回 |
| D2 | なし | 0 | 0 | 0 | 0 | 0 | 0 | 6 件 | 33 回 |

  - 表の数字は章ごとの配信元以外への通信の件数。CSP の違反・WebSocket・ページ内エラー・コンソールの CSP 警告も全回 0 件。
  - `csp-runs` は 8 件すべて合格、`ch4-9-run`（`withDeliveryCopy` の移設後）は 6 件すべて合格。
  - 陽性の確認: T5 の CH-6 の観察の直前に `127.0.0.1:9` への `fetch` を 1 件起こすと T5 が失敗した（検出器が働く）。この変更はコミットしていない。検出器自体の対照は `harness.test.mjs`・`safety-control.test.mjs` が受け持つ。
- 限界:
  - 本物の CH-4〜CH-9（TASK-672）が無いため、形だけを写した試験用の教材（17 Step・6 章）で数えた。本物での流し直しは未実施。AI の代理の確認であり、人の評価には数えない（D-34）。
  - WebSocket 以外の経路（Service Worker 等）は道具の範囲に依る。
  - `scripts/expected-test-counts.txt` は更新していない（`csp-runs` が 6 件から 8 件になる。後で `make update-test-counts`）。
  - `make test-browser` 全体は未検証（CI の `browser-test` で確かめる）。
  - 5.180 の「配信する設定は実行時に教材を読み込まない」は、5.283 の B 以後、配信する設定の写しに `ir/main.json` を置けば読み込める。D1〜T4 を配信する設定へ広げるのは TASK-152 か別の範囲である。
  - `pack-body-xss.test.mjs`・`ir-recv-limit.test.mjs` に `withDeliveryCopy` 相当の重複が残る（本件では触れない）。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.286 入力から描画までの速さ（ログ 120〜158 項目。TASK-223 2/2。EVT-24）

- 目的: 最小教材でログが 120〜158 項目の状態にして、入力（送信ボタンの信頼された click）から描画（次の `requestAnimationFrame` の中の `setTimeout(0)`）までの 95 パーセンタイルが 100ms 以内かを、CPU の減速なしで測る。Rust は変えていない。
- 方式（`tools/measure/input-paint.sh`、`tools/measure/browser.mjs --kind input-paint`。条件の詳細は `docs/measurement.md` §4）: 10 試行 × 20 操作の計 200 値。操作は送信ボタンの CDP の信頼された click で、入力欄の拒否される下書き（`/make-note Bad_Name`）は欄に残るため同じ本文が毎回送られ、1 回でログが 2 件増える。PoC の停止ボタンは、現行の実装では処理中でなければ状態を変えない（`crates/web/src/hello.rs` の注記）ため使わなかった。この点は注記の読みによる判断で、停止ボタンの click 単体でログが増えないことの実測は行っていない（未確認）。
- 検査の確認: 全操作で、`isTrusted` が真、ログの長さが窓の中で 2 件増え、`li` も 2 件増え、Step が動かず、下書きが変わらず、上限の通知が無く、`fromEvent >= fromCapture >= 0` であることを確かめた（外れれば測定が失敗する。今回の測定は全 200 操作で通った）。
- 結果（2026-10-23。開発機。Chromium 153.0.8010.12・Playwright 1.63.0。`SKIP_WASM_OPT=1`。試験の設定。実装の作業ツリー上の測定で `dirty: true`、基点の commit は 586754b）:

| 測定 | 値 | 判定 |
|---|---|---|
| `input_paint_median` | 16,100 us | |
| `input_paint_p95` | 16,600 us | 100,000 us 以内 |
| `input_paint_max` | 16,800 us | |
| `input_paint_capture_p95`（参考。キャプチャ起点） | 16,300 us | |

- 値が約 16ms に揃うのは、終点が次のフレームの描画に結び付くためで、60Hz の 1 フレームの長さに当たる。
- 基点: commit 586754b（作業ツリーに本件の変更あり）・`wasm-opt` 未適用（`SKIP_WASM_OPT=1`）・機能の範囲 test（`test-hooks`）・圧縮は `tools/serve.mjs` の gzip レベル 9。教材は `min-3step` 0.0.1（Step 3 件、IR の SHA-256 は `5b653cce...6961`）。端末は Linux x86_64（13th Gen Intel Core i7-13700K、論理コア 10、メモリ約 29GB）。
- 最小教材の値で、教材全体を代表しない。`performance.now()` の分解能は約 0.1ms で、値はこの粒度に丸まる。
- 限界・未確認:
  - **未実施（基準端末 A）**: 上の端末は自動運転の開発機で、基準端末 A（Apple シリコンの macOS）ではない。受け入れ条件の「基準端末 A」は未達である。追記は人が行う。手順: macOS（Apple シリコン）で `npm ci && npx playwright install chromium` の後、`FANDHE_BROWSER_CHANNEL=chrome tools/measure/input-paint.sh | jq .`（`baseline.device.apple_silicon_macos` が `true` になる）。
  - Chromium の headless だけで測った。Chrome の原本・Windows（TASK-224）は未測定。コミット後の `dirty: false` での再測定、`wasm-opt` 適用ありの測定も未実施。
  - ログの長さによる傾き（TASK-232）は対象外。
  - `make measure` の束・`scripts/expected-test-counts.txt` には入れていない。回帰は `make test-measure`。
- 限界（作成者）: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない。

### 5.287 validate の出力・終了コード・読み込み失敗時の打ち切り（TASK-403。AUTHOR-2・AUTHOR-3）

`fandhe-pack validate <pack-dir>` を `source::load` の成否で分岐させて結線した。契約は `docs/cli.md` 版 8 の 8 節。試験は `crates/pack/tests/validate_command.rs` の 8 件（`author2_` 4 件・`author3_` 4 件。Unix では `author3_symlink_is_unreadable` を含む）と、`tests/cli.rs` の `i16_validate_is_dispatched_to_validate`、`cli.rs` の単体試験 `c1_validate_missing_dir_is_unreadable` である。

- 診断の行は `validate::validate` の結果と完全に一致し、最終行は `cli_contract::final_line` と完全に一致する。診断 0 件で 0、1 件以上で 1、標準エラー出力は空
- 読み込みに失敗する 6 形（構文の誤り・未知の欄・型の違い・必須欄の欠け・深さの上限・シンボリックリンク）と存在しないパスは、`NG: 診断 N 件（読み込めません）` で終わり、検査は流れない。構文の誤りと参照の欠陥を同時に持つパックの負の対照では、`E-REF-`・`E-UNREACHABLE`・`E-DEAD-END`・`E-STRUCTURE` が出ず、構文の誤りだけを直した写しでは `E-REF-STEP` が出る
- 実測で分かったこと: `E-FORMAT-VERSION` は `source::load` が先に出す（`probe_format_version`）ため、CLI では `読み込めません` 側になる。計画の想定（読み込めた側）は誤りだったので、試験と文書を実測に合わせた（`author3_format_version_mismatch_is_unreadable`）
- 「診断の末尾の案内」は出力に行を足さず、`docs/cli.md` の 8 節と `docs/pack-authoring.md` に書いた（出力は「診断の行の後に最終行がちょうど 1 行」の契約のまま）
- `scripts/expected-test-counts.txt` は本変更では更新しない（5.229 の前例）
- 既知の不足: `AUTHOR1-3.mutation` は main の時点で `find` が一致せず実行できない（`Subcommand::Compile` の引数の形が `--chapters` を含む形に変わったため）。本変更の範囲外

変異注入（実行日: 2026-10-23、元にしたコミット: 2c43e49 に本変更を載せた作業ツリー、コマンド: `scripts/check-mutations.sh crates/pack/tests/mutations/AUTHOR3-*.mutation crates/pack/tests/mutations/AUTHOR2-*.mutation crates/pack/tests/mutations/AUTHOR1-{1,2,4,5,6}.mutation`）。

| ID | 対象 | 期待した失敗 | 結果 |
|---|---|---|---|
| AUTHOR3-1 | crates/pack/src/cli.rs: 読み込みの失敗の分岐が別の経路の診断も書く | author3_unreadable_causes_end_with_unreadable_line | DETECTED |
| AUTHOR3-2 | crates/pack/src/cli.rs: 診断があっても終了コードが 0 になる | author2_defective_pack_prints_diagnostics_then_ng_line_and_exits_1 | DETECTED |
| AUTHOR2-1〜2-4 | 既存 | 既存 | すべて DETECTED |
| AUTHOR1-1・1-2・1-4・1-5・1-6 | 既存 | 既存 | すべて DETECTED（1-4・1-6 は形の誤りが読み込みの前に報告されること） |

集計: 11 件すべて DETECTED（2026-10-23）。

### 5.288 未信頼の値の表示の試験の骨格と負の対照（TASK-482 1/2。ART-6）

世界状態の値に HTML・スクリプト・`javascript:` のリンク・属性の閉じ忘れの形を入れた行（escape の行。`testdata/artifact-view-parts/test-data-addendum/escape-rows.json`）を、部品に直接渡して表示させ、値が文字として出ることを数える検査器の骨格と、差を検出できることの負の対照を作った。部品レベルの試験であり、教材パック・IR からの注入は TASK-145 の担当である。24 通りすべての合格と記録は TASK-482 2/2（#675）に渡す。

- 口: 代役の部品（`crates/web/src/view_fixture.rs`。試験の設定だけ）の `fixture_data` が、文書以外の 6 種の `<種類>:escape`（`filetree`・`diff`・`tasks`・`cards`・`checks`・`appdemo`）と、戻すための `filetree:sample`・`diff:sample`・`tasks:sample` を受ける。閉じた集合で、`document:escape`・大文字違い・空白つきは拒否する。配信する WASM には入らない
- 検査器: `tools/browser/artifact-view-check.mjs` の `runEscapeCase`（前 → 後 → 前）。固定の違反文言は次の 4 つ（値は埋め込まない）
  - `item count or keys differ from the escape rows`（件数と key の並びが escape の行から独立に数えた期待と違う。前 3・後 4）
  - `dangerous elements or attributes are present`（root の下の `DANGER_SELECTOR` が 1 件以上）
  - `an escape payload was executed`（`window.__escape_hit` が代入された。`installEscapeTrap` が `rec.hits` にも `escape:<番号>` で送る）
  - `a value is not shown as literal text`（行の欄の文字が、項目の `textContent` か中の `input`・`textarea` の値に含まれない。実体参照の形が復号されずに出ることもこれで確かめる）
  - 外部通信・CSP 違反・ダイアログは呼び出し側が `collectSafety`・`nonZero` で主張する。文字として確かめる欄は `literalFields`（既定はラベル、カードはラベル・補足・タグ）
- ネイティブの単体試験（`view_fixture` の 3 件）: payload の復号、行数（前 3・後 4）と sample へ戻せること、6 種 × 2 変種 × 前後の 24 通りで `<img`・`<script`・`<iframe`・`<svg` が HTML に出ず `&lt;img` が出ること
- ブラウザ試験（`tools/browser/view-escape.test.mjs`。11 件）

| ID | 内容 | 期待 |
|---|---|---|
| E1 | CSP あり・なし × カード (a)・(b)（4 件） | 違反 0・件数 `[3, 4, 3]`・全計数 0・CSP のコンソール警告 0・`__escape_hit` 未定義 |
| E2 | 行を切り替えずに流す／不正な payload | 件数と文字の違反が出る／拒否される |
| N1 | PoC-8 の対照の文字列を不活性の文書の `innerHTML` に入れる | `innerHTML` はちょうど 3、`textContent` は 0、実行 0、外部通信 0 |
| N2 | CSP あり・なしで、後の状態の e1 のラベルを表示された文字で `innerHTML` に入れ直す | 両方で危険な要素と文字の違反。CSP なしは `rec.hits` に `escape:3`・`rec.csp` 0、CSP ありは `rec.csp` 1 件以上・`rec.hits` 0 |
| N3 | CSP あり・なしで `alert` と外部への `fetch` | ダイアログ 1。CSP なしは外部通信 1、CSP ありは CSP 違反 1 件以上 |

- N2 で CSP の有無により `hits` と `csp` が入れ替わることが、CSP の有無が実際に効いていることの証拠である
- 実行結果（2026-10-23。Chromium headless、`TEST_HOOKS=1`・`SKIP_WASM_OPT=1` のビルド）: `view-escape.test.mjs` 11 件・`artifact-view-controls`・`artifact-view-check`・`card-view` の 46 件が全通過。ネイティブの `view_fixture` の 37 件が全通過
- `scripts/expected-test-counts.txt` は本変更では更新しない
- #675 に渡す事項: 6 ビューごとの「文字として出る欄」の調整、24 通りすべての流し込みと記録（`ESCAPE_CASES` に 12 組を用意した）。ネイティブの 24 通りの試験は全通過で、どのビューも escape の行を落とす・解釈する欠陥は見つかっていない
- 限界: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない

### 5.289 24 通りの未信頼の値の表示（TASK-482 2/2。ART-6）

文書を除く 6 ビュー × 2 変種 × 前後の 24 通りすべてで、escape の行の値が文字として出て、危険な要素・属性と実行が 0 件であることを、CSP あり・なしの両方で確かめた（`tools/browser/view-escape.test.mjs`）。部品レベルの試験であり、教材パック・IR からの注入は TASK-145 の担当である。

- 文字として確かめる欄（`literalFields(kind, row)`）。レンダラのコードで値が文字になることを確かめた欄だけを数え、未知の種類は例外にする

| ビュー | 確かめる欄 | 確かめない欄と理由 |
|---|---|---|
| `file_tree`・`task_tree`・`check_list` | ラベル・補足 | state は閉じた語彙の記号・語・字形に変換される |
| `card_list` | ラベル・補足・タグ | state は固定の語に変換される |
| `diff` | ラベル | 補足は `:` で分けた行番号の数字に変換される。state は記号に変換される |
| `app_demo` | field・item の行はラベル・補足、他の行はラベルだけ | state は固定の文字に変換される |

- `app_demo` の field は、(a) では wireframe の入力の文字に、(b) では `input` の `value` に補足が出る。属性の閉じ忘れの形（e4 の補足）が `value` に入って属性にならないことは、この欄の照合で初めて確かめられる
- 空振りの防止: `runEscapeCase` が状態ごとに文字として確かめた値の件数を `stats.literalCounts` で返し、0 件なら `no literal value is checked` を違反にする。空の値は照合から除く
- 試験（`view-escape.test.mjs`。37 件）

| ID | 内容 | 期待 |
|---|---|---|
| E1 | 12 組 × CSP あり・なし（24 件） | 違反 0・項目数 `[3, 4, 3]`・値の件数が固定の表と一致（ラベル・補足の 3 ビューは 6・8・6、カードは 7・9・7、差分は 3・4・3、`app_demo` は 3・5・3）・全計数 0・CSP のコンソール警告 0・`__escape_hit` 未定義 |
| E2 | 行を切り替えずに 6 ビューの (b) を流す（6 件） | 件数と文字の違反が出る。各ビューで照合が効いている証拠 |
| E2 | `app_demo` (b) の e4 の `value` を書き換える | 文字の違反が出る。補足の照合が `value` の経路で効いている証拠 |
| E2 | 不正な payload | 拒否される |
| N1〜N3 | 1/2 のまま（5.288） | 5.288 のとおり |

- 実行結果（2026-10-23。Chromium headless、`TEST_HOOKS=1`・`SKIP_WASM_OPT=1` のビルド）: `view-escape.test.mjs`・`artifact-view-check.test.mjs`・`artifact-view-controls.test.mjs` の 72 件が全通過（約 13 秒）。どのビューにも、escape の行を落とす・解釈する欠陥は見つからなかった
- 5.288 の「#675 に渡す事項」（欄の調整と 24 通りの流し込み）は本節で済ませた
- `make test-browser` の全体は手元では流していない（未実行。CI の `browser-test` に委ねる）。`scripts/expected-test-counts.txt` は更新しない
- 限界: 試験の作成者と実装の作成者が同じ AI である。人の確認は得ていない

### 5.290 test の誤入力の行・FAIL の行・集計（TASK-404 3/3。AUTHOR-4）

`fandhe-pack test` が、誤入力の PASS の行、不合格の FAIL の行（実際の結果つき）、集計の 3 行、試験の診断を、`docs/cli.md` 版 11 の 12.3 節・12.4 節の形で書くことを確かめた。

| 試験 | 確かめること |
|---|---|
| `author4_pass_line_per_playthrough`・`author4_pass_line_counts_ticks`・`author4_pass_line_order_follows_script`・`author4_pass_line_does_not_echo_unsafe_id`（`test_command.rs`） | 標準出力の全体（PASS の行、誤入力の行、集計）が一致する |
| `author4_error_input_fail_line_has_actual_result`・`author4_playthrough_fail_line_has_actual_result` | D09・D10 の FAIL の行に期待と実際の語が出る |
| `author4_test_diagnostics_after_summary` | 診断が集計の後ろに 1 件出る |
| `author4_fail_line_for_end_not_reached_has_fixed_words` | 終わりの条件の不成立が固定語で出て実体 ID を写さない |
| `author4_fail_line_does_not_echo_unsafe_values` | 安全でない ID・入力の値が写らない |
| `cli_doc_test_error_lines_example_matches`・`cli_doc_test_summary_example_matches`（`cli_contract_doc.rs`） | 文書の例と関数の出力が完全一致する |
| `pack29_play_defects_reported_by_test`（`defect_packs.rs`） | `#[ignore]` を外し、D09・D10 の 4 パックで 24/24 |

- 実行: `cargo test -p fandhe-workshop-pack --locked`
- 経過措置: 最終行・R-22／R-21・入った実体の集計は TASK-405、文面・案内レベルの確認の判定は後続。判定していない定義があるときは終了コード 1 のまま


### 5.292 compile した IR の手元の配信と CH-0〜CH-3 の完走（TASK-427 1/2。AUTHOR-21）

作成者の手順そのものを自動ブラウザで通した。形式 B の教材パックを `fandhe-pack compile` で IR にし、その出力を `IR_FILE` に渡して配信する設定（試験用の口なし）でビルドし、`tools/serve.mjs` でサブパスの下に gzip つきで配り、実クリックで CH-0〜CH-3 の 20 Step を最後まで通す。試験は `tools/browser/author-preview.test.mjs`（補助は同名の `.mjs`）。

#### 解釈

- 教材 repo の本物の CH-0〜CH-3（TASK-643・TASK-642）はまだ無い（2026-10-23 時点）。そのため教材は、手書きの `crates/web/tests/data/ch0_3/ir.json` と同じ形を形式 B で書いた試験用の教材 `crates/web/tests/data/ch0_3_pack` である（5.117 の `ch0-3-run`・5.283 の `ch4-9-run` と同じ扱い）
- 手順（compile → `IR_FILE` を指定したビルド → サブパス配信 → 完走）は端から端まで実物で流す。本物の教材での流し直しは**未実施**である
- 試作の 73 項目は再現しない。台帳の件数は「N/N 項目」と書く
- AI による代理の確認であり、人の評価には数えない（D-34）

#### 手順

1. `scripts/test-browser.sh` が教材を compile し、`IR_FILE=<IR> tools/build-web.sh`（`TEST_HOOKS` なし）で `web-author` を作る。ビルドは試験の外で行う（計時の試験に負荷を与えない）
2. 試験が `FANDHE_WEB_AUTHOR_DIR`（配信物）と `FANDHE_AUTHOR_IR`（compile した IR）を受け、`/fandhe-author-preview/` の下で配る
3. DOM だけの観察（`__fandheTest` は無い）で 20 Step を通す。操作の列は `ch0-3-journey.mjs` の `journey` を使う

#### 確かめること

| 試験 | 確かめること |
|---|---|
| P1 | `validate` が終了コード 0 |
| P2 | もう一度 compile した出力が、配信物の `ir/main.json` と受け取った IR のどちらともバイト単位で一致する（決定性と、作成者が見る IR が受講者に配る IR と同じこと） |
| P3 | 配信物の `index.html` に `fandhe-ir` の meta がちょうど 1 個で、試験用の口（meta・`test-hooks.js`）が無い |
| P4 | compile した IR の構造が、手書きの IR の変種（各章の最後を `final` にし、開始点を章の先頭 4 件にしたもの）と一致する。`journey` の前提が崩れたときに Step を名指しして落とす |
| P5 | `STEPS` の題が IR の題と一致し、20 件とも違う |
| B（CSP あり・なし） | 20 Step を最後の達成まで通す。各章の最初の Step 4 件に「いまやること」と do、本文の区画（見える・空でない・先頭の見出しに題）が出る。章の境目で「次の章へ進む」がちょうど 1 個。復習の行で 20 Step がすべて達成。配信元以外への通信・ページ内エラー・コンソールの error と CSP 警告・安全の計数がすべて 0。`ir/main.json` の取得は 200 で 1 回 |
| C | DOM の観察が各時点で試験の設定の `stateJson` と一致し、1 Step 先・同じ Step の別の状態を示さない（B の空振りの防止） |
| D | 本文を隠す・do を書き換えると、先頭の Step の確認が不合格になる（負の対照） |

`ch0-3-journey.mjs` は、`cross` の世代の確認を `observe.hasState !== false` のときだけ行うよう変えた。既定の観察は `hasState` を持たず、既存の利用者の振る舞いは変わらない。

#### 確かめないこと

- 章の開始点への直接リンクと、作成者向けの文書（TASK-427 2/2）
- 本物の CH-0〜CH-3（TASK-643・TASK-642）
- キーボードだけの完走（`ch0-3-run` が持つ）。処理中の Esc による停止（5.117 の限界）
- 人が作成者として画面を確かめた記録（仕様が「含まない」と定める）

#### 結果の 1 行の様式

```text
AUTHOR-PREVIEW csp=<on|off> steps=<達成数>/20 crossings=<回数> first-steps=<合格>/4 external=<件数> page-errors=<件数> ledger=<合格>/<項目>
```

名前・件数・合否だけを出し、入力した値と本文は出さない。

#### 実行結果

- 実行日: 2026-10-23。Chromium（同梱）。`SKIP_WASM_OPT=1`
- P1〜P5・B（2 件）・C・D の 9 件が通った
- `AUTHOR-PREVIEW csp=on steps=20/20 crossings=3 first-steps=4/4 external=0 page-errors=0 ledger=88/88`
- `AUTHOR-PREVIEW csp=off steps=20/20 crossings=3 first-steps=4/4 external=0 page-errors=0 ledger=88/88`
- C: 235/235 項目
- `ch0-3-journey.mjs` の変更後も、`ch0-3-run`・`next-chapter`・`shell-journey`・`step-body`・`shell-widths`・`csp-runs`・`l2-journey-calibrate`・`chapter-select-absent`・`checkpoint-restore` の 47 件が通った。`make test-browser` の全体は未検証

#### 限界

- `SKIP_WASM_OPT=1` のビルドである。wasm-opt は大きさを縮めるだけで振る舞いは変えない
- CI は Chromium で流す。Chrome の原本で流すには `FANDHE_BROWSER_CHANNEL=chrome` を付ける（5.128）
- DOM だけの観察の確かさは、較正 C に依る
- 本物の教材での流し直しは未実施である
