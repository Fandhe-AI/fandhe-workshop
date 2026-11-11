#!/usr/bin/env bash
# scripts/test-ime-composing-control.sh と scripts/lib-ime-control.sh の回帰テスト（#2123）。
# 誤用の拒否・判定の関数（合成の出力）・変異の定義の解決を確かめる。ビルド・ブラウザは使わない。
# `make test-lib-ime-control` と CI の check ジョブから呼ばれる。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"
REAL_ROOT="$(cd "${ROOT_DIR}" && pwd -P)"
# shellcheck source=./lib-mutations.sh
source "${DIR}/lib-mutations.sh"
# shellcheck source=./lib-ime-control.sh
source "${DIR}/lib-ime-control.sh"
require_cmd mktemp
require_cmd grep
require_cmd rm
reject_args "$(basename "$0")" "$@"
fail=0

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-test-lib-ime-control.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

pass() { echo "ok: $1"; }
bad() { echo "FAIL: $1" >&2; fail=1; }

expect_rc() {
  local want="$1" desc="$2"
  shift 2
  local rc=0
  "$@" >/dev/null 2>&1 || rc=$?
  if [ "${rc}" -ne "${want}" ]; then bad "${desc} (exit ${rc}, want ${want})"; else pass "${desc}"; fi
}

expect_eq() {
  local want="$1" got="$2" desc="$3"
  if [ "${got}" = "${want}" ]; then pass "${desc}"; else bad "${desc} (got '${got}', want '${want}')"; fi
}

expect_rc 2 "control script rejects a positional argument" "${DIR}/test-ime-composing-control.sh" extra
expect_rc 2 "control script rejects an unknown option" "${DIR}/test-ime-composing-control.sh" --nope

# 判定の関数。出力ファイルは合成する。
mark="${TMP}/mark.txt"
printf '✖ S-A 下書き 1 行 IME-MIS-SEND 誤送信 3 件 行 1\n' >"${mark}"
title="${TMP}/title.txt"
printf '✖ S-A 試験文 30 文 変換中の Enter で誤送信 0 件、確定後の Enter で送信漏れ 0 件\nIME-MIS-SEND 誤送信 0 件\n' >"${title}"
mount="${TMP}/mount.txt"
printf 'mount failed\nIME-MIS-SEND 誤送信 2 件\n' >"${mount}"
plain="${TMP}/plain.txt"
printf '✖ something else\n' >"${plain}"

expect_eq not-established "$(judge_m1 0 "${mark}" 2>/dev/null)" "M1: rc 0 is not established"
expect_eq other "$(judge_m1 1 "${mount}" 2>/dev/null)" "M1: mount failure is another reason"
expect_eq other "$(judge_m1 1 "${plain}" 2>/dev/null)" "M1: failure without the mark is another reason"
expect_eq other "$(judge_m1 1 "${title}" 2>/dev/null)" "M1: zero-count mark is another reason"
expect_eq established "$(judge_m1 1 "${mark}" 2>/dev/null)" "M1: marked failure with a count is established"
expect_eq survived "$(judge_m3 0 "${plain}" 2>/dev/null)" "M3: rc 0 is survived"
esc="${TMP}/esc.txt"
printf '✖ S-B 取り消しの 3 文 待機中の変換中の Esc はログも世代も変えない\nAssertionError [ERR_ASSERTION]: IME-ESC-STOP 停止 1 件 S-01 待機中\n' >"${esc}"
# 集計の失敗: 試験名に「変換中の Esc」を含むが、Esc の後の表明ではない（行列が不完全なだけ）。
summary="${TMP}/summary.txt"
printf '✖ 集計 試験文 30 文 × 下書き 3 通りで、誤送信 0 件・確定後の送信漏れ 0 件・変換中の Esc での停止 0 件\nAssertionError [ERR_ASSERTION]: 行列の全ての組が積まれている\n' >"${summary}"
# Esc の前の表明の失敗: S-C の runStatus が busy の表明。
prebusy="${TMP}/prebusy.txt"
printf '✖ S-C S-01 処理中の変換中の Esc は世代・停止・進行を変えない\nAssertionError [ERR_ASSERTION]: S-01 処理中 (模擬) 1 !== 0\n' >"${prebusy}"
# 件数 0 の印（印の取り違え）。
zero="${TMP}/zero.txt"
printf '✖ x\nAssertionError: IME-ESC-STOP 停止 0 件\n' >"${zero}"
expect_eq other "$(judge_m3 1 "${summary}" 2>/dev/null)" "M3: summary failure is another reason"
expect_eq other "$(judge_m3 1 "${prebusy}" 2>/dev/null)" "M3: failure before the Esc assertion is another reason"
expect_eq other "$(judge_m3 1 "${zero}" 2>/dev/null)" "M3: zero-count mark is another reason"
expect_eq other "$(judge_m3 1 "${plain}" 2>/dev/null)" "M3: failure without an Esc assertion is another reason"
expect_eq other "$(judge_m3 3 "${plain}" 2>/dev/null)" "M3: launch failure (rc 3) is another reason"
expect_eq detected "$(judge_m3 1 "${esc}" 2>/dev/null)" "M3: Esc assertion failure is detected"
expect_eq other "$(judge_m3 1 "${mount}" 2>/dev/null)" "M3: mount failure is another reason"

expect_eq 0 "$(combine_rc established survived)" "combine: established + survived"
expect_eq 3 "$(combine_rc established detected)" "combine: M3 detected"
expect_eq 1 "$(combine_rc not-established survived)" "combine: M1 not established"
expect_eq 2 "$(combine_rc other survived)" "combine: M1 other"
expect_eq 2 "$(combine_rc established other)" "combine: M3 other"
expect_eq 2 "$(combine_rc not-established other)" "combine: not-established + other"
expect_eq 1 "$(combine_rc not-established detected)" "combine: not-established + detected"

# 定義の解決（実 repo の読み取りだけ）。
DEF_DIR="${ROOT_DIR}/crates/contract/tests/mutations"
load_variant "${DEF_DIR}/JUDGE14-2.mutation"
m1_target="${M_TARGET}" m1_find="${M_FIND}" m1_repl="${M_REPLACE}"
load_variant "${DEF_DIR}/JUDGE16-1.mutation"
expect_eq "${m1_target}" "${M_TARGET}" "M1 and M3 target the same file"
expect_eq 1 "$(count_occurrences "${ROOT_DIR}/${m1_target}" "${m1_find}")" "M1 find occurs exactly once"
expect_eq 1 "$(count_occurrences "${ROOT_DIR}/${M_TARGET}" "${M_FIND}")" "M3 find occurs exactly once"
case "${m1_repl}" in *'"Enter"'*) bad "M1 replacement still has Enter" ;; *) pass "M1 replacement drops Enter" ;; esac
case "${M_REPLACE}" in *'"Escape"'*) bad "M3 replacement still has Escape" ;; *) pass "M3 replacement drops Escape" ;; esac

# 不正な定義（target が ".."）は 2 で拒否される。
badref="${TMP}/bad.mutation"
{
  echo "id: X-1"
  echo "package: fandhe-workshop-contract"
  echo "test: key_decide_contract"
  echo "target: ../x"
  echo "find: a"
  echo "replace: b"
  echo "expect-fail: t"
} >"${badref}"
rc=0
(load_variant "${badref}") >/dev/null 2>&1 || rc=$?
expect_eq 2 "${rc}" "definition with a parent-segment target is rejected"

exit "${fail}"
