# testdata

試験データの置き場です。2 つ以上の crate が使うデータと、spec repo の試験で使っていたデータの写しを置きます。
`docs/architecture.md` の第 8 節で決めた置き場に当たります。

実装 repo は、repo の外（兄弟の `03-poc/`・`prototype/` など）を参照せずに試験できる必要があります（BUILD-14）。
そのために、試験に必要なデータを出所付きで写しました。

## 出所

| 項目 | 内容 |
|---|---|
| 取り込み元 | `Fandhe-AI/fandhe-workshop-spec`（private） |
| コミット | `159702f13d8d38c90e1b703901a26cd3616e912a` |
| 取り込み日 | 2026-10-12 |
| 取り込み元の一覧 | `03-poc/third-party-build/test-data/sibling-paths-2.txt`（17 path） |
| 方針 | 読むだけ・移さずに写す・バイト一致 |

配置は、取り込み元の `03-poc/<PoC 名>/...` から先頭の `03-poc/` だけを外した形です。
`.sha256` の manifest と、データの中の相対参照（`../min-3step.json` など）を書き換えずに使えるようにするためです。

## 取り込んだ 17 path

「git オブジェクト」は、取り込み元のコミットでのオブジェクト ID の先頭 8 文字です。
15 path は、写した後も同じ ID になります（`git rev-parse HEAD:testdata/<行き先>` で確かめられます）。

| # | 行き先（`testdata/` の下） | 種別 | ファイル数 | git オブジェクト | 内容 |
|---|---|---|---|---|---|
| 1 | `engine-event-consistency/test-data` | ディレクトリ | 13 | 3a62651e | 最小 3 Step の教材（形式 A と IR）、台本の入力、性質テストの seed |
| 2 | `input-field-ime/test-data` | ディレクトリ | 26 | 4577a0fa | IME の試験文、貼り付けの見本、ch1・ime のパック |
| 3 | `module-reuse-rounds/test-data` | ディレクトリ | 52 | 14e02dcc | 周回の教材パック、欠陥の周回 |
| 4 | `save-restore-versions/test-data` | ディレクトリ | 271 | e9e7547d | 版違いの教材パック（v1.0.0 から v2.0.0 と other-pack） |
| 5 | `save-restore-versions/test-data-addendum` | ディレクトリ | 16 | 9f3f1a8c | 版違いの履歴、壊れた履歴 |
| 6 | `save-restore-versions/test-data-addendum.sha256` | ファイル | 1 | 22191bb4 | manifest（`test-data-addendum/` の中から検証） |
| 7 | `save-restore-versions/test-data.sha256` | ファイル | 1 | 88b2af73 | manifest（`test-data/` の中から検証） |
| 8 | `untrusted-input-safety/test-data` | ディレクトリ | 4 | fd796c9a | XSS の試験文、秘密情報に似せた形、過大入力の生成器 |
| 9 | `untrusted-input-safety/test-data-addendum` | ディレクトリ | 3 | d95f49d8 | 頑健性・木のケース |
| 10 | `untrusted-input-safety/test-data-addendum-2` | ディレクトリ | 1 | 79a7bc56 | 木のケース 2 |
| 11 | `untrusted-input-safety/test-data-addendum-3` | ディレクトリ | 5（元は 6） | 0457eb77（一致しない） | 欠陥パック（巡回する IR） |
| 12 | `untrusted-input-safety/test-data-addendum-4` | ディレクトリ | 3 | 853efb41 | 追加ケース、秘密情報の形 |
| 13 | `untrusted-input-safety/test-data.sha256` | ファイル | 1 | e27200c4 | manifest |
| 14 | `untrusted-input-safety/test-data-addendum.sha256` | ファイル | 1 | 30b98942 | manifest |
| 15 | `untrusted-input-safety/test-data-addendum-2.sha256` | ファイル | 1 | 5e0274c3 | manifest |
| 16 | `untrusted-input-safety/test-data-addendum-3.sha256` | ファイル（作り直し） | 1 | 468b5152（元。一致しない） | manifest（6 行から 5 行へ） |
| 17 | `untrusted-input-safety/test-data-addendum-4.sha256` | ファイル | 1 | ac734b56 | manifest |

合計は 401 ファイルです（取り込み元は 402 ファイル）。

### 取り込まなかったもの

| 対象 | 理由 |
|---|---|
| `untrusted-input-safety/test-data-addendum-3/make-addendum3.py` | 生成の記録で、試験は読みません。`prototype/` の golden を相対パスで読むため、写すと「`prototype/` への相対参照 0 件」に反します |
| PoC-3 の変異パック 24 件 | `prototype/packs/bbs-tutorial` と `defects/mutations*.toml` から `fandhe-pack defects` で生成するデータです。生成器（後続の pack の実装）と教材パックの公開可否（D-11・TASK-14）に依存するため、今回は取り込みません |

`test-data-addendum-3.sha256` は、`make-addendum3.py` の 1 行（ハッシュ `3cc5d8ee...`）を除いた 5 行です。
元の manifest（git オブジェクト `468b5152...`）は 6 行でした。

## 本 repo で作ったデータ

上の取り込みとは別に、本 repo で作ったデータです。

| 行き先（`testdata/` の下） | 出所 | 作成日 | 用途 |
|---|---|---|---|
| `contract/` | 本 repo（TASK-205 1/2） | 2026-10-15 | 公開 API の契約のイベント列のサンプル。結線の合否の基準 |

## 種類から探す

| 種類 | 場所 |
|---|---|
| 最小 3 Step の教材 | `engine-event-consistency/test-data/min-3step-pack/`（形式 A）と `min-3step.json`（IR） |
| 欠陥パック | `untrusted-input-safety/test-data-addendum-3/cycle-ir/`（巡回）、`save-restore-versions/test-data-addendum/histories/b-*.json`（壊れた履歴）、`module-reuse-rounds/test-data/defect-cycle.md`（欠陥の周回） |
| 版違いの履歴 | `save-restore-versions/test-data/packs/`（版違いのパック）、`save-restore-versions/test-data-addendum/histories/` |
| 入力の試験文 | `untrusted-input-safety/` の各 `test-data*/`、`input-field-ime/test-data/paste-samples/` |
| IME の試験文 | `input-field-ime/test-data/ime-sentences.{md,json}`、`key-procedures.md` |
| 契約のサンプル | `contract/event-sequence.json`（説明は `contract/README.md`） |

## manifest の検証

`.sha256` の manifest は、検証する場所が 2 通りあります。どちらも、中身を書き換えていなければ全行が `OK` になります。

```bash
# save-restore-versions: データのディレクトリの中から流す
(cd testdata/save-restore-versions/test-data && shasum -a 256 -c ../test-data.sha256)
(cd testdata/save-restore-versions/test-data-addendum && shasum -a 256 -c ../test-data-addendum.sha256)

# untrusted-input-safety: PoC のディレクトリから流す
(cd testdata/untrusted-input-safety && for f in *.sha256; do shasum -a 256 -c "$f"; done)
```

## 規則

- データを直さない。写したデータは、取り込み元と同じバイト列を保ちます。
  整形ツール（`editorconfig-checker`・`shellcheck`・エディタの自動整形）の対象から外し、`.gitattributes` で
  改行の変換も止めています。CRLF やタブなど、試験の意図に含まれる形が変わらないようにするためです。
- repo の外への相対参照を持たない。`make check-testdata`（`make check` に含まれます）が、`testdata/` の中の
  `../` で始まる参照をファイルの位置から解決し、repo のルートより上へ出る参照と、兄弟の `03-poc/`・`prototype/`
  を指す参照を失敗にします。
- 新しいデータを足すときは、上の表に出所の行を足します。
- 1 つの crate だけが使う小さな入力は、その crate の `tests/` に置いてもかまいません。
- 最小 3 Step の教材が読めることは、`crates/pack/tests/testdata_min_3step.rs` で確かめます。

## 既知の扱い

- md の中の `../env.md`・`../README.md`・`../logs/...`・`../measurements/...` は、spec repo 側の文書を指す
  記録上のリンクです。本 repo には宛先がなく、試験も読みません。
- `XSS-IR3` や `XSS-54/etc/passwd` に `../` を重ねたパス遡りは、入力の安全性を確かめる試験値です。
  試験のコードでパスとして辿らないでください。
- `make-variants.py`・`make-paste-samples.py`・`oversize/generate.sh` は、データを作り直すための記録・生成器です。
  `make test` からは実行しません。
- 種類別の再配置（`testdata/ime/` など）は、後続のタスクで決めます。

## 公開の扱い

この repo は public です。写したデータは、マージ時に公開されます。

- 中身は、spec repo の試験データの写しです。`kakuu-*` などの題材は架空です。
- 2026-10-12 に、取り込み元の 17 path（402 ファイル）を機械で走査しました。秘密情報の検知パターン、
  `fandhe.com`・個人のパスの文字列は 0 件でした。URL のホストは `example.invalid`・`example.com`・
  `github.com`・`localhost`・`127.0.0.1` だけでした。
- 公開前の人の確認の方法は TASK-14 で決めます。2026-10-12 時点で未決定のため、人の確認・承認は得ていません。
