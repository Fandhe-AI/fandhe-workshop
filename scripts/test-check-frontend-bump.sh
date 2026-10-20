#!/usr/bin/env bash
# scripts/check-frontend-bump.sh（版上げ PR の受け入れ確認 (1)(2)(4)(5)。TASK-101 1/2）自体の回帰テスト
# （`make test-check-frontend-bump` から呼ばれる）。
#
# 版上げを模擬する。mktemp -d の一時 git repo に base と head のコミットを作り、基準を外すと失敗することを確かめる。
# 作業ツリーは変えない。件数の入力は見本のログと TAP で、実際の試験は流さない。
# 出力は TAP 風。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
for cmd in git jq mktemp rm mkdir cp printf; do
  require_cmd "${cmd}"
done

SCRIPT="${DIR}/check-frontend-bump.sh"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

passed=0
failed=0

# expect <name> <expected-rc> <actual-rc>
expect() {
  if [ "$2" = "$3" ]; then
    echo "ok - $1 (exit $3)"
    passed=$((passed + 1))
  else
    echo "not ok - $1 (expected exit $2, got $3)"
    failed=$((failed + 1))
  fi
}

ESC="$(printf '\033')"

# 見本のログ。ANSI つきの cargo の出力と、node --test の TAP の末尾。
write_rust_log() {
  local dest="$1" n="$2"
  {
    printf '     Running unittests src/lib.rs (target/debug/deps/foo-0123abcd)\n'
    printf 'running %s tests\n' "${n}"
    printf 'test result: %s[32mok%s[0m. %s passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n' "${ESC}" "${ESC}" "${n}"
    printf '   Doc-tests foo\n'
    printf 'test result: %s[32mok%s[0m. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n' "${ESC}" "${ESC}"
  } >"${dest}"
}

write_tap() {
  local dest="$1" tests="$2" fail="$3"
  {
    printf 'TAP version 13\n# Subtest: a\n    # tests 99\n'
    printf '1..1\n# tests %s\n# suites 0\n# pass %s\n# fail %s\n# cancelled 0\n# skipped 0\n# todo 0\n# duration_ms 1\n' \
      "${tests}" "$((tests - fail))" "${fail}"
  } >"${dest}"
}

write_metadata() {
  local dest="$1" default_features="$2" features="$3"
  printf '{"packages":[{"name":"fandhe-workshop-web","dependencies":[{"name":"fandhe-frontend-wasm-full","uses_default_features":%s,"features":%s}]}]}\n' \
    "${default_features}" "${features}" >"${dest}"
}

write_lock() {
  printf '[[package]]\nname = "fandhe-frontend-core"\nversion = "%s"\nsource = "registry+https://example.invalid/index"\nchecksum = "aa"\n\n[[package]]\nname = "other"\nversion = "1.0.0"\n' "$2" >"$1"
}

write_root_toml() {
  printf '[workspace.dependencies]\nfandhe-frontend-wasm-full = { version = "0.46.0", default-features = false, features = [%s] }\nserde = "1"\n' "$2" >"$1"
}

# base の repo を作る。件数の見本の期待値は normalize の出力から作る（書式を二重に持たない）。
write_rust_log "${TMP_DIR}/rust-base.log" 3
write_tap "${TMP_DIR}/base.tap" 10 0
EXPECTED_BASE="${TMP_DIR}/expected-base.txt"
{
  echo "# sample"
  "${SCRIPT}" normalize rust "${TMP_DIR}/rust-base.log"
  "${SCRIPT}" normalize browser "${TMP_DIR}/base.tap"
} >"${EXPECTED_BASE}"

REPO_N=0
# new_repo: base コミット済みの repo を作り、パスを REPO に入れる。
new_repo() {
  REPO_N=$((REPO_N + 1))
  REPO="${TMP_DIR}/repo${REPO_N}"
  mkdir -p "${REPO}/crates/web" "${REPO}/tools" "${REPO}/scripts"
  write_lock "${REPO}/Cargo.lock" "0.4.3"
  write_root_toml "${REPO}/Cargo.toml" '"action-keydown"'
  printf '[dependencies]\nfandhe-frontend-wasm-full.workspace = true\n' >"${REPO}/crates/web/Cargo.toml"
  printf 'EXC_N=1\n' >"${REPO}/tools/check-deps.sh"
  printf '# gate\n' >"${REPO}/scripts/check-frontend-bump.sh"
  cp "${EXPECTED_BASE}" "${REPO}/scripts/expected-test-counts.txt"
  echo "unrelated 1" >"${REPO}/README.md"
  git -C "${REPO}" init -q
  git -C "${REPO}" config user.email t@example.invalid
  git -C "${REPO}" config user.name t
  git -C "${REPO}" config commit.gpgsign false
  git -C "${REPO}" add -A
  git -C "${REPO}" commit -q -m base
}

commit_head() {
  git -C "${REPO}" add -A
  git -C "${REPO}" commit -q -m head
}

# run <env-event> <args...>: 終了コードを RC に入れる。標準出力・標準エラーは捨てない（失敗時の診断のため OUT に残す）。
RC=0
OUT=""
run() {
  local event="$1"
  shift
  RC=0
  OUT="$(CI_EVENT_NAME="${event}" "${SCRIPT}" --repo "${REPO}" --base HEAD^1 "$@" 2>&1)" || RC=$?
}

# expect_out <name> <expected-stdout>: 直前の run が 0 で、出力が一致すること。
expect_out() {
  if [ "${RC}" = 0 ] && [ "${OUT}" = "$2" ]; then
    expect "$1" 0 0
  else
    expect "$1" 0 "${RC}:${OUT}"
  fi
}

# --- detect ---
new_repo
write_lock "${REPO}/Cargo.lock" "0.4.4"
commit_head
run pull_request detect
expect "detect: lock version only changed" 0 "${RC}"
expect_out "detect: prints true (lock)" true

new_repo
write_root_toml "${REPO}/Cargo.toml" '"action-keydown", "extra"'
commit_head
run pull_request detect
expect_out "detect: root features changed -> true" true

new_repo
echo "unrelated 2" >"${REPO}/README.md"
commit_head
run pull_request detect
expect_out "detect: unrelated file -> false" false

new_repo
commit_head_empty() { git -C "${REPO}" commit -q --allow-empty -m head; }
commit_head_empty
run pull_request detect --base no-such-rev
expect "detect: missing base rev fails closed" 2 "${RC}"

# --- guard ---
new_repo
write_lock "${REPO}/Cargo.lock" "0.4.4"
commit_head
write_metadata "${TMP_DIR}/md-ok.json" false '["action-keydown"]'
run pull_request guard --cargo-metadata-json "${TMP_DIR}/md-ok.json"
expect "guard: bump with nothing guarded changed passes" 0 "${RC}"

new_repo
write_lock "${REPO}/Cargo.lock" "0.4.4"
cp "${EXPECTED_BASE}" "${REPO}/scripts/expected-test-counts.txt"
echo "rust|total|binaries=1|passed=1|ignored=0" >>"${REPO}/scripts/expected-test-counts.txt"
commit_head
run pull_request guard --cargo-metadata-json "${TMP_DIR}/md-ok.json"
expect "guard: bump + expected counts changed fails" 1 "${RC}"

new_repo
write_lock "${REPO}/Cargo.lock" "0.4.4"
printf 'EXC_N=2\n' >"${REPO}/tools/check-deps.sh"
commit_head
run pull_request guard --cargo-metadata-json "${TMP_DIR}/md-ok.json"
expect "guard: bump + check-deps.sh EXC_N 1 -> 2 fails" 1 "${RC}"

new_repo
write_lock "${REPO}/Cargo.lock" "0.4.4"
echo "# weaker" >>"${REPO}/scripts/check-frontend-bump.sh"
commit_head
run pull_request guard --cargo-metadata-json "${TMP_DIR}/md-ok.json"
expect "guard: bump + gate script changed fails" 1 "${RC}"

new_repo
printf 'EXC_N=2\n' >"${REPO}/tools/check-deps.sh"
commit_head
run pull_request guard --cargo-metadata-json "${TMP_DIR}/md-ok.json"
expect "guard: non-bump may change check-deps.sh" 0 "${RC}"

new_repo
commit_head_empty
run push guard --cargo-metadata-json "${TMP_DIR}/md-ok.json"
expect "guard: non-PR event skips the diff guard" 0 "${RC}"

write_metadata "${TMP_DIR}/md-default.json" true '["action-keydown"]'
run pull_request guard --cargo-metadata-json "${TMP_DIR}/md-default.json"
expect "guard (5): default features enabled fails" 1 "${RC}"

write_metadata "${TMP_DIR}/md-nokey.json" false '[]'
run pull_request guard --cargo-metadata-json "${TMP_DIR}/md-nokey.json"
expect "guard (5): action-keydown missing fails" 1 "${RC}"

printf '{broken' >"${TMP_DIR}/md-broken.json"
run pull_request guard --cargo-metadata-json "${TMP_DIR}/md-broken.json"
expect "guard (5): broken metadata JSON fails closed" 2 "${RC}"

# --- counts rust ---
write_rust_log "${TMP_DIR}/rust-minus.log" 2
write_rust_log "${TMP_DIR}/rust-plus.log" 4

new_repo
write_lock "${REPO}/Cargo.lock" "0.4.4"
commit_head
run pull_request counts rust "${TMP_DIR}/rust-base.log"
expect "counts rust: bump with matching counts (ANSI) passes" 0 "${RC}"
run pull_request counts rust "${TMP_DIR}/rust-minus.log"
expect "counts rust: bump with one test fewer fails" 1 "${RC}"
run pull_request counts rust "${TMP_DIR}/rust-plus.log"
expect "counts rust: bump with one test more fails" 1 "${RC}"

printf 'no results here\n' >"${TMP_DIR}/rust-empty.log"
run pull_request counts rust "${TMP_DIR}/rust-empty.log"
expect "counts rust: no test result lines fails closed" 2 "${RC}"

new_repo
cp "${EXPECTED_BASE}" "${REPO}/scripts/expected-test-counts.txt"
echo "# touched" >>"${REPO}/scripts/expected-test-counts.txt"
commit_head
run pull_request counts rust "${TMP_DIR}/rust-minus.log"
expect "counts rust: non-bump + expected changed + mismatch fails" 1 "${RC}"
run pull_request counts rust "${TMP_DIR}/rust-base.log"
expect "counts rust: non-bump + expected changed + match passes" 0 "${RC}"

new_repo
echo "unrelated 2" >"${REPO}/README.md"
commit_head
run pull_request counts rust "${TMP_DIR}/rust-minus.log"
expect "counts rust: non-bump + expected unchanged + mismatch skips" 0 "${RC}"
run push counts rust "${TMP_DIR}/rust-minus.log"
expect "counts rust: non-PR event skips" 0 "${RC}"

# --- counts browser ---
write_tap "${TMP_DIR}/tap-minus.tap" 9 0
write_tap "${TMP_DIR}/tap-fail.tap" 10 1
printf 'TAP version 13\n# tests 10\n' >"${TMP_DIR}/tap-partial.tap"

new_repo
write_lock "${REPO}/Cargo.lock" "0.4.4"
commit_head
run pull_request counts browser "${TMP_DIR}/base.tap"
expect "counts browser: bump with matching counts passes" 0 "${RC}"
run pull_request counts browser "${TMP_DIR}/tap-minus.tap"
expect "counts browser: bump with one test fewer fails" 1 "${RC}"
run pull_request counts browser "${TMP_DIR}/tap-fail.tap"
expect "counts browser: failing tests fail closed" 2 "${RC}"
run pull_request counts browser "${TMP_DIR}/tap-partial.tap"
expect "counts browser: missing TAP summary fails closed" 2 "${RC}"

# --- 変更ファイル照合の異常（grep が 2 以上を返す）は「変更なし」にせず fail-closed にする ---
REAL_GREP="$(command -v grep)"
FAKE_BIN="${TMP_DIR}/fakebin"
mkdir -p "${FAKE_BIN}"
cat >"${FAKE_BIN}/grep" <<EOF
#!/usr/bin/env bash
for a in "\$@"; do
  case "\${a}" in */changed.txt) exit 2 ;; esac
done
exec "${REAL_GREP}" "\$@"
EOF
chmod +x "${FAKE_BIN}/grep"

new_repo
write_lock "${REPO}/Cargo.lock" "0.4.4"
commit_head
RC=0
OUT="$(PATH="${FAKE_BIN}:${PATH}" CI_EVENT_NAME=pull_request "${SCRIPT}" --repo "${REPO}" --base HEAD^1 guard --cargo-metadata-json "${TMP_DIR}/md-ok.json" 2>&1)" || RC=$?
expect "guard: grep failure on the changed list fails closed" 2 "${RC}"
RC=0
OUT="$(PATH="${FAKE_BIN}:${PATH}" CI_EVENT_NAME=pull_request "${SCRIPT}" --repo "${REPO}" --base HEAD^1 counts rust "${TMP_DIR}/rust-base.log" 2>&1)" || RC=$?
expect "counts rust: grep failure on the changed list fails closed" 2 "${RC}"

# changed.txt の読み取り失敗（--changed-files が読めない）も判定不能
run pull_request guard --changed-files "${TMP_DIR}/no-such-list.txt" --cargo-metadata-json "${TMP_DIR}/md-ok.json"
expect "guard: unreadable changed-files list fails closed" 2 "${RC}"

# --- misuse ---
run pull_request --no-such-option detect
expect "misuse: unknown option" 2 "${RC}"
run pull_request bogus
expect "misuse: unknown subcommand" 2 "${RC}"
run pull_request counts rust
expect "misuse: counts without file" 2 "${RC}"

echo "# passed=${passed} failed=${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
