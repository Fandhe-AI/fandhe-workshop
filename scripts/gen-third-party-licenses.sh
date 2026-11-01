#!/usr/bin/env bash
# fandhe-pack の配布物に同梱する THIRD-PARTY-LICENSES を Cargo.lock から決定的に作る（LIC-9・D-80。#1957）。
#
# 目的: 静的リンクした依存（native）と、埋め込んだ web の配信物の依存（wasm32）のライセンス文を、配布物に載せる。
# 呼び出し元は scripts/build-pack-release.sh（梱包の前、ビルドの前に呼び、方針の違反で長いビルドを無駄にしない）。
# 回帰テストは scripts/test-build-pack-release.sh。新しい外部ツールは使わない（bash・jq・cargo だけ）。
#
# 対象: fandhe-workshop-pack（--target の triple）と fandhe-workshop-web（wasm32-unknown-unknown）から、
#   通常の依存（dep_kinds の kind が null の辺）だけを辿った crate。proc-macro の crate（とその先）・ビルド時と試験の依存・
#   ワークスペースのメンバーは載せない。
# 本文: crate の配布物にあるライセンスのファイル（ルート直下の LICENSE*・COPYING*・UNLICENSE*・NOTICE*・COPYRIGHT*、
#   metadata の license_file、.licenses/・licenses/・LICENSES/ の 1 段下）。1 つも無い crate は、license 式の識別子ごとの
#   標準の本文（MIT は tools/pack-release/licenses/MIT.txt、出所は https://spdx.org/licenses/MIT.html。Apache-2.0 は
#   LICENSE-APACHE）で補い、ヘッダの一覧に示す。標準の本文を持たない識別子が出れば失敗する。
# fail-closed: 許可の集合に無い license 式（OR の一方だけでも）・WITH・license の欠如・registry でない source・
#   symlink のライセンスのファイルは、出力を作らず終了コード 1。
# 決定性: 日付・絶対パス・ホスト名を出力に入れない。並びは LC_ALL=C の (名前, 版)。通信しない（cargo metadata は --offline）。
#
# 使い方: scripts/gen-third-party-licenses.sh --target <triple> --out <file> [--native <json> --wasm <json>]
#   --native/--wasm  試験用。cargo metadata の JSON の差し込み（2 つ揃えて渡す）。本番の呼び出しでは渡さない
# 終了コード: 0 = 成功 / 1 = 方針の違反 / 2 = 誤用・道具が無い・読めない入力
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

usage() {
  echo "usage: $(basename "$0") --target <triple> --out <file> [--native <metadata.json> --wasm <metadata.json>]" >&2
  exit 2
}

# 許可する SPDX の識別子。足すときはレビューを要する。
#   MIT, Apache-2.0  許容型
#   Unlicense        パブリックドメイン相当（memchr）
#   BSL-1.0          許容型（ryu は Apache-2.0 との OR）
#   Unicode-3.0      許容型のデータのライセンス（unicode-ident の AND。本文は同梱の LICENSE-UNICODE）
readonly ALLOWED_IDS="MIT Apache-2.0 Unlicense BSL-1.0 Unicode-3.0"
readonly WASM_TRIPLE="wasm32-unknown-unknown"

TARGET="aarch64-apple-darwin"
OUT=""
NATIVE_JSON=""
WASM_JSON=""
while [ "$#" -gt 0 ]; do
  [ "$#" -ge 2 ] || usage
  case "$1" in
    --target) TARGET="$2" ;;
    --out) OUT="$2" ;;
    --native) NATIVE_JSON="$2" ;;
    --wasm) WASM_JSON="$2" ;;
    *) usage ;;
  esac
  shift 2
done
[ -n "${OUT}" ] || usage
case "${TARGET}" in
  aarch64-apple-darwin | x86_64-unknown-linux-gnu) ;;
  *)
    echo "error: unsupported --target: ${TARGET}" >&2
    exit 2
    ;;
esac
if [ -n "${NATIVE_JSON}" ] || [ -n "${WASM_JSON}" ]; then
  if [ -z "${NATIVE_JSON}" ] || [ -z "${WASM_JSON}" ]; then
    echo "error: --native and --wasm must be given together" >&2
    exit 2
  fi
  for f in "${NATIVE_JSON}" "${WASM_JSON}"; do
    [ -f "${f}" ] || {
      echo "error: metadata file not found: ${f}" >&2
      exit 2
    }
  done
fi
for c in jq mktemp find sort tr grep; do
  require_cmd "${c}" || exit 2
done
if [ -z "${NATIVE_JSON}" ]; then
  require_cmd cargo || exit 2
fi
case "${OUT}" in
  /*) ;;
  *) OUT="$(pwd)/${OUT}" ;;
esac

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT
export LC_ALL=C

if [ -z "${NATIVE_JSON}" ]; then
  NATIVE_JSON="${TMP_DIR}/native.json"
  WASM_JSON="${TMP_DIR}/wasm.json"
  # --all-features は付けない（配信する設定に入らない試験用の依存まで拾うため）
  (cd "${ROOT_DIR}" && cargo metadata --format-version 1 --locked --offline --filter-platform "${TARGET}" >"${NATIVE_JSON}" 2>"${TMP_DIR}/c1.err") || {
    echo "error: cargo metadata --offline failed (run cargo fetch --locked first)" >&2
    sed 's/^/  | /' "${TMP_DIR}/c1.err" >&2
    exit 2
  }
  (cd "${ROOT_DIR}" && cargo metadata --format-version 1 --locked --offline --filter-platform "${WASM_TRIPLE}" >"${WASM_JSON}" 2>"${TMP_DIR}/c2.err") || {
    echo "error: cargo metadata --offline (${WASM_TRIPLE}) failed (run cargo fetch --locked first)" >&2
    sed 's/^/  | /' "${TMP_DIR}/c2.err" >&2
    exit 2
  }
fi

# 通常の依存の閉包。起点から kind が null の辺だけを辿り、proc-macro の crate は入れず先も辿らない。
# ワークスペースのメンバーは辿るが一覧に出さない。出力は TSV:
# 名前 版 license license_file source manifest_path repository triple
# shellcheck disable=SC2016 # jq の変数（$m など）をシェルに展開させない
readonly JQ_CLOSURE='
  . as $m
  | ($m.packages | map({key: .id, value: .}) | from_entries) as $pk
  | ($m.resolve.nodes | map({key: .id, value: ([.deps[] | select(any(.dep_kinds[]; .kind == null)) | .pkg])}) | from_entries) as $adj
  | ($m.packages | map(select(any(.targets[]; any(.kind[]; . == "proc-macro"))) | .id)) as $pm
  | ($m.workspace_members) as $ws
  | ($m.packages | map(select(.name == $root)) | .[0].id) as $rid
  | if $rid == null then error("root package not found: " + $root) else . end
  | {seen: [$rid], frontier: [$rid]}
  | until(.frontier == [];
      . as $s
      | ([$s.frontier[] | ($adj[.] // [])[]] | unique) as $next
      | ($next - $s.seen - $pm) as $new
      | {seen: ($s.seen + $new), frontier: $new})
  | .seen[]
  | select(. as $id | $ws | index($id) | not)
  | $pk[.]
  | [.name, .version, (.license // "-"), (.license_file // "-"), (.source // "-"), .manifest_path, (.repository // "-"), $triple]
  | @tsv'

closure() {
  jq -r --arg root "$1" --arg triple "$2" "${JQ_CLOSURE}" "$3"
}

closure "fandhe-workshop-pack" "${TARGET}" "${NATIVE_JSON}" >"${TMP_DIR}/rows" || {
  echo "error: cannot read native metadata" >&2
  exit 2
}
closure "fandhe-workshop-web" "${WASM_TRIPLE}" "${WASM_JSON}" >>"${TMP_DIR}/rows" || {
  echo "error: cannot read wasm32 metadata" >&2
  exit 2
}
tab="$(printf '\t')"
sort -t "${tab}" -k1,1 -k2,2 -k8,8 "${TMP_DIR}/rows" >"${TMP_DIR}/rows.sorted"

VIOLATIONS=0
violation() {
  echo "error: $*" >&2
  VIOLATIONS=$((VIOLATIONS + 1))
}

# normalize_expr <式>: / を OR に、括弧を空白にし、識別子（と OR・AND・WITH）を 1 行ずつ出す
normalize_expr() {
  local norm="$1"
  norm="${norm//\// OR }"
  norm="${norm//(/ }"
  norm="${norm//)/ }"
  # shellcheck disable=SC2020 # 空白と tab の両方を改行へ写す（集合の対応を揃えている）
  printf '%s\n' "${norm}" | tr -s ' \t' '\n\n'
}

# license_ok <式>: すべての識別子が許可の集合にあれば 0。WITH・識別子 0 個・集合外は 1
license_ok() {
  local w found=0
  while read -r w; do
    case "${w}" in
      "" | OR | AND) ;;
      WITH) return 1 ;;
      *)
        found=1
        case " ${ALLOWED_IDS} " in
          *" ${w} "*) ;;
          *) return 1 ;;
        esac
        ;;
    esac
  done <<EOT
$(normalize_expr "$1")
EOT
  [ "${found}" -eq 1 ]
}

# license_ids <式>: 識別子を 1 行ずつ（OR・AND を除く）
license_ids() {
  normalize_expr "$1" | grep -vxE 'OR|AND' | grep -v '^$' || true
}

# standard_text <識別子>: 標準の本文のパス。無ければ 1
standard_text() {
  case "$1" in
    MIT) echo "${ROOT_DIR}/tools/pack-release/licenses/MIT.txt" ;;
    Apache-2.0) echo "${ROOT_DIR}/LICENSE-APACHE" ;;
    *) return 1 ;;
  esac
}

# collect_files <crate dir> <license_file>: ライセンスのファイルの相対パスを files.list に書く。
# symlink・crate の外への参照・実体の欠如は 1
collect_files() {
  local cdir="$1" lf="$2" name rel sub p bad=0
  : >"${TMP_DIR}/files.raw"
  if [ -n "${lf}" ]; then
    case "${lf}" in
      "${cdir}"/*) lf="${lf#"${cdir}"/}" ;;
    esac
    case "${lf}" in
      /* | .. | ../* | */../* | */..)
        echo "license_file escapes the crate: ${lf}" >&2
        return 1
        ;;
    esac
    printf '%s\n' "${lf}" >>"${TMP_DIR}/files.raw"
  fi
  while IFS= read -r p; do
    [ -n "${p}" ] || continue
    name="${p##*/}"
    if printf '%s\n' "${name}" | grep -qiE '^(licen[cs]e|copying|unlicense|notice|copyright)'; then
      printf '%s\n' "${name}" >>"${TMP_DIR}/files.raw"
    fi
  done <<EOT
$(find "${cdir}" -maxdepth 1 -mindepth 1 \( -type f -o -type l \) 2>/dev/null)
EOT
  for sub in .licenses licenses LICENSES; do
    if [ -d "${cdir}/${sub}" ] && [ ! -L "${cdir}/${sub}" ]; then
      while IFS= read -r p; do
        [ -n "${p}" ] || continue
        printf '%s\n' "${sub}/${p##*/}" >>"${TMP_DIR}/files.raw"
      done <<EOT
$(find "${cdir}/${sub}" -maxdepth 1 -mindepth 1 \( -type f -o -type l \) 2>/dev/null)
EOT
    fi
  done
  sort -u "${TMP_DIR}/files.raw" >"${TMP_DIR}/files.list"
  while IFS= read -r rel; do
    [ -n "${rel}" ] || continue
    if [ -L "${cdir}/${rel}" ]; then
      echo "license file is a symlink: ${rel}" >&2
      bad=1
    elif [ ! -f "${cdir}/${rel}" ]; then
      echo "license file not found: ${rel}" >&2
      bad=1
    fi
  done <"${TMP_DIR}/files.list"
  [ "${bad}" -eq 0 ]
}

# emit_file <パス> <表示名>: 区切りと本文（末尾に改行が無ければ 1 つ足す）
emit_file() {
  printf -- '---- %s ----\n' "$2"
  cat "$1"
  if [ -s "$1" ] && [ -n "$(tail -c 1 "$1")" ]; then
    printf '\n'
  fi
}

: >"${TMP_DIR}/body"
: >"${TMP_DIR}/fallback"
COUNT=0

emit_crate() {
  local name="$1" ver="$2" lic="$3" lf="$4" src="$5" mpath="$6" repo="$7" triples="$8"
  local cdir ids id txt rel
  # 空の欄は jq が "-" で渡す（read が連続する tab を 1 つに潰すため）
  [ "${lic}" != "-" ] || lic=""
  [ "${lf}" != "-" ] || lf=""
  [ "${src}" != "-" ] || src=""
  [ "${repo}" != "-" ] || repo=""
  cdir="$(dirname "${mpath}")"
  case "${src}" in
    registry+*) ;;
    *)
      violation "source is not a registry: ${name} ${ver}: ${src:-(none)}"
      return 0
      ;;
  esac
  if [ -z "${lic}" ]; then
    violation "disallowed license: ${name} ${ver}: (no license expression)"
    return 0
  fi
  if ! license_ok "${lic}"; then
    violation "disallowed license: ${name} ${ver}: ${lic}"
    return 0
  fi
  if [ ! -d "${cdir}" ]; then
    violation "crate directory not found: ${name} ${ver}"
    return 0
  fi
  if ! collect_files "${cdir}" "${lf}" 2>"${TMP_DIR}/cf.err"; then
    violation "license files of ${name} ${ver}: $(tr '\n' ' ' <"${TMP_DIR}/cf.err")"
    return 0
  fi
  ids="$(license_ids "${lic}")"
  if [ ! -s "${TMP_DIR}/files.list" ]; then
    # 標準の本文を持たない識別子があれば、本文を書き始める前に違反にする
    while IFS= read -r id; do
      [ -n "${id}" ] || continue
      if ! standard_text "${id}" >/dev/null; then
        violation "no license file and no standard text for ${id}: ${name} ${ver}"
        return 0
      fi
    done <<EOT
${ids}
EOT
  fi
  {
    printf '==== %s %s ====\n' "${name}" "${ver}"
    printf 'license: %s\n' "${lic}"
    printf 'targets: %s\n' "${triples}"
    printf 'repository: %s\n' "${repo:-(none)}"
  } >>"${TMP_DIR}/body"
  if [ -s "${TMP_DIR}/files.list" ]; then
    while IFS= read -r rel; do
      [ -n "${rel}" ] || continue
      emit_file "${cdir}/${rel}" "${rel}" >>"${TMP_DIR}/body"
    done <"${TMP_DIR}/files.list"
  else
    while IFS= read -r id; do
      [ -n "${id}" ] || continue
      txt="$(standard_text "${id}")"
      emit_file "${txt}" "SPDX ${id} (standard text)" >>"${TMP_DIR}/body"
    done <<EOT
${ids}
EOT
    printf '%s %s (%s)\n' "${name}" "${ver}" "${repo:-no repository}" >>"${TMP_DIR}/fallback"
  fi
  printf '\n' >>"${TMP_DIR}/body"
  COUNT=$((COUNT + 1))
}

# 同じ (名前, 版) の行を 1 つにまとめ、triple を集める
pn="" pv="" pl="" plf="" ps="" pm="" pr="" pt=""
while IFS="${tab}" read -r n v l lf s m r t; do
  if [ "${n}" = "${pn}" ] && [ "${v}" = "${pv}" ]; then
    pt="${pt}, ${t}"
    continue
  fi
  if [ -n "${pn}" ]; then
    emit_crate "${pn}" "${pv}" "${pl}" "${plf}" "${ps}" "${pm}" "${pr}" "${pt}"
  fi
  pn="${n}" pv="${v}" pl="${l}" plf="${lf}" ps="${s}" pm="${m}" pr="${r}" pt="${t}"
done <"${TMP_DIR}/rows.sorted"
if [ -n "${pn}" ]; then
  emit_crate "${pn}" "${pv}" "${pl}" "${plf}" "${ps}" "${pm}" "${pr}" "${pt}"
fi

if [ "${VIOLATIONS}" -ne 0 ]; then
  echo "error: ${VIOLATIONS} violation(s); THIRD-PARTY-LICENSES not generated" >&2
  exit 1
fi
if [ "${COUNT}" -eq 0 ]; then
  echo "error: no third-party crates found (empty closure)" >&2
  exit 1
fi

{
  echo "第三者のライセンス（fandhe-pack に静的リンクした依存と、埋め込んだ web の配信物の依存）"
  echo
  echo "このファイルは Cargo.lock の解決結果から機械的に作ったものです。"
  echo "対象は次の 2 つの、通常の依存です。"
  echo "  - fandhe-workshop-pack（${TARGET}）"
  echo "  - fandhe-workshop-web（${WASM_TRIPLE}。実行ファイルに埋め込んだ web の配信物）"
  echo "proc-macro の crate、ビルド時と試験だけの依存、このリポジトリ自身の crate は含めません。"
  echo "crate の数: ${COUNT}"
  echo
  echo "ライセンスのファイルを持たない crate:"
  if [ -s "${TMP_DIR}/fallback" ]; then
    echo "（crate の配布物にライセンスのファイルが無いため、SPDX の標準の本文を載せています。"
    echo "  著作権者は各 crate の repository の記載に従ってください。）"
    sed 's/^/  - /' "${TMP_DIR}/fallback"
  else
    echo "  なし"
  fi
  echo
  cat "${TMP_DIR}/body"
} >"${TMP_DIR}/out.txt"

mkdir -p "$(dirname "${OUT}")"
mv "${TMP_DIR}/out.txt" "${OUT}.tmp.$$"
mv "${OUT}.tmp.$$" "${OUT}"
log "wrote ${OUT} (${COUNT} crates)"
