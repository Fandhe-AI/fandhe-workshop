# 手順 4: 1 周目のモジュールの定義の 1 か所の直し（実施の前に固定）

直す場所は 1 か所です。`pack/modules/issue-to-merge/module.yaml` の `st-6-1` の規則 `r-new-root` の、子 Issue を作る状態変化の 1 行。

変更前:

```yaml
            - put: { path: "issues/child-${child}", fixture: fx-kakuu-issue-child, attrs: { state: open } }
```

変更後:

```yaml
            - put: { path: "issues/child-${child}", fixture: fx-kakuu-issue-child, attrs: { state: open, label: tracked } }
```

（このファイルには、`issues/child-${child}` を作る同じ書き方の行が 2 つある。行頭の空白が 12 個の行が `st-6-1` の規則の行で、直すのはこの行だけ。行頭の空白が 6 個の行は `s-7-1`（途中開始）の開始用の行で、直さない。）

## 確かめること

1. 直した後のパックが `validate` を診断 0 件で通り、`test` が 4/4 完走・8/8 誤入力のまま通る。
2. 直す前と直した後の IR を比べて、違う Step の実体が `issue-to-merge@round1/st-6-1` と `issue-to-merge@round2/st-6-1` の 2 つだけ（実体の数、ほかの Step、開始、`instances` は同じ）。
3. 直した後で 1 周目から 2 周目へ通すと、世界状態の `issues/child-1` と `issues/child-9` の両方が `label=tracked` を持つ（2 周目にも反映）。
4. 比較（C-7 の代替案 (a) 「2 周目を別の章として書く」との違い）: 直す場所の数は、モジュールで 1、周回ごとに別の章へ写した場合は周回の数（ここでは 2）。
