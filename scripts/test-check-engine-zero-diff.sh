#!/usr/bin/env bash
# scripts/check-engine-zero-diff.sh 自体の回帰テスト（`make test-check-engine-zero-diff` から呼ばれる。CI の check ジョブ）。
#
# 一時ディレクトリに作った見本の git repo を --root で検査し、差分 0 のとき 0、範囲内の変更・追加・移動・
# 一覧の改ざん・教材の窓の誤りで 1、誤用・前提の欠けで 2 になることを確かめる。現物の repo は検査しない
# （現物は make check-engine-zero-diff）。作業ツリーは変えない。ネットワーク・cargo は使わない。
# commit は identity・署名・フックの設定に左右されないよう -c で固定する。
#
# 出力は TAP 風。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
for c in mktemp git rm grep sed ln dirname bash awk cmp head wc cat mv tr; do
  require_cmd "${c}"
done

CHECK="${DIR}/check-engine-zero-diff.sh"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

passed=0
failed=0
BASE=""
expect() {
  if [ "$2" = "$3" ]; then
    echo "ok - $1 (exit $3)"
    passed=$((passed + 1))
  else
    echo "not ok - $1 (expected exit $2, got $3)"
    failed=$((failed + 1))
  fi
}

# g <repo> <git の引数...>: identity・署名・フックの設定に依らない git
g() {
  local r="$1"
  shift
  git -C "${r}" -c user.name=t -c user.email=t@example.invalid -c commit.gpgsign=false -c core.hooksPath=/dev/null "$@"
}

# new_repo: base を作って BASE に SHA を入れ、一覧を TMP_DIR/m.<repo名> に書き、教材を足す commit を作る。
new_repo() {
  local r
  r="$(mktemp -d "${TMP_DIR}/repo.XXXXXX")"
  git -C "${r}" init -q
  mkdir -p "${r}/crates/a/src" "${r}/template-workshop" "${r}/docs"
  printf '[workspace]\n' > "${r}/Cargo.toml"
  printf '# lock\n' > "${r}/Cargo.lock"
  printf '[package]\nname = "a"\n' > "${r}/crates/a/Cargo.toml"
  printf 'fn main() {}\n' > "${r}/crates/a/src/lib.rs"
  printf 'x\n' > "${r}/template-workshop/README.md"
  printf 'd\n' > "${r}/docs/d.md"
  g "${r}" add -A
  g "${r}" commit -q -m base
  BASE="$(git -C "${r}" rev-parse HEAD)"
  printf '%s' "${BASE}" > "${TMP_DIR}/b.$(basename "${r}")"
  "${CHECK}" --root "${r}" --base "${BASE}" --head "${BASE}" --manifest "${TMP_DIR}/m.$(basename "${r}")" --write-manifest >/dev/null
  mkdir -p "${r}/template-workshop/examples/p"
  printf 'p\n' > "${r}/template-workshop/examples/p/a.md"
  g "${r}" add -A
  g "${r}" commit -q -m pack
  printf '%s' "${r}"
}

base_of() { cat "${TMP_DIR}/b.$(basename "$1")"; }

# run <name> <rc> <repo> [引数...]
run() {
  local name="$1" want="$2" r="$3" rc=0
  shift 3
  "${CHECK}" --root "${r}" --base "$(base_of "${r}")" --manifest "${TMP_DIR}/m.$(basename "${r}")" --pack template-workshop/examples/p "$@" >"${TMP_DIR}/out" 2>"${TMP_DIR}/err" || rc=$?
  expect "${name}" "${want}" "${rc}"
}
commit() {
  g "$1" add -A
  g "$1" commit -q -m "$2"
}
has() {
  if grep -q -- "$2" "${TMP_DIR}/out"; then expect "$1" 0 0; else expect "$1" 0 1; fi
}

r="$(new_repo)"; run "no change" 0 "${r}"
has "uncounted template-workshop is reported" '^INFO uncounted template-workshop/ files=1'
r="$(new_repo)"; printf 'y\n' >> "${r}/template-workshop/examples/p/a.md"; commit "${r}" t; run "pack change only" 0 "${r}"
r="$(new_repo)"; printf 'y\n' >> "${r}/docs/d.md"; commit "${r}" t; run "docs change only" 0 "${r}"
r="$(new_repo)"; printf 'fn x() {}\n' >> "${r}/crates/a/src/lib.rs"; commit "${r}" t; run ".rs change" 1 "${r}"
has "diff-lines fails" '^FAIL diff-lines'
has "manifest-head fails" '^FAIL manifest-head'
has "changed path is listed" 'changed crates/a/src/lib.rs'
r="$(new_repo)"; printf '# l\n' >> "${r}/Cargo.lock"; commit "${r}" t; run "Cargo.lock change" 1 "${r}"
has "cargo-lock fails" '^FAIL cargo-lock'
r="$(new_repo)"; printf '# t\n' >> "${r}/Cargo.toml"; commit "${r}" t; run "root Cargo.toml change" 1 "${r}"
has "cargo-toml fails" '^FAIL cargo-toml'
r="$(new_repo)"; printf '# t\n' >> "${r}/crates/a/Cargo.toml"; commit "${r}" t; run "crate Cargo.toml change" 1 "${r}"
has "crate-manifests fails" '^FAIL crate-manifests'
r="$(new_repo)"; printf 'n\n' > "${r}/crates/a/src/new.rs"; commit "${r}" t; run "file added in crates" 1 "${r}"
has "manifest-count fails" '^FAIL manifest-count'
r="$(new_repo)"; g "${r}" mv crates/a/src/lib.rs crates/a/src/moved.rs; commit "${r}" t; run "file moved in crates" 1 "${r}"
has "move counts as lines" '^FAIL diff-lines'
r="$(new_repo)"; printf 'z\n' >> "${r}/crates/a/src/lib.rs"; commit "${r}" t; g "${r}" revert --no-edit HEAD >/dev/null 2>&1; run "change then revert nets to 0" 0 "${r}"
has "window commits are counted" '^INFO window commits 3'
r="$(new_repo)"; printf '0000000000000000000000000000000000000000000000000000000000000000  crates/a/Cargo.toml\n' > "${TMP_DIR}/m.$(basename "${r}")"; run "record rewritten" 1 "${r}"
has "manifest-base fails" '^FAIL manifest-base'
r="$(new_repo)"; printf 'w\n' >> "${r}/crates/a/src/lib.rs"; run "uncommitted change" 1 "${r}"
has "worktree-clean fails" '^FAIL worktree-clean'
r="$(new_repo)"; printf 'w\n' > "${r}/crates/a/src/untracked.rs"; run "untracked file" 1 "${r}"
has "untracked file fails worktree-clean" '^FAIL worktree-clean'
r="$(new_repo)"; printf 'y\n' >> "${r}/docs/d.md"; commit "${r}" t; printf 'w\n' >> "${r}/crates/a/src/lib.rs"; run "uncommitted change is skipped for a fixed head" 0 "${r}" --head "$(git -C "${r}" rev-parse HEAD~1)"
has "worktree-clean is skipped" '^SKIP worktree-clean'
r="$(new_repo)"; rc=0
"${CHECK}" --root "${r}" --base "$(git -C "${r}" rev-parse HEAD)" --manifest "${TMP_DIR}/m.$(basename "${r}")" --pack template-workshop/examples/p >/dev/null 2>&1 || rc=$?
expect "base after the pack was added" 1 "${rc}"
r="$(new_repo)"; run "base is not an ancestor of head" 1 "${r}" --base "$(git -C "${r}" rev-parse HEAD)" --head "$(base_of "${r}")"
r="$(new_repo)"; run "unknown base" 2 "${r}" --base 0000000000000000000000000000000000000000
r="$(new_repo)"; run "unknown option" 2 "${r}" --bogus
r="$(new_repo)"; run "missing manifest" 2 "${r}" --manifest "${TMP_DIR}/none.sha256"
r="$(new_repo)"; run "dotdot in pack" 2 "${r}" --pack ../x
r="$(new_repo)"; run "dash-leading head" 2 "${r}" --head --all

# SHA-256 の道具が無い PATH では 2 になる
tools="${TMP_DIR}/bin"
mkdir -p "${tools}"
for c in git awk sed cmp head wc mktemp rm cat mv dirname tr; do
  ln -s "$(command -v "${c}")" "${tools}/${c}"
done
r="$(new_repo)"; rc=0
PATH="${tools}" "$(command -v bash)" "${CHECK}" --root "${r}" --base "$(base_of "${r}")" --manifest "${TMP_DIR}/m.$(basename "${r}")" >/dev/null 2>&1 || rc=$?
expect "no sha256 tool" 2 "${rc}"

echo "1..$((passed + failed))"
echo "# passed=${passed} failed=${failed}"
[ "${failed}" = "0" ]
