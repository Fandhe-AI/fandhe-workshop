# ツールチェーンの固定

```text
ツールチェーンの版: 5
決定日: 2026-10-27
状態: 固定
```

## 1. 位置づけ

- 本文書は、第三者や CI が同じ版でビルド・試験するために固定した道具の版と前提を、一か所にまとめる
- 固定した版の唯一の正は本文書の表（第 2 節）である。固定ファイルとの一致は `make check-toolchain`
  （`scripts/check-toolchain.sh`）が検査する
- 版を固定するだけで、CI への組み込みは TASK-33（`ci.yml` の `rust-lint`・`rust-test`。導入後の版は `scripts/ci-rust-toolchain.sh` が照合する）、ビルドスクリプトは TASK-27、ブラウザ試験は TASK-28、その CI への組み込み（`ci.yml` の `browser-test`。wasm-bindgen CLI は SHA256 検証つきで導入し、Node は `engines.node` から読む）は TASK-79 が扱う
- 値は PoC-11（`third-party-build`）で動作を確かめたものに限る

## 2. 固定した版

`scripts/check-toolchain.sh` は、次の表の「キー」と「値」の列を読む。表の形式を変えるときは、スクリプトと
`scripts/test-check-toolchain.sh` も同じ変更で直す。キーは 1 行ずつ、ちょうど 1 回だけ書く。

| キー | 値 | 固定するファイル | 確かめた記録 |
|---|---|---|---|
| `rust` | `1.98.1` | `rust-toolchain.toml` の `channel` | PoC-11（`rust:1.98` イメージ） |
| `rust-target` | `wasm32-unknown-unknown` | `rust-toolchain.toml` の `targets` | PoC-11 |
| `wasm-bindgen` | `0.2.128` | `Cargo.toml` の `[workspace.dependencies]`・`Cargo.lock` | PoC-11 の試作の `Cargo.lock`、`_setup` F-1 |
| `node` | `>=24 <25` | `package.json` の `engines.node`（`.npmrc` の `engine-strict`） | PoC-11（Node 24.20.0・npm 11.19.0） |
| `playwright` | `1.63.0` | `package.json`・`package-lock.json` | PoC-11（`playwright:v1.63.0-noble` イメージ） |

- `wasm-bindgen` CLI は、`Cargo.lock` の `wasm-bindgen` と同じ版にする。版が違うと生成が失敗するか、壊れた glue になる
- `Cargo.toml` では `=0.2.128` と exact で書く。`Cargo.lock` なしの解決は 0.2.129 になり、CLI と食い違うため
  （PoC-11 `_setup` F-1）
- README の「前提」の表は、この表の写しである。`check-toolchain.sh` が値の一致を検査する
- `Cargo.lock` と `package-lock.json` はコミットする。ビルドは `--locked`、Node の依存の導入は `npm ci` で行う

## 3. 前提

| 道具 | 扱い | 備考 |
|---|---|---|
| rustfmt・clippy | 必須 | `rust-toolchain.toml` の `components` で導入される |
| wasm-opt | 任意 | 配信物を小さくするときに使う |
| Chrome または Chromium | 固定しない | Playwright 1.63.0 が同梱する Chromium 153.0.8010.12 を使う。Linux arm64 には Chrome の安定版が無い（PoC-11 F-2）。Chrome の原本で流すときは `FANDHE_BROWSER_CHANNEL=chrome` を付ける。扱いは README の「Chrome の原本と Chromium の扱い」、記録は docs/testing.md 5.128 |
| jq | 必須。版は固定しない | 1.7 以上。macOS 同梱の 1.7.1 と 1.8 系で動く。1.7 の文法（オブジェクトの値の二項演算は括弧で囲む）の回帰は `make test-measure-result`（CI の `check` ジョブ）が見る |
| Python | 不要 | 試験用の配信は Node の `tools/serve.mjs`（TASK-28）が担う |

## 4. 導入手順

```text
rustup toolchain install                      # rust-toolchain.toml を読む（版・component・ターゲット）
cargo install wasm-bindgen-cli --version 0.2.128 --locked
npm ci                                        # package-lock.json の版を導入する
npx playwright install chromium
```

- 既に同じ版の toolchain がある場合は、`rustup target add wasm32-unknown-unknown --toolchain 1.98.1` で
  ターゲットを足す
- `make doctor` は、導入した版が本文書と一致するかを読み取りだけで確かめる

## 5. 版を上げる手順

1. 固定ファイル（`rust-toolchain.toml`・`Cargo.toml`・`Cargo.lock`・`package.json`・`package-lock.json`）、
    本文書の表、README の「前提」の表とセットアップの導入行、導入する CLI を、同じ PR で上げる
2. `make check-toolchain` を通す
3. wasm-bindgen を上げるときは、`web-sys`・`js-sys` の版を揃える。0.2.128 に対応するのは 0.3.105 で、
    0.3.106 は `wasm-bindgen =0.2.129` を要求する（TASK-26 への申し送り）

## 6. `package.json` をルートに置く決定

- `package.json` と `package-lock.json` はリポジトリのルートに置く。`npm ci` の入口を一つにするため
- TASK-28 の `tools/browser/` は、ルートの `node_modules` を解決できる。`docs/architecture.md` 第 12 節が
  委ねた置き場の決定は、これで答える

## 7. 変更履歴

| 版 | 日付 | 内容 | タスク |
|---|---|---|---|
| 1 | 2026-10-12 | 初版。Rust・wasm-bindgen・Node・Playwright の版を固定 | TASK-25 |
| 2 | 2026-10-14 | 試験用の配信を Node の `tools/serve.mjs` に確定（Python は不要）。固定する版は変えない | TASK-28 |
| 3 | 2026-10-18 | README の「前提」に値を写し、check-toolchain が一致を検査する。固定する版は変えない | TASK-90 |
| 4 | 2026-10-19 | Chrome の原本と Chromium で流す扱いを第 3 節に追記。固定する版は変えない | TASK-94 |
| 5 | 2026-10-27 | 第 3 節に jq の対応版を追記（測定の結果の組み立てが jq 1.7 で通ることの回帰検査を足した）。固定する版は変えない | #1822 |
