#!/usr/bin/env bash
# まっさらなコンテナで cargo test を流し、第三者が公開の依存だけで試験を通せることを確かめる（BUILD-1。TASK-77 1/2）。
#
# 呼び出し元: make clean-room（人が手で流す。遅く、docker と通信が要るため CI・フックには入れない）。
# 回帰テスト: scripts/test-clean-room.sh（make test-clean-room。docker なしで judge だけを合成のログに当てる）。
#
# 使い方:
#   tools/clean-room.sh [run] [--out-dir <dir>] [--ref <rev>]
#   tools/clean-room.sh judge <dir>
#
#   run（既定）: git archive <rev>（既定 HEAD）の tar だけを、認証情報・ホームの設定・環境変数・bind mount を持たない
#     公開イメージのコンテナへ stdin で渡し、`cargo test --locked`（既定の contract・core・pack）と
#     `cargo test --locked --workspace`（web の native 試験を含む）を順に流して判定する。
#     未コミットの変更・.git・_/・target/・dist/・node_modules・.env・docs/spec（private の submodule）の中身は
#     コンテナに入らない。つまり「コミットした内容だけで通る」ことを確かめる（spec が無くても通ることも同時に確かめる）。
#   judge <dir>: run が保存した <dir>/run.log を判定だけする（ログの再判定と回帰テスト用）。
#   --out-dir: ログの置き場（存在しないか空であること。既定は mktemp -d。パスは標準エラーに出す）。
#
# 環境変数: CONTAINER_RUNTIME（docker か podman。既定 docker。それ以外は誤用）。
#
# 不変条件（件数は固定せず docs/testing.md の記録に置く。試験は変更のたびに増えるため）:
#   1. どちらの実行も cargo の終了コードが 0 で、failed が 0 件
#   2. 無視された試験は ALLOWED_IGNORED にあるものだけ
#   3. コンテナのランタイムの終了コード（meta.txt の container_exit）が 0（非 0 は判定不能として 2）
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
  echo "usage: $(basename "$0") [run] [--out-dir <dir>] [--ref <rev>]" >&2
  echo "       $(basename "$0") judge <dir>" >&2
}

# コンテナの中で実行する固定のプログラム。外部の文字列から組み立てず、eval もしない。
# 区切りの行（### clean-room: ...）を標準出力に出し、ホストの judge がそれで実行を切り分ける。
read -r -d '' INNER <<'EOF' || true
exec 2>&1
M="### clean-room:"
pre_fail() { echo "${M} precondition fail $1"; exit 1; }
mkdir /w
tar -x -C /w
for d in .cargo .ssh .config; do
  if [ -e "${HOME}/${d}" ]; then pre_fail "home-has-${d}"; fi
done
n="$(env | awk -F= 'tolower($1) ~ /token|secret|key|pass|cred|auth/ { c++ } END { print c + 0 }')"
if [ "${n}" != "0" ]; then pre_fail "secret-like-env-${n}"; fi
if [ -e /w/.git ]; then pre_fail "git-dir-present"; fi
if [ -n "$(ls -A /w/docs/spec 2>/dev/null)" ]; then pre_fail "spec-present"; fi
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
run_cargo default
run_cargo workspace --workspace
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
      for (i = 2; i <= NF; i++) {
        if ($i == "passed;") p += $(i - 1)
        if ($i == "failed;") f += $(i - 1)
        if ($i == "ignored;") g += $(i - 1)
      }
    }
    END { print p + 0, f + 0, g + 0 }')"
  local passed failed ignored
  passed="${sums%% *}"
  failed="${sums#* }"
  failed="${failed%% *}"
  ignored="${sums##* }"
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

# judge_dir <dir>: <dir>/run.log を判定する。戻り値は最も重い終了コード。
judge_dir() {
  local log="$1/run.log" worst=0 code pre_ok pre_fail
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
  for name in default workspace; do
    code=0
    judge_run "${log}" "${name}" || code=$?
    if [ "${code}" -gt "${worst}" ]; then worst="${code}"; fi
  done
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
  local out_dir="" ref="HEAD" rt
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
  } >"${out_dir}/meta.txt"

  local started="${SECONDS}" drc=0
  "${rt}" run --rm -i --pull=missing "${IMAGE}" bash -euo pipefail -c "${INNER}" \
    <"${out_dir}/source.tar" >"${out_dir}/run.log" 2>&1 || drc=$?
  echo "container_exit=${drc}" >>"${out_dir}/meta.txt"
  echo "wall_seconds=$((SECONDS - started))" >>"${out_dir}/meta.txt"

  local name
  for name in default workspace; do
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
    if [ "$#" -ne 1 ]; then
      usage
      exit 2
    fi
    local code=0
    judge_dir "$1" || code=$?
    exit "${code}"
  fi
  do_run "$@"
}

main "$@"
