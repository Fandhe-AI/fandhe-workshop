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

## LIC-1 エンジン repo のライセンス表記の確認（TASK-129）

ライセンスは D-6 で `MIT OR Apache-2.0` と決まっている。機械が `cargo metadata` で全 crate の `license` を、直下のファイルで 2 つの本文を確かめる（`make check-license`）。公開は TASK-122 で行う。

### 記録 1（2026-10-28）

| 欄 | 内容 |
|---|---|
| 確かめた版 | エンジン repo: `c6d064b3c19dd3af48f129bc8194e6bc62bfd64a`（照合のときの `main`）に本変更を載せた作業ブランチ |
| 日付 | 2026-10-28 |
| 方法 | `make check-license` が終了コード 0（4 members、license `MIT OR Apache-2.0`、本文 2 つ）。4 crate が `license.workspace = true` で `[workspace.package]` の値を継承していること、`LICENSE-MIT`・`LICENSE-APACHE` が直下にあることを目で確かめた。回帰テスト `make test-check-license` は 16 件が通った |
| 確かめた人 | AI（Claude Code）による機械の照合と目視。人の確認は未実施 |
| 承認 | 承認点なし（spec の TASK-129。ライセンスは D-6 で決定済み） |
| 限界 | 本文の照合は最小の目印の固定文字列だけで、完全一致は保証しない。`package.json` の `license` は crate ではないため対象外 |

## LIC-2 雛形のライセンス表記の確認（TASK-130）

雛形はエンジンと同じ `MIT OR Apache-2.0` で配布する（D-80）。初回はエンジン repo 内の `template-workshop/` に置く（D-4）ため、ルートと同じ本文の写しと README の節を置き、`make check-license` が一致と、雛形から作った repo への引き継ぎ（模擬）を確かめる。

### 記録 1（2026-10-28）

| 欄 | 内容 |
|---|---|
| 確かめた版 | エンジン repo: `003b2a25` の `main` に本変更を載せた作業ブランチ |
| 日付 | 2026-10-28 |
| 方法 | `make check-license` が終了コード 0。回帰テスト `make test-check-license` は 24 件が通った。`template-workshop/` の 2 ファイルがルートと `cmp` で一致し、README に `## ライセンス` の節があることを確かめた |
| 04 への反映 | spec の submodule（gitlink `d70ad10`）の `04-behavior/license-notice.md` で、LIC-2 が D-80 により確定し、期待が本変更の内容と合うことを確かめた。spec は編集していない |
| 確かめた人 | AI（Claude Code）による機械の照合と目視。人の確認は未実施 |
| 承認 | spec の TASK-130 は承認点なし（雛形のライセンスの方針は TASK-13 で決定済み）。Issue 側の「人間の承認点: あり」とは食い違うが、承認は得ていない |
| 限界 | 模擬は `cp -RP` でリポジトリの追跡対象かは見ない。GitHub のテンプレート機能そのものでは確かめていない。copyright の行は雛形のまま引き継ぐ。教材 repo 側の表記は TASK-131・TASK-782 |

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

## SAFE-16 学習分析・進捗の同期・提出の機能を持たないことの机上の照合（TASK-153）

### 記録 1（2026-10-29）

| 欄 | 内容 |
|---|---|
| 確かめた版 | エンジン repo: `b5c3af4b`（照合のときの `main`）に本変更を載せた作業ブランチ |
| 日付 | 2026-10-29 |
| 方法 | `make check-no-egress` が終了コード 0（通信・学習分析の crate 0 件、wasm32 の `web-sys` の feature に通信の型 0 件、npm の `dependencies` 無し）。配信する設定の実ビルドへの `scripts/check-no-egress.sh dist` が終了コード 0。ブラウザの計数 `egress-count.test.mjs` が G1・G2・C すべて通過（`docs/testing.md` 5.371） |
| 確かめた人 | AI（Claude Code。自動運転）による機械の照合。人の確認は未実施 |
| 承認 | 人間の承認点（CI のガードとして依存と配信物の検査を足すこと）は未承認。レビューで判断を求める |
| 限界 | `docs/known-gaps.md` の「SAFE-16: 通信の計数の範囲」 |

## EXT-1・EXT-2・EXT-3 CLI の橋渡し・backend・提出・LLM を持たないことの机上の照合（TASK-185 1/2）

### 記録 1（2026-10-29）

| 欄 | 内容 |
|---|---|
| 確かめた版 | エンジン repo: 本変更を載せた作業ブランチ（`main` の `d8af4bf2` が基点） |
| 日付 | 2026-10-29 |
| 方法 | `make check-no-egress` と `make test-check-no-egress` が終了コード 0。`SKIP_WASM_OPT=1 make build-web` の出力への `scripts/check-no-egress.sh dist dist/web` が終了コード 0（区分 SAFE-16・EXT-1〜EXT-3 とも拒否 0 件）。L2 の起動の引数は `tools/build-l2.test.mjs` の完全一致の固定に委ねる（`docs/testing.md` 5.380） |
| 確かめた人 | AI（Claude Code。自動運転）による機械の照合。人の確認は未実施 |
| 承認 | 人間の承認点（拡張を持たないことの検査を CI の失敗で止まる関門として足すこと）は未承認。レビューで判断を求める |
| 限界 | `docs/known-gaps.md` の「EXT-1〜EXT-3: 持たない機能の静的な検査の範囲」 |

### 記録 2（2026-10-29。TASK-185 2/2）

| 欄 | 内容 |
|---|---|
| 確かめた版 | エンジン repo: 本変更を載せた作業ブランチ（`main` の `800462e7` が基点） |
| 日付 | 2026-10-29 |
| 方法 | 自動ブラウザ `tools/browser/free-text-egress.test.mjs`（`make test-browser`）。自然文の送信で通信の試みが全区分 0 件（CSP あり・なし）、自己確認の観点が応答の行に出ること、対照の自己確認（`docs/testing.md` 5.382） |
| 確かめた人 | AI（Claude Code。自動運転）による機械の照合。人の確認は未実施 |
| 限界 | 「次の一歩」の案内が案内だけであること（ROUND2-24・ROUND2-25）は教材の内容の確認で、本 repo では確かめていない |

## DLV-10 配信物の公開名と読むクエリの記録（TASK-111 1/2）

DLV-10・SHELL-35 の確認の記録。`debug_` の公開名の記録は D-25 の扱いで成功基準にはしない（記録の手順を整えるだけである）。ただし配信する設定で 0 件でなければ、初回公開の前に直す。

読むクエリは `start`（直接リンク）の 1 つだけである。Issue #917 と spec の DLV-10・TASK-111 にある `?ir` の文面は D-68（2026-10-21）より前の書き方で、現在の IR は固定の `./ir/main.json` から読み、URL から場所を受けない（SAFE-33・TASK-168）。このため本記録は「直接リンクの 1 つ＝`start`」として確かめ、`app.js` に `ir` を読む処理は足していない。PoC 期の試験用クエリ 7 種（`persist`・`store`・`manual`・`tick`・`os`・`osmode`・`flood`）は読まない。

### 手順

1. `make build-web` で配信する設定を作る（wasm-opt を適用した既定のビルド）
2. `tools/check-delivery.sh` の終了コードが 0 であること
3. 配信物の `wasm/fandhe_workshop_web_bg.wasm` を `new WebAssembly.Module(...)` に渡し、`WebAssembly.Module.exports` の名前のうち `debug_` で始まるものを件数と一覧で記録する（期待は 0 件で、`mount_app` が有る）
4. `OUT_DIR=<一時の場所> TEST_HOOKS=1 tools/build-web.sh` で試験の設定を作り、手順 3 と同じ数え方で対照の件数を記録する。対照が 0 件なら数え方の空振りとして、この記録を無効にする
5. 配信物の `app.js` が `crates/web/static/app.js` と一致し、読むキーが `start` だけであることを、`cargo test -p fandhe-workshop-web --test tick_interval --test delivery_ports` の通過で確かめる
6. ブラウザの `tools/browser/query-inert.test.mjs`（試験用クエリ 7 種で挙動が変わらない）の通過を、手元または CI の `browser-test` の結果で記録する

#1924（TASK-82）の取り込み後は、手順 2 と 4 を `tools/check-delivery.sh --control <試験の設定の配信物>` の 1 回で行える（未マージの間は上の手順を使う）。

### 記録 1（2026-10-30）

| 欄 | 内容 |
|---|---|
| 確かめた版 | エンジン repo: 本変更を載せた作業ブランチ（`main` の `6474a357` が基点） |
| 日付 | 2026-10-30 |
| コマンド | `make build-web`、`tools/check-delivery.sh`、`OUT_DIR=<一時の場所> TEST_HOOKS=1 tools/build-web.sh`、手順 3 の書き出しの列挙、`cargo test -p fandhe-workshop-web --test tick_interval --test delivery_ports` |
| 配信する設定 | wasm-opt 適用済み。書き出しは 17 件、`debug_` で始まる名前は 0 件、`mount_app` あり。`check-delivery.sh` は終了コード 0（`RESULT: ok`、`FAIL` の行 0 件） |
| 試験の設定（対照） | 書き出しは 55 件、`debug_` で始まる名前は 37 件（`debug_autosave_state`・`debug_boot_kind`・`debug_check_doc` ほか）。0 件ではないため数え方は空振りしていない |
| 読むクエリ | 配信物の `app.js` と `crates/web/static/app.js` は `cmp` で一致。`new URLSearchParams(location.search).getAll("start")` の 1 か所だけ |
| 試験 | native: `delivery_ports` 11 件・`tick_interval` 6 件が通過。ブラウザの `query-inert`: この記録では未実施（CI の `browser-test` に任せる） |
| 確かめた人 | AI（Claude Code。自動運転）による機械の照合。人の確認は未実施 |
| 承認 | 人間の承認点なし（TASK-111）。得ていない承認は書かない |
| 限界 | BUILD-27 の `debug_` 5 件と実装の 37 件の差は #1924 と spec 側の扱い。wasm-opt を適用しない配信物では byte の検査が `name` セクション由来で FAIL になる（`docs/testing.md` 5.407 の限界 1）。spec の DLV-10 の `?ir` の文面の更新は spec repo で行う |

## DLV-11 CSP の meta と直接リンク 10 件の記録（TASK-111 2/2）

DLV-11・SAFE-13 の確認の記録。`index.html` の CSP の `<meta>` は TASK-151 1/2 で置き済みで、本件で変えていない。配置と内容は `csp-placement`、実行時の確認は `direct-link-csp`（`docs/testing.md` 5.410）が持つ。

### 記録 1（2026-10-31）

| 欄 | 内容 |
|---|---|
| 確かめた版 | エンジン repo: 本変更を載せた作業ブランチ（`main` の `4666ee32` が基点） |
| 日付 | 2026-10-31 |
| コマンド | `make build-web`、`FANDHE_WEB_DIR=<dist/web> node --test tools/browser/direct-link-csp.test.mjs` |
| 結果 | 直接リンク 10 件（CSP あり）で違反 0・CSP の警告 0・ページ内エラー 0・外部通信 0、開始点の Step まで WASM が動く。CSP なしと要求の種類が 10 件とも一致。`'wasm-unsafe-eval'` を外した対照で mount が error |
| 確かめた人 | AI（Claude Code。自動運転）による機械の照合。人の確認は未実施 |
| 承認 | 人間の承認点なし（TASK-111）。得ていない承認は書かない |
| 限界 | `<meta>` では効かない指令（DLV-12）、Pages の実物（DLV-24）、Windows（SAFE-39）は対象外 |

## SAFE-32・DLV-12 埋め込みの拒否の確認（TASK-167 2/2）

SAFE-32・DLV-12 の確認の記録。対策（案 A）は TASK-167 1/2 で実装済みで、本件は `iframe` に入れたときの試験と記録である。試験は `iframe-embed`（`docs/testing.md` 5.413）。

### 記録 1（2026-10-31）

| 欄 | 内容 |
|---|---|
| 確かめた版 | エンジン repo: 本変更を載せた作業ブランチ（`main` の `fdab5e82` が基点） |
| 日付 | 2026-10-31 |
| コマンド | `FANDHE_WEB_AUTHOR_DIR=<IR 同梱の配信物> node --test tools/browser/iframe-embed.test.mjs` |
| 結果 | 12 件（X・XS・SO・S・T・N × CSP あり・なし）が通過。別の起源・別のサイト・同じ起源の親の `iframe` で `data-mounted` が `framed`、操作要素・wasm・IR の要求・保存先が 0 件。関門を常に真にした対照では教材が動く |
| 確かめた人 | AI（Claude Code。自動運転）による機械の照合。人の確認は未実施 |
| 承認 | 人間の承認点なし（TASK-167）。得ていない承認は書かない |
| 限界 | 案 B（ヘッダ）は GitHub Pages で採れず、案 A は JS が無効のとき効かず HTTP ヘッダほど強くない。Pages の実物の応答（D-29）、Windows は対象外。Chromium だけ。`make test-browser` の通しは CI に任せる |

## SAFE-38 公開前の安全性の確認（TASK-170）

確認の方法と担当は D-78。公開する配信物（配信する設定でビルドしたもの）に対して SAFE-1〜SAFE-7 と SAFE-13 の自動ブラウザ試験を CSP あり・なしの両方で流し、
実行 0・通信 0・CSP の違反 0 を確かめる。記録の種類は D-28 の代わりの確認（AI）で、ユーザー（リポジトリの管理者）が記録を承認する。
確認は公開の直前に行う。記録が無い状態、または承認の無い状態では公開しない。各試験の中身と手元の変異の記録は `docs/testing.md` の各節に置き、ここでは重複させない。

### 記録 1（2026-10-31。試験用の教材での予行）

| 欄 | 内容 |
|---|---|
| 確認の種類 | D-28 の代わりの確認（AI）。D-78 が SAFE-38 の記録の種類と決めたもの。人によるセキュリティレビューではない |
| 確かめた版 | エンジン repo: `4637eef3b6f8eca617dc22e61a3d80fbd76adb89`（`main`。作業ツリーに変更なし）。`docs/spec` の submodule: `d70ad10760234049beae9cd0130291231f86fb09` |
| 日付 | 2026-10-31 |
| 配信物 | 配信する設定（`TEST_HOOKS` 空・release・`wasm-opt -Os` 適用。wasm-opt version 116。WASM の `gzip -9` 439,218 bytes。WASM の sha256 `5056bc2e929290a82b5af25dab7ad6be4a4203813e5172198ac24864bc276002`）。IR を同梱した配信物は、同じ設定に `IR_FILE`（`crates/web/tests/data/ch0_3_pack` を `fandhe-pack compile` した IR）を渡して作った。IR の有無の 2 つの配信物は、`index.html` の `fandhe-ir` の meta 1 行と `ir/main.json` の有無だけが違い、WASM・JS・CSS はバイト単位で同一だった（`diff -r`）。同梱の配信物に `tools/check-delivery.sh --control <試験の設定>` を流し、終了コード 0（`RESULT: ok`。試験用の口 0 件） |
| 注入の口 | SAFE-1〜SAFE-5・SAFE-7 の試験は、配信する設定の配信物の一時の写しに `fandhe-ir` の meta と、件ごとに作った `ir/main.json` を足して開く（IR を同梱した配信物と同じ形）。注入した値は件ごとの IR・履歴・入力欄・URL の側にあり、エンジンの配信物（WASM・JS・HTML・CSS）は上の 1 組のまま変えていない。SAFE-6 の場面ごとの配信物は「試験ごとの結果」の SAFE-6 の行に書く |
| 試験の設定 | 対照、SAFE-6 の `data-payload` の場面の大半、SAFE-13 の T 系の試験に使う試験の設定（`TEST_HOOKS=1`・`SKIP_WASM_OPT=1`）を同じ版で作った |
| コマンド | `node --test --test-concurrency=1` で下の 17 ファイルを 1 回で流した（`FANDHE_WEB_DIR`＝配信する設定、`FANDHE_WEB_AUTHOR_DIR`＝IR を同梱した配信する設定、`FANDHE_WEB_TEST_DIR`＝試験の設定、`FANDHE_PACK_BIN`＝`scripts/build-pack-bin.sh` の出力）。ブラウザは Playwright の Chromium（headless）。所要は約 24 分 |
| 結果（全体） | 183 件が通過、失敗 0・取り消し 0・飛ばし 0（終了コード 0）。各試験は実行（記録用の関数の呼び出し）0・配信元以外への通信 0・CSP の違反 0・ページ内エラー 0 を assert し、0 でなければ失敗にする |
| 確かめた人 | AI（Claude Code。自動運転）による自動ブラウザの試験の実行と記録の下書き。人の確認は未実施 |
| 承認 | 未承認（ユーザーの承認待ち。承認者: ユーザー（リポジトリの管理者）） |
| 次回 | 公開する教材 repo（TASK-36）の IR を同梱した、公開する版の配信物で、TASK-122（DLV-28）の直前に流し直す。版・配信物・IR が本記録と違えば、本記録では公開しない |

#### 試験ごとの結果

| ビヘイビア | 試験（`tools/browser/`） | CSP | 件数・計数 | `docs/testing.md` |
|---|---|---|---|---|
| SAFE-1（E1。Step の本文） | `pack-body-xss`（5 件） | あり・なし | 49 件 × 2。全区分 0、生の HTML が文字として出た箇所 66（各回）、外部の画像の代わりの文字 3 件。対照 D が数える | 5.175 |
| SAFE-2（E2。構造定義の欄と ID） | `structure-xss`（13 件）・`e2-ir`（11 件） | あり・なし | 欄の変種 55 件 × 開始点 2（s-0-1・s-1-4）で 110 回中 110 回が文字として出た（各 CSP）。ID の変種 55 件 × 2 で全区分 0。対照 D1・D2 が働く | 5.366 |
| SAFE-3（E3。練習データ） | `practice-data-xss`（9 件）・`e3-ir`（6 件） | あり・なし | 50 件 × 開始点 2 で 100 回中 100 回（各 CSP）。属性の値だけの変種は画面に出ない（F）。対照 D1・D2 が働く | 5.367 |
| SAFE-4（E4。履歴ファイル） | `history-xss`（10 件）・`history-regular`（6 件）・`e4-history`（7 件） | あり・なし | 置き換えた履歴 14 件 × 3 変種（ops・snapshot・meta）× 2 経路（ファイル・保存先）の 84 回中 84 回が断られ、断る文面は固定（各 CSP）。正規の経路は 14 件中 14 件が保存・復元・読み込みの後も文字として出た（restore 14・file 14） | 5.398・5.400 |
| SAFE-5（E5。入力欄） | `input-field-xss`（6 件）・`xss-cases`（9 件） | あり・なし | 49 件 × 3 通りで 147 回中 147 回がログに文字として出た（各 CSP） | 5.368 |
| SAFE-6（操作要素の書き換え） | `data-payload-tamper`（21 件）・`data-payload-cases`（3 件） | あり・なし | `data-payload` の click 2,530 回（値が元と一致して除いた組 18）＋ `data-action` の click 50 回 = 2,580 回（PoC-7 の 377 回以上）。ただし配信物は場面で違う。`data-payload` の 6 場面（開いた直後・幅 959px の tablist・章の終わり・承認・枠の通知・試験の設定の `data-action`）は、場面へ進むのに試験用の口が要るため**試験の設定**で流した。配信する設定で流したのは、IR を同梱した配信物での `submit-input` の `data-action` の書き換え（4 名 × click・Enter と、`raw` に正しい Op を置いた 1 回）、IR なしの配信物での `data-action` の書き換えと枠の口の `data-payload` の書き換え（7 項目）だけである | 5.369・5.372 |
| SAFE-7（E6。URL のクエリとハッシュ） | `url-xss`（5 件）・`url-cases`（4 件） | あり・なし | 61 回中 61 回で値が画面に出ず、取得する資源がクエリなしと同じ（各 CSP）。囮の対照 D3 が働く | 5.370 |
| SAFE-13（CSP の meta と計数） | `csp-placement`（34 件） | 該当なし（静的な検査） | 配信する設定・試験の設定とも、CSP の meta が `<meta charset>` の直後で内容が仕様と一致。加工の対照 30 件で不合格になる | 5.176 |
| SAFE-13 | `inline-free`（24 件） | あり・なし（ブラウザの対照） | インラインの script・style・`on*`・`data:`・外部の取得が 0 件。加工の対照で不合格。`data:` の画像は CSP ありで違反ちょうど 1 件・CSP なしで 0 件 | 5.180 |
| SAFE-13 | `csp-runs`（10 件） | 各試験の中で あり・なし | D1（配信する設定の最小の画面）・D2（CH-4〜CH-9 の形の 17 Step）・D3（CH-0〜CH-3 の形の 20 Step）と、試験の設定の T1〜T6 で、通信・違反・CSP 警告・ページ内エラーが 0 件、CSP の有無で配信元への要求の種類が同じ | 5.180・5.285・5.321 |

#### 限界

- 公開する本物の配信物ではない。公開する教材 repo（TASK-36）が未作成で、同梱した IR は試験用の教材（`ch0_3_pack`）である。SAFE-1〜SAFE-7 の試験は件ごとに IR を入れ替えるため、本物の教材の IR を同梱した配信物そのものは開いていない。エンジンの配信物は IR の有無に依らず同一であることを `diff -r` で確かめたが、本物の教材の本文・練習データの描画はこの試験の範囲外である
- SAFE-38 は確認を公開の直前に行うとする。本記録は公開の前の下書きで、公開する版で流し直すまでは公開の根拠にしない
- 試験用の入力（E1 49・E2 55・E3 50・E4 14・E5 49・E6 8 と `?ir=` の固定の値 5）は網羅ではなく、試した入力で実行されなかったことだけを示す。人のセキュリティレビューの代わりにならない
- SAFE-6 の `data-payload` の書き換え（操作要素 10 種 × 13 値）は、IR を同梱した配信する設定の配信物では流れていない（`docs/testing.md` 5.369・5.372 の限界のまま）。配信する設定には試験用の口が無く、試験がその場面へ進めないためで、閉じるには試験の変更が要る。IR を同梱した配信物で確かめたのは `data-action` の書き換えだけである
- 試験の作成者と実装の作成者、本記録の作成者が同じ AI である
- ブラウザは Linux の Playwright の Chromium（headless）だけ。基準端末の Chrome の原本（`FANDHE_BROWSER_CHANNEL=chrome`）、macOS・Windows（SAFE-39・TASK-171）、GitHub Pages の実物の応答（DLV-24）、`<meta>` では効かない CSP の指令（DLV-12）は対象外
- 比べた保存先は試験ごとの範囲（多くは localStorage）で、IndexedDB の比較は各試験の限界の節に従う
- `make test-browser` の全体は流していない。流したのは上の 17 ファイルだけ

#### D-34 の区別

D-34 の区別に従い、ユーザー自身の体験や AI の代理は人の評価に数えない。本記録の試験は AI が流した自動ブラウザの試験で、人の評価は 0 件である。
D-28 の代わりの確認は、満たしたと扱ってよい基準を広げるだけで、証拠の種類（自動ブラウザ）は書き換えず、人の評価ではない。
ユーザーの承認は記録の承認であり、人によるセキュリティレビューを行ったことにはならない。

## AUTHOR-14 作成者向けの実行ファイルの公開前の確認（TASK-415 2/2）

対象は作成者向けの実行ファイル `fandhe-pack`（Apple シリコンの macOS 用）である。公開は GitHub Release で行い、外部への公開にあたる。最終の承認はユーザー（リポジトリの管理者）が行う（D-78）。承認は得ていない。下の記録は、承認を待つ下書きである。

### 記録 1（2026-11-01）

| 項目 | 内容 |
|---|---|
| 確かめた版 | 作業ブランチの基点の `main`（`14bca9c7`）に本変更を足したもの。公開する版（`0.0.0` のまま。下の「ユーザーが決める事項」）ではない |
| 方法 | `make test-build-pack-release`（偽の実行ファイルで、梱包の中身・案内文の一致・`xattr` と `shasum` の記載・sha256 の照合・失敗時に成果物が出ないこと・案内文の元が無いときの終了コード 2 を確かめた）。実ビルドの確認は `release.yml` の run が行い、本記録の時点では未実施 |
| 確かめた人 | AI（Claude Code。自動運転）による機械の照合。人の確認は未実施 |
| 承認 | 未承認（承認者: ユーザー（リポジトリの管理者）） |
| 次回 | 公開する版で `release.yml` を流し、下の関門を満たしたうえで流し直す |

### 公開の関門

次がすべて揃うまで公開しない。

1. SAFE-38（TASK-170）の記録が、公開する版で流し直され、ユーザーが承認している。「SAFE-38 公開前の安全性の確認」の記録 1 は試験用の教材での予行で、未承認であり、対象は web の配信物だけである。実行ファイルは含まない
2. LIC-12（TASK-139・TASK-794）の照合を、公開する版で行っている
3. 本節の記録 1 を、ユーザーが承認している
4. 下の「ユーザーが決める事項」が決まっている

### 公開の手順（ユーザーが行う）

1. `gh workflow run release.yml` で成果物を作る
2. `gh run download` で取得する
3. `shasum -a 256 -c` で照合する
4. `gh release create <tag> --draft` で、tar.gz と `.sha256` を添付した draft を作る
5. draft を目で確かめてから公開する

tag の名前と版は TASK-791 の決定に従う。

### ユーザーが決める事項

- LIC-9: 2026-11-01 にユーザーが「同梱する」と決めた。#1957 で、依存のライセンス文を tar.gz の `THIRD-PARTY-LICENSES` として同梱した（`scripts/gen-third-party-licenses.sh`。`Cargo.lock` から決定的に作り、通信しない）
    - 対象は、`fandhe-pack`（`--target` の triple）と、埋め込んだ web（`wasm32-unknown-unknown`）の通常の依存。proc-macro の crate・ビルド時と試験だけの依存・ワークスペースのメンバーは除く
    - 許可する license の識別子は `MIT`・`Apache-2.0`・`Unlicense`・`BSL-1.0`・`Unicode-3.0` で、OR の一方でも外れる式・`WITH`・license の欠如・registry でない source・symlink のライセンスのファイルがあれば、配布物を作らずに失敗する
    - 本文は crate の配布物のライセンスのファイルから取る。ファイルを持たない `fandhe-frontend-*` の 7 件は、SPDX の標準の本文（`tools/pack-release/licenses/MIT.txt` と `LICENSE-APACHE`）で補い、ヘッダの一覧に示す。著作権者は各 crate の repository の記載に従う
- 成果物の名前の版は `0.0.0` のままとする。公開する版は TASK-791 で決める（未決）

### 採らなかった案

`release.yml` に `contents: write` の draft の Release を作るジョブを足す案。書き込み権限と入力の検証が要り、承認の無い自動運転で権限を広げることになるため採らなかった。採るかどうかはユーザーが判断する。

### 限界

- 案内文は、人が実機で quarantine を外して起動することをまだ確かめていない。Gatekeeper の画面は誰も観察していない
- Apple シリコン以外は対象外（TASK-98・D-19 の結果を待つ）
- 試験の作成者・実装者・記録の作成者が同じ AI である
- `THIRD-PARTY-LICENSES` の 43 件は Linux の手元（2026-11-01）で計算した値である。`release.yml` の run での照合は記録 2 で行う
- ライセンス文の生成の前提は `cargo fetch --locked` である（`cargo metadata` を offline で流すため）
- wasm-bindgen の CLI が生成する JS の glue は、wasm-bindgen の crate の区画で扱っている

### D-34 の区別

AI の代理は人の評価に数えない。ユーザーの承認は記録の承認であり、人によるセキュリティレビューを行ったことにはならない。
