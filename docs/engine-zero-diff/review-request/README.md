# エンジンの差分 0 の確認（第 2 教材「レビューの頼み方」。TASK-752）

第 2 教材 `template-workshop/examples/review-request/` が、エンジンの Rust を変えずに足せたことを、数えて示す記録です（SECOND-1・W-20・D-91）。
道具は `scripts/check-engine-zero-diff.sh`（`make check-engine-zero-diff`）、回帰テストは `scripts/test-check-engine-zero-diff.sh` です。

## 結論

- 固定した commit から測った HEAD までの間で、数える範囲（`crates/`・直下の `Cargo.toml`・`Cargo.lock`）の差分は 0 行でした
- 内訳は、直下の `Cargo.toml` が 0 行、`Cargo.lock` が 0 行、`crates/*/Cargo.toml` が 0 行、`crates/` 全体が 0 行です
- SHA-256 の一覧は 1,628 件で、固定した commit から作り直した一覧とバイト単位で一致し、測った HEAD の 1,628 件とも全件一致しました
- したがって、分類の表（後述）に埋める行はありません

## 固定した commit と測った時点

| 項目 | 値 |
|---|---|
| 固定した commit（base） | `505fef25de073875d814500e76a9e597e4513639`（#1988。TASK-758）。第 2 教材を最初に足した `e2db6bd0a642ae9bed4ea19a7c93205f40e27ea3`（#1989）の親 |
| 測った HEAD | `cddeca51a9b3ef9c9669e01a5b5a54edf6a11fd1`（`origin/main`。#2002）。実装ブランチの HEAD も同じ commit から始め、`crates/`・`Cargo.*` は変えていない |
| 測った日付 | 2026-11-03 |
| 窓の中の commit | 14 件（`505fef25` の次から測った HEAD まで） |

固定した commit は、第 2 教材を足す直前です。教材をエンジンの repo の雛形の例に置く（D-91）ため、追加の前の commit を基点にする必要があります。
道具は `base-before-pack` の項目で、教材のパスが base に無いことと、窓の中で最初に足した commit の親が base であることを毎回確かめます。差分が 0 になる窓を選んでいない、という確認です。

窓の手前にある、第 2 教材に近い commit を、窓の外に置いた理由と合わせて挙げます。

- `45ff2afc`・`a29e0321`（TASK-748）: `tools/browser/`・`scripts/`・`docs/` だけを変え、`crates/` は変えていません。題材に依らない完走の道具で、教材の追加ではありません
- `505fef25`（TASK-758）自身: `crates/pack/tests/examples_pack.rs` に 57 行を足しました。`docs/examples/choice-only-pack` の扱い（D-92）の試験で、第 2 教材の追加ではありません。base に含まれるため差分には出ません

## 数える範囲と数えない範囲

| 区分 | 範囲 |
|---|---|
| 数える（判定する） | `crates/` の全体（`src`・`tests`・`static`・各 crate の `Cargo.toml`）、直下の `Cargo.toml`、`Cargo.lock` |
| 参考の行（数えないが出す） | `rust-toolchain.toml`（測定時は 0 行） |
| 数えない | `template-workshop/`（雛形と教材の例。第 2 教材の本体を含む）、`docs/`（`docs/examples/`・`docs/second-pack-repro/`・本記録を含む）、`scripts/`、`tools/`、`testdata/`、`.github/`、`docs/spec`、直下のその他のファイル |

教材と雛形のディレクトリ `template-workshop/` は数えません。教材の追加はここに現れるのが正しいためです。
数えない範囲の変更も隠さず、道具は上位のディレクトリごとの行数とファイル数を `INFO uncounted` の行で出します。測定時は次のとおりです。

| 上位 | ファイル数 | 追加 | 削除 |
|---|---|---|---|
| `template-workshop/` | 18 | 2,964 | 1 |
| `docs/` | 21 | 2,476 | 7 |
| `tools/` | 9 | 2,308 | 42 |
| `scripts/` | 7 | 1,219 | 0 |
| 直下のファイル | 4 | 27 | 4 |

## 3 つの方法

1. 固定した commit の記録: 上の表
2. `git diff --numstat --no-renames <base> <head> -- crates Cargo.toml Cargo.lock` の行数。移動も差分に数え、バイナリは 1 件の変更として数える
3. ファイルの SHA-256 の一覧の照合: `engine-manifest.sha256`（base の時点の 1,628 件。形は `sha256sum -c` と同じ `<sha256>  <path>`）。git の blob から作り、作業ツリーの改行の変換に左右されない。手で編集しない。作り直すときは `scripts/check-engine-zero-diff.sh --base 505fef25de073875d814500e76a9e597e4513639 --manifest docs/engine-zero-diff/review-request/engine-manifest.sha256 --write-manifest`（個別の指定モードなので `--base` と `--manifest` が必須）

手の照合として、`git ls-tree -r --name-only 505fef25 -- crates Cargo.toml Cargo.lock | wc -l` も 1,628 でした。

## 実測の結果（2026-11-03）

`make check-engine-zero-diff`（head は実装ブランチの HEAD。作業ツリーは clean）は、次の 10 項目がすべて PASS で終了コード 0 でした。
`base-commit`・`base-before-pack`・`manifest-base`（1,628 件）・`manifest-head`（1,628 件）・`manifest-count`・`diff-lines`（0）・`cargo-toml`（0）・`cargo-lock`（0）・`crate-manifests`（0）・`worktree-clean`。
`--head origin/main` でも同じ結果でした。

負の対照として、`crates/core/src/lib.rs` に 1 行を足した作業ツリーで流すと、`worktree-clean` が FAIL になり終了コード 1 でした（その後に戻した）。
コミットした差分は `diff-lines` と `manifest-head` が検出します。この経路は回帰テストの見本の repo が確かめます。

## 分類の表の枠

差分が 0 のため、今回は行がありません。差分が出た場合は、道具の `FAIL` の出力（ファイルごとの行数と範囲に触れた commit）から 1 件ずつ埋めます。

| 変更（commit・パス） | 分類 | 根拠 |
|---|---|---|
| （なし） | | |

- 汎用性不足: 教材の要件を満たす手段が語彙に無く、通常の教材追加が成り立たない変更
- 妥当な新拡張: 回避で書けて忠実さが下がるだけの変更を、題材に依らない名前つきの拡張点か、新しい語彙の版として足すもの
- 汎用性不足に当たりうる変更があれば、W-20 の扱いと SECOND-1 を直す差分の案を、人の判断に送ります。仕様の repo は本 repo から編集しません。分類の採否と W-20 の扱いの承認は、差分が出た場合にだけ生じます

## CI について

CI（`check` ジョブ）は回帰テスト `make test-check-engine-zero-diff` だけを流します。本体は、次の理由で CI に入れていません。

- `ci.yml` の checkout は浅く（深さ 2）、固定した commit が取れません

`make check-engine-zero-diff` は、記録の再現です（#2038）。扱いは次のとおりです。

- 既定の head は、このディレクトリの `record.txt`（`base`・`head`・`pack`）に書いた「測った HEAD」です。いまの HEAD ではないので、記録の後にエンジンが変わっても落ちません
- いまの HEAD を測るときは `--head HEAD`（任意の rev）を明示します。エンジンが変わっていれば FAIL するのが正しい挙動です
- `record.txt` が機械可読な正で、上の表と食い違うと `record-readme` の項目が FAIL します
- 既定の実行では head が作業ツリーの HEAD でないため、`worktree-clean` は SKIP になります。退行ではありません
- 第 3 教材の記録（`../task-breakdown/`）も同じ入口で再現します。1 件に絞るときは `--record docs/engine-zero-diff/review-request` を渡します

## 限界

- 1 つの教材で差分が 0 でも、すべての教材で 0 とは言えません（SECOND-20）。教材ごとに、教材の追加の前の commit を base にして同じ道具を流します（`--base`・`--pack`・`--manifest`）
- 同じ repo に置くため、「エンジンを外から使う」実証としては弱いです（SECOND-18）
- 現在の第 2 教材は、エンジンのソースを読める環境で Claude Code が書きました。「エンジンのソースを見ない別の作者」の条件は満たしません。その条件の確認は TASK-413 の記録（`docs/second-pack-repro/`）が入力です。その写しは、作りの上で数えない範囲に入ります
- 回避で書けて忠実さが下がる箇所は、差分 0 では見えません。それは `docs/known-gaps.md` に記録します
- 仕様の PoC-5 の `prototype/` は証拠の出所としてだけ挙げ、振る舞いの根拠にはしていません
