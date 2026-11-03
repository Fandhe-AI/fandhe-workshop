#!/usr/bin/env bash
# scripts/check-third-pack.sh の回帰テスト（`make test-check-third-pack`。CI の rust-test が流す。本物の fandhe-pack を
# 使うため cargo が要る。TASK-797）。
#
# 確かめること: 誤用の拒否（終了コード 2）・変えていない教材の合格・教材の写しを 1 か所ずつ壊すと対応する項目が
# 不合格になり、狙った診断コードが validate に出ること（負の対照。置き換えが起きないときはテスト自体を失敗にする）。
# 置き換えは bash の文字列置換で行い、GNU sed 専用の書き方に頼らない（bash 3.2・BSD sed でも動く）。
# 終了コード: 0 = 全件期待どおり / 1 = 期待と違う件がある
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

SCRIPT="${DIR}/check-third-pack.sh"
SRC="${ROOT_DIR}/template-workshop/examples/task-breakdown"

require_cmd cargo
require_cmd mktemp
require_cmd rm
require_cmd cp
require_cmd cmp
require_cmd grep
if [ -z "${FANDHE_PACK_BIN:-}" ]; then
  FANDHE_PACK_BIN="$("${DIR}/build-pack-bin.sh")"
  export FANDHE_PACK_BIN
fi

WORK="$(mktemp -d)"
cleanup() {
  if [ -n "${WORK}" ] && [ -d "${WORK}" ]; then
    rm -rf "${WORK}"
  fi
}
trap cleanup EXIT

FAILS=0
ok() { echo "ok: $1"; }
ng() {
  echo "not ok: $1" >&2
  FAILS=$((FAILS + 1))
}

# expect_rc <名前> <期待の終了コード> <引数...>: 出力は OUT に残す。
OUT=""
expect_rc() {
  local name="$1" want="$2" rc=0
  shift 2
  OUT="$("${SCRIPT}" "$@" 2>&1)" || rc=$?
  if [ "${rc}" -eq "${want}" ]; then ok "${name}"; else ng "${name} (exit ${rc}, expected ${want})"; fi
}

expect_rc "未知の引数は 2" 2 --bogus
expect_rc "--pack の値なしは 2" 2 --pack
expect_rc "親ディレクトリの要素を含む値は 2" 2 --pack "${SRC}/../task-breakdown"
expect_rc "- で始まる値は 2" 2 --pack -x
expect_rc "workshop.yaml の無い値は 2" 2 --pack "${WORK}"
expect_rc "変えていない教材は 0" 0 --pack "${SRC}"

# mutate <名前> <ファイル(教材からの相対)> <置き換え前の文字列> <置き換え後の文字列> <FAIL になるべき項目(空白区切り)> <PASS のままの項目(空白区切り。無ければ空)> <診断コード(無ければ空)>
# 最初の 1 か所だけを置き換えた写しを作り、check-third-pack.sh が期待どおりに落ちることと、
# その写しの validate に診断コードが出ることを確かめる。
mutate() {
  local name="$1" file="$2" old="$3" new="$4" fail_items="$5" pass_items="$6" code="$7"
  local copy="${WORK}/m" content edited item vout
  local -a fail_arr=() pass_arr=()
  # 空白区切りの文字列を配列へ分ける（引用符なしの展開に頼らない）
  if [ -n "${fail_items}" ]; then read -r -a fail_arr <<<"${fail_items}"; fi
  if [ -n "${pass_items}" ]; then read -r -a pass_arr <<<"${pass_items}"; fi
  rm -rf "${copy}"
  cp -R "${SRC}" "${copy}"
  content="$(cat "${copy}/${file}")"
  edited="${content/"${old}"/${new}}"
  if [ "${edited}" = "${content}" ]; then
    ng "${name} (置き換えが起きていない)"
    return
  fi
  printf '%s\n' "${edited}" >"${copy}/${file}"
  expect_rc "${name}" 1 --pack "${copy}"
  for item in ${fail_arr[@]+"${fail_arr[@]}"}; do
    if printf '%s' "${OUT}" | grep -q "^FAIL ${item}:"; then ok "${name}: ${item} が FAIL"; else ng "${name}: ${item} が FAIL になっていない"; fi
  done
  for item in ${pass_arr[@]+"${pass_arr[@]}"}; do
    if printf '%s' "${OUT}" | grep -q "^PASS ${item}\$"; then ok "${name}: ${item} は PASS のまま"; else ng "${name}: ${item} が PASS のままでない"; fi
  done
  if [ -n "${code}" ]; then
    vout="$("${FANDHE_PACK_BIN}" validate "${copy}" 2>&1 || true)"
    if printf '%s' "${vout}" | grep -q ": ${code}:"; then ok "${name}: 診断 ${code} が出る"; else ng "${name}: 診断 ${code} が出ていない"; fi
  fi
}

CH="chapters/ch-tasks/chapter.yaml"
mutate "存在しない Step を指す変異" "${CH}" "next: [{ to: st-2 }]" "next: [{ to: st-9 }]" "validate test" "" "E-REF-STEP"
mutate "Step を到達不能にする変異" "${CH}" "next: [{ to: st-2 }]" "next: [{ to: st-3 }]" "validate" "" "E-UNREACHABLE"
mutate "行き止まりにする変異" "${CH}" "    final: true" "" "validate" "" "E-DEAD-END"
mutate "壊れた YAML の変異" "workshop.yaml" "pack:" "pack: [" "validate" "" "E-PARSE"
mutate "模範解答の期待を変える変異" "tests/playthrough.yaml" "{ choice: ans-big, expect: achieved }" "{ choice: ans-big, expect: accepted }" "test" "validate" ""
mutate "誤入力の期待を変える変異" "tests/playthrough.yaml" "expect: \"rejected:not_in_this_step\"" "expect: \"rejected:wrong_order\"" "test" "validate" ""

if [ "${FAILS}" -ne 0 ]; then
  echo "${FAILS} test(s) failed" >&2
  exit 1
fi
echo "all tests passed"
