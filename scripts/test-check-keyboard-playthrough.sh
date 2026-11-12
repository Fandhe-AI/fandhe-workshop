#!/usr/bin/env bash
# scripts/check-keyboard-playthrough.sh の回帰テスト（誤用の拒否。ビルド・ブラウザは使わない）。
# `make test-check-keyboard-playthrough` と CI の check ジョブから呼ばれる。TASK-92 2/2。
# 前提コマンドの確認とビルドより前の引数の検査だけを見る（本流はブラウザ試験 keyboard-playthrough と make check-keyboard-playthrough が確かめる）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"
S="${DIR}/check-keyboard-playthrough.sh"
fail=0

expect_rc() {
  local want="$1" desc="$2"
  shift 2
  local rc=0
  "$@" >/dev/null 2>&1 || rc=$?
  if [ "${rc}" -ne "${want}" ]; then
    echo "FAIL: ${desc} (exit ${rc}, want ${want})" >&2
    fail=1
  else
    echo "ok: ${desc}"
  fi
}

expect_rc 2 "no arguments" "${S}"
expect_rc 2 "one argument" "${S}" "${ROOT_DIR}"
expect_rc 2 "three arguments" "${S}" "${ROOT_DIR}" a b
expect_rc 2 "pack directory missing" "${S}" /nonexistent-pack-for-test pt-x
expect_rc 2 "playthrough id with a quote" "${S}" "${ROOT_DIR}" 'a"b'
expect_rc 2 "playthrough id with a slash" "${S}" "${ROOT_DIR}" 'a/b'
expect_rc 2 "empty playthrough id" "${S}" "${ROOT_DIR}" ''
expect_rc 2 "bad browser channel" env FANDHE_BROWSER_CHANNEL=firefox "${S}" "${ROOT_DIR}" pt-x
exit "${fail}"
