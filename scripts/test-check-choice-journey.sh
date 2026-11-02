#!/usr/bin/env bash
# scripts/check-choice-journey.sh の回帰テスト（誤用の拒否と、偽の node による引数の受け渡し・終了コードの伝搬。
# ビルド・ブラウザは使わない。`make test-check-choice-journey` と CI の check ジョブから呼ばれる。TASK-748 2/2）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
S="${DIR}/check-choice-journey.sh"
fail=0

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-test-choice-journey.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT
mkdir -p "${TMP}/bin" "${TMP}/web"
printf '{}\n' >"${TMP}/c.json"
printf '#!/bin/sh\nexit 0\n' >"${TMP}/pack"
chmod +x "${TMP}/pack"
# 偽の node: 受け取った引数を記録し、FAKE_NODE_RC で終了する。
cat >"${TMP}/bin/node" <<'EOF'
#!/bin/sh
printf '%s\n' "$@" >"${FAKE_NODE_LOG}"
exit "${FAKE_NODE_RC:-0}"
EOF
chmod +x "${TMP}/bin/node"

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
expect_rc 2 "positional argument" "${S}" extra
expect_rc 2 "config without value" "${S}" --config
expect_rc 2 "missing config file" "${S}" --config /nonexistent-config-for-test.json
expect_rc 2 "config path with parent segment" "${S}" --config "${TMP}/../c.json"
expect_rc 2 "option-like config path" "${S}" --config -x
expect_rc 2 "bad browser channel" env FANDHE_BROWSER_CHANNEL=firefox "${S}" --config "${TMP}/c.json"

export FAKE_NODE_LOG="${TMP}/node.log"
common=(env "PATH=${TMP}/bin:${PATH}" "FANDHE_PACK_BIN=${TMP}/pack" "FANDHE_WEB_AUTHOR_DIR=${TMP}/web" "FAKE_NODE_LOG=${FAKE_NODE_LOG}")
expect_rc 0 "passes configs to node and exits 0" "${common[@]}" "${S}" --config "${TMP}/c.json"
if [ "$(grep -c -- '^--config$' "${FAKE_NODE_LOG}")" -ne 1 ] || ! grep -qxF "${TMP}/c.json" "${FAKE_NODE_LOG}"; then
  echo "FAIL: node did not receive --config <file>" >&2
  fail=1
else
  echo "ok: node received --config <file>"
fi
expect_rc 1 "propagates node failure" "${common[@]}" FAKE_NODE_RC=1 "${S}" --config "${TMP}/c.json"
expect_rc 2 "propagates node misuse" "${common[@]}" FAKE_NODE_RC=2 "${S}" --config "${TMP}/c.json"
# 既定（引数なし）は同梱の設定の全件を渡す
expect_rc 0 "default configs" "${common[@]}" "${S}"
if [ "$(grep -c -- '^--config$' "${FAKE_NODE_LOG}")" -lt 3 ]; then
  echo "FAIL: default run should pass all bundled configs" >&2
  fail=1
else
  echo "ok: default run passes all bundled configs"
fi
exit "${fail}"
