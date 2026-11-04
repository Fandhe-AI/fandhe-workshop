#!/usr/bin/env bash
# clean-room の「無視してよい試験」の許可一覧が、crates/ の #[ignore] の現物とずれていないことの静的な検査。
#
# なぜ要るか: tools/clean-room.sh の judge は、cargo test が無視した試験のうち ALLOWED_IGNORED に無いものを失敗にする。
#   #[ignore] の試験を足したのに一覧を直し忘れると、週 1 回の clean-room.yml まで気づけない（#2045）。
#   そこで cargo も docker も使わず、make check で一覧と現物を突き合わせる。
#
# 呼び出し元: scripts/check.sh（make check）、make check-ignored-tests。回帰テストは scripts/test-check-ignored-tests.sh。
# 照合先: tools/clean-room.sh の 1 行の `readonly ALLOWED_IGNORED=(...)`。ファイルは source も実行もせず、行を読むだけである。
#
# 現物の取り方: crates/ 配下の全 .rs で、先頭の空白を除いた行が `#[ignore]`・`#[ignore = ...]` で始まるものを数える。
#   `//` で始まるコメントの行は数えない。その後の属性・空行・コメントを飛ばして最初の `fn <名前>` の名前を取る。
#
# 既知の近似: 字句の照合であり、cfg_attr(..., ignore)・マクロが生成する試験・同名の別関数は区別しない。
#   名前は関数名だけで比べる（judge が `s/.*:://` で名前だけにするのと同じ粒度）。
#
# 終了コード: 0 = 一致 / 1 = ずれ（足りない・余り・重複） / 2 = 誤用・判定不能（fail-closed）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
for cmd in awk grep sort comm find mktemp; do
  require_cmd "${cmd}" || exit 2
done

CLEAN_ROOM="${ROOT_DIR}/tools/clean-room.sh"
[ -f "${CLEAN_ROOM}" ] || {
  echo "error: not found: ${CLEAN_ROOM}" >&2
  exit 2
}

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT
ACTUAL="${TMP_DIR}/actual.txt" # "<name><TAB><path>:<line>"
LISTED="${TMP_DIR}/listed.txt"
: >"${ACTUAL}"

# 現物。1 ファイルずつ awk で読み、名前が取れなければ判定不能で止める。
while IFS= read -r -d '' f; do
  rel="${f#"${ROOT_DIR}"/}"
  if ! awk -v file="${rel}" '
    function take(s) {
      sub(/^.*fn[ \t]+/, "", s)
      printf "%s\t%s:%d\n", s, file, ign_line
    }
    function try_fn(s) {
      if (match(s, /^(pub(\([^)]*\))?[ \t]+)?(async[ \t]+)?fn[ \t]+[A-Za-z_][A-Za-z0-9_]*/)) {
        take(substr(s, RSTART, RLENGTH))
        return 1
      }
      return 0
    }
    {
      line = $0
      sub(/^[ \t]+/, "", line)
      if (pending) {
        if (line == "" || line ~ /^\/\// || line ~ /^#\[/) next
        if (!try_fn(line)) {
          printf "error: #[ignore] without fn: %s:%d\n", file, ign_line > "/dev/stderr"
          bad = 1
        }
        pending = 0
        next
      }
      if (line ~ /^\/\//) next
      if (line ~ /^#\[ignore[ \t]*(\]|=)/) {
        ign_line = NR
        rest = line
        sub(/^#\[ignore[^\]]*\][ \t]*/, "", rest)
        if (!try_fn(rest)) pending = 1
      }
    }
    END {
      if (pending) {
        printf "error: #[ignore] without fn: %s:%d\n", file, ign_line > "/dev/stderr"
        bad = 1
      }
      exit bad
    }
  ' "${f}" >>"${ACTUAL}"; then
    exit 2
  fi
done < <(find "${ROOT_DIR}/crates" -type f -name '*.rs' -print0)

# 一覧。ちょうど 1 行の `readonly ALLOWED_IGNORED=(...)` だけを読む。
decl_count="$(grep -cE '^readonly ALLOWED_IGNORED=\(.*\)$' "${CLEAN_ROOM}" || true)"
if [ "${decl_count}" != "1" ]; then
  echo "error: expected exactly one single-line 'readonly ALLOWED_IGNORED=(...)' in tools/clean-room.sh, found ${decl_count}" >&2
  exit 2
fi
decl="$(grep -E '^readonly ALLOWED_IGNORED=\(.*\)$' "${CLEAN_ROOM}")"
decl="${decl#readonly ALLOWED_IGNORED=(}"
decl="${decl%)}"
: >"${LISTED}"
for name in ${decl}; do
  if ! [[ "${name}" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]; then
    echo "error: ALLOWED_IGNORED has an element that is not an identifier: ${name}" >&2
    exit 2
  fi
  printf '%s\n' "${name}" >>"${LISTED}"
done

status=0
dups="$(LC_ALL=C sort "${LISTED}" | LC_ALL=C uniq -d)"
if [ -n "${dups}" ]; then
  while IFS= read -r n; do
    echo "error: ALLOWED_IGNORED lists a name twice: ${n}" >&2
  done <<<"${dups}"
  status=1
fi

cut -f1 "${ACTUAL}" | LC_ALL=C sort -u >"${TMP_DIR}/actual-names.txt"
LC_ALL=C sort -u "${LISTED}" >"${TMP_DIR}/listed-names.txt"

while IFS= read -r n; do
  [ -n "${n}" ] || continue
  where="$(grep -F "${n}"$'\t' "${ACTUAL}" | cut -f2 | head -n 1)"
  echo "error: ignored test not in ALLOWED_IGNORED: ${n} (${where})" >&2
  status=1
done < <(LC_ALL=C comm -23 "${TMP_DIR}/actual-names.txt" "${TMP_DIR}/listed-names.txt")

while IFS= read -r n; do
  [ -n "${n}" ] || continue
  echo "error: ALLOWED_IGNORED has no #[ignore] test: ${n}" >&2
  status=1
done < <(LC_ALL=C comm -13 "${TMP_DIR}/actual-names.txt" "${TMP_DIR}/listed-names.txt")

if [ "${status}" -eq 0 ]; then
  log "ignored tests match ALLOWED_IGNORED ($(wc -l <"${TMP_DIR}/listed-names.txt" | tr -d ' ') names)"
fi
exit "${status}"
