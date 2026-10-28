#!/usr/bin/env bash
# 2 周分の通しでの右のログの項目数（最大）を、基点つきで測って JSON を出す（TASK-454 1/2。TERM-35・FEND-22）。
#
# 使い方: tools/measure/log-items.sh（引数なし。make measure-log-items から呼ばれる）
#
# crates/web/tests/log_rounds_count.rs を `--nocapture` で流し、`LOG-ROUNDS ` の行（ちょうど 1 行）を
# lib.sh の measure_native_log_rounds_result_json で様式 fandhe-workshop/measure-native/v1 に包む。基点は commit・教材・操作列。
# `make measure` の束には入れない。結果の JSON は標準出力に 1 つだけ、進行は標準エラーへ出す。出力は lib.sh の measure_emit_native だけが行う。
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

log "running log_rounds_count"
if ! (cd "${ROOT_DIR}" && cargo test --locked -p fandhe-workshop-web --test log_rounds_count \
  -- --nocapture --test-threads=1) >"${TMP}/test.out" 2>"${TMP}/test.err"; then
  cat "${TMP}/test.err" >&2
  echo "error: log_rounds_count failed" >&2
  exit 1
fi

# grep -c は 0 件のとき非 0 を返す。0 件は直後の件数の検査で失敗にするため、ここでは握りつぶす。
N="$(grep -c '^LOG-ROUNDS ' "${TMP}/test.out" || true)"
if [ "${N}" != "1" ]; then
  echo "error: expected exactly one LOG-ROUNDS line (got ${N})" >&2
  exit 1
fi
grep '^LOG-ROUNDS ' "${TMP}/test.out" | sed -e 's/^LOG-ROUNDS //' >"${TMP}/line.json"

if ! jq -e -s '
  length == 1 and (.[0] |
    (.pack.id | type == "string") and (.ops_sequence.ops_sha256 | type == "string")
    and (.log | all(.[]; type == "number")))
' "${TMP}/line.json" >/dev/null 2>&1; then
  echo "error: unexpected LOG-ROUNDS line" >&2
  exit 1
fi

measure_assert_commit_unchanged "${COMMIT_JSON}" || exit 1

measure_native_log_rounds_result_json "${COMMIT_JSON}" "${TMP}/line.json" >"${TMP}/result.json" || exit 1
measure_emit_native "${TMP}/result.json"
