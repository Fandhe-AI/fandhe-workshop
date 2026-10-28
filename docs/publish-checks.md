# 公開前の確認の記録

公開する版について、公開前に行う確認の結果を残す文書（版 1）。確認ごとに節を足し、既存の節は書き換えずに日付つきで追記する。
最終の承認は、リポジトリの管理者であるユーザーが行う。public の repo に人名を書くこと自体が社内固有の情報（D-77 の「社内の人名」）に
あたるため、確かめた人は役割名で書く。

## LIC-12 社内固有の情報・認証情報が 0 件であることの確認（TASK-139）

確認の方法と担当は D-78。機械が「社内固有の語の一覧」と認証情報の形を照合し、架空の名前が実在の private repo と紛らわしくないかは人が判断する。
確認は公開する版で 2 回行う（本節の記録と、版を変えた後の TASK-794）。

### 記録 1（2026-10-26）

| 欄 | 内容 |
|---|---|
| 確かめた版 | エンジン repo: `8b8d50700ee5883bd510be1525a2b22116555de7`（照合の実行時の `main`）。`docs/spec` の submodule: `d70ad10760234049beae9cd0130291231f86fb09` |
| 日付 | 2026-10-26 |
| 照合のスクリプト | `tools/check-internal-info.mjs`（回帰テストは `make test-check-internal-info`。9 件が通った）。実行したコマンドは `make check-internal-info-secrets` |
| 対象 | 認証情報の形: 追跡中の全ファイル 3,779 件（外部取得のスキル `.agents/skills/**` を含む。gitlink の `docs/spec` は対象外）。社内固有の語（実施する場合）: `docs/examples/**`・`testdata/**`・`crates/web/static/**`・`.github/workflows/**`。教材 repo（TASK-36）と講師資料は本 repo の外にあり、本記録では照合していない（TASK-794 で照合する） |
| 認証情報の形 | 終了コード 0（違反 0 件）。コアの 15 形で照合した。固定した例外は `.agents/skills/` 配下の 4 ファイル（確かめた内容の SHA-256 で固定。内容が変われば例外を適用せず再確認を求める）で、いずれも `u:p@h`・`${TOKEN}`・仕様書の例の JWT のようなダミーか変数の展開であることを目で確かめた。例外の一覧と理由は `tools/check-internal-info.mjs` の `SECRET_EXCEPTIONS` |
| 確かめた人（認証情報） | AI（Claude Code）による機械の照合と、例外の目視。人の確認は未実施 |
| 社内固有の語 | 未実施。語の一覧が private の spec repo に未作成（D-77・D-81）。一覧ができたら、ユーザーが `WORDS=<一覧のパス> make check-internal-info` を実行し、件数と一覧の sha256（標準エラーの `list:` 行）をここへ追記する |
| 紛らわしい名前の人の判断 | 未実施。判断の対象の下見: `testdata/` の教材の `workshop.yaml` などに `Fandhe-AI/template-ideas`（組織名つきの repo 名）が現れる。`testdata/README.md`・`testdata/version-histories/README.md` は private の仕様 repo の名前を参照している。実在の private repo と紛らわしいかは人が判断する |
| 承認 | 未承認（承認者: ユーザー（リポジトリの管理者）） |
| 次回 | TASK-794 で、公開する版について照合をやり直す。その版が本記録より後であることを記録に書く |

### 本 repo の範囲外として扱ったもの

`README.md`・`CLAUDE.md`・`.gitmodules` などは private の仕様 repo の名前を参照している。LIC-12 の 4 種類（教材パック・練習データ・講師資料・教材 repo の CI 設定）の外にある。
エンジン repo の文書として許容するかは、ユーザーが判断する。

### 追記 1（2026-10-26。TASK-420 2/2・D-81）

- 教材パックの練習データについて、実在を疑う private repo・社内 runner の名前は `fandhe-pack validate` では調べない。本確認（社内固有の語の一覧の照合と、紛らわしさの人の判断）の対象とする。
- `validate` が調べるのは、秘密情報らしい形（`E-SECRET-SHAPE`）と、練習データの見出しの【架空】の印の欠け（`E-FICTIONAL-MARK`）だけである。印があっても内容が本物でないことの保証にはならない。
- 教材 repo の練習データは、TASK-794 の照合の対象に含める。

## DLV-27 WASM の大きさの基点そろえ（TASK-126）

配信する WASM の `gzip -9` の大きさを、同じ commit・同じビルド設定で測り直した記録。値と基点つきの一覧は `docs/testing.md` 5.361 に置き、ここでは重複させない。

### 記録 1（2026-10-28）

| 欄 | 内容 |
|---|---|
| 確かめた版 | エンジン repo: `c6d064b3c19dd3af48f129bc8194e6bc62bfd64a`（`dirty: false`） |
| 日付 | 2026-10-28 |
| コマンド | `scripts/check-wasm-size.sh --require-wasm-opt`（2 回流して値が一致） |
| 値 | `gzip -9` 410,548 bytes（上限 1,048,576 bytes 以下）。`wasm-opt -Os` 適用・配信する設定・release |
| 確かめた人 | AI（Claude Code）による機械の測定。人の確認は未実施 |
| 承認 | 未承認（CI のガードの変更の採否を含む。CI は引き続き `wasm-opt` 未適用の値で判定する） |
| 次回 | 復習の画面が入った後と、TASK-122（DLV-28）の直前に測り直す。手順は 5.361 |
