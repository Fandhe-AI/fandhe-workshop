#!/usr/bin/env bash
# エンジン repo のライセンス表記を、`cargo metadata` と直下のファイルで機械検査する（LIC-1・TASK-129）。
#
# 正は D-6（ライセンスは `MIT OR Apache-2.0`）と LIC-1。呼び出し元は `make check-license` と CI の rust-lint ジョブ。
# 回帰テストは scripts/test-check-license.sh（`make test-check-license`）。
#   - workspace の全 member の `license` が `MIT OR Apache-2.0` に完全一致すること
#   - `license-file` を使っていないこと（SPDX 式の `license` を正とするため）
#   - workspace 直下に LICENSE-MIT・LICENSE-APACHE が通常のファイルとして在り、空でなく、取り違えていないこと
#   - 雛形 template-workshop/ に同じ本文の写し（バイト一致）と README の表記があり、雛形から作った repo の模擬
#     （cp -RP）にも引き継がれること（LIC-2・TASK-130。symlink は引き継がれないため拒否する）
# エンジン直下の本文の完全一致は保証しない（最小の目印の固定文字列だけを照合する）。package.json は crate ではないため範囲外。
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

# links_leave_dir <file>: Markdown のリンク先（インライン・参照定義）を相対パスとして正規化し、
# 置き場の外へ出るもの（./../ の表記、先頭の / を含む）が 1 件でもあれば成功を返す。
# 外部 URL（スキーム付き）と同一ファイル内の # だけは対象外。
links_leave_dir() {
  local verdict
  verdict="$(links_verdict "$1")"
  [ "${verdict}" = leave ]
}

# links_verdict <file>: leave または stay を出力する（grep の不一致が pipefail で結果を壊さないよう、状態は出力で返す）。
links_verdict() {
  { { grep -oE '\]\([^)]*\)' "$1" || true; } | sed -e 's/^](//' -e 's/)$//'
    { grep -E '^[[:space:]]*\[[^]]+\]:' "$1" || true; } | sed -E 's/^[[:space:]]*\[[^]]+\]:[[:space:]]*//'
  } | awk '
    {
      t = $1; gsub(/^</, "", t); gsub(/>$/, "", t)
      sub(/[#?].*$/, "", t)
      if (t == "" || t ~ /^[A-Za-z][A-Za-z0-9+.-]*:/) next
      if (t ~ /^\//) { bad = 1; next }
      n = split(t, seg, "/"); depth = 0
      for (i = 1; i <= n; i++) {
        if (seg[i] == "" || seg[i] == ".") continue
        if (seg[i] == "..") { depth--; if (depth < 0) { bad = 1; break } }
        else depth++
      }
    }
    END { print bad ? "leave" : "stay" }'
}

# check_template_root <dir> <label>: 雛形（またはその写し）の直下の表記を確かめる。本文は出力しない。
check_template_root() {
  local dir="$1" label="$2" name
  for name in LICENSE-MIT LICENSE-APACHE; do
    if [ -L "${dir}/${name}" ] || [ ! -f "${dir}/${name}" ] || ! cmp -s "${dir}/${name}" "${WORKSPACE_ROOT}/${name}"; then
      echo "error: ${label}/${name} is missing, a symlink, or differs from the engine's ${name}" >&2
      VIOLATIONS=$((VIOLATIONS + 1))
    fi
  done
  if [ -L "${dir}/README.md" ] || [ ! -f "${dir}/README.md" ]; then
    echo "error: ${label}/README.md is missing or a symlink" >&2
    VIOLATIONS=$((VIOLATIONS + 1))
    return 0
  fi
  local marker
  for marker in 'MIT OR Apache-2.0' 'LICENSE-MIT' 'LICENSE-APACHE'; do
    if ! grep -qF -- "${marker}" "${dir}/README.md"; then
      echo "error: ${label}/README.md lacks the license notice" >&2
      VIOLATIONS=$((VIOLATIONS + 1))
      return 0
    fi
  done
  if links_leave_dir "${dir}/README.md"; then
    echo "error: ${label}/README.md has a link that leaves the directory" >&2
    VIOLATIONS=$((VIOLATIONS + 1))
  fi
}

TEMPLATE_DIR="${WORKSPACE_ROOT}/template-workshop"
if [ -L "${TEMPLATE_DIR}" ] || [ ! -d "${TEMPLATE_DIR}" ]; then
  echo "error: template-workshop is missing" >&2
  VIOLATIONS=$((VIOLATIONS + 1))
else
  check_template_root "${TEMPLATE_DIR}" template-workshop
  # 雛形から作った repo の模擬。-P で symlink を symlink のまま写し、検出する。
  NEW_REPO="${TMP_DIR}/new-repo"
  if ! mkdir "${NEW_REPO}" || ! cp -RP "${TEMPLATE_DIR}/." "${NEW_REPO}/"; then
    echo "error: cannot copy template-workshop to simulate a new repo" >&2
    exit 2
  fi
  check_template_root "${NEW_REPO}" "new repo from template-workshop"
fi

if [ "${VIOLATIONS}" -ne 0 ]; then
  exit 1
fi
log "ok: ${MEMBER_COUNT} members, license ${EXPECTED_LICENSE}, 2 license files, template-workshop notice (2 files + README, carried over to a copy)"
