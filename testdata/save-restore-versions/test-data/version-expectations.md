# 版の違いの期待する振る舞い（手順 6。実装の前に固定）

## 1. 読み込みの順序（上から。最初に当たったもので決まる）

開く側の条件を `caps`（エンジンが読める上限）で表す。通常は `caps = { history_format: 1, ir_format_max: 1, vocab: "1.2", engine: "0.0.0" }`。「語彙 v1 の実装」「新しい／古いエンジン」は `caps` を替えて模す。

| 順 | 条件 | 結果（コード） |
|----|------|---------------|
| 1 | ファイルが 10 × 1024 × 1024 バイトを超える | 開かない（`too_large`）。JSON を読む前に判定する |
| 2 | JSON として読めない。入力の終わりで途切れている | 開かない（`truncated`） |
| 3 | JSON として読めない。途中で文法が壊れている | 開かない（`not_json`） |
| 4 | `format` が違う、または `history_format`・`versions` の欄が無い | 開かない（`missing_version`） |
| 5 | 型に当てはまらない（欄の型が違う） | 開かない（`malformed`） |
| 6 | `history_format` が `caps.history_format` より大きい | 開かない（`history_format_newer`） |
| 7 | `pack_id` が開く教材の ID と違う | 開かない（`other_pack`） |
| 8 | 版の文字列が読めない | 開かない（`bad_version`） |
| 9 | 履歴の `ir_format` が `caps.ir_format_max` より大きい | 開かない（`format_newer`） |
| 10 | 履歴の `vocab` または開く教材の `vocab` が `caps.vocab` より大きい | 開かない（`vocab_newer`） |
| 11 | 履歴の `engine` が `caps.engine` より新しい | 開かない（`engine_newer`） |
| 12 | 履歴の `ir_format` が開く教材の `ir_format` より小さい（変換の規則が無い） | 表示だけ（`format_older`） |
| 13 | 履歴の `engine` の主版が `caps.engine` の主版と違う | 表示だけ（`engine_major`） |
| 14 | 履歴の `content` の主版が開く教材の `pack.version` の主版と違う | 表示だけ（`content_major`） |
| 15 | 履歴の `start` が開く教材に無い | 表示だけ（`start_missing`） |
| 16 | `ops` を適用し直した結果が `snapshot` と一致する | 版の軸がすべて一致し `content_hash` も同じなら「変換なしで開く」。どれかが違えば「変換して開く」（`notes` に違う軸を書く） |
| 17 | 一致しない。`content_hash` と `engine`・`ir_format`・`vocab` がすべて履歴と同じ | 開かない（`replay_mismatch`）。履歴が壊れている（書き換えられている） |
| 18 | 一致しない。上の 17 以外 | 表示だけ（`replay_diverged`）。教材かエンジンの違いで進み方が変わった |

「一致」は、`Session` の写し（`state`・`log`・`entry`・`checkpoints`・`next_checkpoint`・`draft`・`draft_root`・`drafts`）が完全に等しいこと。

## 2. 試験の履歴（すべて基準版 v1.0.0 の教材で作る）

- `H-mid`: 1 周目の `idea-cycle@round1/st-2-7` に入った所まで進めた履歴。チェックポイント 2 件（`st-2-3` と `st-2-5` の位置）、ヒントを 1 つ開いた、停止 1 回、誤入力 3 件を含む。下書き `draft-mid` が残っている。
- `H-early`: `idea-cycle@round1/st-2-3` に入った所まで（足す Step・消す Step の手前）。チェックポイント 1 件。
- `H-busy`: `issue-to-merge@round1/st-7-2`（待機の Step）で処理中（進行 1/2）の履歴。
- `H-full`: 1 周目と 2 周目を完走した履歴（`synth-inputs.md` の合成。容量の測定の元）。

## 3. 版の違い 5 種の期待

| ID | 種 | 履歴 | 開く側の教材（`caps`） | 期待する結果（コード） |
|----|----|------|------------------------|-----------------------|
| V-0 | 同一 | `H-mid` | v1.0.0 | 変換なしで開く |
| V-i-1 | (i) 文言だけ違う | `H-mid` | v1.1.0 | 変換して開く。復元した `state` は保存したものと等しい。`notes` に「教材の内容が変わった（1.0.0 → 1.1.0）」を含む |
| V-i-2 | (i) | `H-early` | v1.1.0 | 変換して開く |
| V-ii-1 | (ii) Step を 1 つ足した | `H-mid` | v1.2.0 | 表示だけ（`replay_diverged`） |
| V-ii-2 | (ii) | `H-early` | v1.2.0 | 変換して開く（足した Step `st-2-3b` は履歴の位置より先。続きは新しい Step を通る） |
| V-iii-1 | (iii) Step を 1 つ消した（主版が違う） | `H-mid` | v2.0.0 | 表示だけ（`content_major`） |
| V-iii-2 | (iii) | `H-early` | v2.0.0 | 表示だけ（`content_major`。消した Step の手前でも開かない） |
| V-iv-1 | (iv) 語彙の宣言が古い履歴（語彙 v1 → v1.2 の教材） | `H-mid` を、`vocab: "1.0"` を宣言した教材 `v1.0.0-vocab-1.0` で作る | v1.0.0（`vocab: "1.2"`） | 変換して開く。`notes` に語彙の版の違いを含む |
| V-iv-2 | (iv) 語彙が新しい履歴を、語彙 v1 の実装で開く | `H-mid`（`vocab` 1.2） | v1.0.0、`caps.vocab = "1.0"` | 開かない（`vocab_newer`） |
| V-iv-3 | (iv) 教材形式が新しい | `H-mid` の `versions.ir_format` を 2 にしたもの | v1.0.0 | 開かない（`format_newer`） |
| V-iv-4 | (iv) 教材形式が古く、変換の規則が無い | `H-mid` の `versions.ir_format` を 0 にしたもの | v1.0.0 | 表示だけ（`format_older`） |
| V-v-1 | (v) エンジンが古い（副版） | `H-mid` の `versions.engine` を `0.0.0`、開く側の `caps.engine` を `0.1.0` | v1.0.0 | 変換して開く。`notes` にエンジンの版の違いを含む |
| V-v-2 | (v) エンジンが新しい履歴 | `H-mid` の `versions.engine` を `0.2.0`、`caps.engine` を `0.1.0` | v1.0.0 | 開かない（`engine_newer`） |
| V-v-3 | (v) エンジンの主版が違う | `H-mid` の `versions.engine` を `0.0.0`、`caps.engine` を `1.0.0` | v1.0.0 | 表示だけ（`engine_major`） |
| V-ref | 参照元の版が違う（追加の確認） | `H-mid` の `versions.refs` の commit を 1 つ替えたもの | v1.0.0 | 変換して開く。`notes` に参照元の版の違いを含む |

注: 版の違いのうち、`versions` の欄だけを書き換えて作るもの（V-iv-2〜4、V-v-1〜3、V-ref）は、実在しない版を模した**作り物**である。実際に別の版のエンジン・別の形式を作ったものではない。記録に作り物と書く。

## 4. 「黙って誤った状態で開いた例」の数え方

次のどれかを 1 件と数える。基準は 0 件。

1. 「開く」または「変換して開く」の結果の `Session` の写しが、履歴の `snapshot` と一致しない。
2. 「表示だけ」「開かない」の結果で `Session`（操作できる実行）が作られた。
3. 期待の表と結果が違う（理由のコードを含む）。ただし、期待より**慎重な側**（開く → 表示だけ、表示だけ → 開かない）に倒れた場合は、黙って誤った状態で開いた例ではなく、「期待と違う結果」として別に数える。
4. 「表示だけ」「開かない」の結果に、理由の文面が無い（空）。
