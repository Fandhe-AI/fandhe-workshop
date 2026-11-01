#!/usr/bin/env bash
# scripts/build-pack-preview.sh の回帰テスト（誤用の拒否だけ。ビルドは行わない。`make test-build-pack-preview` から呼ばれる）。
# 実ビルドを通す確認は scripts/test-browser.sh（同梱つきの実行ファイルを作る）と build-pack-release.sh が行う。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
S="${DIR}/build-pack-preview.sh"
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
expect_rc 2 "unknown option" "${S}" --nope
expect_rc 2 "unsupported target" "${S}" --target evil-triple
expect_rc 2 "target without value" "${S}" --target
expect_rc 2 "missing web dir" "${S}" --web-dir /nonexistent-dir-for-test
expect_rc 2 "positional argument" "${S}" extra
exit "${fail}"
