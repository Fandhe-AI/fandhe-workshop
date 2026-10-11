---
paths:
  - ".github/workflows/**"
description: >
  GitHub Actions ワークフローを編集する際の方針。public リポジトリの runner 方針、必須チェックと
  集約ジョブ、発火条件・concurrency・paths の扱い、ジョブ追加チェックリスト、所要時間の計測方法と
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
- `concurrency` は `group` に workflow と ref を含め、`cancel-in-progress` は PR のみ true にする
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
- [ ] ツール導入はバージョン固定 + SHA256 検証（`ci.yml` の `check` ジョブの方式）。キャッシュが有効な言語ツールは公式のキャッシュ機能を使う
- [ ] サードパーティ action は既存と同じピン止め方式（コミット SHA + バージョンコメント）。自組織の reusable workflow は `@latest`
- [ ] macOS など高コスト runner は必要な変更時のみ実行する（`changes` ジョブの matrix 参照）
- [ ] `timeout-minutes` を設定した
- [ ] `actionlint` を通した

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
