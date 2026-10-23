#!/usr/bin/env bash
# scripts/check-crate-deps.sh 自体の回帰テスト（`make test-check-deps` から呼ばれる）。
#
# 「core から web への依存を足すと検査が失敗する」ことを、手作業ではなくここで自動で確かめる。
# 作業ツリーは変えない。Cargo.toml・Cargo.lock・crates/ を mktemp -d の一時ディレクトリへ写して改変する。
# cargo の失敗・resolve が null の場合は、偽の cargo（PATH の先頭）で再現する。ネットワークには頼らない
# （lock の更新は `cargo update --workspace --offline`。registry の cache が無い環境では失敗扱いにする）。
#
# 出力は TAP 風（scripts/hooks/test-hooks.sh に合わせる）。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd cargo
require_cmd jq

CHECK="${DIR}/check-crate-deps.sh"
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

# fresh_copy <name>: 作業ツリーの Cargo 関連ファイルを一時ディレクトリへ写し、そのパスを出力する。
fresh_copy() {
  local dest="${TMP_DIR}/$1"
  mkdir -p "${dest}"
  cp "${ROOT_DIR}/Cargo.toml" "${ROOT_DIR}/Cargo.lock" "${dest}/"
  cp -R "${ROOT_DIR}/crates" "${dest}/crates"
  echo "${dest}"
}

# run_check <dir>: 一時コピーを検査し、終了コードを RC に入れる（失敗しても止まらない）。
run_check() {
  RC=0
  "${CHECK}" --manifest-path "$1/Cargo.toml" >/dev/null 2>"${TMP_DIR}/stderr.txt" || RC=$?
}

# add_after_dependencies <toml> <line>: [dependencies] 表の直後に 1 行足す。
add_after_dependencies() {
  awk -v line="$2" '{ print } /^\[dependencies\]$/ { print line }' "$1" >"$1.new"
  mv "$1.new" "$1"
}

# refresh_lock <dir>: manifest を変えた後に、パッケージを変えずに lock を更新する。
refresh_lock() {
  if ! cargo update --workspace --offline --manifest-path "$1/Cargo.toml" >/dev/null 2>&1; then
    echo "# cargo update --offline failed (registry cache missing?)"
    return 1
  fi
}

# mutate_case <name> <expected-rc> <toml-relative-path> <mode> <line>
#   mode = dev: [dev-dependencies] を末尾に足す / normal: [dependencies] の直後に足す
mutate_case() {
  local name="$1" want="$2" toml="$3" mode="$4" line="$5" dir
  dir="$(fresh_copy "case-${passed}-${failed}")"
  if [ "${mode}" = "dev" ]; then
    if grep -q '^\[dev-dependencies\]$' "${dir}/${toml}"; then
      awk -v line="${line}" '{ print } /^\[dev-dependencies\]$/ { print line }' "${dir}/${toml}" >"${dir}/${toml}.new"
      mv "${dir}/${toml}.new" "${dir}/${toml}"
    else
      printf '\n[dev-dependencies]\n%s\n' "${line}" >>"${dir}/${toml}"
    fi
  else
    add_after_dependencies "${dir}/${toml}" "${line}"
  fi
  # 期待が 2（検査不能）のケースは cargo が循環で lock 更新自体に失敗するため、更新せずに検査する
  if [ "${want}" != 2 ] && ! refresh_lock "${dir}"; then
    expect "${name} (lock refresh)" 0 1
    return 0
  fi
  run_check "${dir}"
  expect "${name}" "${want}" "${RC}"
}

# 1. 実際のワークスペースは違反 0 件
RC=0
"${CHECK}" >/dev/null 2>&1 || RC=$?
expect "real workspace passes" 0 "${RC}"

# 2. 受け入れ条件: core から web への依存を足すと失敗する
mutate_case "core -> web (dev) is rejected" 1 crates/core/Cargo.toml dev \
  'fandhe-workshop-web = { path = "../web" }'
# cargo 自身が循環依存で失敗する経路も、合格にしない
mutate_case "core -> web (normal) fails closed" 2 crates/core/Cargo.toml normal \
  'fandhe-workshop-web = { path = "../web" }'
mutate_case "contract -> core (dev) is rejected" 1 crates/contract/Cargo.toml dev \
  'fandhe-workshop-core = { path = "../core" }'
mutate_case "pack -> web (normal) is rejected" 1 crates/pack/Cargo.toml normal \
  'fandhe-workshop-web = { path = "../web" }'
mutate_case "web -> pack (normal) is rejected" 1 crates/web/Cargo.toml normal \
  'fandhe-workshop-pack.workspace = true'

# 2b. EVT-23・TASK-206 の受け入れ条件: core・contract の依存の閉包に表示基盤の crate を足すと失敗し、
#     戻すと通る。禁止の 4 系統（web-sys・js-sys・wasm-bindgen・fandhe-frontend）を実物の Cargo.toml への改変で確かめる。
mutate_case "core -> web-sys (normal) is rejected" 1 crates/core/Cargo.toml normal \
  'web-sys.workspace = true'
mutate_case "core -> js-sys (normal) is rejected" 1 crates/core/Cargo.toml normal \
  'js-sys.workspace = true'
mutate_case "core -> wasm-bindgen (normal) is rejected" 1 crates/core/Cargo.toml normal \
  'wasm-bindgen.workspace = true'
mutate_case "core -> fandhe-frontend-core (normal) is rejected" 1 crates/core/Cargo.toml normal \
  'fandhe-frontend-core.workspace = true'
mutate_case "contract -> wasm-bindgen (normal) is rejected" 1 crates/contract/Cargo.toml normal \
  'wasm-bindgen.workspace = true'
# 境界の固定: 禁じるのは normal 依存の閉包だけで、試験だけの dev 依存は通る（architecture.md 第 4 節）
mutate_case "core -> web-sys (dev) is allowed" 0 crates/core/Cargo.toml dev \
  'web-sys.workspace = true'

# mutate_and_revert_case: 同じ一時コピーで、禁止の依存を足すと失敗し、元の Cargo.toml・Cargo.lock を戻すと通る。
mutate_and_revert_case() {
  local dir
  dir="$(fresh_copy mutate-revert)"
  add_after_dependencies "${dir}/crates/core/Cargo.toml" 'web-sys.workspace = true'
  if ! refresh_lock "${dir}"; then
    expect "mutate-and-revert (lock refresh)" 0 1
    return 0
  fi
  run_check "${dir}"
  expect "mutate-and-revert: mutated copy is rejected" 1 "${RC}"
  cp "${ROOT_DIR}/crates/core/Cargo.toml" "${dir}/crates/core/Cargo.toml"
  cp "${ROOT_DIR}/Cargo.lock" "${dir}/Cargo.lock"
  run_check "${dir}"
  expect "mutate-and-revert: restored copy passes" 0 "${RC}"
}
mutate_and_revert_case

# 3. default-members に web を足すと失敗する
dir="$(fresh_copy default-members)"
awk '/^default-members = \[$/ { print; print "    \"crates/web\","; next } { print }' "${dir}/Cargo.toml" >"${dir}/Cargo.toml.new"
mv "${dir}/Cargo.toml.new" "${dir}/Cargo.toml"
run_check "${dir}"
expect "web in default-members is rejected" 1 "${RC}"

# 4. 偽の cargo（metadata の出力を固定する）
FAKE_BIN="${TMP_DIR}/bin"
mkdir -p "${FAKE_BIN}"
cat >"${FAKE_BIN}/cargo" <<'EOF'
#!/usr/bin/env bash
case "${FAKE_MODE}" in
  fail) echo "error: boom" >&2; exit 101 ;;
  nullresolve) echo '{"packages":[],"workspace_members":[],"workspace_default_members":[],"resolve":null}' ;;
  wasm)
    cat <<'JSON'
{"packages":[{"id":"c","name":"fandhe-workshop-contract"},{"id":"k","name":"fandhe-workshop-core"},
  {"id":"p","name":"fandhe-workshop-pack"},{"id":"w","name":"fandhe-workshop-web"},{"id":"b","name":"wasm-bindgen"}],
  "workspace_members":["c","k","p","w"],"workspace_default_members":["c","k","p"],
  "resolve":{"nodes":[
  {"id":"c","deps":[]},
  {"id":"k","deps":[{"pkg":"c","dep_kinds":[{"kind":null}]},{"pkg":"b","dep_kinds":[{"kind":null,"target":"cfg(target_arch = \"wasm32\")"}]}]},
  {"id":"p","deps":[{"pkg":"k","dep_kinds":[{"kind":null}]},{"pkg":"c","dep_kinds":[{"kind":null}]}]},
  {"id":"w","deps":[{"pkg":"k","dep_kinds":[{"kind":null}]},{"pkg":"c","dep_kinds":[{"kind":null}]},{"pkg":"p","dep_kinds":[{"kind":"dev"}]}]},
  {"id":"b","deps":[]}]}}
JSON
    ;;
esac
EOF
chmod 755 "${FAKE_BIN}/cargo"

RC=0
FAKE_MODE=wasm PATH="${FAKE_BIN}:${PATH}" "${CHECK}" >/dev/null 2>"${TMP_DIR}/stderr.txt" || RC=$?
expect "wasm-bindgen in core closure is rejected" 1 "${RC}"
RC=0
FAKE_MODE=fail PATH="${FAKE_BIN}:${PATH}" "${CHECK}" >/dev/null 2>&1 || RC=$?
expect "cargo failure fails closed" 2 "${RC}"
RC=0
FAKE_MODE=nullresolve PATH="${FAKE_BIN}:${PATH}" "${CHECK}" >/dev/null 2>&1 || RC=$?
expect "null resolve fails closed" 2 "${RC}"

# 5. 誤用
RC=0
"${CHECK}" --unknown >/dev/null 2>&1 || RC=$?
expect "unknown argument is a usage error" 2 "${RC}"

echo "# passed: ${passed}, failed: ${failed}"
if [ "${failed}" -ne 0 ]; then
  exit 1
fi
