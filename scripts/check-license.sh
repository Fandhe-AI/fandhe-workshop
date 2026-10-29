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
#   - 雛形 README に HTML のリンク・参照（href=・src= 等の属性。<a>・<img> 等のタグ）が無いこと（宛先を解析しないため未対応として拒否）
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

# links_extract <file>: Markdown のリンク先を 1 行 1 件で出力する。
# インラインは括弧の対応を数えて全体を取る。改行をまたぐ記法も取る（閉じが無ければ末尾まで取り、厳しい側に倒す）。
# 参照定義は `[label]:` の次の空でない行にリンク先が続く記法も扱う。
links_extract() {
  awk '
    function inline(line,   p, rest, depth, i, c, e) {
      while ((p = index(line, "](")) > 0) {
        rest = substr(line, p + 2)
        if (substr(rest, 1, 1) == "<") {
          e = index(rest, ">")
          if (e == 0) { print rest; return }
          print substr(rest, 1, e)
          line = substr(rest, e + 1)
          continue
        }
        depth = 1
        for (i = 1; i <= length(rest); i++) {
          c = substr(rest, i, 1)
          if (c == "(") depth++
          else if (c == ")") { depth--; if (depth == 0) break }
        }
        # 閉じの無い ]( は後続のリンクを取り込んで隠すため、判定不能として拒否される印（先頭 /）を出す
        if (depth != 0) { print "/UNCLOSED"; return }
        print substr(rest, 1, i - 1)
        line = substr(rest, i + 1)
      }
    }
    {
      # 宛先を消費した行も、続けて参照定義として解析する（宛先なしの定義の直後の定義を見逃さない）
      if (pending && $0 !~ /^[[:space:]]*$/) { print $0; pending = 0 }
      if ($0 ~ /^[[:space:]]*\[[^]]+\]:/) {
        t = $0
        sub(/^[[:space:]]*\[[^]]+\]:[[:space:]]*/, "", t)
        if (t == "") pending = 1; else print t
      }
      # インラインは改行をまたぐため、全行を空白でつないでから END で解析する
      buf = buf " " $0
    }
    END { inline(buf) }' "$1"
}

# links_leave_dir <file>: Markdown のリンク先（インライン・参照定義）を相対パスとして正規化し、
# 置き場の外へ出るもの（./../ の表記、先頭の / を含む）が 1 件でもあれば 0 を返す。
# 外部 URL（スキーム付き）と同一ファイル内の # だけは対象外。
# 戻り値: 0 = 外へ出るリンクあり / 1 = なし / 2 = 検査を完了できない（awk の実行失敗）。
# if の条件で呼ぶと関数内の set -e が効かないため、呼び出し側は `rc=0; links_leave_dir f || rc=$?` で確かめる。
links_leave_dir() {
  local file="$1" raw="${TMP_DIR}/links.raw" verdict
  links_extract "${file}" >"${raw}" || return 2
  verdict="$(links_verdict "${raw}")" || return 2
  case "${verdict}" in
    leave) return 0 ;;
    stay) return 1 ;;
    *) return 2 ;;
  esac
}

# links_verdict <targets-file>: リンク先を 1 行ずつ読み、leave または stay を出力する。
# URL の符号化を先に復号する（%2e は . に、%2f・%5c・\ は / に）。% の後が 16 進 2 桁でないもの、
# 二重の符号化になりうる %25 は判定できないため leave（拒否）とする。
links_verdict() {
  awk '
    function decode(t,   out, h, c) {
      out = ""
      while (length(t) > 0) {
        c = substr(t, 1, 1)
        if (c == "%") {
          h = tolower(substr(t, 2, 2))
          if (h !~ /^[0-9a-f][0-9a-f]$/ || h == "25") return "BAD"
          if (h == "2e") out = out "."
          else if (h == "2f" || h == "5c") out = out "/"
          else out = out substr(t, 1, 3)
          t = substr(t, 4)
        } else if (c == "\\") {
          # バックスラッシュ + 記号は Markdown のエスケープ（\. は . 等）で区切りと区別できないため拒否する
          if (substr(t, 2, 1) ~ /[^A-Za-z0-9]/) return "BAD"
          out = out "/"; t = substr(t, 2)
        }
        else { out = out c; t = substr(t, 2) }
      }
      return out
    }
    {
      t = $0; sub(/^[[:space:]]+/, "", t)
      if (substr(t, 1, 1) == "<") {
        # 山括弧のリンク先は空白を含みうるため、最初の > までの全体を取る（後ろはタイトル）。閉じが無ければ判定不能で拒否
        e = index(t, ">")
        if (e == 0) { bad = 1; next }
        t = substr(t, 2, e - 2)
      } else {
        t = $1
      }
      orig = t
      sub(/[#?].*$/, "", t)
      if (t == "" || t ~ /^[A-Za-z][A-Za-z0-9+.-]*:/) next
      # Markdown の文字参照（&#46; &#x2e; &period; 等）は復号せず、判定不能として拒否する（# 以降の切り落としをすり抜けるため）
      if (orig ~ /&[#A-Za-z0-9]+;/) { bad = 1; next }
      t = decode(t)
      if (t == "BAD" || t ~ /^\//) { bad = 1; next }
      n = split(t, seg, "/"); depth = 0
      for (i = 1; i <= n; i++) {
        if (seg[i] == "" || seg[i] == ".") continue
        if (seg[i] == "..") { depth--; if (depth < 0) { bad = 1; break } }
        else depth++
      }
    }
    END { print bad ? "leave" : "stay" }' "$1"
}

# html_links_present <file>: HTML のリンク・参照の属性（href=・src=・srcset=・poster=・action=・data=・xlink:href=）があれば 0 を返す。
# 宛先を持つタグ（<a>・<img> 等）は属性で必ず検出できる。属性の無い `<a b/c.md>` は Markdown の山括弧の宛先と区別できないため対象にしない。
# HTML は解析しないため、未対応の記法として拒否する（fail-closed）。大文字小文字は区別しない。
# 戻り値: 0 = あり / 1 = なし / 2 = 検査を完了できない（grep の実行失敗。1 以外の失敗を握りつぶさない）。
html_links_present() {
  local grc=0
  grep -Eiq -- '(href|src|srcset|poster|action|data|xlink:href)[[:space:]]*=' "$1" || grc=$?
  case "${grc}" in
    0) return 0 ;;
    1) return 1 ;;
    *) return 2 ;;
  esac
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
  local hrc=0
  html_links_present "${dir}/README.md" || hrc=$?
  case "${hrc}" in
    0)
      echo "error: ${label}/README.md uses an unsupported link notation (HTML link or reference); use Markdown links" >&2
      VIOLATIONS=$((VIOLATIONS + 1))
      ;;
    1) ;;
    *)
      echo "error: HTML link check failed to run for ${label}/README.md" >&2
      exit 2
      ;;
  esac
  local lrc=0
  links_leave_dir "${dir}/README.md" || lrc=$?
  case "${lrc}" in
    0)
      echo "error: ${label}/README.md has a link that leaves the directory" >&2
      VIOLATIONS=$((VIOLATIONS + 1))
      ;;
    1) ;;
    *)
      echo "error: link check failed to run for ${label}/README.md" >&2
      exit 2
      ;;
  esac
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
