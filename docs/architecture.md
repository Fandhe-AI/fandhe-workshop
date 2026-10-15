# 実装 repo の構成設計

```text
構成設計の版: 1
決定日: 2026-10-12
状態: 固定（契約）
```

## 1. 位置づけと版

- 本文書は、エンジン実装の crate の境界・依存の向き・公開 API の型の置き場を固定する契約である
- 責務ごとに並列で進む実装（共通エンジン・入力と表示・教材形式と検証・作成ツール・保存復元）が、境界の食い違いで
  統合時に作り直しにならないよう、実装より先に文書で固定する
- 他のタスクは、本文書を「`docs/architecture.md` 構成設計の版 1」の形で入力に書ける
- 仕様は private の submodule `docs/spec`（`Fandhe-AI/fandhe-workshop-spec`）にある。本文書は仕様を ID
  （`D-4`・`W-22`・`EVT-23`・`TASK-24` など）で参照するだけで、本文を転記しない
- crate 名・ディレクトリ名はすべて仮称である（`Q-10`）。名前を確定する変更は版上げの対象とする
- 変更規則: 変更する場合は、先に spec の 04 を直し、新しい契約タスク（版上げ）を立てる。黙って変えない。
  契約の型の版（`contract_version`・`ir_format` など）は各契約タスクが持ち、本文書の版とは別に管理する

## 2. 設計の原則

- 依存は一方向に保つ。下層（`contract`）は上層を知らない
- 複数の責務が参照するものは、振る舞いを持たない契約として最下層に置く。並列作業で、契約の変更と実装の変更が
  同じ crate で衝突しないようにする
- エンジンは特定の教材の知識を持たない（`W-22`）
- 教材パック・履歴・IR は未信頼入力として扱い、失敗時は拒否に倒す

## 3. crate 一覧

| crate（仮称） | 種別 | 役割 | 外部依存（版は TASK-24・TASK-26 が固定する） |
|---|---|---|---|
| `fandhe-workshop-contract` | lib | 責務をまたぐ共有の契約。データ型・定数・版・スキーマ・サンプル・純粋な表・純関数。I/O・状態遷移・時刻・乱数を持たない | `serde`・`serde_json` のみ |
| `fandhe-workshop-core` | lib | DOM 非依存の純 Rust の中核。`Session`・`apply`・入力・イベント・Step の状態・`Projection`・進行管理・周回・停止・チェックポイント・保存復元の処理・状態変化の提供元の境界 | contract、`serde`・`serde_json` |
| `fandhe-workshop-pack` | lib + bin `fandhe-pack` | 教材パックの読み込み・検査・コンパイル・教材テストと、作成者向け CLI | core、contract、`serde`・`serde_json`・`toml`・`yaml_serde`・`pulldown-cmark`（native のみ） |
| `fandhe-workshop-web` | lib（`cdylib`・`rlib`） | `fandhe-frontend` の Component 実装と glue。wasm32 向け。native でも `rlib` として試験できる | core、contract、`fandhe-frontend-*`（wasm-full 0.46.0。TASK-26）、`wasm-bindgen`、`web-sys`（`cfg(target_arch = "wasm32")` のみ） |

- バイナリ名 `fandhe-pack` は TASK-399（CLI の外部仕様）の契約が持つ。本文書は pack crate に置くことだけを固定する
- ワークスペース設定は `resolver = "2"`、`members` は 4 crate すべて、`default-members` は contract・core・pack
  とする（web を含めない。wasm32 向けの依存を素の `cargo test` に引き込まないため）
- `[profile.release]` はワークスペースのルートにだけ書ける（メンバーの `[profile]` は無視される）。値は
  `prototype/Cargo.toml` を参考に TASK-24 が確定する

## 4. 依存の向き

許可する辺（normal 依存）は次の 5 本だけである。

```text
web ──▶ core ──▶ contract
pack ─▶ core
pack ─▶ contract
web ──▶ contract
```

| 辺 | 扱い | 根拠 |
|---|---|---|
| contract → core / pack / web | 禁止 | 契約は最下層 |
| core → pack / web | 禁止（normal・dev とも） | `EVT-23`・`W-22` |
| pack → web | 禁止（normal・dev とも） | 作成ツールが表示層に依らない |
| web → pack（normal） | 禁止 | 配信物に pack（Markdown 変換など native 専用）が入らない |
| web → pack（dev-dependency） | 許可（試験専用の例外） | 教材パックを IR にコンパイルして web の試験で使う。配信物には入らない |
| contract・core の外部依存に `web-sys`・`js-sys`・`wasm-bindgen`・`fandhe-frontend-*` | 禁止（normal 依存の推移的閉包） | `EVT-23` |

TASK-24 が置く `cargo metadata` の検査は、上表の禁止を機械的に確かめる。

- 許可の一覧は、web → pack の dev 辺だけを明示的な allowlist にする
- `cargo` の失敗（空の出力）を「依存なし」と読まず、失敗として扱う（`prototype/tools/check-deps.sh` の方式）

## 5. 不変条件

依存の向きとは別に、次を規則として固定する。

1. contract・core は DOM・表示基盤を知らない（`EVT-23`）。web は `Session` の操作と `Projection` を呼ぶ
2. web の glue は状態を変えない。状態を変える経路は `Component::update` を通る `apply` だけである。glue は
    表示の文字列を読まない（例外と検査は TASK-343 と web 側の試験が持つ。本文書では再定義しない）
3. contract・core は教材固有の知識を持たない（`W-22`）。特定の教材の固有名を型・定数・識別子に含めない。
    この版では文書上の規則であり、語彙の機械検査は TASK-48 などが持つ。この版で自動検査があるものとして扱わない
4. 教材パック・履歴・IR は未信頼入力として扱う。読み込みは pack と core の境界で行い、上限と拒否理由は
    contract の定数と型を使う（TASK-199）
5. 表示文字列（文言）は状態・イベントの決定に使わない（`EVT-2`・`W-24`）
6. 教材データを HTML に埋め込む箇所のエスケープ、利用者入力から URL を組み立てて取得する箇所の宛先制限は、
    web 層の責務とする（コードを書く後続タスクの確認観点。`.claude/rules/security.md`）

## 6. 公開 API の型の置き場

### 判定規則

1. 2 つ以上の責務が参照する純粋なデータ・定数・版・スキーマ・表は contract に置く
2. 状態を進める入口と、その入出力に現れる型（`Input`・`Control`・イベント・`RunState`・`Projection`）は core に置く
3. 表示部品・glue・DOM に触れる型は web に置く
4. 作成ツールの出力形（診断・CLI）は pack に置く

### 置き場の表

| 型・規則 | 置く crate | 出所タスク |
|---|---|---|
| `Session`、`apply(状態, 入力) → (新しい状態, イベント列)`、`Input`・`Control`・イベント（`Changed`・`Refused`・`Judged`・`HintOpened` など）、Step の状態、`Projection`、宛先、世代、`RunState` | core | TASK-204 |
| 状態変化の提供元の境界（トレイト） | core | TASK-187 |
| IR の型、`ir_format`、加算の規則 | contract | TASK-41・TASK-60 |
| 判定結果 6 種、拒否の分類、次の行動、字句化の結果の型 | contract | TASK-256 |
| キー判定表 `decide` の入出力と表 | contract | TASK-257・TASK-281 |
| 案内レベルとヒントの値の型（`Control::OpenHint`・`HintOpened` は core） | contract | TASK-290 |
| 学習目標の確認の記録の型 | contract | TASK-291 |
| 履歴の 4 軸の版の型、判定の結果の型、履歴の包みの型、保存先の口と拒否理由コード | contract | TASK-173・TASK-309・TASK-310 |
| 未信頼入力の上限の定数と拒否理由の型 | contract | TASK-199 |
| `content_hash` の求め方の仕様・サンプル・計算の関数（`content_hash::content_hash`） | contract（pack と core・web が同じ関数を呼ぶ。TASK-180 の本文は計算を pack と書くが、ブラウザで開く経路が履歴と現在の IR のハッシュを比べる（TASK-174）ため、pack だけに置かない） | TASK-180 |
| 診断の型（`Diagnostic`）、診断コード（`E-*`）の一覧、CLI の外部仕様 | pack（`E-LIMIT-*` の拒否理由の型は contract） | TASK-195・TASK-399 |
| Component の口、action の名前と宛先の形、操作プロファイル、glue の許される操作、左のビューの行データ、右のエリアの識別子、枠の区画と通知の口、アクセシビリティの構造 | web | TASK-343・TASK-467・TASK-431・TASK-511・TASK-366 |

- 本表は「どの crate に置くか」だけを固定する。型の中身は各契約タスクが固定し、本文書は型を再定義しない
- `Input`・`Judged` の位置づけは、公開 API の契約（TASK-204）を正とする。TASK-256 の判定の型はそれに従う
- web 側の契約（TASK-343 など）の型は wasm 専用にしない。契約適合試験を native で流せるようにする

## 7. feature

| feature | crate | 内容 |
|---|---|---|
| `test-hooks` | web | 試験用の読み出し口（`debug_*` の公開関数と `Action::Raw` など）。既定は無効。配信する設定では付けない。ブラウザ試験のビルドだけが有効にする（切り替えは、ビルドスクリプトが環境変数で行う。スクリプトは TASK-25 以降が `scripts/` に置く） |

- feature は加算的に保つ（有効にして既存の挙動を壊さない）
- contract・core に、I/O や表示基盤を引き込む feature を作らない
- `prototype/` の結線確認用の `hello` feature と、`Greeting`・`Input::Click`（結線確認用の型）は製品に不要なので
  持ち込まない
- wasm32 向け clippy の組み合わせは「feature なし」と「`test-hooks`」の 2 通りである（BUILD-5。TASK-24・TASK-33 の入力）

## 8. リポジトリの配置

```text
.
├── Cargo.toml              # ワークスペース（TASK-24）
├── crates/
│   ├── contract/           # fandhe-workshop-contract（仮称）
│   ├── core/               # fandhe-workshop-core（仮称）
│   ├── pack/               # fandhe-workshop-pack（仮称）+ bin fandhe-pack
│   └── web/                # fandhe-workshop-web（仮称）
├── testdata/               # 試験データの置き場（TASK-38）
├── template-workshop/      # 雛形の置き場（仮称。TASK-769。初期は作らない）
├── docs/
│   ├── architecture.md     # 本文書
│   └── spec/               # private の submodule
└── scripts/                # 処理の実体（既存。Makefile は薄い入口）
```

- `crates/` を使う。このリポジトリ自体がエンジンなので、`prototype/` の `engine/` の階層は要らない。各 crate の中は
  Cargo 標準の `src/`・`tests/` とする
- `testdata/`: 2 つ以上の crate が使う試験データと、`prototype/`・`03-poc/` から写した試験データを置く。
  出所を `testdata/README.md` に記録する（TASK-38）。リポジトリの外への相対参照を持たない（BUILD-14 の前提）。
  1 つの crate だけが使う小さな入力は、その crate の `tests/` に置いてよい。契約のサンプルは `testdata/contract/` に置く
  （TASK-205。参照先は `docs/engine-api.md` の 7 節）。golden の置き場の細目は TASK-38 が決める
- `template-workshop/`: 雛形はエンジン repo 内のディレクトリである（`D-4`）。構成の定義と適合の検査は TASK-769 が
  固定する。3 repo への移行の時期は未回答（`Q-24`）で、本文書では決めない
- シェルの処理は既存規約どおり `scripts/` に置く（`.claude/rules/makefile.md`）

## 9. ビルド・検査が前提にする設定

- `default-members` は contract・core・pack とする。web の試験は `--workspace` で流す
- web の native 試験（`rlib`）は、契約適合試験を流すために wasm 専用にしない
- wasm32 向け clippy は feature なしと `test-hooks` の 2 通りで流す
- 依存の版は固定する。固定は TASK-24・TASK-26 が持ち、本文書は版を決めない（`.claude/rules/security.md`）
- Rust 導入時の dev プロファイル・`target/` の掃除は `.claude/rules/rust-build-profile.md` に従う

## 10. TASK-24 への受け渡し

TASK-24 は、本文書だけでワークスペースを作れる。作るものの確認項目は次のとおり。

- 4 crate の空の骨格（名称は仮称のまま。種別は第 3 節）
- `resolver = "2"`、`members` は 4 crate すべて、`default-members` は contract・core・pack
- 依存の向きの検査（第 4 節）。contract・core から web への依存を足すと失敗すること
- web → pack の dev 辺だけを allowlist にしていること
- `cargo` の失敗を「依存なし」と読まないこと
- `test-hooks` feature（web）と、wasm32 向け clippy の 2 通りの組み合わせ
- `.editorconfig` の `[*.rs]` 追加と dev プロファイル（`.claude/rules/rust-build-profile.md`）

## 11. 検討した代替案

「契約を表す crate」の解釈は、spec の記述が 2 通りに読めたため、検討した。

- TASK-204 は「core crate（契約を表す型と公開 API）」と書く
- 他の複数のタスクは「契約を表す crate」「契約の crate（仮称）」と書き、「core 系 crate」と区別している
- `prototype/` は 3 crate（core・pack・web）で、IR・履歴・上限・判定は `core` のモジュールだった

採用: crate は 4 つとする。振る舞いを持たない共有の契約を `contract` に置き、`Session`/`apply` と入出力型は
TASK-204 のとおり `core` に置く。

- 「契約を表す crate」という別名の多用と、TASK-204 の「core」を、両方満たす唯一の読みである
- 契約の型だけに依る側（pack の IR 検査、保存復元、web の表示部品）が、進行管理の実装に結合せず並列に作業できる
- core の依存を contract のみに保ち、`EVT-23` の検査対象を contract と core の 2 つに限定できる

却下した案は次の 2 つである。

| 案 | 却下理由 |
|---|---|
| 3 crate のまま、契約を `core` の `pub mod` に置く（`prototype/` の構成） | 「契約を表す crate」と「core 系 crate」を区別する記述が多く、並列作業時に契約の変更と実装の変更が同じ crate で衝突する。ただし移行は機械的で、contract の公開パスを core から再エクスポートしない規則にしておけば、後戻りもできる |
| 契約ごとに crate を分ける（IR・履歴・判定など） | crate が増えすぎ、TASK-24 の雛形と検査が重くなる。契約の版は crate 内のモジュール単位で管理できる |

## 12. 未決・確認事項

- 第 11 節の採否は、spec の記述からの推論を含む。推論であり、レビューでの採否確認を求める（TASK-23 の承認点）
- `default-members`: TASK-24・BUILD-1 は「core・pack」と書くが、本文書は contract を加える。contract の試験を素の
  `cargo test` で流すためである。spec の文言との意図的な差分で、spec 側を「web を除く全 crate」と読み替える修正を提案する
- `content_hash`: 計算の寄せ先は contract に確定した（2026-10-15、TASK-180 1/2）。TASK-180 の本文の「pack 系 crate」との差を、spec 側で読み替える修正を提案する。SHA-256 は contract の中の小さな実装で、外部依存は足していない（第 3 節は不変）
- ブラウザ試験（Node）の置き場は本文書で固定せず、TASK-25 以降に委ねる
- 3 repo への移行の時期は未回答（`Q-24`）
- 依存の向きの機械検査は、この版では未実装である（TASK-24 が実装する。Rust 未導入のため実行していない）

## 13. 変更履歴

| 版 | 日付 | 変更 | 理由 |
|---|---|---|---|
| 1 | 2026-10-12 | 初版 | TASK-23。crate の境界・依存の向き・型の置き場を契約として固定する |

- 2026-10-15: 第 6 節・第 12 節の `content_hash` の寄せ先を、未決から contract に確定した。第 12 節が TASK-180 に委ねた未決事項の確定で、境界・依存の向き・外部依存は変わらないため、構成設計の版は 1 のまま据え置く
- 2026-10-15: 第 8 節が TASK-205 に委ねた契約のサンプルの置き場を `testdata/contract/` に確定した。境界・依存の向きは変わらないため、構成設計の版は 1 のまま据え置く
