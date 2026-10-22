# ir_before_ep13

EP-13（`views.<ID>.body`、語彙 1.4）の追加の前のエンジンが出力した IR の凍結物です。
新しい項目を使わない教材パックの IR が、受け口の追加の前後でバイト単位で同じことの試験
（`crates/pack/tests/ext9_ir_identity.rs`、EXT-9・D-52・D-53、TASK-190 の 2/2）が読みます。
JSON はコメントを書けないため、説明は本ファイルに置きます。

## 作り方（2026-10-19、元にしたコミット 0fa7146）

- 0fa7146 は EP-13 の追加コミット（ad4629c）の親。`tests/data/golden/` と `tests/data/ir_format_1/` を最後に書いたコミットでもある
- そのコミットのソースを一時ディレクトリへ展開し、`cargo build --locked -p fandhe-workshop-pack` で作った
  `fandhe-pack compile <パックのディレクトリ>` の標準出力を、そのまま保存した
- 入力は今の作業ツリーの教材（`tests/data/*-pack`）。chosen-value-pack 以外は 0fa7146 から入力が変わっていない。
  chosen-value-pack は 0fa7146 の後に足されたが、旧いエンジンで compile できたため今の入力で作った
- 同じ旧いエンジンで golden の 5 パックを出力し、`tests/data/golden/` とバイト一致することを確かめた（保存はしていない）
- 出力に絶対パスは入っていない（`grep` で確認）

## ファイル

| ファイル | `format_version` |
|---|---|
| `chosen-value-pack.ir.json` | 2 |
| `confirm-kinds-pack.ir.json` | 2 |
| `confirm-label-pack.ir.json` | 2 |
| `fill-pack.ir.json` | 2 |
| `input-kinds-pack.ir.json` | 2 |
| `judgment-pack.ir.json` | 2 |
| `slot-pack.ir.json` | 2 |
| `no-tree-pack.ir.json` | 1 |
| `fixture-pack-choice.ir.json` | 2 |
| `no-tree-pack-choice.ir.json` | 1 |

`no-tree-pack` は本文も練習データも持たない最小パックで、入力は試験の定数と同じ。

`workshop.yaml`:

```yaml
pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}
```

`chapters/c1/chapter.yaml`:

```yaml
id: c1
title: C
starts: [{id: s1, node: a}]
steps:
  - {id: a, title: A, next: [{to: b}]}
  - {id: b, title: B, final: true}
```

## 後から足した 2 本（2026-10-22、PACK-19、#1708）

`fixture-pack` と `no-tree-pack` の途中の Step `a` に、入力を足した（入力も `use` も無い終端でもない Step を `E-STRUCTURE`
で拒否する PACK-19 の準備）。入力を足すと compile の出力が変わるため、上の凍結物を書き換えずに、同じ手順
（0fa7146 のソースの展開、`cargo build --locked -p fandhe-workshop-pack`、`fandhe-pack compile <ディレクトリ>` の標準出力を保存）で
別名の 2 本を足した。今のエンジンの出力と、バイト一致することを確かめた。出力に絶対パスは入っていない。

- `fixture-pack-choice.ir.json`: 入力は今の `tests/data/fixture-pack`。`a` に `input: {kind: choice, options: [{id: ok, label: 読みました}]}`、規則 `r-a`、`confirm: {kind: none, ...}` を足した
- `no-tree-pack-choice.ir.json`: 入力は下の 2 つ目の `chapters/c1/chapter.yaml`

`chapters/c1/chapter.yaml`（`no-tree-pack-choice`）:

```yaml
id: c1
title: C
starts: [{id: s1, node: a}]
steps:
  - {id: a, title: A, next: [{to: b}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-a, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}
  - {id: b, title: B, final: true}
```

## 範囲の限界

`testdata/` の教材（`condition-vocab-pack` など）は対象にしていない。

## 書き換えない規則

凍結物を書き換えて試験を通してはいけません。食い違うなら、互換が壊れたか、受け口とは別の変換が変わったことを意味します。
原因を調べて直すか、版の規則（D-52）に沿った扱いを決めてから、新しい凍結物を別の名前で足します。
