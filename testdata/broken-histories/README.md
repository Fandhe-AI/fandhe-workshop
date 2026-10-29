# broken-histories

壊れた履歴 7 種（SAVE-20 の B-1〜B-7）の拒否について、実施の前に固定した期待の表です（TASK-332 1/2、#880）。
`expectations.json` が表の本体で、`crates/core/tests/broken_histories.rs` が ID・件数・コードを定数と照合します。
spec は private のため、本文は写さず ID で参照します。

- B-0 は対照です。開くことを確かめ、検出器が「開く」を観測できることを示します。
- B-4b はちょうど 10 MiB の境界の対照です。`padding` は未知の欄なので `too_large` ではなく後段の `malformed` で断られ、大きさが理由でないことを示します。
- 履歴ファイルは置きません。現行のエンジンで試験の中で作ります（版を上げるたびに古くなるため）。10 MiB 超のファイルも置きません。
- B-1 は、選択肢の無い min-3step に合わせて「最初の choice の値」を「最初の submit の値」と読み替えています。
- ブラウザ試験の入力は `emit_broken_histories_for_browser`（`#[ignore]`）が一時ディレクトリへ書き出します。

## ops-rewrites.json（TASK-332 2/2、#881）

SAVE-20 の後半「`ops` の 22 か所の書き換え」について、実施の前に固定した期待の表です。22 は PoC-6 の H-mid の `ops` の件数です。
`crates/core/tests/ops_rewrites.rs` が、件数・分類の数・同一値の位置を定数と照合し、基準の履歴の位置ごとの種類も表の `kind` と照合します。

- 書き換えの規則は 3 つです。choice の入力は `Choice("no-such-option")`、`Control::Save` は `Control::OpenHint`、それ以外は `Input::Command("/no-such-command")` にします。
- 期待は 2 通りです。拒否（`replay_mismatch`、21 件）と、書き換えた値が元と同一で開いても写しと一致するもの（`same_value`、1 件。添字 15）です。誤った状態で開くものは 0 件です。
- W-00 は書き換えなしの対照で、表の外の定数として持ちます。W-01〜W-22 が添字 0〜21 に対応します。
- 履歴ファイルは置きません。試験の中で型の値のまま書き換えて作り、ブラウザ試験の入力は `emit_ops_rewrites_for_browser`（`#[ignore]`）が一時ディレクトリへ書き出します。
