#!/usr/bin/env bash
# fandhe-frontend の版上げ PR の受け入れ確認 (1)(2)(4)(5) の関門（FEND-17・D-63・TASK-101 1/2）。
#
# なぜ存在するか: 版上げは (1) ビルド・clippy・fmt、(2) 既存の試験の件数、(3) WASM の gzip +10%、(4) check-deps.sh の
# 例外の数、(5) wasm-full の feature の明示 の 5 項目すべてが外れないことを、版上げ PR の CI で確かめて通す（D-63）。
# (1)(4)(5) は ci.yml の既存ジョブがすでに全 PR で流している。本スクリプトは次の 2 つの穴を塞ぐ。
#   - 版上げ PR の中で (4)(5) の関門自体（tools/check-deps.sh・期待値・本スクリプト）を緩めて通すこと
#   - (2) の件数の照合が無いこと。期待値は scripts/expected-test-counts.txt に固定する
# (3) は別の Issue（TASK-101 2/2）が受け持ち、版上げ PR の判定（detect）を本スクリプトから再利用する。
#
# 呼び出し元: ci.yml の rust-lint（guard）・rust-test（counts rust）・browser-test（counts browser）。
# 既存ジョブへの step 追加のため、ジョブの集合・名前・ci-complete の needs・ruleset の必須チェックは変わらない。
# 件数の照合は版上げ PR か期待値ファイルを変えた PR に限る。試験を足す PR のすべてが同じファイルを書き換える運用を避けるため。
#
# サブコマンド:
#   detect                     版上げ PR なら true、そうでなければ false を標準出力へ出す
#   guard                      (5) の feature の明示を cargo metadata で確かめ、PR なら版上げ PR で変えてはいけないファイルを確かめる
#   counts rust <log>          make test の出力の件数を期待値と照合する（版上げ PR か期待値の変更のときだけ）
#   counts browser <tap>       node --test の TAP の件数を期待値と照合する（同上）
#   normalize rust <log>       件数を正規化した行を出す（期待値の書式）
#   normalize browser <tap>    同上（ブラウザ）
# オプション: --repo <dir>（既定は repo ルート）・--base <rev>（既定 HEAD^1。PR の merge commit の第 1 親 = base）・
#   --changed-files <file>・--cargo-metadata-json <file>・--expected <file>（後ろの 3 つは回帰テスト用。CI は渡さない）。
# 入力: CI_EVENT_NAME（pull_request のときだけ git の差分で判定する。それ以外は skip）。
# 限界: ci.yml 自体の書き換えは防げない（版上げで wasm-bindgen の版を合わせる正当な変更があるため守る対象にしない）。
# それは ai-review とレビュースレッドの解決必須で補う。
#
# 終了コード: 0 = 合格か skip / 1 = 基準外 / 2 = 誤用か判定不能（fail-closed）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

EXPECTED_REL="scripts/expected-test-counts.txt"
# 版上げ PR で base と同じであることを求めるファイル（穴 A を塞ぐ）。
GUARDED_FILES=("${EXPECTED_REL}" "tools/check-deps.sh" "scripts/check-frontend-bump.sh")

usage() {
  echo "usage: check-frontend-bump.sh [options] {detect|guard|counts rust <log>|counts browser <tap>|normalize rust <log>|normalize browser <tap>}" >&2
  echo "options: --repo <dir> --base <rev> --changed-files <file> --cargo-metadata-json <file> --expected <file>" >&2
}

REPO="${ROOT_DIR}"
BASE="HEAD^1"
CHANGED_FILES=""
METADATA_JSON=""
EXPECTED_FILE=""
ARGS=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo | --base | --changed-files | --cargo-metadata-json | --expected)
      if [ "$#" -lt 2 ]; then
        usage
        exit 2
      fi
      case "$1" in
        --repo) REPO="$2" ;;
        --base) BASE="$2" ;;
        --changed-files) CHANGED_FILES="$2" ;;
        --cargo-metadata-json) METADATA_JSON="$2" ;;
        --expected) EXPECTED_FILE="$2" ;;
      esac
      shift 2
      ;;
    --*)
      usage
      exit 2
      ;;
    *)
      ARGS+=("$1")
      shift
      ;;
  esac
done
if [ "${#ARGS[@]}" -eq 0 ]; then
  usage
  exit 2
fi
[ -n "${EXPECTED_FILE}" ] || EXPECTED_FILE="${REPO}/${EXPECTED_REL}"

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-bump.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

fail2() {
  echo "error: $*" >&2
  exit 2
}

is_pr() {
  [ "${CI_EVENT_NAME:-}" = "pull_request" ]
}

# frontend_state <rev> <out>: その rev の fandhe-frontend-* の宣言と解決済みの版を、並べ替えた行にして書く。
# git show の失敗は呼び出し側で終了状態を見る（$(...) の中の失敗を見逃さない）。
frontend_state() {
  local rev="$1" out="$2"
  git -C "${REPO}" show "${rev}:Cargo.lock" >"${TMP}/lock.txt" || return 1
  git -C "${REPO}" show "${rev}:Cargo.toml" >"${TMP}/root.toml" || return 1
  git -C "${REPO}" show "${rev}:crates/web/Cargo.toml" >"${TMP}/web.toml" || return 1
  {
    awk '
      function flush() {
        if (name ~ /^fandhe-frontend-/) print "lock|" name "|" version "|" source "|" checksum
        name = ""; version = ""; source = ""; checksum = ""
      }
      /^\[\[package\]\]/ { flush(); next }
      /^name = / { name = $0; sub(/^name = "/, "", name); sub(/"$/, "", name) }
      /^version = / { version = $0; sub(/^version = "/, "", version); sub(/"$/, "", version) }
      /^source = / { source = $0; sub(/^source = "/, "", source); sub(/"$/, "", source) }
      /^checksum = / { checksum = $0; sub(/^checksum = "/, "", checksum); sub(/"$/, "", checksum) }
      END { flush() }
    ' "${TMP}/lock.txt"
    # grep は該当 0 行で 1 を返す。宣言が無いこと自体は HEAD の空判定（detect_bump）で扱うため、ここでは許容する。
    grep '^fandhe-frontend-' "${TMP}/root.toml" | sed 's/^/root|/' || true
    grep '^fandhe-frontend-' "${TMP}/web.toml" | sed 's/^/web|/' || true
  } | LC_ALL=C sort >"${out}"
}

detect_bump() {
  git -C "${REPO}" rev-parse --verify --quiet "${BASE}^{commit}" >/dev/null ||
    fail2 "base revision not found: ${BASE} (checkout needs fetch-depth: 2)"
  frontend_state "${BASE}" "${TMP}/state-base.txt" || fail2 "cannot read files at base ${BASE}"
  frontend_state "HEAD" "${TMP}/state-head.txt" || fail2 "cannot read files at HEAD"
  if [ ! -s "${TMP}/state-head.txt" ]; then
    fail2 "no fandhe-frontend-* entries found at HEAD (parse failure)"
  fi
  if cmp -s "${TMP}/state-base.txt" "${TMP}/state-head.txt"; then
    echo false
  else
    echo true
  fi
}

# changed_list <out>: base から HEAD までに変わったファイルの一覧。
changed_list() {
  local out="$1"
  if [ -n "${CHANGED_FILES}" ]; then
    cp "${CHANGED_FILES}" "${out}" || fail2 "cannot read ${CHANGED_FILES}"
    return 0
  fi
  git -C "${REPO}" diff --name-only --no-renames "${BASE}" HEAD >"${out}" ||
    fail2 "cannot diff ${BASE}..HEAD"
}

is_changed() {
  grep -qxF -- "$1" "${TMP}/changed.txt"
}

# (5) wasm-full が default-features = false で action-keydown を明示していること。
check_features() {
  local json="${TMP}/metadata.json" ok
  if [ -n "${METADATA_JSON}" ]; then
    cp "${METADATA_JSON}" "${json}" || fail2 "cannot read ${METADATA_JSON}"
  else
    require_cmd cargo
    (cd "${REPO}" && cargo metadata --no-deps --format-version 1 --locked) >"${json}" ||
      fail2 "cargo metadata failed"
  fi
  require_cmd jq
  ok="$(jq -r '
    [ .packages[] | select(.name == "fandhe-workshop-web") | .dependencies[]
      | select(.name == "fandhe-frontend-wasm-full") ]
    | if length > 0 and all(.uses_default_features == false and ((.features // []) | index("action-keydown") != null))
      then "ok" else "ng" end
  ' "${json}")" || fail2 "cannot parse cargo metadata JSON"
  case "${ok}" in
    ok) log "(5) ok: fandhe-frontend-wasm-full has default-features = false and action-keydown" ;;
    *)
      echo "error: (5) fandhe-frontend-wasm-full must be declared with default-features = false and features including action-keydown (FEND-17)" >&2
      return 1
      ;;
  esac
}

cmd_guard() {
  local rc=0 bump f
  check_features || rc=1
  if ! is_pr; then
    log "not a pull_request event; skipping version-bump file guard"
    return "${rc}"
  fi
  bump="$(detect_bump)"
  if [ "${bump}" != "true" ]; then
    log "not a frontend version bump; file guard not applicable"
    return "${rc}"
  fi
  changed_list "${TMP}/changed.txt"
  for f in "${GUARDED_FILES[@]}"; do
    if is_changed "${f}"; then
      echo "error: a fandhe-frontend version bump must not change ${f} (FEND-17 / D-63). Update it in a separate PR before the bump." >&2
      rc=1
    fi
  done
  [ "${rc}" -eq 0 ] && log "version bump detected; guarded files unchanged"
  return "${rc}"
}

# normalize_rust <log>: ANSI を除いて test result 行を記述子ごとに合算し、期待値の書式で出す。
normalize_rust() {
  local logf="$1" esc
  esc="$(printf '\033')"
  [ -r "${logf}" ] || fail2 "cannot read ${logf}"
  sed "s/${esc}\[[0-9;]*[A-Za-z]//g" "${logf}" >"${TMP}/rust-plain.txt"
  awk '
    /^[ ]*Running / {
      line = $0
      sub(/^[ ]*Running /, "", line)
      path = line; sub(/ \(.*$/, "", path)
      bin = line; sub(/^.*\(/, "", bin); sub(/\)$/, "", bin)
      sub(/^.*\//, "", bin); sub(/-[0-9a-f]+$/, "", bin)
      desc = path " [" bin "]"
      next
    }
    /^[ ]*Doc-tests / {
      line = $0
      sub(/^[ ]*Doc-tests /, "", line)
      desc = "doc-tests " line
      next
    }
    /^test result: / {
      if (desc == "") { bad = 1; next }
      if ($0 !~ / 0 failed;/) { failed = 1 }
      p = $4; ig = $8
      passed[desc] += p; ignored[desc] += ig
      tp += p; ti += ig
      if (!(desc in seen)) { seen[desc] = 1; n++ }
      desc = ""
      results++
      next
    }
    END {
      if (results == 0) { print "no test result lines" > "/dev/stderr"; exit 2 }
      if (bad) { print "test result line without a Running line" > "/dev/stderr"; exit 2 }
      if (failed) { print "failing tests in log" > "/dev/stderr"; exit 2 }
      for (d in passed) printf "rust|%s|passed=%d|ignored=%d\n", d, passed[d], ignored[d]
      printf "rust|total|binaries=%d|passed=%d|ignored=%d\n", n, tp, ti
    }
  ' "${TMP}/rust-plain.txt" | LC_ALL=C sort
}

# normalize_browser <tap>: TAP 末尾の集計（インデント無しの最後の出現）を読む。
normalize_browser() {
  local tap="$1"
  [ -r "${tap}" ] || fail2 "cannot read ${tap}"
  awk '
    /^# tests [0-9]+$/ { v["tests"] = $3 }
    /^# pass [0-9]+$/ { v["pass"] = $3 }
    /^# fail [0-9]+$/ { v["fail"] = $3 }
    /^# cancelled [0-9]+$/ { v["cancelled"] = $3 }
    /^# skipped [0-9]+$/ { v["skipped"] = $3 }
    /^# todo [0-9]+$/ { v["todo"] = $3 }
    END {
      split("tests pass fail cancelled skipped todo", k, " ")
      for (i = 1; i <= 6; i++) if (!(k[i] in v)) { print "TAP summary is missing: " k[i] > "/dev/stderr"; exit 2 }
      if (v["fail"] != 0) { print "TAP reports failures" > "/dev/stderr"; exit 2 }
      printf "browser|total|tests=%d|pass=%d|skipped=%d|todo=%d|cancelled=%d\n", v["tests"], v["pass"], v["skipped"], v["todo"], v["cancelled"]
    }
  ' "${tap}"
}

cmd_normalize() {
  local kind="${1:-}" file="${2:-}"
  [ -n "${file}" ] || {
    usage
    exit 2
  }
  case "${kind}" in
    rust) normalize_rust "${file}" ;;
    browser) normalize_browser "${file}" ;;
    *)
      usage
      exit 2
      ;;
  esac
}

cmd_counts() {
  local kind="${1:-}" file="${2:-}" bump="false" exp_changed="false"
  [ -n "${file}" ] || {
    usage
    exit 2
  }
  case "${kind}" in rust | browser) ;; *)
    usage
    exit 2
    ;;
  esac
  # 先に解析する。解析できないログは skip の対象にせず fail-closed にする。
  cmd_normalize "${kind}" "${file}" >"${TMP}/actual.txt" || fail2 "cannot parse ${kind} test output"
  if ! is_pr; then
    log "not a pull_request event; skipping ${kind} test count check"
    return 0
  fi
  bump="$(detect_bump)"
  changed_list "${TMP}/changed.txt"
  if is_changed "${EXPECTED_REL}"; then exp_changed="true"; fi
  if [ "${bump}" = "true" ] && [ "${exp_changed}" = "true" ]; then
    echo "error: a fandhe-frontend version bump must not change ${EXPECTED_REL}. Update it in a separate PR before the bump." >&2
    return 1
  fi
  if [ "${bump}" != "true" ] && [ "${exp_changed}" != "true" ]; then
    log "not a version bump and ${EXPECTED_REL} unchanged; skipping ${kind} count check. actual:"
    sed 's/^/  /' "${TMP}/actual.txt" | grep -E "\|total\|" >&2 || true
    return 0
  fi
  [ -r "${EXPECTED_FILE}" ] || fail2 "cannot read ${EXPECTED_FILE}"
  grep "^${kind}|" "${EXPECTED_FILE}" | LC_ALL=C sort >"${TMP}/expected.txt" || true
  if [ ! -s "${TMP}/expected.txt" ]; then
    fail2 "no ${kind} entries in ${EXPECTED_FILE}"
  fi
  if diff -u "${TMP}/expected.txt" "${TMP}/actual.txt" >&2; then
    log "(2) ok: ${kind} test counts match ${EXPECTED_REL}"
    return 0
  fi
  echo "error: (2) ${kind} test counts differ from ${EXPECTED_REL} (FEND-17 / D-63). If a version bump added or removed tests, fix the cause. If the expectation is stale, run \`make update-test-counts\` in a separate PR." >&2
  return 1
}

case "${ARGS[0]}" in
  detect)
    [ "${#ARGS[@]}" -eq 1 ] || {
      usage
      exit 2
    }
    detect_bump
    ;;
  guard)
    [ "${#ARGS[@]}" -eq 1 ] || {
      usage
      exit 2
    }
    cmd_guard
    ;;
  counts)
    [ "${#ARGS[@]}" -eq 3 ] || {
      usage
      exit 2
    }
    cmd_counts "${ARGS[1]}" "${ARGS[2]}"
    ;;
  normalize)
    [ "${#ARGS[@]}" -eq 3 ] || {
      usage
      exit 2
    }
    cmd_normalize "${ARGS[1]}" "${ARGS[2]}"
    ;;
  *)
    usage
    exit 2
    ;;
esac
