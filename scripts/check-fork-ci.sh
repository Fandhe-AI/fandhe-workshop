#!/usr/bin/env bash
# fork の CI の構成（secret なしでビルドと試験が通る形）を静的に検査する
# （`make check-fork-ci` と ci.yml の check ジョブから呼ばれる。TASK-96・BUILD-22）。
#
# 理由: 第三者がエンジン repo を fork し、secret を設定しないまま CI を流してもビルドと試験が通る、
# という性質を、fork での実行（D-29 により行わない）ではなく構成の検査で確かめる。
# 検査は行ベース（awk・grep・jq）で、YAML の構文木は作らない。既存の書式（インデント 2・ブロック形式）に
# 依存するため、書式を変える場合は scripts/test-check-fork-ci.sh の見本も同時に直す。
#
# 検査の項目:
#   A private な依存が 0 件
#     A1 Cargo.lock の source が crates.io（と許可リストの公開 git）だけ
#     A2 package-lock.json の resolved が registry.npmjs.org だけ
#     A3 .npmrc に registry・認証の行が無い
#     A4 .cargo/config(.toml) に registry・認証・git-fetch-with-cli が無い
#     A5 .gitmodules の各 submodule が `update = none` を持ち、ci.yml の checkout が submodule を取得しない
#   B ビルド・試験のジョブ（ci.yml）からの secret の参照が 0 件
#     B1 scripts/check-workflow-secrets.sh を呼ぶ（実体は二重に書かない。ruby を要する）
#     B2 BUILD-2 が挙げる語（GITHUB_TOKEN・CARGO_REGISTR*・git-fetch-with-cli・語としての _TOKEN / _PAT）が無い
#     B3 uses: が 40 桁の SHA 固定で、所有者が許可リストにあり、reusable workflow を呼ばない
#   C 配信の切り離し
#     C1 ci.yml に write の権限・environment が無い
#     C2 deploy.yml が workflow_dispatch だけで動き、permissions が {} で、全ジョブが fork では飛ばされる
#     C3 ci.yml に deploy ジョブが無い
#   D ENGINE_REV での教材 repo のビルド（--pack-workflow と --engine-rev の両方を渡したときだけ）
#     D1 ENGINE_REV が 40 桁の小文字の 16 進 1 行
#     D2 エンジン repo を取得する checkout が 1 つ以上ある
#     D3 その ref が ENGINE_REV の値、または ENGINE_REV を読む step の出力
#     D4 run でエンジン repo を取得する行が ENGINE_REV を参照している
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

# ---- A1 Cargo.lock ----
need_file "${CARGO_LOCK}"
a1_out=""
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
$(grep -n '^source = ' -- "${CARGO_LOCK}" || true)
EOT
report "${CARGO_LOCK}" "${a1_out}"

# ---- A2 package-lock.json ----
need_file "${PACKAGE_LOCK}"
rc=0
a2_out="$(jq -r '[.. | objects | .resolved? | select(type == "string") | select(startswith("https://registry.npmjs.org/") | not)] | .[] | "0:A2 npm dependency resolved outside registry.npmjs.org: " + .' "${PACKAGE_LOCK}")" || rc=$?
if [ "${rc}" -ne 0 ]; then
  echo "error: cannot parse ${PACKAGE_LOCK} as JSON" >&2
  exit 2
fi
# jq の出力に行番号は無い。resolved の値は公開 URL のため、そのまま出してよい
report "${PACKAGE_LOCK}" "${a2_out}"

# ---- A3 .npmrc（無ければ合格） ----
if [ -e "${NPMRC}" ]; then
  need_file "${NPMRC}"
  report "${NPMRC}" "$(grep_hits "A3 .npmrc sets a registry or credentials" "${NPMRC}" '^[[:space:]]*(@[^=]*:)?registry[[:space:]]*=|_authtoken|_auth[[:space:]]*=|_password|^[[:space:]]*//' -i)"
fi

# ---- A4 .cargo/config(.toml)（無ければ合格） ----
for cfg in "${CARGO_CONFIG_DIR}/config" "${CARGO_CONFIG_DIR}/config.toml"; do
  if [ -e "${cfg}" ]; then
    need_file "${cfg}"
    report "${cfg}" "$(grep_hits "A4 cargo config sets a registry, source replacement or credentials" "${cfg}" '\[registries|\[source|replace-with|git-fetch-with-cli|credential-provider|token' -i)"
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
report "${CI_FILE}" "$(grep_hits "B2 forbidden credential word (GITHUB_TOKEN, CARGO_REGISTR*, git-fetch-with-cli, *_TOKEN, *_PAT)" "${CI_FILE}" 'GITHUB_TOKEN|CARGO_REGISTR|git-fetch-with-cli|_(TOKEN|PAT)([^A-Za-z0-9_]|$)' -i)"

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

# ---- C1 ci.yml に write の権限・environment が無い ----
report "${CI_FILE}" "$(grep_hits "C1 write permission (the build/test workflow must be read-only)" "${CI_FILE}" ':[[:space:]]*write([[:space:]#]|$)|write-all' -i)"
report "${CI_FILE}" "$(grep_hits "C1 environment (deployment settings belong in deploy.yml)" "${CI_FILE}" '^[[:space:]]*(-[[:space:]]+)?environment[[:space:]]*:' -i)"

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
    if (index($0, "github.repository == \047" repo "\047") > 0) job_guard = 1
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
report "${CI_FILE}" "$(grep_hits "C3 deploy job in the build/test workflow (it belongs in deploy.yml)" "${CI_FILE}" '^  deploy[[:space:]]*:')"

# ---- D ENGINE_REV ----
if [ -n "${PACK_WORKFLOW}" ]; then
  need_file "${PACK_WORKFLOW}"
  need_file "${ENGINE_REV_FILE}"
  # D1: 40 桁の小文字の 16 進が 1 行だけ（末尾の改行 1 つは許す。空白・余分な行は不可）
  rev_lines="$(awk 'END { print NR }' "${ENGINE_REV_FILE}")"
  rev="$(head -n 1 "${ENGINE_REV_FILE}")"
  if [ "${rev_lines}" -ne 1 ] || ! printf '%s' "${rev}" | grep -Eq '^[0-9a-f]{40}$'; then
    fail "${ENGINE_REV_FILE}:1:D1 ENGINE_REV must be a single line of 40 lowercase hex digits"
    rev="invalid"
  fi
  rc=0
  d_out="$(awk -v rev="${rev}" -v guard="${GUARD_REPO}" '
    function indent(s) { match(s, /^ */); return RLENGTH }
    function trim_val(s) {
      sub(/^[^:]*:[[:space:]]*/, "", s); sub(/[[:space:]]*#.*$/, "", s); gsub(/["\047]/, "", s); return s
    }
    function end_step(   m, id) {
      if (!in_step) return
      if (s_uses ~ /^actions\/checkout@/ && tolower(s_repo) == tolower(guard)) {
        engine++
        if (s_ref == "") print s_line ":D3 engine checkout has no ref (the version is not pinned)"
        else if (s_ref == rev) { }
        else if (match(s_ref, /^\$\{\{ *steps\.[A-Za-z0-9_-]+\.outputs\.[A-Za-z0-9_-]+ *\}\}$/)) {
          m = s_ref; sub(/^\$\{\{ *steps\./, "", m); sub(/\..*$/, "", m)
          pend[++np] = m; pend_line[np] = s_line
        } else print s_line ":D3 engine checkout ref is not ENGINE_REV or an output of the step that reads it"
      }
      if (s_id != "") reads[s_id] = (index(s_run, "ENGINE_REV") > 0)
      if (tolower(s_run) ~ /fandhe-ai\/fandhe-workshop/ && tolower(s_run) ~ /(git clone|cargo install|git fetch|gh repo clone|git submodule|curl|wget)/ && index(s_run, "ENGINE_REV") == 0)
        print s_line ":D4 run fetches the engine repo without referencing ENGINE_REV"
      in_step = 0
    }
    /^[[:space:]]*($|#)/ { next }
    {
      ind = indent($0)
      if ($0 ~ /^[[:space:]]*steps[[:space:]]*:/) { end_step(); in_steps = 1; steps_ind = ind; item_ind = -1; next }
      if (in_steps && ind <= steps_ind) { end_step(); in_steps = 0 }
      if (!in_steps) next
      if ($0 ~ /^[[:space:]]*-[[:space:]]/ && (item_ind < 0 || ind == item_ind)) {
        end_step(); item_ind = ind
        in_step = 1; s_uses = ""; s_repo = ""; s_ref = ""; s_id = ""; s_run = ""; in_run = 0; s_line = NR
        body = $0; sub(/^[[:space:]]*-[[:space:]]+/, "", body); line = body; key_ind = ind + 2
      } else line = $0
      if (!in_step) next
      if (in_run && indent($0) > key_ind) { s_run = s_run "\n" $0; next }
      in_run = 0
      if (line ~ /^[[:space:]]*uses[[:space:]]*:/) s_uses = trim_val(line)
      else if (line ~ /^[[:space:]]*repository[[:space:]]*:/) s_repo = trim_val(line)
      else if (line ~ /^[[:space:]]*ref[[:space:]]*:/) {
        s_ref = line; sub(/^[[:space:]]*ref[[:space:]]*:[[:space:]]*/, "", s_ref); sub(/[[:space:]]+#.*$/, "", s_ref); gsub(/["\047]/, "", s_ref)
      }
      else if (line ~ /^[[:space:]]*id[[:space:]]*:/) s_id = trim_val(line)
      else if (line ~ /^[[:space:]]*run[[:space:]]*:/) { s_run = s_run "\n" trim_val(line); in_run = 1 }
    }
    END {
      end_step()
      if (engine == 0) print "0:D2 no actions/checkout step fetches the engine repo " guard
      for (i = 1; i <= np; i++) if (!(pend[i] in reads) || !reads[pend[i]]) print pend_line[i] ":D3 ref comes from step " pend[i] " which does not read ENGINE_REV"
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
