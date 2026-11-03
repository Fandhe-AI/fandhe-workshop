#!/usr/bin/env bash
# scripts/check-second-pack-repro.sh の回帰テスト（`make test-check-second-pack-repro`。本物の fandhe-pack を使うため
# cargo が要る。TASK-413 1/2）。
#
# 確かめること: 誤用の拒否（終了コード 2）・変えていない写しの合格・写しを 1 か所ずつ壊すと対応する項目が
# 不合格になること（負の対照。置き換えが 1 回も起きないときはテスト自体を失敗にする）。
# 終了コード: 0 = 全件期待どおり / 1 = 期待と違う件がある
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
reject_args "$(basename "$0")" "$@"

SCRIPT="${DIR}/check-second-pack-repro.sh"
SRC="${ROOT_DIR}/docs/second-pack-repro"

require_cmd cargo
require_cmd mktemp
require_cmd rm
require_cmd cp
require_cmd sed
require_cmd cmp
require_cmd ln
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
expect_rc "親ディレクトリの要素を含む値は 2" 2 --pack "${SRC}/../second-pack-repro/pack"
expect_rc "- で始まる値は 2" 2 --pack -x
expect_rc "workshop.yaml の無い値は 2" 2 --pack "${WORK}"
expect_rc "変えていない写しは 0" 0 --pack "${SRC}/pack"

# mutate <名前> <ファイル(pack からの相対)> <sed の式> <期待する FAIL の項目>: 写しを作り直し、1 か所を書き換える。
mutate() {
  local name="$1" file="$2" expr="$3" item="$4" copy="${WORK}/m"
  rm -rf "${copy}"
  mkdir -p "${copy}"
  cp -R "${SRC}/pack" "${copy}/pack"
  cp "${SRC}/baseline.txt" "${copy}/baseline.txt"
  sed -E "${expr}" "${copy}/pack/${file}" >"${copy}/edited"
  if cmp -s "${copy}/edited" "${copy}/pack/${file}"; then
    ng "${name} (置き換えが起きていない)"
    return
  fi
  cp "${copy}/edited" "${copy}/pack/${file}"
  expect_rc "${name}" 1 --pack "${copy}/pack"
  if printf '%s' "${OUT}" | grep -q "^FAIL ${item}"; then ok "${name}: ${item} が FAIL"; else ng "${name}: ${item} が FAIL になっていない"; fi
}

mutate "入力欄を出す変異" "workshop.yaml" 's/input_field: hidden/input_field: visible/' constraints
mutate "外観を変える変異" "workshop.yaml" 's/theme: reading/theme: terminal/' constraints
mutate "壊れた YAML の変異" "workshop.yaml" 's/^pack:/pack: [/' validate

mutate "作者の IR を 1 バイト変える変異" "tests/ir.json" '2s/format_version/format_versioN/' ir-match
mutate "本文の文面を変える変異" "chapters/c-review/st-1.md" 's/所要は 10 分程度/所要は 十分程度/' ir-match

# 基準の IR が無い・シンボリックリンクの写しは 1（fail-closed）。
for kind in missing symlink; do
  icopy="${WORK}/i-${kind}"
  mkdir -p "${icopy}"
  cp -R "${SRC}/pack" "${icopy}/pack"
  cp "${SRC}/baseline.txt" "${icopy}/baseline.txt"
  rm -f "${icopy}/pack/tests/ir.json"
  if [ "${kind}" = symlink ]; then ln -s "${SRC}/pack/tests/ir.json" "${icopy}/pack/tests/ir.json"; fi
  expect_rc "ir.json が ${kind} の写しは 1" 1 --pack "${icopy}/pack"
  if printf '%s' "${OUT}" | grep -q '^FAIL ir-match'; then ok "ir.json ${kind}: ir-match が FAIL"; else ng "ir.json ${kind}: ir-match が FAIL になっていない"; fi
done

# 件数のずれ: 基準値のほうを書き換える。
bcopy="${WORK}/b"
mkdir -p "${bcopy}"
cp -R "${SRC}/pack" "${bcopy}/pack"
sed -E 's/^total_inputs=.*/total_inputs=1/' "${SRC}/baseline.txt" >"${bcopy}/baseline.txt"
expect_rc "件数のずれは 1" 1 --pack "${bcopy}/pack"
if printf '%s' "${OUT}" | grep -q '^FAIL baseline'; then ok "件数のずれ: baseline が FAIL"; else ng "件数のずれ: baseline が FAIL になっていない"; fi

# baseline.txt が無い写しは 2（fail-closed）。
ncopy="${WORK}/n"
mkdir -p "${ncopy}"
cp -R "${SRC}/pack" "${ncopy}/pack"
expect_rc "baseline.txt の無い写しは 2" 2 --pack "${ncopy}/pack"

if [ "${FAILS}" -ne 0 ]; then
  echo "${FAILS} test(s) failed" >&2
  exit 1
fi
echo "all tests passed"
