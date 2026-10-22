# 構造定義（形式 B）

教材パックの構造定義（`workshop.yaml`・`chapter.yaml`・`module.yaml`・`tests/playthrough.yaml`）に、どの欄を、どの型で、必須か省略できるかを書いた文書です。教材の作成者が、エンジンのソースを読まずに形式 B の教材パックを書けるようにするために置いています（AUTHOR-12）。

- 文書の版: 1
- 確認した日: 2026-10-20（実装は `main` の `a0ec183` 時点。10 節に机上の照合の記録があります）
- 仕様は private のため、本文を転記せず ID（PACK-2〜PACK-5・PACK-25・AUTHOR-12・AUTHOR-13・D-64）で参照します

## 1. 位置づけと読む順

この文書は「どの欄に何をどの型で書くか」に絞ります。欄の意味と書き方の詳細は、次の文書に任せます。

| 知りたいこと | 読む文書 |
|---|---|
| 語（入力方式・条件・状態変化など）の一覧と、語ごとの語彙の版 | `docs/vocabulary/vocabulary.md`（索引は `docs/vocabulary/README.md`） |
| 入力方式・確認・拒否の文面・モジュール・左エリアなどの書き方 | `docs/pack-authoring.md` |
| 操作プロファイル（接頭辞・入力欄）と外観で選べるもの | `docs/operation-profile.md` |
| 左エリアのビューと、`views` の書き方 | `docs/views.md` |
| 診断コード（`E-*`）の意味 | `docs/diagnostics.md` |
| `fandhe-pack` の使い方 | `docs/cli.md` |
| 書けないこと（忠実さの限界）と回避 | `docs/known-gaps.md` |

読む順は、この文書で全体の形をつかみ、書きたい欄の詳細を上の表から引く流れを勧めます。

## 2. ファイルの置き方

教材パックは 1 つのディレクトリです。形式 A（Markdown と TOML）と形式 B（Markdown と YAML）があり、この文書は形式 B を扱います。拡張子以外の欄は、2 つの形式で同じです。

```text
<pack-dir>/
├── workshop.yaml                  # 必須。教材全体の定義
├── chapters/<名前>/chapter.yaml   # 章（0 個以上）
├── chapters/<名前>/*.md           # Step の本文（章の定義ファイルからの相対パス）
├── modules/<名前>/module.yaml     # 学習モジュール（0 個以上）
├── modules/<名前>/*.md            # モジュールの Step の本文
├── fixtures/...                   # 練習データ（workshop.yaml の fixtures が指す）
└── tests/playthrough.yaml         # 教材テスト（省略できる）
```

- `workshop.yaml` と `workshop.toml` の両方がある、またはどちらも無い教材パックは `E-PARSE` で拒否します（PACK-2）
- 章とモジュールのディレクトリは、名前順に読みます。定義ファイルが無いディレクトリは `E-STRUCTURE` です
- 本文のパスは、章・モジュールのディレクトリの下の安全な相対パスです。`..`・絶対パス・`\` を含むパスは `E-REF-BODY` で拒否します。存在しないファイルも `E-REF-BODY` です
- 練習データのパスは、教材パックの根から見て `fixtures/` の下の安全な相対パスです。それ以外は `E-REF-FIXTURE` です
- 読む前に、上限（全体 20 MiB・本文 1 ファイル 512 KiB・ディレクトリの深さ 32・シンボリックリンクの禁止）を確かめます。超えると `E-LIMIT-*` です（値の正は `crates/contract/src/limits.rs`）
- 教材テストの読み込みでは、`tests/playthrough.yaml` が無いことだけを「省略」と認めます。ディレクトリや特殊ファイルの場合は `E-STRUCTURE` です

## 3. `workshop.yaml`

最上位に書ける欄は次の 9 つです。これ以外の欄は `unknown field` として `E-PARSE` で拒否します（PACK-3）。

| 欄 | 型 | 必須 | 既定 | 検査 |
|---|---|---|---|---|
| `pack` | 表（3.1） | 必須 | なし | 3.1 のとおり |
| `refs` | 列（要素は任意の値） | 省略可 | 空 | pack crate は型を検査しません |
| `commands` | 表（キーはコマンド名、値は任意の値） | 省略可 | 空 | キーを、Step の `input`・規則が指す名前と照合します（`E-REF-COMMAND`）。値の型は検査しません |
| `views` | 表（キーはビューの ID、値は任意の値） | 省略可 | 空 | キーを `E-REF-VIEW` で照合します。値のうち `body` だけ検査します（`docs/views.md`）。`kind` の綴りは検査しません |
| `fixtures` | 表（キーは練習データの ID、値は `path` と `kind`） | 省略可 | 空 | 3.2 のとおり |
| `hints` | 表（キーはヒントの ID、値は任意の値） | 省略可 | 空 | キーを `E-REF-HINT` で照合します。値のうち `levels` だけ検査し、`A`・`B`・`C` の文字列の列でなければ `E-PARSE`（項目 `hint:<ID>`）です。他の欄の型は検査しません |
| `rejections` | 表（キーは文面の ID） | 省略可 | 空 | 3.3 のとおり |
| `clarifications` | 表（キーは文面の ID） | 省略可 | 空 | 3.3 のとおり |
| `defaults` | 表（キーは拒否の分類、値は文面の ID） | 省略可 | 空 | キーは 8 語の分類に限ります（`E-PARSE`） |

「任意の値」の欄は、読み込みの段階では型を付けず、後段の検査（`validate`・`compile`）が語彙の規則で読みます。型を検査しない欄は、書き間違えても読み込みでは止まりません。診断の有無は `docs/diagnostics.md` で確かめ、欄ごとの試験を持つ欄だけを「検査する」と書いています。

### 3.1 `pack`

| 欄 | 型 | 必須 | 既定 | 備考 |
|---|---|---|---|---|
| `id` | 文字列 | 必須 | なし | 教材の ID |
| `title` | 文字列 | 必須 | なし | 教材の題名 |
| `version` | 文字列 | 必須 | なし | 教材の版。`主.副.修` の 10 進で書く運用です（`docs/pack-authoring.md` の「主版を上げる条件」）。形は検査しません |
| `ir_format` | 整数 | 必須 | なし | 1 または 2。それ以外は `E-FORMAT-VERSION` |
| `guidance` | 文字列 | 必須 | なし | 案内の水準。`A`・`B`・`C` のどれかで、それ以外は読み込みで `E-PARSE`（項目 `pack:guidance`）です（EP-8。2026-10-21 時点） |
| `vocab` | 文字列（`主.副`） | 省略可 | 省略 | 語彙の版の宣言。省略は非推奨です（`docs/pack-authoring.md` の「`pack.vocab` の書き方」） |
| `guidance_levels` | 表 | 省略可 | 省略 | EP-8。案内レベルと隠す語の宣言。キーは `A`・`B`・`C`、値は `hide_command_kinds`（`builtin`・`skill`・`flow`・`unsupported`）と `hide_words`（文字列の列）だけを持てます。読めない形は `E-PARSE`（項目 `pack:guidance_levels`） |
| `profile` | 表 | 省略可 | 省略（接頭辞 `/`・入力欄あり） | EP-10。`docs/operation-profile.md` |
| `appearance` | 表 | 省略可 | 省略（`standard`） | EP-11。`docs/operation-profile.md` |
| `left_pane` | 表 | 省略可 | 省略（`shown`・`all`） | EP-12。`docs/pack-authoring.md` の「左エリア」 |

`pack` の中の未知の欄も `E-PARSE` です。`vocab` は 1.2 のように引用符で囲みます（`1.10` が `1.1` にならないよう、文字列として読みます。PACK-4）。

### 3.2 `fixtures`

| 欄 | 型 | 必須 | 備考 |
|---|---|---|---|
| `path` | 文字列 | 必須 | 教材パックの根からの相対パス。`fixtures/` の下に限ります |
| `kind` | 文字列 | 必須 | 種類。pack crate は値を検査しません |

### 3.3 `rejections`・`clarifications`・`defaults`

- `rejections.<ID>` は `category`（拒否の分類）・`message`・`next_action` を持ちます。`category` は 8 語に限ります（`E-PARSE`）。`message`・`next_action` に差し込みの枠 `{{...}}` を書くときは 9 節の注意が要ります
- `clarifications.<ID>` は `message` と `how_to_answer` を持ちます。`how_to_answer` が次の行動の欄です
- 次の行動の欄が空だと `E-EMPTY-NEXT-ACTION` です（R-21、PACK-12）
- 8 語の分類と枠の規則は `docs/pack-authoring.md` の「拒否の分類」「差し込みの枠」を見てください

## 4. `chapter.yaml`・`module.yaml`

章と学習モジュールは、同じ形の定義ファイルです。違いは、モジュールだけが `params` を使い、`use` から呼ばれることです。

| 欄 | 型 | 必須 | 既定 | 備考 |
|---|---|---|---|---|
| `id` | 文字列 | 必須 | なし | 章・モジュールの ID |
| `title` | 文字列 | 必須 | なし | 題名 |
| `params` | 表（文字列から文字列） | 省略可 | 空 | モジュールの引数の既定値。値は文字列で書く（数値・真偽値は `E-PARSE`） |
| `param_types` | 表（引数の名前から型の宣言） | 省略可 | 空 | 語彙 1.7（PACK-51）。`params` の引数ごとの型と範囲です。宣言は `type`（`bool`・`int`・`enum`）が必須で、`enum` は `values`（空・重複は不可）、`int` は `min`・`max`（両端を含む）を書けます。鍵が `params` に無い・組み合わせの誤り・既定値が範囲外は `E-STRUCTURE`、未知の型・欄は `E-PARSE` です。呼び出しの引数が範囲外でも `E-STRUCTURE` です。IR には載りません |
| `starts` | 列（4 節の開始点） | 省略可 | 空 | 途中開始。全章を合わせて 1 件以上が要ります（`E-STRUCTURE`。PACK-25）。モジュールの `starts` は数えません |
| `steps` | 列（5 節の Step） | 必須 | なし | Step の列（ファイルの中の順） |

未知の欄は `E-PARSE` です。

### 開始点（`starts[]`）

| 欄 | 型 | 必須 | 既定 | 備考 |
|---|---|---|---|---|
| `id` | 文字列 | 必須 | なし | 開始点の ID |
| `node` | 文字列 | 必須 | なし | 開始する Step の ID（`E-REF-STEP` で照合） |
| `effects` | 列（状態変化。7 節） | 省略可 | 空 | 開始時の世界状態を作る |
| `guidance` | 値 | 省略可 | 省略 | EP-8。開始点ごとの案内レベル。`A`・`B`・`C` のどれかで、それ以外は `E-PARSE`（項目 `scope`） |
| `requires` | 条件（7 節） | 省略可 | 省略 | EP-9。開始の条件。`unmet` と両方そろえる（片方だけは `E-STRUCTURE`。`null` は省略と同じ） |
| `unmet` | 文字列 | 省略可 | 省略 | EP-9。満たさないときの文面（`rejections` の ID）。`requires` と両方そろえる（片方だけは `E-STRUCTURE`） |

## 5. Step

`steps[]` の 1 要素です。未知の欄は `E-PARSE` です。

| 欄 | 型 | 必須 | 既定 | 備考 |
|---|---|---|---|---|
| `id` | 文字列 | 必須 | なし | Step の ID。重複は `E-DUP-ID` |
| `title` | 文字列 | 必須 | なし | 題名 |
| `body` | 文字列（パス） | 省略可 | 省略 | 本文の Markdown。章・モジュールのディレクトリからの相対 |
| `body_levels` | 表（水準から本文のパス） | 省略可 | 空 | EP-8。水準ごとの本文。キーが `A`・`B`・`C` でなければ `E-PARSE`（項目 `scope`）です。水準 A の本文は `body` に書きます |
| `do` | 文字列 | 省略可 | 省略 | 今やるべきことの文。枠 `{{step_do}}` の元（EP-7） |
| `do_levels` | 表（水準から文） | 省略可 | 空 | 語彙 1.6（PACK-50）。水準ごとの `do`。キーは `B`・`C` だけで、水準 A の文は `do` に書きます。`do` が必須です。キーが `A` または空の値は `E-STRUCTURE`、キーが `A`・`B`・`C` でなければ `E-PARSE`（項目 `scope`）です。探す順は `C`・`B`・`do` です |
| `lo` | 列（文字列） | 省略可 | 空 | 学習目標（LO）。要素の中身の検査は未確認です |
| `final` | 真偽値 | 省略可 | `false` | 終端の Step |
| `views` | 列（ビューの ID） | 省略可 | 空 | この Step が宣言するビュー（`E-REF-VIEW`） |
| `fixtures` | 列（練習データの ID） | 省略可 | 空 | `E-REF-FIXTURE` |
| `hints` | 列（ヒントの ID） | 省略可 | 空 | `E-REF-HINT` |
| `on_enter` | 列（状態変化） | 省略可 | 空 | 入ったときの変化 |
| `processing` | 列（`name`・`effects`） | 省略可 | 空 | 処理中の進行 |
| `input` | 表（6 節） | 省略可 | 省略 | 入力方式 |
| `rules` | 列（6 節） | 省略可 | 空 | 入力に対する規則 |
| `on_reject` | 表（分類から文面の ID） | 省略可 | 空 | 分類ごとの拒否の文面 |
| `confirm` | 表（6 節） | 省略可 | 省略 | 確認 |
| `on_achieved` | 列（状態変化） | 省略可 | 空 | 達成したときの変化 |
| `next` | 列（`to`・`when`） | 省略可 | 空 | 遷移。`to` は必須、`when` は条件（7 節）で省略可 |
| `use` | 表（`module`・`instance`・`args`） | 省略可 | 省略 | モジュールの呼び出し。`input` と同時には書けません（`E-STRUCTURE`） |

- 引用符なしの `do`・`body` に `null`・`~` を書くと、未設定ではなく、その字面の文字列になります（PACK-4）
- `final` の Step に `next` を書くかどうかの規則は `docs/diagnostics.md` の `E-DEAD-END`・`E-UNREACHABLE` を見てください

## 6. 入力・規則・確認

読み込みの段階では型を付けず、`validate`・`compile` が語彙の規則で読む欄です。型と閉じた集合は次の場所で決まります。

| 欄 | 決まる場所 | 内容 |
|---|---|---|
| `input` | `crates/pack/src/input.rs`・contract の `input_spec` | `kind` は `command`・`text`・`choice`・`ui_action`・`approval`・`wait`・`env_action`。欄は `docs/pack-authoring.md` の「入力方式」の表。`approval` は省略可の `revise`（`candidates` の要素は `id`・`label`。語彙 1.5）。`env_action` は必須の `action`（`save`・`export`。語彙 1.5） |
| `confirm` | `crates/pack/src/confirm.rs`・contract の `confirm_spec` | `kind` は `point_at`・`answer`・`approve`・`none`。入力を持つ Step には必須（`E-NO-CONFIRM`） |
| `rules[]` | `crates/pack/src/rejection.rs`・`effect_check.rs`・`condition_check.rs` | `id`・`on`（`command`・`args`・`ui`・`cancel` など）・`when`（省略可）・`outcome`（`kind`: `accept`・`reject`・`clarify`） |
| `processing[]` | `crates/pack/src/effect_check.rs` | `name`（空でない文字列）と `effects` だけ。間隔の欄は書けません（PACK-44） |

検証の範囲は欄によって異なります。`validate`・`compile` の成功は、規則の全欄の検証済みを意味しません。

| 欄 | 検査される範囲 |
|---|---|
| `input`・`confirm`・`processing[]` | 未知の欄・未知の `kind`・型の違いは `E-PARSE`、意味の制約の違反は `E-STRUCTURE`、参照先の不在は `E-REF-*` です。`processing[].effects` は状態変化の語彙検査（`effect_check.rs`）を通ります |
| `rules[]` | 読み込みの段階では型を付けず、そのまま保持します。展開（`expand.rs`）が検査するのは `when` の条件だけです。`on`・`outcome` の欄の書き違いは、読み込みでは検出されない場合があります |
| `rules[].outcome.effects` | `effect_check.rs` の語彙検査に渡りません。不正な欄（例: 未知の欄）がコンパイルを通過し、実行時の警告（`Event::Warning`）になる場合があります（H-3。`docs/known-gaps.md`） |

`rules[]` の `outcome.effects` を書いたら、教材テストの模範解答と誤入力で、実際の動きを確かめてください。

## 7. 条件と状態変化

- 条件（`next[].when`・`starts[].requires`・`rules[].when` など）の語は `all`・`any`・`not`・`step`・`flag`・`world_has`・`world_attr`・`count`・`chose`・`param`・`clarifying` です。読み方は `crates/contract/src/condition.rs` と `crates/pack/src/condition_check.rs` が決めます
- 状態変化の語は `put`・`remove`・`replace`・`set_attr`・`set_flag`・`clear_flag`・`set_flag_global` で、`when` を付けて条件つきにできます。読み方は `crates/contract/src/effect.rs` と `crates/pack/src/effect_check.rs` が決めます
- 状態変化の経路と値は、書いた文字列そのものです。受講者の入力の値は入りません（H-3。`docs/known-gaps.md`）
- 無い要素への `remove`・`set_attr` は `validate` が `E-REF-ELEMENT` で報告します（PACK-30）

語ごとの語彙の版は `docs/vocabulary/vocabulary.md` の 7 節です。

## 8. `tests/playthrough.yaml`

教材テストです。IR には載りません。最上位に書ける欄は次の 4 つで、すべて省略できます（既定は空の列）。未知の欄は `E-PARSE` です。

| 欄 | 型 | 内容 |
|---|---|---|
| `playthroughs` | 列 | 模範解答の通し試験 |
| `error_inputs` | 列 | 誤入力の分類 |
| `text_checks` | 列 | 文面の確認 |
| `guidance_checks` | 列 | 案内の確認 |

- 読み込みでは各要素の型を付けません。中身の解釈を持つ `fandhe-pack test` は、2026-10-20 時点で未実装です（TASK-404）
- 今の実装が読むのは 2 か所だけです。`error_inputs[].input.expect`（`accepted`・`accepted:free_text_unjudged`・`rejected:<分類>`・`clarified`。読めない値は `E-PARSE`。`docs/pack-authoring.md` の「教材テストの誤入力の期待」）と、接頭辞が `/` 以外のときの `cmd:`・`text:` の分け（`E-PLAY-INPUT-CLASS`）です
- 書き方の例は、`testdata/input-field-ime/test-data/ch1-pack/tests/playthrough.yaml` にあります（`playthroughs[].segments[].steps[]` に `cmd`・`text`・`choice`・`ui`・`cancel`・`tick` と `expect` を並べる形）。この形を `validate` が検査するかは未確認です

## 9. YAML の書き方の注意

### 9.1 `{{...}}` は引用符で囲む（AUTHOR-13）

差し込みの枠 `{{step_do}}` のような `{{` を含む値は、**引用符で囲んでください**。引用符なしの `{{` は YAML が「表の開始」として読むため、次のとおり `E-PARSE` になります。

| 場合 | 例 | 結果 |
|---|---|---|
| (a) flow 形式 `{...}` の中で、引用符なし | `next_action: {{step_do}}`・`next_action: 今は {{step_do}}`（`{ ... }` の内側） | `E-PARSE`（行と桁つき） |
| (b) flow 形式の中で、引用符あり | `next_action: '{{step_do}}'` | 通る |
| (c) block 形式で、値が `{{` で始まり、引用符なし | `next_action: {{step_do}} 次へ` | `E-PARSE` |
| (d) block 形式で、`{{` が値の途中、引用符なし | `next_action: 今は {{step_do}}` | 通る |

```yaml
rejections:
  # 悪い例: flow 形式の中の引用符なし → E-PARSE
  # rj1: { category: wrong_order, message: x, next_action: {{step_do}} }
  rj1: { category: wrong_order, message: x, next_action: '{{step_do}}' }
  rj2:
    category: wrong_order
    message: x
    next_action: 今は {{step_do}}   # 途中なら block 形式では通る。ただし引用符で囲むのが安全
```

- 迷ったら、`{{` を含む値はすべて引用符で囲んでください。(d) が通るのは YAML の読み方の結果で、書き方の保証ではありません
- 診断には、書いた値を写さず、行と桁だけを示します。`step_do` の文字は診断に出ません
- 根拠の試験は `crates/pack/tests/format_b.rs` の `author13_` です（4 つの場合と、値を写さないこと）
- 差し込みの枠の規則（名前の 6 語・空になりうる枠・代替の文）は `docs/pack-authoring.md` の「差し込みの枠」に書いています

### 9.2 そのほかの YAML の注意

値の読み方（`true`・`1.10`・`010` を書いたとおりの文字列として読むこと）、重複キー、10 進でない整数、アンカー・エイリアス・タグ・複数文書の拒否は、`docs/pack-authoring.md` の「YAML の値の書き方（形式 B）」にまとめています（PACK-4・PACK-5）。

## 10. 机上の照合の記録

この文書に書いた欄の名前と型を、実装の読み込みと 1 行ずつ突き合わせました。実行して確かめたのではなく、ソースを読んで照合した結果です。

- 日付: 2026-10-20
- 方法: 机上の照合（`crates/pack/src/source.rs`・`crates/contract/src/ir.rs` の欄の定義を読み、この文書の表と比べた）。`serde` の `deny_unknown_fields` と `default` の有無から、必須・省略・未知の欄の拒否を判断した
- 結果: 食い違いは 0 件

| この文書の欄 | 実装の型 | 欄の定義 |
|---|---|---|
| 3 節の最上位 9 欄 | `WorkshopSrc`（`deny_unknown_fields`。`pack` だけ既定なし） | `crates/pack/src/source.rs` |
| 3.1 `pack` の 10 欄 | `PackInfo`（`deny_unknown_fields`。必須は `id`・`title`・`version`・`ir_format`・`guidance`） | `crates/contract/src/ir.rs` |
| 3.2 `fixtures` | `FixtureDef`（`path`・`kind` とも必須） | 同上 |
| 4 節の 6 欄 | `ScopeSrc`（`deny_unknown_fields`。`id`・`title`・`steps` が必須） | `crates/pack/src/source.rs` |
| 開始点 | `Start`（`id`・`node` が必須。`effects` は既定が空） | `crates/contract/src/ir.rs` |
| 5 節の Step の 20 欄 | `Step`（`deny_unknown_fields`。`id`・`title` が必須。`do`・`final`・`use` は欄名の付け替えあり） | 同上 |
| `next[]` | `Transition`（`to` が必須、`when` は省略可） | 同上 |
| 8 節の 4 欄 | `TestsSrc`（`deny_unknown_fields`。4 欄とも既定が空） | `crates/pack/src/source.rs` |
| 6 節・7 節の閉じた集合 | `input_spec`・`confirm_spec`・`condition`・`effect` の各モジュール | `crates/contract/src/` |
| 9 節の 4 つの場合 | `author13_` の試験 4 本 | `crates/pack/tests/format_b.rs` |

- 照合していないこと: 6 節・7 節の語の内側の欄（`rules[].on`・`outcome` の全欄、条件・状態変化の各語の引数）は、この文書では表にしていません。語の一覧は `docs/vocabulary/vocabulary.md` の 7 節、入力・確認の欄は `docs/pack-authoring.md` が正です
- 人が教材を書いて確かめた件数は 0 件です
