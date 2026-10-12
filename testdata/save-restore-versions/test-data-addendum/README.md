# 追加の試験データ（実装の後に足したもの）

`test-data/` は実装の前に固定した（`../env.md`）。ここには、履歴の形式の実装が済んでからでないと作れないもの（履歴ファイル）と、固定した文書への訂正を置く。`test-data/` は書き換えていない。

## 1. 固定した文書への訂正（実装の前に気づいた誤り）

`test-data/history-format-v1.md` の 5 節の最後の項「入口の写しは作り直す（現在の状態の写し。以後の停止はここへ戻る）」は誤りだった。処理中に再読み込みして入口の写しを現在の（処理中の）状態で作り直すと、以後の停止が「Step の入口」でなく「処理中の途中」へ戻り、D-22（停止は Step の入口へ戻す）に反する。

**訂正**: 再読み込み（`Session::reloaded`）は入口の写しを**作り直さない**。書き出した履歴の `snapshot.entry` をそのまま持ち越す。固定した文書（`history-format-v1.md`）は書き換えない。試験は、この訂正に従う（処理中に再読み込みして停止すると、Step の入口へ戻ることを S-6 と決定的な試験で確かめる）。

## 2. 履歴ファイル（`histories/`）

`prototype/engine/crates/pack/tests/poc6_generate.rs`（`--ignored` の試験）が、`history-recipes.md`・`broken-histories.md`・`version-expectations.md` のとおりに生成した。生成は履歴の形式（`core/src/history.rs`、`session.rs`）の実装の後。生成の後に SHA-256 を固定した（`../env.md`、`../test-data-addendum.sha256`）。

| ファイル | 内容 |
|---------|------|
| `h-early.json`・`h-mid.json`・`h-busy.json`・`h-full.json` | `history-recipes.md` の 4 つの履歴。`h-full` は条件 A の通し番号 0（`synth-inputs.md`。`recipes` の「通し番号 1」は条件 A の最初の種、つまり通し番号 0 と読む） |
| `h-mid-vocab-1.0.json` | V-iv-1 の元（語彙 1.0 を宣言した教材で作った `H-mid`） |
| `h-mid-other-pack.json` | B-6（別の教材 `other-pack` で作った `H-mid`） |
| `v-iv-3-ir-format-2.json`・`v-iv-4-ir-format-0.json`・`v-v-2-engine-0.2.0.json`・`v-ref.json` | 版の欄だけを書き換えた作り物 |
| `b-1-ops-rewritten.json`・`b-2-truncated.json`・`b-3-not-json.json`・`b-5-no-versions.json`・`b-7-malformed.json` | 壊れた履歴。B-2 は全体の 55% 以下で、文字の境目で切った。バイトの途中で切る形は試験が作る。B-4（10MB 超）は試験が作る |
