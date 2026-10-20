#!/usr/bin/env bash
# fandhe-frontend の版上げ PR の受け入れ確認 (1)〜(5) の関門（FEND-17・D-63・TASK-101 1/2・2/2）。
#
# なぜ存在するか: 版上げは (1) ビルド・clippy・fmt、(2) 既存の試験の件数、(3) WASM の gzip +10%、(4) check-deps.sh の
# 例外の数、(5) wasm-full の feature の明示 の 5 項目すべてが外れないことを、版上げ PR の CI で確かめて通す（D-63）。
# (1)(4)(5) は ci.yml の既存ジョブがすでに全 PR で流している。本スクリプトは次の 2 つの穴を塞ぐ。
#   - 版上げ PR の中で (4)(5) の関門自体（tools/check-deps.sh・期待値・本スクリプト）を緩めて通すこと
#   - (2) の件数の照合が無いこと。期待値は scripts/expected-test-counts.txt に固定する
# (3) は wasm-gzip が受け持つ（TASK-101 2/2）。版上げ PR のときだけ、同じ runner・同じジョブの中で base と HEAD を
# 同じ手順（release・TEST_HOOKS なし・SKIP_WASM_OPT=1）でビルドし、配信用 WASM の gzip -9 の大きさの増加が +10% 以内かを判定する。
# 値を repo に記録しないのは、版上げ以外の PR で古くなるため、また手元（wasm-opt あり）と CI（なし）で基点が食い違うため
# （docs/measurement.md の 6 章が禁じる比較）。wasm-opt は CI に入れない（供給網の面を増やさない）。
# 基点を揃えられない場合（toolchain・wasm-bindgen の版が base と違う）は、別の PR で先に上げるよう案内して失敗する。
#
# 呼び出し元: ci.yml の rust-lint（guard）・rust-test（counts rust）・browser-test（counts browser・wasm-gzip）。
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
#   wasm-gzip                  (3) 版上げ PR のとき、base と HEAD の配信用 WASM の gzip -9 の増加が +10% 以内かを判定する
#                              （head*100 <= base*110 なら合格。ちょうど +10% は合格）。ビルドを 2 回行う（遅い）
# オプション: --repo <dir>（既定は repo ルート）・--base <rev>（既定 HEAD^1。PR の merge commit の第 1 親 = base）・
#   --changed-files <file>・--cargo-metadata-json <file>・--expected <file>・--base-build-log <file>・--head-build-log <file>
#   （これらは回帰テスト用。CI は渡さない。build-log を渡すとビルドを省き、ログの解析と判定だけを行う）・
#   --assume-bump（wasm-gzip で版上げの検出を飛ばして測る。手元での実物の確認用で、判定を厳しくする向きにしか働かない。CI は渡さない）。
# 入力: CI_EVENT_NAME（pull_request のときだけ git の差分で判定する。それ以外は skip）。
# 限界: ci.yml 自体の書き換えは防げない（守る対象にしない）。Cargo.toml の [profile.release] の変更も防げない
# （版上げで Cargo.toml を変えるのは正当なため）。それは ai-review とレビュースレッドの解決必須で補う。
#
# 終了コード: 0 = 合格か skip / 1 = 基準外 / 2 = 誤用か判定不能（fail-closed）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

EXPECTED_REL="scripts/expected-test-counts.txt"
# 版上げ PR で base と同じであることを求めるファイル（穴 A を塞ぐ）。
# tools/build-web.sh は (3) の測定の手順（wasm-opt の扱い・出力の作り方）を、版上げ PR の中で head 側だけ変えて通すことを防ぐ。
GUARDED_FILES=("${EXPECTED_REL}" "tools/check-deps.sh" "scripts/check-frontend-bump.sh" "tools/build-web.sh")

usage() {
  echo "usage: check-frontend-bump.sh [options] {detect|guard|wasm-gzip|counts rust <log>|counts browser <tap>|normalize rust <log>|normalize browser <tap>}" >&2
  echo "options: --repo <dir> --base <rev> --changed-files <file> --cargo-metadata-json <file> --expected <file> --base-build-log <file> --head-build-log <file> --assume-bump" >&2
}

REPO="${ROOT_DIR}"
BASE="HEAD^1"
CHANGED_FILES=""
METADATA_JSON=""
EXPECTED_FILE=""
BASE_BUILD_LOG=""
HEAD_BUILD_LOG=""
ASSUME_BUMP="false"
ARGS=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo | --base | --changed-files | --cargo-metadata-json | --expected | --base-build-log | --head-build-log)
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
        --base-build-log) BASE_BUILD_LOG="$2" ;;
        --head-build-log) HEAD_BUILD_LOG="$2" ;;
      esac
      shift 2
      ;;
    --assume-bump)
      ASSUME_BUMP="true"
      shift
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

require_cmd grep

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-bump.XXXXXX")"
# wasm-gzip が作る一時の worktree。TMP の削除の前に git から外し、.git/worktrees の残骸も掃除する。
WORKTREES=()
cleanup() {
  local w
  for w in ${WORKTREES[@]+"${WORKTREES[@]}"}; do
    git -C "${REPO}" worktree remove --force "${w}" >/dev/null 2>&1 || true
  done
  rm -rf "${TMP}"
  if [ "${#WORKTREES[@]}" -gt 0 ]; then
    git -C "${REPO}" worktree prune >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

fail2() {
  echo "error: $*" >&2
  exit 2
}

is_pr() {
  [ "${CI_EVENT_NAME:-}" = "pull_request" ]
}

# toml_decls <prefix> <file>: fandhe-frontend-* の依存宣言の全体を prefix つきの行にして出す。
# 対象は 1 行の宣言・複数行の配列や inline table（括弧の深さで追う）・[...fandhe-frontend-x] 形式のテーブルとその中身。
# 該当 0 行は正常（空出力。宣言が無いこと自体は detect_bump の HEAD の空判定で扱う）。awk の異常は終了状態で伝える。
toml_decls() {
  awk -v prefix="$1" '
    function delta(l,   t, o, c) {
      t = l; o = gsub(/[\[{]/, "", t)
      t = l; c = gsub(/[\]}]/, "", t)
      return o - c
    }
    depth == 0 && /^[ \t]*\[/ {
      in_table = ($0 ~ /fandhe-frontend-/)
      if (in_table) print prefix "|" $0
      next
    }
    in_table { if ($0 !~ /^[ \t]*$/) print prefix "|" $0; depth += delta($0); next }
    depth > 0 { print prefix "|" $0; depth += delta($0); next }
    /^[ \t]*"?fandhe-frontend-/ { print prefix "|" $0; depth = delta($0) }
  ' "$2"
}

# frontend_state <rev> <out>: その rev の fandhe-frontend-* の宣言と解決済みの版を、並べ替えた行にして書く。
# git show の失敗は呼び出し側で終了状態を見る（$(...) の中の失敗を見逃さない）。
frontend_state() {
  local rev="$1" out="$2"
  git -C "${REPO}" show "${rev}:Cargo.lock" >"${TMP}/lock.txt" || return 1
  git -C "${REPO}" show "${rev}:Cargo.toml" >"${TMP}/root.toml" || return 1
  git -C "${REPO}" show "${rev}:crates/web/Cargo.toml" >"${TMP}/web.toml" || return 1
  # 各抽出は別々に実行して終了状態を見る。呼び出し側は || の文脈で set -e が効かないため、群の途中の失敗が消える。
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
  ' "${TMP}/lock.txt" >"${TMP}/state-lock.txt" || return 1
  toml_decls root "${TMP}/root.toml" >"${TMP}/state-root.txt" || return 1
  toml_decls web "${TMP}/web.toml" >"${TMP}/state-web.txt" || return 1
  cat "${TMP}/state-lock.txt" "${TMP}/state-root.txt" "${TMP}/state-web.txt" | LC_ALL=C sort >"${out}"
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

# is_changed <path>: 一覧に含まれれば 0、含まれなければ 1。grep の異常（2 以上）は「変更なし」にせず判定不能で止める。
is_changed() {
  local rc=0
  grep -qxF -- "$1" "${TMP}/changed.txt" || rc=$?
  case "${rc}" in
    0 | 1) return "${rc}" ;;
    *) fail2 "cannot check changed files (grep exit ${rc})" ;;
  esac
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

# bindgen_version <rev>: その rev の Cargo.lock の wasm-bindgen の版（build-web.sh と同じ抽出）。
bindgen_version() {
  git -C "${REPO}" show "$1:Cargo.lock" | awk '/^name = "wasm-bindgen"$/ {getline; gsub(/version = |"/, ""); print}'
}

# check_same_baseline: 基点を揃えられるかを確かめる。rustc の版（rust-toolchain.toml）と wasm-bindgen の版（Cargo.lock）が
# base と HEAD で違うと、大きさの差が frontend の版以外の理由で出る。tools/build-web.sh は CLI と lock の版の一致を求め、
# CI は 1 つの版の CLI しか持たないため、2 つの版は扱わない。別の PR で先に上げることを案内して止める（fail-closed）。
check_same_baseline() {
  local a b
  git -C "${REPO}" show "${BASE}:rust-toolchain.toml" >"${TMP}/tc-base.txt" || fail2 "cannot read rust-toolchain.toml at base ${BASE}"
  git -C "${REPO}" show "HEAD:rust-toolchain.toml" >"${TMP}/tc-head.txt" || fail2 "cannot read rust-toolchain.toml at HEAD"
  if ! cmp -s "${TMP}/tc-base.txt" "${TMP}/tc-head.txt"; then
    fail2 "(3) rust-toolchain.toml differs between base and HEAD, so the WASM size baselines differ. Bump the toolchain in a separate PR before the frontend bump."
  fi
  a="$(bindgen_version "${BASE}")" || fail2 "cannot read Cargo.lock at base ${BASE}"
  b="$(bindgen_version "HEAD")" || fail2 "cannot read Cargo.lock at HEAD"
  if [ "$(printf '%s\n' "${a}" | grep -c .)" -ne 1 ] || [ "$(printf '%s\n' "${b}" | grep -c .)" -ne 1 ]; then
    fail2 "(3) Cargo.lock must contain exactly one wasm-bindgen entry at base and at HEAD"
  fi
  if [ "${a}" != "${b}" ]; then
    fail2 "(3) wasm-bindgen differs between base (${a}) and HEAD (${b}), so the WASM size baselines differ. Bump wasm-bindgen in a separate PR before the frontend bump."
  fi
}

# build_log_line <log> <prefix>: build-web.sh のログからその接頭辞の行をちょうど 1 行取り出し、接頭辞を除いた残りを返す。
build_log_line() {
  local logf="$1" prefix="$2" n
  n="$(grep -c -F -- "[build-web.sh] ${prefix}" "${logf}" || true)"
  if [ "${n}" != "1" ]; then
    echo "error: (3) expected exactly one '${prefix}' line in the build log ${logf} (got ${n})" >&2
    return 1
  fi
  grep -F -- "[build-web.sh] ${prefix}" "${logf}" | sed -e "s/^\[build-web.sh\] ${prefix} *//"
}

# build_side <side> <rev>: rev を一時の worktree へ取り出し、release・配信の設定・wasm-opt なしでビルドしたログを TMP/build-<side>.log に残す。
# HEAD も worktree から取り出すのは、手元の作業ツリーの未コミットの変更に左右されず、両側の扱いを揃えるため。
build_side() {
  local side="$1" rev="$2" wt="${TMP}/wt-$1"
  git -C "${REPO}" worktree add --detach "${wt}" "${rev}" >"${TMP}/wt-${side}.out" 2>&1 ||
    fail2 "(3) cannot check out ${rev} for the ${side} build: $(cat "${TMP}/wt-${side}.out")"
  WORKTREES+=("${wt}")
  log "(3) building ${side} (${rev})"
  # 出力先は存在しない一時の場所にする（build-web.sh の削除の安全策をそのまま使う）。dist/ と作業ツリーは変えない。
  if ! (cd "${wt}" && TEST_HOOKS="" IR_FILE="" SKIP_WASM_OPT=1 OUT_DIR="${TMP}/out-${side}" \
    bash tools/build-web.sh release) >"${TMP}/build-${side}.log" 2>&1; then
    tail -n 30 "${TMP}/build-${side}.log" >&2
    fail2 "(3) the ${side} build failed; cannot compare WASM sizes"
  fi
}

# measured_gzip <side>: 出力された WASM を自分でも gzip -9 で測る（ログの値の取り違えの検出。tools/measure/wasm-size.sh と同じ考え方）。
measured_gzip() {
  local f="${TMP}/out-$1/wasm/fandhe_workshop_web_bg.wasm"
  [ -f "${f}" ] || fail2 "(3) built WASM not found: ${f}"
  gzip -9 -c "${f}" | wc -c | tr -d ' '
}

# parse_side <side> <log>: ログを解析し、wasm-opt の行と gzip の大きさを SIDE_OPT・SIDE_GZ に入れる。
parse_side() {
  local side="$1" logf="$2" config_line size_line
  [ -r "${logf}" ] || fail2 "cannot read ${logf}"
  config_line="$(build_log_line "${logf}" "config:")" || fail2 "(3) cannot parse the ${side} build log"
  SIDE_OPT="$(build_log_line "${logf}" "wasm-opt:")" || fail2 "(3) cannot parse the ${side} build log"
  size_line="$(build_log_line "${logf}" "wasm size:")" || fail2 "(3) cannot parse the ${side} build log"
  [ "${config_line}" = "delivery / profile: release" ] ||
    fail2 "(3) the ${side} build is not the delivery release configuration: ${config_line}"
  case "${size_line}" in
    "raw "*" bytes, gzip -9 "*" bytes") ;;
    *) fail2 "(3) unexpected wasm size line in the ${side} build log" ;;
  esac
  SIDE_GZ="${size_line#*gzip -9 }"
  SIDE_GZ="${SIDE_GZ% bytes}"
  case "${SIDE_GZ}" in
    '' | *[!0-9]*) fail2 "(3) the gzip size in the ${side} build log is not a number" ;;
  esac
}

# cmd_wasm_gzip: (3) 配信用 WASM の gzip -9 の増加が +10% 以内か。判定は整数だけで行う（head*100 <= base*110）。
cmd_wasm_gzip() {
  local bump base_sha head_sha base_gz head_gz base_opt head_opt injected="false" delta pm
  if ! is_pr && [ "${ASSUME_BUMP}" != "true" ]; then
    log "not a pull_request event; skipping WASM gzip growth check"
    return 0
  fi
  if [ "${ASSUME_BUMP}" = "true" ]; then
    bump=true
    git -C "${REPO}" rev-parse --verify --quiet "${BASE}^{commit}" >/dev/null ||
      fail2 "base revision not found: ${BASE} (checkout needs fetch-depth: 2)"
  else
    bump="$(detect_bump)"
  fi
  if [ "${bump}" != "true" ]; then
    log "not a frontend version bump; WASM gzip growth check not applicable"
    return 0
  fi
  # 片方だけの注入は誤用（半端な入力で測定を省かない）
  if [ -n "${BASE_BUILD_LOG}" ] || [ -n "${HEAD_BUILD_LOG}" ]; then
    if [ -z "${BASE_BUILD_LOG}" ] || [ -z "${HEAD_BUILD_LOG}" ]; then
      echo "error: --base-build-log and --head-build-log must be given together" >&2
      exit 2
    fi
    injected="true"
  fi
  check_same_baseline
  base_sha="$(git -C "${REPO}" rev-parse --short "${BASE}^{commit}")" || fail2 "cannot resolve ${BASE}"
  head_sha="$(git -C "${REPO}" rev-parse --short HEAD)" || fail2 "cannot resolve HEAD"
  if [ "${injected}" = "true" ]; then
    parse_side base "${BASE_BUILD_LOG}"
    base_gz="${SIDE_GZ}"
    base_opt="${SIDE_OPT}"
    parse_side head "${HEAD_BUILD_LOG}"
    head_gz="${SIDE_GZ}"
    head_opt="${SIDE_OPT}"
  else
    require_cmd gzip
    require_cmd wc
    build_side base "${BASE}"
    build_side head HEAD
    parse_side base "${TMP}/build-base.log"
    base_gz="${SIDE_GZ}"
    base_opt="${SIDE_OPT}"
    [ "$(measured_gzip base)" = "${base_gz}" ] || fail2 "(3) the base gzip size in the build log differs from the measured one"
    parse_side head "${TMP}/build-head.log"
    head_gz="${SIDE_GZ}"
    head_opt="${SIDE_OPT}"
    [ "$(measured_gzip head)" = "${head_gz}" ] || fail2 "(3) the head gzip size in the build log differs from the measured one"
  fi
  [ "${base_opt}" = "${head_opt}" ] ||
    fail2 "(3) wasm-opt status differs between base (${base_opt}) and head (${head_opt}); the baselines differ"
  [ "${base_gz}" -gt 0 ] || fail2 "(3) the base gzip size is not positive: ${base_gz}"
  [ "${head_gz}" -gt 0 ] || fail2 "(3) the head gzip size is not positive: ${head_gz}"
  delta=$((head_gz - base_gz))
  # 増加率は千分率の整数（0 に向けて切り捨て）。表示だけに使い、判定には使わない。
  pm=$((delta * 1000 / base_gz))
  log "(3) base ${base_sha}: gzip -9 ${base_gz} bytes / head ${head_sha}: gzip -9 ${head_gz} bytes / diff ${delta} bytes (${pm} per mille; limit 100) / wasm-opt: ${base_opt}"
  if [ $((head_gz * 100)) -le $((base_gz * 110)) ]; then
    log "(3) ok: WASM gzip growth is within +10%"
    return 0
  fi
  echo "error: (3) WASM gzip -9 grew from ${base_gz} to ${head_gz} bytes (${pm} per mille), more than +10% (FEND-17 / D-63). Find the cause of the growth in the bump." >&2
  return 1
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
  wasm-gzip)
    [ "${#ARGS[@]}" -eq 1 ] || {
      usage
      exit 2
    }
    cmd_wasm_gzip
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
