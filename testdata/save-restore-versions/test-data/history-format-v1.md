# 履歴の形式 版 1（手順 1 の決定。実装の前に固定）

PoC-6 の手順 1 の決定を書く。実装（`prototype/engine/crates/core/src/history.rs`、`session.rs`）はこの文書に従う。
`../env.md` に固定の時刻と SHA-256 を記録する。書き換えが要る場合は `test-data-addendum/` に足す。

## 1. 復元の方式（決定）

- **主**は「初期状態に、入力の列を順に適用し直す」こと（仮説）。履歴の包みに `ops`（入力の列）を、ログとは別に持つ。
- 包みには、適用し直した結果の**写し**（`snapshot`）も持つ。写しの役目は 2 つ: (a) 読み込みのときの照合（適用し直した結果が写しと完全に一致しなければ、履歴は信頼しない）、(b) 履歴を表示だけで開く場合の元。
- したがって読み込みは常に「`ops` を適用し直す → 写しと照合する」を行う。写しだけを信じて開くことはしない。

## 2. 包み

```json
{
  "format": "fandhe-workshop-history",
  "history_format": 1,
  "pack_id": "bbs-tutorial",
  "versions": {
    "engine": "0.0.0",
    "ir_format": 1,
    "vocab": "1.2",
    "content": "1.0.0",
    "content_hash": "<IR の JSON の SHA-256（小文字の 16 進）>",
    "refs": { "<参照元の名前>": "<commit>" }
  },
  "start": "s-r1",
  "ops": [ ... ],
  "snapshot": { ... }
}
```

- `history_format`: 履歴の包みの形の版（今は 1）。
- 4 軸の版（C-12）の置き方:

| 軸 | 欄 | 取り方 |
|----|----|--------|
| エンジンの版 | `versions.engine` | エンジンの crate の版（`CARGO_PKG_VERSION`） |
| 教材形式の版 | `versions.ir_format`、`versions.vocab` | IR の `format_version`、教材の宣言 `pack.vocab`（省略は `1.0`） |
| 教材内容の版 | `versions.content`、`versions.content_hash` | `pack.version`、IR の JSON（`serde_json::to_string`）の SHA-256 |
| 参照元の版 | `versions.refs` | IR の `refs` の名前 → commit |

- **`ir_format` と語彙の版は分ける**（決定。PoC-3 の追加の検証の判断事項 4）。v1.1・v1.2 は加算だけで `ir_format` は 1 のまま。`ir_format` は IR の構造の版、`vocab` は教材が使うと宣言した語彙の版。`vocab` の宣言と実際の使用の一致は検査しない（Phase 4 の課題）。
- 版の比較は `主.副.修` の 3 つの数（語彙は `主.副`）。数でない版は読めない履歴として拒否する。

## 3. 入力の列 `ops`

`Op` は次のどれか（JSON は serde の外部タグ、名前は snake_case）。

| Op | 中身 | 適用 |
|----|------|------|
| `input` | `Input` | `Session::input` |
| `submit` | 下書きの文字列 | 下書きを置いて `Session::submit` |
| `tick` | `TickAddr` | `Session::tick` |
| `control` | `Control` | `Session::control` |
| `draft` | 下書きの文字列 | `Session::set_draft`。連続する `draft` は最後の 1 つに畳む |
| `reload` | なし | `Session::reloaded`（下の 5 節） |

空の `submit`（実行環境が何もしない操作）は記録しない。拒否された操作・無視された進行は、状態を変えるので記録する。

## 4. 写し `snapshot`

`state`（`RunState`）、`log`（`Entry` の列）、`entry`（現在の Step の入口の写し。`state` と `log_len`）、`checkpoints`（番号、`state`、`entry`、`log_len`、`owner` の表）、`next_checkpoint`、`draft`、`draft_root`、`drafts`（根 → 下書き。空は除く）。

- **入口の写しの置き場所（PoC-1 の F-7、PoC-4 の設計への示唆）**: 現在の Step の分を 1 つだけ `snapshot.entry` に持つ。チェックポイントは自分の `entry` を持つ。実体ごと・Step ごとに溜めない。
- **ログの並び**: 書き出し・読み込みで並びを変えない（`Restored.log_len`・`CheckpointSaved.log_len` がログの位置を指す。PoC-1 の F-8）。
- **下書き**: 現在の根の下書きは `snapshot.draft`、他の根は `snapshot.drafts`。下書きの保存の口は `Session::set_draft` で、`Op::draft` として履歴に入る。
- **案内レベル**: `RunState.guidance_level` に入る。受講者が変える操作（`set_guidance`）は、この PoC の実行環境に無い（PoC-4 の時点でも `Control` に無い）。履歴に残すかの決定は、「操作ができた時点で `Op::control` に入る」ことにとどめる（操作が無いので試験できない）。開始点（`Control::Begin`）が決める案内レベルは、`Begin` の `op` と `state` に残る。

## 5. 再読み込み（`reload`）

- 読み込みに成功して `Session` を作った後、`Session::reloaded()` を行う。世代を **保存した世代 + 1** にし、`SessionEvent::Reloaded { at, new_generation }` をログに足し、`Op::reload` を記録する。
- 処理中だった場合は**そのまま続ける**（停止にはしない）。次の進行の宛先は新しい世代で作り直されるので、再読み込みの前に予約された進行（古い世代の宛先）は無視される。
- 入口の写しは作り直す（現在の状態の写し。以後の停止はここへ戻る）。

## 6. 読み込みの結果

```
Opened::Open        { session }                 変換なしで開く
Opened::Converted   { session, notes }          変換して開く（notes は空でない）
Opened::ViewOnly    { reason, history }         理由を示して、表示だけで開く（Session を作らない）
Opened::Refused     { code, reason }            理由を示して開かない
```

読み込みの順序は `version-expectations.md` に固定する。上限: 履歴ファイルは 10MB（10 × 1024 × 1024 バイト）以下。

## 7. 保存先（ブラウザ）

- 保存の単位は 1 つの履歴 JSON。localStorage のキーは `fandhe-workshop/history`、補助のキーは `fandhe-workshop/meta`（`{"boot": 起動の通し番号}`）。IndexedDB は DB `fandhe-workshop`、ストア `kv`、キー `history`・`meta`。
- **別のタブの古いページが新しい保存を上書きしない**: ページを開くたびに `boot` を 1 増やして `meta` に書く。保存の直前に `meta.boot` が自分の値と違えば書かない（古いページ）。
- 自動保存: 変更の後 200ms の遅延（連続する変更は畳む。最大 500ms で必ず書く）。`pagehide`・`visibilitychange`（非表示）では遅延なしで書く。
- 書き出し: 履歴 JSON をファイルとしてダウンロード。読み込み: ファイルを選び、検査（`check_history`）が「開く／表示だけ」のときだけ保存先に書いて再読み込み。「開かない」なら、今の保存を変えず、理由を表示する。
- 保存に失敗した（容量の超過など）場合は、メモリ上の状態は変えず、画面に「保存できませんでした」と理由を出す。
