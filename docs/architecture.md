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
| `fandhe-workshop-pack` | lib + bin `fandhe-pack` | 教材パックの読み込み・検査・コンパイル・教材テストと、作成者向け CLI | core、contract、`serde`・`serde_json`・`toml`・`yaml_serde`・`saphyr-parser`（アンカー・タグの拒否。TASK-57 2/2）・`pulldown-cmark`（native のみ） |
| `fandhe-workshop-web` | lib（`cdylib`・`rlib`） | `fandhe-frontend` の Component 実装と glue。wasm32 向け。native でも `rlib` として試験できる | core、contract、`fandhe-frontend-*`（wasm-full 0.46.0。TASK-26。headless-ui 0.69.2 は `tree_view::item_text`、wireframe-ui 0.52.0 は `rich_text` のため。TASK-473）、`wasm-bindgen`、`web-sys`（`cfg(target_arch = "wasm32")` のみ） |

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
| 受講者の OS の推定（`learner_os::estimate`・`LearnerOs`。User-Agent の部分一致の純関数で、履歴・`Session` には入れない）。修飾キーの語と案内文（`modifier_key`）と `key-hint` の束縛は web が持つ。受講者が選ぶ欄（`os-select`・`data-action-change="os"`・`HelloAction::SetOs`）で表示を切り替えられ、選んだ値は保存しない。JUDGE-28 | core（推定）・web（表示） | TASK-285 |
| 秘密情報らしい文字列の検出（`secret_scan`。固定の 15 形（SAFE-27 の 10 形と SAFE-30 の 5 形）・値を持たない `Finding`。純関数で、保存の直前の警告は web の `secret_notice`・`host`・`persist` が、書き出しの関門（`EngineHost::export_live`）と、警告の選択（「このまま保存する」の確認済みの除外・「この入力を残さない」の読み直し。確認済みは形と指紋だけを持つ）は web の `host` が持ち、教材を取り込むときの検査は pack が再利用する）。SAFE-27 | core | TASK-164 |
| 履歴の読み込みの段 1〜8 の入口（`history_load::load_history`。段 1〜3 は `parse_history_json`、段 4 の 3 条件・型・件数は `parse_history_envelope`、段 5 の後半〜段 8 は `open_history` を呼ぶ） | core | TASK-331 |
| 履歴を開く可否の判定（`history_open::open_history`）・開く側の上限 `OpeningCaps`・エンジンが読める語彙の上限 `VOCAB_MAX`（pack の `vocab::LATEST` との一致は pack の試験が見る） | core | TASK-174 |
| 開いた履歴から実行環境を作る入口（`history_load::load_session`。判定は `load_history` に任せ、開いたときだけ `Session::from_state` を呼ぶ。状態から実行環境を作り直す処理は保存復元の担当のため core に置く） | core | TASK-330 |
| 操作を `ops` に記録しながら進める実行環境（`recorded_session::RecordedSession`。`Session`・`OpsRecorder`・最初の開始点を束ね、下書きの写しとの同期も core で行う。`ops` が 0 件なら保存用の包みを作らない。SAVE-28） | core | TASK-319 |
| 起動の分岐（`history_boot::boot_session`。保存済みが無い・未操作なら新しく始めて世代を上げず、開けたら再読み込みとして復元し、開かなければ初期状態で始める。判定は `history_load` の段 1〜8 に任せて複製しない。再読み込みを呼ぶため web ではなく core に置く） | core | TASK-319 |
| IR の型、`ir_format`、加算の規則 | contract | TASK-41・TASK-60 |
| 判定結果 6 種、拒否の分類、次の行動、字句化の結果の型 | contract | TASK-256 |
| キー判定表 `decide` の入出力と表（`key_decide`） | contract | TASK-257・TASK-281 |
| 案内レベルとヒントの値の型（`Control::OpenHint`・`HintOpened` は core） | contract | TASK-290 |
| 入力方式 5 種と待機（`wait`）の宣言の型と `text` の判定の純関数（実行時の判定は core が呼ぶ。診断は pack） | contract | TASK-44 |
| 確認 4 種の宣言の型と、受付と達成の分け方の純関数（実行時の判定は core、診断は pack、表示は web） | contract | TASK-47 |
| 差し込みの枠の閉じた集合・構文の読み取り・差し込みの純関数（`slot`。実行時の値の組み立ては core〔TASK-267〕、診断と IR を総なめにする数え上げは pack） | contract | TASK-46 |
| 学習目標の確認の記録の型と導出（`lo_check`） | contract | TASK-291 |
| 保存先の口 `HistoryStore`・配信の区間・履歴の鍵の生成規則と読み戻し（`storage`。鍵に教材の版・内容を混ぜない） | contract | TASK-310 |
| 履歴の 4 軸の版の型、判定の結果の型、履歴の包みの型、保存先の口、拒否理由コードと判定の順序（`load_reason`。順序は `HISTORY_LOAD_ORDER`、`STORAGE_CONTRACT_VERSION` 1） | contract | TASK-173・TASK-309・TASK-310 |
| 未信頼入力の上限の定数と拒否理由の型 | contract | TASK-199 |
| `content_hash` の求め方の仕様・サンプル・計算の関数（`content_hash::content_hash`） | contract（pack と core・web が同じ関数を呼ぶ。TASK-180 の本文は計算を pack と書くが、ブラウザで開く経路が履歴と現在の IR のハッシュを比べる（TASK-174）ため、pack だけに置かない） | TASK-180 |
| 状態変化の語彙（要素・属性・フラグ）の効果の型と `Value` からの解釈は contract、1 語ごとの適用は core、読み込み時の検査と診断は pack（spec の TASK-43 は「pack crate」と書くが、判定規則に従い分ける。IR の型は `Value` のまま変えない。進行 `processing` は contract が解釈し core が適用、`when` は core に評価器を注入する形で条件の語彙は TASK-42 が与える。TASK-43 2/2） | contract・core・pack | TASK-43 |
| 条件の語彙（状態・世界状態・論理。`step`・`flag`・`world_has`・`world_attr`・`count`・`chose`・`all`・`any`・`not`）の型と `Value` からの解釈は contract、評価と `ConditionEval` の実体は core、読み込み時の検査と診断は pack（IR の型は `Value` のまま変えない。`param`・`clarifying` も型と評価を持ち、`param` は `VocabConditions` が借りる IR の実体の引数、`clarifying` は `Slice.clarifying`（`CONTRACT_VERSION` 5）を読む。TASK-42 1/2・2/2） | contract・core・pack | TASK-42 |
| 操作 1 件の世界状態の差分から変更前の値つきの `Changed` を作る記録（`changed`。契約の版の対象外で、`apply` が呼ぶ。復習の導出は TASK-215 2/2） | core | TASK-215 |
| コマンドの字句化の処理と宣言との照合（`command_lex::lex_command`・`command_judge::judge_command`。結果の型は contract の `judgment`、接頭辞の分けは contract の `profile`、宣言の型は contract の `input_spec`） | core | TASK-262 |
| 選択肢・画面操作・承認の判定の処理（`select_judge::judge_select`・`judge_confirm`。LLM なし。状態の変更と `Judged` の発行は含まない） | core | TASK-263 |
| 入力・進行の判定結果の統合と `Judged` の材料（`judge::judge_input`・`judge_tick`・`judged_event`。拒否・問い返しの文面の ID と次の行動を IR から解決する。結果の型は contract の `judgment`。`Op::Input` の `apply` への結線は `accept::on_input` が持ち、判定を 1 回だけ行ってその結果に従う。`Op::Tick` への結線は含まない） | core | TASK-267 |
| 承認の「直す」の対象の一覧と、対象なしの「直す」の規則（`select_judge::revise_targets`） | core | TASK-264 |
| 拒否の文面の解決と空でない次の行動（`rejection_text::resolve_rejection`・`rejection_of`。差し込みは含まない） | core | TASK-264 |
| 拒否・問い返し・注記の文面への実行時の値の差し込み（`text_fill`。値の組み立て・近い候補の編集距離・次の行動と文面の充填。`judge` が結線して `Judged` の次の行動を充填する。枠の書式は contract の `slot`。画面への表示は含まない） | core | TASK-274 |
| コマンド入力の規則の照合と問い返しの判定（`command_rule`。状態の変更・`Judged` の発行・問い返しの待ちの保持は含まない） | core | TASK-272 |
| 学習目標の確認の記録に当てる、最初の回答の正答だけを自力に数える規則（`lo_first_answer`。GUIDE-18。型は contract の `lo_check`。履歴から観測への写像と他の LO の数え方は含まない） | core | TASK-305 |
| 処理中の停止と入口の写しへの復帰（`stop`。公開しない。実体ごとの戻り方・入口の写しの更新は含まない。処理中でない停止は `session` が取り消しの入力として `accept` へ渡す） | core | TASK-211・TASK-212 |
| 入口の写し（処理を始めた受付の時点の `run`）を状態に書かず、要る場面（処理中の停止・上限で止めた後の戻り）でだけログから導くこと（`entry_derive`。公開しない。末尾から処理中の区間を逆にたどり、`Changed` の変更前の値で世界状態を戻す。`Restored` では保存の時点へ起点を移す。一意に決められなければ導かない。フラグ・選んだ選択肢は戻さない） | core | TASK-160 |
| Step の移動（`go_to`。公開しない。移動先の `on_enter`・入口の写しの作り直しは含まない） | core | TASK-213 |
| 1 回の操作で起きる内部の遷移（`next`・`use` の `enter`・`final` の `exit_to`）のたどりと、1,000 回の上限での停止（`transit`。公開しない。再帰を使わずループでたどり、上限に達したら `Warning` と専用の `Event::TransitionLimitReached` を 1 件ずつ残し、`session::apply` が操作の前の `run`・入口の写しへ戻す（`busy` は空、世代は減らさない。SAFE-35）。受講者向けの見出し・通知欄・まとめ表示は web の `log_lines`・`limit_notice` が持つ。実体ごとの状態・`on_enter` の効果は含まない） | core | TASK-160 |
| チェックポイントの保存と復帰（`checkpoint`。公開しない。実体ごとの戻り方は含まない。復習・変化の印の作り直しは `projection` が導き、`apply()` を通した試験は TASK-214 2/3 で追加済み。復帰前の進行の無視と処理中の再開は TASK-214 3/3 で試験済み） | core | TASK-214 |
| 受け付けたうえで止まる場面の判定（`command_accept`。規則の受付の効果の読み出しと適用。`Judged` の発行・`session::apply` への結線は含まない） | core | TASK-273 |
| コマンドの表と Step の状態によるコマンドの拒否の分類（`command_class`。未知・順序違い・教材未対応・この Step では使わない。`judge::judge_input` が結線して使う。`Judged` の発行は含まない。引数不足と引数の形は `command_judge`・`judge` が返し、4 場面の教材テストは pack の結合試験に置く） | core | TASK-268・TASK-269 |
| 処理中の入力の `busy` の分類（`busy_class`。`Judged` の発行・文面の差し込みは含まない） | core | TASK-209 |
| 受講者の操作を履歴の `ops` に追記する記録（`ops_record`。`SessionOp` は contract の `HistoryOp<Op>` の別名。契約の版の対象外。再読み込みは含まない） | core | TASK-315 |
| `ops`・写し・4 軸の版を履歴の包み（`SessionEnvelope` = contract の `HistoryEnvelope<Op, SessionState>`）に組み立てる処理と、下書きの記録と写しの `drafts` の同期（`history_envelope`。契約の版の対象外。適用し直しによる復元・読み込み・書き出しは含まない） | core | TASK-315 |
| 保存の前に、直列化したバイト数と `ops` の件数が読み込みの上限を超えないかを判定する純関数（`history_limit`。契約の版の対象外。記録は止めず、超える包みは保存しない仮の規則。文面と上限の値は持たない） | core | #1414 |
| `ops` を初期状態に順に適用し直す復元と送信の分類（`history_replay`。契約の版の対象外。写しとの照合と `replay_mismatch` への対応づけ（`verify_envelope`）を含み、再読み込みは含まない） | core | TASK-316 |
| 確認の操作による達成と外れた指し先（`confirm`。公開しない。達成の効果と `next` の無条件の先頭の辺への遷移まで。章の完了・`on_enter`・`Judged` は含まない） | core | TASK-207 |
| 診断の型（`Diagnostic`）、診断コード（`E-*`）の一覧、CLI の外部仕様 | pack（`E-LIMIT-*` の拒否理由の型は contract） | TASK-195・TASK-399 |
| Component の口、action の名前と宛先の形、操作プロファイル、glue の許される操作、左のビューの行データ、右のエリアの識別子、枠の区画と通知の口、アクセシビリティの構造 | web | TASK-343・TASK-467・TASK-431・TASK-511・TASK-366 |
| ホストの Component（`EngineHost`）と部品の登録簿への振り分け、3 つの表示（ログ・左の成果物・復習）の部品（`host`・`engine_parts`。同じイベント列から導いた値だけを読む。`raw` の action・`raw-trigger`・試験用の教材の読み込みと差し替え（`load-fixture:` の `answer`・`approval`）は `test-hooks` の背後だけ。履歴ファイルの読み込み `EngineHost::open_history` は `load_session` を呼び、開いたときだけ `Session` を差し替える。操作は `RecordedSession` を通して `ops` に記録し、起動時は `EngineHost::boot` が `boot_session` を呼び、離れる直前に保存する値は `history_to_save`） | web | TASK-347・TASK-330・TASK-319 |
| 右のログの行のモデルと描画（`log_lines`。TASK-431 の行の構造に準拠。入力（承認の操作の行は `承認の操作: 直す（対象: …）` の文言。TASK-447 2/2）・進行・停止・再読み込み・拒否・問い返しを結線し（拒否・問い返しの文面は `HostModel` が `apply_engine` の直後に core の `filled_text` で充填して控える。TASK-437 1/2・2/2）、応答は TASK-435・TASK-438。`log-list` の引き継ぎは TASK-438 または TASK-517） | web | TASK-434 |
| 状態表示の導出（`run_status`。待機中・処理中・停止。Esc と停止ボタンは `host` が処理中のときだけ `Control::Stop` に結ぶ。TASK-436 2/2。描画は `engine_parts` の `LogPart` の仮の位置） | web | TASK-436 |
| Step の状態表示の導出（`step_status`。受付済み（確認待ち）・達成。現在の Step の状態とログの末尾から導き、確認を省く Step の受付と同時の達成も見せる。描画は `engine_parts` の `LogPart` の仮の位置。TASK-435 1/2） | web | TASK-435 |
| 選択肢のボタン（`offers`。確認が `answer` の Step が確認待ちのとき、表示名のボタンを `Offers` の部品として描く。クリックは `data-action="choose"` で `decode_action` が `Op::Input(Input::Choice)` に結び、照合と判定は core。配置は TASK-517。承認のボタンも同じ部品で出す: 入力方式 `approval` の入力待ちと、確認が `approve`（または無し）の確認待ちで、はい・いいえ・直す、入力待ちでは規則に書かれた対象ごとの「直す（対象: …）」を加える。対象は core の `revise_targets` だけから得る。クリックは `data-action="decide"`・`"revise-target"` で `Op::Input(Input::Approval)` に結ぶ。入力方式 `choice` の入力待ちでも宣言の選択肢を同じ口で出す: 選択肢は core の `choice_options` だけから得て、入力欄・送信は Step の方式では出し入れしない。教材全体の `pack.profile.input_field: hidden` のときは、`host` が教材を読んだとき（mount の前）に設定を渡し、`hello::HelloView` が入力欄・補完の候補の一覧・送信を DOM に出さず、下書き・送信・補完の action は状態を変えない。停止と選択肢・画面操作・承認・確認は出る。PACK-20。TASK-450 2/3） | web | TASK-445・TASK-447・TASK-450 |
| ツリーの位置の計算（`aria-level`・`aria-posinset`・`aria-setsize`）と、木の属性の組・親の `role=tree`（ART-9・ART-13。全項目を開いた表示で、開閉とキー操作は持たない） | web（`tree_attrs`・`artifact_view`） | TASK-472 |
| ファイルツリーのビュー（変種 (a)・(b)。(b) は木の属性・変化の印・headless-ui の `item_text`、(a) は wireframe-ui の `rich_text`。製品の結線は後続） | web（`file_tree_view`） | TASK-473 |
| カードのビュー（変種 (a)・(b)。(a) は wireframe-ui の `card_basic`・`tag`、(b) は自作の `article`・`ul` と headless-ui の `marker`。ラベルと状態は行データから受け取り、特定の教材の語を持たない。製品の結線は後続） | web（`card_view`） | TASK-477 |
| チェックの一覧のビュー（変種 (a)・(b)。(a) は wireframe-ui の `rich_text`・`tag` と自作の記号の `span`、(b) は `li` 自身が印の宿主で headless-ui の `marker`。状態は記号つきの文字。状態・ラベル・補足は行データから受け取り、特定の教材の語を持たない。製品の結線は後続） | web（`check_view`） | TASK-478 |
| アプリのデモのワイヤーのビュー（変種 (a)・(b)。(a) は wireframe-ui の `text`・`tag`・`rich_text`・`input`・`button`・`card_basic`、(b) は headless-ui の `field`（読み取り専用）と自作の `span`・`button` で `role`・`aria-` は出さない。タブの横並びは外部の CSS で補う。状態の文字は固定で、`Active`・`Disabled`・`disabled` は行データで切り替えない。製品の結線は後続） | web（`app_demo_view`） | TASK-479 |
| タスクツリーの変種 (a)・(b) の本体（(b) は木の属性、(a) はチェックボックスの代役・深さのクラスで木の意味なし。どちらも状態のバッジで、状態は記号と文字の両方。ART-1・A11Y-22） | web（`task_tree`） | TASK-474 |
| 差分のビューの変種 (b) の本体（`pre`・行番号・記号の列。行の印と差分の記号の 2 層を別の列に出す。ART-1・ART-8）と、対照専用の変種 (a)（`diff_item_a`。製品では選べない） | web（`diff_view`） | TASK-475 |
| 文書のビューの変種 (b) の本体（本文の木 `Block` を行の中身に結び、包みの中に木の要素を 1 つ描く。実行時に Markdown は解釈しない。ART-1・ART-7）と、対照専用の変種 (a)（`doc_item_a`。平文の行に落とす。製品では選べない） | web（`document_view`） | TASK-476 |
| 世界状態とイベント列から、左のビューの行の前後の組と印の数を導く処理（`artifact_rows`。属性の読み方とビューの選別は呼び出し側が持つ。増分の口 `PhaseWorlds` は `Projection::extend` と同じ契約でログの全体を受け、一括と同じ結果になる） | web（`artifact_rows`） | TASK-468 |
| 項目 1 件を変化の印つきで作る部品（`artifact_mark`。記号・文字・`data-mark` の 3 手段、変種 (b) は根・(a) は外枠が宿主、深さのクラス `d0`〜`d6`。見た目は同一オリジンの外部ファイル `static/app.css` で、`<style>` 要素と `style` 属性は使わない。指せる行の 4 属性は `pointed_item` で付け、宛先は呼び出し側が `PointOf` で注入する。クリックは host の `decode_action` が `PointTarget::decode` で受け、`Input::Ui` として core に渡す） | web（`artifact_mark`） | TASK-469・TASK-480 |
| 文書のブロックの突き合わせ（`artifact_doc_rows`。前後のブロックを LCS で突き合わせ、変更を `-` と `+` の組にして `~` は作らない。ID は「文書の経路 + `#` + 位置」。ブロックの型は型パラメータで受け、`contract::doc::Block` との結びは TASK-476。上限超過は `TooLarge`） | web（`artifact_doc_rows`） | TASK-468 |
| 左の成果物の部品の結線（ビューの種類ごとの keyed list 7 本・選ばれていない list は空で `hidden`）と、ビューの選択（宣言の順・今の Step が宣言した種類だけを選べる。選択は Step ごとに持ち、移動すると先頭に戻る。action `select-view`。宣言が無い Step の既定は試作の仮置き） | web（`engine_parts`・`view_select`・`host`） | TASK-470 |
| 進行の間隔の定数（300ms。D-47。URL・教材から変えられない。core は時刻を持たない。EVT-16） | web（`tick_interval`） | TASK-229 |
| 履歴の保存先の実装（localStorage・IndexedDB）と選択の規則（切り替えの閾値・一方向の切り替え・二重の写しの優先。D-52。口 `HistoryStore` は contract） | web（`storage_select`・`storage_local`・`storage_idb`・`save_pending`・`leave_flush`。保存の直前の起動番号の照合は `boot_guard`。SAVE-12） | TASK-317・TASK-324 |
| 受講中の保存の結線（起動の前に保存先を開いて読み、`Promise` でマウントを待たせ、離れる直前の `pagehide` で `history_to_save` を書く。最初の変更まで保存しない。試験用の口の保存先・保留とは別に持つ。試験の設定だけ、起動の前に URL のクエリの完全一致（`?fixture=reload-points`・`?fixture=no-input`・`?fixture=ch0-3`）で mount の教材を選ぶ） | web（`persist`・`test_fixture`・`test_hooks`。wasm のみ） | TASK-319 |

- 本表は「どの crate に置くか」だけを固定する。型の中身は各契約タスクが固定し、本文書は型を再定義しない
- `Input`・`Judged` の位置づけは、公開 API の契約（TASK-204）を正とする。TASK-256 の判定の型はそれに従う
- web 側の契約（TASK-343 など）の型は wasm 専用にしない。契約適合試験を native で流せるようにする
- TASK-343 の契約（`parts`・`action`・`profile`・`glue_ops`・`list_policy`）の版は `parts::PARTS_CONTRACT_VERSION`（版 2）が持つ。アクセシビリティの構造の契約の版（`a11y::A11Y_CONTRACT_VERSION`）とは別に管理する。TASK-283 で版 2 に上げ、上限を超える貼り付けの切り捨てのための操作範囲（`beforeinput` の登録・選択位置の読み取り・キャレット・合成の `input`）を足した
- TASK-431 の契約（`terminal_area`。ログの行・状態表示・Step の状態の語・入力の口の識別子・観察点の制約）の版は `terminal_area::TERMINAL_AREA_CONTRACT_VERSION`（版 3）が持つ。`parts`・`a11y` の版とは別に管理する
- CLI の外部仕様（TASK-399・TASK-407・TASK-402・TASK-408。バイナリ名・コマンド一覧・引数の形・使い方・終了コード・診断 1 行・項目の書式・最終行・診断の出し先・`compile` の出力・`metrics` の出力・`vocab` の出力）の版は `cli_contract::CLI_CONTRACT_VERSION`（版 5）が持ち、文書は `docs/cli.md`。他の契約の版とは別に管理する
- 左の成果物ビューの契約（TASK-467。`artifact_view`）の版は `artifact_view::ARTIFACT_VIEW_CONTRACT_VERSION`（版 1）が持つ。上の 2 つの版とは別に管理する。TASK-467 2/2 で足した束縛点・指せる行の属性・変種の口も同じ版 1 の中にある

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
- 2026-10-15: 第 6 節に、状態変化の語彙（TASK-43 1/2）の型・適用・検査の置き場を追加した。境界・依存の向きは変わらないため、構成設計の版は 1 のまま据え置く
- 2026-10-15: 第 6 節の状態変化の語彙に、進行と条件つき（TASK-43 2/2）の型・適用・検査の置き場を追記した。境界・依存の向きは変わらないため、構成設計の版は 1 のまま据え置く
- 2026-10-15: 第 6 節に、操作 1 件の `Changed` の記録（TASK-215 1/2）の置き場を追加した。境界・依存の向きは変わらないため、構成設計の版は 1 のまま据え置く
- 2026-10-15: 第 6 節に、コマンドの字句化（TASK-262 1/2）の処理の置き場を追加した。境界・依存の向きは変わらないため、構成設計の版は 1 のまま据え置く
- 2026-10-15: 第 6 節に、選択肢・画面操作・承認の判定（TASK-263）の処理の置き場を追加した。境界・依存の向きは変わらないため、構成設計の版は 1 のまま据え置く
- 2026-10-15: 第 6 節に、判定結果の統合と `Judged` の材料（TASK-267）の処理の置き場を追加した。境界・依存の向きは変わらないため、構成設計の版は 1 のまま据え置く
- 2026-10-15: 第 6 節に、差し込みの枠（TASK-46 2/2）の純関数の置き場を追加した。境界・依存の向きは変わらないため、構成設計の版は 1 のまま据え置く
- 2026-10-15: 第 6 節に、承認の「直す」の対象と拒否の文面の解決（TASK-264）の置き場を追加した。境界・依存の向きは変わらないため、構成設計の版は 1 のまま据え置く
- 2026-10-15: 第 6 節に、コマンド入力の規則の照合と問い返しの判定（TASK-272）の処理の置き場を追加した。境界・依存の向きは変わらないため、構成設計の版は 1 のまま据え置く
- 2026-10-18: 第 6 節に、条件の語彙（TASK-42 1/2）の型・評価・検査の置き場を追加した。境界・依存の向きは変わらないため、構成設計の版は 1 のまま据え置く
- 2026-10-18: 第 3 節の pack の外部依存に `saphyr-parser`（形式 B のアンカー・エイリアス・タグの拒否。TASK-57 2/2）を追加した。pack は native のみで web には入らず、境界・依存の向きは変わらない（外部依存の追加の前例は TASK-473）ため、構成設計の版は 1 のまま据え置く
- 2026-10-20: 第 6 節に、秘密情報らしい文字列の検出（TASK-164 1/3）の置き場を追加した。境界・依存の向きは変わらないため、構成設計の版は 1 のまま据え置く
- 2026-10-21: 第 6 節の秘密情報の行に、書き出しの関門（TASK-164 2/3）は web の host が持つことを追記した。境界・依存の向きは変わらないため、構成設計の版は 1 のまま据え置く
- 2026-10-21: 第 6 節の秘密情報の行に、警告の選択（TASK-164 3/3。確認済みの除外と読み直し）は web の host が持つことを追記した。境界・依存の向きは変わらないため、構成設計の版は 1 のまま据え置く
- 2026-10-21: 第 6 節の秘密情報の行を固定の 15 形に直した（SAFE-30・TASK-165 1/2。追加の 5 形は core の `secret_scan` の中）。境界・依存の向きは変わらないため、構成設計の版は 1 のまま据え置く
