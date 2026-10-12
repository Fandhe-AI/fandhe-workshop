#!/usr/bin/env bash
# PoC-7 手順 5・6: 過大入力の決定的な生成。乱数を使わない。
# 使い方:
#   generate.sh markdown <バイト数> > out.md
#   generate.sh history  <バイト数> > out.json
#   generate.sh pack     <合計バイト数> <出力ディレクトリ>
#   generate.sh input    <文字数> > out.txt
#   generate.sh nested   <段数>    > out.json
#   generate.sh bigarray <要素数>  > out.json
set -euo pipefail
mode="${1:?mode}"; n="${2:?size}"

fill() { # fill <バイト数>: 1 行 "あいうえお 0123456789 abcdefghij\n" を繰り返し、最後は x で埋める
  local want="$1" line="あいうえお 0123456789 abcdefghij"$'\n' ll
  ll="$(printf '%s' "${line}" | wc -c | tr -d ' ')"
  local full=$(( want / ll )) rest=$(( want % ll ))
  local i=0
  while [ "${i}" -lt "${full}" ]; do printf '%s' "${line}"; i=$((i + 1)); done
  [ "${rest}" -gt 0 ] && head -c "${rest}" /dev/zero | tr '\0' 'x'
  return 0
}

case "${mode}" in
  markdown) fill "${n}" ;;
  history)
    head="{\"version\":1,\"entries\":["; tail="]}"
    elem="{\"step\":\"st-1\",\"text\":\"xxxxxxxxxxxxxxxx\"},"
    last_fixed=25 # {"step":"st-1","text":"  と  "}  の合計（最後の要素の text 以外）
    budget=$(( n - ${#head} - ${#tail} - last_fixed ))
    cnt=$(( budget / ${#elem} )); pad=$(( budget - cnt * ${#elem} ))
    printf '%s' "${head}"
    i=0; while [ "${i}" -lt "${cnt}" ]; do printf '%s' "${elem}"; i=$((i + 1)); done
    printf '{"step":"st-1","text":"'
    [ "${pad}" -gt 0 ] && head -c "${pad}" /dev/zero | tr '\0' 'x'
    printf '"}%s' "${tail}" ;;
  pack)
    out="${3:?出力ディレクトリ}"; mkdir -p "${out}"
    per=500000; k=1; left="${n}"
    while [ "${left}" -gt 0 ]; do
      sz=$(( left < per ? left : per ))
      fill "${sz}" > "${out}/pad-$(printf '%04d' "${k}").md"
      left=$(( left - sz )); k=$(( k + 1 ))
    done ;;
  input) i=0; while [ "${i}" -lt "${n}" ]; do printf 'あ'; i=$((i + 1)); done ;;
  nested)
    head -c "${n}" /dev/zero | tr '\0' '['; head -c "${n}" /dev/zero | tr '\0' ']' ;;
  bigarray)
    printf '['; i=1; while [ "${i}" -lt "${n}" ]; do printf '0,'; i=$((i + 1)); done; printf '0]' ;;
  *) echo "mode が不正" >&2; exit 1 ;;
esac
