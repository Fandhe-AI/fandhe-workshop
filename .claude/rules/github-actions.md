---
paths:
  - ".github/workflows/**"
description: >
  GitHub Actions ワークフローを編集する際の方針。public リポジトリの runner 方針、必須チェックと
  集約ジョブ、発火条件・concurrency・paths の扱い、ジョブ追加チェックリスト、キャッシュ方針（保存は main のみ・
  キー設計・容量）、所要時間の計測方法と
  2026-10-11 時点の棚卸し結果を規定する。
---

# GitHub Actions 編集ルール

ワークフロー数・ジョブ数・実行時間（課金分数）を増やさないことを優先する。フックとの役割分担は
`.claude/rules/git-hooks.md`、Makefile・scripts は `.claude/rules/makefile.md` を参照する。

## runner 方針

- 本リポジトリは **public**。runner は GitHub-hosted（`ubuntu-latest`）を使う（組織方針: Fandhe-AI/actions の `docs/runner-policy.md`）
- OS 要件による例外: `ci.yml` の `test-hooks` は macOS（BSD 系コマンド・bash 3.2 の挙動検証が目的）で `macos-latest` を使う
- 例外: `ai-review.yml` の `codex / review` は reusable workflow 側が使う self-hosted の codex 専用 runner。
  fork PR は reusable workflow 側が fail-closed で拒否する前提で受容している
- public リポジトリで self-hosted を新規に使わない。fork PR 経由で任意コードが実行されるリスクがある。
  やむを得ない場合は fork PR の拒否を実装し、理由をワークフローのコメントに残す

## 必須チェック

- 2026-10-11 時点で ruleset / branch protection は未設定（API 空）。導入時は集約ジョブ `ci-complete`（と ai-review の `codex / *`）を required にする（`setup-repo-guards` スキルの手順）
- ジョブ名（`name:`）を変えると required check の参照が切れて pending のままマージ不能になる。名前を変える場合は ruleset 側も同時に更新する
- 個別ジョブ（`check` 等）は required にせず、`ci-complete` に集約する

## 発火条件

- `ci.yml`: `pull_request` と `push`（`main` のみ）。push を全ブランチにしない（PR との二重実行になる）
- `concurrency` は `group` は PR では workflow + ref、それ以外は `run_id` で分け、`cancel-in-progress` は PR のみ true にする
  （`${{ github.event_name == 'pull_request' }}`。main / release の実行は cancel しない）
- 必須チェックを含む workflow に workflow 単位の `paths` / `paths-ignore` を付けない
  （未実行の必須チェックが pending のままマージ不能になる）。変更検知はジョブ内で行い、
  `ci-complete` で skipped を許容する（例: `changes` ジョブ + `scripts/ci-changes.sh`）
- draft PR をスキップする場合は `types` に `ready_for_review` を含める
- `schedule` は必要最小限の頻度にする（`update-external.yml` は日次 1 回）

## ジョブ追加チェックリスト

- [ ] 新しいワークフローを作らず、既存ワークフローへのジョブ追加を優先する
- [ ] `ci-complete` の `needs` に追加した（漏れると未検査のままマージ可能になる）。skipped を許容する場合は jq の条件も更新する
- [ ] 実処理は `make <target>` / `scripts/*.sh` を呼ぶ（CI 独自にロジックを書かない）
- [ ] lefthook と同じ検査を同じ範囲で重複させない（`git-hooks.md`）
- [ ] 不要な `needs` を作らない（独立ジョブは並列に走らせる）。逆にセットアップ時間が支配的な小ジョブは統合する
- [ ] `actions/checkout` は `persist-credentials: false`、深さは必要最小限（既定の 1。差分が要る場合のみ 2）
- [ ] ツール導入はバージョン固定 + SHA256 検証（`ci.yml` の `check` ジョブの方式）
- [ ] キャッシュを使う場合は「キャッシュ方針」に従う（効果を実測し、保存は main への push のみ。`setup-*` 系 action の
      一体型キャッシュは PR でも保存するため使わない。確認結果を PR 本文に書く）
- [ ] サードパーティ action は既存と同じピン止め方式（コミット SHA + バージョンコメント）。自組織の reusable workflow は `@latest`
- [ ] macOS など高コスト runner は必要な変更時のみ実行する（`changes` ジョブの matrix 参照）
- [ ] `timeout-minutes` を設定した
- [ ] `actionlint` を通した

## キャッシュ方針

現状（2026-10-11）: 現在 GitHub Actions のキャッシュは使っていない（キャッシュは 0 件・0 バイト）。`ci.yml` は
言語のツールチェーン・依存を導入しておらず、キャッシュする対象も無い。言語のツール・依存を追加してキャッシュを
使う場合は本節に従う。

### 前提（容量と課金）

- リポジトリあたりの上限は 10GB（Org 設定も 10GB）。超えると最終アクセスの古い順に追い出され、7 日間アクセスの無い
  キャッシュは削除される
- Org の Actions 予算は $0（超過時は停止）のため、10GB を超える上限は使えない。上げると 10GB を超えた時点で
  キャッシュが読み取り専用になる。上限・予算の変更はユーザー判断

### 入れる前に効果を測る

- 本リポは GitHub-hosted runner で毎回まっさらな VM から始まるため、キャッシュが効く余地はある。ただし現状の
  各ジョブの実処理は 5〜15 秒で、復元にかかる時間の方が長くなり得る。足す前後でジョブログの復元秒数・依存の
  ダウンロード件数・再ビルド数を比べ、効果を確かめてから入れる
- self-hosted runner ではツールチェーンや pnpm store・`CARGO_TARGET_DIR` 等がジョブをまたいで残り、キャッシュが
  無意味になることがある（他リポの実例: mirror-ui は pnpm キャッシュの復元に 1 ジョブ平均 75 秒かけていたが
  `downloaded 0` で効果ゼロだったため撤去。team-hub は `Swatinem/rust-cache` が `./target` を保存対象にしていたが
  runner の `CARGO_TARGET_DIR` が別の場所で registry しか保存されておらず、後処理が共有 `CARGO_HOME` の crate を
  消していたため撤去）。runner の種類を変えるときも測り直す

### 保存は main への push だけ

- PR から保存したキャッシュは `refs/pull/<N>/merge` スコープに入り、その PR からしか読めないのに上限を共有して
  main のキャッシュを追い出す
- `actions/cache` は `actions/cache/restore`（全イベント）と `actions/cache/save` に分け、save には
  `if: github.event_name == 'push' && github.ref == 'refs/heads/main' && steps.<id>.outputs.cache-hit != 'true'` を付け、
  `key: ${{ steps.<id>.outputs.cache-primary-key }}` で保存する（`<id>` は restore ステップの `id`）
- `actions/setup-node` の `cache: pnpm` など、`setup-*` 系 action の一体型キャッシュは PR でも保存する。使うなら
  保存を main に限れる restore / save の分離に置き換える
- Rust を追加する場合: `Swatinem/rust-cache` なら `save-if: ${{ github.ref == 'refs/heads/main' }}` を渡す。
  Fandhe-AI/actions の `rust-base-ci.yml` を `cache: true` で呼ぶなら `cache-save: ${{ github.ref == 'refs/heads/main' }}`
  も渡す（共通側の `cache-save` の既定は true で、PR からも保存する）

### キーの設計

- キーは中身のハッシュ（lockfile 等）にする。`github.sha`・`github.run_id` など毎回変わる値は入れない。入れる場合は
  `restore-keys` の前方一致で直近を拾う設計にし、保存は main に限る
- `hashFiles('**/pnpm-lock.yaml')` `hashFiles('**/Cargo.lock')` のような再帰 glob は使わず、ルートの lockfile
  （ビルド対象に本当に入るものだけ）を指定する。本リポにも `.agents/skills/make/samples/projects/` 配下にスキルの
  サンプル（`Cargo.lock` 3 件・`pnpm-lock.yaml` 2 件）があり、再帰 glob はこれを拾う。依存が変わらないのにスキル
  更新でキーが変わり全キャッシュを作り直す事故が fandhe-browser・fandhe-frontend・fandhe-db・fandhe-container・
  template-frontend-react_router で起きた
- ビルド成果物（Rust の `target/` 等）をキャッシュするなら、キーにツールチェーンの版（Rust なら `rustc -vV` の
  `release`・`commit-hash`）を入れる。入れないと toolchain 更新後も古い成果物に exact hit し続け、exact hit では
  再保存されないため依存を毎回全再ビルドする（fandhe-db で依存 135 crate を毎回再ビルドしていた）
- `restore-keys` で前方一致を許し、キーが変わっても cold にならないようにする

### サイズを抑える

- 1 世代の合計が 10GB に余裕をもって収まるようにする。キャッシュの種類を増やすときは 1 件のサイズ × 種類 × OS
  （本リポの `test-hooks` のような ubuntu / macOS の matrix）を見積もる
- ビルド成果物は保存前に自リポの成果物を除くなど必要な分だけに絞る（Rust なら workspace メンバーの成果物の prune、
  `CARGO_INCREMENTAL=0`、CI 限定の `CARGO_PROFILE_DEV_DEBUG=line-tables-only` 等）
- 上限に達しても、毎回読まれる main の最新キャッシュは最後まで残る（LRU）。キーの切り替えで古い世代が追い出されても
  新しい世代が保存済みならミスにならない。この状態を保てていれば上限を上げる必要はない

### 計測

```bash
gh api repos/Fandhe-AI/fandhe-workshop/actions/cache/usage                      # 件数・合計サイズ
gh cache list -R Fandhe-AI/fandhe-workshop -L 100                                # key・サイズ・最終アクセス
gh cache list -R Fandhe-AI/fandhe-workshop -L 100 --json id,key,ref,sizeInBytes  # ref（main か PR か）
```

- ジョブログの `Cache restored from key:`（primary key と一致なら exact、それ以外は partial）・`Cache not found`・
  `Cache saved with key:` で hit / miss と保存を確かめる
- 不要なキャッシュは `gh cache delete <id>` で消せるが、共有状態なので対象（key・ref）を確認してから消す

### PR のチェックリスト

ワークフローにキャッシュを追加・変更する PR では、次を確認したことを PR 本文に書く。

- [ ] 効果を実測した（復元秒数・依存のダウンロード件数・再ビルド数。効果が無ければ入れない・残さない）
- [ ] 保存は main への push に限っている
- [ ] キーはルートの lockfile のハッシュで、成果物を含むならツールチェーンの版を含み、`restore-keys` がある
- [ ] 1 世代の合計サイズを見積もり、10GB に余裕をもって収まる

## 計測方法

```bash
gh run list --workflow ci.yml --limit 30 --json databaseId,conclusion,createdAt,updatedAt,event
gh run view <run-id> --json jobs --jq '.jobs[]|[.name,.conclusion,.startedAt,.completedAt]|@tsv'
```

## 棚卸し結果（2026-10-11）

| ワークフロー | ジョブ | 実測（直近の実行） |
| --- | --- | --- |
| `ci.yml` | check・test-hooks（ubuntu / macOS）・pr-title・ci-complete | wall-clock 約 2〜3 分。各ジョブの実処理は 5〜15 秒で、大半は macOS ランナーの起動待ち（約 2 分） |
| `ai-review.yml` | codex の preflight・review・post_feedback | 約 1〜2 分 |
| `update-external.yml` | 日次の外部ソース追従（reusable） | 実績なし（未計測） |

施策: macOS の `test-hooks` はフック関連の変更時のみ実行（`changes` ジョブ）。main への push 実行は cancel しない。
