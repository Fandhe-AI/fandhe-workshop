# input_field_hidden

入力欄なしの教材（`pack.profile.input_field: hidden`）の試験データ。TASK-450 2/3 の試験が使う。

出所は `testdata/engine-event-consistency/test-data/min-3step.json` で、次だけを変えた。

- `pack.id` を `min-3step-no-input` にした（保存先の鍵が既定の教材と衝突しないようにする）
- `pack.profile` を `{"input_field": "hidden"}` にした
- `main/st-1` の入力を選択肢 2 件（`opt-a`・`opt-b`）にした。`opt-a` は元の `r-make` と同じ効果で受付、`opt-b` は拒否。確認は元のまま `point_at`
- `main/st-2` の入力を `ui_action`（`v-files` の `notes/draft`）にし、確認を `answer`（選択肢 2 件）にした
- `main/st-3` は元のまま（入力 `approval`・確認 `approve`）

`command`・`text` の入力と、`on.command`・`on.text` の規則は残していない（`E-PROFILE-NO-INPUT` に当たらない形）。
