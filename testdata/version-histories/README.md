# version-histories

版の違う履歴 15 件について、実施の前に固定した期待の表です（VER-2、TASK-175 1/2、#448）。
`expectations.json` が表の本体で、`crates/core/tests/version_histories.rs` が読みます。
spec は private のため、本文は写さず ID（VER-2・V-0 など）で参照します。

## 表の欄

| 欄 | 意味 |
|---|---|
| `id` | 履歴の ID。PoC-6 の `version-expectations.md` の 3 節に対応する（V-0・V-i-1〜V-ref の 15 件） |
| `category` | 種。同一 1・文言だけ 2・Step を足した 2・Step を消した 2・語彙と教材形式 4・エンジン 3・参照元 1 |
| `fabricated` | `true` は、版の欄だけを書き換えた作り物（実在しない版を模す）。`snapshot`・`ops` は書き換えない |
| `history` | 履歴の作り方。`path` は経路（`early`・`mid`）、`pack` は作るときの教材、`edit` は版の欄の書き換え（`engine`・`ir_format`・`refs` だけ） |
| `opening` | 開く側。`pack` は教材、`engine` が `null` なら履歴を作ったときと同じ版、`vocab_max` が `null` なら現行の上限 |
| `expect` | 期待。`result`（`open`・`converted`・`view_only`・`refused`）、`code`（理由コード。開く 2 種は `null`）、`axes`（違いの軸。`converted` だけが 1 件以上） |

結果の合計は、変換なし 1・変換して開く 6・表示だけ 5・開かない 3 です。

## 教材の変種と経路

履歴は、現行のエンジンで試験の中で決定的に作ります。golden の履歴ファイルは置きません。
`SessionState` の形が変わるたびに書き直しが要るためです。表だけを `testdata/` に置くのは、ブラウザ試験
（#449）が Node 側から同じ表を読めるようにするためです。ブラウザで開く履歴は、native の試験と同じ作り方を
`emit_histories_for_browser`（`#[ignore]`）が一時ディレクトリへ書き出します。

教材の変種は、`engine-event-consistency/test-data/min-3step.json` を書き換えて作ります。

| 名前 | 作り方 |
|---|---|
| `base` | 版 `1.0.0`・語彙 `1.2`・参照元 1 件（架空の名前とダミーの commit）を足す |
| `vocab-1.0` | `base` の語彙を `1.0` にする |
| `text-1.1.0` | `base` の版を `1.1.0` にし、ヒントの文を替える（本文の木ではない欄なのでハッシュが変わる） |
| `step-added-1.2.0` | `st-1` と `st-2` の間に Step を足し、版を `1.2.0` にする |
| `step-removed-2.0.0` | `st-2` を除いて `st-1` を `st-3` へつなぎ、版を `2.0.0` にする |

経路は 2 種です。`early` は `st-1` の中で処理を終え、チェックポイントを 1 つ保存したところまで（足した Step・消した Step
より手前）、`mid` は `early` に誤入力を 1 件足し、`st-1` を達成して `st-2` へ進め、下書きを残したところまでです。

## PoC からの読み替え

- V-iv-3: PoC は「`caps.ir_format_max` を超える」で断りました。現行の上限は 2 のため、開く教材の IR 版（1）を超える条件
  （履歴の `ir_format` が開く教材の `format_version` より大きい）で断ります。理由コードは同じ `format_newer` です。
- 語彙の上限: PoC は 1.2、現行は 1.4 です。V-iv-2 は上限を `1.0` にして語彙 v1 の実装を模します。
- V-iv-1: `content_hash` が `pack.vocab` を含むため、`vocab` に加えて `content_hash` の軸も出ます。
- 経路: PoC の H-mid・H-early（7 Step の教材）を、3 Step の上の同じ位置関係に置き換えました。

## 作り物の行

V-iv-3・V-iv-4・V-v-1〜V-v-3・V-ref は、版の欄だけを書き換えた作り物です。
`reload`（適用し直し）は使いません。TASK-320 まで、適用し直しが `replay_mismatch` になるためです。

## 公開の扱い

名前と commit は架空のダミー値です。秘密情報・URL・個人のパスを含みません。

## 試験と変異

| 試験 | 場所 | 見るもの |
|---|---|---|
| native | `crates/core/tests/version_histories.rs` | 結果の種類・理由・軸が表と合う（慎重な側とそれ以外に分けて数える） |
| 口の native | `crates/web/tests/version_histories_host.rs`（feature `test-hooks`） | 開く側のエンジンの版・語彙の上限の上書き、差し替えの回数、軸の書き出し |
| ブラウザ | `tools/browser/version-histories.test.mjs`（`make test-browser`） | 15 件を実際に開く。開く 7 件は写しが `snapshot` と完全に一致し、開かない 8 件は実行環境の差し替えが 0 回で、状態・保存領域・URL が変わらない。理由は開かない 8 件で空でなく、開く 7 件で `null` |

変異は `crates/core/tests/mutations/VER2-1.mutation`・`VER2-2.mutation`（判定）と、
`crates/web/tests/mutations/VER2-3.mutation`・`VER2-4.mutation`（上書きの無視・軸の欠落）です。
ブラウザ試験は変異注入の対象外です（`scripts/check-mutations.sh` は `cargo test` だけを流します）。

ブラウザ試験で開く側の条件を作るために、試験の設定（`TEST_HOOKS=1`）にだけ次の口を足しています。配信物には入りません。

| 口 | 内容 |
|---|---|
| `raw` の `load-ir:` | 開く側の教材（`packs/<名前>.ir.json`）への差し替え |
| `raw` の `opening-override:` | 開く側のエンジンの版・語彙の上限の上書き（V-v-1〜3・V-iv-2） |
| `stateJson().engine.installCount` | 実行環境の差し替えの回数 |
| `stateJson().engine.lastImport.axes` | 変換して開いたときの違いの軸 |
| `lastImportStateJson()` | 開いた直後の状態（写しの一致の照合用） |

## 線引きを変えたときの期待の更新手順

版の線引き（VER-18・D-52・D-59）を変える、または判定の実装（`crates/core/src/history_open.rs` など）を変えて
15 件の分類が変わりうるときの手順です。

1. spec を先に直します。04 の VER-2〜VER-7 の期待を、spec repo（`Fandhe-AI/fandhe-workshop-spec`）で直します。
   この repo の `docs/spec` は編集しません。
2. 試験を流す前に、決定から `expectations.json` の行ごとの `expect`（`result`・`code`・`axes`）を書き直します。
   結果から表を作りません。実施の前に固定した期待であることが、VER-2 の前提だからです。
3. 合計が変わる場合は、`crates/core/tests/version_histories.rs` の `IDS`・`CATEGORY_COUNTS`・`RESULT_COUNTS`、
   `tools/browser/version-histories.test.mjs` の `IDS`・`RESULT_COUNTS`、本 README の合計の行、
   `docs/testing.md` の記述を同じ変更で直します。行を足すときは、教材の変種（`pack_json`）と経路も足します。
4. 表の形を変える場合だけ `table_version` を上げ、2 つの読み手（Rust・Node）を同時に直します。
5. native（`cargo test -p fandhe-workshop-core --test version_histories`）の後に `make test-browser` を流します。
   食い違いは「慎重な側」も「それ以外」も 0 件でなければなりません。食い違ったら、表と実装のどちらが決定に
   反しているかを判断します。表を結果に合わせて直しません。
6. `VER2-*` の変異を `make check-mutations` で流し直します。対象の行の期待が変わった場合は、変異の内容と期待する
   失敗を決め直し、`docs/testing.md` の記録を更新します。
7. 変更の日付（`YYYY-MM-DD`）と根拠の決定（D-n）を、`docs/testing.md` の該当の節に残します。
