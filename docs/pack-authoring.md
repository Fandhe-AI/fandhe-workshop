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
- モジュールの `starts` は、実体ごとに `<実体 ID>#<開始 ID>` として展開します。置き換えるのは `effects` と `requires` だけです。
- 入れ子は 8 段までです。9 段目の呼び出しは展開せず `E-MODULE-CYCLE` にします。未定義のモジュールは `E-REF-MODULE`、未定義の `${名前}` は `E-STRUCTURE` です。
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
`E-NO-CONFIRM` で拒否します（`wait` の Step も入力を持つ Step です）。`use` を持つ Step は入力を持たないため、確認の検査の対象外です。
モジュールの `confirm` が `${名前}` を含む場合は、引数で置き換えた後の宣言を検査します（`reason: "${why}"` に空文字を渡すと `E-NO-CONFIRM`）。

| `kind` | 欄 | 意味の制約 |
|---|---|---|
| `point_at` | `targets`（各要素は `view`・`path`）、`on_miss`（任意） | `targets` は 1 件以上で、どれも空でない |
| `answer` | `options`（各要素は `id`・`correct`、任意で `on_wrong`・`label`）、`on_wrong` | 1 件以上で、`id` は空でなく重複しない。`correct: true` が 1 件以上。`on_wrong` は空でない。`label` は書くなら空でない |
| `approve` | なし | 欄を足すと `E-PARSE` |
| `none` | `reason` | `reason` が前後の空白を除いて空でないこと。無い・空・空白だけは `E-NO-CONFIRM` |

- 未知の欄・未知の `kind`・欄の欠け・型の違いは `E-PARSE`、意味の制約の違反は `E-STRUCTURE` です
- `none` の理由の欠けを `E-PARSE` でなく `E-NO-CONFIRM` にするのは、PACK-13 の「理由つき」の違反として扱うためです（2026-10-15 の決定）
- 入力の無い Step に `confirm` を書いても、形が正しければ診断しません。該当する診断コードが spec に無いためで、spec 側の判断が要ります（2026-10-15）
- `answer` の選択肢の `on_wrong`（EP-6）は、形として受け付けるだけです。意味の検査は後続のタスクが受け持ちます
- `answer` の選択肢の `label`（v1.3）は表示名です。画面の選択肢と、操作のログの「選択: …」に同じ表示名が出ます。省略すると ID がそのまま出ます。前後の空白を除いて空になる `label` は `E-STRUCTURE` で拒否します（2026-10-15 の決定。契約の版を 2 に上げました）
- 履歴に残る値は選択肢の ID で、表示名ではありません。`label` を後で直しても、記録した履歴の再生は変わりません
- 確認の `options[].label` は、入力方式 `choice` の `options[].label`（必須）とは別の欄です。表示名の解決は 1 つの Step の確認の選択肢の中だけで行い、入力方式 `choice` の `label` へは落ちません
- `on_miss`・`on_wrong` の参照先と `targets[].view` の解決は、参照検査（TASK-46）が受け持ち、ここでは検査しません

## 処理中の進行（`processing`）

`processing` の要素に書ける欄は、`name`（空でない文字列）と `effects`（効果の列）だけです。未知の欄は `E-PARSE`、空の名前は
`E-STRUCTURE` で拒否します。

**進行の間隔は宣言できません。** `processing` の要素にも、Step にも、`pack:` にも、間隔の欄はありません。書くと `E-PARSE`
になります（PACK-44、D-43）。間隔はエンジンのビルド時の定数で、300ms です（D-47、EVT-18）。配信する設定では、教材からも URL からも
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

## 根拠となる試験

| 試験 | 確かめること |
|---|---|
| `crates/contract/tests/content_hash_contract.rs` | 本文の木を除いて求めること、本文だけの修正でハッシュが変わらないこと |
| `crates/core/tests/version_record.rs` の `body_only_edit_yields_the_same_version_record` | 本文だけを直した IR から作る版の記録が、全欄で同じになること |
| `crates/contract/tests/fixed_histories.rs` | 固定の履歴 4 本の版の記録が全軸で同一で、期待値を D-51 に合わせて変えていないこと |
| `crates/pack/tests/format_b.rs` の `pack4_` | YAML の値の文字列としての読み取り・重複キーの拒否・構文エラーの行と桁（PACK-4） |
| `crates/pack/tests/modules.rs` | 2 つの実体が別の Step の実体になること・1 か所の修正が両方に反映されること・3 周目をファイルの追加だけで書けること・入れ子の上限と不正な呼び出し（PACK-19） |
| `crates/pack/tests/input_vocabulary.rs` の `pack10_` | 5 方式と `wait`・`cancel` の宣言が検証を通ること、`text` に内容を判定する欄が書けないこと、不正な宣言の診断と値を出さないこと（PACK-10） |
| `crates/contract/tests/input_spec_contract.rs` | `text` の空・下限・上限・コードポイントの数え方と、各方式の意味の制約（PACK-10） |
| `crates/pack/tests/confirm_vocabulary.rs` の `pack13_` | 確認 4 種が検証を通ること、確認の無い入力つき Step と理由の無い `none` が `E-NO-CONFIRM` になること、検証を通った教材に確認なしで達成になりうる Step が 0 件であること（PACK-13） |
| `crates/contract/tests/confirm_spec_contract.rs` | 確認 4 種の型の形・意味の制約と、受付と達成の分け方（`none` だけが受付で達成になる。R-22）。`pack23_` は選択肢の表示名の解決・ログの文面・履歴に残す値が ID であること・空白だけの `label` の拒否（PACK-23） |
| `crates/pack/tests/confirm_vocabulary.rs` の `pack23_` | `label` が IR に残ること、IR から読んだ表示名とログの文面、履歴に ID だけが残ること、空白だけの `label` の `E-STRUCTURE`（PACK-23） |
| `crates/pack/tests/effects.rs` の `pack44_` | 進行の間隔の欄がどの位置でも `E-PARSE` で拒否され値を出さないこと、件数を増やす回避が検証を通ること（PACK-44） |
