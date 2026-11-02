#!/usr/bin/env bash
# scripts/check-second-pack.sh の回帰テスト（`make test-check-second-pack`。本物の fandhe-pack を使うため cargo が要り、やや遅い。TASK-743）。
#
# 確かめること: 誤用の拒否（終了コード 2）・変えていない写しの合格・設定値や文面を 1 か所ずつ壊した写しが
# 対応する項目で不合格になること（負の対照。置き換えが写しの中でちょうど 1 回起きない場合はテスト自体を失敗にする）。
# 終了コード: 0 = 全件期待どおり / 1 = 期待と違う件がある
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

SCRIPT="${DIR}/check-second-pack.sh"
SRC="${ROOT_DIR}/template-workshop/examples/review-request"

require_cmd cargo
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
  if [ "${rc}" -eq "${want}" ]; then ok "${name}"; else ng "${name}（終了コード ${rc}、期待 ${want}）"; fi
}

expect_rc "未知の引数は 2" 2 --bogus
expect_rc "--pack の値なしは 2" 2 --pack
expect_rc "親ディレクトリの要素を含む値は 2" 2 --pack "${SRC}/../review-request"
expect_rc "- で始まる値は 2" 2 --pack -x
expect_rc "workshop.yaml の無い値は 2" 2 --pack "${WORK}"
expect_rc "変えていない写しは 0" 0 --pack "${SRC}"

# mutate <名前> <相対パス> <sed の式> <期待する FAIL の項目>: 写しを作り、置き換えがちょうど 1 行に効くことを確かめる。
n=0
mutate() {
  local name="$1" rel="$2" expr="$3" item="$4" dir before after
  n=$((n + 1))
  dir="${WORK}/m${n}"
  cp -R "${SRC}" "${dir}"
  before="$(cat "${dir}/${rel}")"
  sed "${expr}" "${dir}/${rel}" >"${dir}/tmp.out"
  mv "${dir}/tmp.out" "${dir}/${rel}"
  after="$(cat "${dir}/${rel}")"
  if [ "${before}" = "${after}" ]; then
    ng "${name}（置き換えが効かない）"
    return
  fi
  expect_rc "${name}は 1" 1 --pack "${dir}"
  if printf '%s\n' "${OUT}" | grep -q "^FAIL ${item}:"; then ok "${name}は ${item} で不合格"; else ng "${name}（FAIL ${item} の行が無い）"; fi
}

W="workshop.yaml"
# BSD sed と GNU sed の両方で通る改行の渡し方（置換文字列の中は バックスラッシュ + 実改行）
NL="$(printf '\nX')"; NL="${NL%X}"
BS="\\" # 1 文字のバックスラッシュ
SED_VIEWS="s/^  v-doc: { kind: document }/  v-doc: { kind: document }${BS}${NL}  v-x: { kind: document }/"
SED_STARTS="s/^steps:/  - { id: s-2, node: st-1 }${BS}${NL}steps:/"
mutate "外観を standard にした写し" "${W}" 's/theme: reading/theme: standard/' appearance
mutate "入力欄を出す写し" "${W}" 's/input_field: hidden/input_field: shown/' profile
mutate "command_prefix を消した写し" "${W}" '/command_prefix:/d' profile
mutate "ビューの切り替えを出す写し" "${W}" 's/view_tabs: hidden/view_tabs: shown/' left_pane
mutate "コマンドを 1 件持つ写し" "${W}" 's/^commands: {}/commands: { noop: { args: [] } }/' commands
mutate "ビューを 1 件足した写し" "${W}" "${SED_VIEWS}" views
mutate "文面に禁止語を足した写し" "chapters/ch-request/st-1.md" 's/^# 状況を確認する/# 状況を確認する（掲示板）/' no-board-words
mutate "開始点を 1 件足した写し" "chapters/ch-request/chapter.yaml" "${SED_STARTS}" starts

if [ "${FAILS}" -ne 0 ]; then
  echo "NG: ${FAILS} 件が期待と違う" >&2
  exit 1
fi
echo "OK: test-check-second-pack"
