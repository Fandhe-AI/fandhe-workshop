# 教材の作成者向けの文書

教材（パック）を作る人向けの文書です。仕様は private のため本文を転記せず、ID（VER-11・VER-12・VER-14・D-43・D-51・D-52）で参照します。

版と保存済みの履歴の節（TASK-180）に続けて、主版を上げる条件と `pack.vocab` の書き方の規約（TASK-179）を書いています。

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
次の「主版を上げる条件」に書きます。

## 主版を上げる条件（`pack.version`）

`pack.version` は `主.副.修` の十進表記（例: `1.0.0`）で書きます。先頭に 0 を付けたり成分の数を過不足にしたりすると、
エンジンが版の記録を作れません。`validate` は、この形を検査しません（2026-10-19 時点）。YAML では文字列として読まれます。

主版を上げるのは、流れが変わる変更をしたときです（VER-11・D-52）。基準は「既存の履歴の入力の列を新しい版で適用し直したとき、
同じ状態にならない変更」です。次は例で、網羅ではありません。

- Step の追加・削除
- Step の順序・`next`・分岐の条件の変更
- 開始点（`starts`）の変更
- 入力・確認・規則の変更のうち、同じ入力で進み方や世界状態が変わるもの

主版を上げる理由は 3 つです。

- 主版が違う履歴は、履歴の位置によらず表示だけになります（`content_major`。VER-6）
- 旧版の成果物を残すのは、主版を上げたときだけです（D-52）。旧版で続けられる経路が残ります。残す範囲と版つき URL の形は、
  まだ決まっていません（VER-9）
- 上げ忘れて副版のまま Step を足すと、足した Step より先へ進んだ履歴は表示だけ（`replay_diverged`）になり、旧版も残りません
  （VER-5・D-52）。上げ忘れを警告する検査はありません（D-52）

流れが変わらない変更（文言・表示名・外観など、IR は変わっても適用し直した結果が変わらないもの）は、副版か修正版を上げます。
エンジンは副版と修正版を区別せず、どちらも適用し直して写しと一致すれば変換して開きます（VER-4）。使い分けは作成者に任せます
（決まりはありません）。

本文だけの修正は主版の判定に現れません（前の 2 節）。本文の版を示したいときは、副版か修正版を上げます（VER-12）。

## YAML の値の書き方（形式 B）

文字列の欄は、引用符なしで `true`・`no`・`1.10`・`010`・`null` と書いても、書いたとおりの文字列として読みます
（PACK-4）。`1.10` が `1.1` に、`010` が `10` や `8` に変わることはありません。

`do`・`body` のような省略できる欄に引用符なしの `null`・`~` を書くと、未設定ではなく、その字面の文字列になります。
未設定にしたいときは、欄そのものを書かないでください。

同じ表に同じキーを 2 回書くと `E-PARSE` で拒否します。引用符の有無だけが違うキー（`true:` と `'true':`）も同じキーです。
構文の誤りには、行と桁が付きます（複数文書の YAML は位置が付きません）。

数値の欄は 10 進で書いてください。`0x1f`・`0o17`・`0b11` のような 10 進でない整数は、どの欄でも `E-PARSE` で拒否します
（PACK-5。黙って 31 などに読み替えないためです）。形式 A（TOML）も同じです。形式 B で文字列として書きたいときは
`'0x1f'` のように引用符で囲んでください。診断には値を写さず、行と桁だけを示します。

形式 B では、アンカー（`&x`）・エイリアス（`*x`。マージキー `<<: *x` を含む）・タグ（`!t`・`!!str`・`!!int` など）・
複数文書（`---` で区切った 2 つ目以降の文書）も、どの欄でも `E-PARSE` で拒否します（PACK-5。YAML の処理系が黙って
展開・解決して、書いた内容と違う値になるのを避けるためです）。同じ値を何度も使いたいときはそのまま書くか、
モジュールを使ってください。文字列にしたいときは `!!str` ではなく引用符で囲んでください。1 つの文書の先頭の `---`
と末尾の `...` は書けます。形式 A（TOML）には、これらの記法がありません。

開始点（`starts`）は、全章を合わせて 1 件以上が必要です。0 件の教材パックは、`validate` の検査で `E-STRUCTURE` として
拒否します（PACK-25）。モジュールの `starts` は数えません。

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

## 左エリア（`left_pane`）

`pack.left_pane` で、左エリアに出す情報を絞れます（PACK-22）。2 項目とも省略でき、省略すると既定になります。

| 項目 | 値 | 意味 |
|---|---|---|
| `world_list` | `shown`（既定） | 世界の一覧を出す |
| | `hidden` | 世界の一覧を出さない |
| `view_tabs` | `all`（既定） | すべてのビューの切り替えを出す |
| | `declared` | Step が宣言したビューだけに絞る |
| | `hidden` | 切り替えを出さず、Step ごとに 1 つのビューだけを見せる |

```yaml
pack:
  left_pane: { world_list: hidden, view_tabs: declared }
```

名前にない値（`view_tabs: x` など）、文字列でない値、2 項目以外の欄は `E-PARSE` になります。このとき `E-LEFT-*` は検査しません。

設定を既定（`shown`・`all`）から変えたときだけ、設定と矛盾する Step を次の診断で知らせます。報告先は、その Step を定義した章・モジュールのファイルと項目 `step:<Step ID>` です。

| コード | 検査する設定 | 報告する場合 |
|---|---|---|
| `E-LEFT-MULTI-VIEW` | `view_tabs: hidden` | Step の `views` に 2 つ以上のビューを宣言している |
| `E-LEFT-TARGET-VIEW` | `view_tabs` が `all` 以外 | 操作の宛先（`input` の `ui_action`・`confirm` の `point_at` の `targets`、`rules[].on.ui`）のビューが、その Step の画面に出ない |
| `E-LEFT-TARGET-ROW` | 設定が既定でない（`world_list` だけの変更を含む） | 宛先の `path` が、教材のどの `put`・`replace` の `path` にも現れない |

- 画面に出るビューは、`views` で宣言した順に、種類（`kind`）ごとの最初の 1 つです。`hidden` では先頭の 1 つだけです。`views` を宣言しない Step では、ビューの ID の辞書順で最初の 1 つとして扱います（試作の仮置きで、決まった規則は後続のタスクで定めます）
- `E-LEFT-TARGET-ROW` は必要条件だけを調べる静的な近似です。経路がどこかで世界状態に置かれることしか確かめないため、画面でしか分からない矛盾は報告されません。宛先の経路は、`put`・`replace` で世界状態に置いてください。`put` の `path` に含めた `${...}` は、空文字列を含む任意の文字列として照合します
- 宛先の `view` や `path` に `${` を含む場合（モジュールの引数）は、展開前に決まらないため判定しません。`workshop.views` に無いビューは `E-REF-VIEW` が報告します
- 同じ宛先が複数あれば、同じ診断が重ねて出ます
- 診断の文面は固定です。ビューの ID や経路は、英数字・`_`・`.`・`-` だけで書かれているときに限って写します

## 文書のビューの本文（`views.<ID>.body`）

文書のビューが、練習データ（`fixtures/` のファイル）の本文を出すかどうかを、ビューごとに宣言できます（PACK-38・D-50・D-51。語彙 1.4、拡張点 EP-13）。

| 値 | 意味 |
|---|---|
| `hidden`（既定） | 題・属性・注記だけを出す（今までと同じ） |
| `shown` | 行の要素が持つ練習データの本文を、その行の本文として出す |

```yaml
views:
  v-doc: { kind: document, body: shown }
```

- 出す本文は、世界状態の要素の `fixture`（練習データの ID）で決まります。状態変化の `put`・`replace` で `fixture` を別の練習データの ID に替えると、出る本文も替わります。書き換えた後の本文は、あらかじめ別の練習データとして用意してください。新しい語は要りません
- `body` は `kind` を問わず読み、検査します。文書のビュー以外に書いても、画面は使いません
- 値は `shown`・`hidden` だけです。名前にない値（大文字を含む）と、文字列でない値（`null` を含む）は `E-PARSE`（項目 `view:<ID>`）になり、`compile` は IR を出しません。診断の文面は固定で、書いた値を写しません
- `body` を書く教材は、`pack.vocab` に `1.4` 以上を宣言してください。宣言が古いと `validate` が `E-VOCAB-MISMATCH` を報告します
- IR の型は変わりません。`body` を書かない教材の IR は、今までとバイト単位で同じです。本文の木は、版 2 の `fixture_docs` に宣言したすべての練習データが既に載っています
- 画面への表示は、画面のタスク（ART-18）が結線します。本節の時点では、宣言と、本文の木を引く関数（`contract::view_body::body_doc`）までが入っています
- 練習データの中の、選んだ値の差し込みは、この語では扱いません。足さないと決まっています（次の節）

## 選んだ値の差し込み（限界と回避）

受講者が選んだ値を、文中・経路・属性に差し込む語はありません。語を足さないことは D-50（2026-10-12）で決まっています（PACK-39）。

- 状態変化の経路と値は、書いた文字列そのものになります。`${名前}` は学習モジュールの引数をコンパイル時に置き換えるだけで、章の中では文字のまま残ります
- 受講者の入力の値は規則の照合にだけ使われ、世界状態には入りません。差し込みらしい欄（`value_from` など）を足しても動きません。`on_enter` などの状態変化では未知の欄として `E-PARSE` になります。`rules[].outcome.effects` では読み込みを通過し、実行時に警告（`Event::Warning`）となって入力が拒否されます
- 回避は、起こりうる入力を規則として列挙し、それぞれに固定の効果を書くことです。列挙にない値は `not_in_this_step` で拒否され、世界状態は変わりません
- 選択ごとに、属性へ固定の文面を書いて変化を見せる回避も使えます。本文を出す語（前の節）が入った後も、選んだ値を本文に差し込むことはできません。書き換えた本文は、別の練習データへの `replace` で表します
- 限界の詳しい内容と書き方の例は `docs/known-gaps.md` にあります

## 存在しない要素への `remove`・`set_attr`（`E-REF-ELEMENT`）

世界状態は空で始まり、要素を作るのは `put`・`replace` だけです。置かれていない経路への `remove`・`set_attr` は、
実行時に警告（`Event::Warning`）になり、教材テストが失敗します。`validate` は、その手前で次のように報告します
（PACK-30・D-43。診断コード一覧の版 2）。

- 教材のどこの `put`・`replace` も置かない経路を `remove`・`set_attr` が指すと、`E-REF-ELEMENT` です。
  `compile` は報告しません（`validate` だけの検査です）
- 「置く」に数えるのは、章・モジュールの全 Step の `on_enter`・`on_achieved`・`processing[].effects`・
  `rules[].outcome.effects` と、`starts[].effects` の `put`・`replace` です。`when` つきの `put` も数えます
- `put` の経路に `${...}` があれば、空文字列を含む任意の文字列として照合します
- モジュールの引数を含む `remove`・`set_attr` の経路は、引数を置き換えた後の Step で判定します。診断は Step を
  定義したモジュールのファイルに付きます
- `replace` は報告しません（要素が無ければ置くため）
- 診断は `on_enter[0]: remove target path ... is not placed by any put or replace` の形です。経路は、英数字・
  `_`・`.`・`-` だけで書かれているときに限って写します。属性の値は写しません

必要条件だけを調べる近似です。次は報告しないので、教材テストの模範解答と、実行時の警告で補ってください。

- 順序に依る不在: `put` より前の `remove`、同じ経路への 2 度目の `remove`
- `put` が通らない枝（条件・入力の違い）で置かれる要素への `set_attr`

## `pack.vocab` の書き方

`pack.vocab` は必ず書きます（VER-11）。省略すると、履歴には `1.0` と記録され、`validate` の一致の検査も働きません（PACK-31）。

- 値は、使っている語の中で最も新しい語彙の版以上を、`主.副` で書きます（例: `"1.4"`）。YAML では引用符で囲みます
- 現在のエンジンが読める語彙の上限は 1.4 です（`docs/vocabulary/vocabulary.md` の版と拡張点の対応表）。上限を超える宣言の
  履歴は開きません（`vocab_newer`。VER-7）。`validate` は、上限を超える宣言を報告しません（2026-10-19 時点）
- 語彙 1.1 の語は、`validate` がまだ検査しません。作成者が対応表で確かめます。`fandhe-pack vocab` は EP-1〜EP-8 と EP-13 だけを Step ごとに数え、EP-9 と `label` は数えません
  （AUTHOR-17）
- `fandhe-pack vocab <pack-dir>` は、Step ごとの拡張点の使用と、既存の語彙の組み合わせだけで書けた Step の数を出します（AUTHOR-9。出力の形は
  `docs/cli.md` の 11 節）。EP-7 は Step の `do` だけ、EP-8 は Step の `body_levels` とヒントの `levels` だけを数え、教材パック全体の
  `guidance_levels` と開始点の `guidance` は数えません。`pack.profile`・`pack.appearance`・`pack.left_pane` は、既定から変えたときだけ末尾に出し、Step ごとの数に入れません。
  読み込みか `compile` に診断があるときは表を出しません。`pack.vocab` に何を宣言するかを確かめるときは、先に `compile` を通します
- 語彙の版を上げるだけの変更は流れを変えないため、主版ではなく副版か修正版の扱いです。語彙が古い履歴は、適用し直して一致すれば
  変換して開きます（VER-4）

宣言と使用の一致の検査は、次の節に書きます。

## `pack.vocab` の宣言と使用の一致（`E-VOCAB-MISMATCH`）

宣言より新しい語彙を使う教材は、履歴の「語彙の軸」が宣言に頼るため、版違いの履歴の理由を特定できません。
`validate` は、`pack.vocab` を書いた教材について、次のように報告します（PACK-31・VER-14・D-43。診断コード一覧の版 5）。

- 宣言より新しい版の拡張点を使うと、拡張点ごとに 1 件の `E-VOCAB-MISMATCH` です。`compile` は報告しません
  （`validate` だけの検査です）
- 対象は EP-9（`starts[].requires`・`unmet`。語彙 1.2）と、EP-10〜EP-12（`pack.profile`・`pack.appearance`・
  `pack.left_pane`。語彙 1.3）と、EP-13（`views.<ID>.body`。語彙 1.4）と、確認 `answer` の選択肢の `label`（`confirm.options[].label`。語彙 1.3）です。
  値が既定と同じでも、欄を書けば使用です。呼ばれないモジュールの開始点・Step も数えます。何か所で使っても 1 件です
- 入力方式 `choice` の `input.options[].label`（語彙 v1）は別の欄で、対象ではありません。`confirm` の側だけを見ます
- `pack.vocab` を省略した教材は検査しません。書き方の規約は別に定めます
- `pack.vocab` が文字列でない、または `主.副` の形でない（`1.2.0`・`v1.2`・`01.2` など）ときは `E-PARSE` です。
  YAML では `1.2` を引用符で囲んでください。値は診断に写しません
- 診断は `EP-9 (starts.requires) needs vocab 1.2 or later (declared 1.0)` や `confirm.options[].label needs vocab 1.3 or later (declared 1.2)` の形で、file は定義ファイル、item は `pack:vocab` です。`label` の値は写しません
- 語彙 1.1 の拡張点は、まだ検査しません

## 本文の行の途中の改行

本文（Markdown）を 1 段落 1 行で書く必要はありません（PACK-27・D-51）。

- 前後がともに日本語（かな・漢字・全角記号）の行末の改行は、連結されます。文の途中に空白は入りません
- 英数字が接する改行（`hello` と `world` など）や、日本語と英数字の間の改行は残り、画面では空白として描かれます
- コード・強調・リンクの直前や直後の改行は、連結されません
- 段落を分けるには空行を、強制改行には行末の空白 2 つか `\` を使います

## `validate` が見つけない欠陥

`validate` は、次の 3 つを報告しません（PACK-30・D-43 で、検出できない限界として受け入れた範囲です）。

- 条件の充足可能性: 決して真にならない `when`（例: ある旗とその否定を `all` で並べた条件）の先の Step。静的な辺は
  あるので、到達できるものとして扱います
- Step の巡回: 無条件の `next` で前の Step に戻り、`final` に着かない巡回。モジュールの呼び出しの巡回
  （`E-MODULE-CYCLE`）だけは報告します
- 規則の遮蔽: 先の規則が後の規則を隠し、後ろの規則が決して選ばれないこと。規則は上から照合し、最初に一致した
  ものの `outcome` を使います。たとえば引数の条件を持たない `make-item` の規則の後ろに、`name` の値を指定した
  `make-item` の規則を置くと、後ろの規則は使われません。規則は具体的なものから先に書いてください

補うには、教材テストの `playthroughs` に、すべての開始点と、条件で分かれるそれぞれの枝を通る模範解答を書き、
各枝が `final` まで進むことを確かめてください。遮蔽は、模範解答と誤入力で、規則ごとに意図した結果になる入力を
通して確かめます。

## 新しい表現が要るとき（拡張点の追加）

既存の語の組み合わせで書けない表現が要るときは、教材ごとにエンジンを直さず、汎用の名前つき拡張点として足します。
教材パックからは名前の欄を書いて使うだけで、任意のコードは書けません。実行時に拡張点を登録する仕組みもありません。
手順は `docs/extension-point-procedure.md` にあります。

## IR の出力と配信物（`compile`）

`fandhe-pack compile <pack-dir> -o <file>` が書く IR は、画面に配る IR そのものです。標準出力に出す IR と `-o` のファイルは同じバイト列で、`-o` を付けたときは標準出力に何も出ません。

- 同じ内容の教材パックは、何度 compile しても、別のディレクトリへ写しても、同じバイト列の IR になります。IR にパス・時刻・乱数は入りません
- 診断が 1 件でもあるときは、ファイルを作りません。同じ名前のファイルが既にあっても書き換えません
- 書き出せないとき（親のディレクトリが無い・既存のディレクトリを指すなど）は、`error:` の 1 行を標準エラー出力に出して終了コード 2 で終わります。一時ファイルへ書いてから置き換えるので、途中まで書いた IR は残りません
- 配信物には `IR_FILE=<file> tools/build-web.sh` で入ります（`ir/main.json`）。配信の実装は TASK-109・TASK-121 の側です
- 外部仕様は `docs/cli.md` の 10 節（AUTHOR-8）です

## 根拠となる試験

| 試験 | 確かめること |
|---|---|
| `crates/contract/tests/content_hash_contract.rs` | 本文の木を除いて求めること、本文だけの修正でハッシュが変わらないこと |
| `crates/core/tests/version_record.rs` の `body_only_edit_yields_the_same_version_record` | 本文だけを直した IR から作る版の記録が、全欄で同じになること |
| `crates/contract/tests/fixed_histories.rs` | 固定の履歴 4 本の版の記録が全軸で同一で、期待値を D-51 に合わせて変えていないこと |
| `crates/pack/tests/format_b.rs` の `pack4_` | YAML の値の文字列としての読み取り・重複キーの拒否・構文エラーの行と桁（PACK-4） |
| `crates/pack/tests/yaml_features.rs` の `pack5_` | アンカー・エイリアス・マージキー・タグ・複数文書をどの欄でも `E-PARSE` で拒否し、名前・字面を写さないこと（PACK-5） |
| `crates/pack/tests/no_starts.rs` の `pack25_` | 開始点が 0 件のパックを `validate` の段階で `E-STRUCTURE` として拒否すること（PACK-25） |
| `crates/pack/tests/radix_literals.rs` の `pack5_` | 10 進でない整数（`0x1f` 等）をどの位置でも両形式で `E-PARSE` で拒否し、値を写さないこと（PACK-5） |
| `crates/pack/tests/static_limits.rs` の `pack30_` | 条件の充足可能性・Step の巡回・規則の遮蔽を含む限界パックを `validate` が報告しないこと（PACK-30・D-43） |
| `crates/pack/tests/missing_elements.rs` の `pack30_` | 置かれない経路への `remove`・`set_attr` が `E-REF-ELEMENT` になること、置かれる経路（`put`・`replace`・`${}` つきの `put`・開始点）は報告しないこと、経路以外の値を写さないこと（PACK-30・D-43） |
| `crates/pack/tests/chosen_value_gap.rs` の `pack39_` | 選んだ値の差し込みの語が無いこと（差し込みらしい欄が `E-PARSE`、受講者の値の枠が `E-SLOT-UNKNOWN`、章の `${}` が文字のまま、値が世界状態に入らない）と、列挙・固定値の属性の回避が書けて、列挙にない値が拒否されること（PACK-39・D-50） |
| `crates/core/tests/version_histories.rs` の `fifteen_histories_match_the_fixed_expectations` | 副版で Step を足すと先へ進んだ履歴が `replay_diverged`・手前の履歴が変換になること、主版が違えば位置によらず `content_major` で表示だけになること（VER-5・VER-6・VER-18） |
| `crates/core/tests/version_record.rs` の `t2_default_vocab_record`・`t6_fail_closed` | `pack.vocab` の省略が `1.0` で記録されること、読めない `pack.version` で記録を作らないこと |
| `crates/pack/tests/vocab_declaration.rs` の `pack31_` | 宣言より新しい拡張点（EP-9〜EP-13）と確認の選択肢の `label` の使用が語ごとに 1 件の `E-VOCAB-MISMATCH` になること、入力方式 `choice` の `label` を取り違えないこと、宣言と使用が一致する教材で誤検出が 0 件であること、一致する宣言と `pack.vocab` の省略は報告しないこと、読めない宣言が値を写さない `E-PARSE` になること（PACK-31・D-43） |
| `crates/pack/tests/soft_break_join.rs` の `pack27_` | 前後が CJK の行の途中の改行だけが連結され、英数字・混在・インラインの境界・強制改行は変わらないこと（PACK-27・D-51） |
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
| `crates/pack/tests/left_pane.rs` | 左エリアの設定の読み込みと `E-PARSE`、`E-LEFT-MULTI-VIEW`・`E-LEFT-TARGET-VIEW`・`E-LEFT-TARGET-ROW` が期待のファイル・項目で出ること、既定では出ないこと、`compile`・`validate`・TOML・CLI の一致（PACK-22） |
| `crates/contract/tests/left_pane_contract.rs` | 左エリアの 2 項目の閉じた集合、既定値、名前にない値・型違い・未知の欄の拒否（PACK-22） |
| `crates/pack/tests/view_body.rs` | 文書ビューの本文の宣言（`body: shown`）が IR に載り本文の木が引けること、`replace` で本文が替わること、省略と `hidden` で本文が出ないこと、不正な値が `E-PARSE` で `compile` が IR を出さないこと、語彙 1.3 の宣言で `E-VOCAB-MISMATCH` になること（PACK-38） |
| `crates/contract/tests/view_body_contract.rs` | `body` の閉じた集合・既定・型違いと未知の綴りの拒否・`body_doc` が `None` に倒れる場合（PACK-38） |
| `crates/pack/tests/compile_output.rs` の `author8_` | `compile` の標準出力と `-o` が同じバイト列であること、複数回・別ディレクトリの写しでも同一であること、末尾の形、診断があるとき書かないこと、書き出せないとき 2 になること（AUTHOR-8） |
