# エンジンの差分 0 の確認（第 3 教材「タスクの分解と進行管理」。TASK-763 3/3）

第 3 教材 `template-workshop/examples/task-breakdown/` が、エンジンの Rust を変えずに足せたことを、数えて示す記録です（SECOND-20）。
形は第 2 教材の記録（`docs/engine-zero-diff/review-request/`）と同じで、道具も同じ `scripts/check-engine-zero-diff.sh` です。教材ごとに `--base`・`--pack`・`--manifest` を指定して流しました。

## 結論

- 固定した commit から測った HEAD までの間で、数える範囲（`crates/`・直下の `Cargo.toml`・`Cargo.lock`）の差分は 0 行でした
- 内訳は、直下の `Cargo.toml` が 0 行、`Cargo.lock` が 0 行、`crates/*/Cargo.toml` が 0 行、`crates/` 全体が 0 行です
- SHA-256 の一覧は 1,628 件で、固定した commit から作り直した一覧とバイト単位で一致し、測った HEAD の 1,628 件とも全件一致しました
- 第 2 教材の一覧（`docs/engine-zero-diff/review-request/engine-manifest.sha256`）ともバイト単位で一致しました（`cmp` で確認。第 2 教材の base から第 3 教材の base までの間も、エンジンは変わっていません）
- したがって、分類の表（後述）に埋める行はなく、人の承認が要る分類はありません

## 固定した commit と測った時点

| 項目 | 値 |
|---|---|
| 固定した commit（base） | `7dbc58a161a62faeef6a1aad221f8c947a1521a5`（#2010）。第 3 教材を最初に足した `c243e816ce82208eb606b70eebbdfaa67f5f5883`（#2014）の親 |
| 測った HEAD | `ae3cc2bed398908035120e13fdca27f1202072fa`（`origin/main`。#2015）。実装ブランチの HEAD も同じ commit から始め、`crates/`・`Cargo.*` は変えていない |
| 測った日付 | 2026-11-03 |
| 窓の中の commit | 2 件（`7dbc58a1` の次から測った HEAD まで。#2014・#2015） |
| 一覧の件数 | 1,628 件 |

固定した commit は、第 3 教材を足す直前です。道具は `base-before-pack` の項目で、教材のパックが base に無いことと、窓の中で最初に足した commit の親が base であることを毎回確かめます。差分が 0 になる窓を選んでいない、という確認です。
base 自身の `7dbc58a1`（#2010）は `docs/` の測定の記録だけを変えた commit で、第 3 教材の追加ではありません。

## 数える範囲と数えない範囲

| 区分 | 範囲 |
|---|---|
| 数える（判定する） | `crates/` の全体（`src`・`tests`・`static`・各 crate の `Cargo.toml`）、直下の `Cargo.toml`、`Cargo.lock` |
| 参考の行（数えないが出す） | `rust-toolchain.toml`（測定時は 0 行） |
| 数えない | `template-workshop/`（第 3 教材の本体と確認用の `probes/` を含む）、`docs/`（本記録を含む）、`scripts/`、`tools/`、`testdata/`、`.github/`、`docs/spec`、直下のその他のファイル |

数えない範囲の変更も隠さず、道具は上位のディレクトリごとの行数とファイル数を `INFO uncounted` の行で出します。測定時（`--head` は測った HEAD）は次のとおりです。

| 上位 | ファイル数 | 追加 | 削除 |
|---|---|---|---|
| `template-workshop/` | 22 | 973 | 1 |
| `tools/` | 4 | 431 | 0 |
| `docs/` | 1 | 34 | 0 |
| 直下のファイル | 2 | 2 | 2 |

## 3 つの方法

1. 固定した commit の記録: 上の表
2. `git diff --numstat --no-renames <base> <head> -- crates Cargo.toml Cargo.lock` の行数。移動も差分に数え、バイナリは 1 件の変更として数える
3. ファイルの SHA-256 の一覧の照合: `engine-manifest.sha256`（base の時点の 1,628 件。形は `sha256sum -c` と同じ `<sha256>  <path>`）。blob から作り、作業ツリーの改行の変換に左右されない。手で編集しない。作り直すときは `--write-manifest`

手の照合として、`git ls-tree -r --name-only 7dbc58a1 -- crates Cargo.toml Cargo.lock | wc -l` も 1,628 でした。

## 実測の結果（2026-11-03）

次のコマンドで、10 項目がすべて PASS でした（終了コード 0）。`worktree-clean` は、head が作業ツリーの HEAD でないと SKIP になることがあります。

```text
scripts/check-engine-zero-diff.sh --base 7dbc58a161a62faeef6a1aad221f8c947a1521a5 \
  --pack template-workshop/examples/task-breakdown \
  --manifest docs/engine-zero-diff/task-breakdown/engine-manifest.sha256 \
  --head ae3cc2bed398908035120e13fdca27f1202072fa
```

項目は `base-commit`・`base-before-pack`・`manifest-base`（1,628 件）・`manifest-head`（1,628 件）・`manifest-count`・`diff-lines`（0）・`cargo-toml`（0）・`cargo-lock`（0）・`crate-manifests`（0）・`worktree-clean` です。

負の対照として、`crates/core/src/lib.rs` に 1 行を足した作業ツリーで流すと、`worktree-clean` が FAIL になり、結果は `result: FAIL` でした（その後に戻し、`crates/` が clean であることを確かめた）。この実行の終了コードは取っていません。
引数なしの `make check-engine-zero-diff`（第 2 教材の記録）も同じ日に流し、差分 0 行（`result: engine diff is 0 lines`）でした。

あわせて、教材そのものの受け入れ条件を再確認しました（`fandhe-pack` は debug ビルド）。

| コマンド | 結果 |
|---|---|
| `fandhe-pack validate template-workshop/examples/task-breakdown` | `OK: 診断 0 件（形式 B(YAML)、章 1、モジュール 0、Step の定義 8）`、終了コード 0 |
| `fandhe-pack test` 同上 | 模範解答 1/1・誤入力 18/18・入った Step の実体 8/8、終了コード 0 |
| `validate`・`test` を `probes/views` に | 診断 0 件、模範解答 1/1・誤入力 2/2・実体 4/4、終了コード 0 |

## 分類の表の枠

差分が 0 のため、今回は行がありません。差分が出た場合は、道具の `FAIL` の出力（ファイルごとの行数と範囲に触れた commit）から 1 件ずつ埋め、採否は人の承認に送ります。

| 変更（commit・パス） | 分類 | 根拠 |
|---|---|---|
| （なし） | | |

- 汎用性不足: 教材の要件を満たす手段が語彙に無く、通常の教材追加が成り立たない変更
- 妥当な新拡張: 回避で書けて忠実さが下がるだけの変更を、題材に依らない名前つきの拡張点か、新しい語彙の版として足すもの

## CI について

第 2 教材の記録と同じ理由で、本体は CI に入れていません（checkout が浅く固定した commit が取れない。以後の別タスクがエンジンを変えると HEAD 既定の実行が落ちる）。再測定は、上の「測った HEAD」の SHA を `--head` に渡します。

## 限界

- 2 つの教材で差分が 0 でも、すべての教材で 0 とは言えません（SECOND-20）。次の教材も、追加の前の commit を base にして同じ道具を流します
- 同じ repo に置くため、「エンジンを外から使う」実証としては弱いです（SECOND-18）
- 第 3 教材は、エンジンのソースを読める環境で Claude Code が書き、試験も同じ作者が書きました。人は画面を見ていません。「エンジンのソースを見ない別の作者」の条件は満たしません
- 回避で書けて忠実さが下がる箇所は、差分 0 では見えません。それは `docs/known-gaps.md` と教材の記録（`template-workshop/examples/task-breakdown/docs/requirements.md` の 8 節）に残します
