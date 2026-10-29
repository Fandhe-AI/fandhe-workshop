# broken-histories

壊れた履歴 7 種（SAVE-20 の B-1〜B-7）の拒否について、実施の前に固定した期待の表です（TASK-332 1/2、#880）。
`expectations.json` が表の本体で、`crates/core/tests/broken_histories.rs` が ID・件数・コードを定数と照合します。
spec は private のため、本文は写さず ID で参照します。

- B-0 は対照です。開くことを確かめ、検出器が「開く」を観測できることを示します。
- B-4b はちょうど 10 MiB の境界の対照です。`padding` は未知の欄なので `too_large` ではなく後段の `malformed` で断られ、大きさが理由でないことを示します。
- 履歴ファイルは置きません。現行のエンジンで試験の中で作ります（版を上げるたびに古くなるため）。10 MiB 超のファイルも置きません。
- B-1 は、選択肢の無い min-3step に合わせて「最初の choice の値」を「最初の submit の値」と読み替えています。
- ブラウザ試験の入力は `emit_broken_histories_for_browser`（`#[ignore]`）が一時ディレクトリへ書き出します。
