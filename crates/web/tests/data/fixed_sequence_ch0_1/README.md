# fixed_sequence_ch0_1

TASK-114 2/2 の固定の入力列と、それを流す IR の凍結した写しです。`tools/browser/l1-l2-fixed.test.mjs` が使います。

| ファイル | 内容 |
|---|---|
| `ir.json` | 凍結した IR。凍結時点（2026-10-22）の `../ch0_3/ir.json` のバイト写し |
| `sequence.json` | 固定の入力列（版 1）。`ir.json` の SHA-256・凍結時の `content_hash`・項目（`op`・`expect`・`still`）・凍結時の `ops` |

## 位置づけ

- 配信する木の `ir/main.json` をこの `ir.json` に差し替えて流すため、`ch0_3` や本物の教材が後で変わっても、列は同じ IR で再生できます
- 列は手で書かず、`tools/browser/ch0-3-journey.mjs` の `journey` を記録用の包みに通して一度だけ導きました。`journey` や `ch0_3` が後で変わっても追随させません
- 画面の語は列に入れていません（Step・状態・口の数・`ops` だけ）。入力の値はすべて試験データの架空の語です

## 凍結と作り直し

- 凍結: `FANDHE_FREEZE_SEQUENCE=<このディレクトリの絶対パス> L2_SERVER=<miniserve> make test-l2-run`。列と IR の写しを書き出し、書き出した後は必ず失敗で終わります（凍結の実行が合格に数えられないため）。通常の実行・CI では指定しません
- 既存の列は書き換えません。IR の形式が変わって再生できなくなったときだけ、版つきの新しいディレクトリ（例: `fixed_sequence_ch0_1_v2`）を足し、`docs/testing.md` に理由を記録します
