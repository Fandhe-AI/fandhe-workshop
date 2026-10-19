# 履歴の形式の文書

```text
履歴の形式の文書の版: 1
対象の契約の版（HISTORY_CONTRACT_VERSION）: 1
対象の契約の版（HISTORY_VERSION_CONTRACT_VERSION）: 1
決定日: 2026-10-19
状態: 固定（契約）
```

## 1. 位置づけと版

- 本文書は、履歴（保存・書き出しする包み）の形式を一か所で見られるようにした一覧である（TASK-188 2/2）
- 正は `crates/contract/src/history.rs`（包み・`ops`）と `crates/contract/src/history_version.rs`（版の欄）の型と rustdoc である。本文書は一覧であり、スキーマを再定義しない。食い違いを見つけたら、型を正として本文書を直す
- 「履歴の形式の文書の版」は本文書の版（整数）である。型の版（`HISTORY_CONTRACT_VERSION`・`HISTORY_VERSION_CONTRACT_VERSION`）とは別に管理し、冒頭の囲みに書く。型の版を上げたら本文書の版も上げ、変更履歴に記録する
- 変更が要るときは、仕様を先に直す。仕様は private のため、本文を転記せず ID（D-53、VER-1 など）で参照する
- 冒頭の版と、2 節・3 節の表の欄名の並びは、`crates/contract/tests/history_format_doc.rs` が型と照合する

## 2. 包みの欄

包み `HistoryEnvelope` の欄を、直列化の順に並べる（SAVE-2）。

| 欄 | 型 | 必須 | 意味 |
|---|---|---|---|
| `format` | 文字列 | 必須 | 形式名。書くときは `fandhe-workshop-history` |
| `history_format` | 整数 | 必須 | 包みの形の版。書くときは `1` |
| `pack_id` | 文字列 | 必須 | 教材の ID |
| `versions` | 版の欄 | 必須 | 4 軸の版（3 節） |
| `start` | 文字列 | 必須 | 最初の開始点の ID |
| `ops` | 配列 | 必須 | 入力の列。空でもよい |
| `snapshot` | 写し | 必須 | 実行環境の写し。core の `SessionState` |

- 全欄が必須で、最上位の未知の欄は拒否する（失敗を拒否に倒す）
- `ops` の要素は `submit`・`draft`・`reload`・`input`・`tick`・`control` の 6 種である。Enter 専用の送信と早送りの入・切は入らない（SAVE-24・D-54）
- `snapshot` の中の形は `docs/engine-api.md`（`CONTRACT_VERSION`）が正である
- サンプルは `testdata/contract/history/` にある（説明は同じ場所の `README.md`）

## 3. 4 軸の版

履歴は `versions` の欄に、エンジン・教材形式・教材内容・参照元の 4 軸の版を持つ（VER-1）。軸と JSON の欄の対応は次のとおりである。

| 軸 | 欄 | 意味 |
|---|---|---|
| エンジン | `engine` | エンジンの crate の版（主.副.修） |
| 教材形式 | `ir_format`・`vocab` | IR の構造の版と、教材が宣言した語彙の版 |
| 教材内容 | `content`・`content_hash` | `pack.version` と、本文の木を除いた IR の SHA-256（D-51） |
| 参照元 | `refs` | 参照元の名前から commit への表。空でも `{}` を書く |

- 欄の順は `engine`・`ir_format`・`vocab`・`content`・`content_hash`・`refs` で、全欄が必須である。版の欄が無い履歴は開かない（`missing_version`）
- `history_format` は包みの形の版で、4 軸には含まれない
- 版の書き方は core の `version_record`、開く可否の判定は core の `history_open` が担う。作成者向けの説明は `docs/pack-authoring.md` にある

## 4. 追記だけで増える列

結論: 同じ実行の中で、履歴は追記だけで増え、確定した部分を書き換えない「版つき・単調に増える列」である（EXT-5・D-53）。初回は同期を実装せず、同期の試験も無い（D-5）。対象は書き出しの形（`ops` と写し。D-52）で、写しの列の欄を含む。

前の包みを prev、後の包みを next とすると、次の 4 点がすべて成り立つ。

| 対象 | 規則 |
|---|---|
| 頭の欄 | `format`・`history_format`・`pack_id`・`start`・4 軸の `versions` が変わらない |
| `ops` | n を prev の件数として、next は n 件以上で、先頭 n - 1 件が等しい。n 件目は等しいか、両方が `Draft` である |
| `snapshot.log` | prev のものが next のものの接頭辞である |
| `snapshot.checkpoints` | prev の各項目が next に同じ値で残り、`next_checkpoint` が減らない |

- `ops` の末尾の未確定の `Draft` だけは置き換わる。連続する下書きを最後の 1 件に畳む今の記録規則のままである
- 対象外: `snapshot` の `run`・`entry`・`drafts`・`hints_opened` は現在の状態で、操作のたびに上書きされる。写しの全体が単調なのではなく、単調と主張するのは上の欄だけである
- 成り立つ経路: 記録 1 段ごと、離れた 2 点、書き出し→読み込みを通した包み、`boot_session` で復元した後の保存の入口と続きの記録
- 根拠: `crates/core/tests/history_append_only.rs`、変異 `EXT5-1`〜`EXT5-4`（`crates/core/tests/mutations/`）。記録は `docs/testing.md` の 5.141 節にある

## 5. 書き出しの形

- 書き出しは `ops` と写しの両方である（D-52・SAVE-17。`EXPORT_FORM`）。`ops` だけの包みは作れず、読み込みでも拒否する
- 未送信の下書きを含める（SAVE-18。`EXPORT_INCLUDES_DRAFT`）。書き出す前の案内は画面の責務である
- 早送りの入・切は `ops` に入れない（D-54。`OPS_RECORDS_FAST_FORWARD`）

## 6. 主張しないこと

- 同期（D-5）
- `ops` を適用し直した結果と写しの一致の詳細（TASK-174・TASK-310・TASK-315 の範囲）
- 読み込みの判定の順序と拒否理由（`load_reason`）、大きさ・件数・深さの上限（読み込み口の責務）
- 文字列の欄（`pack_id`・`start`・下書き・送信）は利用者・教材由来で信頼できず、包みの型は加工しない。HTML へのエスケープは web 層の責務である

## 変更履歴

| 文書の版 | 日付 | 内容 |
|---|---|---|
| 1 | 2026-10-19 | 新設（TASK-188 2/2） |
