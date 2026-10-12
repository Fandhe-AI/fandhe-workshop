# 決定的な試験の定義（手順 3 の (i)〜(vii)、手順 4・5、縦断の確認、#20。実施の前に固定）

固定の日時と SHA-256 は `../env.md` に記録します。書き換えが要る場合は、別ファイル（`test-data-addendum/`）に足します。

## A. 共通の定義

- 教材: `pack/`（形式 B、語彙 v1＋v1.1＋v1.2）。実体: `idea-cycle@round1`・`issue-to-merge@round1`（章 `ch-round1`、開始 `s-r1`）、`idea-cycle@round2`・`issue-to-merge@round2`（章 `ch-round2`、開始 `s-r2`・`s-r2-fresh`）。
- 1 周目の模範解答の入力列と 2 周目の入力列は `pack/tests/playthrough.yaml` の `pt-r1`・`pt-rounds` と同じ。試験のコードは、同じ列を `Session` に流す（`Session::new("s-r1")`、1 周目の終わりの後で `Control::Begin("s-r2")`）。
- **ハッシュ**: SHA-256。入力は、正準の JSON（`serde_json::to_string`。鍵は `BTreeMap` の並び、空白なし）を UTF-8 にしたもの。
  - **世界状態のハッシュ**: `RunState.world` の JSON。
  - **切り身のハッシュ**: `Session::slice(根)` の JSON（`session-design.md` の 1 節）。
  - SHA-256 は試験のコードの中の小さな実装で計算し、既知の値（空文字列、`abc`）と `shasum -a 256` の出力で照合する。依存は足さない。
- 「1 周目の終了」: 1 周目の最後の入力（`st-7-4` の確認）の直後（`ChapterCompleted` が出た直後）。「2 周目の開始」: `Control::Begin("s-r2")` が成功した直後（まだ入力を送っていない）。

## B. 手順 3 の試験

### (i) 1 周目の完了フラグを立てても 2 周目は未着手のまま

1. 1 周目を終わらせる。`issue-to-merge@round1` のフラグ `stopped` が立っている（1 周目は `branch_demo=true`）。
2. この時点で、2 周目の実体（`idea-cycle@round2`・`issue-to-merge@round2`）の Step の状態がすべて `not_started`、フラグが空、`main/r2-idea`・`main/r2-impl` も `not_started`。
3. `Control::Begin("s-r2")` の直後も、2 周目の実体の Step の状態は、現在の Step（`idea-cycle@round2/st-2-1`）が `entered` で、ほかは `not_started`。フラグは空。
4. 2 周目を終わらせると、`issue-to-merge@round2` のフラグ `stopped` は立たない（2 周目は `branch_demo=false`）。1 周目のフラグ `stopped` は立ったまま。

### (ii) 2 周目のチェックポイント復帰が 1 周目の状態を変えない

復帰が戻すのは、チェックポイントの持ち主の根（保存した時点の現在の Step の実体）の切り身と位置だけ（`session-design.md` の 5 節）。同じ周回のもう一方の実体も、別の周回の実体も戻さない。

- (ii-a) 自然な順: 1 周目を終えて 2 周目を開始し、`idea-cycle@round2` の `st-2-4` まで進めて保存（番号 1）、2 周目を `issue-to-merge@round2` の `st-7-1` まで進めて復帰（番号 1）。復帰の前後で、1 周目の 2 つの実体の切り身のハッシュが変わらない。持ち主 `idea-cycle@round2` の切り身は保存の時点のものに戻り、位置も `idea-cycle@round2/st-2-4` に戻る。`issue-to-merge@round2` の切り身は復帰の前と変わらない（実体単位の復帰。周回単位ではない）。この「同じ周回のもう一方の実体が戻らない」帰結を記録する。
- (ii-b) 識別する順: (ii-a) の保存（番号 1）の直後に `Control::GoTo("issue-to-merge@round1/st-7-2")`（1 周目の待機の Step）へ移動し、進行を 1 つ送る（1 周目の切り身が変わる。`prs/pr-1-a` が `merged` から `open` を経て `ci-running` になる）。ここで 1 周目の切り身のハッシュ H1 を取り、番号 1 に復帰する。復帰の後の 1 周目の切り身のハッシュが H1 と等しい（巻き戻らない）。世界状態の `prs/pr-1-a` は `ci-running` のまま（復帰の持ち主 `idea-cycle@round2` は書いていない経路）。
- (ii-c) 逆向き: 1 周目の `issue-to-merge@round1/st-7-2` で進行を 1 つ送った所で保存（番号 1）し、1 周目を終えて 2 周目を開始し、2 周目の `idea-cycle@round2` の `st-2-4` まで進め、番号 1 に復帰する。復帰の後、2 周目の 2 つの実体の切り身が復帰の前と等しく（2 周目の進みが消えない）、持ち主 `issue-to-merge@round1` の切り身と位置が保存の時点に戻る。
- (ii-d) 比較（負の対照）: 全体を戻す実装（変異 X1）に替えると、(ii-b) と (ii-c) が失敗する。変異の確認は `property-test.md` の変異の表で行う。

### (iii) 1 周目の下書き・ヒントの使用回数が 2 周目に出ない

1. 1 周目の `issue-to-merge@round1/st-6-1` の位置で、`set_draft("下書き1")` し、`Control::OpenHint` を 1 回行う。
2. 2 周目の開始の後、`Session.draft` が空、2 周目の根のヒントの使用回数がすべて 0、2 周目の根の `drafts` が空。
3. 2 周目の `idea-cycle@round2/st-2-1` で `set_draft("下書き2")`、`OpenHint` を 2 回。1 周目の根の下書きは `"下書き1"` のまま、ヒントの使用回数は 1 のまま。
4. `Control::GoTo("issue-to-merge@round1/st-6-1")` で 1 周目へ移ると、`Session.draft` は `"下書き1"`、2 周目へ戻ると `"下書き2"`。
5. ヒントの使用回数は、停止・復帰で戻らない（`st-7-2` の位置で `OpenHint` → 停止、保存 → ヒント → 復帰の後も、増えた数が残る）。

### (iv) 2 周目の開始状態が、1 周目の終了時の世界状態と一致する（ハッシュ）

1. H_end_r1 = 1 周目の終了時の世界状態のハッシュ。H_start_r2 = 2 周目の開始（`s-r2`）の直後の世界状態のハッシュ。**H_end_r1 == H_start_r2**。
2. H_fresh = `Session::new("s-r2-fresh")` の直後（1 周目をしていない人が 2 周目から始める）の世界状態のハッシュ。**H_end_r1 == H_fresh**（`s-r2-fresh` が書く 13 経路が、1 周目の終了時の世界状態の範囲と過不足なく一致する）。一致しなければ、差の経路を記録する。
3. 1 周目の根の切り身のハッシュは、1 周目の終了時と、2 周目の終了時で一致する。
4. 以上のハッシュを `../measurements/state-hash.csv` に出す（列は `point,description,sha256`）。

### (v) 2 周目から途中開始した場合、開始用の状態を読み込み、飛ばした Step を「省略」と記録する

1. `Session::new("issue-to-merge@round2#s-6-1")`: `issue-to-merge@round2/st-3-2`・`st-4-1`・`st-5-1` が `skipped`（`Skipped` のイベントが 3 件）、`st-6-1` が `entered`、ほかの Step は `not_started`。世界状態は `impl/docs/behavior-bbs-search.md` の 1 経路だけ（開始用の状態）。1 周目の根の切り身はすべて空。案内レベルは `A`（この開始点は案内レベルを宣言しない）。
2. `Control::Begin("issue-to-merge@round2#s-6-1")` を、1 周目を終えた状態に対して行う: 世界状態は 1 周目の終了時の世界状態に `impl/docs/behavior-bbs-search.md` を足したものだけ。`skipped` は 2 周目の実体の 3 Step だけで、1 周目の根の切り身は変わらない。
3. `s-r2-fresh` では、飛ばした実体（`idea-cycle@round1` ほか 1 周目の実体）は `not_started` のまま（省略の印は、開始 Step と同じ実体の中の先の Step だけに付く）。
4. 省略と、引数で省く Step（2 周目の通常の通り道の `st-4-1`・`st-5-1`、`not_started` のまま）は別。この違いを記録する。

### (vi) 開始条件を満たさない場合に、開始できない理由を示す

1. `Session::new("s-r2")`（1 周目をしていない）: `Err(StartUnmet { start: "s-r2", rejection: "rj-start-needs-previous-round" })`。
2. 1 周目の途中の `Session` に `Control::Begin("s-r2")`: 実行の状態（`RunState` の全体）・下書きは変わらず、世代も変わらない。ログに増えるのは `Refused { op: Begin, reason: StartUnmet, rejection: Some("rj-start-needs-previous-round") }` の 1 件。
3. 示す文面（`rejections` の `rj-start-needs-previous-round`）の分類は `wrong_order`、「次の行動」が空でない。
4. 1 周目を終えた後に `Control::Begin("s-r2")` が成功する。
5. 存在しない開始 ID は `Refused { reason: UnknownStart }`、状態は変わらない。

### (vii) 実体の並べ方に巡回を作ると、PoC-3 の検査で拒否される

`defect-cycle.md` の C1・C2・C3 を、正しいパックの写しに置換で入れ、`check` の診断に `E-MODULE-CYCLE` が出ること。正しいパックでは出ないこと。`compile` が止まらずに診断を返すこと。

## C. 手順 4・5

- 手順 4: `step4-edit.md` のとおり。
- 手順 5: `step5/ch-round3.chapter.yaml` を、パックの写しの `chapters/ch-round3/chapter.yaml` として足す。確かめること: (1) 既存のファイル（`pack/` の全ファイル）が 1 バイトも変わらない（SHA-256 の一覧が一致）。(2) `validate` が診断 0 件。(3) IR の実体が 2 つ増え（`idea-cycle@round3`・`issue-to-merge@round3`）、既存の Step の実体は 1 つも変わらない。(4) 1 周目、2 周目、3 周目を通しで流すと完走し、3 周目の実体のフラグ・Step の状態が 1・2 周目と混ざらない。(5) `s-r3` は 2 周目を終えていないと開始できない。

## D. 縦断の確認が実体をまたいでも成り立つか（観察。性質テストの S-1・C-1 と別に、決定的に確かめる）

- 縦断 1 の「checkpoint→復習」: 2 周目の中で保存・進行・復帰したとき、復習の項目のうち 2 周目の根のものは保存の時点に戻り、1 周目の根の項目は変わらない（`Projection` の導出）。
- 縦断 2 の「停止・Step の移動の後に古い処理が新しい状態を変えない」: 2 周目の待機の Step（`issue-to-merge@round2/st-7-2`）で進行の宛先を取り、停止・1 周目への移動・復帰のそれぞれの後で、その古い宛先の進行を送り、実行の状態が変わらず「無視」が 1 件だけ増える。1 周目の待機の Step の宛先も同様。

## E. #20（`wait` の Step での処理中の停止。記録だけで、判定に使わない）

動きは変えない。次の手順で観察し、結果をそのまま `../README.md` に書く。

1. 2 周目の `issue-to-merge@round2/st-7-2` に入る（`wait` の Step。入った時点で処理中）。
2. 進行を 1 つ送る（進行 1/2。`prs/pr-2-a` が `ci-running`）。
3. `Control::Stop`。停止の前後の、処理中の進み（`busy`）、`prs/pr-2-a` の状態、世代、ログに増えたイベントを記録する。
4. 続けて進行を現在の宛先で 2 つ送り、受付済みになり、確認で達成できるかを記録する。
5. 1 周目の `issue-to-merge@round1/st-7-2` で同じ操作を行い、同じ動きかを記録する。

## F. その他の記録（判定に使わない）

- 2 周目の `wrong_order` の文面（`do` の差し込み。EP-7）が、案内レベル B の意図（skill 名を伏せる）に合うか: 2 周目の Step で `/advance-phase` を送り、返る文面を記録する。
- 案内レベル B で始めた 2 周目の本文・`do` に、隠すと宣言した語が出ないか: `E-GUIDANCE-LEAK` の検査（`validate`）と、実体の引数で渡した `do` の文面に隠す語を入れたときの検査の結果。
- 案内レベルは実行全体で 1 つ。2 周目の開始の後に 1 周目の Step へ移ったとき、案内レベルが B のままかを記録する。
