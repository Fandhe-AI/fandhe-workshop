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
- OS 要件による例外: `ci.yml` の `test-hooks-macos` は macOS（BSD 系コマンド・bash 3.2 の挙動検証が目的）で `macos-latest` を使う
- 例外: `ai-review.yml` の `codex / review` は reusable workflow 側が使う self-hosted の codex 専用 runner。
  fork PR は reusable workflow 側が fail-closed で拒否する前提で受容している
- public リポジトリで self-hosted を新規に使わない。fork PR 経由で任意コードが実行されるリスクがある。
  やむを得ない場合は fork PR の拒否を実装し、理由をワークフローのコメントに残す

## 必須チェック

- 2026-10-11 に ruleset `main-protection`（id 24879606）を適用した。対象は既定ブランチ、enforcement は active、
  bypass は無し。classic branch protection は使っていない。変更は `setup-repo-guards` スキルの手順に従う
- ruleset の内容: ブランチの削除と force push の禁止、PR 必須（承認数 0・レビュースレッドの解決必須・マージ方式は
  squash のみ）、必須チェック 13 件（下表）。`main` へ直接 push はできず、すべて PR を経由する
- 承認数を 0 にしているのは、人の承認者を置かない運用のため。レビューのゲートは ai-review の必須チェック
  （`codex / review` は P0・P1 の指摘で失敗する）と、レビュースレッドの解決必須で担保する

| 必須チェック | 発行元 App（`integration_id`） |
| --- | --- |
| `ci-complete` | github-actions（15368） |
| `changes (test-hooks macOS)` | github-actions（15368） |
| `make check (editorconfig-checker + shellcheck)` | github-actions（15368） |
| `hook script tests (ubuntu-latest)` | github-actions（15368） |
| `hook script tests (macos-latest)` | github-actions（15368） |
| `PR title (Conventional Commits)` | github-actions（15368） |
| `codex / preflight` | github-actions（15368） |
| `codex / review` | github-actions（15368） |
| `codex / post_feedback` | github-actions（15368） |
| `rust lint (fmt + clippy + check-deps)` | github-actions（15368） |
| `rust build + test (--workspace)` | github-actions（15368） |
| `browser test (Chromium)` | github-actions（15368） |
| `Cursor Bugbot` | cursor（1210556） |

- ジョブ名（`name:`）を変えると required check の参照が切れて pending のままマージ不能になる。名前を変える場合は ruleset 側も同時に更新する
- implement-issue-tree の自動マージ（`autoMerge: true`）を使うため、ruleset の必須チェックには PR HEAD に報告される
  全チェック（`ci.yml` の全ジョブ・ai-review の `codex / *`・Cursor Bugbot）を、発行元 App の `integration_id` を束縛して
  登録する。自動マージの事前検証（G0）は、必須に無いチェックが HEAD に 1 件でもある場合と、必須のチェックが HEAD に
  報告されない場合の両方で辞退するため。`ci-complete` は全ジョブの結果を集約する必須チェックとして残す
- 条件付きのジョブは、ワークフロー単位の `paths` や動的 matrix ではなく、ジョブ単位の `if:` で制御し、チェック自体は
  常に報告させる（不要時は skipped。例: `test-hooks-macos`）。matrix ジョブを `if:` でスキップするとチェック名の
  `${{ matrix.* }}` が展開されないため、静的な名前の別ジョブにする
- ジョブの追加・改名・削除は、`ci-complete` の `needs` と ruleset の必須チェックを同じタイミングで更新する。改名は
  マージ前に ruleset を新しい名前へ更新する（旧名のままだと「Expected」のままマージできない）
- `strict_required_status_checks_policy` は false にする（true だと 1 件マージするたびに他の open PR の base が古くなり、
  並列実行が収束しない）
- ruleset を API の PUT で更新すると、必須チェックの一覧が丸ごと置き換わり、`integration_id` の束縛が落ちることがある。
  GET した JSON の必須チェックだけを差し替えて送り、更新後に下記で未束縛が 0 件であることを確かめる

```bash
# 件数・strict・bypass・integration_id が未束縛の必須チェック（unbound は空配列であること）
gh api repos/Fandhe-AI/fandhe-workshop/rulesets/24879606 --jq '{
  enforcement, bypass: (.bypass_actors | length),
  strict: ([.rules[] | select(.type=="required_status_checks") | .parameters.strict_required_status_checks_policy] | first),
  total: ([.rules[] | select(.type=="required_status_checks") | .parameters.required_status_checks[]] | length),
  unbound: [.rules[] | select(.type=="required_status_checks") | .parameters.required_status_checks[] | select(.integration_id == null) | .context]
}'
```

### TASK-33 で追加したチェック（2026-10-14 に ruleset へ反映済み）

`ci.yml` に追加した `rust-lint`・`rust-test` の 2 件（上の表の `rust ...`）を、2026-10-14 に必須チェックへ加えた。
必須チェックは 10 件から 12 件になった。strict は false、bypass は無しのまま、他の rule は変えていない。

- `deploy.yml` の `deploy` は `workflow_dispatch` だけで動き、PR に報告されないため、必須に入れない
- 反映は、ジョブを追加した変更のマージ直後に行った。マージ前に追加すると、古い `ci.yml` を持つ他の open PR で
  新しいチェックが「Expected」のまま残りマージできなくなるため。今後ジョブを追加するときも同じ順序で行う

### TASK-79 で追加したチェック（2026-10-18 に ruleset へ反映済み）

`ci.yml` に `browser-test`（チェック名 `browser test (Chromium)`、発行元は github-actions の 15368）を追加した。
`make test-browser` を同梱の Chromium で流し、1 本でも失敗すれば `ci-complete` ごと失敗する。ruleset の
必須チェックへの追加は、この変更（#1577）のマージ直後の 2026-10-18 に行った（上の TASK-33 と同じ順序の理由）。
必須チェックは 12 件から 13 件になった。strict は false、bypass は無しのまま、他の rule は変えていない。

- 手順: GET した JSON の必須チェックに `browser test (Chromium)`（`integration_id` 15368）を足して PUT し、
  上記のコマンドで `unbound` が空（total 13）であることを確かめた

## リポジトリ設定（2026-10-11 適用）

ワークフローの書き方に影響する設定。変更は `setup-repo-guards` スキルの手順に従う。

- `GITHUB_TOKEN` の既定権限は read。ワークフローはトップレベルで `permissions:` を明示し、書き込みが要るジョブだけ
  ジョブ単位で付与する（明示が無いワークフローは、書き込みが必要な処理で失敗する）
- Actions による PR の作成・承認は許可している（`update-external.yml` の日次の同期 PR のため）
- マージ方式は squash のみ。squash のコミット件名は PR タイトル、本文は PR 本文になる。マージ後のブランチは自動で削除される
- secret scanning と push protection、Dependabot の alerts と自動修正 PR、private vulnerability reporting が有効
- wiki と projects は無効

## 発火条件

- `ci.yml`: `pull_request` と `push`（`main` のみ）。push を全ブランチにしない（PR との二重実行になる）。`ci.yml` は secret を参照しない（`scripts/check-workflow-secrets.sh` が `check` ジョブで強制する）
- 例外: `deploy.yml` は `workflow_dispatch` だけで動く。配信は secret・書き込み権限・`environment` を要するため、PR の起点を持たない別ワークフローに置く（ジョブ追加チェックリストの「新しいワークフローを作らない」の例外）。`pull_request`・`pull_request_target`・`push` を足さない。トップレベルとジョブの `permissions` は `{}` から始め、必要な権限だけをジョブ単位で付ける
- `concurrency` は `group` は PR では workflow + ref、それ以外は `run_id` で分け、`cancel-in-progress` は PR のみ true にする
  （`${{ github.event_name == 'pull_request' }}`。main / release の実行は cancel しない）
- 必須チェックを含む workflow に workflow 単位の `paths` / `paths-ignore` を付けない
  （未実行の必須チェックが pending のままマージ不能になる）。変更検知はジョブ内で行い、対象のジョブをジョブ単位の
  `if:` でスキップして、`ci-complete` で skipped を許容する（例: `changes` ジョブが `scripts/ci-changes.sh` で出力
  `test-hooks-os` を決め、`test-hooks-macos` の `if:` がそれを読む。`ci-complete` は同じ出力と突き合わせ、skip が
  正しい場合だけ許容する）
- draft PR をスキップする場合は `types` に `ready_for_review` を含める
- `schedule` は必要最小限の頻度にする（`update-external.yml` は日次 1 回）

## ジョブ追加チェックリスト

- [ ] 新しいワークフローを作らず、既存ワークフローへのジョブ追加を優先する
- [ ] `ci-complete` の `needs` と ruleset の必須チェックに追加した（`needs` から漏れると未検査のままマージ可能になり、
      ruleset から漏れると自動マージが辞退する）。skipped を許容する場合は jq の条件も更新する
- [ ] 実処理は `make <target>` / `scripts/*.sh` を呼ぶ（CI 独自にロジックを書かない）
- [ ] lefthook と同じ検査を同じ範囲で重複させない（`git-hooks.md`）
- [ ] 不要な `needs` を作らない（独立ジョブは並列に走らせる）。逆にセットアップ時間が支配的な小ジョブは統合する
- [ ] `actions/checkout` は `persist-credentials: false`、深さは必要最小限（既定の 1。差分が要る場合のみ 2）
- [ ] ツール導入はバージョン固定 + SHA256 検証（`ci.yml` の `check` ジョブの方式）
- [ ] キャッシュを使う場合は「キャッシュ方針」に従う（効果を実測し、保存は main への push のみ。`setup-*` 系 action の
      一体型キャッシュは PR でも保存するため使わない。確認結果を PR 本文に書く）
- [ ] サードパーティ action は既存と同じピン止め方式（コミット SHA + バージョンコメント）。自組織の reusable workflow は `@latest`
- [ ] macOS など高コスト runner は必要な変更時のみ実行する（`changes` ジョブの出力と、`test-hooks-macos` のジョブ単位の
      `if:` を参照。チェック自体は skipped として常に報告させる）
- [ ] `timeout-minutes` を設定した
- [ ] `actionlint` を通した

## キャッシュ方針

現状（2026-10-12）: GitHub Actions のキャッシュは使っていない。`ci.yml` の `rust-lint`・`rust-test` もキャッシュなしで
導入した。「入れる前に効果を測る」を満たすため、まずキャッシュなしの所要時間・依存のダウンロード件数・ビルド件数を
計測する。キャッシュは、足す前後の復元秒数・ダウンロード件数・再ビルド数を比べて効果を確認できた後に、別の変更で
追加する（`Swatinem/rust-cache` を使う場合は下記の保存条件とキーの設計に従う）。

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
- Rust を追加する場合: `Swatinem/rust-cache` なら `save-if: ${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}`
  を渡す。Fandhe-AI/actions の `rust-base-ci.yml` を `cache: true` で呼ぶなら
  `cache-save: ${{ github.event_name == 'push' && github.ref == 'refs/heads/main' }}` も渡す（共通側の `cache-save` の
  既定は true で、PR からも保存する）。どちらも `github.ref` だけの式は main 上の `schedule`・`workflow_dispatch`
  でも保存するため、`github.event_name` も判定する

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
- キーが捕捉しない変更で保存内容が変わるときは、キーの版セグメント（`-v2-` 等）を上げる。`restore-keys` の版も同時に
  上げ、版セグメントが無ければ追加する。対象はルートの `Cargo.toml` の `[profile]`、`RUSTFLAGS`・`CARGO_PROFILE_*` 等の
  env、`.cargo/config.toml`、ビルドコマンド・フラグなど（Rust の具体例は `.claude/rules/rust-build-profile.md`）。上げないと
  ツールチェーンの版と同じく古い成果物に exact hit し続ける。キーが `Cargo.toml` を含んで変わる場合も、`restore-keys` で
  旧世代を部分復元すると新旧の依存成果物が混ざった大きな初回保存になるため、版を上げて旧世代を拾わないようにする
- `Swatinem/rust-cache` はキーに `CARGO`・`RUST` 等で始まる env（action を実行した時点の値）・`.cargo/config.toml`・
  `rust-toolchain(.toml)`・**workspace メンバーの** `Cargo.toml` を含める（上流 `src/config.ts` は `cargo metadata --no-deps`
  の packages の manifest だけをハッシュする）。ルートが仮想 manifest（`[package]` なし）だとルートの `Cargo.toml` は
  ハッシュされず、ルートの `[profile]` の変更はキーに入らない。これや、後続のステップで設定する env・ビルドコマンドの
  フラグを変えたときは、`prefix-key` の版（既定 `v0-rust`）を上げる（fandhe-edge-model #530）
- Fandhe-AI/actions の `rust-base-ci.yml` を `cache: true` で呼ぶ場合、clippy・test のキーは
  `<cache-key-prefix>-<OS>-{clippy,test}-v4-<rustc の版>-<Cargo.lock のハッシュ>`（`cache-key-prefix` の既定は `rust-base-ci`）で、
  版セグメントは呼び出し側から変えられない（`cache-key-prefix` を変えると deny ジョブを含む全キャッシュが cold になる）。
  キーに入らない変更をしたら、**マージ後に** main スコープの `<prefix>-<OS>-{clippy,test}-v4-*` を削除する。
  マージ前に消すと、その間の main への push が旧設定の内容を同じキーで保存し直す

    ```bash
    # clippy・test それぞれ、OS（runner.os: Linux 等）ごとに一覧して id で消す
    gh cache list -R Fandhe-AI/fandhe-workshop -L 100 --ref refs/heads/main --key <prefix>-Linux-clippy-v4- --json id,key
    gh cache delete -R Fandhe-AI/fandhe-workshop <id>
    ```

### サイズを抑える

- 1 世代の合計が 10GB に余裕をもって収まるようにする。キャッシュの種類を増やすときは 1 件のサイズ × 種類 × OS
  （本リポの `test-hooks`・`test-hooks-macos` のような ubuntu / macOS の組）を見積もる
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
| `ci.yml` | rust-lint・rust-test | 未計測（初回の実行後に更新する） |
| `ci.yml` | browser-test | 未計測（TASK-79。初回の実行後に更新する。timeout は 60 分） |
| `ai-review.yml` | codex の preflight・review・post_feedback | 約 1〜2 分 |
| `update-external.yml` | 日次の外部ソース追従（reusable） | 実績なし（未計測） |

施策: macOS のフック回帰テスト（`test-hooks-macos`）はフック関連の変更時のみ実行（`changes` ジョブ。不要時は skipped として報告）。main への push 実行は cancel しない。
