#!/usr/bin/env bash
# scripts/check-mutations.sh 自体の回帰テスト（`make test-check-mutations` から呼ばれる）。
#
# 作業ツリーは変えない。check-mutations.sh と lib.sh を mktemp -d の疑似リポジトリへ写して流す
# （定義ファイルは「repo の中」に置く必要があるため）。cargo は PATH の先頭に置いた偽物で再現し、
# 検出・生存・元の状態の失敗・コンパイル不能・定義の不正が、それぞれ期待の終了コードになることを確かめる。
# 最後に実物の cargo で、リポジトリ本体の変異を 1 回流し、作業ツリーが変わらないことも確かめる。
#
# 出力は TAP 風（scripts/hooks/test-hooks.sh に合わせる）。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd cargo

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

# 疑似リポジトリ: scripts/ に検査本体、crates/c/src/lib.rs に変異の対象を置く。
FAKE_REPO="${TMP_DIR}/repo"
mkdir -p "${FAKE_REPO}/scripts" "${FAKE_REPO}/crates/c/src" "${FAKE_REPO}/crates/c/tests/mutations" "${TMP_DIR}/bin"
cp "${DIR}/check-mutations.sh" "${DIR}/lib-mutations.sh" "${DIR}/lib.sh" "${FAKE_REPO}/scripts/"
: >"${FAKE_REPO}/Cargo.toml"
: >"${FAKE_REPO}/Cargo.lock"
: >"${FAKE_REPO}/rust-toolchain.toml"
printf 'let v = 1;\nlet w = 1;\nlet w = 1;\n' >"${FAKE_REPO}/crates/c/src/lib.rs"
CHECK="${FAKE_REPO}/scripts/check-mutations.sh"
DEFS_DIR="${FAKE_REPO}/crates/c/tests/mutations"

# 偽の cargo。写しの中の lib.rs に変異（MUT または特殊文字の文字列）が入っているかで振る舞いを変える。
#   FAKE_BASE_FAIL=1   : 変異の有無によらず試験が失敗する
#   FAKE_SURVIVE=1     : 変異が入っても試験が通る（生存）
#   FAKE_COMPILE_FAIL=1: 変異が入ると --no-run が失敗する
#   FAKE_LITERAL=1     : 変異が入った行が「& \1 "x"」と文字どおりのときだけ試験が失敗する
cat >"${TMP_DIR}/bin/cargo" <<'EOF'
#!/usr/bin/env bash
src="${PWD}/crates/c/src/lib.rs"
mutated=0
if [ "${FAKE_LITERAL:-0}" = "1" ]; then
  grep -Fxq 'let v = & \1 "x";' "${src}" && mutated=1
else
  grep -Fq 'MUT' "${src}" && mutated=1
fi
for a in "$@"; do
  if [ "${a}" = "--no-run" ]; then
    if [ "${FAKE_COMPILE_FAIL:-0}" = "1" ] && [ "${mutated}" = "1" ]; then exit 101; fi
    exit 0
  fi
done
if [ "${FAKE_BASE_FAIL:-0}" = "1" ]; then
  echo "test alpha ... FAILED"
  echo "test beta ... ok"
  exit 101
fi
if [ "${mutated}" = "1" ] && [ "${FAKE_SURVIVE:-0}" != "1" ]; then
  echo "test alpha ... FAILED"
  echo "test beta ... ok"
  exit 101
fi
echo "test alpha ... ok"
echo "test beta ... ok"
exit 0
EOF
chmod 755 "${TMP_DIR}/bin/cargo"

# write_def <name> <find> <replace> [extra-line...]: 既定は妥当な定義を DEFS_DIR/<name>.mutation に書く。
write_def() {
  local name="$1" find="$2" replace="$3"
  shift 3
  {
    echo "id: ${name}"
    echo "package: fake-pkg"
    echo "test: fake_test"
    echo "target: crates/c/src/lib.rs"
    echo "find: ${find}"
    echo "replace: ${replace}"
    echo "expect-fail: alpha"
    for extra in "$@"; do echo "${extra}"; done
  } >"${DEFS_DIR}/${name}.mutation"
}

# run_check <args...>: 偽の cargo を先頭にして流し、終了コードを RC に入れる。環境変数は呼び出し側が前置する。
run_check() {
  RC=0
  PATH="${TMP_DIR}/bin:${PATH}" "${CHECK}" "$@" >"${TMP_DIR}/stdout.txt" 2>"${TMP_DIR}/stderr.txt" || RC=$?
}

# --- 検出・生存 ---
write_def ok 'let v = 1;' 'let v = MUT;'
run_check
expect "all mutations detected" 0 "${RC}"
grep -q '| ok | .* | DETECTED |' "${TMP_DIR}/stdout.txt" && r=0 || r=1
expect "record row says DETECTED" 0 "${r}"

RC=0
FAKE_SURVIVE=1 PATH="${TMP_DIR}/bin:${PATH}" "${CHECK}" >"${TMP_DIR}/stdout.txt" 2>/dev/null || RC=$?
expect "surviving mutation is reported" 1 "${RC}"
grep -q 'SURVIVED' "${TMP_DIR}/stdout.txt" && r=0 || r=1
expect "record row says SURVIVED" 0 "${r}"

# --- 元の状態・コンパイル ---
RC=0
FAKE_BASE_FAIL=1 PATH="${TMP_DIR}/bin:${PATH}" "${CHECK}" >/dev/null 2>&1 || RC=$?
expect "failing unmodified state is an error" 2 "${RC}"

RC=0
FAKE_COMPILE_FAIL=1 PATH="${TMP_DIR}/bin:${PATH}" "${CHECK}" >/dev/null 2>&1 || RC=$?
expect "mutation that does not compile is an error" 2 "${RC}"

write_def ghost 'let v = 1;' 'let v = MUT;' 'expect-fail: gamma'
run_check "${DEFS_DIR}/ghost.mutation"
expect "expect-fail name absent from the unmodified run is an error" 2 "${RC}"
rm -f "${DEFS_DIR}/ghost.mutation"

# --- find の出現回数 ---
write_def none 'let nothing = 0;' 'let v = MUT;'
run_check "${DEFS_DIR}/none.mutation"
expect "find occurring 0 times is an error" 2 "${RC}"
write_def twice 'let w = 1;' 'let w = MUT;'
run_check "${DEFS_DIR}/twice.mutation"
expect "find occurring 2 times is an error" 2 "${RC}"
rm -f "${DEFS_DIR}/none.mutation" "${DEFS_DIR}/twice.mutation"

# --- find / replace は文字どおりに扱う ---
write_def literal 'let v = 1;' 'let v = & \1 "x";'
RC=0
FAKE_LITERAL=1 PATH="${TMP_DIR}/bin:${PATH}" "${CHECK}" "${DEFS_DIR}/literal.mutation" >/dev/null 2>&1 || RC=$?
expect "replace text is applied literally (& backslash quote)" 0 "${RC}"
rm -f "${DEFS_DIR}/literal.mutation"

# --- target の検査 ---
write_def abs 'let v = 1;' 'let v = MUT;'
sed 's|^target: .*|target: /etc/hosts|' "${DEFS_DIR}/abs.mutation" >"${DEFS_DIR}/abs2.mutation"
run_check "${DEFS_DIR}/abs2.mutation"
expect "absolute target is rejected" 2 "${RC}"
sed 's|^target: .*|target: crates/c/../c/src/lib.rs|' "${DEFS_DIR}/abs.mutation" >"${DEFS_DIR}/dots.mutation"
run_check "${DEFS_DIR}/dots.mutation"
expect "target with .. is rejected" 2 "${RC}"
ln -s src/lib.rs "${FAKE_REPO}/crates/c/link.rs"
sed 's|^target: .*|target: crates/c/link.rs|' "${DEFS_DIR}/abs.mutation" >"${DEFS_DIR}/sym.mutation"
run_check "${DEFS_DIR}/sym.mutation"
expect "symlink target is rejected" 2 "${RC}"
rm -f "${DEFS_DIR}/abs.mutation" "${DEFS_DIR}/abs2.mutation" "${DEFS_DIR}/dots.mutation" "${DEFS_DIR}/sym.mutation"

# --- 定義の形式 ---
write_def nokey 'let v = 1;' 'let v = MUT;'
grep -v '^test: ' "${DEFS_DIR}/nokey.mutation" >"${DEFS_DIR}/nokey2.mutation"
run_check "${DEFS_DIR}/nokey2.mutation"
expect "missing required key is rejected" 2 "${RC}"
write_def unknown 'let v = 1;' 'let v = MUT;' 'bogus: 1'
run_check "${DEFS_DIR}/unknown.mutation"
expect "unknown key is rejected" 2 "${RC}"
write_def dup 'let v = 1;' 'let v = MUT;' 'note: a' 'note: b'
run_check "${DEFS_DIR}/dup.mutation"
expect "duplicate single key is rejected" 2 "${RC}"
write_def badpkg 'let v = 1;' 'let v = MUT;'
sed 's|^package: .*|package: Bad;pkg|' "${DEFS_DIR}/badpkg.mutation" >"${DEFS_DIR}/badpkg2.mutation"
run_check "${DEFS_DIR}/badpkg2.mutation"
expect "package with unsafe characters is rejected" 2 "${RC}"
rm -f "${DEFS_DIR}"/nokey*.mutation "${DEFS_DIR}"/unknown.mutation "${DEFS_DIR}"/dup.mutation "${DEFS_DIR}"/badpkg*.mutation

# --- 引数 ---
run_check --bogus
expect "unknown option is a usage error" 2 "${RC}"
run_check "${DEFS_DIR}/missing.mutation"
expect "nonexistent definition is a usage error" 2 "${RC}"
run_check "${TMP_DIR}/outside.mutation"
expect "definition outside the repository is a usage error" 2 "${RC}"
rm -f "${DEFS_DIR}/ok.mutation"
run_check
expect "no definitions at all is an error" 2 "${RC}"

# --- 実物の cargo で、リポジトリ本体の変異を流す ---
before="$(git -C "${ROOT_DIR}" status --porcelain)"
RC=0
"${DIR}/check-mutations.sh" >"${TMP_DIR}/real-stdout.txt" 2>"${TMP_DIR}/real-stderr.txt" || RC=$?
expect "real cargo: repository mutations are all detected" 0 "${RC}"
after="$(git -C "${ROOT_DIR}" status --porcelain)"
[ "${before}" = "${after}" ] && r=0 || r=1
expect "real cargo: the working tree is unchanged" 0 "${r}"

echo "# ${passed} passed, ${failed} failed"
[ "${failed}" -eq 0 ]
