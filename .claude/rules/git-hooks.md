---
paths:
  - "lefthook.yml"
  - "lefthook-local.yml"
  - "scripts/hooks/**"
description: >
  lefthook.yml・scripts/hooks/ を編集する際の方針。フックは staged 限定で一瞬で終わる検査と
  コミット単位でしか意味がない検査に限り、テスト・ビルド・全体 lint や CI と重複する検査は
  CI（.github/workflows/ci.yml）へ置く。時間予算・判断手順・計測方法を規定する。
---

# Git hooks（lefthook）編集ルール

## 目的

フックはコミットのたびに実行されるため、遅いフックはそのまま開発速度の低下になる。
フックはローカルでの早期検知専用であり、CI の代替ではない。**開発速度を落とさないことを最優先**し、
重い検査・網羅的な検査は CI（`.github/workflows/ci.yml`）に任せる。

本リポジトリは今後エンジンの実装（言語固有の fmt / lint / test / build）を追加していく。
それらは本ルールに従い、フックではなく `ci.yml` のジョブとして追加する。

## 時間予算

| フック | 予算 | 判定方法 |
| --- | --- | --- |
| pre-commit | 全ジョブ合計 1 秒未満 | `lefthook run pre-commit --all-files` の `summary: (done in N seconds)` |
| commit-msg | 即時（外部コマンド・ネットワーク不可） | 実コミット時に表示される commit-msg の summary（メッセージファイルを引数に取るため `--all-files` では計測しない） |
| pre-push・post-checkout・post-merge 等 | 原則追加しない | 追加する場合は理由と実測値を PR 本文に記載する |

`--all-files` は全追跡ファイルを対象にする最悪ケースであり、実際のコミット（staged のみ）はこれより速い。

## フックに置かないもの（→ `ci.yml` へ）

- テストスイート（unit / integration / e2e）・ビルド・型チェック
- リポジトリ全体を走査する lint / fmt チェック（`make check` 全体の呼び出しを含む）
- 依存パッケージのインストール・ネットワークアクセス
- `ci.yml` のジョブと同じコマンドを同じ範囲で実行するもの
- フックスクリプト（`scripts/hooks/*.sh`）自体のテスト — これも CI のジョブとして実行する
  （`ci.yml` の `test-hooks` ジョブ。実体は `scripts/hooks/test-hooks.sh`、ローカルでは `make test-hooks`）

## フックに置いてよいもの

- `{staged_files}` に限定し、`glob` / `exclude` で対象を絞った 1 秒未満の検査
- CI ではコミット単位で検知できない、または push 後では遅いもの
  - secret-scan（`scripts/hooks/secret-scan.sh`）: 秘密情報は push 前に止める必要がある
  - commit-msg（`scripts/hooks/commit-msg-check.sh`）: CI の `pr-title` ジョブは PR タイトルしか検証しない

### 明示的な例外

- pre-commit の editorconfig-checker / shellcheck は CI の `check` ジョブ（`make check`）と重複するが、
  staged ファイル限定で、`--all-files`（最悪ケース）でも合計 0.2 秒程度のため維持する（2026-10-11 計測・決定）
- `secret-scan.sh` 冒頭の正規表現自己テストは ms 単位の fail-closed ガードであり、
  上記「フックスクリプト自体のテスト」には当たらない。削除・CI への移動はしない

## 書き方の規約

- `pre-commit` の `parallel: true` を維持する（直列化しない）
- `{staged_files}` と `glob` / `exclude` で対象を絞り、外部取得物の `.agents/skills/**`・`.claude/skills/**` を除外する
- 実体が長くなる場合は `scripts/hooks/` へ切り出し、`lefthook.yml` は呼び出しのみにする
- 条件付き実行が必要な場合は lefthook の `skip` / `only` を使ってよい
  （`.claude/skills/lefthook/references/examples/skip.md`）

## フックスクリプトの書き方

秘密情報検査のように「検出できなかった = 合格」になる検査は、検査そのものの失敗を合格と取り違えると
黙って素通りする（fail-open）。以下は過去に実際に見つかった不具合クラスであり、`scripts/hooks/` を
書く・変える際は必ず守る。各項目には `scripts/hooks/test-hooks.sh` に回帰ケースがある。

- (a) 検査エラーを「一致なし」と同一視しない（fail-closed）
  - `grep` の終了状態は 0 = 一致・1 = 不一致・2 以上 = エラー。`if grep -q ...` / `grep ... || true` は
    エラーを不一致として扱うため、終了状態を受けて 2 以上は NG（拒否）にする
  - bash の `[[ =~ ]]` も正規表現のコンパイル失敗で 2 を返す。`secret-scan.sh` は終了状態を区別し、
    冒頭の自己テストで「一致すべき例が一致する」ことも確認してから走査する
  - 検査を完了できない場合の終了コードは検出時（1）と分ける（`secret-scan.sh` は 2）
- (b) ASCII のパターンで照合する検査は `LC_ALL=C` で実行する
  - UTF-8 ロケールのまま非 UTF-8 のバイト列（Latin-1 等）を含む行を照合すると、macOS の awk / grep は
    異常終了・バイナリ扱いし、bash の `[[ =~ ]]` は（macOS・glibc とも）同じ行のトークンに一致しなくなる
  - bash はスクリプト内で `export LC_ALL=C` すると以降の照合に効く。`grep` は `-a` も付ける
- (c) `git diff` の `+++ b/<path>` は `diff --git` から最初の `@@` までの間でだけ解釈する
  - hunk 内の追加行の本文が `++ b/...`・`++...` で始まると diff 上は `+++...` になる。行頭だけで
    ヘッダと判定すると、その行が走査から漏れ、以降の行のファイル名も取り違える
  - ヘッダ区間以外の `+` 行はすべて走査する。空白を含むパスの末尾タブ・`+++ "b/..."` の引用符も扱う
- (d) `git diff` の出力形式を利用者の git 設定に依存させない
  - `--no-color --no-ext-diff` は必須（`color.diff=always` / `color.ui=always` で行頭に色コードが付き
    `+` 行を検出できない。`diff.external` は本文を差し替える）。内容検査では `--no-textconv` も付ける
  - リネーム・コピー検出で移動先の行が出ない・除外パスからの移動が素通りする場合は `--no-renames`
  - ファイル名を使う場合は `--src-prefix=a/ --dst-prefix=b/`（`diff.noprefix`・`diff.mnemonicPrefix` 対策）、
    パス一覧は `-z`（非 ASCII のパスが `core.quotePath` で引用符付き 8 進表記になり末尾一致しなくなる）
- (e) パイプの左辺・`$(...)` に置いた git の失敗を隠さない
  - `git diff ... | while ...` や `done <<< "$(git diff ...)"` は git が失敗しても `set -e` /
    `pipefail` で止まらず「対象なし」として合格し得る。`git ... > "$tmp/file" || exit 2` のように
    一時ファイルへ書き出して終了状態を確認してから読む
  - `cd "$(git rev-parse --show-toplevel)"` も失敗が `cd ""` に化ける。先に変数へ代入して確認する
- 検出時はファイル名（とパターン種別）のみを出力し、値そのものは出力しない
- macOS 標準の bash 3.2 で動く書き方にする（lefthook は PATH 上の `bash` で起動する。`mapfile`・
  連想配列・`${var,,}`・空配列の `"${arr[@]}"`（`set -u` で未定義扱い）は使わない）
- プロセス起動回数を diff 行数・ファイル数に比例させない（1 行ごとに `grep` 等を起動しない）
- 例示値（トークン等）はスクリプト・テスト内で実行時に組み立て、そのファイル自体のコミットが
  `secret-scan.sh` に検知されないようにする

## 検査を追加するときの判断手順

1. その検査は `ci.yml` で既に実施されているか
    - 実施済みで、staged 限定にしても 1 秒以上かかる → フックに入れない
2. staged ファイルに限定できるか
    - できない → CI へ置く
3. CI へ置く場合は `ci.yml` にジョブを追加し、集約ジョブ `ci-complete` の `needs` にも必ず追加する
    - ツールの導入・バージョン固定は既存の `check` ジョブの方式（バージョン + SHA256 検証）に合わせる
4. `scripts/hooks/` を変更した場合は `scripts/hooks/test-hooks.sh` に検出・許可（OK / NG）ケースを追加し、
    `make test-hooks` を通す（Linux の検証は CI の `test-hooks`、macOS の検証は `test-hooks-macos` ジョブが行う）
5. 編集後に以下を実行し、summary の合計時間をユーザーへ報告する（1 秒以上なら構成を見直す）

```bash
time lefthook run pre-commit --all-files
```

## 手元で CI 相当を確認する場合

フックで強制しないだけで、push 前に手元で確認する運用自体は変わらない。CI の `check` ジョブと同じ
検査は `make check`（直接実行: `scripts/check.sh`）、`test-hooks` ジョブと同じテストは `make test-hooks` で実行できる。言語固有のターゲットを追加した場合も、
`Makefile` / `scripts/` 経由で手元から呼べるようにし、フックには組み込まない。

## バイパス禁止

遅いからといって `--no-verify` や `LEFTHOOK=0` で回避しない。遅いフックはフック側を直す
（検査を CI へ移す・staged 限定にする・対象 glob を絞る）。
