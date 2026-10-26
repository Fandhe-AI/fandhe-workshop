# 操作プロファイルと外観

教材の作成者が、入力の作法（コマンドの接頭辞・入力欄の有無）と画面の外観を、どこまで選べるかをまとめた文書です。選べるもの、選べないもの、外観の 4 値の意味を書きます（AUTHOR-12・AUTHOR-13）。

- 文書の版: 1
- 確認した日: 2026-10-26
- 仕様は private のため、本文を転記せず ID（PACK-20・PACK-21・PACK-22・W-13・C-8・W-22・SECOND-10・D-64）で参照します
- 語彙の文書（`docs/vocabulary/vocabulary.md`）の本文は固定の契約で、この文書では書き換えません。外観の定義はこの文書と `docs/pack-authoring.md` の外観の節に置きます（TASK-398 の決定）

## 1. 選べるもの

`pack` の下に 3 つの欄があります。どれも省略でき、省略すると既定になります。値が既定と同じでも、欄を書けば語彙の使用です。`pack.vocab` には 1.3 以上を宣言してください（`E-VOCAB-MISMATCH`。`docs/pack-authoring.md` の「`pack.vocab` の宣言と使用の一致」）。

| 欄 | 拡張点 | 既定 | 選べる値 |
|---|---|---|---|
| `pack.profile.command_prefix` | EP-10 | `/` | 4 文字以下の記号（1.1） |
| `pack.profile.input_field` | EP-10 | `shown` | `shown`・`hidden` |
| `pack.appearance.theme` | EP-11 | `standard` | `standard`・`mono`・`high_contrast`・`reading`（2 節） |
| `pack.left_pane` | EP-12 | `world_list: shown`・`view_tabs: all` | `docs/pack-authoring.md` の「左エリア」と `docs/views.md` |

### 1.1 コマンドの接頭辞（`command_prefix`）

受講者の入力は、接頭辞で始まれば「コマンド」、そうでなければ「自然文」に分けます。分け方は画面と教材テストが同じ関数（`crates/contract/src/profile.rs`）を使います。

- 接頭辞は文字列です。次のどれかに当たると `E-PROFILE-PREFIX` で拒否します: 5 文字以上・制御文字を含む・空白を含む・ASCII の英数字を含む・先頭が `-`
- 空文字列（`""`）は書けます。接頭辞なしの教材で、入力はすべてコマンドとして扱います。このとき次の 2 つを検査します
  - 接頭辞なしでは打てないコマンド名（空・空白や制御文字を含む）があると `E-PROFILE-COMMAND-NAME` です。診断の項目は名前を写さず、名前順の位置（`command:#<n>`）で示します
  - 自然文の入力（`input.kind: text`）や自然文の規則（`on.text: true`）があると `E-PROFILE-TEXT` です
- 型が文字列でない、表でない、未知の欄がある場合は `E-PARSE` です
- 診断には、接頭辞の値を写しません
- 拒否・問い返しの文面の枠 `{{input_head}}`・`{{nearest_command}}` には、教材の接頭辞が付きます。`/` を決め打ちにしません

```yaml
pack:
  vocab: "1.3"
  profile: { command_prefix: "$" }
```

### 1.2 入力欄（`input_field`）

- `shown`（既定）: コマンドや自然文を打つ入力欄を出します
- `hidden`: 入力欄を出しません。次の画面の部品も出ません: コマンドの候補の一覧。選択肢のボタン・承認のボタン・チェックポイントなど、入力欄を使わない部品は出ます
- `hidden` の教材では、受講者が打つ入力を要する Step は書けません。コマンドまたは自然文を受ける `input`、またはコマンドや自然文の規則があると `E-PROFILE-NO-INPUT` です。選択・承認・画面の操作・待機の Step は書けます
- 値が `shown`・`hidden` 以外、型が文字列でない場合は `E-PARSE` です

### 1.3 教材テストの `cmd:` と `text:`（`E-PLAY-INPUT-CLASS`）

教材テストの入力は、`cmd:`（コマンドとして送る）か `text:`（自然文として送る）で書きます。接頭辞が `/` 以外の教材では、`cmd:` の値が接頭辞で始まらない、または `text:` の値が接頭辞で始まると、画面で別の種類の入力になるため `E-PLAY-INPUT-CLASS` で知らせます。接頭辞が `/` のときは検査しません。この診断は `fandhe-pack test` が出します。入力は止めず画面と同じ判定で流した上で報告し、1 件でもあれば終了コードは 1 です。`test` は画面の表示と停止（Esc）を確かめません。分ける関数と診断への写しは `crates/pack/src/profile.rs` の `check_play_input_class` にあります。

## 2. 外観の定義

外観（`pack.appearance.theme`）は、閉じた 4 値から選びます（PACK-21、EP-11）。

```yaml
pack:
  vocab: "1.3"
  appearance: { theme: reading }
```

### 2.1 契約として決まっていること

| 値 | 意味 |
|---|---|
| `standard` | 標準。省略したときの既定です |
| `mono` | 色に頼らず、モノクロで見せます |
| `high_contrast` | 文字と背景の差を大きくします |
| `reading` | 読みやすさを重視します。仕様（PACK-21）は、文字を 18px 以上、行間を 1.7 倍以上にすると定めます |

- 4 値の意味は要旨です。`reading` の数値は仕様に拠り、この repo の実装・試験ではまだ確認できていません（未確認。2.2 のとおり、外観ごとの CSS が無いためです）
- 画面は、外観を `main` 要素の `data-theme` 属性（値は上の 4 語）で表します。教材から任意の属性値は渡せません
- 外観は見た目だけを変えます。入力の判定・状態変化・遷移・履歴には影響しません
- どの外観でも、世界状態の変化の印（記号・文字・属性）は残ります。色だけで変化を伝えません
- 教材から CSS・色・書体・HTML は指定できません。そのための欄は 0 件です。値は 4 語の完全一致で、大文字を含む綴りや `dark` のような別の語、`theme` 以外の欄は `E-PARSE` です。診断には書いた値を写しません

### 2.2 2026-10-20 時点の実装の状態

契約と、画面の実装の進み具合は別です。確認した範囲は次のとおりです。

| 項目 | 状態 | 根拠 |
|---|---|---|
| 4 語の閉じた集合・既定・読み取りと `E-PARSE` | 実装済み | `crates/contract/src/appearance.rs`、`crates/pack/tests/appearance.rs` |
| 表示層へ渡す型と、`main` に付ける `data-theme` 属性の組（`DisplaySettings::root_attrs`） | 型は実装済み | `crates/web/src/profile.rs` |
| 画面の `main` への `data-theme` の付与 | 未結線。`root_attrs` を呼ぶ箇所が web の他のモジュールに無いことを確かめました | `crates/web/src` を `root_attrs`・`data-theme` で検索（2026-10-20） |
| 外観ごとの CSS（色・文字の大きさ・行間） | 未実装。`crates/web/static/app.css` と `index.html` に、外観ごとのスタイルがありません | 同上（`theme`・`data-theme` の検索で 0 件） |
| どの外観でも変化の印が残ること | 代役の部品を `data-theme` つきの `main` に置いた検査で、4 値とも記号・文字が残り見えていることを確かめました（2026-10-25）。製品の画面への `data-theme` の付与は未結線のままです | `tools/browser/theme-marks.test.mjs`、`docs/testing.md` の 5.303 |
| 人が画面を見て外観を確かめた件数 | 0 件 | 未確認 |

つまり、`reading` を選んでも、文字が大きくなる・行間が広がるという見た目の反映は、2026-10-20 時点の画面にはまだ現れません。外観ごとの見た目の実装は、画面のタスク（TASK-431〜TASK-603）が担当します。教材は、`theme` を書いておけば、画面が対応した時点で外観が効きます。対応後の見た目を、この文書の 2.1 の約束と照らして確かめてください。

### 2.3 「Claude Code 風の外観」について

作成者が「Claude Code 風の外観」を求めるときは、次のように読んでください。

- エンジンに、特定の製品名がついた外観はありません（C-8・W-22。エンジンは特定の教材・製品の知識を持ちません）。外観の 4 値の中に「Claude Code 風」という値はありません
- 画面の構成（左に成果物、右に操作の端末風の領域）は固定で、教材からは変えられません（W-13）
- 操作の作法は、`pack.profile` の既定（接頭辞 `/`・入力欄あり）と `standard` の組み合わせが、コマンドを `/` で始めて打つ端末風の既定です。この対応は、設定の既定値から読み取った説明です。「Claude Code 風」を設定の組み合わせとして定めた規則は、この repo の資料・実装には見つかっていません。未確認です
- 端末風の操作にしたくない教材は、`input_field: hidden` で入力欄をなくし、選択・承認・画面の操作で進める組み立てにできます。外観を `reading` などにして読みやすさを優先する選び方もあります（SECOND-10 の例）
- 人が「Claude Code 風に見える」と評価した件数は 0 件です

## 3. 選べないもの

次は、教材から変えられません。今の実装で確かめた項目だけを挙げます。

| 項目 | 状況 |
|---|---|
| 画面の領域の構成（左に成果物、右に操作） | 固定です（W-13）。`pack.left_pane` で絞れるのは、左エリアの一覧と切り替えの表示だけです |
| 色・書体・文字の大きさ・CSS・HTML | 指定する欄がありません。外観の 4 値の中から選びます |
| 進行の間隔 | 300ms でエンジンの定数です。`pack` にも Step にも欄がありません（`E-PARSE`。PACK-44） |
| 動きの抑制（処理中の段階の表示の有無） | OS の `prefers-reduced-motion` を読み込み時に 1 回だけ読みます。`pack` にも Step にも欄がありません（`E-PARSE`。A11Y-12・D-82） |
| 入力欄の仕様（Enter で送信、Shift+Enter で改行など） | 画面の仕様で、教材からは変えられません |
| 外観ごとの具体的な色の値 | 教材から指定できません。画面側で決まります |
| 接頭辞の長さ・文字種 | 1.1 の規則に限ります |

選べる範囲を広げたいときは、教材ごとにエンジンを直さず、拡張点の手続き（`docs/extension-point-procedure.md`）で汎用の名前つき拡張点として足します。

## 4. 関係する診断コード

| コード | 条件 | 検査する設定 |
|---|---|---|
| `E-PROFILE-PREFIX` | 接頭辞の規則に反する | `pack.profile` |
| `E-PROFILE-COMMAND-NAME` | 接頭辞なしで打てないコマンド名がある | `command_prefix: ""` |
| `E-PROFILE-TEXT` | 接頭辞なしの教材に、自然文の入力・規則がある | `command_prefix: ""` |
| `E-PROFILE-NO-INPUT` | 入力欄なしの教材に、コマンド・自然文の入力・規則がある | `input_field: hidden` |
| `E-PLAY-INPUT-CLASS` | 教材テストの `cmd:`・`text:` が画面で別の種類になる | 接頭辞が `/` 以外（`test` で出す） |
| `E-PARSE` | 型違い・未知の欄・名前にない値 | `pack.profile`・`pack.appearance` |

意味と条件の正は `docs/diagnostics.md` です。`E-PROFILE-*` は、`pack.profile` を既定から変えたときだけ検査します。

## 5. 根拠の試験

| 試験 | 確かめること |
|---|---|
| `crates/pack/tests/profile.rs` | 接頭辞の検査・空の接頭辞でのコマンド名の検査・IR に残ること・教材テストの入力の分け |
| `crates/pack/tests/profile_input.rs` | 接頭辞と入力欄に基づく Step の検査（`E-PROFILE-TEXT`・`E-PROFILE-NO-INPUT`） |
| `crates/pack/tests/effects.rs` の `a11y12_` | 動きの設定の鍵が最上位・`pack`・`profile`・`appearance`・Step・`processing[]`・形式 A のどこでも拒否されること（A11Y-12。TASK-378 2/2） |
| `crates/pack/tests/appearance.rs` | 外観 4 値の読み込み、名前にない値と未知の欄の `E-PARSE`、スタイル欄が 0 件であること |
| `crates/contract/tests/profile_contract.rs`・`appearance_contract.rs` | 閉じた集合・既定値・読み取りの規則 |
| `tools/browser/input-field-hidden.test.mjs`（PACK-20） | `input_field: hidden` の教材で入力欄が出ないこと |
