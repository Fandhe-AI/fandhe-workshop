#!/usr/bin/env bash
# scripts/check-frontend-bump.sh（版上げ PR の受け入れ確認 (1)〜(5)。TASK-101 1/2・2/2）自体の回帰テスト
# （`make test-check-frontend-bump` から呼ばれる）。
#
# 版上げを模擬する。mktemp -d の一時 git repo に base と head のコミットを作り、基準を外すと失敗することを確かめる。
# 作業ツリーは変えない。件数の入力は見本のログと TAP で、実際の試験は流さない。
# (3) の WASM の gzip は build-web.sh のログの見本を注入して判定だけを確かめる（実際のビルドは流さない）。
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

# write_lock <dest> <frontend-version> [wasm-bindgen-version]
write_lock() {
  printf '[[package]]\nname = "fandhe-frontend-core"\nversion = "%s"\nsource = "registry+https://example.invalid/index"\nchecksum = "aa"\n\n[[package]]\nname = "other"\nversion = "1.0.0"\n\n[[package]]\nname = "wasm-bindgen"\nversion = "%s"\n' "$2" "${3:-0.2.100}" >"$1"
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
  printf '# build\n' >"${REPO}/tools/build-web.sh"
  printf '[toolchain]\nchannel = "1.90.0"\n' >"${REPO}/rust-toolchain.toml"
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

# テーブル形式の宣言の中身が変わる（Cargo.lock は不変）
new_repo
printf '[workspace.dependencies.fandhe-frontend-wasm-full]\nversion = "0.46.0"\ndefault-features = false\nfeatures = ["action-keydown"]\n\n[workspace.dependencies.serde]\nversion = "1"\n' >"${REPO}/Cargo.toml"
git -C "${REPO}" commit -q -am base2
printf '[workspace.dependencies.fandhe-frontend-wasm-full]\nversion = "0.46.0"\ndefault-features = true\nfeatures = ["action-keydown"]\n\n[workspace.dependencies.serde]\nversion = "1"\n' >"${REPO}/Cargo.toml"
commit_head
run pull_request detect
expect_out "detect: table-form declaration changed -> true" true

# テーブル形式の外（別の依存）だけが変わる
new_repo
printf '[workspace.dependencies.fandhe-frontend-wasm-full]\nversion = "0.46.0"\n\n[workspace.dependencies.serde]\nversion = "1"\n' >"${REPO}/Cargo.toml"
git -C "${REPO}" commit -q -am base2
printf '[workspace.dependencies.fandhe-frontend-wasm-full]\nversion = "0.46.0"\n\n[workspace.dependencies.serde]\nversion = "2"\n' >"${REPO}/Cargo.toml"
commit_head
run pull_request detect
expect_out "detect: other table changed -> false" false

# 複数行の features の変更（Cargo.lock は不変）
new_repo
printf '[workspace.dependencies]\nfandhe-frontend-wasm-full = { version = "0.46.0", default-features = false, features = [\n  "action-keydown",\n] }\nserde = "1"\n' >"${REPO}/Cargo.toml"
git -C "${REPO}" commit -q -am base2
printf '[workspace.dependencies]\nfandhe-frontend-wasm-full = { version = "0.46.0", default-features = false, features = [\n  "action-keydown",\n  "extra",\n] }\nserde = "1"\n' >"${REPO}/Cargo.toml"
commit_head
run pull_request detect
expect_out "detect: multi-line features changed -> true" true

# 複数行の宣言の外の変更は無視する
new_repo
printf '[workspace.dependencies]\nfandhe-frontend-wasm-full = { version = "0.46.0", features = [\n  "action-keydown",\n] }\nserde = "1"\n' >"${REPO}/Cargo.toml"
git -C "${REPO}" commit -q -am base2
printf '[workspace.dependencies]\nfandhe-frontend-wasm-full = { version = "0.46.0", features = [\n  "action-keydown",\n] }\nserde = "2"\n' >"${REPO}/Cargo.toml"
commit_head
run pull_request detect
expect_out "detect: change after a multi-line declaration -> false" false

# 抽出の異常（awk の失敗）は「変更なし」にせず判定不能で止める
FAKE_AWK_BIN="${TMP_DIR}/fakeawk"
mkdir -p "${FAKE_AWK_BIN}"
REAL_AWK="$(command -v awk)"
cat >"${FAKE_AWK_BIN}/awk" <<EOF
#!/usr/bin/env bash
for a in "\$@"; do
  case "\${a}" in *.toml) exit 2 ;; esac
done
exec "${REAL_AWK}" "\$@"
EOF
chmod +x "${FAKE_AWK_BIN}/awk"
new_repo
write_lock "${REPO}/Cargo.lock" "0.4.4"
commit_head
RC=0
OUT="$(PATH="${FAKE_AWK_BIN}:${PATH}" CI_EVENT_NAME=pull_request "${SCRIPT}" --repo "${REPO}" --base HEAD^1 detect 2>&1)" || RC=$?
expect "detect: declaration extraction failure fails closed" 2 "${RC}"

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

# --- guard: tools/build-web.sh も版上げ PR で変えてはいけない（(3) の測定の手順を守る） ---
new_repo
write_lock "${REPO}/Cargo.lock" "0.4.4"
echo "# weaker" >>"${REPO}/tools/build-web.sh"
commit_head
run pull_request guard --cargo-metadata-json "${TMP_DIR}/md-ok.json"
expect "guard: bump + tools/build-web.sh changed fails" 1 "${RC}"

# --- wasm-gzip (3) ---
# write_build_log <dest> <gzip-bytes> [config] [wasm-opt-detail]: build-web.sh の終わりの 3 行の見本。
write_build_log() {
  local dest="$1" gz="$2" config="${3:-delivery}" opt="${4:-not applied (SKIP_WASM_OPT=1)}"
  {
    printf '[build-web.sh] cargo build --locked\n'
    printf '[build-web.sh] config: %s / profile: release\n' "${config}"
    printf '[build-web.sh] wasm-opt: %s\n' "${opt}"
    printf '[build-web.sh] wasm size: raw %s bytes, gzip -9 %s bytes\n' "$((gz * 3))" "${gz}"
    printf '[build-web.sh] output: /x\n'
  } >"${dest}"
}
write_build_log "${TMP_DIR}/b1000.log" 1000
write_build_log "${TMP_DIR}/h1100.log" 1100
write_build_log "${TMP_DIR}/h1101.log" 1101
write_build_log "${TMP_DIR}/h900.log" 900
write_build_log "${TMP_DIR}/h1000.log" 1000
write_build_log "${TMP_DIR}/h-test.log" 1000 test
write_build_log "${TMP_DIR}/h-opt.log" 1000 delivery "applied (wasm-opt version 1)"
write_build_log "${TMP_DIR}/b0.log" 0
{
  printf '[build-web.sh] config: delivery / profile: release\n'
  printf '[build-web.sh] wasm-opt: not applied (SKIP_WASM_OPT=1)\n'
} >"${TMP_DIR}/h-nosize.log"
{
  cat "${TMP_DIR}/h1000.log"
  printf '[build-web.sh] wasm size: raw 3 bytes, gzip -9 1 bytes\n'
} >"${TMP_DIR}/h-dup.log"

# gz <head-log> [args...]: 版上げ PR（Cargo.lock の frontend の版が違う）で wasm-gzip を流す。
new_repo
write_lock "${REPO}/Cargo.lock" "0.4.4"
commit_head
gz() {
  local headlog="$1"
  shift
  run pull_request wasm-gzip --base-build-log "${TMP_DIR}/b1000.log" --head-build-log "${headlog}" "$@"
}
gz "${TMP_DIR}/h1100.log"
expect "wasm-gzip: bump with exactly +10% passes" 0 "${RC}"
gz "${TMP_DIR}/h1101.log"
expect "wasm-gzip: bump with one byte over +10% fails" 1 "${RC}"
gz "${TMP_DIR}/h900.log"
expect "wasm-gzip: bump with a smaller WASM passes" 0 "${RC}"
gz "${TMP_DIR}/h1000.log"
expect "wasm-gzip: bump with the same size passes" 0 "${RC}"
gz "${TMP_DIR}/h-test.log"
expect "wasm-gzip: non-delivery config fails closed" 2 "${RC}"
gz "${TMP_DIR}/h-opt.log"
expect "wasm-gzip: wasm-opt status differs fails closed" 2 "${RC}"
gz "${TMP_DIR}/h-nosize.log"
expect "wasm-gzip: missing wasm size line fails closed" 2 "${RC}"
gz "${TMP_DIR}/h-dup.log"
expect "wasm-gzip: duplicated wasm size line fails closed" 2 "${RC}"
run pull_request wasm-gzip --base-build-log "${TMP_DIR}/b0.log" --head-build-log "${TMP_DIR}/h1000.log"
expect "wasm-gzip: base gzip size 0 fails closed" 2 "${RC}"
run pull_request wasm-gzip --base-build-log "${TMP_DIR}/b1000.log" --head-build-log "${TMP_DIR}/no-such.log"
expect "wasm-gzip: unreadable head log fails closed" 2 "${RC}"
run pull_request wasm-gzip --base-build-log "${TMP_DIR}/b1000.log"
expect "wasm-gzip: only one build log is a misuse" 2 "${RC}"
run pull_request wasm-gzip extra
expect "wasm-gzip: extra argument is a misuse" 2 "${RC}"
run push wasm-gzip --base-build-log "${TMP_DIR}/b1000.log" --head-build-log "${TMP_DIR}/h1101.log"
expect "wasm-gzip: non-PR event skips" 0 "${RC}"
run push wasm-gzip --assume-bump --base-build-log "${TMP_DIR}/b1000.log" --head-build-log "${TMP_DIR}/h1101.log"
expect "wasm-gzip: --assume-bump measures even outside a PR (over limit fails)" 1 "${RC}"

# 版上げでない PR は測らない（外れる値でも skip）。--assume-bump なら測る。
new_repo
echo "unrelated 2" >"${REPO}/README.md"
commit_head
gz "${TMP_DIR}/h1101.log"
expect "wasm-gzip: non-bump PR skips" 0 "${RC}"
gz "${TMP_DIR}/h1101.log" --assume-bump
expect "wasm-gzip: non-bump PR with --assume-bump fails over the limit" 1 "${RC}"

# 基点が揃わない版上げ PR は判定不能（別の PR で先に上げる）
new_repo
write_lock "${REPO}/Cargo.lock" "0.4.4" "0.2.101"
commit_head
gz "${TMP_DIR}/h1000.log"
expect "wasm-gzip: wasm-bindgen differs fails closed" 2 "${RC}"

new_repo
write_lock "${REPO}/Cargo.lock" "0.4.4"
printf '[toolchain]\nchannel = "1.91.0"\n' >"${REPO}/rust-toolchain.toml"
commit_head
gz "${TMP_DIR}/h1000.log"
expect "wasm-gzip: rust-toolchain.toml differs fails closed" 2 "${RC}"

new_repo
write_lock "${REPO}/Cargo.lock" "0.4.4"
commit_head
run pull_request wasm-gzip --base no-such-rev --base-build-log "${TMP_DIR}/b1000.log" --head-build-log "${TMP_DIR}/h1000.log"
expect "wasm-gzip: missing base rev fails closed" 2 "${RC}"

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
