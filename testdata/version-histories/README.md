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
`SessionState` の形が変わるたびに書き直しが要るためです。表だけを `testdata/` に置くのは、後続のブラウザ試験
（#449）が Node 側から同じ表を読めるようにするためです。

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

試験は `crates/core/tests/version_histories.rs`、変異は `crates/core/tests/mutations/VER2-1.mutation`・`VER2-2.mutation` です。
この試験は判定の結果の種類・理由・軸が表と合うことを見ます。履歴の写しの一致と `Session` を作らないことの確認は、#449 で扱います。

## 線引きを変えたときの期待の更新手順

#449（TASK-175 2/2）で書きます。
