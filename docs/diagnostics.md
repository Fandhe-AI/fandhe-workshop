# 診断コードの一覧

```text
診断コードの一覧の版: 1
決定日: 2026-10-15
状態: 固定（契約）
```

## 1. 位置づけと版

教材パックの検査（`fandhe-pack validate`・`test`・`compile`）が出す診断コード（`E-*`）を、名前・出る条件・出所の 04 の ID・区分で固定する契約である（TASK-195。PACK-12・PACK-15・PACK-16・PACK-20・PACK-22・PACK-28）。後続の実装タスク、作成ツール、講師資料は、本一覧を入力にする。

- 「一覧の版」は本文書の整数の版である。語彙の版・`ir_format`・`LIMITS_CONTRACT_VERSION` とは別に管理する
- 区分の「語彙の文書」は、PoC の語彙の文書（v1 の 13 節、v1.1、v1.3）を指す。repo の `docs/vocabulary/vocabulary.md` ではない（同文書の 10 節が診断コードを本一覧に委ねている）
- 条件は要旨である。検査の中身の正は、04 の各 PACK と各実装タスク（TASK-46・TASK-47・TASK-49〜TASK-55 など）とする
- 一覧と実装（`crates/pack/src/diag.rs` の `DiagCode`）の一致の自動検査は TASK-400 が持つ。版 1 は自動検査を持たない
- 仕様は private のため、本文を転記せず ID で参照する

### 変更規則

- 固定した行は書き換えない・消さない・改名しない
- 足すときは行を加え、版を上げ、変更履歴に記録する
- 名前を変える必要が出たら、先に spec の 04 を直し、版上げの契約タスクを立てる

## 2. 本表（04 の PACK-2〜PACK-28 に現れるコード）

本表の行数を「一覧の件数」とする（32 件）。他の節には、本表と同じ形の行を置かない。

| コード | 出る条件（要旨） | 出所の 04 の ID | 区分 |
|---|---|---|---|
| `E-PARSE` | 構文として読めない。未知の欄（`script` など語彙にない欄）がある。`workshop.yaml` と `workshop.toml` が両方ある、またはどちらも無い。外観に名前にない値や未知の項目がある | PACK-2・PACK-3・PACK-7・PACK-21 | 語彙 v1 |
| `E-FORMAT-VERSION` | `pack.ir_format` が、エンジンの読める版（1 と 2）のどちらでもない | PACK-24・PACK-28 | 語彙 v1 |
| `E-STRUCTURE` | 構造の誤り。正答の選択肢に `on_wrong` がある。`body_levels` に `A` がある。`requires`・`unmet` の片方だけがある。`use` と `input` の同時指定、またはどちらも無い。`use` と `confirm` の同時指定。`params` にない `args`。正答の無い `answer` | PACK-15・PACK-17・PACK-18・PACK-19・PACK-28 | 実装にだけ |
| `E-REF-STEP` | 存在しない Step を参照している | PACK-28 | 語彙 v1 |
| `E-REF-VIEW` | 存在しないビューを参照している | PACK-28 | 語彙 v1 |
| `E-REF-FIXTURE` | 存在しない練習データを参照している | PACK-28 | 語彙 v1 |
| `E-REF-HINT` | 存在しないヒントを参照している | PACK-28 | 語彙 v1 |
| `E-REF-REJECTION` | 存在しない拒否の文面を参照している | PACK-15・PACK-28 | 語彙 v1 |
| `E-REF-MODULE` | 存在しないモジュールを参照している | PACK-28 | 語彙 v1 |
| `E-REF-COMMAND` | 存在しないコマンドを参照している | PACK-28 | 実装にだけ |
| `E-REF-BODY` | 存在しない本文を参照している | PACK-28 | 語彙 v1 |
| `E-DUP-ID` | Step の ID が重複している（章をまたいでも） | PACK-28 | 語彙 v1 |
| `E-UNREACHABLE` | どの開始点からも到達できない Step がある | PACK-28 | 語彙 v1 |
| `E-DEAD-END` | `final` でなく、`next` が空（またはすべて条件つき）の Step がある | PACK-28 | 語彙 v1 |
| `E-MODULE-CYCLE` | モジュールの呼び出しが巡回している。入れ子が 8 段を超える | PACK-19・PACK-28 | 語彙 v1 |
| `E-EMPTY-NEXT-ACTION` | 拒否・問い返しの文面で「次の行動」が空になる（`test` では差し込みの後の値で数える） | PACK-12・PACK-28 | 語彙 v1 |
| `E-NO-CONFIRM` | 入力を持つ Step が、確認も、理由つきの `confirm: { kind: none }` も持たない | PACK-13・PACK-28 | 語彙 v1 |
| `E-SLOT-SYNTAX` | 差し込みの `{{` が閉じていない | PACK-16 | 語彙 v1.1 |
| `E-SLOT-UNKNOWN` | 差し込みの名前が、閉じた集合にない | PACK-16 | 語彙 v1.1 |
| `E-SLOT-CONTEXT` | 差し込みを、使えない分類の文面で使っている | PACK-16 | 語彙 v1.1 |
| `E-SLOT-NO-FALLBACK` | 空になりうる差し込みに、代替の文が無い | PACK-16 | 語彙 v1.1 |
| `E-SLOT-NEEDS-DO` | `do` の無い Step で `step_do` を使っている | PACK-16 | 語彙 v1.1 |
| `E-GUIDANCE-LEAK` | 隠すと宣言した語が、その水準の本文または `do` に含まれる | PACK-17 | 語彙 v1.1 |
| `E-HINT-HIDDEN` | ヒントを持つ Step が、ある案内レベルでヒントを 1 つも出さない | PACK-17 | 語彙 v1.1 |
| `E-PROFILE-PREFIX` | コマンドの接頭辞が不正（1〜4 文字でない など） | PACK-20 | 語彙 v1.3 |
| `E-PROFILE-COMMAND-NAME` | 接頭辞なしでは打てないコマンド名がある | PACK-20 | 語彙 v1.3 |
| `E-PROFILE-TEXT` | 接頭辞なしの設定で、自然文の入力・規則がある | PACK-20 | 語彙 v1.3 |
| `E-PROFILE-NO-INPUT` | 入力欄なしの設定で、コマンド・自然文の入力・規則がある | PACK-20 | 語彙 v1.3 |
| `E-PLAY-INPUT-CLASS` | `test` で、教材テストの `cmd:`・`text:` が画面で別の種類になる（接頭辞が `/` 以外のときだけ） | PACK-20 | 語彙 v1.3 |
| `E-LEFT-MULTI-VIEW` | `view_tabs: hidden` で 2 つ以上のビューを宣言した Step がある | PACK-22 | 語彙 v1.3 |
| `E-LEFT-TARGET-VIEW` | 宛先のビューが、その Step の画面に出ない | PACK-22 | 語彙 v1.3 |
| `E-LEFT-TARGET-ROW` | 宛先の経路が行にならない（静的な近似で、必要条件だけを調べる） | PACK-22 | 語彙 v1.3 |

- `E-PROFILE-*`・`E-LEFT-*` は、`pack.profile`・`pack.left_pane` を既定から変えたときだけ検査される

## 3. 件数の記録

- 本表は 32 件である。2026-10-15 に、`pack-format.md` の PACK-2〜PACK-28 の行から数えた
- 数え方: 当該の行から正規表現 `E-[A-Z][A-Z0-9*]*(-[A-Z0-9*]+)*` で取り出して重複を除く。`GUIDE-14` のような ID の後半が当たった数字だけの形は数えない
- PACK-2〜PACK-28 の行には、`E-LIMIT-*`・`E-INSTR-*` は現れない

### 出所の 04 の ID ごとの内訳

| 04 の ID | コード | 件数 |
|---|---|---|
| PACK-2 | `E-PARSE` | 1 |
| PACK-3 | `E-PARSE` | 1 |
| PACK-7 | `E-PARSE` | 1 |
| PACK-12 | `E-EMPTY-NEXT-ACTION` | 1 |
| PACK-13 | `E-NO-CONFIRM` | 1 |
| PACK-15 | `E-STRUCTURE`・`E-REF-REJECTION` | 2 |
| PACK-16 | `E-SLOT-SYNTAX`・`E-SLOT-UNKNOWN`・`E-SLOT-CONTEXT`・`E-SLOT-NO-FALLBACK`・`E-SLOT-NEEDS-DO` | 5 |
| PACK-17 | `E-GUIDANCE-LEAK`・`E-HINT-HIDDEN`・`E-STRUCTURE` | 3 |
| PACK-18 | `E-STRUCTURE` | 1 |
| PACK-19 | `E-MODULE-CYCLE`・`E-STRUCTURE` | 2 |
| PACK-20 | `E-PROFILE-PREFIX`・`E-PROFILE-COMMAND-NAME`・`E-PROFILE-TEXT`・`E-PROFILE-NO-INPUT`・`E-PLAY-INPUT-CLASS` | 5 |
| PACK-21 | `E-PARSE` | 1 |
| PACK-22 | `E-LEFT-MULTI-VIEW`・`E-LEFT-TARGET-VIEW`・`E-LEFT-TARGET-ROW` | 3 |
| PACK-24 | `E-FORMAT-VERSION` | 1 |
| PACK-28 | `E-REF-STEP`・`E-REF-VIEW`・`E-REF-FIXTURE`・`E-REF-HINT`・`E-REF-REJECTION`・`E-REF-MODULE`・`E-REF-COMMAND`・`E-REF-BODY`・`E-DUP-ID`・`E-UNREACHABLE`・`E-DEAD-END`・`E-MODULE-CYCLE`・`E-EMPTY-NEXT-ACTION`・`E-NO-CONFIRM`・`E-FORMAT-VERSION`・`E-STRUCTURE` | 16 |

件数は ID ごとの出現数であり、足し合わせても本表の件数にはならない（同じコードが複数の ID に現れるため）。

### 区分の内訳

| 区分 | 件数 |
|---|---|
| 語彙の文書（v1） | 15 |
| 語彙の文書（v1.1） | 7 |
| 語彙の文書（v1.3） | 8 |
| 実装にだけある | 2（`E-STRUCTURE`・`E-REF-COMMAND`） |
| 合計 | 32 |

### 実装の列挙との対応

`DiagCode::ALL` は 39 件である。内訳は、本表 32 件、本表外 2 件（5 節）、上限超過 5 件（4 節）である。

## 4. 上限超過のコード（参照のみ）

次のコードの値・条件・拒否の動作の正は、`engine-safety.md`（SAFE-17〜SAFE-24・SAFE-43）と contract の `PackLimitCode`（TASK-199）である。本一覧は条件を再定義せず、件数にも数えない。

- `E-LIMIT-PACK-SIZE`・`E-LIMIT-MD-SIZE`: SAFE-22・AUTHOR-10
- `E-LIMIT-DEPTH`・`E-LIMIT-SYMLINK`: SAFE-43・AUTHOR-10
- `E-LIMIT`: AUTHOR-10（AUTHOR-11 が検討中）

- 素の `E-LIMIT` は contract の `PackLimitCode` に無い。扱いは TASK-398・TASK-400 が決める
- 上限超過は、中身を読まずに拒否する（SAFE-22・SAFE-43）。本一覧はこの条件を緩めない

## 5. 本表に入れないコード

- `E-PLAY-INCOMPLETE`・`E-PLAY-CATEGORY-MISMATCH`: PoC の語彙 v1 の 13 節と骨格の列挙にはあるが、04 に出所が無い。版 1 では固定せず、件数に数えない。扱いは TASK-398・TASK-400 が決める
- `E-INSTR-*`: PACK-49・INSTR-18 の検討中の案である。講師資料の契約（TASK-559）が持つ
- 実行時の読み込みの拒否理由（`too_large`・`too_deep` など）: CLI の診断コードではない。contract の `LimitReason` が持つ

## 6. 本一覧を入力にするタスク

TASK-40・TASK-42〜TASK-47・TASK-49〜TASK-59（spec の 05 の入力欄に TASK-195 が書かれていることを 2026-10-15 に確認した）、TASK-400（一致の検査）、TASK-398、TASK-399、TASK-604、TASK-559、TASK-189（診断コードの追加の手続き）。

## 7. 変更履歴

| 版 | 決定日 | 内容 | 理由 |
|---|---|---|---|
| 1 | 2026-10-15 | 初版。04 の PACK-2〜PACK-28 に現れる 32 件を固定。`E-LIMIT-*` は参照のみ | TASK-195 |
