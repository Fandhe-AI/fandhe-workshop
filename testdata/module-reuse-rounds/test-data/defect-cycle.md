# 手順 3 (vii): 実体の並べ方に巡回を作る欠陥（実施の前に固定）

正しいパックの写しに、次の置換を行い、`validate` の診断に `E-MODULE-CYCLE` が出ること（場所つき）を確かめる。正しいパックは `E-MODULE-CYCLE` を 1 件も出さない。

## 欠陥 C1: 2 つのモジュールが互いを呼ぶ（issue-to-merge → idea-cycle → issue-to-merge）

- `modules/issue-to-merge/module.yaml` の `steps:` の末尾に、次の Step を足す。
- `modules/idea-cycle/module.yaml` の `steps:` の末尾に、次の Step を足す。

issue-to-merge に足す:

```yaml
  - id: st-x-loop
    title: 巡回の試験
    final: true
    use:
      module: idea-cycle
      instance: loop
```

idea-cycle に足す:

```yaml
  - id: st-x-loop
    title: 巡回の試験
    final: true
    use:
      module: issue-to-merge
      instance: loop
```

期待: `E-MODULE-CYCLE` が 1 件以上、`modules/` の 2 つの定義ファイルのどちらかの `step:st-x-loop` に付く。メッセージに `issue-to-merge → idea-cycle → issue-to-merge` または `idea-cycle → issue-to-merge → idea-cycle` を含む。`compile` は止まらずに診断を返す（`E-MODULE-CYCLE`）。

## 欠陥 C2: モジュールが自分を呼ぶ

`modules/issue-to-merge/module.yaml` の `steps:` の末尾に、次の Step を足す。

```yaml
  - id: st-x-self
    title: 自己呼び出しの試験
    final: true
    use:
      module: issue-to-merge
      instance: self
```

期待: `E-MODULE-CYCLE` が `modules/issue-to-merge/module.yaml` の `step:st-x-self` に付く。メッセージに `issue-to-merge → issue-to-merge` を含む。

## 欠陥 C3: 3 つ目のモジュールを経由する巡回（idea-cycle → issue-to-merge → relay → idea-cycle）

`modules/relay/module.yaml` を足し、中身は次のとおり。`modules/idea-cycle/module.yaml` の末尾に C1 の `st-x-loop`（呼び先は `issue-to-merge`）を足し、`modules/issue-to-merge/module.yaml` の末尾に次の Step を足す。

```yaml
  - id: st-x-relay
    title: 中継の試験
    final: true
    use:
      module: relay
      instance: r
```

relay:

```yaml
id: relay
title: 中継の試験
steps:
  - id: st-x-back
    title: 戻る
    final: true
    use:
      module: idea-cycle
      instance: back
```

期待: `E-MODULE-CYCLE` が 1 件以上。
