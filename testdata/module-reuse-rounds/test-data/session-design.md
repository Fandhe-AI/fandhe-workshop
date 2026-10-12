# 実体ごとの入口の写し・チェックポイント・下書き・ヒントの使用回数の設計（実装の前に固定）

PoC-4 の実装（`session.rs` ほか）の前に固定する設計です。PoC-1 の F-7（入口の写しの置き場所を PoC-4 と PoC-6 で決める）への答えの案で、結果は `../README.md` の「設計への示唆」に書きます。固定の後は書き換えません。足りないと分かったものは、別ファイル（`session-design-addendum.md`）に足します。

## 1. 切り身（実体の状態のうち、実体ごとに分かれるもの）

- **根の実体（root）**: Step の実体 ID（`uid`）または実体 ID（`scope`）の、最初の `/` までの部分。`idea-cycle@round2/st-2-3` の根は `idea-cycle@round2`。章の Step（`main/...`）の根は `main`。
- **呼び出しの Step の持ち主**: 章にある `use` の Step（例: `main/r1-impl`）は、呼び出した実体（`issue-to-merge@round1`）の根に属する。`Ir.instances[].caller` が `uid` と一致する実体があれば、その実体の根。なければ、`uid` の根。
- **切り身（Slice）**: 根 R について、次のものを集めた値。
  - `status`・`choices`: 持ち主が R の `uid` の項目
  - `flags`: 実体 ID の根が R の項目
  - `hints`（`hints_opened`）: 持ち主が R の `uid` の項目
  - `draft`: R の入力の下書き
- **共有（切り身に入れないもの）**: `world`（教材全体で 1 つ）、`global_flags`、`generation`、`ir_format`、`guidance_level`、そして位置を表す `current`・`busy`・`pending_clarification`・`completed`。
- **混ざり**: 操作が、その操作と無関係な根の切り身を変えること。無関係な根とは、操作で起きたすべてのイベントの宛先 Step の根、`Restored` の持ち主、移動先 Step の根、操作の前後の `current` の根、のどれでもない根。

## 2. ヒントの使用回数

- 語彙 v1 の「ヒントの使用回数は、実体ごとに持つ」を、**Step の実体 ID を鍵にする**形で実装する（`uid` は実体 ID を含むので、実体ごとに分かれる。PoC-3 の予約の欄の鍵は実体 ID と書かれていたが、PoC-14 が「Step ごとに 3 段目のヒントを開いたか」を数えるため、Step の単位にする）。
- 操作 `Control::OpenHint`: 現在の Step について、現在の案内レベルで出せるヒントの数（`guidance::visible_hints`）まで 1 つずつ開く。上限なら、または現在の Step が無ければ、`Refused`（理由 `NoHint`）。世代は変えない。
- ヒントの使用回数は**増えるだけ**で、停止・チェックポイントの復帰では戻さない（学習者の行動の記録のため）。
- イベント `SessionEvent::HintOpened { at, opened, total }`。右のログの行にはしない（`Projection` は無視）。

## 3. 入力の下書き

- 下書きは根ごとに持つ。`Session.draft` は「現在の Step の根の下書き」で、型も名前も変えない（web を変えないため）。
- 根が変わる操作（入力で次の根へ進む・移動・復帰・停止・開始）の後で、`draft` を元の根の置き場（`drafts`）へ退避し、新しい根の下書きを取り出す。
- 下書きは実行の状態（`RunState`）に含めず、停止・復帰では戻さない（PoC-1 のまま）。

## 4. 入口の写しとチェックポイントの置き場所

- **入口の写し（`EntryCopy`）**: Step に入った時点の `RunState` の全体の写しと、その時点のログの長さ。**現在の Step の分だけを 1 つ**持つ（処理中になれるのは現在の Step だけなので、Step ごとの写しを溜めない）。Step に入るたびに置き換える。置き場所は `Session`。
- **チェックポイント（`Checkpoint`）**: `RunState` の全体の写し、入口の写し、保存の時点のログの長さ、**持ち主の根**（保存の時点の現在の Step の根。現在の Step が無ければ空）。置き場所は `Session`（番号は通し）。
- 全体の写しを持つが、**戻すのは持ち主の根の切り身と位置だけ**にする（次の 5 節）。他の根の切り身が写しの後に変わっていても、戻らない。

## 5. 戻し方（停止とチェックポイントの復帰。同じ関数）

戻す元の写し S、戻す範囲の始まり since（ログの位置）、持ち主の根 R に対して次を行う。

1. `status`・`choices`: 持ち主が R の項目を、現在の状態から取り除き、S の持ち主が R の項目を入れる。
2. `flags`: 根が R の実体の項目を同じように置き換える。
3. `hints`: 戻さない（2 節）。
4. 位置（`current`・`busy`・`pending_clarification`・`completed`）と `guidance_level`: S のものにする。
5. `global_flags`: 戻さない。
6. 世界状態: ログの since 以降にある `Changed` のうち、宛先 Step の根が R のものの経路について、世界状態を S の値（無ければ取り除く）にする。他の経路は変えない。
7. 世代: 現在の世代 + 1。
8. 停止: S は入口の写し、since は入口の写しのログの長さ、R は現在の Step の根。復帰: S はチェックポイントの写し、since はチェックポイントのログの長さ、R は持ち主の根。復帰の `Changed` の宛先 Step は、S の `current`（無ければ `<R>/`）にする。
9. 復帰では入口の写しを、チェックポイントが持つ入口の写しに置き換える（世代は新しいものにする）。停止では、戻した後の状態で入口の写しを作り直す。

`Restored` に `owner`（R）を足す。`Projection` は、復習の項目のうち根が `owner` のものだけを、保存の時点までのログから作り直したものに置き換える。他の根の項目は残す。

## 6. 新しいルートの開始（D-24）

- 操作 `Control::Begin(開始 ID)`: 閉じた実行の続きに、新しい開始点から始める。世界状態・Step の状態・フラグは引き継ぐ。
- `Engine::begin_at` を、世代を 1 上げた状態の写しに対して行う。開始条件（EP-9）を満たさない、または開始 ID が無いときは、状態を 1 つも変えず（世代も変えず）、`Refused`（理由 `StartUnmet`・`UnknownStart`。`StartUnmet` には文面の ID を持たせる）を記録する。
- 成功のときは `SessionEvent::Begun { at, start, new_generation }` に続けて、エンジンのイベントと `Changed`（原因 `Start`）を記録し、入口の写しを作り直す。
- `Session::new` は、開始条件を満たさないとき `EngineError::StartUnmet` を返す。

## 7. 追加する型

- `Control::OpenHint`、`Control::Begin(String)`
- `ControlKind::OpenHint`・`ControlKind::Begin`、`RefuseReason::NoHint`・`StartUnmet`・`UnknownStart`
- `SessionEvent::HintOpened`・`SessionEvent::Begun`、`SessionEvent::Refused` に任意の `rejection`、`SessionEvent::Restored` に `owner`
- `Cause::Start`
- `Slice`、`Session::root_of`、`Session::slice`、`Session::roots`、`Session::drafts`（読み取り用）
- `LogItem` は増やさない（右のログの表示は変えない）。web は、`ControlKind`・`RefuseReason` の全件を網羅する表示の分岐に 2 行ずつ足すだけ。
