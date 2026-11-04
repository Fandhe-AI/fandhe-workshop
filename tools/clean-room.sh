#!/usr/bin/env bash
# まっさらなコンテナで cargo test を流し、第三者が公開の依存だけで試験を通せることを確かめる（BUILD-1。TASK-77 1/2）。
# あわせて、第 2 教材（template-workshop/examples/review-request/）のビルドと教材テストが同じ場所で通ることを確かめる
# （SECOND-18・R-32。TASK-754。D-91 により第 2 教材はエンジン repo の側に置くので、ワークスペースの中で通す）。
#
# 呼び出し元: make clean-room（人が手で流す。遅く、docker と通信が要るため PR の CI・フックには入れない）と、
# .github/workflows/clean-room.yml（週 1 回の定期実行と手動実行。#2040）。
# 回帰テスト: scripts/test-clean-room.sh（make test-clean-room。docker なしで judge だけを合成のログに当てる）。
#
# 使い方:
#   tools/clean-room.sh [run] [--out-dir <dir>] [--ref <rev>] [--only pack]
#   tools/clean-room.sh judge <dir> [--mode all|pack]
#
#   run（既定）: git archive <rev>（既定 HEAD）の tar だけを、認証情報・ホームの設定・環境変数・bind mount を持たない
#     公開イメージのコンテナへ stdin で渡し、`cargo test --locked`（既定の contract・core・pack）と
#     `cargo test --locked --workspace`（web の native 試験を含む）を順に流して判定する。
#     未コミットの変更・.git・_/・target/・dist/・node_modules・.env・docs/spec（private の submodule）の中身は
#     コンテナに入らない。つまり「コミットした内容だけで通る」ことを確かめる（spec が無くても通ることも同時に確かめる）。
#     前提の検査には、兄弟の 03-poc/・prototype/ が無いこと（BUILD-14。TASK-89）も含む。
#     第 2 教材の区間（pack）は、fandhe-pack のビルド（build）・`validate`（診断 0 件）・`compile` の出力と基準の IR
#     （tests/ir.json）のバイト一致（ir-match）・`test`（全件合格）の 4 段を流す。
#   --only pack: 第 2 教材の区間だけを流す（既定は default・workspace・pack の 3 区間。3 区間は前景の 10 分を超えうるため分けられる）。
#   judge <dir>: run が保存した <dir>/run.log を判定だけする（ログの再判定と回帰テスト用）。
#     --mode は期待する区間の組（既定 all。meta.txt があるときは meta.txt の mode を使い、mode が無い・未知なら判定不能）。
#   --out-dir: ログの置き場（存在しないか空であること。既定は mktemp -d。パスは標準エラーに出す）。
#
# 環境変数: CONTAINER_RUNTIME（docker か podman。既定 docker。それ以外は誤用）。
#
# 不変条件（件数は固定せず docs/testing.md の記録に置く。試験は変更のたびに増えるため）:
#   1. どちらの実行も cargo の終了コードが 0 で、failed が 0 件
#   2. 無視された試験は ALLOWED_IGNORED にあるものだけ
#   3. コンテナのランタイムの終了コード（meta.txt の container_exit）が 0（非 0 は判定不能として 2）
#   4. pack 区間の 4 段の終了コードがすべて 0 で、validate が診断 0 件、test に FAIL の行が無く模範解答・誤入力が全件合格
#      （正規表現は scripts/check-second-pack.sh の test の項目と同じ）
#
# pack 区間が jq を使わない理由と分担: 公開イメージには jq・node が無く、追加導入（apt-get）は「tar だけを渡す」前提を崩すため行わない。
#   scripts/check-second-pack.sh（jq 必須。設定値・行の変化・誤答の規則などの全項目）は従来どおりホストで流す（docs/testing.md 5.448）。
#   コンテナが確かめるのは、公開の依存だけでのビルドと、教材テスト・IR の一致の 4 段である。
#
# ALLOWED_IGNORED と spec の文言のずれ: spec（PoC-11・BUILD-1・TASK-77）の許容は `measure_*`・`generate_histories` だが、
#   これは試作の時点の名前で本 repo に存在しない。現物の #[ignore] は、ブラウザ試験の入力を書き出す次の 2 件だけで、
#   scripts/test-browser.sh だけが --ignored で呼ぶ。存在しない名前を許すと検査が空回りするため、現物の名前で完全一致にする。
#     crates/core/tests/version_histories.rs の emit_histories_for_browser（spec の generate_histories に当たる）
#     crates/web/tests/round_begin_host.rs の emit_round_handoff_for_browser
#
# 終了コード: 0 = 両方の実行が不変条件を満たす
#   1 = 試験の失敗・許容外の無視・まっさらさの前提の崩れ・cargo の失敗
#   2 = 誤用、または実行を完了できない・判定できない（docker が無い・イメージを取得できない・ログが切れている・
#       解析の取りこぼし）。2 も合格にしない（fail-closed）。
#
# 安全の方針: コンテナへ渡すのは tar（stdin）だけ。-e・--env-file・-v・docker ソケット・--privileged は使わない。
#   コンテナの出力は awk・grep で解析するだけで、コマンドとしては実行しない。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=../scripts/lib.sh
source "${DIR}/../scripts/lib.sh"

# イメージは manifest list の digest で固定する（2026-10-28 に docker buildx imagetools inspect で確認）。
# タグは rust-toolchain.toml の channel と一致させる（食い違えば run は 2 で止まる）。
readonly IMAGE_VERSION="1.98.1"
readonly IMAGE_DIGEST="sha256:a8a5f0a1e5fe7dfe1d352591e4a1c7dd2c08fd70475cae872cf3458ba0df0546"
readonly IMAGE="docker.io/library/rust:${IMAGE_VERSION}@${IMAGE_DIGEST}"
readonly ALLOWED_IGNORED=(emit_histories_for_browser emit_round_handoff_for_browser)
readonly M="### clean-room:"

usage() {
  echo "usage: $(basename "$0") [run] [--out-dir <dir>] [--ref <rev>] [--only pack]" >&2
  echo "       $(basename "$0") judge <dir> [--mode all|pack]" >&2
}

# コンテナの中で実行する固定のプログラム。外部の文字列から組み立てず、eval もしない。
# 区切りの行（### clean-room: ...）を標準出力に出し、ホストの judge がそれで実行を切り分ける。
read -r -d '' INNER <<'EOF' || true
exec 2>&1
M="### clean-room:"
pre_fail() { echo "${M} precondition fail $1"; exit 1; }
# 実行の範囲。ホスト側でも許可リストで検査済みだが、コンテナ側でも再確認する（eval しない）。
MODE="${1:-}"
case "${MODE}" in all | pack) ;; *) pre_fail "bad-mode" ;; esac
PACK=template-workshop/examples/review-request
mkdir /w
tar -x -C /w
for d in .cargo .ssh .config; do
  if [ -e "${HOME}/${d}" ]; then pre_fail "home-has-${d}"; fi
done
n="$(env | awk -F= 'tolower($1) ~ /token|secret|key|pass|cred|auth/ { c++ } END { print c + 0 }')"
if [ "${n}" != "0" ]; then pre_fail "secret-like-env-${n}"; fi
if [ -e /w/.git ]; then pre_fail "git-dir-present"; fi
if [ -n "$(ls -A /w/docs/spec 2>/dev/null)" ]; then pre_fail "spec-present"; fi
# BUILD-14（TASK-89）: 兄弟の 03-poc/・prototype/ が無いこと（/w の親に何も置かれていない）を前提として明示する。
for s in 03-poc prototype; do
  if [ -e "/${s}" ] || [ -L "/${s}" ]; then pre_fail "sibling-present-${s}"; fi
done
if [ ! -f "/w/${PACK}/workshop.yaml" ]; then pre_fail "second-pack-missing"; fi
echo "${M} precondition ok"
cd /w
export CARGO_TARGET_DIR=/tmp/target
rustup toolchain install || pre_fail "toolchain-install"
bash scripts/ci-rust-toolchain.sh || pre_fail "toolchain-mismatch"
echo "${M} rustc $(rustc --version)"
run_cargo() {
  name="$1"; shift
  echo "${M} begin ${name}"
  s="$(date +%s)"
  set +e
  cargo test --locked "$@" --no-fail-fast 2>&1
  rc=$?
  set -e
  echo
  echo "${M} seconds ${name} $(( $(date +%s) - s ))"
  echo "${M} end ${name} exit=${rc}"
}
# 第 2 教材（SECOND-18・TASK-754）。各段は失敗しても次へ進み、段ごとの終了コードを区切りの行に残す。
pack_step() {
  echo "${M} pack ${1} exit=${2}"
  if [ "${2}" -gt "${worst}" ]; then worst="${2}"; fi
}
run_pack() {
  echo "${M} begin pack"
  s="$(date +%s)"
  worst=0
  bin="${CARGO_TARGET_DIR}/debug/fandhe-pack"
  set +e
  echo "${M} pack-begin build"
  cargo build --locked -p fandhe-workshop-pack --bin fandhe-pack 2>&1
  rc=$?
  if [ "${rc}" = "0" ] && [ ! -x "${bin}" ]; then echo "fandhe-pack was not built"; rc=1; fi
  pack_step build "${rc}"
  echo "${M} pack-begin validate"
  "${bin}" validate "${PACK}" 2>&1
  pack_step validate "$?"
  echo "${M} pack-begin ir-match"
  "${bin}" compile "${PACK}" -o /tmp/second-pack-ir.json 2>&1
  rc=$?
  cmp -s "${PACK}/tests/ir.json" /tmp/second-pack-ir.json
  rc2=$?
  if [ "${rc2}" != "0" ]; then echo "compiled IR differs from ${PACK}/tests/ir.json"; fi
  if [ "${rc2}" -gt "${rc}" ]; then rc="${rc2}"; fi
  pack_step ir-match "${rc}"
  echo "${M} pack-begin test"
  "${bin}" test "${PACK}" 2>&1
  pack_step test "$?"
  set -e
  echo
  echo "${M} seconds pack $(( $(date +%s) - s ))"
  echo "${M} end pack exit=${worst}"
}
if [ "${MODE}" = "all" ]; then
  run_cargo default
  run_cargo workspace --workspace
fi
run_pack
EOF

# section <log> <name>: 区切りの間の行だけを出す。
section() {
  awk -v b="${M} begin $2" -v e="${M} end $2 exit=" '
    $0 == b { on = 1; next }
    index($0, e) == 1 { on = 0 }
    on
  ' "$1"
}

# judge_run <log> <name>: 1 回の実行を判定して要約を出す。戻り値は 0 / 1 / 2（本ファイル冒頭の終了コード）。
judge_run() {
  local log="$1" name="$2" begins ends rc results sec
  begins="$(grep -c -x -- "${M} begin ${name}" "${log}" || true)"
  ends="$(grep -c -E -- "^${M} end ${name} exit=[0-9]+\$" "${log}" || true)"
  if [ "${begins}" != "1" ] || [ "${ends}" != "1" ]; then
    echo "[${name}] error: log is incomplete (begin=${begins} end=${ends})"
    return 2
  fi
  rc="$(grep -E -- "^${M} end ${name} exit=[0-9]+\$" "${log}" | sed 's/.*exit=//')"
  sec="$(grep -E -- "^${M} seconds ${name} [0-9]+\$" "${log}" | awk '{ print $NF }' | head -n 1 || true)"
  results="$(section "${log}" "${name}" | grep -c -E '^test result: ' || true)"
  local sums
  sums="$(section "${log}" "${name}" | awk '
    /^test result: / {
      hp = 0; hf = 0; hg = 0
      for (i = 2; i <= NF; i++) {
        if ($i == "passed;" || $i == "failed;" || $i == "ignored;") {
          if ($(i - 1) !~ /^[0-9]+$/) { bad++; continue }
          if ($i == "passed;") { p += $(i - 1); hp++ }
          if ($i == "failed;") { f += $(i - 1); hf++ }
          if ($i == "ignored;") { g += $(i - 1); hg++ }
        }
      }
      if (hp != 1 || hf != 1 || hg != 1) bad++
    }
    END { print p + 0, f + 0, g + 0, bad + 0 }')"
  local passed failed ignored
  passed="${sums%% *}"
  failed="${sums#* }"
  failed="${failed%% *}"
  ignored="${sums#* }"
  ignored="${ignored#* }"
  ignored="${ignored%% *}"
  local malformed="${sums##* }"
  local names count
  names="$(section "${log}" "${name}" | grep -E '^test .* \.\.\. ignored' \
    | sed -e 's/^test //' -e 's/ \.\.\. ignored.*$//' -e 's/.*:://' || true)"
  count=0
  if [ -n "${names}" ]; then count="$(printf '%s\n' "${names}" | wc -l | tr -d ' ')"; fi
  echo "[${name}] cargo exit=${rc} result-lines=${results} passed=${passed} failed=${failed} ignored=${ignored} seconds=${sec:-unknown}"
  if [ -n "${names}" ]; then printf '%s\n' "${names}" | sed "s/^/[${name}] ignored: /"; fi
  if [ "${rc}" != "0" ]; then
    echo "[${name}] error: cargo exited with ${rc}"
    return 1
  fi
  if [ "${results}" = "0" ]; then
    echo "[${name}] error: no 'test result:' line (cannot judge)"
    return 2
  fi
  if [ "${malformed}" != "0" ]; then
    echo "[${name}] error: ${malformed} unparsable 'test result:' line(s) (cannot judge)"
    return 2
  fi
  if [ "${failed}" != "0" ]; then
    echo "[${name}] error: ${failed} test(s) failed"
    return 1
  fi
  if [ "${ignored}" != "${count}" ]; then
    echo "[${name}] error: ignored total ${ignored} != ignored lines ${count} (parse gap)"
    return 2
  fi
  local n bad=0 ok
  while IFS= read -r n; do
    [ -n "${n}" ] || continue
    ok=0
    local a
    for a in "${ALLOWED_IGNORED[@]}"; do
      if [ "${n}" = "${a}" ]; then ok=1; fi
    done
    if [ "${ok}" = "0" ]; then
      echo "[${name}] error: ignored test not in allow list: ${n}"
      bad=1
    fi
  done <<EOT
${names}
EOT
  if [ "${bad}" != "0" ]; then return 1; fi
  echo "[${name}] ok"
  return 0
}

# pack_step_out <log> <step>: pack 区間の 1 段の出力だけを出す。
pack_step_out() {
  awk -v b="${M} pack-begin $2" -v e="${M} pack $2 exit=" '
    $0 == b { on = 1; next }
    index($0, e) == 1 { on = 0 }
    on
  ' "$1"
}

# judge_pack <log>: 第 2 教材の区間（pack）を判定して要約を出す。戻り値は 0 / 1 / 2。
judge_pack() {
  local log="$1" begins ends sec step n b rc bad=0 out
  begins="$(grep -c -x -- "${M} begin pack" "${log}" || true)"
  ends="$(grep -c -E -- "^${M} end pack exit=[0-9]+\$" "${log}" || true)"
  if [ "${begins}" != "1" ] || [ "${ends}" != "1" ]; then
    echo "[pack] error: log is incomplete (begin=${begins} end=${ends})"
    return 2
  fi
  for step in build validate ir-match test; do
    n="$(grep -c -E -- "^${M} pack ${step} exit=[0-9]+\$" "${log}" || true)"
    b="$(grep -c -x -- "${M} pack-begin ${step}" "${log}" || true)"
    if [ "${n}" != "1" ] || [ "${b}" != "1" ]; then
      echo "[pack] error: step ${step} markers are incomplete (begin=${b} end=${n})"
      return 2
    fi
  done
  local summary="" codes
  for step in build validate ir-match test; do
    rc="$(grep -E -- "^${M} pack ${step} exit=[0-9]+\$" "${log}" | sed 's/.*exit=//')"
    summary="${summary}${step}=${rc} "
    if [ "${rc}" != "0" ]; then
      echo "[pack] error: step ${step} exited with ${rc}"
      bad=1
    fi
  done
  sec="$(grep -E -- "^${M} seconds pack [0-9]+\$" "${log}" | awk '{ print $NF }' | head -n 1 || true)"
  codes="${summary}"
  out="$(pack_step_out "${log}" validate)"
  if ! printf '%s\n' "${out}" | grep -q -E '^OK: 診断 0 件（'; then
    echo "[pack] error: validate did not report 'OK: 診断 0 件'"
    bad=1
  fi
  out="$(pack_step_out "${log}" test)"
  if printf '%s\n' "${out}" | grep -q '^FAIL '; then
    echo "[pack] error: test reported FAIL line(s)"
    bad=1
  fi
  local model errin
  model="$(printf '%s\n' "${out}" | grep -E '^模範解答: ([1-9][0-9]*)/\1 完走' | head -n 1 | sed -E 's/^模範解答: ([0-9]+\/[0-9]+).*/\1/' || true)"
  errin="$(printf '%s\n' "${out}" | grep -E '^誤入力: ([0-9]+)/\1 が期待どおり' | head -n 1 | sed -E 's/^誤入力: ([0-9]+\/[0-9]+).*/\1/' || true)"
  if [ -z "${model}" ]; then
    echo "[pack] error: no all-complete 模範解答 line"
    bad=1
  fi
  if [ -z "${errin}" ]; then
    echo "[pack] error: no all-expected 誤入力 line"
    bad=1
  fi
  echo "[pack] ${codes}model=${model:-none} error-inputs=${errin:-none} seconds=${sec:-unknown}"
  if [ "${bad}" != "0" ]; then return 1; fi
  echo "[pack] ok"
  return 0
}

# judge_dir <dir> [<mode>]: <dir>/run.log を判定する。戻り値は最も重い終了コード。
judge_dir() {
  local log="$1/run.log" worst=0 code pre_ok pre_fail mode="${2:-all}"
  if [ ! -r "${log}" ]; then
    echo "error: cannot read ${log}" >&2
    return 2
  fi
  pre_ok="$(grep -c -x -- "${M} precondition ok" "${log}" || true)"
  pre_fail="$(grep -E -- "^${M} precondition fail " "${log}" | head -n 1 | sed "s/^${M} precondition fail //" || true)"
  if [ -n "${pre_fail}" ]; then
    echo "[precondition] error: clean-room precondition failed: ${pre_fail}"
    return 1
  fi
  # run が meta.txt に残したランタイムの終了コード。ログが条件を満たしてもコンテナの異常終了・終了処理の失敗は成功にしない。
  # meta.txt が無い（ログだけの再判定）ときは照合しない。
  local meta="$1/meta.txt" drc=""
  if [ -r "${meta}" ]; then
    drc="$(sed -n 's/^container_exit=//p' "${meta}" | head -n 1)"
  fi
  if [ -r "${meta}" ] && ! printf '%s' "${drc}" | grep -q -E '^[0-9]+$'; then
    echo "[container] error: no valid container_exit in ${meta} (cannot judge)"
    return 2
  fi
  # meta.txt の mode は run が決めた範囲で、引数の --mode より優先する。meta.txt があるのに mode が無い・未知なら判定不能（区間の取り違えを合格にしない）。
  if [ -r "${meta}" ]; then
    local mmode
    mmode="$(sed -n 's/^mode=//p' "${meta}" | head -n 1)"
    case "${mmode}" in
      all | pack) ;;
      *)
        echo "[container] error: no valid mode in ${meta} (cannot judge)"
        return 2
        ;;
    esac
    mode="${mmode}"
  fi
  if [ -n "${drc}" ] && [ "${drc}" != "0" ]; then
    echo "[container] error: container runtime exited with ${drc}"
    return 2
  fi
  if [ "${pre_ok}" != "1" ]; then
    echo "[precondition] error: no 'precondition ok' line (the container did not start or the log is cut)"
    return 2
  fi
  grep -E -- "^${M} rustc " "${log}" | head -n 1 | sed "s/^${M} //" || true
  local name
  if [ "${mode}" = "all" ]; then
    for name in default workspace; do
      code=0
      judge_run "${log}" "${name}" || code=$?
      if [ "${code}" -gt "${worst}" ]; then worst="${code}"; fi
    done
  fi
  code=0
  judge_pack "${log}" || code=$?
  if [ "${code}" -gt "${worst}" ]; then worst="${code}"; fi
  return "${worst}"
}

runtime_cmd() {
  case "${CONTAINER_RUNTIME:-docker}" in
    docker | podman) echo "${CONTAINER_RUNTIME:-docker}" ;;
    *)
      echo "error: CONTAINER_RUNTIME must be docker or podman" >&2
      return 2
      ;;
  esac
}

do_run() {
  local out_dir="" ref="HEAD" rt mode="all" only_seen=0
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --out-dir)
        [ "$#" -ge 2 ] || { usage; exit 2; }
        out_dir="$2"
        shift 2
        ;;
      --ref)
        [ "$#" -ge 2 ] || { usage; exit 2; }
        ref="$2"
        shift 2
        ;;
      --only)
        [ "$#" -ge 2 ] || { usage; exit 2; }
        [ "${only_seen}" = "0" ] || { usage; exit 2; }
        only_seen=1
        case "$2" in
          pack) mode="pack" ;;
          *) usage; exit 2 ;;
        esac
        shift 2
        ;;
      *)
        usage
        exit 2
        ;;
    esac
  done
  rt="$(runtime_cmd)" || exit 2
  require_cmd git || exit 2
  require_cmd "${rt}" || exit 2
  require_cmd awk || exit 2
  require_cmd grep || exit 2
  local -a sha_cmd
  if command -v sha256sum >/dev/null 2>&1; then
    sha_cmd=(sha256sum)
  elif command -v shasum >/dev/null 2>&1; then
    sha_cmd=(shasum -a 256)
  else
    echo "error: sha256sum or shasum is required" >&2
    exit 2
  fi

  local channel
  channel="$(sed -n 's/^channel *= *"\(.*\)"$/\1/p' "${ROOT_DIR}/rust-toolchain.toml" | head -n 1)"
  if [ "${channel}" != "${IMAGE_VERSION}" ]; then
    echo "error: rust-toolchain.toml channel '${channel}' != image version '${IMAGE_VERSION}' (update IMAGE_VERSION and IMAGE_DIGEST)" >&2
    exit 2
  fi

  case "${ref}" in
    -*)
      echo "error: --ref must not start with '-'" >&2
      exit 2
      ;;
  esac
  local commit
  commit="$(git -C "${ROOT_DIR}" rev-parse --verify "${ref}^{commit}")" || {
    echo "error: cannot resolve --ref ${ref}" >&2
    exit 2
  }

  if [ -z "${out_dir}" ]; then
    out_dir="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-clean-room.XXXXXX")"
  else
    mkdir -p "${out_dir}"
    if [ -n "$(ls -A "${out_dir}")" ]; then
      echo "error: --out-dir must be empty: ${out_dir}" >&2
      exit 2
    fi
  fi
  echo "log directory: ${out_dir}" >&2

  git -C "${ROOT_DIR}" archive --format=tar "${commit}" >"${out_dir}/source.tar" || {
    echo "error: git archive failed" >&2
    exit 2
  }
  local tar_sha
  tar_sha="$("${sha_cmd[@]}" "${out_dir}/source.tar" | awk '{ print $1 }')"
  {
    echo "commit=${commit}"
    echo "tar_sha256=${tar_sha}"
    echo "image=${IMAGE}"
    echo "host_arch=$(uname -m)"
    echo "mode=${mode}"
  } >"${out_dir}/meta.txt"

  local started="${SECONDS}" drc=0
  "${rt}" run --rm -i --pull=missing "${IMAGE}" bash -euo pipefail -c "${INNER}" bash "${mode}" \
    <"${out_dir}/source.tar" >"${out_dir}/run.log" 2>&1 || drc=$?
  echo "container_exit=${drc}" >>"${out_dir}/meta.txt"
  echo "wall_seconds=$((SECONDS - started))" >>"${out_dir}/meta.txt"

  local name
  for name in default workspace pack; do
    section "${out_dir}/run.log" "${name}" >"${out_dir}/${name}.log" || true
  done
  "${rt}" image inspect --format '{{index .RepoDigests 0}}' "${IMAGE}" >>"${out_dir}/meta.txt" 2>/dev/null || true
  cat "${out_dir}/meta.txt"
  local code=0
  judge_dir "${out_dir}" || code=$?
  exit "${code}"
}

main() {
  local sub="run"
  if [ "$#" -gt 0 ]; then
    case "$1" in
      run)
        shift
        ;;
      judge)
        sub="judge"
        shift
        ;;
    esac
  fi
  if [ "${sub}" = "judge" ]; then
    local jmode="all"
    if [ "$#" -eq 3 ] && [ "$2" = "--mode" ]; then
      case "$3" in
        all | pack) jmode="$3" ;;
        *) usage; exit 2 ;;
      esac
    elif [ "$#" -ne 1 ]; then
      usage
      exit 2
    fi
    local code=0
    judge_dir "$1" "${jmode}" || code=$?
    exit "${code}"
  fi
  do_run "$@"
}

main "$@"
