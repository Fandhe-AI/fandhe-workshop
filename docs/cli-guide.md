# 作成者向け CLI `fandhe-pack` の使い方

教材の作成者が、エンジンのソースを読まずに `fandhe-pack` を使えるようにするための手引きです（AUTHOR-12）。

- 文書の版: 2
- 確認した日: 2026-11-01（版 1 は 2026-10-24。版 2 の変更は 2 節の配布された実行ファイルの案内）
- 仕様は private のため、本文を転記せず ID で参照します

## 1. 位置づけ

この文書は手引きです。コマンドの引数・出力の形・終了コードの契約の正は `docs/cli.md`（版 13）です。食い違って見えたら `docs/cli.md` に従ってください。この文書は表や形式を固定し直さず、`docs/cli.md` の節を指します。

教材を書くための公開の資料は、次の順に読むことを勧めます。

| 順 | 資料 | 内容 |
|---|---|---|
| 1 | `docs/schema-format-b.md` | 欄・型・必須 |
| 2 | `docs/examples/` | 最小の教材パック 2 つ（コマンドで進む例・選択肢と画面操作だけで進む例） |
| 3 | `docs/vocabulary/README.md` | 語（入力方式・条件・状態変化など）の一覧 |
| 4 | `docs/operation-profile.md` | 操作プロファイルと外観で選べるもの |
| 5 | `docs/views.md` | ビューと左エリア |
| 6 | `docs/diagnostics.md` | 診断コード（`E-*`）の意味と早見表 |
| 7 | 本書 | コマンドの使い方と出力の読み方 |
| 8 | `docs/known-gaps.md` | 書けないことと回避 |

## 2. 実行ファイルの用意

用意の仕方は 2 つあります。Rust を入れたくない場合は 2.1 を使います。

### 2.1 配布された実行ファイルを使う

Apple シリコンの macOS 用だけが対象です。Intel の macOS・Windows・Linux 向けはまだありません。Rust は要りません。配布物（tar.gz）には、実行ファイル・`LICENSE-MIT`・`LICENSE-APACHE`・案内文の `README.txt` が入っています。次の手順は `README.txt` と同じです。食い違って見えたら、配布物の `README.txt` に従ってください。

実行ファイルには署名も公証もありません（TASK-414 の決定。L2 の配布物と同じ方針です）。ブラウザで落とすと macOS が quarantine の属性を付け、そのままでは起動を止められることがあります。

1. 照合します。`.sha256` を tar.gz と同じ場所に置き、`shasum -a 256 -c fandhe-pack-<版>-aarch64-apple-darwin.tar.gz.sha256` を実行して `OK` を確かめます
2. 展開します。`tar -xzf fandhe-pack-<版>-aarch64-apple-darwin.tar.gz`
3. 展開してできたディレクトリの中で、実行ファイル 1 つの quarantine を外します。`xattr -d com.apple.quarantine fandhe-pack`。属性が無いときは `No such xattr` と出ますが、無害です
4. `./fandhe-pack validate <pack-dir>` のように実行します

### 2.2 ソースからビルドする

リポジトリのルートで、次を実行します。実行ファイルは `target/debug/fandhe-pack` にできます。

```text
cargo build -p fandhe-workshop-pack --bin fandhe-pack
```

引数なしで実行すると、使い方が標準エラー出力に出て、終了コードは 2 です（`docs/cli.md` の 4 節）。

## 3. ふだんの流れ

`<pack-dir>` は `workshop.yaml` があるディレクトリです。

1. `fandhe-pack validate <pack-dir>` が `OK:` になるまで直す
2. `tests/playthrough.yaml` を書き、`fandhe-pack test <pack-dir>` が全件合格で終了コード 0 になるまで直す
3. `fandhe-pack compile <pack-dir>` で IR（JSON）を作る。画面に配るのはこの IR
4. `fandhe-pack vocab <pack-dir>` で、使った拡張点を確かめる。`fandhe-pack metrics <pack-dir>` で、ファイルごとの大きさを見る
5. 2 つの教材パックの IR を比べたいときは `fandhe-pack compare <pack-dir-a> <pack-dir-b>`
6. 画面で確かめたいときは `fandhe-pack preview <pack-dir>`（下の「画面で確かめる」）

コマンドと引数の形は `docs/cli.md` の 3 節にあります。

### 画面で確かめる（preview）

`fandhe-pack preview <pack-dir>` は、検査（`validate`）と `compile` に診断が 0 件のときだけ、手元で配信を始めて URL を出します。

```text
OK: ...
URL: http://127.0.0.1:<port>/preview/
```

- 出た URL をブラウザで開きます。ブラウザは自動では開きません。Rust も Node も要りません（エンジンの画面は実行ファイルに入っています）
- 診断があるときは何も配らず、`validate` と同じ形で診断を出して終了コード 1 になります
- 文章や設定を直したら、`Ctrl+C` で止めて、もう一度 `preview` を実行します。ファイルの監視と自動の再読み込みはありません
- 実行のたびにポートが変わるため、画面は保存済みの履歴が空の状態で始まります。前の実行で開いていたタブは読み込めなくなるので、新しい URL を開いてください
- 待ち受けは自分の端末（127.0.0.1）だけです。同じネットワークの他の端末からは見えません

## 4. 出力の読み方

### 4.1 診断の行と終了コード

- 診断は 1 件につき 1 行で、4 欄（ファイル・項目・コード・メッセージ）です（`docs/cli.md` の 6 節・7 節）。ID に制御文字などが含まれるときは `\u{XXXX}` に直して出ます。その形のまま ID を直してください
- コードの意味と、直すときに見る資料は `docs/diagnostics.md` の 2 節（意味）と 8 節（早見表）にあります
- 終了コードは、0 が診断 0 件、1 が診断あり、2 が使い方の誤りです（`docs/cli.md` の 5 節）
- 診断は標準出力に出ます（`docs/cli.md` の 9 節）
- 読み込みに失敗すると、参照の整合などの検査は流れません。`E-PARSE` と `E-LIMIT-*` を直した後で、新しい診断が出ることがあります（`docs/cli.md` の 8 節、`docs/pack-authoring.md`）

### 4.2 `validate`

構造・参照の整合・到達可能性・確認の有無などを静的に検査します。成功すると、最終行が `OK:` の 1 行だけ出ます。失敗すると、診断の行の後に `NG: 診断 N 件（...）` が出ます（失敗の例は `docs/cli.md` の 8 節）。

<!-- cli-guide-run: validate minimal-pack -->
```text
OK: 診断 0 件（形式 B(YAML)、章 1、モジュール 0、Step の定義 2）
```

### 4.3 `test`

検査を通した後に、`tests/playthrough.yaml` の模範解答での完走と、誤入力の分類を流します。検査（読み込み・`validate`・教材テストの読み取り・`compile`）の関門のどれかで診断が出たら、試験は流しません（`docs/cli.md` の 12.1 節）。

- PASS の行は、模範解答（`PASS <ID>（入力 N 件）`）と誤入力（`PASS <ID> → rejected:<分類>`）です
- FAIL の行は、期待と実際の結果の違いと、その位置を示します（`docs/cli.md` の 12.3 節）
- 最後の集計は 3 行です。模範解答の完走数、誤入力の期待どおりの数、入力の総数です（`docs/cli.md` の 12.4 節）。その後ろに R-22・R-21 の行（12.5 節）と、試験で入った Step の実体の行（12.6 節）が続きます。入らなかった実体は失敗ではなく、到達する試験を書く目安です
- `cmd:` は、入力欄に打つ文字列を接頭辞を含めて書きます（接頭辞が `/` なら `cmd: /hello`）。接頭辞が `/` 以外の教材で、画面と別の種類になる書き方は `E-PLAY-INPUT-CLASS` として報告されます
- 模範解答の `end: complete` は、全入力が期待どおりに通り、最後の入力が `achieved` であることで満たします。最後の Step を `confirm: none` にすると `accepted` で終わるため、完走になりません。最後の Step には `point_at` などの確認を置き、最後の入力で達成にしてください。`docs/examples/minimal-pack/` がその形です

<!-- cli-guide-run: test minimal-pack -->
```text
PASS pt-main（入力 3 件）
PASS e-unknown → rejected:unknown_command
PASS e-wrong-step → rejected:not_in_this_step
PASS e-text → rejected:not_in_this_step
模範解答: 1/1 完走
誤入力: 3/3 が期待どおりの結果
入力の総数: 6
R-22 受付と達成の分離: 確認の操作で達成になった 1 件、確認を省くと明示した Step の達成 1 件、確認なしで達成になった 0 件
R-21 返した文面 3 件のうち「次の行動」が空 0 件
試験で入った Step の実体: 2/2（入らなかった実体: なし）
```

`test` が確かめないことと、今は動かないことを、次に示します。

| 項目 | 内容 |
|---|---|
| 画面の表示 | ビューの行・本文の表示・外観・入力欄が隠れること・見た目は確かめません。停止（Esc）も確かめません。ブラウザでの確認が別に要ります（`docs/known-gaps.md` の AUTHOR-7） |
| `asserts`・`text_checks`・`guidance_checks` | 判定しません。定義があるだけで、通ったと見せないために終了コードが 1 になります。書かないでください（経過措置。`docs/cli.md` の 12.1 節） |
| 最終行 | `OK:`・`NG:` の最終行は、後続のタスクが足すまで出ません。出るようになると、上の出力は変わります |

### 4.4 `compile`

IR（JSON）を標準出力に出します。`-o <file>` を付けるとファイルに書きます。診断があれば何も書かず、既存のファイルにも触れません。書き出しは一時ファイルを経由して置き換えるので、途中までの IR は残りません。`--chapters <id>,...` で、選んだ章の Step と、そこから呼ばれる実体だけに絞れます。詳しくは `docs/cli.md` の 10 節にあります。

### 4.5 `vocab`

Step ごとに、使った拡張点の ID を表にして出し、末尾に集計行（`# ` で始まる行）を出します。表の下の集計は、拡張点を使わなかった Step の数と、使った Step の数です。`pack.profile`・`pack.appearance`・`pack.left_pane` を既定から変えると、教材パック全体の行が末尾に出ます（`docs/cli.md` の 12 節）。

<!-- cli-guide-run: vocab choice-only-pack -->
```text
scope,step,kind,extension_points
c-pick,st-pick,step,EP-7
c-pick,st-point,step,
# (i) 既存の語彙の組み合わせだけで書けた Step: 1
# (ii) 名前つき拡張点を使って書けた Step: 1
# EP-7 を使った Step: 1
# EP-10 を使った（教材パック全体）: pack.profile
# EP-11 を使った（教材パック全体）: pack.appearance
# EP-12 を使った（教材パック全体）: pack.left_pane
```

### 4.6 `metrics`

ファイルごとの行数・実効行数・バイト数・入れ子の深さを CSV で出します（`docs/cli.md` の 11 節）。教材の大きさの目安に使います。

## 5. 例で試す

`docs/examples/` の 2 つの教材パックで、上の出力を再現できます。リポジトリのルートで、次を実行します。

```text
target/debug/fandhe-pack validate docs/examples/minimal-pack
target/debug/fandhe-pack test docs/examples/minimal-pack
target/debug/fandhe-pack validate docs/examples/choice-only-pack
target/debug/fandhe-pack test docs/examples/choice-only-pack
```

選択肢と画面操作だけで進む例の出力は次のとおりです。

<!-- cli-guide-run: validate choice-only-pack -->
```text
OK: 診断 0 件（形式 B(YAML)、章 1、モジュール 0、Step の定義 2）
```

<!-- cli-guide-run: test choice-only-pack -->
```text
PASS pt-main（入力 4 件）
PASS e-wrong-pick → rejected:not_in_this_step
模範解答: 1/1 完走
誤入力: 1/1 が期待どおりの結果
入力の総数: 5
R-22 受付と達成の分離: 確認の操作で達成になった 2 件、確認を省くと明示した Step の達成 0 件、確認なしで達成になった 0 件
R-21 返した文面 1 件のうち「次の行動」が空 0 件
試験で入った Step の実体: 2/2（入らなかった実体: なし）
```

この節の出力は、試験（`crates/pack/tests/examples_pack.rs`）が実行ファイルの出力と照合しています。失敗した場合の出力（`NG:`・`FAIL`）は、ここには写しません。`docs/cli.md` の 8 節・12.3 節の例を見てください。

## 6. 診断が出たら

1. 診断の 4 欄から、どのファイルの、どの項目かを確かめる
2. コードを `docs/diagnostics.md` の 2 節で引き、条件を読む。8 節の早見表で、そのコードを出すコマンドと、直すときに見る資料を確かめる
3. 直したら `validate` をもう一度流す。診断が増えて見えるのは、先の失敗が後の検査を止めていたためです
4. 書きたいことが書けないときは、`docs/known-gaps.md` を見る。載っていなければ、回避した方法と合わせて記録してください
