#!/usr/bin/env bash
# scripts/build-pack-bin.sh 自体の回帰テスト（`make test-build-pack-bin` から呼ばれる）。
#
# 「CARGO_TARGET_DIR を別にしても、前提バイナリの位置を解決して実行できる」ことを自動で確かめる（BUILD-16。TASK-91）。
# 別置きのケースは冷えた target から pack と依存をビルドするため遅い。make check・フック・CI には入れない
# （CI では browser-test ジョブが、別の CARGO_TARGET_DIR で make test-browser を流して同じことを毎回確かめる）。
#
# 出力は TAP 風（scripts/test-check-toolchain.sh に合わせる）。終了コード: 0 = 全 pass / 1 = 失敗あり / 2 = 誤用。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd cargo
require_cmd jq
require_cmd mktemp
require_cmd rm

BUILD="${DIR}/build-pack-bin.sh"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

passed=0
failed=0

# expect <name> <expected> <actual>
expect() {
  if [ "$2" = "$3" ]; then
    echo "ok - $1"
    passed=$((passed + 1))
  else
    echo "not ok - $1 (expected '$2', got '$3')"
    failed=$((failed + 1))
  fi
}

# 1. 誤用（引数ありは終了コード 2）
RC=0
"${BUILD}" --unknown >/dev/null 2>&1 || RC=$?
expect "unknown argument is a usage error" 2 "${RC}"

# 2. CARGO_TARGET_DIR を別にしても、その配下に出力され、実行できる（引数なしは usage で終了コード 2）
alt="${TMP_DIR}/target"
RC=0
out="$(CARGO_TARGET_DIR="${alt}" "${BUILD}" 2>/dev/null)" || RC=$?
expect "build with an alternate CARGO_TARGET_DIR succeeds" 0 "${RC}"
case "${out}" in
  "${alt}"/*) expect "output path is under the alternate target" yes yes ;;
  *) expect "output path is under the alternate target" yes "no (${out})" ;;
esac
RC=0
"${out}" >/dev/null 2>&1 || RC=$?
expect "resolved binary runs (usage exit code)" 2 "${RC}"

# 3. CARGO_TARGET_DIR なしでは、cargo metadata の target_directory の配下に出る
default_target="$(cd "${ROOT_DIR}" && env -u CARGO_TARGET_DIR cargo metadata --format-version 1 --no-deps | jq -r .target_directory)"
RC=0
out="$(env -u CARGO_TARGET_DIR "${BUILD}" 2>/dev/null)" || RC=$?
expect "build with the default target succeeds" 0 "${RC}"
case "${out}" in
  "${default_target}"/*) expect "output path is under the default target" yes yes ;;
  *) expect "output path is under the default target" yes "no (${out})" ;;
esac

echo "# passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
