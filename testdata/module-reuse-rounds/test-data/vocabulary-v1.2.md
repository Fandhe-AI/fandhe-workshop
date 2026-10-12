# 語彙 v1.2（`vocabulary-v1.1.md` からの差分）

PoC-4（学習モジュールの再利用と周回の引き継ぎ）の手順 3 (vi)（開始条件を満たさない場合に、開始できない理由を示す）を書くために足す語です。固定した `../../pack-declarative-format/test-data/vocabulary-v1.md` と `../../pack-declarative-format/addendum-g/test-data/vocabulary-v1.1.md` は書き換えません。固定の日時と SHA-256 は `../env.md` に記録します。固定の後は書き換えません。足りないと分かったものは別ファイル（`vocabulary-v1.3.md` など）に足します。

本書に書かない事項は、すべて v1.1 のままです。

## 1. 位置づけと互換性

- 教材形式の版 `ir_format` は **1 のまま**です。v1.2 は**加算だけ**です。新しい項目はすべて任意で、省くと v1.1 と同じ意味になり、既定値のときは IR に出力しません（`skip_serializing_if`）。
- v1 と v1.1 の教材パックは、v1.2 の実装でもそのまま読め、同じ IR（JSON）になります。v1.2 の項目を使った教材パックを v1.1 の実装で読むと、未知の項目として `E-PARSE` で拒否されます。
- スクリプトの欄も、式の文法も足しません。

## 2. 足す語（1 つ）

| ID | 名前 | 対応する要求 | 足すもの |
|----|------|------------|---------|
| EP-9 | `starts.requires`（開始条件と、満たさないときの文面） | PoC-4 手順 3 (vi) | `starts[].requires`（条件）、`starts[].unmet`（拒否の文面の ID） |

## 3. EP-9 開始条件（`starts[].requires`、`starts[].unmet`）

### 3.1 語

途中開始（`starts[]`）に、任意の `requires` と `unmet` を足します。

```yaml
starts:
  - id: s-r2
    node: r2-idea
    guidance: B
    requires: { step: { step: r1-impl, state: achieved } }   # v1.2
    unmet: rj-start-needs-previous-round                      # v1.2。拒否の文面の ID
```

- `requires`: 条件（`vocabulary-v1.md` の条件の語。`all`・`any`・`not`・`flag`・`step`・`world_has`・`world_attr`・`count`・`chose`・`param`・`clarifying` のどれか 1 つ）。
- `unmet`: `requires` を満たさないときに受講者へ示す文面（`rejections` の ID）。分類・文面・次の行動を持つ。

### 3.2 意味

- 開始（`start`）または途中開始（`begin_at`）の時に、開始用の状態変化を適用する**前**に `requires` を評価します。評価する実体は、開始 Step が属する実体です（`step` の語は `<実体 ID>/<Step ID>` を引く）。
- 満たすとき: v1.1 と同じに開始します。
- 満たさないとき: **開始しません**。状態（世界状態、Step の状態、フラグ、案内レベル、現在の Step）は 1 つも変えません。開始できない理由として `unmet` の文面の ID を返します。
- `requires` と `unmet` は、片方だけを書けません（`E-STRUCTURE`）。`unmet` が存在しない文面を指すとき `E-REF-REJECTION`。`unmet` の「次の行動」が空のとき `E-EMPTY-NEXT-ACTION`（v1 の R-21 の検査のまま）。

### 3.3 汎用であること

「前の章・前の周回が終わっていないと始められない」「必要な成果物が無いと始められない」は、章立ての教材・周回のある教材に共通です。エンジンは条件の語を評価して文面の ID を返すだけで、特定の教材・上流の語を知りません。

## 4. v1 の実装の補い（語彙は増やしません）

語彙 v1 は、モジュール（`module`）にも `starts`（途中開始。ID・開始 Step・開始用の状態変化の列）を書けると定めています（`vocabulary-v1.md` 8 節）。v1 の実装（`compile`）は章の `starts` しか IR に出さず、モジュールの `starts` を読み飛ばしていました。PoC-4 で、v1 の定めどおりに実装します。新しい項目ではありません。

- 呼び出しの実体ごとに、モジュールの `starts` を IR の `starts` に展開します。IR の開始 ID は `<実体 ID>#<開始 ID>`（例: `idea-cycle@round2#s-2-5`）、開始 Step は `<実体 ID>/<Step ID>`。
- 開始用の状態変化の中の `${名前}` は、その実体の引数で置換します（Step と同じ）。
- `requires`（EP-9）は、モジュールの `starts` にも書けます。評価する実体は、その実体です。
- 省略の印（`skipped`）は、v1.1 と同じく、開始 Step と同じ実体の中の、先の Step だけに付きます。
