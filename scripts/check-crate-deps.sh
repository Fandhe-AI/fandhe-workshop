#!/usr/bin/env bash
# Cargo ワークスペースの crate 間の依存の向きを、`cargo metadata` で機械検査する（`make check-deps` から呼ばれる）。
#
# 正は docs/architecture.md 第 4 節の表。許可した辺（ALLOWED_EDGES）以外はすべて違反にする（allowlist 方式）。
#   - member の集合と default-members の集合が期待と一致すること（web を default に入れない）
#   - member 間の辺は allowlist にあるものだけ（build-dependencies を含む）
#   - contract・core から（dev を除く）推移的に、web-sys・js-sys・wasm-bindgen・fandhe-frontend* へ届かないこと
#
# 注意: これは crate 間の依存の検査である。DOM 読み取りの検査（TASK-81 の tools/check-deps.sh）とは別物。
#
# 使い方: scripts/check-crate-deps.sh [--manifest-path <Cargo.toml>]
#   --manifest-path は回帰テスト（scripts/test-check-crate-deps.sh）が一時コピーを検査するために使う。
# 終了コード: 0 = 違反なし / 1 = 違反あり / 2 = 誤用、または検査を完了できない
#   （cargo・jq の失敗、resolve が null、Cargo.lock が古い）。2 でも合格にしない（fail-closed）。
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

# 期待する member（architecture 第 3 節）と default-members（web を含まない）。
EXPECTED_MEMBERS='["fandhe-workshop-contract","fandhe-workshop-core","fandhe-workshop-pack","fandhe-workshop-web"]'
EXPECTED_DEFAULTS='["fandhe-workshop-contract","fandhe-workshop-core","fandhe-workshop-pack"]'
# 許可する member 間の辺（from>to>kind）。web>pack>dev は試験専用の唯一の例外。
ALLOWED_EDGES='["fandhe-workshop-core>fandhe-workshop-contract>normal","fandhe-workshop-pack>fandhe-workshop-core>normal","fandhe-workshop-pack>fandhe-workshop-contract>normal","fandhe-workshop-web>fandhe-workshop-core>normal","fandhe-workshop-web>fandhe-workshop-contract>normal","fandhe-workshop-web>fandhe-workshop-pack>dev"]'
# core・contract の閉包に入ってはならない crate の名前。
FORBIDDEN_PATTERN='^(web-sys|js-sys|wasm-bindgen.*|fandhe-frontend.*)$'

# 判定の jq プログラム。違反を 1 行ずつ出力する。値は --argjson / --arg で渡し、文字列連結しない。
# shellcheck disable=SC2016 # $ は jq の変数であり、シェルに展開させない
JQ_PROGRAM='
  . as $m
  | ($m.packages | map({key: .id, value: .name}) | from_entries) as $n
  | ($m.workspace_members | map($n[.])) as $mem
  | ($m.workspace_default_members | map($n[.])) as $def
  | def step($ids): [ $m.resolve.nodes[] | select(.id as $i | $ids | index($i))
        | .deps[] | select(any(.dep_kinds[]; .kind != "dev")) | .pkg ];
    def closure($s): {ids: ($s | unique)}
        | until(((.ids + step(.ids)) | unique | length) == (.ids | length);
                .ids = ((.ids + step(.ids)) | unique))
        | .ids;
  (
    (($mem - $exp) + ($exp - $mem) | unique | .[] | "member set mismatch: \(.)"),
    (($def - $expdef) + ($expdef - $def) | unique | .[] | "default-members mismatch: \(.)"),
    ( $m.resolve.nodes[]
      | select(.id as $i | $m.workspace_members | index($i))
      | ($n[.id]) as $from
      | .deps[]
      | ($n[.pkg]) as $to
      | select($mem | index($to))
      | .dep_kinds[]
      | (.kind // "normal") as $k
      | select(("\($from)>\($to)>\($k)") as $e | $allow | index($e) | not)
      | "forbidden edge: \($from) -> \($to) (\($k))" ),
    ( [ $m.workspace_members[] | select(($n[.] == "fandhe-workshop-contract") or ($n[.] == "fandhe-workshop-core")) ] as $roots
      | closure($roots)[] | $n[.] | select(test($forbid))
      | "forbidden crate in contract/core closure: \(.)" )
  )
'

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT
META="${TMP_DIR}/metadata.json"

cd "${ROOT_DIR}"
# --locked: 作業ツリーの Cargo.lock を書き換えない。--filter-platform は付けず、wasm32 限定の依存も含める。
if ! cargo metadata --format-version 1 --locked --all-features ${MANIFEST_ARGS[@]+"${MANIFEST_ARGS[@]}"} >"${META}" 2>"${TMP_DIR}/cargo.err"; then
  echo "error: cargo metadata failed; cannot check dependencies (if Cargo.lock is stale, run \`cargo update --workspace\`)" >&2
  head -n 5 "${TMP_DIR}/cargo.err" >&2
  exit 2
fi
if ! jq -e '.resolve != null and (.resolve.nodes | length) > 0' "${META}" >/dev/null 2>&1; then
  echo "error: cargo metadata returned no resolved dependency graph" >&2
  exit 2
fi

if ! jq -r \
  --argjson exp "${EXPECTED_MEMBERS}" \
  --argjson expdef "${EXPECTED_DEFAULTS}" \
  --argjson allow "${ALLOWED_EDGES}" \
  --arg forbid "${FORBIDDEN_PATTERN}" \
  "${JQ_PROGRAM}" "${META}" >"${TMP_DIR}/violations.txt" 2>"${TMP_DIR}/jq.err"; then
  echo "error: jq failed while checking dependencies" >&2
  head -n 5 "${TMP_DIR}/jq.err" >&2
  exit 2
fi

if [ -s "${TMP_DIR}/violations.txt" ]; then
  while IFS= read -r line; do
    echo "error: ${line}" >&2
  done <"${TMP_DIR}/violations.txt"
  exit 1
fi

log "ok: 4 members, 3 default-members, 0 violations"
