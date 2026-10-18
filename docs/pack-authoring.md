# 教材の作成者向けの文書

教材（パック）を作る人向けの文書です。仕様は private のため本文を転記せず、ID（VER-11・VER-12・D-51・D-52）で参照します。

現時点で書いているのは、版と保存済みの履歴の節だけです（TASK-180 で追加）。主版（`pack.version` の主版）を上げる条件と
`pack.vocab` の書き方の規約は、TASK-179（#453）がこの文書に書き足します。それまでは未整備です。

## 教材の版と保存済みの履歴

保存済みの履歴は、4 軸の版（エンジン・教材形式・教材内容・参照元）を記録します。教材内容の軸は、`pack.version` と
`content_hash` の組です。

`content_hash` は、IR から本文の木（`body_doc`・`body_levels_doc`）だけを除いて求めます（D-51）。求め方の実装は
`crates/contract/src/content_hash.rs` にあります。

## 本文だけの修正は `content_hash` を変えない

本文だけを直した教材は、`content_hash` が変わりません。保存済みの履歴から見ると同じ版のままで、受講中の画面の本文だけが
新しくなります。

本文以外の欄（例: 選択肢の `label`）を足すと IR が変わり、`content_hash` も変わります。

## 主版を上げ忘れた本文の修正は判定に現れない

主版を上げ忘れて本文だけを直すと、その修正は履歴の版の判定に現れません（VER-12）。エンジンは、本文の修正を履歴の版の違いとして
検出できません。

本文の版は `pack.version` で示す運用で補います（VER-11）。上げ忘れを警告する検査はありません（D-52）。主版を上げる判断の基準は、
TASK-179 でこの文書に書き足します。

## YAML の値の書き方（形式 B）

文字列の欄は、引用符なしで `true`・`no`・`1.10`・`010`・`null` と書いても、書いたとおりの文字列として読みます
（PACK-4）。`1.10` が `1.1` に、`010` が `10` や `8` に変わることはありません。

`do`・`body` のような省略できる欄に引用符なしの `null`・`~` を書くと、未設定ではなく、その字面の文字列になります。
未設定にしたいときは、欄そのものを書かないでください。

同じ表に同じキーを 2 回書くと `E-PARSE` で拒否します。引用符の有無だけが違うキー（`true:` と `'true':`）も同じキーです。
構文の誤りには、行と桁が付きます（複数文書の YAML は位置が付きません）。

`commands`・`views` など、語彙の型付けが入るまで自由な値として通す欄の中の引用符なしの値は、文字列にならず
YAML の型のままです。重複キーの拒否だけが及びます。

## 学習モジュールの呼び出し（`use`）

共通の流れを `modules/<名前>/module.yaml` に 1 か所だけ書き、章の Step から `use` で呼ぶと、呼んだ数だけ実体になります（PACK-19）。

```yaml
- id: r1-impl
  title: 1 周目の実装
  use:
    module: issue-to-merge   # モジュールの ID
    instance: round1         # 実体の名前（同じ呼び出し元の中で重複させない）
    args: { root: "42" }     # params の既定値に重ねる
```

- 実体の ID は `<モジュール ID>@<実体名>`、その Step の実体は `<モジュール ID>@<実体名>/<Step ID>` です。入れ子の実体は `<親の実体 ID>/<モジュール ID>@<実体名>` になります。
- モジュールの定義を 1 か所直すと、そのモジュールを使うすべての実体に反映されます。3 周目の実体は、章の定義ファイルを 1 つ足すだけで書けます（既存のファイルは変えません）。
- モジュールの文字列の `${名前}` は、コンパイル時に `args`（なければ `params` の既定値）で 1 回だけ置き換えます。差し込んだ値の中の `${` は、もう一度は置き換えません。`${` を字面のまま書く方法はありません。
- 置き換えない欄は、`id`・`body`・`body_levels`・`next[].to` と、連想配列のキーです。章（`main`）は引数を持たないので、`${` は字面のまま残ります。
- `args` の値は文字列で書きます。`root: 42` のような数値・真偽値は `E-PARSE` になるため、引用符で囲んでください。
- `module` と `instance` は `[A-Za-z0-9_.-]` の短い識別子にします（`@`・`/` は使えません）。
- `use` を持つ Step には `input` を書けません。同時に書くと `E-STRUCTURE` です（章の Step・モジュールの中の Step のどちらも）。
- モジュールの `starts` は、実体ごとに `<実体 ID>#<開始 ID>` として展開します。置き換えるのは `effects` と `requires` だけです。
- 展開中の経路にあるモジュールを再び呼ぶ巡回（自分を呼ぶ場合を含む）は `E-MODULE-CYCLE` です。巡回を閉じる `use` の Step に `a → b → a` の形の経路つきで付きます。モジュール名が `${名前}` に由来する場合は、引数の値を出さないため経路は付きません。
- 入れ子は 8 段までです。9 段目の呼び出しは展開せず `E-MODULE-CYCLE` にします。これは巡回とは別の検査で、巡回しない深い入れ子にだけ効きます。未定義のモジュールは `E-REF-MODULE`、未定義の `${名前}` は `E-STRUCTURE` です。
- 呼ばれないモジュールは IR に出ません。
- モジュールの `input`・`confirm` が `${名前}` を含む場合は、置き換えた後の宣言を、入力方式・確認・操作プロファイル（`E-PROFILE-TEXT`・`E-PROFILE-NO-INPUT`）の規則で検査します。置き換える前の宣言は検査しないので、`kind: "${k}"` のように書けます。診断はモジュールの定義ファイルと Step で示し、引数の値は含めません。

## 入力方式（`input`）

Step の `input` は、`kind` で 5 方式と待機（`wait`）から選びます（PACK-10）。ここに書けるのは次の欄だけで、未知の欄・
未知の `kind` は `E-PARSE`、意味の制約に反する宣言は `E-STRUCTURE` になります。

| `kind` | 欄 | 意味の制約 |
|---|---|---|
| `command` | `name`、`args`（各要素は `name`・`positional`・`required`・`shape`） | 名前は空でなく、引数名は重複しない。`shape` は `kebab_case`・`digits`・`any`（既定は `any`） |
| `text` | `min`・`max`（0 以上の整数。どちらも必須） | `min` は `max` 以下 |
| `choice` | `options`（各要素は `id`・`label`） | 1 件以上で、`id` は重複しない |
| `ui_action` | `targets`（各要素は `view`・`path`） | 1 件以上で、どれも空でない |
| `approval` | なし | 欄を足すと `E-PARSE` |
| `wait`（EP-1） | なし | 欄を足すと `E-PARSE`。Step の `processing` が 1 件以上でないと `E-STRUCTURE` |

`text` が判定するのは、空でないことと文字数だけです。内容の良し悪しを判定する欄は書けません（`contains` や
`pattern` は `E-PARSE`）。受け付けた入力の分類は `free_text_unjudged`、範囲外は `bad_arg_shape` で拒否します。

- 文字数はコードポイントで数えます（絵文字は 1 文字）
- 前後の空白は数えません。空白だけの入力は空として常に拒否します（`min: 0` でも同じです）。この扱いは、spec が
  「空でないこと」としか書いていないことからの推定による決定です（2026-10-15）
- 下限・上限はどちらも含みます

`wait` は受講者の入力を要さず、`processing` の進行の完了で受付になります。`processing` が無い待機は先へ進めない
ため拒否します。この決定は、spec に明文がなく EP-1 の説明からの推定によります（2026-10-15。spec 側の確認が望ましい）。
`processing` があって `wait` でない Step（`command` との併用など）は書けます。`processing` の欄は「処理中の進行（`processing`）」の節を見てください。

取り消し（EP-2）は `input.kind` ではなく、規則の `on: { cancel: true }` で受けます。`kind: cancel` は未知の `kind` として
`E-PARSE` です。どの入力方式の Step にも書け、意味を持つのはその規則を書いた Step だけです。`on.cancel` の値が真偽値でなければ
`E-PARSE`、`false` は `E-STRUCTURE` です。`on` の他のキーとの組み合わせや入力方式との整合は、ここでは検査しません。

## 確認（`confirm`）

入力を持つ Step は、受け付けただけでは達成にならず、受講者の確認の操作で初めて達成になります（PACK-13、R-22）。
確認を省けるのは、理由を添えて `kind: none` と明示した Step だけです。入力を持つ Step が確認も理由つきの省略も持たない場合は
`E-NO-CONFIRM` で拒否します（`wait` の Step も入力を持つ Step です）。`use` を持つ Step は入力を持たないため、確認の検査の対象外です（`input` との同時指定は `E-STRUCTURE`）。
モジュールの `confirm` が `${名前}` を含む場合は、引数で置き換えた後の宣言を検査します（`reason: "${why}"` に空文字を渡すと `E-NO-CONFIRM`）。

| `kind` | 欄 | 意味の制約 |
|---|---|---|
| `point_at` | `targets`（各要素は `view`・`path`）、`on_miss`（任意） | `targets` は 1 件以上で、どれも空でない |
| `answer` | `options`（各要素は `id`・`correct`、任意で `on_wrong`・`label`）、`on_wrong` | 1 件以上で、`id` は空でなく重複しない。`correct: true` が 1 件以上。`on_wrong` は空でない。`label` と選択肢の `on_wrong` は書くなら空でなく、選択肢の `on_wrong` は `correct: true` の選択肢に書けない |
| `approve` | なし | 欄を足すと `E-PARSE` |
| `none` | `reason` | `reason` が前後の空白を除いて空でないこと。無い・空・空白だけは `E-NO-CONFIRM` |

- 未知の欄・未知の `kind`・欄の欠け・型の違いは `E-PARSE`、意味の制約の違反は `E-STRUCTURE` です
- `none` の理由の欠けを `E-PARSE` でなく `E-NO-CONFIRM` にするのは、PACK-13 の「理由つき」の違反として扱うためです（2026-10-15 の決定）
- 入力の無い Step に `confirm` を書いても、形が正しければ診断しません。該当する診断コードが spec に無いためで、spec 側の判断が要ります（2026-10-15）
- `answer` の選択肢の `on_wrong`（EP-6）は、その選択肢を選んだ誤答に返す文面です。選択肢が持たなければ Step の `on_wrong` を返します。正答の選択肢に書くと `E-STRUCTURE`、書くなら空白だけにはできません（2026-10-15 の決定。契約の版を 3 に上げました）
- `answer` の選択肢の `label`（v1.3）は表示名です。画面の選択肢と、操作のログの「選択: …」に同じ表示名が出ます。省略すると ID がそのまま出ます。前後の空白を除いて空になる `label` は `E-STRUCTURE` で拒否します（2026-10-15 の決定。契約の版を 2 に上げました）
- 履歴に残る値は選択肢の ID で、表示名ではありません。`label` を後で直しても、記録した履歴の再生は変わりません
- 確認の `options[].label` は、入力方式 `choice` の `options[].label`（必須）とは別の欄です。表示名の解決は 1 つの Step の確認の選択肢の中だけで行い、入力方式 `choice` の `label` へは落ちません
- `on_miss`・`on_wrong`（Step・選択肢とも）の参照先が `rejections` に無ければ `validate` が `E-REF-REJECTION` にします。`targets[].view` は `E-REF-VIEW` です

## 処理中の進行（`processing`）

`processing` の要素に書ける欄は、`name`（空でない文字列）と `effects`（効果の列）だけです。未知の欄は `E-PARSE`、空の名前は
`E-STRUCTURE` で拒否します。

**進行の間隔は宣言できません。** `processing` の要素にも、Step にも、`pack:` にも、間隔の欄はありません。書くと `E-PARSE`
になります（PACK-44、D-43）。間隔はエンジンのビルド時の定数（web crate の `tick_interval`）で、300ms です（D-47、EVT-18）。配信する設定では、教材からも URL からも
変えられません。早送りの扱いはこの節の範囲外で、早送りの項目に任せます。

処理中に受講者が Esc を押せる長さが要る Step は、進行の件数を増やして長さを稼いでください。長さはおおよそ「件数 × 300ms」で、
20 件ならおよそ 6 秒です（ST-1.5 の規模）。効果を持たない進行（`name` だけ）を足して構いません。

```yaml
processing:
  - {name: scan, effects: [{set_flag: scanning}]}
  - {name: wait-1}
  - {name: wait-2}
  - {name: wait-3}
```

処理中の Esc は、間隔や件数によらず常にエンジンの停止です（D-22）。人が実際に押せる長さかどうかの画面での確認は
TERM-34 の担当で、2026-10-15 時点では済んでいません。

## 拒否の分類（`category`・`defaults`・`on_reject`）

拒否の分類は 8 語（`unknown_command`・`wrong_order`・`missing_arg`・`bad_arg_shape`・`busy`・`not_in_this_step`・`not_supported`・`free_text_unjudged`）で、
型の正は contract の `judgment` です（PACK-11）。次の 3 か所の分類の語は、この 8 語で読めなければ `E-PARSE` で拒否します。

- `workshop.*` の `rejections.<id>.category`（欠け・文字列でない・語彙にない語）
- `workshop.*` の `defaults` のキー
- 章・モジュールの Step の `on_reject` のキー

- 大文字小文字や前後の空白の揺れは受けません
- `free_text_unjudged` は受付に付く語で、3 か所とも書けます
- 診断には、識別子として安全な語だけを写し、値そのものは写しません
- `defaults` が 8 分類を網羅すること、キーと参照先の文面の `category` の一致は、この検査では見ません。診断コードの条件に無いためです。`next_action` の空と差し込みの枠は、次の節の検査が見ます（2026-10-15）

## 差し込みの枠（`{{...}}`）と次の行動

拒否・問い返しの文面には、受講者が次にすべきこと（次の行動）が必ず要ります（R-21、PACK-12）。次の行動を書く欄は、拒否（`rejections`）では
`next_action`、問い返し（`clarifications`）では `how_to_answer` です。文面の `message` と次の行動の欄には、値を差し込む枠を書けます（語彙 v1.1 の EP-7、PACK-16）。

枠は `{{名前}}` か `{{名前|代替の文}}` の形です。名前は次の 6 語に限られ、式・スクリプト・入れ子は書けません。

| 名前 | 差し込む値 | 空になりうるか | 使える文面 |
|---|---|---|---|
| `input_head` | 受講者の入力の先頭の語 | なりうる（代替の文が要る） | すべて |
| `nearest_command` | 近いコマンド | なりうる（代替の文が要る） | 分類が `unknown_command` の拒否だけ |
| `step_title` | 今の Step の `title` | ならない | すべて |
| `step_do` | 今の Step の `do` | ならない（Step に `do` が要る） | すべて |
| `remaining` | 処理中の残りの回数 | ならない | 分類が `busy` の拒否だけ |
| `total` | 処理中の全体の回数 | ならない | 分類が `busy` の拒否だけ |

- 分類が限られた枠は、分類を持たない問い返しの文面では使えません
- 代替の文は、値が無い（または空白だけの）ときに使われます。空になりうる枠に代替の文が無い、または空白だけの場合は拒否します
- 差し込んだ値の中の `{{...}}` は展開されません。受講者の入力に枠の形が混ざっても、そのまま文字として表示されます
- 枠を含む値は、YAML で必ず引用符で囲みます。flow 形式（`{...}`）の中で、引用符なしの `{{` は `E-PARSE` になります（AUTHOR-13）

| 診断 | 条件 | ファイル・項目 |
|---|---|---|
| `E-SLOT-SYNTAX` | `{{` が閉じていない・名前が空・枠が入れ子になっている | `workshop.*`・`rejection:<id>` または `clarification:<id>` |
| `E-SLOT-UNKNOWN` | 名前が 6 語にない | 同上 |
| `E-SLOT-CONTEXT` | その分類の文面では使えない枠 | 同上 |
| `E-SLOT-NO-FALLBACK` | 空になりうる枠に代替の文が無い | 同上 |
| `E-SLOT-NEEDS-DO` | `step_do` を使う文面が出うる Step に `do` が無い（空白だけを含む） | その Step を定義した章・モジュールのファイル・`step:<id>` |
| `E-EMPTY-NEXT-ACTION` | 次の行動の欄が空、または空になりうる枠を値なしで差し込むと空になる | `workshop.*`・`rejection:<id>` または `clarification:<id>` |

- `step_do` の検査で「出うる文面」は、Step の `on_reject`、入力を持つ Step では `on_reject` に無い分類の `defaults`、規則の `reject`・`clarify`、確認の `on_wrong`・`on_miss`・選択肢の `on_wrong` が指す文面です
- 欄が無い・文字列でない場合は `E-PARSE` です。診断には、本文・代替の文の値を写しません
- `validate` と `compile` の両方がこの検査を行います。枠に誤りのある教材は IR を出しません
- モジュールの引数の展開の後で `do` が空になる場合は、静的には見えません。展開後の IR を総なめにする `slot_check::tally_empty_next_actions` が数えます。`fandhe-pack test` への結線は TASK-404 の担当です。実行時の値の組み立てと差し込みは core の `text_fill`（TASK-274）が持ち、2026-10-16 時点では、結合試験（文面の確認 38 件と、差し込みの後で次の行動が空の拒否が 0 件であることの確認）までです

### 実行時の値の規則

- `input_head` と `nearest_command` には、教材の接頭辞（`profile.command_prefix`）が付きます。`/` を決め打ちにしません
- `input_head` は、コマンドとして読めた入力だけに値があります。自然文・選択・画面操作・承認・取り消しでは値なしになり、代替の文が使われます
- `nearest_command` は、分類が `unknown_command` のときだけ計算します。コマンド表から、編集距離（隣り合う 2 文字の入れ替えを 1 と数えます）が 2 以下で最小のものを選び、`unsupported` は選びません。同じ距離なら名前の辞書順の先頭です
- `step_do` は Step の `do` だけを使い、無いときに題名で代えません。`remaining` は残りの回数、`total` は全体の回数です

## 教材テストの誤入力の期待（`expect`）

教材テストの `error_inputs[].input.expect` は、誤入力に対する判定の結果を書く欄です（PACK-11）。書き方は 3 つの結果（4 通りの書き方）に分かれます。

| 書き方 | 意味 |
|---|---|
| `accepted` | 受付 |
| `accepted:free_text_unjudged` | 自然文の受付（内容は判定しない注記が付く） |
| `rejected:<分類>` | 拒否。分類は `unknown_command`・`wrong_order`・`missing_arg`・`bad_arg_shape`・`busy`・`not_in_this_step`・`not_supported` の 7 語 |
| `clarified` | 問い返し |

- `rejected` は分類が必須です。分類を省くと別の拒否でも通ってしまい、誤入力の分類を確かめるという目的が果たせないためです
- `rejected:free_text_unjudged` は書けません。`free_text_unjudged` は受付に付く分類で、拒否ではありません
- `accepted` に付けられる分類は `free_text_unjudged` だけで、`clarified` には分類を付けません
- `confirmed`・`progressed`・`ignored` は語彙ではないため書けません。大文字小文字や前後の空白の揺れも受けません
- 読めない `expect` は `E-PARSE` で拒否します。診断には `expect` の値を写しません
- 読むのは `error_inputs[].input.expect` だけです。`pre[].expect`・`playthroughs` の期待は `fandhe-pack test`（TASK-404）が読みます
- エンジンで実際に判定して期待と比べる実行は、入力の判定の実装（TASK-267）と TASK-404 の後になります（2026-10-15 時点）。いまの試験は、期待の読み取りと、拒否の文面・次の行動が空でないことの静的な確認までです

## 外観（`appearance`）

`pack.appearance.theme` で画面の外観を選べます（PACK-21）。値は次の 4 つに限られ、省略すると `standard` です。

| `theme` | 意味 |
|---|---|
| `standard` | 標準（既定） |
| `mono` | モノクロ |
| `high_contrast` | 高コントラスト |
| `reading` | 読みやすさ重視 |

```yaml
pack:
  appearance: { theme: reading }
```

- 外観は見た目だけを変えます。入力の判定・状態変化・遷移・履歴には影響しません
- 名前にない値（`dark` など）や、`theme` 以外の欄は `E-PARSE` になります。色・フォント・CSS を指定する欄はありません
- 診断には書いた値を写しません

## 根拠となる試験

| 試験 | 確かめること |
|---|---|
| `crates/contract/tests/content_hash_contract.rs` | 本文の木を除いて求めること、本文だけの修正でハッシュが変わらないこと |
| `crates/core/tests/version_record.rs` の `body_only_edit_yields_the_same_version_record` | 本文だけを直した IR から作る版の記録が、全欄で同じになること |
| `crates/contract/tests/fixed_histories.rs` | 固定の履歴 4 本の版の記録が全軸で同一で、期待値を D-51 に合わせて変えていないこと |
| `crates/pack/tests/format_b.rs` の `pack4_` | YAML の値の文字列としての読み取り・重複キーの拒否・構文エラーの行と桁（PACK-4） |
| `crates/pack/tests/modules.rs` | 2 つの実体が別の Step の実体になること・1 か所の修正が両方に反映されること・3 周目をファイルの追加だけで書けること・巡回の検出・入れ子の上限と不正な呼び出し（PACK-19） |
| `crates/pack/tests/input_vocabulary.rs` の `pack10_` | 5 方式と `wait`・`cancel` の宣言が検証を通ること、`text` に内容を判定する欄が書けないこと、不正な宣言の診断と値を出さないこと（PACK-10） |
| `crates/contract/tests/input_spec_contract.rs` | `text` の空・下限・上限・コードポイントの数え方と、各方式の意味の制約（PACK-10） |
| `crates/pack/tests/confirm_vocabulary.rs` の `pack13_` | 確認 4 種が検証を通ること、確認の無い入力つき Step と理由の無い `none` が `E-NO-CONFIRM` になること、検証を通った教材に確認なしで達成になりうる Step が 0 件であること（PACK-13） |
| `crates/contract/tests/confirm_spec_contract.rs` | 確認 4 種の型の形・意味の制約と、受付と達成の分け方（`none` だけが受付で達成になる。R-22）。`pack23_` は選択肢の表示名の解決・ログの文面・履歴に残す値が ID であること・空白だけの `label` の拒否（PACK-23）。`pack15_` は選択肢の `on_wrong` の規則（契約の版 3） |
| `crates/pack/tests/confirm_vocabulary.rs` の `pack23_` | `label` が IR に残ること、IR から読んだ表示名とログの文面、履歴に ID だけが残ること、空白だけの `label` の `E-STRUCTURE`（PACK-23） |
| `crates/pack/tests/confirm_on_wrong.rs` の `pack15_` | 正答の選択肢の `on_wrong` が `E-STRUCTURE`（モジュールの置換後も）、存在しない文面が `E-REF-REJECTION`、誤答の文面が選択肢、無ければ Step の順で決まること（PACK-15） |
| `crates/pack/tests/effects.rs` の `pack44_` | 進行の間隔の欄がどの位置でも `E-PARSE` で拒否され値を出さないこと、件数を増やす回避が検証を通ること（PACK-44） |
| `crates/pack/tests/judgment_vocabulary.rs` の `pack11_` | 語彙の `result`・`rejection` の語が contract の型と一致すること、`category`・`defaults`・`on_reject` の語彙にない語が `E-PARSE` になり値を出さないこと（PACK-11） |
| `crates/pack/tests/error_input_expect.rs` の `pack11_` | 誤入力の期待の 3 形の読み取りと不正な書き方の拒否、架空の教材の誤入力 20 件の分類、3 場面の区別、拒否の文面と次の行動が空でないこと（PACK-11） |
| `crates/pack/tests/text_slots.rs` の `pack16_`・`pack12_` | 5 つの `E-SLOT-*` が期待のファイル・項目で出ること、次の行動の空が静的・動的の両方で 0 件になること、公開の試験データで診断が 0 件であること（PACK-12、PACK-16） |
| `crates/contract/tests/slot_contract.rs` の `pack16_` | 枠の構文・差し込みの再走査をしない性質・エラーの固定文（PACK-16） |
| `crates/pack/tests/appearance.rs` | 外観 4 値の読み込み、名前にない値と未知の欄の `E-PARSE`、`validate` との一致、TOML・CLI、スタイル欄が 0 件であること（PACK-21） |
