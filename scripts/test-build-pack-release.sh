#!/usr/bin/env bash
# scripts/build-pack-release.sh 自体の回帰テスト（`make test-build-pack-release` と ci.yml の check ジョブから呼ばれる。TASK-415 1/2）。
#
# ビルドはしない（速く保つ）。--bin に偽の実行ファイル（validate・test に 0 か 1 を返すシェルスクリプト）を渡し、
# file の偽物を PATH の先頭に置いて Mach-O arm64 などのふりをさせる。実ビルドの確認は release.yml の run と手元で行う。
# 依存のライセンス文（THIRD-PARTY-LICENSES。LIC-9）は、合成の cargo metadata（--metadata-dir）で確かめる。
# 確かめること: 梱包の中身と mode・sha256 の照合、THIRD-PARTY-LICENSES の中身・決定性・許容外の license などでの失敗（1）、アーキテクチャ違い・煙試験の失敗・Rust が見える PATH の失敗（1）、
# 誤用（2）、失敗のときに成果物が出ないこと（fail-closed）。
# 作業ツリーは変えない。ネットワークは使わない。
#
# 出力は TAP 風（scripts/test-check-fork-ci.sh に合わせる）。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
for c in mktemp mkdir rm tar file jq cargo; do
  require_cmd "${c}" || exit 1
done

BUILD="${DIR}/build-pack-release.sh"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

passed=0
failed=0

expect() {
  if [ "$2" = "$3" ]; then
    echo "ok - $1"
    passed=$((passed + 1))
  else
    echo "not ok - $1 (expected '$2', got '$3')"
    failed=$((failed + 1))
  fi
}

# 偽の実行ファイル。自分と同じ場所に fail-<サブコマンド> があるときだけ 1 を返す
# （煙試験は env -i で環境変数を消すため、失敗の指定は環境変数では渡せない）
FAKE_BIN="${TMP_DIR}/fandhe-pack"
cat >"${FAKE_BIN}" <<'EOF'
#!/bin/sh
[ ! -e "$(dirname "$0")/fail-$1" ] || exit 1
case "$1" in
  preview)
    # 偽の配信。URL の行を出して待つ（煙試験が止める）。no-url があるときは URL を出さずに終わる
    [ ! -e "$(dirname "$0")/no-url" ] || exit 0
    echo "OK: fake"
    echo "URL: http://127.0.0.1:1/preview/"
    exec sleep 30
    ;;
  compile) echo '{}' ;;
esac
exit 0
EOF
chmod 0755 "${FAKE_BIN}"

# file の偽物。実際の file は PATH の後ろにあっても、-b の出力だけ FAKE_FILE_DESC に置き換える
SHIM="${TMP_DIR}/shim"
mkdir -p "${SHIM}"
cat >"${SHIM}/file" <<'EOF'
#!/bin/sh
echo "${FAKE_FILE_DESC}"
EOF
chmod +x "${SHIM}/file"

# curl の偽物。URL の末尾で本文と content-type を決める。SHIM の ir-differs があるときは IR だけ違う本文を返す
cat >"${SHIM}/curl" <<'EOF'
#!/bin/sh
out=""
url=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) out="$2"; shift ;;
    -w | --noproxy | --max-time) shift ;;
    -*) ;;
    *) url="$1" ;;
  esac
  shift
done
case "${url}" in
  *.wasm) body="w"; ct="application/wasm" ;;
  *ir/main.json)
    ct="application/json"
    if [ -e "$(dirname "$0")/ir-differs" ]; then body="{x}"; else body="{}"; fi
    ;;
  *) body="<html>"; ct="text/html; charset=utf-8" ;;
esac
printf '%s\n' "${body}" >"${out}"
printf '%s' "${ct}"
EOF
chmod +x "${SHIM}/curl"

MACHO="Mach-O 64-bit executable arm64"
OUT="${TMP_DIR}/out"

# 合成の cargo metadata。mk_meta <dir> <m の license> <m の source> <m のファイル: file|symlink|none>
# native: pack -> core(ws) -> f / a / pm(proc-macro) -> viapm / bld(build) / dev(dev) / m / nofile / sub / slash
# wasm:   web -> a / w
REG_SRC="registry+https://github.com/rust-lang/crates.io-index"
mk_meta() {
  local d="$1" mlic="$2" msrc="$3" mfiles="$4" c
  mkdir -p "${d}/reg"
  for c in a pm viapm bld dev f m nofile sub slash w; do
    mkdir -p "${d}/reg/${c}"
  done
  printf 'MIT text of a\n' >"${d}/reg/a/LICENSE-MIT"
  printf 'Apache text of a\n' >"${d}/reg/a/LICENSE-APACHE"
  printf 'no trailing newline' >"${d}/reg/f/LICENSE"
  mkdir -p "${d}/reg/sub/.licenses"
  printf 'sub X-MIT text\n' >"${d}/reg/sub/.licenses/X-MIT"
  printf 'sub X-Apache text\n' >"${d}/reg/sub/.licenses/X-Apache"
  printf 'slash text\n' >"${d}/reg/slash/COPYING"
  printf 'unicode text\n' >"${d}/reg/w/LICENSE-UNICODE"
  printf 'pm text\n' >"${d}/reg/pm/LICENSE"
  case "${mfiles}" in
    file) printf 'm text\n' >"${d}/reg/m/LICENSE" ;;
    symlink) ln -s /etc/hostname "${d}/reg/m/LICENSE" ;;
    none) ;;
  esac
  jq -n --arg d "${d}" --arg mlic "${mlic}" --arg msrc "${msrc}" --arg reg "${REG_SRC}" '
    def pkg($n; $lic; $src; $kind): {id: $n, name: $n, version: "1.0.0", license: ($lic | if . == "null" then null else . end),
      license_file: null, source: ($src | if . == "" then null else . end), manifest_path: ($d + "/reg/" + $n + "/Cargo.toml"),
      repository: ("https://example.invalid/" + $n), targets: [{kind: [$kind]}]};
    def ws($n): {id: $n, name: $n, version: "0.0.0", license: "MIT OR Apache-2.0", license_file: null, source: null,
      manifest_path: ($d + "/" + $n + "/Cargo.toml"), repository: null, targets: [{kind: ["lib"]}]};
    def dep($p; $k): {pkg: $p, dep_kinds: [{kind: $k, target: null}]};
    {packages: [ws("fandhe-workshop-pack"), ws("fandhe-workshop-core"), ws("fandhe-workshop-web"),
        pkg("a"; "MIT OR Apache-2.0"; $reg; "lib"), pkg("pm"; "MIT"; $reg; "proc-macro"), pkg("viapm"; "GPL-3.0-only"; $reg; "lib"),
        pkg("bld"; "GPL-3.0-only"; $reg; "lib"), pkg("dev"; "GPL-3.0-only"; $reg; "lib"), pkg("f"; "MIT"; $reg; "lib"),
        pkg("m"; $mlic; $msrc; "lib"), pkg("nofile"; "MIT OR Apache-2.0"; $reg; "lib"),
        pkg("sub"; "MIT OR Apache-2.0"; $reg; "lib"), pkg("slash"; "MIT/Apache-2.0"; $reg; "lib"),
        pkg("w"; "(MIT OR Apache-2.0) AND Unicode-3.0"; $reg; "lib")],
      workspace_members: ["fandhe-workshop-pack", "fandhe-workshop-core", "fandhe-workshop-web"],
      resolve: {nodes: [
        {id: "fandhe-workshop-pack", deps: [dep("fandhe-workshop-core"; null), dep("a"; null), dep("pm"; null), dep("bld"; "build"),
          dep("dev"; "dev"), dep("m"; null), dep("nofile"; null), dep("sub"; null), dep("slash"; null)]},
        {id: "fandhe-workshop-core", deps: [dep("f"; null)]},
        {id: "pm", deps: [dep("viapm"; null)]},
        {id: "fandhe-workshop-web", deps: [dep("a"; null), dep("w"; null)]},
        {id: "a", deps: []}, {id: "viapm", deps: []}, {id: "bld", deps: []}, {id: "dev", deps: []}, {id: "f", deps: []},
        {id: "m", deps: []}, {id: "nofile", deps: []}, {id: "sub", deps: []}, {id: "slash", deps: []}, {id: "w", deps: []}]}}' >"${d}/native.json"
  cp "${d}/native.json" "${d}/wasm.json"
}
META_OK="${TMP_DIR}/meta-ok"
mk_meta "${META_OK}" "MIT" "${REG_SRC}" file

# run_build <args...>: 終了コードを RC、標準エラーを ERR に残す
run_build() {
  RC=0
  PATH="${SHIM}:${PATH}" FAKE_FILE_DESC="${FAKE_FILE_DESC:-${MACHO}}" "${BUILD}" --metadata-dir "${META_USE:-${META_OK}}" "$@" >"${TMP_DIR}/stdout" 2>"${TMP_DIR}/stderr" || RC=$?
}

# 1. 合格
rm -rf "${OUT}"
run_build --out "${OUT}" --bin "${FAKE_BIN}"
expect "passes with an arm64 Mach-O fake" 0 "${RC}"
ARCHIVE="$(find "${OUT}" -name 'fandhe-pack-*-aarch64-apple-darwin.tar.gz' | head -n 1)"
if [ -n "${ARCHIVE}" ]; then
  members="$(tar -tzf "${ARCHIVE}" | sed 's|^[^/]*/||' | grep -v '^$' | sort | tr '\n' ' ')"
  expect "archive holds the executable, both licenses, the notice and the third-party licenses" "LICENSE-APACHE LICENSE-MIT README.txt THIRD-PARTY-LICENSES fandhe-pack " "${members}"
  X="${TMP_DIR}/x"
  mkdir -p "${X}"
  tar -xzf "${ARCHIVE}" -C "${X}"
  rc=0
  cmp -s "${X}"/*/README.txt "${DIR}/../tools/pack-release/README.txt" || rc=$?
  expect "bundled README.txt equals the notice source" 0 "${rc}"
  for needle in "xattr -d com.apple.quarantine fandhe-pack" "shasum -a 256 -c" "THIRD-PARTY-LICENSES"; do
    rc=0
    grep -qF -- "${needle}" "${X}"/*/README.txt || rc=$?
    expect "notice contains: ${needle}" 0 "${rc}"
  done
  mode="$(find "${X}" -name fandhe-pack -type f -perm 755 | wc -l | tr -d ' ')"
  expect "executable has mode 755" 1 "${mode}"
  rc=0
  (cd "${OUT}" && if command -v shasum >/dev/null 2>&1; then shasum -a 256 -c "$(basename "${ARCHIVE}").sha256"; else sha256sum -c "$(basename "${ARCHIVE}").sha256"; fi) >/dev/null 2>&1 || rc=$?
  expect "sha256 file verifies" 0 "${rc}"
  grep -q '^sha256: ' "${TMP_DIR}/stdout" && rc=0 || rc=1
  expect "stdout reports the sha256" 0 "${rc}"
  TPL_FILE="$(find "${X}" -name THIRD-PARTY-LICENSES -type f | head -n 1)"
  for needle in "==== a 1.0.0 ====" "==== f 1.0.0 ====" "==== w 1.0.0 ====" "==== sub 1.0.0 ====" "==== slash 1.0.0 ====" \
    "==== nofile 1.0.0 ====" "sub X-MIT text" "Apache text of a" "unicode text" "  - nofile 1.0.0 (" "Permission is hereby granted" \
    "targets: aarch64-apple-darwin, wasm32-unknown-unknown"; do
    rc=0
    grep -qF -- "${needle}" "${TPL_FILE}" || rc=$?
    expect "third-party licenses contain: ${needle}" 0 "${rc}"
  done
  for absent in "==== pm " "==== viapm " "==== bld " "==== dev " "==== fandhe-workshop-" "${TMP_DIR}"; do
    rc=0
    grep -qF -- "${absent}" "${TPL_FILE}" || rc=$?
    expect "third-party licenses omit: ${absent}" 1 "${rc}"
  done
  expect "third-party licenses list each crate once" 1 "$(grep -c '^==== a 1.0.0 ====' "${TPL_FILE}")"
  expect "third-party licenses count the crates" 1 "$(grep -c '^crate の数: 7$' "${TPL_FILE}")"
  # 決定性: もう一度作って同じバイト列になる
  rm -rf "${TMP_DIR}/out2"
  run_build --out "${TMP_DIR}/out2" --bin "${FAKE_BIN}"
  mkdir -p "${TMP_DIR}/x2"
  tar -xzf "$(find "${TMP_DIR}/out2" -name '*.tar.gz' | head -n 1)" -C "${TMP_DIR}/x2"
  rc=0
  cmp -s "${TPL_FILE}" "${TMP_DIR}"/x2/*/THIRD-PARTY-LICENSES || rc=$?
  expect "third-party licenses are byte-identical across builds" 0 "${rc}"
  GEN="${DIR}/gen-third-party-licenses.sh"
  "${GEN}" --target aarch64-apple-darwin --out "${TMP_DIR}/g1" --native "${META_OK}/native.json" --wasm "${META_OK}/wasm.json" >/dev/null 2>&1
  "${GEN}" --target aarch64-apple-darwin --out "${TMP_DIR}/g2" --native "${META_OK}/native.json" --wasm "${META_OK}/wasm.json" >/dev/null 2>&1
  rc=0
  cmp -s "${TMP_DIR}/g1" "${TMP_DIR}/g2" || rc=$?
  expect "generator output is byte-identical across runs" 0 "${rc}"
else
  expect "archive was created" "yes" "no"
fi

# 2. 検査の失敗（1）。成果物が出ないことも確かめる
fail_case() {
  local name="$1"
  shift
  rm -rf "${OUT}"
  run_build "$@"
  expect "${name} exits 1" 1 "${RC}"
  n=0
  [ ! -d "${OUT}" ] || n="$(find "${OUT}" -type f | wc -l | tr -d ' ')"
  expect "${name} leaves no artifact" 0 "${n}"
}
FAKE_FILE_DESC="Mach-O 64-bit executable x86_64" fail_case "wrong architecture" --out "${OUT}" --bin "${FAKE_BIN}"
FAKE_FILE_DESC="ELF 64-bit LSB executable, ARM aarch64" fail_case "wrong format" --out "${OUT}" --bin "${FAKE_BIN}"
for sub in validate test preview; do
  : >"${TMP_DIR}/fail-${sub}"
  fail_case "failing ${sub} smoke test" --out "${OUT}" --bin "${FAKE_BIN}"
  rm -f "${TMP_DIR}/fail-${sub}"
done

# ライセンスの方針の違反（1）。m の license・source・ファイルだけを差し替える
license_case() {
  local name="$1" lic="$2" src="$3" files="$4" d
  d="${TMP_DIR}/meta-${name// /_}"
  mk_meta "${d}" "${lic}" "${src}" "${files}"
  META_USE="${d}" fail_case "license policy: ${name}" --out "${OUT}" --bin "${FAKE_BIN}"
  rm -rf "${d}"
}
license_case "GPL only" "GPL-3.0-only" "${REG_SRC}" file
license_case "OR with GPL" "MIT OR GPL-3.0-only" "${REG_SRC}" file
license_case "WITH exception" "Apache-2.0 WITH LLVM-exception" "${REG_SRC}" file
license_case "null license" "null" "${REG_SRC}" file
license_case "source not a registry" "MIT" "git+https://example.invalid/m" file
license_case "symlink license file" "MIT" "${REG_SRC}" symlink
license_case "no file and no standard text" "Unlicense" "${REG_SRC}" none

# preview の煙試験の失敗（URL が出ない・IR が compile と違う）
: >"${TMP_DIR}/no-url"
fail_case "preview prints no URL" --out "${OUT}" --bin "${FAKE_BIN}"
rm -f "${TMP_DIR}/no-url"
: >"${SHIM}/ir-differs"
fail_case "preview IR differs from compile" --out "${OUT}" --bin "${FAKE_BIN}"
rm -f "${SHIM}/ir-differs"

# cargo が見える PATH で煙試験を流すと失敗する（Rust の無い環境、という前提を保つ）
RUSTY="${TMP_DIR}/rusty"
mkdir -p "${RUSTY}"
printf '#!/bin/sh\nexit 0\n' >"${RUSTY}/cargo"
chmod +x "${RUSTY}/cargo"
SMOKE_PATH="${RUSTY}:/usr/bin:/bin" fail_case "cargo visible on the smoke PATH" --out "${OUT}" --bin "${FAKE_BIN}"

# 3. 誤用（2）
rm -rf "${OUT}"
run_build --unknown x
expect "unknown option exits 2" 2 "${RC}"
run_build --target x86_64-pc-windows-msvc --out "${OUT}" --bin "${FAKE_BIN}"
expect "unsupported target exits 2" 2 "${RC}"
run_build --out "${OUT}" --bin "${TMP_DIR}/none"
expect "missing --bin file exits 2" 2 "${RC}"
run_build --out
expect "option without a value exits 2" 2 "${RC}"
META_USE="${TMP_DIR}/no-such-meta" run_build --out "${OUT}" --bin "${FAKE_BIN}"
expect "missing --metadata-dir exits 2" 2 "${RC}"
mkdir -p "${TMP_DIR}/meta-half"
cp "${META_OK}/native.json" "${TMP_DIR}/meta-half/"
META_USE="${TMP_DIR}/meta-half" run_build --out "${OUT}" --bin "${FAKE_BIN}"
expect "--metadata-dir with native.json only exits 2" 2 "${RC}"
RC=0
"${DIR}/gen-third-party-licenses.sh" --target aarch64-apple-darwin --out "${TMP_DIR}/g3" --native "${META_OK}/native.json" >/dev/null 2>&1 || RC=$?
expect "generator with --native only exits 2" 2 "${RC}"

# 案内文の元が無いときは成果物を出さない。元は ROOT_DIR 固定のため、写しのツリーで確かめる
NT="${TMP_DIR}/nt"
mkdir -p "${NT}/scripts" "${NT}/docs"
cp "${DIR}/build-pack-release.sh" "${DIR}/gen-third-party-licenses.sh" "${DIR}/lib.sh" "${NT}/scripts/"
mkdir -p "${NT}/tools/pack-release/licenses"
cp "${DIR}/../tools/pack-release/licenses/MIT.txt" "${NT}/tools/pack-release/licenses/"
cp -R "${DIR}/../docs/examples" "${NT}/docs/examples"
cp "${DIR}/../LICENSE-MIT" "${DIR}/../LICENSE-APACHE" "${NT}/"
rm -rf "${OUT}"
RC=0
PATH="${SHIM}:${PATH}" FAKE_FILE_DESC="${MACHO}" "${NT}/scripts/build-pack-release.sh" --metadata-dir "${META_OK}" --out "${OUT}" --bin "${FAKE_BIN}" >/dev/null 2>"${TMP_DIR}/nt.err" || RC=$?
expect "missing notice source exits 2" 2 "${RC}"
rc=0
grep -q 'notice file not found' "${TMP_DIR}/nt.err" || rc=$?
expect "missing notice source reports the notice file" 0 "${rc}"
n=0
[ ! -d "${OUT}" ] || n="$(find "${OUT}" -type f | wc -l | tr -d ' ')"
expect "missing notice source leaves no artifact" 0 "${n}"

echo "# ${passed} passed, ${failed} failed"
[ "${failed}" -eq 0 ]
