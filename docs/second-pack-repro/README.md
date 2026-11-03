# 公開の資料だけで書いた第 2 教材（再現の記録）

別のコンテキストの作者（AI のエージェント）が、公開の資料と `fandhe-pack` の実行ファイルだけで、第 2 教材「レビューの頼み方」を形式 B で書き、`validate` と `test` を通した結果の写しです（AUTHOR-12。TASK-413 1/2）。製品の第 2 教材（`template-workshop/examples/review-request/`）とは別物で、そちらは変えていません。

| ファイル | 内容 |
|---|---|
| `brief.md` | 作者に渡した要件の要約（答えを含めない） |
| `public.sha256` | 作者に渡した公開の資料の SHA-256（repo からの相対パス。資料の写しは置かない） |
| `pack/` | 作者が書いた教材パックの写し。`pack/tests/ir.json` は作者が `compile` した IR で、手を加えていない（TASK-413 2/2 の比較の入力） |
| `baseline.txt` | 写しの件数の基準値 |

## 作者の起動の条件

- 作業場所は repo の外の一時ディレクトリ。`public/`（公開の資料の写し）・`bin/fandhe-pack`・`brief.md`・空の `pack/` だけを置いた
- 作者は、実装の担当とは別の `claude -p` のセッションとして起動した（cwd は作業場所。エンジンのソースと repo を渡していない）
- 実行ファイルは、base のコミット 7721da31 の `scripts/build-pack-bin.sh` の出力（debug ビルド）。SHA-256 は `dbad8e88f2d18477fe69213fc34c5fc5792e6217f4211b7f0e64efca7728b88d`

## 写した後に変えた点

なし。作者の `pack/` をそのまま写した。

## 再実行

```text
make check-second-pack-repro
make test-check-second-pack-repro
```

`ir-match` は、作者が `compile` した `pack/tests/ir.json` と、写しを再 `compile` した IR のバイト一致を見ます。写しは凍結しており、一致が崩れたら書き直さず所見として記録します。`public.sha256` は作者に渡した時点の資料の値で、`docs/known-gaps.md` を足したため今の資料とは合いません。

## 不足の一覧

書けなかった箇所は `docs/known-gaps.md` に記録しています。新しい不足は `AUTHOR-13（3）`（`world_attr` の書き方）と `AUTHOR-13（4）`（確認待ちの間の入力の既定の拒否）です。ほかは既存の SECOND-12（条件つきの拒否）・AUTHOR-4・PACK-22・SECOND-8・SECOND-6・PACK-21・PACK-35・ART-17 に対応します。対応表と記録は `docs/testing.md` の 5.451（1/2 は 5.449）にあります。
