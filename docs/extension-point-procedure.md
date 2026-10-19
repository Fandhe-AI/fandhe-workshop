# 拡張点の追加の手続き

```text
手続きの文書の版: 1
決定日: 2026-10-19
状態: 固定（契約）
```

## 1. 位置づけと版

- 本文書は、汎用の名前つき拡張点を新しく足すときの手順を 1 か所に固定する（TASK-189、EXT-8、D-53）。個別の教材のための場当たり的なエンジン改修を防ぐ
- 入力は `docs/vocabulary/vocabulary.md` の 5 節（固定・実装・公開の順。TASK-48）である。本文書は、その順に診断コードの追加と既存の教材パックの IR の確認を加えて手続きにする
- **実行時に拡張点を登録する仕組みは作らない**（D-53）。拡張点はエンジンの版上げで足す。本文書は現状の手続きを文書にしたもので、実行時の機能は足さない
- 版の規則: 手順を変えるときは版を上げ、変更履歴に記録する。語彙の文書の版（`語彙の文書の版`）・語彙の版・`ir_format` とは別に管理する
- 仕様は private のため、本文を転記せず ID（EXT-8、PACK-14〜PACK-24、D-53）で参照する
- 文書の形（版の欄・節の見出し・実在するパス）は `crates/pack/tests/extension_point_procedure_doc.rs` が見る。正は本文書である

## 2. 前提と不変条件

- **教材パックから任意のコードは書けない**。拡張点が足すのは、閉じた値の集合を持つ宣言の欄だけである。スクリプト・式・コードを書く欄を、拡張点として足してはならない
- 未知の欄は `E-PARSE` で拒否される。根拠の試験は、`script`・`code`・`eval`・`exec`・`expr`・`js` の拒否を見る `crates/pack/tests/effects.rs`・`crates/pack/tests/condition_vocabulary.rs`・`crates/pack/tests/format_b.rs`・`crates/pack/tests/confirm_vocabulary.rs` である
- 教材パックからは、名前の欄を書いて参照するだけである。受け口の有効化も教材パックからはできない（EXT-10、D-53）
- 教材パックは信頼できない入力として扱う。新しい欄の読み取りは、不正な値を安全側（拒否）に倒す
- 手順 0 として、先に仕様の 04（PACK-n の行）と決定（D-n）を用意する。EP-13 では仕様の記録が実装の後になった（9 節）ため、同じことを繰り返さない。名前を変えるときは、語彙の文書の 1 節のとおり仕様を先に直す
- 通常の教材の追加では、エンジンの差分は 0 行である（SECOND-20）。拡張点の追加は、既存の語の組み合わせで書けないと確かめた後に限る

## 3. 汎用であることの説明

- 語彙の文書の 2 節の「汎用である理由」の列に、性質の違う 2 つ以上の教材で同じ語が使える理由を書く。特定の教材の語や場面を理由にしない
- 名前の形は語彙の文書の 3 節に従う。種別は既存のものから選び、新しい種別を足す場合は語彙の文書の版上げの判断にする
- 付録 A の固有の語の照合を通す（`crates/pack/tests/vocabulary_extension_points.rs`）。固有の語を足す場合は、語彙の文書の版上げにする
- 実装のコードに教材固有の語を入れない（`make check-generic-words`。実体は `tools/check-generic-words.mjs`）

## 4. 語彙の文書の固定

実装より先に行う。固定した行は書き換えない・消さない・改名しない。

1. `docs/vocabulary/vocabulary.md` に、2 節の EP の行、7 節の語、8 節の版の行（新しい語彙の版のとき）、9 節の照合表の行、1 節の記述、版の繰り上げと変更履歴の行を足す
2. 実装の写しを揃える。`crates/pack/src/vocab.rs`（`VocabVersion` の値・`LATEST`・`EXTENSION_POINTS`・`TERMS`）と、`crates/core/src/history_open.rs` の `VOCAB_MAX` を同時に変える
3. 試験の期待を足す。`crates/pack/tests/vocabulary_extension_points.rs` の `EXPECTED`、`crates/pack/tests/vocabulary_terms.rs`、`crates/core/tests/history_open.rs` である
4. 実装する。型は `crates/contract/src/`、読み込みと検査は `crates/pack/src/` に置く。置き場の判定は `docs/architecture.md` の 6 節に従う。実例は EP-13 の `crates/contract/src/view_body.rs` である

## 5. 診断コードの追加

- 新しい欄の誤りを、既存の診断コードのどれで報告するかを決める（読めない値は `E-PARSE`、構造の誤りは `E-STRUCTURE` など）。既存のコードで表せないときだけ、新しいコードを `crates/pack/src/diag.rs` の `DiagCode` に足す
- 新しいコードを足したときは、`docs/diagnostics.md` に行を足し、一覧の版を上げ、件数と変更履歴を更新する
- `pack.vocab` との一致の検査に載せる。`crates/pack/src/vocab_check.rs` の `PROBES` に使用の判定を足し、`docs/diagnostics.md` の `E-VOCAB-MISMATCH` の注と `docs/pack-authoring.md` の対象の列挙を更新する。試験は `crates/pack/tests/vocab_declaration.rs` である
- 診断に入力値・パスを写さない（既存の診断の規約）
- 変異注入を足す場合は、`crates/pack/tests/mutations/` と `docs/testing.md` の手順に従う
- 公開の一覧と `DiagCode` の一致の自動検査は未実装（TASK-400）である。当面は手作業で照合する

## 6. 公開の一覧への追記

- 公開の一覧の正は、語彙の文書の 2 節の表である。行は、実装の後に実装済みの語として示す（語彙の文書の 5 節の 3）
- 作成者向けの書き方を `docs/pack-authoring.md` に節として足し、根拠となる試験の表に行を足す
- 用語集（`docs/glossary.md`）、`README.md`・`CLAUDE.md` の語彙の文書の行（EP の範囲・語彙の版・版）を揃える
- `fandhe-pack vocab` の数え上げに現れることを確かめる。数えない語は `docs/pack-authoring.md` に書く

## 7. 既存の教材パックが同じ IR になることの確認

- 規則: 新しい欄を使わない教材パックの IR は、追加の前後でバイト単位で同じである（PACK-24、EXT-9、D-52）。省略時の既定値を IR に出力しない
- 判別の決め手: `crates/pack/tests/data/golden/` と `crates/pack/tests/data/ir_format_1/` の `*.ir.json` を作り直さずに、`crates/pack/tests/ir_format.rs`（`ir2_golden_bytes_match`・`ir3_explicit_defaults_match_omitted` など）と `crates/pack/tests/ir_compat.rs` が変更なしで通ること
- golden の書き換えが要るなら、加算ではない。`ir_format` の版上げとして、仕様を先に直す
- 補助: `fandhe-pack compare <a> <b>` は、同じ版のエンジンで 2 つの教材パックを比べる道具である。追加の前後の同一性の証明にはならないため、補助に留める
- 旧い履歴が開くことも確かめる（`crates/core/tests/history_open.rs`）

## 8. 確認の一覧

PR の本文に写して使う。

| 確認 | 手段 |
|---|---|
| 仕様の行（PACK-n）と決定（D-n）が先にある | 仕様の repo（読み取り） |
| 教材パックにコードを書く欄を足していない | 3 節・2 節の試験、`E-PARSE` の拒否の試験 |
| 汎用である理由を書き、固有の語を含まない | `vocabulary_extension_points.rs`、`make check-generic-words` |
| 語彙の文書を先に固定し、実装の写しを揃えた | `make test`（`vocabulary_terms.rs`・`history_open.rs`） |
| 診断コードを決め、`docs/diagnostics.md` を更新した | `vocab_declaration.rs`、机上の照合 |
| 公開の一覧・作成者向けの文書・索引を更新した | 机上の照合 |
| 既存の教材パックの IR が同じである | golden を変えずに `ir_format.rs`・`ir_compat.rs` が通る |

検証コマンドは `make check`・`make lint`・`make test`・`make check-deps`・`make check-generic-words` である。

## 9. 実例

EP-13（`view.document_body`、語彙 v1.4、TASK-66 1/2。2026-10-18）で、各手順に当たった変更は次のとおりである。

| 手順 | 変更 |
|---|---|
| 固定（4 節） | 語彙の文書の版 3。EP-13 と 3 語を加算し、`vocab.rs` と `VOCAB_MAX` を 1.4 に揃えた |
| 実装（4 節） | `crates/contract/src/view_body.rs` に宣言の読み取りと本文の解決を置いた |
| 診断（5 節） | 不正値を `E-PARSE`、宣言が 1.4 未満なら `E-VOCAB-MISMATCH`（EP-13）とした。新しいコードは足していない。変異 PACK38-1・PACK38-2 を足した |
| 公開（6 節） | `docs/pack-authoring.md` に文書のビューの本文の節を足した |
| IR の確認（7 節） | 省略・`hidden` は従来どおり本文なしで、golden は変えていない |

仕様の PACK-38 の記録は実装の後追いになった。手順 0 は、これを繰り返さないために置いた。

## 10. 拡張点の型の契約との照合

机上の照合の記録である。仕様（PACK-14〜PACK-24、EXT-8）は private のため、写さず ID で示す。

| 日付 | 照合した節 | 結果 |
|---|---|---|
| 2026-10-19 | 語彙の文書の 1 節（版の規則・変更規則） | 矛盾なし。4 節は固定した行を書き換えない規則に従う |
| 2026-10-19 | 語彙の文書の 2・3 節（名前・種別・語彙の版・名前の形） | 矛盾なし。3 節は名前の形を再定義せず参照する |
| 2026-10-19 | 語彙の文書の 4 節・付録 A（汎用名の検査） | 矛盾なし。3 節は検査を通すことを手順にした |
| 2026-10-19 | 語彙の文書の 5 節（固定・実装・公開の順） | 矛盾なし。4・6 節がその順で、5・7 節を間に足す |
| 2026-10-19 | PACK-14〜PACK-24（EXT-8 が残した整合の確認） | 食い違いは見つけていない。IR の同一は 7 節（PACK-24）、受け口は 2 節（EXT-10）に対応する |

## 変更履歴

| 版 | 日付 | 変更 | 理由 |
|---|---|---|---|
| 1 | 2026-10-19 | 初版。手順 0・汎用性・語彙の文書の固定・診断コード・公開の一覧・IR の同一の確認を固定 | TASK-189、EXT-8、D-53 |
