#!/usr/bin/env bash
# 2 周分の履歴（書き出しの包み）の大きさと `Changed` の割合を、基点つきで測って JSON を出す（TASK-216。EVT-13）。
#
# 使い方: tools/measure/history-size.sh（引数なし。make measure-history-size から呼ばれる）
#
# crates/pack/tests/history_size_rounds.rs を `--nocapture` で流し、`HISTORY-SIZE ` の行（ちょうど 1 行）を
# lib.sh の measure_native_result_json で様式 fandhe-workshop/measure-native/v1 に包む。基点は commit・教材・操作列。
# 大きさは端末に依らないため端末は入れない。5MB の判定・条件 B／C は TASK-336 の `crates/pack/tests/history_size_conditions.rs`（別の試験バイナリ・別の接頭辞）が持ち、`make measure` の束には入れない。
# 結果の JSON は標準出力に 1 つだけ、進行は標準エラーへ出す。出力は lib.sh の measure_emit_native だけが行う。
#
# 終了コード: 0 = 成功 / 1 = 試験・基点の収集・検証の失敗（標準出力は空） / 2 = 誤用。
# 回帰テストは scripts/test-measure-result.sh（様式の組み立てと検証）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../../scripts/lib.sh
source "${DIR}/../../scripts/lib.sh"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd cargo
require_cmd jq
require_cmd git

COMMIT_JSON="$(measure_commit_json)" || {
  echo "error: cannot determine the baseline commit" >&2
  exit 1
}

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-measure.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

log "running history_size_rounds"
if ! (cd "${ROOT_DIR}" && cargo test --locked -p fandhe-workshop-pack --test history_size_rounds \
  -- --nocapture --test-threads=1) >"${TMP}/test.out" 2>"${TMP}/test.err"; then
  cat "${TMP}/test.err" >&2
  echo "error: history_size_rounds failed" >&2
  exit 1
fi

# grep -c は 0 件のとき非 0 を返す。0 件は直後の件数の検査で失敗にするため、ここでは握りつぶす。
N="$(grep -c '^HISTORY-SIZE ' "${TMP}/test.out" || true)"
if [ "${N}" != "1" ]; then
  echo "error: expected exactly one HISTORY-SIZE line (got ${N})" >&2
  exit 1
fi
grep '^HISTORY-SIZE ' "${TMP}/test.out" | sed -e 's/^HISTORY-SIZE //' >"${TMP}/line.json"

if ! jq -e -s '
  length == 1 and (.[0] |
    (.pack.id | type == "string") and (.ops_sequence.ops_sha256 | type == "string")
    and (.size | all(.[]; type == "number")))
' "${TMP}/line.json" >/dev/null 2>&1; then
  echo "error: unexpected HISTORY-SIZE line" >&2
  exit 1
fi

measure_assert_commit_unchanged "${COMMIT_JSON}" || exit 1

measure_native_result_json "${COMMIT_JSON}" "${TMP}/line.json" >"${TMP}/result.json" || exit 1
measure_emit_native "${TMP}/result.json"
