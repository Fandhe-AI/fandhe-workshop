#!/usr/bin/env bash
# scripts/check-build-inventory.sh 自体の回帰テスト（`make test-check-build-inventory` から呼ばれる。CI の check ジョブ）。
#
# 一時ディレクトリに作った見本の git repo を --root で検査し、private な依存・secret らしい語の混入で
# 検査が失敗すること（と、道具の欠落・解析不能で 2 になること）を確かめる。許可リストは回帰テスト専用の
# --allowlist-file で差し替える。現物の repo は検査しない（関門化は TASK-96 の承認事項）。
# 作業ツリーは変えない。ネットワークは使わない。
#
# 出力は TAP 風。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd mktemp
require_cmd git

CHECK="${DIR}/check-build-inventory.sh"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

passed=0
failed=0

expect() {
  if [ "$2" = "$3" ]; then
    echo "ok - $1 (exit $3)"
    passed=$((passed + 1))
  else
    echo "not ok - $1 (expected exit $2, got $3)"
    failed=$((failed + 1))
  fi
}

REG='registry+https://github.com/rust-lang/crates.io-index'
EMPTY_ALLOW="${TMP_DIR}/empty-allow.txt"
: > "${EMPTY_ALLOW}"

# new_repo: 見本の repo を作って標準出力にパスを返す。member は crates/a の 1 つ。
new_repo() {
  local r
  r="$(mktemp -d "${TMP_DIR}/repo.XXXXXX")"
  git -C "${r}" init -q
  mkdir -p "${r}/crates/a"
  printf '[workspace]\nmembers = [\n    "crates/a",\n]\n' > "${r}/Cargo.toml"
  printf '[package]\nname = "a"\nversion = "0.1.0"\n' > "${r}/crates/a/Cargo.toml"
  printf '# generated\nversion = 4\n\n[[package]]\nname = "a"\nversion = "0.1.0"\ndependencies = [\n "dep",\n]\n\n[[package]]\nname = "dep"\nversion = "1.0.0"\nsource = "%s"\nchecksum = "00ff"\n' "${REG}" > "${r}/Cargo.lock"
  printf '%s' "${r}"
}

# run_repo <name> <expected-rc> <repo> [allowlist]: 追跡に加えて検査する。
run_repo() {
  local name="$1" want="$2" r="$3" allow="${4:-${EMPTY_ALLOW}}" rc=0
  git -C "${r}" add -A
  "${CHECK}" --root "${r}" --allowlist-file "${allow}" >"${TMP_DIR}/out" 2>"${TMP_DIR}/err" || rc=$?
  expect "${name}" "${want}" "${rc}"
}

# put_lock <repo> <package block>: 2 つ目の package を足した Cargo.lock にする。
add_pkg() {
  printf '\n[[package]]\nname = "x"\nversion = "1.0.0"\n%s\n' "$2" >> "$1/Cargo.lock"
}

echo "TAP version 13"

r="$(new_repo)"; run_repo "clean sample passes" 0 "${r}"
if grep -q 'private-deps: 0' "${TMP_DIR}/out" && grep -q 'secret-refs: 0' "${TMP_DIR}/out"; then
  expect "record shows zero counts" 0 0
else
  expect "record shows zero counts" 0 1
fi

r="$(new_repo)"; add_pkg "${r}" 'source = "git+https://example.invalid/x#abc"'; run_repo "git source is a violation" 1 "${r}"
if grep -q 'example.invalid' "${TMP_DIR}/err"; then expect "source URL is not echoed" 0 1; else expect "source URL is not echoed" 0 0; fi
r="$(new_repo)"; add_pkg "${r}" 'source = "registry+https://example.invalid/index"'; run_repo "other registry is a violation" 1 "${r}"
r="$(new_repo)"; add_pkg "${r}" 'source = "sparse+https://example.invalid/"'; run_repo "sparse registry is a violation" 1 "${r}"
r="$(new_repo)"; add_pkg "${r}" ''; run_repo "sourceless non-member is a violation" 1 "${r}"

r="$(new_repo)"; sed -i.bak 's/^version = 4$/version = 3/' "${r}/Cargo.lock"; rm -f "${r}/Cargo.lock.bak"; run_repo "lock version 3 is unparsable (2)" 2 "${r}"
r="$(new_repo)"; printf 'unknown = "1"\n' >> "${r}/Cargo.lock"; run_repo "unknown line in lock (2)" 2 "${r}"
r="$(new_repo)"; rm "${r}/Cargo.lock"; run_repo "missing Cargo.lock (2)" 2 "${r}"
r="$(new_repo)"; printf '\n[[package]]\nname = "x"\nversion = "1.0.0"\nsource = "%s"\ndependencies = [\n "dep",\n' "${REG}" >> "${r}/Cargo.lock"; run_repo "truncated dependencies array (2)" 2 "${r}"
rc=0; "${CHECK}" --root "${TMP_DIR}" >/dev/null 2>&1 || rc=$?; expect "root that is not a git work tree (2)" 2 "${rc}"
rc=0; "${CHECK}" --bogus >/dev/null 2>&1 || rc=$?; expect "unknown argument (2)" 2 "${rc}"
rc=0; "${CHECK}" --root >/dev/null 2>&1 || rc=$?; expect "--root without a value (2)" 2 "${rc}"

# 6 形の語（ダミー。実在の認証情報の形ではない）
word_case() {
  local name="$1" text="$2" want="${3:-1}"
  r="$(new_repo)"; mkdir -p "${r}/scripts"; printf '%s\n' "${text}" > "${r}/scripts/x.sh"
  run_repo "${name}" "${want}" "${r}"
}
word_case "GITHUB_TOKEN is detected" 'echo GITHUB_TOKEN'
word_case "CARGO_REGISTRIES_X_TOKEN is detected" 'export CARGO_REGISTRIES_X_TOKEN=dummy'
word_case "git-fetch-with-cli is detected" 'net.git-fetch-with-cli = true'
word_case "secrets. is detected" 'x: {{ secrets.DUMMY }}'
word_case "MY_TOKEN is detected" 'MY_TOKEN=dummy'
word_case "MY_PAT is detected" 'MY_PAT=dummy'
word_case "mixed case My_Pat is detected" 'My_Pat=dummy'
word_case "GITHUB_PATH passes (end rule)" 'echo GITHUB_PATH' 0
word_case "LEFT_STATE_TOKENS passes (end rule)" 'LEFT_STATE_TOKENS=1' 0

r="$(new_repo)"; mkdir -p "${r}/.github/workflows"; printf 'env:\n  T: {{ secrets.DUMMY }}\n' > "${r}/.github/workflows/ci.yml"; run_repo "ci.yml secrets reference is detected" 1 "${r}"
r="$(new_repo)"; mkdir -p "${r}/.github/workflows"; printf 'env:\n  T: {{ secrets.DUMMY }}\n' > "${r}/.github/workflows/deploy.yml"; run_repo "deploy.yml is out of scope" 0 "${r}"
r="$(new_repo)"; mkdir -p "${r}/docs"; printf 'MY_TOKEN\n' > "${r}/docs/x.md"; run_repo "docs are out of scope" 0 "${r}"
r="$(new_repo)"; mkdir -p "${r}/scripts"; printf 'MY_TOKEN\n' > "${r}/scripts/untracked.sh"
rc=0; "${CHECK}" --root "${r}" --allowlist-file "${EMPTY_ALLOW}" >/dev/null 2>&1 || rc=$?; expect "untracked file is not scanned" 0 "${rc}"

# 許可リスト
ALLOW="${TMP_DIR}/allow.txt"
printf 'pair\tscripts/x.sh\tmy_token\n' > "${ALLOW}"
r="$(new_repo)"; mkdir -p "${r}/scripts"; printf 'MY_TOKEN=dummy\n' > "${r}/scripts/x.sh"; run_repo "allowlisted pair passes" 0 "${r}" "${ALLOW}"
r="$(new_repo)"; mkdir -p "${r}/scripts"; printf 'MY_TOKEN=dummy\n' > "${r}/scripts/y.sh"; run_repo "same word in another path is detected" 1 "${r}" "${ALLOW}"
r="$(new_repo)"; run_repo "stale allowlist pair fails" 1 "${r}" "${ALLOW}"
printf 'exclude\tscripts/gone.sh\n' > "${ALLOW}"
r="$(new_repo)"; run_repo "stale excluded file fails" 1 "${r}" "${ALLOW}"
printf 'exclude\tscripts/x.sh\n' > "${ALLOW}"
r="$(new_repo)"; mkdir -p "${r}/scripts"; printf 'MY_TOKEN=dummy\n' > "${r}/scripts/x.sh"; run_repo "excluded file passes" 0 "${r}" "${ALLOW}"
printf 'bogus line\n' > "${ALLOW}"
r="$(new_repo)"; run_repo "bad allowlist line (2)" 2 "${r}" "${ALLOW}"

# 範囲の中の二進・symlink は検査を完了できない
r="$(new_repo)"; mkdir -p "${r}/scripts"; printf 'a\000b\n' > "${r}/scripts/bin.dat"; run_repo "binary file in scope (2)" 2 "${r}"
r="$(new_repo)"; mkdir -p "${r}/scripts"; ln -s ../Cargo.toml "${r}/scripts/link"; run_repo "symlink in scope (2)" 2 "${r}"

# 出力に行の本文（値）が出ない
r="$(new_repo)"; mkdir -p "${r}/scripts"; printf 'MY_TOKEN=dummyvalue123\n' > "${r}/scripts/x.sh"; run_repo "violation case for output check" 1 "${r}"
if grep -q 'dummyvalue123' "${TMP_DIR}/err" "${TMP_DIR}/out"; then expect "line body is not echoed" 0 1; else expect "line body is not echoed" 0 0; fi

echo "1..$((passed + failed))"
echo "# passed=${passed} failed=${failed}"
[ "${failed}" = "0" ]
