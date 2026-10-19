#!/usr/bin/env bash
# fork の CI の構成（secret なしでビルドと試験が通る形）を静的に検査する
# （`make check-fork-ci` と ci.yml の check ジョブから呼ばれる。TASK-96・BUILD-22）。
#
# 理由: 第三者がエンジン repo を fork し、secret を設定しないまま CI を流してもビルドと試験が通る、
# という性質を、fork での実行（D-29 により行わない）ではなく構成の検査で確かめる。
# 検査は行ベース（awk・grep・jq）で、YAML の構文木は作らない。既存の書式（インデント 2・ブロック形式）に
# 依存するため、書式を変える場合は scripts/test-check-fork-ci.sh の見本も同時に直す。
# C1・D は許可リストで判定する: 正準の形を 1 つだけ認め、シェルの意味は解釈せず、正準の形から外れたものは拒否する。
#
# 検査の項目:
#   Y YAML の表記の字句の関門（検査する全 workflow: ci.yml・deploy.yml・--pack-workflow。A〜D より先に流す）
#     A〜D は裸の `key:` の行だけを読むため、key を字句で確定できない表記は解釈せず一律に拒否する
#     （解釈して検査を続けることはしない）。
#     Y1 引用符つきの key（`"uses": ...`・`'submodules': ...`。行の先頭と `- ` の後ろ。run の本文の行も含む）
#     Y2 複合キー（`? `）
#     Y3 フロー形式のマッピング・シーケンス（run の本文の外で、${{ }} を除いた行に `{` か `[` がある。`{` は
#        permissions の行だけを除く（C1 が値を判定する）。`[` は要素にマッピング（`[uses: x]`）を隠せるため、
#        許可リストの `branches: [<語>, ...]`（語は英数字と _ . / - だけ）の行だけを除く。step・job に限らず
#        with・concurrency・needs 等のフロー形式も拒否する）
#     Y4 key の前のタグ・アンカー（`!!str uses:`・`&a uses:`）と、key に置いた別名（`*a :`）
#     Y5 run の値が別名（`run: *a`。アンカーの本文は run の外にありうるため、A5 の run の検査が届かない）か、
#        run: の行に値が無い（値が次の行から始まると、その形を字句で確定できない）
#     Y6 引用符つきのスカラー: ダブルクォートの値に \ がある（YAML がエスケープを復号するため、字句で見た文字列と
#        実行時の文字列が食い違う）か、引用符が同じ行で閉じない（続きの行の範囲を字句で確定できない）。
#        プレーン・ブロックのスカラーの \ は復号されないので対象外（シェルの行の継続を使える）
#     Y7 if: の値が同じ行で完結しない（値が空・ブロックや折り返しのスカラー・if: より深い字下げの続きの行がある。
#        続きの行は条件に連結され、C2 の比較をすり抜ける）
#   A private な依存が 0 件
#     A1 Cargo.lock の source が crates.io（と許可リストの公開 git）だけ
#     A2 package-lock.json の resolved が registry.npmjs.org だけ
#     A3 .npmrc に registry・認証の行が無い
#     A4 .cargo/config(.toml) に registry・認証・git-fetch-with-cli が無い
#     A5 .gitmodules の各 submodule が `update = none` を持ち、ci.yml の checkout が submodule を取得しない。
#        明示の取得は update = none を上書きし、git はオプション・設定キー（clone.recurseSubmodules 等）・
#        環境変数（GIT_CONFIG_*）のどれからでも submodule を取りうるため、意味は解釈せず一律に拒否する:
#        ci.yml の run の本文（ブロック・複数行のスカラーの続きの行を含む）に、語としての git と、大文字小文字を
#        問わず submodule・recurse・git_ の部分文字列が無い。ci.yml の key に GIT_ で始まるもの（env のどの階層でも）
#        が無く、env: の値が同じ行に無い（式で書いた env は key を字句で確定できない）
#   B ビルド・試験のジョブ（ci.yml）からの secret の参照が 0 件
#     B1 scripts/check-workflow-secrets.sh を呼ぶ（実体は二重に書かない。ruby を要する）
#     B2 BUILD-2 が挙げる語（GITHUB_TOKEN・CARGO_REGISTR*・git-fetch-with-cli・語としての _TOKEN / _PAT）が無い
#     B3 uses: が 40 桁の SHA 固定で、所有者が許可リストにあり、reusable workflow を呼ばない
#   C 配信の切り離し
#     C1 ci.yml がトップレベルの permissions を持ち（省略は既定の権限の継承になるため不可）、permissions の値が
#        {}・リテラルの read / none だけ（write・式 ${{ }} は不可）で、environment が無い
#     C2 deploy.yml が workflow_dispatch だけで動き、permissions が {} で、全ジョブが fork では飛ばされる
#     C3 ci.yml に deploy ジョブが無い
#   D ENGINE_REV での教材 repo のビルド（--pack-workflow と --engine-rev の両方を渡したときだけ）
#     版の固定は正準の形を 1 つだけ認める。シェルの意味は解釈せず、正準の形から外れたものは拒否する（許可リスト）。
#     エンジンの repo か ENGINE_REV を参照するジョブ（以下、エンジンのジョブ）が対象。
#     D1 ENGINE_REV が 40 桁の小文字の 16 進 1 行
#     D2 正準の 3 step を持つエンジンのジョブが 1 つ以上ある
#     D3 エンジンのジョブが次の 3 step で始まる。ジョブの key は許可リストのものだけ（env・defaults 等は不可）で、
#        workflow に env・defaults が無い（いずれも正準の step の動きを変えうるため）
#          1. この repo の checkout: uses: actions/checkout@<40 桁の SHA>、with は persist-credentials: false だけ
#          2. 出力: id と run だけ。run は前後の空白を除いてちょうど 1 行の
#             echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"（行の文字列の完全一致）
#          3. エンジンの checkout: uses: actions/checkout@<40 桁の SHA>、with は repository: Fandhe-AI/fandhe-workshop・
#             ref: ${{ steps.<2 の id>.outputs.rev }}・path: <固定のディレクトリ名>・persist-credentials: false だけ
#        git clone・cargo install 等で取得する経路は正準に含めない（取得の正しさがシェルの解釈を要するため）
#        repository を式（${{ }}）で決める step を持つジョブも、エンジンを取得しうるものとしてエンジンのジョブに含める
#     D4 4 つ目以降の step が、actions/checkout を使わず（checkout は保存先を clean し、path を省けば作業領域の
#        ルートごとエンジンを消すため。保存先は解釈しない）、語としての git・エンジンの repo・ENGINE_REV・3 の path の
#        文字列を含まず（name の行は除く）、with の repository・ref・path に ${{ を使わない
#
# deploy.yml の起点を変える場合（TASK-121）は、C2 と回帰テストを同時に直す。
#
# 使い方: scripts/check-fork-ci.sh [--ci <file>] [--deploy <file>] [--cargo-lock <file>]
#           [--package-lock <file>] [--npmrc <file>] [--gitmodules <file>] [--cargo-config-dir <dir>]
#           [--secrets-checker <script>] [--pack-workflow <file> --engine-rev <file>]
#   各オプションは回帰テストが一時ファイルを検査するために使う。--secrets-checker は B1 の実体の差し替え。
# 終了コード: 0 = 合格 / 1 = 違反あり / 2 = 誤用、または検査を完了できない（ファイルが無い・道具が無い・B1 が 2）。
#   2 でも合格にしない（fail-closed）。回帰テストは scripts/test-check-fork-ci.sh。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

CI_FILE="${ROOT_DIR}/.github/workflows/ci.yml"
DEPLOY_FILE="${ROOT_DIR}/.github/workflows/deploy.yml"
CARGO_LOCK="${ROOT_DIR}/Cargo.lock"
PACKAGE_LOCK="${ROOT_DIR}/package-lock.json"
NPMRC="${ROOT_DIR}/.npmrc"
GITMODULES="${ROOT_DIR}/.gitmodules"
CARGO_CONFIG_DIR="${ROOT_DIR}/.cargo"
SECRETS_CHECKER="${DIR}/check-workflow-secrets.sh"
PACK_WORKFLOW=""
ENGINE_REV_FILE=""

# 配信を許す repo。deploy.yml のジョブの if: がこの比較を含むことを C2 で検査する
GUARD_REPO="Fandhe-AI/fandhe-workshop"
# ci.yml の uses: に許す所有者（初期は actions のみ）
ALLOWED_USES_OWNERS="actions"
# Cargo.lock に許す公開 git 依存の repo（`owner/repo`。空白区切り。初期は 0 件）
ALLOWED_GIT_REPOS=""

usage() {
  echo "usage: $(basename "$0") [--ci <file>] [--deploy <file>] [--cargo-lock <file>] [--package-lock <file>]" >&2
  echo "       [--npmrc <file>] [--gitmodules <file>] [--cargo-config-dir <dir>] [--secrets-checker <script>]" >&2
  echo "       [--pack-workflow <file> --engine-rev <file>]" >&2
  exit 2
}

while [ "$#" -gt 0 ]; do
  [ "$#" -ge 2 ] || usage
  case "$1" in
    --ci) CI_FILE="$2" ;;
    --deploy) DEPLOY_FILE="$2" ;;
    --cargo-lock) CARGO_LOCK="$2" ;;
    --package-lock) PACKAGE_LOCK="$2" ;;
    --npmrc) NPMRC="$2" ;;
    --gitmodules) GITMODULES="$2" ;;
    --cargo-config-dir) CARGO_CONFIG_DIR="$2" ;;
    --secrets-checker) SECRETS_CHECKER="$2" ;;
    --pack-workflow) PACK_WORKFLOW="$2" ;;
    --engine-rev) ENGINE_REV_FILE="$2" ;;
    *) usage ;;
  esac
  shift 2
done
if [ -n "${PACK_WORKFLOW}" ] && [ -z "${ENGINE_REV_FILE}" ]; then usage; fi
if [ -z "${PACK_WORKFLOW}" ] && [ -n "${ENGINE_REV_FILE}" ]; then usage; fi

require_cmd awk || exit 2
require_cmd grep || exit 2
require_cmd jq || exit 2

# need_file <path>: 読めない・通常ファイルでないなら検査を完了できないものとして 2 で止める。
need_file() {
  if [ ! -r "$1" ] || [ ! -f "$1" ]; then
    echo "error: cannot read $1" >&2
    exit 2
  fi
}

violations=0

# fail <message>: 違反を 1 件記録する。値（秘密になりうる行の全文）は出さず、場所と理由だけを出す。
fail() {
  echo "error: $1" >&2
  violations=$((violations + 1))
}

# report <file> <lines>: `行:理由` の出力を 1 行ずつ違反として記録する。
report() {
  local file="$1" out="$2" line
  [ -n "${out}" ] || return 0
  while IFS= read -r line; do
    fail "${file}:${line}"
  done <<EOT
${out}
EOT
}

# grep_hits <description> <file> <extended-regex> [grep-flags...]: 一致した行番号だけを `行:説明` で返す。
# grep の終了コード 1（一致なし）は正常、2 以上は検査を完了できないものとして 2 で止める。
grep_hits() {
  local desc="$1" file="$2" re="$3" rc=0 out
  shift 3
  out="$(grep -n "$@" -E -e "${re}" -- "${file}")" || rc=$?
  if [ "${rc}" -gt 1 ]; then
    echo "error: grep failed on ${file}" >&2
    exit 2
  fi
  [ -n "${out}" ] || return 0
  # 説明に / や & が含まれても壊れないよう、sed ではなく awk に値として渡す
  printf '%s\n' "${out}" | awk -F: -v d="${desc}" '{ print $1 ":" d }'
}

# grep_report <description> <file> <extended-regex> [grep-flags...]: grep_hits の結果を違反として記録する。
# $(...) の中の exit は親に届かず空文字列（合格）になるため、終了状態を受けて 2 以上なら親側で 2 で止める（fail-closed）。
grep_report() {
  local hits rc=0
  hits="$(grep_hits "$@")" || rc=$?
  if [ "${rc}" -ne 0 ]; then
    echo "error: check could not complete (grep_hits exit ${rc})" >&2
    exit 2
  fi
  report "$2" "${hits}"
}

# ---- Y YAML の表記の字句の関門（と A5 の run の本文） ----
# yaml_gate <file> <0|1>: Y1〜Y7 を検査して違反を記録する。第 2 引数が 1 なら、ci.yml の git・submodule の経路（A5）も検査する。
# run の本文は「run: の行の値と、その key の列より深く字下げした行・空行の続き」とみなす（スカラーの形は解釈しない）。
yaml_gate() {
  local file="$1" sub_check="$2" out rc=0
  out="$(awk -v sub_check="${sub_check}" '
    BEGIN { if_col = -1 }
    function indent(s) { match(s, /^ */); return RLENGTH }
    # strip_props <s>: 値の前のタグ・アンカー（`!!str `・`&a `）を除く
    function strip_props(s) { while (match(s, /^[!&][^[:space:]]*([[:space:]]+|$)/)) s = substr(s, RLENGTH + 1); return s }
    # value_of <line>: 字下げ・`- `・裸の `key:` を除いた値の位置の文字列（タグ・アンカーも除く）
    function value_of(s) {
      sub(/^[[:space:]]*(-[[:space:]]+)*/, "", s)
      sub(/^[A-Za-z0-9_.-]+[[:space:]]*:([[:space:]]+|$)/, "", s)
      return strip_props(s)
    }
    # quoted_check <value> <line>: 値が引用符で始まるなら Y6 を検査する。ダブルクォートは \ を含めば拒否し
    # （YAML がバックスラッシュの後ろの u0075 等を復号するため、字句の検査と実行時の文字列が食い違う）、引用符が同じ行で閉じなければ拒否する
    # （複数行の引用符つきスカラーは、続きの行の字下げが浅くても続くことがあり、本文の範囲を字句で確定できない）
    function quoted_check(v, n,   r) {
      if (substr(v, 1, 1) == "\"") {
        r = substr(v, 2)
        if (index(r, "\\")) print n ":Y6 backslash in a double-quoted YAML scalar (escapes are decoded by YAML; use a plain or block scalar)"
        else if (!index(r, "\"")) print n ":Y6 double-quoted YAML scalar does not close on the same line"
      } else if (substr(v, 1, 1) == "\047") {
        r = substr(v, 2); gsub(/\047\047/, "", r)
        if (!index(r, "\047")) print n ":Y6 single-quoted YAML scalar does not close on the same line"
      }
    }
    # scan_run <text> <line>: run の本文の 1 行を A5 の語で調べる（ci.yml だけ）。git の取得はオプション・設定キー・
    # 環境変数で submodule を取りうるため、意味は解釈せず、語としての git と submodule・recurse・git_ の部分文字列を一律に拒否する
    function scan_run(s, n,   l) {
      if (!sub_check) return
      l = tolower(s)
      if (l ~ /(^|[^a-z0-9_])git([^a-z0-9_]|$)/) print n ":A5 run mentions git (git can fetch submodules through options, config keys and environment; the build/test workflow must not run git)"
      if (index(l, "submodule") || index(l, "recurse")) print n ":A5 run mentions submodule or recurse (an explicit submodule fetch overrides update = none; private docs/spec must not be fetched)"
      if (index(l, "git_")) print n ":A5 run mentions GIT_* (git reads config from GIT_CONFIG_* and similar variables, also via GITHUB_ENV)"
    }
    {
      raw = $0
      # Y7: if: の次の空でない行が if: の key より深ければ、値の続き（条件の連結）とみなして拒否する
      if (if_col >= 0 && raw !~ /^[[:space:]]*$/) {
        if (indent(raw) > if_col) print if_line ":Y7 if: continues on the next line (write the whole condition on the if: line)"
        if_col = -1
      }
      if (raw ~ /^[[:space:]]*(-[[:space:]]+)?("([^"\\]|\\.)*"|\047([^\047]|\047\047)*\047)[[:space:]]*:/) print NR ":Y1 quoted YAML key (keys must be plain words; quoted keys are not interpreted)"
      if (raw ~ /^[[:space:]]*(-[[:space:]]+)?\?([[:space:]]|$)/) print NR ":Y2 complex YAML key (? ) cannot be read lexically"
      if (raw ~ /^[[:space:]]*(-[[:space:]]+)?([!&][^[:space:]]*[[:space:]]+)+[^[:space:]:]+[[:space:]]*:([[:space:]]|$)/ \
        || raw ~ /^[[:space:]]*(-[[:space:]]+)?\*[^[:space:]]+[[:space:]]+:/) print NR ":Y4 tag, anchor or alias on a YAML key cannot be read lexically"
      if (in_run) {
        if (raw ~ /^[[:space:]]*$/ || indent(raw) > run_col) { scan_run(raw, NR); next }
        in_run = 0
      }
      if (raw ~ /^[[:space:]]*(-[[:space:]]+)?run[[:space:]]*:/) {
        t = raw; sub(/^[[:space:]]*(-[[:space:]]+)?/, "", t); run_col = length(raw) - length(t)
        v = t; sub(/^run[[:space:]]*:[[:space:]]*/, "", v)
        if (v ~ /^\*/) print NR ":Y5 run value is an alias (the anchored text can live outside any run and is not checked)"
        w = strip_props(v); sub(/^#.*$/, "", w); sub(/[[:space:]]+#.*$/, "", w)
        if (w ~ /^[[:space:]]*$/) print NR ":Y5 run value does not start on the run: line (write run: | or the command on the same line)"
        quoted_check(w, NR)
        in_run = 1; scan_run(v, NR)
        next
      }
      if (raw ~ /^[[:space:]]*#/) next
      quoted_check(value_of(raw), NR)
      t = raw; gsub(/\$\{\{[^}]*\}\}/, "", t)
      if (index(t, "{") && t !~ /^[[:space:]]*(-[[:space:]]+)?permissions[[:space:]]*:/) print NR ":Y3 flow-style mapping outside permissions (write steps, jobs and inputs as block mappings)"
      # フロー形式のシーケンスは、要素にマッピング（`[uses: x]`）を隠せる。許可するのは on: の branches の
      # 単純な語の列だけ（キー名と要素の文字を許可リストで限定し、要素に `:`・`{`・引用符・別名を置かせない）
      if (index(t, "[") && t !~ /^[[:space:]]*branches[[:space:]]*:[[:space:]]*\[[[:space:]]*[A-Za-z0-9_.\/-]+([[:space:]]*,[[:space:]]*[A-Za-z0-9_.\/-]+)*[[:space:]]*\][[:space:]]*$/) print NR ":Y3 flow-style sequence (write sequences in block style; only branches: [<names>] is allowed)"
      if (raw ~ /^[[:space:]]*(-[[:space:]]+)?if[[:space:]]*:/) {
        t = raw; sub(/^[[:space:]]*(-[[:space:]]+)?/, "", t); if_col = length(raw) - length(t); if_line = NR
        v = t; sub(/^if[[:space:]]*:[[:space:]]*/, "", v); sub(/^#.*$/, "", v); sub(/[[:space:]]+#.*$/, "", v)
        if (v ~ /^[[:space:]]*$/) print NR ":Y7 if: value is empty or does not start on the if: line"
        else if (v ~ /^[|>]/) print NR ":Y7 if: value is a block or folded scalar (write the whole condition on the if: line)"
      }
      if (sub_check) {
        # git の設定を環境変数で渡す経路（GIT_CONFIG_COUNT・GIT_CONFIG_KEY_0 等）。env の階層は問わない
        if (tolower(raw) ~ /^[[:space:]]*(-[[:space:]]+)*git_[^[:space:]:]*[[:space:]]*:/) print NR ":A5 GIT_* environment variable key (git reads config such as submodule settings from GIT_CONFIG_*)"
        # env を式などで同じ行に書くと、key を字句で確定できない
        if (raw ~ /^[[:space:]]*(-[[:space:]]+)?env[[:space:]]*:/) {
          v = raw; sub(/^[^:]*:[[:space:]]*/, "", v); sub(/^#.*$/, "", v); sub(/[[:space:]]+#.*$/, "", v)
          if (v !~ /^[[:space:]]*$/) print NR ":A5 env: value on the same line (write env as a block mapping so that GIT_* keys can be checked)"
        }
      }
    }
  ' "${file}")" || rc=$?
  [ "${rc}" -eq 0 ] || { echo "error: awk failed on ${file}" >&2; exit 2; }
  report "${file}" "${out}"
}

need_file "${CI_FILE}"
need_file "${DEPLOY_FILE}"
yaml_gate "${CI_FILE}" 1
yaml_gate "${DEPLOY_FILE}" 0
if [ -n "${PACK_WORKFLOW}" ]; then
  need_file "${PACK_WORKFLOW}"
  yaml_gate "${PACK_WORKFLOW}" 0
fi

# ---- A1 Cargo.lock ----
need_file "${CARGO_LOCK}"
a1_out=""
# grep の終了コード 1（source 行なし）は正常、2 以上（読み取り失敗）は検査未完了として 2 で止める（fail-closed）
a1_rc=0
a1_src="$(grep -n '^source = ' -- "${CARGO_LOCK}")" || a1_rc=$?
if [ "${a1_rc}" -gt 1 ]; then
  echo "error: grep failed on ${CARGO_LOCK}" >&2
  exit 2
fi
while IFS= read -r entry; do
  [ -n "${entry}" ] || continue
  lineno="${entry%%:*}"
  value="${entry#*:}"
  value="${value#source = \"}"
  value="${value%\"}"
  ok=0
  case "${value}" in
    "registry+https://github.com/rust-lang/crates.io-index" | "sparse+https://index.crates.io/") ok=1 ;;
    git+https://github.com/*)
      repo="${value#git+https://github.com/}"
      repo="${repo%%[?#.]*}"
      case "${repo}" in
        */*)
          for allowed in ${ALLOWED_GIT_REPOS}; do
            [ "${repo}" = "${allowed}" ] && ok=1
          done
          ;;
      esac
      ;;
  esac
  # 認証情報入りの URL は、許可リストに載っていても通さない
  case "${value}" in *@*) ok=0 ;; esac
  if [ "${ok}" -ne 1 ]; then
    a1_out="${a1_out:+${a1_out}
}${lineno}:A1 dependency source is not crates.io or an allowed public git repo"
  fi
done <<EOT
${a1_src}
EOT
report "${CARGO_LOCK}" "${a1_out}"

# ---- A2 package-lock.json ----
need_file "${PACKAGE_LOCK}"
rc=0
a2_out="$(jq -r '[paths(type == "object" and (.resolved? | type) == "string" and (.resolved | startswith("https://registry.npmjs.org/") | not))] | .[] | "0:A2 npm dependency resolved outside registry.npmjs.org at ." + (map(tostring) | join("."))' "${PACKAGE_LOCK}")" || rc=$?
if [ "${rc}" -ne 0 ]; then
  echo "error: cannot parse ${PACKAGE_LOCK} as JSON" >&2
  exit 2
fi
# jq の出力に行番号は無い。resolved の値は認証情報入り URL でありうるため出さず、JSON 内の位置だけを出す
report "${PACKAGE_LOCK}" "${a2_out}"

# ---- A3 .npmrc（無ければ合格） ----
if [ -e "${NPMRC}" ]; then
  need_file "${NPMRC}"
  grep_report "A3 .npmrc sets a registry or credentials" "${NPMRC}" '^[[:space:]]*(@[^=]*:)?registry[[:space:]]*=|_authtoken|_auth[[:space:]]*=|_password|^[[:space:]]*//' -i
fi

# ---- A4 .cargo/config(.toml)（無ければ合格） ----
for cfg in "${CARGO_CONFIG_DIR}/config" "${CARGO_CONFIG_DIR}/config.toml"; do
  if [ -e "${cfg}" ]; then
    need_file "${cfg}"
    grep_report "A4 cargo config sets a registry, source replacement or credentials" "${cfg}" '\[registries|\[source|replace-with|git-fetch-with-cli|credential-provider|token' -i
  fi
done

# ---- A5 submodule ----
need_file "${GITMODULES}"
rc=0
a5_out="$(awk '
  function close_section() {
    if (name != "" && !has_none) print start ":A5 submodule " name " lacks update = none"
    name = ""
  }
  /^\[submodule / { close_section(); name = $2; start = NR; has_none = 0; next }
  /^[[:space:]]*update[[:space:]]*=[[:space:]]*none[[:space:]]*$/ { has_none = 1 }
  END { close_section() }
' "${GITMODULES}")" || rc=$?
[ "${rc}" -eq 0 ] || { echo "error: awk failed on ${GITMODULES}" >&2; exit 2; }
report "${GITMODULES}" "${a5_out}"

need_file "${CI_FILE}"
rc=0
a5_ci_out="$(awk '
  /^[[:space:]]*submodules[[:space:]]*:/ {
    v = $0; sub(/^[^:]*:[[:space:]]*/, "", v); sub(/[[:space:]]*#.*$/, "", v); gsub(/["\047]/, "", v)
    if (v != "false") print NR ":A5 checkout fetches submodules (private docs/spec must not be fetched)"
  }
' "${CI_FILE}")" || rc=$?
[ "${rc}" -eq 0 ] || { echo "error: awk failed on ${CI_FILE}" >&2; exit 2; }
report "${CI_FILE}" "${a5_ci_out}"

# ---- B1 check-workflow-secrets.sh ----
b1_rc=0
"${SECRETS_CHECKER}" --file "${CI_FILE}" >/dev/null || b1_rc=$?
if [ "${b1_rc}" -eq 1 ]; then
  fail "${CI_FILE}:0:B1 check-workflow-secrets reported secret references (run it directly for details)"
elif [ "${b1_rc}" -ne 0 ]; then
  echo "error: B1 secrets checker could not complete (exit ${b1_rc}); treating as failure" >&2
  exit 2
fi

# ---- B2 BUILD-2 が挙げる語 ----
# コメントも除外しない（run のブロックスカラーとコメントを行頭の # だけでは区別できないため。fail-closed）。
# 語の境界で判定し、GITHUB_PATH のような別の語は通す。
grep_report "B2 forbidden credential word (GITHUB_TOKEN, CARGO_REGISTR*, git-fetch-with-cli, *_TOKEN, *_PAT)" "${CI_FILE}" 'GITHUB_TOKEN|CARGO_REGISTR|git-fetch-with-cli|_(TOKEN|PAT)([^A-Za-z0-9_]|$)' -i

# ---- B3 uses: ----
rc=0
b3_out="$(awk -v owners="${ALLOWED_USES_OWNERS}" '
  BEGIN { n = split(owners, o, " ") }
  /^[[:space:]]*(-[[:space:]]+)?uses[[:space:]]*:/ {
    v = $0
    sub(/^[^:]*:[[:space:]]*/, "", v)
    sub(/[[:space:]]*#.*$/, "", v)
    gsub(/["\047]/, "", v)
    ok = 0
    if (v ~ /^[A-Za-z0-9._-]+\/[A-Za-z0-9._\/-]+@[0-9a-f]{40}$/) {
      owner = v; sub(/\/.*$/, "", owner)
      for (i = 1; i <= n; i++) if (tolower(owner) == tolower(o[i])) ok = 1
      if (v ~ /\.github\/workflows\//) ok = 0
    }
    if (!ok) print NR ":B3 uses: must be an allowed owner pinned by 40-hex SHA and not a reusable workflow"
  }
' "${CI_FILE}")" || rc=$?
[ "${rc}" -eq 0 ] || { echo "error: awk failed on ${CI_FILE}" >&2; exit 2; }
report "${CI_FILE}" "${b3_out}"

# ---- C1 ci.yml にトップレベルの permissions があり、write の権限・environment が無い ----
# permissions の値は許可リストで判定する: `{}`、ブロック形式の `<scope>: read|none`、フロー形式の `{<scope>: read|none, ...}` だけ。
# 式（${{ ... }}）は値を確定できないため、permissions のブロックのどこにあっても拒否する。値の引用符は外してから比べる。
rc=0
c1_out="$(awk '
  function indent(s) { match(s, /^ */); return RLENGTH }
  function bare(s) { sub(/[[:space:]]+#.*$/, "", s); sub(/^[[:space:]]+/, "", s); sub(/[[:space:]]+$/, "", s); gsub(/["\047]/, "", s); return s }
  function perm_ok(v) { return v == "read" || v == "none" }
  function bad(n) { print n ":C1 permissions must be {} or literal <scope>: read|none (no write, write-all or ${{ }} expressions; the build/test workflow must be read-only)" }
  # flow_ok <value>: permissions: の同じ行の値が {} か、全要素が read・none のフロー形式か
  function flow_ok(v,   inner, n, a, i, kv) {
    if (v == "{}") return 1
    if (v !~ /^\{.*\}$/) return 0
    inner = substr(v, 2, length(v) - 2); n = split(inner, a, ",")
    for (i = 1; i <= n; i++) {
      kv = a[i]
      if (kv !~ /^[[:space:]]*[A-Za-z-]+[[:space:]]*:/) return 0
      sub(/^[^:]*:/, "", kv)
      if (!perm_ok(bare(kv))) return 0
    }
    return 1
  }
  in_perm && ($0 ~ /^[[:space:]]*$/ || $0 ~ /^[[:space:]]*#/) { next }
  in_perm && indent($0) > perm_ind {
    if ($0 ~ /\$\{\{/ || $0 !~ /^[[:space:]]*[A-Za-z-]+[[:space:]]*:/) { bad(NR); next }
    v = $0; sub(/^[^:]*:/, "", v)
    if (!perm_ok(bare(v))) bad(NR)
    next
  }
  { in_perm = 0 }
  /^[[:space:]]*(-[[:space:]]+)?["\047]?permissions["\047]?[[:space:]]*:/ {
    if ($0 ~ /^["\047]?permissions["\047]?[[:space:]]*:/) top_perm = 1
    v = $0; sub(/^[^:]*:/, "", v)
    if (v ~ /\$\{\{/) { bad(NR); next }
    v = bare(v)
    if (v == "") { in_perm = 1; perm_ind = indent($0); next }
    if (!flow_ok(v)) bad(NR)
  }
  # 省略するとリポジトリ・組織の既定の権限（書き込みでありうる）を継承するため、トップレベルの宣言を必須にする
  END { if (!top_perm) print "0:C1 top-level permissions: not found (omitting it inherits the repository or organization default, which may be write; declare {} or read-only scopes)" }
' "${CI_FILE}")" || rc=$?
[ "${rc}" -eq 0 ] || { echo "error: awk failed on ${CI_FILE}" >&2; exit 2; }
report "${CI_FILE}" "${c1_out}"
grep_report "C1 environment (deployment settings belong in deploy.yml)" "${CI_FILE}" '^[[:space:]]*(-[[:space:]]+)?environment[[:space:]]*:' -i

# ---- C2 deploy.yml ----
need_file "${DEPLOY_FILE}"
rc=0
c2_out="$(awk -v repo="${GUARD_REPO}" '
  function indent(s) { match(s, /^ */); return RLENGTH }
  function close_job() {
    if (job != "" && !job_guard) print job_line ":C2 job " job " has no fork guard (if: github.repository == \047" repo "\047)"
    job = ""
  }
  /^[[:space:]]*($|#)/ { next }
  indent($0) == 0 {
    section = ""
    if ($0 ~ /^on[[:space:]]*:/) {
      section = "on"; seen_on = 1
      rest = $0; sub(/^on[[:space:]]*:[[:space:]]*/, "", rest); sub(/[[:space:]]*#.*$/, "", rest)
      if (rest != "") print NR ":C2 on: must be a block whose only key is workflow_dispatch"
    } else if ($0 ~ /^permissions[[:space:]]*:/) {
      seen_perm = 1
      if ($0 !~ /^permissions:[[:space:]]*\{\}[[:space:]]*(#.*)?$/) print NR ":C2 top-level permissions must be {}"
    } else if ($0 ~ /^jobs[[:space:]]*:/) {
      section = "jobs"; seen_jobs = 1
    }
    next
  }
  section == "on" && indent($0) == 2 {
    key = $0; sub(/^ +/, "", key); sub(/[[:space:]]*:.*$/, "", key)
    if (key != "workflow_dispatch") print NR ":C2 on: has trigger " key " (only workflow_dispatch is allowed)"
    else seen_wd = 1
    next
  }
  section == "jobs" && indent($0) == 2 {
    close_job()
    job = $0; sub(/^ +/, "", job); sub(/[[:space:]]*:.*$/, "", job); job_line = NR; job_guard = 0; njobs++
    next
  }
  section == "jobs" && indent($0) == 4 && $0 ~ /^    if[[:space:]]*:/ {
    # 条件全体が許可した比較式と一致するときだけ合格にする（`|| true` などの付加を通さない）。
    # if: が 1 行で完結すること（続きの行・ブロックのスカラーが無いこと）は関門 Y7 が保証する
    cond = $0; sub(/^    if[[:space:]]*:[[:space:]]*/, "", cond); sub(/[[:space:]]+#.*$/, "", cond)
    sub(/^\$\{\{[[:space:]]*/, "", cond); sub(/[[:space:]]*\}\}$/, "", cond); gsub(/[[:space:]]+$/, "", cond)
    if (cond == "github.repository == \047" repo "\047") job_guard = 1
    next
  }
  END {
    close_job()
    if (!seen_on) print "0:C2 on: not found"
    if (seen_on && !seen_wd) print "0:C2 on: does not contain workflow_dispatch"
    if (!seen_perm) print "0:C2 top-level permissions: not found"
    if (!seen_jobs || njobs == 0) print "0:C2 no jobs found"
  }
' "${DEPLOY_FILE}")" || rc=$?
[ "${rc}" -eq 0 ] || { echo "error: awk failed on ${DEPLOY_FILE}" >&2; exit 2; }
report "${DEPLOY_FILE}" "${c2_out}"

# ---- C3 ci.yml に deploy ジョブが無い ----
grep_report "C3 deploy job in the build/test workflow (it belongs in deploy.yml)" "${CI_FILE}" '^  deploy[[:space:]]*:'

# ---- D ENGINE_REV ----
# 正準の出力 step の run（文字列の完全一致で比べる。拒否の理由にも見本として出す）
# shellcheck disable=SC2016 # 正準の行の文字列なので、シェルに展開させない
D_CANON_RUN='echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"'
if [ -n "${PACK_WORKFLOW}" ]; then
  need_file "${PACK_WORKFLOW}"
  need_file "${ENGINE_REV_FILE}"
  # D1: 40 桁の小文字の 16 進が 1 行だけ（末尾の改行 1 つは許す。空白・余分な行は不可）
  rev_lines="$(awk 'END { print NR }' "${ENGINE_REV_FILE}")"
  rev="$(head -n 1 "${ENGINE_REV_FILE}")"
  if [ "${rev_lines}" -ne 1 ] || ! printf '%s' "${rev}" | grep -Eq '^[0-9a-f]{40}$'; then
    fail "${ENGINE_REV_FILE}:1:D1 ENGINE_REV must be a single line of 40 lowercase hex digits"
  fi
  # D2〜D4: 行ベースで step を読み、エンジンのジョブを正準の形と字句で比べる。シェルの意味は解釈しない。
  # step の情報は通し番号（gsn）で配列に持ち、ジョブの終わり（end_job）でまとめて判定する。
  rc=0
  d_out="$(awk -v guard="${GUARD_REPO}" -v canon="${D_CANON_RUN}" '
    BEGIN {
      JOB_KEYS = "name runs-on permissions timeout-minutes needs if steps strategy concurrency continue-on-error outputs"
      n = split(JOB_KEYS, a, " "); for (i = 1; i <= n; i++) job_ok[a[i]] = 1
      OWN = "uses: actions/checkout@<40-hex sha> with only persist-credentials: false (no repository, ref or path)"
      OUT = "only id: <id> and run: " canon " (exactly this one line; no if, env, shell or working-directory)"
      ENG = "uses: actions/checkout@<40-hex sha> with only repository: " guard ", ref: ${{ steps.<id of step 2>.outputs.rev }}, path: <directory name>, persist-credentials: false"
    }
    function indent(s) { match(s, /^ */); return RLENGTH }
    function trim(s) { sub(/^[[:space:]]+/, "", s); sub(/[[:space:]]+$/, "", s); return s }
    # key_of <s>: 字下げと "- " を除いた行が `key:` の形ならその key、そうでなければ ""（引用した key・<< 等は ""）
    function key_of(s) {
      if (!match(s, /^[A-Za-z0-9_-]+[[:space:]]*:([[:space:]]|$)/)) return ""
      s = substr(s, 1, RLENGTH); sub(/[[:space:]]*:[[:space:]]*$/, "", s); return s
    }
    # raw_of <s>: `key:` の後ろの値（前後の空白だけを除く。コメントも残す）
    function raw_of(s) { sub(/^[A-Za-z0-9_-]+[[:space:]]*:/, "", s); return trim(s) }
    # val_of <s>: raw_of から行末のコメント（空白の後ろの # 以降）を除いた値。引用符は外さない（正準の形は引用しない）
    function val_of(s) { s = raw_of(s); if (s ~ /^#/) return ""; sub(/[[:space:]]+#.*$/, "", s); return s }
    # engine_ref <line>: 行がエンジンの repo か ENGINE_REV を参照するか。repository を式で決める行は、エンジンでありうるものとして扱う
    function engine_ref(s) {
      if (s ~ /^[[:space:]]*(-[[:space:]]+)?repository[[:space:]]*:.*\$\{\{/) return 1
      s = tolower(s); return index(s, "fandhe-ai/fandhe-workshop") || index(s, "engine_rev")
    }
    function start_job(name) {
      end_job()
      job = (name == "" ? "?" : name); job_line = NR; jfirst = gsn + 1
      job_eng = 0; job_badline = 0; job_badkey = ""; jk = -1; in_steps = 0; cur_step = 0; cur = ""
    }
    # key_line <body> <raw>: step の直下の `key: value` の行を記録する
    function key_line(body, raw,   k, v) {
      k = key_of(body); wind = -1; wcur = ""
      if (k == "") { st_bad[s] = 1; cur = ""; st_text[s] = st_text[s] "\n" raw; return }
      if (index(st_keys[s], " " k " ")) st_dup[s] = 1
      st_keys[s] = (st_keys[s] == "" ? " " : st_keys[s]) k " "
      cur = k
      if (k != "name") st_text[s] = st_text[s] "\n" raw
      v = raw_of(body)
      # 別名・アンカー・タグは値を字句で確定できない
      if (v ~ /^[*&!]/) st_bad[s] = 1
      if (k == "uses") st_uses[s] = val_of(body)
      else if (k == "id") st_id[s] = val_of(body)
      else if (k == "run") {
        if (v ~ /^\|-?$/) { st_style[s] = "block"; st_run[s] = "" }
        else if (v == "" || v ~ /^[|>"\047{\[]/) st_style[s] = "other"
        else { st_style[s] = "plain"; st_run[s] = v }
      }
      else if (k == "with" && v != "" && v !~ /^#/) st_wbad[s] = 1
    }
    # with_line <body>: with の直下の `key: value` の行を記録する
    function with_line(body,   k) {
      k = key_of(body); wcur = k
      if (k == "") { st_wbad[s] = 1; return }
      if (index(st_wkeys[s], " " k " ")) st_wbad[s] = 1
      st_wkeys[s] = (st_wkeys[s] == "" ? " " : st_wkeys[s]) k " "
      if (raw_of(body) ~ /^[*&!]/) st_wbad[s] = 1
      if (k == "repository") st_repo[s] = val_of(body)
      else if (k == "ref") st_ref[s] = val_of(body)
      else if (k == "path") st_path[s] = val_of(body)
      else if (k == "persist-credentials") st_pc[s] = val_of(body)
    }
    # keys_ok <have> <required> <optional>: " a b " の形の key の集合が、required を全て持ち、required と optional 以外を持たないか
    function keys_ok(have, req, opt,   n, a, i) {
      n = split(req, a, " "); for (i = 1; i <= n; i++) if (!index(have, " " a[i] " ")) return 0
      n = split(trim(have), a, " "); for (i = 1; i <= n; i++) if (!index(" " req " " opt " ", " " a[i] " ")) return 0
      return 1
    }
    function plain_step(i) { return !st_bad[i] && !st_dup[i] && !st_wbad[i] }
    function pinned_uses(i) { return st_uses[i] ~ /^actions\/checkout@[0-9a-f]{40}$/ }
    # is_own <i>: この repo（教材 repo）の正準の checkout か。path・ref があると ENGINE_REV を読む場所・版が変わる
    function is_own(i) {
      return plain_step(i) && keys_ok(st_keys[i], "uses with", "name") && pinned_uses(i) \
        && keys_ok(st_wkeys[i], "persist-credentials", "") && st_pc[i] == "false"
    }
    # run_line <i>: run の値が前後の空白を除いて 1 行ならその行、そうでなければ改行（どの正準の行とも一致しない）
    function run_line(i,   n, a, j, c, r, t) {
      if (st_style[i] == "plain" && !st_multi[i]) return st_run[i]
      if (st_style[i] != "block") return "\n"
      n = split(st_run[i], a, "\n"); c = 0; r = ""
      for (j = 1; j <= n; j++) { t = trim(a[j]); if (t != "") { c++; r = t } }
      return c == 1 ? r : "\n"
    }
    # is_out <i>: 正準の出力 step か（run は文字列の完全一致。if・env・shell・working-directory 等の key を持たない）
    function is_out(i) {
      return plain_step(i) && keys_ok(st_keys[i], "id run", "name") && st_id[i] ~ /^[A-Za-z0-9_-]+$/ && run_line(i) == canon
    }
    # is_eng <i> <aid>: 正準のエンジンの checkout か（ref は step 2 の出力 aid の rev だけ）
    function is_eng(i, aid) {
      return plain_step(i) && keys_ok(st_keys[i], "uses with", "name") && pinned_uses(i) \
        && keys_ok(st_wkeys[i], "repository ref path persist-credentials", "") && st_repo[i] == guard \
        && aid != "" && st_ref[i] == "${{ steps." aid ".outputs.rev }}" && st_path[i] ~ /^[A-Za-z0-9_-]+$/ && st_pc[i] == "false"
    }
    # c_why <i> <ep>: 正準の 3 step より後ろの step が無関係な step の条件を満たさなければ理由を返す（名前の行は見ない）
    function c_why(i, ep,   t, u) {
      t = st_text[i]
      if (st_bad[i] || st_wbad[i]) return "cannot be read as a plain block mapping (alias, anchor, tag, flow style or an unexpected line)"
      # checkout は保存先（path を省けば作業領域のルート）を clean するため、エンジンを消しうる。保存先は解釈せず一律に拒否する
      u = tolower(st_uses[i]); gsub(/["\047]/, "", u)
      if (u ~ /^actions\/checkout@/) return "is an actions/checkout (a checkout cleans its destination, the workspace root when path is omitted, and can remove or replace the engine; destinations are not interpreted)"
      # uses を複数行のスカラーで書くと値を字句で確定できない。step の本文のどこかに checkout が現れれば同じく拒否する
      if (u ~ /^[>|]/) return "has a uses value that is not on the same line (block or folded scalar)"
      if (index(tolower(t), "actions/checkout")) return "mentions actions/checkout (a checkout can remove or replace the engine; only a plain same-line uses is read)"
      if (tolower(t) ~ /(^|[^a-z0-9])git([^a-z0-9]|$)/) return "mentions git"
      if (index(tolower(t), "fandhe-ai/fandhe-workshop")) return "mentions the engine repo"
      if (index(tolower(t), "engine_rev")) return "mentions ENGINE_REV"
      if (ep != "" && index(t, ep)) return "mentions the engine path " ep
      if (st_repo[i] ~ /\$\{\{/ || st_ref[i] ~ /\$\{\{/ || st_path[i] ~ /\$\{\{/) return "uses an expression in repository, ref or path"
      return ""
    }
    function end_job(   n, s1, s2, s3, aid, ep, k, r) {
      if (job == "") return
      if (job_eng) {
        any_eng = 1
        n = gsn - jfirst + 1; s1 = jfirst; s2 = jfirst + 1; s3 = jfirst + 2
        if (job_badline) print job_badline ":D3 job " job " has " job_badkey "; a job that refers to the engine repo or ENGINE_REV may only use the keys " JOB_KEYS " (env, defaults and the like change how the canonical steps run)"
        if (n < 1 || !is_own(s1)) print (n >= 1 ? st_line[s1] : job_line) ":D3 step 1 of job " job " must be the canonical checkout of this repository: " OWN
        if (n < 2 || !is_out(s2)) print (n >= 2 ? st_line[s2] : job_line) ":D3 step 2 of job " job " must be the canonical ENGINE_REV output step: " OUT
        aid = (n >= 2 && is_out(s2)) ? st_id[s2] : ""
        if (n < 3 || !is_eng(s3, aid)) print (n >= 3 ? st_line[s3] : job_line) ":D3 step 3 of job " job " must be the canonical engine checkout: " ENG
        else if (is_own(s1)) eng_ok++
        ep = (n >= 3 && st_path[s3] ~ /^[A-Za-z0-9_-]+$/) ? st_path[s3] : ""
        for (k = 4; k <= n; k++) {
          r = c_why(jfirst + k - 1, ep)
          if (r != "") print st_line[jfirst + k - 1] ":D4 step " k " of job " job " " r "; only the three canonical steps (" OWN "; " OUT "; " ENG ") may use actions/checkout or refer to git, the engine repo, ENGINE_REV or the engine path"
        }
      }
      job = ""
    }
    {
      raw = $0; ind = indent(raw)
      if (job != "" && !(in_jobs && ind == 2) && engine_ref(raw)) job_eng = 1
      # ブロック形式の run の中身（# で始まる行も中身）。プレーンの run が次の行へ続く形は正準ではない
      if (cur_step && cur == "run" && raw !~ /^[[:space:]]*$/ && ind > key_ind) {
        st_text[s] = st_text[s] "\n" raw
        if (st_style[s] == "block") st_run[s] = st_run[s] "\n" raw; else st_multi[s] = 1
        next
      }
      if (raw ~ /^[[:space:]]*$/ || raw ~ /^[[:space:]]*#/) next
      if (ind == 0) {
        k = key_of(raw)
        if (k == "jobs") { end_job(); in_jobs = 1; next }
        end_job(); in_jobs = 0
        if ((k == "env" || k == "defaults") && !wf_line) { wf_line = NR; wf_key = k }
        next
      }
      if (!in_jobs) next
      if (ind == 2) { start_job(key_of(trim(raw))); if (engine_ref(raw)) job_eng = 1; next }
      if (job == "") next
      if (jk < 0) jk = ind
      if (ind <= jk) {
        k = key_of(trim(raw)); in_steps = 0; cur_step = 0; cur = ""
        if (ind == jk && k == "steps" && raw_of(trim(raw)) ~ /^(#.*)?$/) { in_steps = 1; item_ind = -1; next }
        if (!job_badline && (ind < jk || !(k in job_ok))) { job_badline = NR; job_badkey = (k == "" ? "an unexpected line" : k) }
        next
      }
      if (!in_steps) next
      if (raw ~ /^[[:space:]]*-([[:space:]]|$)/ && (item_ind < 0 || ind == item_ind)) {
        item_ind = ind; s = ++gsn; cur_step = 1; cur = ""; st_line[s] = NR
        body = raw; sub(/^[[:space:]]*-[[:space:]]*/, "", body)
        if (body == "") { key_ind = -1; next }
        key_ind = length(raw) - length(body)
        key_line(body, raw)
        next
      }
      if (!cur_step) { if (!job_badline) { job_badline = NR; job_badkey = "an unexpected line under steps" } ; next }
      body = trim(raw)
      if (key_ind < 0) key_ind = ind
      if (ind == key_ind) { key_line(body, raw); next }
      st_text[s] = st_text[s] "\n" raw
      if (ind < key_ind) { st_bad[s] = 1; next }
      # 現在の key の値の続き。with の直下は入力として読み、ref・path 等の値が次の行へ続く形は正準ではない
      if (cur == "with") {
        if (wind < 0) wind = ind
        if (ind == wind) with_line(body)
        else if (wcur == "repository" || wcur == "ref" || wcur == "path" || wcur == "persist-credentials") st_wbad[s] = 1
      }
    }
    END {
      end_job()
      if (any_eng && wf_line) print wf_line ":D3 workflow-level " wf_key " changes how the canonical steps run; remove it from a workflow that fetches the engine"
      if (eng_ok == 0) print "0:D2 no job fetches the engine repo " guard " with the canonical steps (1. " OWN "; 2. " OUT "; 3. " ENG ")"
    }
  ' "${PACK_WORKFLOW}")" || rc=$?
  [ "${rc}" -eq 0 ] || { echo "error: awk failed on ${PACK_WORKFLOW}" >&2; exit 2; }
  report "${PACK_WORKFLOW}" "${d_out}"
fi

if [ "${violations}" -gt 0 ]; then
  echo "fork CI check failed: ${violations} violation(s)" >&2
  exit 1
fi
echo "fork CI check passed (no private dependencies, no secret references, deploy is detached)"
