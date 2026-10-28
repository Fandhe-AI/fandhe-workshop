# CLAUDE.md

Claude Code がこのリポジトリで作業するときの要約。人向けの詳しい説明（セットアップ・コマンド・CI・MCP）は
`README.md` に置き、ここでは重複させない。

## Overview

fandhe-workshop は、体験型教材を作成・実行する汎用 OSS エンジンの実装リポジトリ（public）。
教材作成者が章・Step・入力条件・状態変化・完了条件・ヒントをデータとして書くと、ブラウザの中で、ターミナル風の
操作に応じて成果物が変わる教材として動くことを目指す。エンジンは特定の教材の知識を持たない。

- 開発基盤は `Fandhe-AI/template-dev` から作成した。現時点の中身は Make・シェルスクリプト・lefthook・GitHub Actions
- Cargo ワークスペース（4 crate）を導入済み。pack crate は読み込み層と `fandhe-pack validate <pack-dir>`・`compile <pack-dir> [--chapters <id>,...]`・`compare <pack-dir-a> <pack-dir-b>`・`metrics <pack-dir>`・`vocab <pack-dir>`・`test <pack-dir>`（枠・終了コード・PASS／FAIL の行・集計・R-22／R-21 の行・入った Step の実体の行。最終行は後続）を実装済みで、他は骨格中心（2026-10-15 時点）。Rust のコーディング規約は `coding-rust.md`。エンジンの実装・Rust 向けの builder Agent は、タスクに沿って追加する
- 仕様は private の submodule `docs/spec`（`Fandhe-AI/fandhe-workshop-spec`）。取得手順は README の「仕様」

## Repository Structure

```text
.
├── CLAUDE.md                 # 本ファイル
├── AGENTS.md                 # レビュー観点集（ai-review が PR の base コミットから読む。P0 / P1 / P2）
├── README.md                 # 人向けの説明（構成・セットアップ・コマンド・CI・MCP）
├── Cargo.toml / Cargo.lock   # Cargo ワークスペース（仮想 manifest）と固定した依存の版
├── crates/                   # contract / core / pack / web の 4 crate（pack は compile・compare・metrics・vocab・validate まで実装済み）
├── testdata/                 # 試験データ（spec repo の写し。出所は testdata/README.md。整形ツールの対象外）
├── rust-toolchain.toml       # Rust の版・component・wasm32 ターゲットの固定
├── package.json / package-lock.json / .npmrc  # Node・Playwright の版の固定（ブラウザ試験用）
├── docs/toolchain.md         # 固定した版と前提の一か所の正（check-toolchain が一致を検査）
├── docs/dependency-inventory.md # 依存の棚卸しの手順と日付つきの結果の記録（BUILD-25・D-58。実行は make inventory-deps）
├── docs/measurement.md       # 測定の結果の様式（基点の 4 項目と比較の規則。実行は make measure）
├── docs/testing.md           # 性質テストと変異注入の手順・記録の様式（変異の定義は crates/*/tests/mutations/）
├── Makefile                  # scripts/ を呼ぶだけの薄い入口（make help で一覧）
├── lefthook.yml              # Git hooks 定義（pre-commit / commit-msg）
├── tools/                    # spec が置き場を指定したスクリプト（build-web.sh・check-deps.sh・check-delivery.sh・clean-room.sh・serve.mjs・inventory-deps.mjs・upstream-impact.mjs・build-l2.mjs・l2/・browser/・measure/。規約・shellcheck は scripts/ と同じ）
├── scripts/
│   ├── lib.sh                # 共有ヘルパー（require_cmd / reject_args / log）
│   ├── help.sh / doctor.sh / setup.sh / check.sh
│   ├── test-keydown-control.sh # action-keydown を外した対照の検査（make test-keydown-control）
│   ├── test-chapter-key-control.sh # 鍵を修正前の固定の文字列にした対照の検査（make test-chapter-key-control）
│   ├── check-mutation-defs.sh # 変異の定義の find の一意性と expect-fail の名前の実在の静的検査（make check に含む。回帰テストは test-check-mutation-defs.sh。共有は lib-mutations.sh）
│   ├── check-build-inventory.sh # Cargo.lock の private な依存 0 件と経路の secret らしい語 0 件の棚卸し検査（回帰テストは test-check-build-inventory.sh）
│   ├── check-mutations.sh    # 変異注入の検査（回帰テストは test-check-mutations.sh）
│   ├── check-toolchain.sh    # 固定ファイルと docs/toolchain.md の版の一致の検査（回帰テストは test-check-toolchain.sh）
│   ├── check-workflow-secrets.sh # ci.yml が secret を参照しないことの検査（回帰テストは test-check-workflow-secrets.sh）
│   ├── check-fork-ci.sh      # fork が secret 無しで CI を通せる構成か（A〜C）と教材 repo の ENGINE_REV の正準の形（D。指定時のみ）をパーサ復号で検査（回帰テストは test-check-fork-ci.sh。ruby・jq・cargo が必要）
│   ├── ci-rust-toolchain.sh  # CI で導入された rustc の版と docs/toolchain.md の照合
│   ├── check-frontend-bump.sh # 版上げ PR の受け入れ確認 (1)〜(5) の関門（detect・guard・counts・wasm-gzip。回帰テストは test-check-frontend-bump.sh。期待値は expected-test-counts.txt、更新は update-test-counts.sh）
│   ├── check-wasm-size.sh    # 配信する設定の WASM の gzip -9 が 1 MB 以下であることの関門（回帰テストは test-check-wasm-size.sh）
│   ├── ci-changes.sh         # CI の test-hooks-macos の実行要否の判定
│   └── hooks/                # lefthook から呼ばれるフックの実体と回帰テスト
├── .github/workflows/
│   ├── ci.yml                # changes / check / rust-lint / rust-test / browser-test / test-hooks / test-hooks-macos / pr-title / ci-complete
│   ├── deploy.yml            # 配信の骨格（workflow_dispatch のみ・permissions {}・fork では skip・未実装で失敗。TASK-121 で実装）
│   ├── ai-review.yml         # codex による PR 自動レビュー
│   └── update-external.yml   # スキル・submodule の日次追従 PR
├── .claude/
│   ├── agents/<category>/    # Sub-agents（research / implement / testing / quality / docs）
│   ├── rules/                # ルール（一部は paths で読み込み条件を限定）
│   ├── settings.json         # 共有設定（SessionStart hook）
│   ├── skills/               # .agents/skills/ への symlink
│   └── workflows/            # implement-issue-tree.js（スキル側の実体への symlink）
├── .agents/skills/           # スキルの実体（skills-lock.json で管理する外部取得物）
├── skills-lock.json
├── .mcp.json                 # プロジェクト共有 MCP サーバー（Playwright MCP）
├── CONTRIBUTING.md           # 貢献ガイド（人向けの入口）
├── docs/glossary.md          # 用語集（章・Step・IR・語彙など）
├── docs/architecture.md      # 実装 repo の構成設計（crate の境界・依存の向き・型の置き場。版つきの契約）
├── docs/vocabulary/          # 語彙の文書（vocabulary.md が固定の契約、README.md が作成者向けの索引）
├── docs/upstream-bump-procedure.md # 参照元の版を上げる手順書（洗い出し・教材テスト・公開前の人の確認と記録・生成物を無検証で公開しない。版 1。TASK-182 2/2）
├── docs/extension-point-procedure.md # 拡張点の追加の手続き（固定→実装→公開・診断コード・IR の同一の確認。版 1。TASK-189）
├── docs/engine-api.md        # 公開 API の契約の文書（core の型の一覧・世代の表・宛先の形・机上の照合の記録）
├── docs/cli-guide.md         # 作成者向けのコマンドの使い方と出力の読み方（版 1。TASK-412）
├── docs/examples/            # 最小の教材パックの例 2 つ（validate 0 件・test 全件合格。MANIFEST.sha256 つき。TASK-412）
├── docs/diagnostics.md       # 診断コード（E-*）の一覧（版 19。TASK-195・TASK-58・TASK-59・TASK-400・TASK-62・TASK-64・TASK-70・TASK-71・TASK-420）
├── docs/cli.md               # CLI の外部仕様（fandhe-pack のコマンド・引数・使い方・終了コード・診断 1 行・最終行・出し先・`compile`・`metrics`・`vocab` の出力・`test` の PASS／FAIL の行と集計。版 14。TASK-399・TASK-407・TASK-402・TASK-408・TASK-409・TASK-67・TASK-403・TASK-404・TASK-405）
├── docs/history-format.md    # 履歴の形式の文書（版 1。包みの 7 欄・4 軸の版・追記だけで増える列）
├── docs/pack-authoring.md    # 教材の作成者向けの文書（版と履歴・主版を上げる条件・pack.vocab の規約）
├── docs/publish-checks.md    # 公開前の確認の記録（LIC-12。確かめた版・日付・確かめた人。TASK-139）
├── docs/known-gaps.md        # 既知の不足（忠実さの限界）の記録（H-3・H-6〜H-8・AUTHOR-13 など）
├── docs/schema-format-b.md   # 構造定義（形式 B）の欄・型・必須と YAML の注意書き（版 1。TASK-411）
├── docs/operation-profile.md # 操作プロファイルと外観で選べるもの・外観 4 値の定義（版 1。TASK-411）
├── docs/views.md             # ビュー 7 種・左エリアの絞り込み・「指す」入力の宛先（版 1。TASK-411）
├── docs/spec/                # 仕様（private submodule。update = none）
└── _/                        # スキルが使うローカル状態（git 管理外。_/local-plans/ 等）
```

## コマンド

| コマンド | 内容 |
|---|---|
| `make help` | ターゲット一覧 |
| `make doctor` | 開発環境の診断（読み取りのみ） |
| `make setup` | Git hooks の有効化と `.env` の作成（再実行可） |
| `make check` | editorconfig-checker + shellcheck + testdata の参照検査 + 変異の定義の静的検査（ソースは変更しない） |
| `make test-hooks` | `scripts/hooks/` の回帰テスト |
| `make lint` | Rust の fmt check + clippy + web の wasm32 clippy（ソースは変更しない） |
| `make fmt` | Rust の整形（ソースを変更する） |
| `make test` | Rust の native 試験（web を含む全 crate） |
| `make check-deps` | crate 間の依存の向きの検査（`cargo metadata` + jq） |
| `make test-check-deps` | 依存の向きの検査自体の回帰テスト |
| `make check-testdata` | repo の外への相対参照が 0 件であることの検査 |
| `make test-check-testdata` | 参照の検査自体の回帰テスト |
| `make check-mutation-defs` | 変異の定義の `find` の一意性と `expect-fail` の名前の実在の静的検査（`make check` に含む。cargo 不要） |
| `make test-check-mutation-defs` | 上の静的検査自体の回帰テスト |
| `make check-dom-reads` | web crate が DOM の値を読まないことの検査（`tools/check-deps.sh`。例外は件数固定。crate 間の依存は `make check-deps`） |
| `make test-check-dom-reads` | DOM 読み取りの検査自体の回帰テスト |
| `make check-frontend-bump` | 版上げ PR の受け入れ確認 (4)(5) の関門（feature の明示と、版上げ PR で変えてはいけないファイル。(2) の件数の照合は CI の rust-test・browser-test が、(3) の WASM の gzip +10% の判定は browser-test の wasm-gzip が行う） |
| `make test-check-frontend-bump` | 上の関門自体の回帰テスト（模擬の版上げ） |
| `make check-wasm-size` | 配信する設定の WASM の `gzip -9` が 1 MB（1,048,576 bytes）以下であることの関門（TASK-78 2/2。CI の `browser-test` が `SKIP_WASM_OPT=1` で流す。基点つきの値をログに残す。DLV-4 の `wasm-opt` 適用後の判定は `scripts/check-wasm-size.sh --require-wasm-opt` を手元で直接実行。TASK-107 2/2。遅い） |
| `make test-check-wasm-size` | 上の関門自体の回帰テスト（ビルドなし） |
| `make update-test-counts` | 試験の件数の期待値（`scripts/expected-test-counts.txt`）を実測で書き直す（遅い。ブラウザ試験も流す） |
| `make build-l2` | L2 の配布物（zip）を作る（`tools/build-l2.mjs`。`L2_SERVER` に miniserve が要る。固定値は `tools/l2/miniserve.sha256`） |
| `make test-build-l2` | 上の作成スクリプトの回帰テスト（合成の配信物。ビルドなし。CI の `browser-test` が流す） |
| `make test-l2-run` | L2 の配布物を Rust のない模擬環境（`PATH=/usr/bin:/bin`）で展開・起動し、ブラウザで CH-0〜CH-1 を 3 回完走し、待ち受けが 127.0.0.1 だけで他のアドレスから拒否されること・起動とマウントの時間の記録・L1（サブパス 2 名）と L2 の 3 配信先 × 3 回の 9 回で操作ごとの観察・`snapshot.json`・`history.json` がバイト単位で一致することの試験（`l1-l2-match`。DLV-15）と、凍結した IR と組の固定の入力列での比較・履歴の `ops` に揺れる欄が無いことの試験（`l1-l2-fixed`。TASK-114 2/2）と、同じ browser context で L1・L2 の 2 か所・別名 `localhost` の 3 サーバを同時に起動したオリジンの分離（保存が互いに見えず L1 の状態が残る）の試験（`l1-l2-origins`。DLV-16）も行う（`L2_SERVER` 必須。遅く、`make test-browser` には入れない。CI の `browser-test` が miniserve を固定値で照合してから流す。TASK-112 2/2・TASK-113・TASK-114 1/2・2/2・TASK-115。`docs/testing.md` 5.239・5.241b・5.242・5.245b・5.357） |
| `make serve` | `dist/web` をサブパスの下で gzip つきで配る（`tools/serve.mjs`） |
| `make test-serve` | 配信サーバの試験（`node:test`） |
| `make test-browser` | 前段で `fandhe-pack` の debug ビルドを行い `FANDHE_PACK_BIN` で渡す（TASK-91）。ブラウザ試験の共通ヘルパ（`tools/browser/`）・最小の画面・サブパス配信の構成要素と相対パスの参照の検査（delivery-subpath。DLV-1・DLV-2。TASK-106 1/3。取得資源が構成要素だけであること）・章への直接リンク 10 件のサブパス配信の試験（delivery-direct-links。DLV-1。TASK-106 2/3。query の 1 形で取得資源が構成要素だけ・警告 0 件）・名前の違うサブパス 2 か所とルート配信の検査（delivery-bases。DLV-2。TASK-106 3/3。同じ成果物で同じ判定が通りルート絶対・サブパス名固定の参照を対照で検出）・`.wasm` が `application/wasm`・gzip で届き MIME の警告 0 件・`instantiateStreaming` の成功とマウントを、誤った MIME の対照つきで確かめる試験（wasm-mime。DLV-3。TASK-107 1/2。`docs/testing.md` 5.354）・幅 1280px と 960px で成果物と操作の 2 領域が横に並び tablist が隠れ 959px では隠れない試験（shell-columns。SHELL-1。TASK-517 1/2）・画面の枠の上部に「模擬」が全 Step・全幅・通知中・再読み込みの後も 1 個見えアクセシビリティツリーにも出る試験（shell-mock。SHELL-6・D-72。TASK-523。`docs/testing.md` 5.241）・`body: shown` の文書ビューの本文の限界（`body` 省略・`hidden`・`fixture` なし・文書以外のビューで区画 0 件、`.txt` は `pre > code`、`replace` の前後の印 0 件）を負の対照つきで固定する試験（doc-body-limits。ART-18。TASK-489 2/2。`docs/testing.md` 5.300）・app-demo-login（模擬ログインのデータで、アプリのデモ (a)・(b) のパスワード欄と文言が 0 件。名前の行を指すだけで進む。データと DOM の負の対照つき。ART-24。TASK-492。`docs/testing.md` 5.301）・view-scale（再描画の速さの測定が使う規模のデータの写しが generate.mjs の出力と一致し、12 組の前・後の項目数が期待値と一致し、測定のページ内の関数が 24 組で通る。測定は `tools/measure/view-redraw.sh`（束に入れない）。ART-4。TASK-485 1/3。`docs/testing.md` 5.316）・product-redraw（本体の画面で `?fixture=artifact-redraw-<n>` の固定データが件数どおりの指せる行を出し、再描画の測定のページ内の関数が 7 組で通る。測定は `tools/measure/product-redraw.sh`（束に入れない）。ART-26。TASK-485 2/3。`docs/testing.md` 5.318）・上部に教材名と「章の ID　Step M / K」が出て Step を進めると M が 1 増え再読み込み・書き出し→読み込みの後も復元した位置が出る試験（shell-position。SHELL-8・D-72。TASK-524。`docs/testing.md` 5.242）・上部の保存・復帰のボタンの click だけで、保存 → 先へ進む → 復帰の後に左の成果物と右の位置が保存の時点に戻り、書き換えた payload が状態を変えない試験（checkpoint-restore。SHELL-13。TASK-529 1/2。`docs/testing.md` 5.266）・上部のチェックポイントの一覧の各項目に保存の時点の章と Step の位置が出て、再読み込みの前後と書き出し→別のオリジンでの読み込みの前後で一覧が一致し、読み込んだ側で復帰できる試験（checkpoint-list。SHELL-13。TASK-529 2/2。`docs/testing.md` 5.267）・Step の題・do・本文が構造を保って出て、20 Step の DOM が IR と一致し親の要素が残る試験（step-body。SHELL-7。TASK-525 1/2。`docs/testing.md` 5.265）・`pack.left_pane` の 6 通り（`world_list` 2 値 × `view_tabs` 3 値）でボタン・選択の親の `hidden`・一覧の項目数が設定どおりになり、宣言外の選択が `declared`・`hidden` で無視され、Step の移動で先頭のビューに戻る試験（left-pane。PACK-22。TASK-481 1/2。`docs/testing.md` 5.302）・4 つの外観で変化の印の記号と文字が残り見えていることを、`data-theme` つきの要素に置いた代役の部品と製品の画面で確かめ、印を隠す規則を足す負の対照を含む試験（theme-marks。PACK-21。TASK-481 2/2。`docs/testing.md` 5.303）・doc-body（文書ビューで `body: shown` の教材の承認のたびに行の練習データの本文が出て同じ行のまま替わり、`body` を外した対照では出ない。CSP あり・なしで計数 0。ART-18。TASK-489 1/2。`docs/testing.md` 5.299）・形式 B の試験用教材を `compile` し、その出力を `IR_FILE` に渡した配信する設定の画面をサブパスの下で配り、DOM だけの観察で CH-0〜CH-3 の 20 Step を最後まで通して、各章の最初の Step に「いまやること」と本文が出て配信元以外への通信・ページ内エラーが 0 件の試験（author-preview。AUTHOR-21。TASK-427 1/2。`docs/testing.md` 5.292。章の開始点への直接リンクは同じ試験の E。TASK-427 2/2。5.298）・幅 959px と 360px で tablist の切り替え（クリック・矢印・Home・End・Esc がちょうど 1 回）と境目を確かめる試験（shell-tabs。SHELL-1・A11Y-5。TASK-517 2/2）・959px・960px の冷えた起動と両方向の境目・tablist と tab の名前と AX ツリーの形・Tab で辿り着く／960px で届かない・中身のある画面でのキー切り替えとキーボードだけの CH-1・検出器の対照の試験（shell-boundary。A11Y-5。TASK-369。`docs/testing.md` 5.304）・1280px・960px・959px・360px で CH-0 から CH-1 の 6 Step を通す試験（shell-widths。SHELL-1。TASK-517 2/2）・CH-0 から CH-3 の 20 Step を 1280px と 360px で最後まで通し、左右の取り付け点・Step の題と do・上部の位置・「次の章へ進む」での章の移動と、幅をまたぐ最終の世界状態の一致を確かめる通しの試験の骨格（shell-journey。SHELL-1・SHELL-7・SHELL-8・SHELL-10。TASK-548 1/2。`docs/testing.md` 5.269。途中にチェックポイントの保存→復帰・再読み込み・書き出し→別のオリジンでの読み込み・保存の失敗の表示を挟み、前後で同じ Step・世界状態になることも確かめる。TASK-548 2/2。5.270）・keydown の属性の試験・Esc が 10 場面（処理中か否か × フォーカスなし・root の外のボタン・入力欄・root の中のボタン・main の外の部品）でちょうど 1 回届く esc-once の試験（FEND-6・FEND-8。TASK-353）・Enter の送信・Shift+Enter の改行・複数行の送信の試験・貼り付け（改行の保持・自動送信 0 件・合成の paste）の試験・IME の模擬（CDP）と入力欄の層 3 の試験・保存済みの履歴が無い場合と未操作の再読み込みで新しく始まる試験（SAVE-3・SAVE-28）・保存・復元・書き出し・版の層 3 の試験・枠の書き出し（ダウンロードのリンク）と読み込み（ファイル入力）の操作を画面の操作だけで通す試験（history-ops。SHELL-15。TASK-531 1/2。`docs/testing.md` 5.234）・受講の途中の履歴を別のオリジンで読み込むと同じ状態・画面になり、続きの操作の結果と書き出しのバイト列が一致する試験（history-continue。SHELL-15。TASK-531 2/2。`docs/testing.md` 5.244）・版の違う履歴 15 件の判定と写しの一致の試験（VER-2）・表示だけで開いた履歴の理由が出て操作が状態・ログを変えない試験（view-only。VER-8。TASK-176 1/2）・表示だけで開いた履歴に 4 種の操作を送っても保存先の値が変わらず書き出しが読み込んだ履歴とバイト単位で同一の試験（view-only-storage。SAVE-25。TASK-335。`docs/testing.md` 5.231）・開かない履歴の理由が枠の通知欄に出て状態・保存先が変わらず起動時の復元では閉じられない通知が出る試験（refused-history。VER-8。TASK-176 2/2）・変換して開いた履歴で枠の通知欄に通知が 1 件出て閉じると消え、変換なし・表示だけでは出ない試験（version-notice。SHELL-20。TASK-535 1/2。変換して開いた後の操作の保存が開いた側の新しい版で書かれる。TASK-535 2/2）・章 A を進めて保存した後に章 B の直接リンクを開いても A の履歴が消えず通知が出ず、A に戻ると続きが出る試験（direct-link-roundtrip。DLV-8。TASK-110 2/2。`docs/testing.md` 5.206）・`?ir=` の値で読み込み先・要求が変わらず配信元の別の JSON を取得せず、`start` は `starts` と完全一致で断る試験（ir-location。SAFE-33・D-68。TASK-168。`docs/testing.md` 5.223）・Chrome 以外・旧版の Chrome で案内が 1 件出て閉じられ再読み込みでも再表示されない試験（browser-notice。SHELL-30・D-74。TASK-544。`docs/testing.md` 5.246）・入力方式ごとの入力の口の切り替えの試験（TERM-23）・入力欄なしの教材の表示の試験（PACK-20）・CH-0〜CH-3 の形の 20 Step の通しと CH-1 のキーボードだけの完走の試験（TERM-23）・`st-1-4`・`st-2-7` を含む 20 Step の全場面で選択肢のボタンに ID の文字が出る件数が 0 で、`label` を外した対照（`?fixture=ch0-3-nolabel`）では検出する試験（choice-labels。TERM-25。TASK-446。`docs/testing.md` 5.271）・読むだけ・確認だけの Step が 1 択＋確認の回答で操作 2 回（余計な操作 1 回）になることを CSP あり・なし×マウス・キーボードで固定し、誤答を挟む対照で数え方を確かめる試験（read-only-step。TERM-30・PACK-35・D-69。TASK-449。`docs/testing.md` 5.348）・「次の章へ進む」だけで CH-0 から CH-3 まで章をまたいで通す試験（next-chapter。SHELL-10・INTRO-3。TASK-526 2/2。`docs/testing.md` 5.227）・1 周目の終わりの履歴を読み込み欄から開き、「次の章へ進む」の実クリックで 2 周目を新しい根で始め、書き出しで世界状態の一致・閉じた根の保持・`ops` の追記を確かめる試験（round-handoff。MOD-19・MOD-5・MOD-8。TASK-254 2/2。入力は native の `emit_round_handoff_for_browser` が書き出す。`docs/testing.md` 5.343）・CH-3 を最後に使う絞った IR（範囲外の章のノードは残し開始点だけ除く）で CH-3 の完了後に範囲外の章へ入る操作が 0 件で、範囲外の `?start=` は断られ、開始点を足した対照では出る試験（out-of-range-chapter。SHELL-12・D-67。TASK-527。`docs/testing.md` 5.228）・章の飛ばし・選択の部品が全時点で 0 件で章をまたぐ操作は「次の章へ進む」の 0 か 1 個だけの試験（chapter-select-absent。SHELL-11・D-73。TASK-528。`docs/testing.md` 5.240）・未信頼の値（HTML・スクリプト・`javascript:` の形）の表示の試験の骨格と負の対照（view-escape。文書を除く 6 ビュー × 2 変種 × 前後の 24 通りが CSP あり・なしで違反 0・計数 0、不活性の文書の `innerHTML` で危険な要素がちょうど 3 件、表示した文字の `innerHTML` への入れ直しが検出される。ART-6。TASK-482 1/2・2/2。`docs/testing.md` 5.288・5.289）・ログが 1,000 項目を超える入力で 1 リストが 1,000 項目以下に畳まれ親が同じ要素のままである試験（list-fold。FEND-22。TASK-360 1/2）・artifact-limit（左の成果物の 4,095・4,096・4,097 項目と、境目をまたぐ入力と、古い側の畳みで、状態の行と root の非置換を確かめる試験。ART-14・FEND-22。TASK-484 2/2。`docs/testing.md` 5.295）・log-limit（右のログの元の行が 4,097 件を超える入力でも 1,000 項目に畳まれ、`data-list-error`・`data-list-limit` が 0 件で全再描画が起きない試験。TERM-35・FEND-22。TASK-454 2/2。`docs/testing.md` 5.350）・巡回する IR と長い連鎖の遷移 1,000 回の上限の試験（transition-limit。SAFE-23・SAFE-34・SAFE-35。TASK-160 1/3・2/3・3/3。止めた後は操作の前の Step に戻る）・root の全置換が 0 回で参照を保持した 18 要素の同一性と入力欄のフォーカス・入力途中の値が保たれる試験と、全再描画（`rerender`）を起こすと保持した 6 要素が 0 件しか残らずフォーカスが失われる負の対照（element-identity。FEND-3。B-7・B-7n。TASK-348 2/3・3/3）・秘密情報らしい 10 形の入力で保存の直前に形の名前だけの警告が出て保存先へ書かれず、対照 6 件では出ない試験（secret-warning。SAFE-27・SAFE-29。TASK-164 1/3。`docs/testing.md` 5.181）・警告の間は IndexedDB を含む保存先が変わらず履歴の書き出しの関門が止まり、警告のまま閉じて開き直しても保存先に書かれない試験（secret-hold。SAFE-27・SAFE-28・SAFE-44。TASK-164 2/3。`docs/testing.md` 5.182）・警告の「このまま保存する」で保存されて書き出しが通り、「この入力を残さない」で保存先・入力欄・画面に値が残らない試験（secret-choice。SAFE-28。TASK-164 3/3。`docs/testing.md` 5.183）・CSP の meta の配置と内容の検査（csp-placement。SAFE-13。TASK-151 1/2）・インラインを使わない画面の検査（inline-free。配信・試験の両設定の index.html・app.css に script・style 属性・on* 属性・data: と外部の取得が無く、実物の加工の対照で不合格になる。SAFE-15。TASK-151 2/2）・画面を開いて操作したときの CSP あり・なしの計数（csp-runs。最小の画面・最小教材と CH-1 の誘導・試験用の教材の全 Step の移動・表示部品 14 通り・CH-4〜CH-9 の形の 17 Step の通し（試験の設定・配信する設定。章ごとの外部通信 0。TASK-674 2/2。`docs/testing.md` 5.285）・CH-0〜CH-3 の形の 20 Step の通し（同。TASK-152。5.321）で通信・違反・コンソールの CSP 警告・エラーが 0 件、要求の種類が同じ。SAFE-13。TASK-151 2/2。`docs/testing.md` 5.180）・対照の自己確認（SAFE-8。CSP あり・なし）・安全性の観察の記録（safety-observe。BUILD-4。観察値は assert せず diagnostic に残す）・変更から履歴が保存先へ書き込まれるまでの遅延の測定（save-latency。localStorage・IndexedDB × 小さい・大きい履歴の 4 条件で p95 が 1 秒以内。SAVE-7。TASK-318。`docs/testing.md` 5.169。測定は `make measure` の束には入れていない）・10MB に近い履歴で秘密情報の検査を含む保存が変更から 1 秒以内に収まることの測定（secret-scan-time。IndexedDB・plain と dense の 2 条件と、中ほどに検出に掛かる値を置く対照。SAFE-31。TASK-166。`docs/testing.md` 5.186。測定は `make measure` の束には入れていない）・同じ教材の別の章を順に開いても履歴が混ざらない試験（chapter-keys。SAVE-14・DLV-8。TASK-326 1/2）・版・内容を変えた教材でも同じ鍵で履歴が開け読めない IR で保存先が変わらない試験（key-stability。SAVE-14・DLV-8。TASK-326 2/2。`docs/testing.md` 5.173）・開始点を指す直接リンク `?start=` の試験（direct-link-start。試験の設定で開始点 3 件の教材を指定なし・各開始点・一致しない値 10 形で開く。一致でその節点で開き開始点ごとに履歴の鍵が分かれ、不一致は値を出さず保存先を変えず断る。DLV-9・SAFE-33。TASK-109 1/2。`docs/testing.md` 5.199）・開始点を増やしても配信物の IR が 1 つのまま写しで増えず、同じ IR のどの開始点から開いても履歴の内容のハッシュが同じ試験（start-count-invariance。旧方式の写しと開始点ごとの別 IR の対照つき。DLV-9。TASK-109 2/2。`docs/testing.md` 5.201）・直接リンク 10 件が新しい文脈でその章の開始点で開き再読み込みの後・1 操作して受付済みにした後の再読み込みでも同じ状態になる試験（direct-link-reload。開始点 10 件の教材 `?fixture=ch0-3-links`。DLV-7。TASK-110 1/2。`docs/testing.md` 5.204）・保存領域が失われたときの案内の試験（保存領域を消した・保存できない状態でも新しく始まり、最初の操作と章の終わりで書き出しの案内が出る。SAVE-29。TASK-340）・上限に近い下書きの途中へ超過の貼り付けをしても貼った分だけが切られ後ろの既存の文字とキャレットの位置が保たれる試験（JUDGE-24・D-60。TASK-283。`docs/testing.md` 5.189）・問い返しの後に入力欄が空になり、全選択せずに続けて打った入力が順序違いの拒否になって未知のコマンドにならない試験（clarify-draft。JUDGE-27・TERM-22。TASK-284 1/2。`docs/testing.md` 5.198）・拒否の後に下書きの全体が選択状態になり、全選択せずに打った文字で置き換わり連結されない試験（reject-draft-select。JUDGE-27・TERM-22。TASK-284 2/2。`docs/testing.md` 5.200）・問い返し・拒否の後の下書きが連結されないことを画面の行と欄の値だけで確かめ、AI 代理の道具と同じ操作の経路と連結を見分ける対照を含む試験（draft-no-concat-screen。JUDGE-27・TERM-22。TASK-444。`docs/testing.md` 5.272）・旧いエンジンが新しい教材を理由つきで断り保存済みの履歴が書き換わらない試験（VER-13。TASK-181 2/2。`docs/testing.md` 5.140。Playwright。`npm ci` と `npx playwright install chromium` が前提。`FANDHE_BROWSER_CHANNEL=chrome` で Chrome の原本（記録は `docs/testing.md` 5.128）。CI は `browser-test` ジョブが Chromium で流す）・`prefers-reduced-motion: reduce` の文脈で処理中の Step の段階の行・progress 要素が 0 件で、進行（ticks）・結果の行・状態の語は通常どおりの試験（reduced-motion。A11Y-11・EVT-19。TASK-372 1/2。`docs/testing.md` 5.305。`no-preference` で処理中に中間の段階が見え、両方の文脈でアニメーション・`transition`・`@keyframes`・文字の逐次表示が 0 件で負の対照つき。TASK-372 2/2。5.306）・L2 の完走試験が使う DOM だけの観察の較正（l2-journey-calibrate。試験の設定の `stateJson` と各 Step で一致。TASK-112 2/2。`docs/testing.md` 5.239）・`reduce` の ST-1.5 で段階の行が 0 件・停止の意味が `no-preference` と同じ・設定は読み込み時に 1 回だけ読み開いたままの切り替えに追随せず開き直すと反映される試験（reduced-motion-st15。A11Y-12・A11Y-13・A11Y-29・D-82。TASK-378 1/2。`docs/testing.md` 5.307）・`reduce` でも Step の題・do・本文（補いの文面）と、チェックポイントの復帰・差し戻し→再承認の結果が `no-preference` と同じになる試験（reduced-motion-lo3。教材内に動きの設定が無いことは native の `a11y12_`。A11Y-12・A11Y-13・INTRO-20。TASK-378 2/2。`docs/testing.md` 5.310） |
| `make test-build-pack-bin` | `fandhe-pack` の前提バイナリの解決が `CARGO_TARGET_DIR` に追従することの回帰テスト（遅く、CI・フックには入れない） |
| `make test-chapter-key-control` | 鍵を修正前の固定の文字列にした写しで、章の試験（`chapter-keys`）が「先の章の履歴が出た」理由で失敗することの対照（遅く、CI・フックには入れない） |
| `make test-keydown-control` | `action-keydown` を外した写しで keydown の試験が正しい理由で失敗することの対照（遅く、CI・フックには入れない） |
| `make check-toolchain` | 固定ファイルの版・README の「前提」の表と `docs/toolchain.md` の一致の検査 |
| `make test-check-toolchain` | ツールチェーンの一致の検査自体の回帰テスト |
| `make check-workflow-secrets` | `ci.yml` が secret を参照していないことの検査 |
| `make test-check-workflow-secrets` | secret 参照の検査自体の回帰テスト |
| `make check-fork-ci` | fork が secret 無しで CI を通せる構成かの検査（A〜C。D は教材 repo 用で `--pack-workflow` と `--engine-rev` の指定時のみ。パーサで復号して判定。ruby・jq・cargo が必要） |
| `make test-check-fork-ci` | fork CI 検査自体の回帰テスト |
| `make build-web` | web を wasm にビルドして `dist/web` に配る一式を作る（`TEST_HOOKS=1` で試験の設定） |
| `make test-build-web` | `build-web` の受入基準（配信物に試験用の口が無い・compile した IR が配信する設定の `ir/main.json` に入る等）を実ビルドで検査する。CI の `browser-test` が流す |
| `make check-delivery` | 配信物（`dist/web`。先に `make build-web`）に試験用の口が無いことの検査（`tools/check-delivery.sh`。CI には未組み込み） |
| `make test-check-delivery` | 配信検査自体の回帰テスト（実ビルドを行うため遅い） |
| `make check-mutations` | 変異注入の検査（誤りを入れると対応する試験が失敗することの確認。遅く、CI・フックには入れない） |
| `make test-check-mutations` | 変異注入の検査自体の回帰テスト |
| `make measure-wasm-size` | WASM の `gzip -9` の大きさを基点つきで測り JSON を出す（様式は `docs/measurement.md`） |
| `make measure-history-size` | 2 周分の履歴（書き出しの包み）の大きさと `Changed` の割合を基点つきで測り JSON を出す（様式は `docs/measurement.md` §4a。`make measure` の束には入れない。TASK-216） |
| `make measure-log-items` | 2 周分の通しでの右のログの項目数（最大）を基点つきで測り JSON を出す（様式は `docs/measurement.md` §4b。`make measure` の束には入れない。TASK-454 1/2） |
| `make test-measure-wasm-size` | 測定の様式の検証と実ビルドの回帰テスト（遅く、CI・フックには入れない） |
| `make check-generic-words` | エンジンの crate のコードに教材固有の語（語彙の付録 A）が無いことの検査（`tools/check-generic-words.mjs`。CI 未組み込み） |
| `make test-check-generic-words` | 教材固有の語の検査自体の回帰テスト |
| `make check-internal-info WORDS=<一覧のパス>` | 社内固有の語（private の spec repo の一覧。`tools/check-internal-info.mjs`）と認証情報の形の公開前の照合（CI 未組み込み。語の一覧は本 repo に置かない。記録は `docs/publish-checks.md`） |
| `make check-internal-info-secrets` | 認証情報の形だけの照合（語の照合は行わない。CI 未組み込み） |
| `make test-check-internal-info` | 上の照合自体の回帰テスト |
| `make inventory-deps` | `Cargo.lock` の依存の取り下げ・所有者の数を crates.io で調べる（`tools/inventory-deps.mjs`。通信が要り数分かかる。CI 未組み込み。手順と記録は `docs/dependency-inventory.md`） |
| `make test-inventory-deps` | 依存の棚卸し自体の回帰テスト（通信なし） |
| `make test-upstream-impact` | 影響する Step の洗い出し道具（`tools/upstream-impact.mjs`。TASK-182）の回帰テスト（通信なし。CI 未組み込み） |
| `make measure` | 3 つの測定（WASM の gzip・初回表示・再描画）を基点つきで流し、束の JSON を出す（遅く、ブラウザを要する） |
| `make test-measure` | 一括測定の束の検証・fail-closed・実行の回帰テスト（遅く、ブラウザを要し、CI・フックには入れない） |
| `make clean-room` | まっさらなコンテナ（公開イメージ・認証情報なし・tar だけを渡す）で `cargo test --locked` と `--workspace` を流し、失敗 0 件・許容外の無視 0 件を判定する（`tools/clean-room.sh`。BUILD-1。TASK-77 1/2。遅く、docker と通信が要る。CI 未組み込み。記録は `docs/testing.md` 5.355） |
| `make test-clean-room` | 上の判定自体の回帰テスト（docker なし。CI の `check` ジョブが流す） |
| `make check-build-inventory` | `Cargo.lock` の private な依存 0 件とビルド・試験の経路の secret らしい語の参照 0 件の棚卸し検査（通信なし。CI 未組み込み。TASK-77 2/2。記録は `docs/testing.md` 5.358） |
| `make test-check-build-inventory` | 上の検査自体の回帰テスト（見本の repo。CI の `check` ジョブが流す） |
| `make test-measure-result` | 測定の結果の組み立て（`measure_result_json`）が jq 1.7 の文法でも通ることの回帰テスト（速く、ビルドなし。CI の `check` ジョブが流す。#1822） |

変更後は `make check` を通す。Rust を変えたら `make lint`・`make test`・`make check-deps` も通す。版の固定ファイルを変えたら `make check-toolchain` も通す。

## 委譲方針（必読）

main のコンテキストは、対話・計画・判断のために残す。調査と実装は subagent へ委譲し、結論だけを受け取る。
詳細は `.claude/rules/delegation.md`（調査・設計）と `.claude/rules/delegation-impl.md`（作成・編集）。

### main の役割

- ユーザーの意図の確認・計画の作成・承認の取得
- subagent への依頼と、結果の統合・検証
- コミット・PR 作成など、承認が要る操作の実行
- 1 ファイル・数行で済む修正は、main が直接行ってよい

### パスベース切り替え表

| パス | 調査 | 実装 | 適用ルール |
|---|---|---|---|
| `Makefile`・`scripts/*.sh` | `explorer` | `infra-builder` | `makefile.md`・`coding-shell.md` |
| `lefthook.yml`・`scripts/hooks/**` | `explorer` | `infra-builder` | `git-hooks.md`・`coding-shell.md` |
| `.github/workflows/**` | `explorer` | `infra-builder` | `github-actions.md` |
| `.claude/rules/**`・`.claude/agents/**`・`.claude/settings.json`・`.mcp.json` | `explorer` | `dotclaude-author` | `delegation.md`・`security.md` |
| `README.md`・`CONTRIBUTING.md`・`CLAUDE.md`・`docs/architecture.md`・`docs/glossary.md` | `explorer` | `docs-writer` | `japanese-style.md` |
| `Cargo.toml`・`**/*.rs` | `explorer` | 未定義（着手時に `update-claude` で builder を追加） | `coding-rust.md`・`rust-build-profile.md` |
| 外部仕様の確認 | `reference-researcher` | — | `.claude/skills/` のリファレンススキル |
| `.agents/skills/**`・`.claude/skills/**` | — | 編集しない（`contribute-skill` で upstream へ） | — |
| `docs/spec/**` | `explorer` | 編集しない（spec リポジトリ側で変更） | — |

実装後は `test-runner`（検証）→ `reviewer`（レビュー）→ `security-auditor`（PR 作成前）の順に委譲する。

### model 配分表

| 用途 | model tier | alias |
|---|---|---|
| 複雑な横断判断・アーキテクチャ設計 | 最上位 tier | `opus` |
| 調査・生成・実装・レビュー | 標準 tier | `sonnet` |
| 機械的な集計・検査の実行・ドキュメント更新 | 軽量 tier | `haiku` |

Agent の `model:` には、この表の alias だけを書く。最上位 tier は、大規模な設計や複数領域にまたがる判断に限って使う。

## Sub-agents

定義は `.claude/agents/<category>/<name>.md`。

| カテゴリ | subagent_type | model | 役割 |
|---|---|---|---|
| research | `explorer` | `sonnet` | コードベース横断の調査（読み取り専用） |
| research | `reference-researcher` | `sonnet` | 外部仕様・リファレンススキルの調査（読み取り専用） |
| implement | `infra-builder` | `sonnet` | Makefile・scripts・lefthook・GitHub Actions の実装 |
| implement | `dotclaude-author` | `sonnet` | `.claude/rules`・`.claude/agents`・`settings.json`・`.mcp.json` の作成・編集 |
| testing | `test-runner` | `haiku` | `make check`・`make test-hooks` の実行と結果の要約 |
| quality | `reviewer` | `sonnet` | 差分のレビュー（読み取り専用） |
| quality | `security-auditor` | `sonnet` | 秘密情報・インジェクション・CI の権限と供給網の監査（読み取り専用） |
| docs | `docs-writer` | `haiku` | `README.md`・`CLAUDE.md` の更新 |

エンジンの実装に対応する builder（`rust-builder` 等）と `e2e-runner` は未定義。実装着手時に追加する。

## Rules

定義は `.claude/rules/*.md`。「読み込み」が「常時」のものは毎セッション読み込まれ、それ以外は該当パスの編集時のみ
読み込まれる。

| ファイル | 読み込み | 内容 |
|---|---|---|
| `delegation.md` | 常時 | 調査・設計フェーズの委譲原則・パスベース切り替え・編集しないパス |
| `delegation-impl.md` | 常時 | 作成・編集フェーズの委譲マッピング・実装後の流れ |
| `security.md` | 常時 | 秘密情報の混入防止・インジェクション・供給網・OWASP Top 10 |
| `japanese-style.md` | 常時 | 日本語の文体・表記 |
| `conventional-commits.md` | 常時 | コミット件名・PR タイトルの形式と検証の仕組み |
| `out-of-scope-tracking.md` | 常時 | スコープ外の事項を Issue に記録する流れ |
| `coding-shell.md` | `**/*.sh`・`.shellcheckrc` | bash 3.2 互換・クォート・エラー処理・shellcheck |
| `coding-rust.md` | `**/*.rs`・`**/Cargo.toml` | workspace lint・unsafe と `#[allow]` の理由・エラー処理・依存の版固定と向き・検証コマンド |
| `code-comment-style.md` | `**/*.sh`・`Makefile`・`lefthook.yml`・`.github/workflows/**`・`**/*.rs`・`**/*.toml` | コメントに残す内容とファイル種別ごとの形式 |
| `makefile.md` | `Makefile`・`scripts/*.sh` | 薄い入口の方針・ターゲット追加手順・スクリプトの規約 |
| `git-hooks.md` | `lefthook.yml`・`lefthook-local.yml`・`scripts/hooks/**` | フックの時間予算・CI との役割分担・fail-closed |
| `github-actions.md` | `.github/workflows/**` | runner 方針・必須チェック・発火条件・キャッシュ方針 |
| `rust-build-profile.md` | `Cargo.toml`・`**/Cargo.toml`・`.cargo/**`・`rust-toolchain`・`rust-toolchain.toml` | Rust 導入時の dev プロファイル・`target/` の掃除・CI キャッシュへの影響 |

## Current Skills

`skills-lock.json` で管理する外部取得物（実体は `.agents/skills/`）。直接編集せず、更新は `update-external.yml`
の日次 PR に任せる。

### 作業スキル（`Fandhe-AI/agent-cli-skills`・16 件）

| 用途 | スキル |
|---|---|
| 計画 | `create-plan` |
| Issue | `create-issue`・`create-issue-tree`・`update-issue-tree` |
| 実装 | `implement-issue`・`implement-issue-tree` |
| レビュー | `implement-review`・`implement-review-pr` |
| コミット・PR | `create-commit`・`create-pr` |
| ドキュメント・コメント | `update-docs`・`comment-code` |
| `.claude/` 体系・リポジトリ設定 | `init-claude`・`update-claude`・`setup-repo-guards` |
| スキルの upstream への貢献 | `contribute-skill` |

### リファレンススキル（`Fandhe-AI/agent-reference-skills`・10 件）

`rust`・`fandhe-frontend`・`playwright`・`make`・`lefthook`・`github-docs`・`editorconfig`・`commitlint`・
`anthropic-claude-code`・`openai-codex`

## Conventions

- **言語**: ユーザーとのやりとり・ドキュメント・コメントは日本語（`japanese-style.md`）
- **Conventional Commits**: コミット件名と PR タイトルは `<type>[(<scope>)][!]: <要約>`（`conventional-commits.md`）
- **フックを回避しない**: `--no-verify`・`LEFTHOOK=0` は使わない。拒否されたら原因を直す
- **承認フロー**: `implement-issue` は計画（`_/local-plans/`）の承認後に実装する。コミット・push・PR・Issue の
  作成は、ユーザーが依頼した場合に行う
- **セキュリティ**: PR 作成前にセキュリティレビューを行い、問題があれば PR を作らない（`security.md`）。
  `.env` は読まない・出力しない
- **二重定義をしない**: 処理の実体は `scripts/` に置き、Makefile・lefthook・CI からはそれを呼ぶ
- **スコープ**: 依頼の範囲外で見つけた問題は直さず、Issue に記録する（`out-of-scope-tracking.md`）
- **検証**: 変更後は `make check` を通す。`scripts/hooks/` を変更したら `make test-hooks` も通す
- **ドキュメントの同期**: 構成・コマンド・CI・ルール・Agent を変えたら、`README.md` と本ファイルの表も更新する

## hooks（settings.json）

| hook | 内容 |
|---|---|
| SessionStart | 日本語・委譲・Conventional Commits・フック回避の禁止・計画承認・`.env` を読まないことのリマインダーを表示する |
| PostToolUse | 設定していない。整形ツール（shfmt 等）を採用しておらず、検査は `make check` と Git hooks が行う |

言語のフォーマッタ（rustfmt 等）を導入したら、PostToolUse の自動整形を検討する。個人用の設定は
`.claude/settings.local.json`（git 管理外）に置く。

## implement-issue-tree の前提

- `gh` CLI が認証済みであること（`gh auth status`）
- `.claude/workflows/implement-issue-tree.js` は、スキル側の実体
  （`../skills/implement-issue-tree/scripts/implement-issue-tree.js`）への相対 symlink。コピーに置き換えない
  （スキルの更新が届かなくなる）
- 一時 worktree は `.claude/worktrees/`、ツリーの状態は `_/issue-trees/` に置かれる（どちらも git 管理外）
- 自動マージ（`autoMerge: true`）は、ruleset `main-protection` が PR HEAD の全チェックを必須にしていることを前提に
  成立する（`.claude/rules/github-actions.md` の「必須チェック」）。起動時は外部 App のチェックを宣言する

    ```json
    { "parent": 42, "repo": "Fandhe-AI/fandhe-workshop", "autoMerge": true,
      "externalChecks": [{ "app": "cursor", "context": "Cursor Bugbot" }] }
    ```

- CI のジョブを追加・改名・削除すると、必須チェックと HEAD のチェックがずれて自動マージが辞退する。ruleset を同じ
  タイミングで更新する

## main ブランチの保護

- `main` へ直接 push はできない（ruleset `main-protection`）。変更は必ずブランチを切り、PR を経由する
- マージは squash のみ。必須チェック 13 件の成功と、レビュースレッドの解決が条件（承認数は 0）
- ruleset・リポジトリ設定の内容は `.claude/rules/github-actions.md`、変更手順は `setup-repo-guards` スキル
