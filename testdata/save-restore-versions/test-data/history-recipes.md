# 試験の履歴の作り方（実装の前に固定）

`version-expectations.md` の 2 節の履歴を、次の操作列で作る。教材は基準版 `packs/v1.0.0`、開始は `s-r1`、操作は `tests/poc6_common/mod.rs` の `guided`（現在の状態での正しい次の操作。PoC-4 の `poc4_common` の写し）を使う。「until X」は、現在の Step が X になるまで `guided` を繰り返すこと。

## H-early
1. `until idea-cycle@round1/st-2-3`
2. `Control::Save`

## H-mid
1. `until idea-cycle@round1/st-2-2`
2. `guided` を 1 手（自然文の送信。処理中になる）
3. `Control::Stop`
4. `until idea-cycle@round1/st-2-3`
5. `Control::Save`（番号 1）
6. `Input::Command("/advance-phase")`（誤入力）
7. `until idea-cycle@round1/st-2-5`
8. `Control::Save`（番号 2）
9. `Control::OpenHint`
10. `Input::Choice("no-such-option")`（誤入力）
11. `until idea-cycle@round1/st-2-7`
12. `set_draft("draft-mid")`

## H-busy
1. `until issue-to-merge@round1/st-7-2`（入った直後は処理中 0/2）
2. 進行を 1 つ（`next_tick` の宛先を `tick`）。処理中 1/2

## H-full
`synth-inputs.md` の条件 A、通し番号 1。

## 版の違いの履歴

- `H-mid` と `H-early` は、基準版 `v1.0.0` の教材で作る。
- V-iv-1 の履歴は、同じ `H-mid` の操作列を `packs/v1.0.0-vocab-1.0`（`pack.vocab` が `1.0`）で流して作る。
- B-6 の履歴は、同じ `H-mid` の操作列を `packs/other-pack` で流して作る。
- `versions` の欄だけを書き換える作り物は、`H-mid` の JSON の `versions.<欄>` を `version-expectations.md` の値に替える（他の欄・`snapshot`・`ops` は変えない）。
