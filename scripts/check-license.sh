#!/usr/bin/env bash
# エンジン repo のライセンス表記を、`cargo metadata` と直下のファイルで機械検査する（LIC-1・TASK-129）。
#
# 正は D-6（ライセンスは `MIT OR Apache-2.0`）と LIC-1。呼び出し元は `make check-license` と CI の rust-lint ジョブ。
# 回帰テストは scripts/test-check-license.sh（`make test-check-license`）。
#   - workspace の全 member の `license` が `MIT OR Apache-2.0` に完全一致すること
#   - `license-file` を使っていないこと（SPDX 式の `license` を正とするため）
#   - workspace 直下に LICENSE-MIT・LICENSE-APACHE が通常のファイルとして在り、空でなく、取り違えていないこと
# 本文の完全一致は保証しない（最小の目印の固定文字列だけを照合する）。package.json は crate ではないため範囲外。
#
# 使い方: scripts/check-license.sh [--manifest-path <Cargo.toml>]
#   --manifest-path は回帰テストが一時コピーを検査するために使う。
# 終了コード: 0 = 違反なし / 1 = 違反あり / 2 = 誤用、または検査を完了できない
#   （cargo・jq の失敗、member が 0 件）。2 でも合格にしない（fail-closed）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

MANIFEST_ARGS=()
if [ "$#" -eq 2 ] && [ "$1" = "--manifest-path" ]; then
  MANIFEST_ARGS=(--manifest-path "$2")
elif [ "$#" -ne 0 ]; then
  echo "usage: $(basename "$0") [--manifest-path <Cargo.toml>]" >&2
  exit 2
fi

require_cmd cargo || exit 2
require_cmd jq || exit 2

EXPECTED_LICENSE='MIT OR Apache-2.0'

# 違反を 1 行ずつ出力する jq プログラム。値は --arg で渡し、文字列連結しない。
# shellcheck disable=SC2016 # $ は jq の変数であり、シェルに展開させない
JQ_PROGRAM='
  . as $m
  | $m.packages[] | select(.id as $i | $m.workspace_members | index($i))
  | (if .license != $want then "license mismatch: \(.name) (\(.license // "null"))" else empty end),
    (if .license_file != null then "license-file must not be set: \(.name)" else empty end)
'

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT
META="${TMP_DIR}/metadata.json"

cd "${ROOT_DIR}"
# --no-deps --offline: 依存の解決も通信も行わない。Cargo.lock も書き換えない。
if ! cargo metadata --format-version 1 --no-deps --offline ${MANIFEST_ARGS[@]+"${MANIFEST_ARGS[@]}"} >"${META}" 2>"${TMP_DIR}/cargo.err"; then
  echo "error: cargo metadata failed; cannot check licenses" >&2
  head -n 5 "${TMP_DIR}/cargo.err" >&2
  exit 2
fi
if ! jq -e '(.workspace_members | length) > 0 and (.packages | length) > 0 and (.workspace_root | type) == "string"' "${META}" >/dev/null 2>&1; then
  echo "error: cargo metadata returned no workspace members" >&2
  exit 2
fi

if ! jq -r --arg want "${EXPECTED_LICENSE}" "${JQ_PROGRAM}" "${META}" >"${TMP_DIR}/violations.txt" 2>"${TMP_DIR}/jq.err"; then
  echo "error: jq failed while checking licenses" >&2
  head -n 5 "${TMP_DIR}/jq.err" >&2
  exit 2
fi
MEMBER_COUNT="$(jq -r '.workspace_members | length' "${META}")"
WORKSPACE_ROOT="$(jq -r '.workspace_root' "${META}")"

VIOLATIONS=0
while IFS= read -r line; do
  [ -n "${line}" ] || continue
  echo "error: ${line}" >&2
  VIOLATIONS=$((VIOLATIONS + 1))
done <"${TMP_DIR}/violations.txt"

# check_text <file> <fixed-string>...: 通常のファイルで空でなく、目印がすべて含まれること。本文は出力しない。
check_text() {
  local name="$1"
  shift
  local path="${WORKSPACE_ROOT}/${name}"
  if [ ! -f "${path}" ] || [ ! -s "${path}" ]; then
    echo "error: ${name} is missing or empty" >&2
    VIOLATIONS=$((VIOLATIONS + 1))
    return 0
  fi
  local marker
  for marker in "$@"; do
    if ! grep -qF -- "${marker}" "${path}"; then
      echo "error: ${name} lacks the expected license text marker" >&2
      VIOLATIONS=$((VIOLATIONS + 1))
      return 0
    fi
  done
}
check_text LICENSE-MIT 'Permission is hereby granted, free of charge'
check_text LICENSE-APACHE 'Apache License' 'Version 2.0, January 2004'

if [ "${VIOLATIONS}" -ne 0 ]; then
  exit 1
fi
log "ok: ${MEMBER_COUNT} members, license ${EXPECTED_LICENSE}, 2 license files"
