#!/usr/bin/env bash
# scripts/check-fork-ci.sh 自体の回帰テスト（`make test-check-fork-ci` と ci.yml の check ジョブから呼ばれる）。
#
# 「private な依存・secret の参照・配信の取り込みを足すと検査が失敗する」ことを、一時ファイルを検査して自動で
# 確かめる。違反の見本は、終了コードに加えて「正しい理由（検査の項目名）で失敗したか」も確かめる。
# B1（check-workflow-secrets.sh。ruby を要する）以外の見本は、B1 の実体を常に成功する stub に差し替えて
# 流すため、ruby が無い環境でも動く。B1 の見本は ruby がある場合だけ本物で確かめ、無い場合は 2 で止まる
# （fail-closed）ことを確かめる。
# 作業ツリーは変えない。ネットワークは使わない。
#
# 出力は TAP 風（scripts/hooks/test-hooks.sh に合わせる）。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
require_cmd mktemp
require_cmd rm

CHECK="${DIR}/check-fork-ci.sh"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

SHA="3d3c42e5aac5ba805825da76410c181273ba90b1"
REV="0123456789abcdef0123456789abcdef01234567"
STUB="${TMP_DIR}/secrets-stub.sh"
printf '#!/bin/sh\nexit 0\n' >"${STUB}"
chmod +x "${STUB}"

passed=0
failed=0

# case_ <name> <expected-rc> <expected-tag|-> <args...>: 検査を B1 の stub つきで流す。
# 失敗の見本は、標準エラーに <expected-tag>（A1 等）が出ていることも確かめる。
case_() {
  local name="$1" want="$2" tag="$3" rc=0 err="${TMP_DIR}/err"
  shift 3
  "${CHECK}" --secrets-checker "${STUB}" "$@" >/dev/null 2>"${err}" || rc=$?
  if [ "${rc}" != "${want}" ]; then
    echo "not ok - ${name} (expected exit ${want}, got ${rc})"
    sed 's/^/# /' "${err}"
    failed=$((failed + 1))
  elif [ "${tag}" != "-" ] && ! grep -q -- ":${tag} " "${err}"; then
    echo "not ok - ${name} (exit ${rc} but no ${tag} violation reported)"
    sed 's/^/# /' "${err}"
    failed=$((failed + 1))
  else
    echo "ok - ${name} (exit ${rc})"
    passed=$((passed + 1))
  fi
}

# ---- 見本の部品 ----
GOOD_CI="${TMP_DIR}/ci.yml"
cat >"${GOOD_CI}" <<EOF
name: CI
on:
  pull_request:
permissions:
  contents: read
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@${SHA} # v7.0.1
        with:
          persist-credentials: false
      - name: Path
        run: echo "\${RUNNER_TEMP}/bin" >> "\${GITHUB_PATH}"
EOF

GOOD_DEPLOY="${TMP_DIR}/deploy.yml"
cat >"${GOOD_DEPLOY}" <<'EOF'
name: Deploy
on:
  workflow_dispatch:
permissions: {}
jobs:
  deploy:
    name: deploy
    if: github.repository == 'Fandhe-AI/fandhe-workshop'
    runs-on: ubuntu-latest
    permissions: {}
    steps:
      - run: exit 1
EOF

GOOD_LOCK="${TMP_DIR}/Cargo.lock"
cat >"${GOOD_LOCK}" <<'EOF'
version = 4

[[package]]
name = "serde"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc"

[[package]]
name = "fandhe-core"
version = "0.1.0"
EOF

GOOD_NPM="${TMP_DIR}/package-lock.json"
cat >"${GOOD_NPM}" <<'EOF'
{"name":"x","lockfileVersion":3,"packages":{"":{"name":"x"},"node_modules/a":{"version":"1.0.0","resolved":"https://registry.npmjs.org/a/-/a-1.0.0.tgz"}}}
EOF

GOOD_MODULES="${TMP_DIR}/gitmodules"
printf '[submodule "docs/spec"]\n\tpath = docs/spec\n\turl = https://github.com/Fandhe-AI/x.git\n\tupdate = none\n' >"${GOOD_MODULES}"

# 正準の形（check-fork-ci.sh の D3 の 3 step）。教材 repo の checkout・ENGINE_REV の出力・エンジンの checkout の順
GOOD_PACK="${TMP_DIR}/pack.yml"
cat >"${GOOD_PACK}" <<EOF
name: Build pack
on:
  push:
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@${SHA} # v7.0.1
        with:
          persist-credentials: false
      - name: Read engine revision
        id: rev
        run: echo "rev=\$(cat ENGINE_REV)" >> "\$GITHUB_OUTPUT"
      - name: Checkout engine
        uses: actions/checkout@${SHA} # v7.0.1
        with:
          repository: Fandhe-AI/fandhe-workshop
          ref: \${{ steps.rev.outputs.rev }}
          path: engine
          persist-credentials: false
EOF

GOOD_REV="${TMP_DIR}/ENGINE_REV"
printf '%s\n' "${REV}" >"${GOOD_REV}"

# splice <src> <out> <after|eq|prefix> <pattern> <text-file>: 複数行の差し替えを行う（BSD sed の置換文字列は
# \n を改行にしないため、sed では作らない）。after = pattern を含む行の後ろへ text を足す / eq = 行全体が
# pattern の行を text で置き換える / prefix = pattern で始まる行を text で置き換える。
splice() {
  awk -v mode="$3" -v pat="$4" -v f="$5" '
    function emit(   l) { while ((getline l < f) > 0) print l; close(f) }
    mode == "after" { print; if (index($0, pat) > 0) emit(); next }
    mode == "eq" { if ($0 == pat) emit(); else print; next }
    mode == "prefix" { if (index($0, pat) == 1) emit(); else print; next }
  ' "$1" >"$2"
}

# variant <out> <sed-expr>: 良い見本の差分から違反の見本を作る（sed -e の式を 1 つ渡す）。
variant() {
  local src="$1" out="$2" expr="$3"
  sed -e "${expr}" "${src}" >"${out}"
}

echo "# positive samples"
case_ "the real repo passes (A-C)" 0 -
case_ "sample inputs pass (A-C)" 0 - --ci "${GOOD_CI}" --deploy "${GOOD_DEPLOY}" --cargo-lock "${GOOD_LOCK}" \
  --package-lock "${GOOD_NPM}" --gitmodules "${GOOD_MODULES}" --npmrc "${TMP_DIR}/none-npmrc" --cargo-config-dir "${TMP_DIR}/none-cargo"
case_ "GITHUB_PATH is not mistaken for a _PAT word" 0 - --ci "${GOOD_CI}"
case_ "canonical pack workflow passes (D)" 0 - --pack-workflow "${GOOD_PACK}" --engine-rev "${GOOD_REV}"
# 正準でない形は、版が固定されていても拒否する（正準は 1 つ）
variant "${GOOD_PACK}" "${TMP_DIR}/pack-literal.yml" "s|\${{ steps.rev.outputs.rev }}|${REV}|"
case_ "pack workflow with the literal ENGINE_REV ref is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-literal.yml" --engine-rev "${GOOD_REV}"

echo "# A: private dependencies"
# 認証情報入りの URL の見本は、secret-scan.sh に検知されないよう実行時に組み立てる
AT="@"
for src in "ssh://git@github.com/Fandhe-AI/private.git" "git@github.com:Fandhe-AI/private.git" \
  "git+https://user:pw${AT}github.com/Fandhe-AI/private" "git+https://github.com/Fandhe-AI/not-allowed?rev=abc#abc" \
  "registry+https://registry.example.invalid/index" "git+https://github.com/Fandhe-AI/private.git"; do
  lock="${TMP_DIR}/lock-$(printf '%s' "${src}" | cksum | cut -d' ' -f1)"
  { cat "${GOOD_LOCK}"; printf 'source = "%s"\n' "${src}"; } >"${lock}"
  case_ "Cargo.lock source ${src%%@*} is rejected (A1)" 1 A1 --cargo-lock "${lock}"
done
variant "${GOOD_NPM}" "${TMP_DIR}/npm-private.json" 's|registry.npmjs.org|npm.example.invalid|'
case_ "private npm registry is rejected (A2)" 1 A2 --package-lock "${TMP_DIR}/npm-private.json"
variant "${GOOD_NPM}" "${TMP_DIR}/npm-git.json" 's|https://registry.npmjs.org/a/-/a-1.0.0.tgz|git+ssh://git@github.com/x/y.git|'
case_ "git npm dependency is rejected (A2)" 1 A2 --package-lock "${TMP_DIR}/npm-git.json"
variant "${GOOD_NPM}" "${TMP_DIR}/npm-file.json" 's|https://registry.npmjs.org/a/-/a-1.0.0.tgz|file:../a|'
case_ "file: npm dependency is rejected (A2)" 1 A2 --package-lock "${TMP_DIR}/npm-file.json"
printf '//npm.example.invalid/:_authToken=dummy\n' >"${TMP_DIR}/npmrc-token"
case_ ".npmrc credentials are rejected (A3)" 1 A3 --npmrc "${TMP_DIR}/npmrc-token"
printf 'registry=https://npm.example.invalid/\n' >"${TMP_DIR}/npmrc-registry"
case_ ".npmrc registry is rejected (A3)" 1 A3 --npmrc "${TMP_DIR}/npmrc-registry"
printf 'engine-strict=true\n' >"${TMP_DIR}/npmrc-ok"
case_ ".npmrc with only engine-strict passes" 0 - --npmrc "${TMP_DIR}/npmrc-ok"
mkdir -p "${TMP_DIR}/cargo-bad" "${TMP_DIR}/cargo-registry"
printf '[net]\ngit-fetch-with-cli = true\n' >"${TMP_DIR}/cargo-bad/config.toml"
case_ "cargo git-fetch-with-cli is rejected (A4)" 1 A4 --cargo-config-dir "${TMP_DIR}/cargo-bad"
printf '[registries.private]\nindex = "sparse+https://x.invalid/"\n' >"${TMP_DIR}/cargo-registry/config"
case_ "cargo private registry is rejected (A4)" 1 A4 --cargo-config-dir "${TMP_DIR}/cargo-registry"
printf '[submodule "docs/spec"]\n\tpath = docs/spec\n\turl = https://github.com/Fandhe-AI/x.git\n' >"${TMP_DIR}/gitmodules-bad"
case_ "submodule without update = none is rejected (A5)" 1 A5 --gitmodules "${TMP_DIR}/gitmodules-bad"
printf '          submodules: recursive\n' >"${TMP_DIR}/t-sub"
splice "${GOOD_CI}" "${TMP_DIR}/ci-submodules.yml" after 'persist-credentials: false' "${TMP_DIR}/t-sub"
case_ "checkout with submodules is rejected (A5)" 1 A5 --ci "${TMP_DIR}/ci-submodules.yml"
printf '          submodules: false\n' >"${TMP_DIR}/t-sub"
splice "${GOOD_CI}" "${TMP_DIR}/ci-submodules-false.yml" after 'persist-credentials: false' "${TMP_DIR}/t-sub"
case_ "checkout with submodules: false passes" 0 - --ci "${TMP_DIR}/ci-submodules-false.yml"

echo "# B: secret references in the build/test workflow"
printf '    env:\n      CARGO_REGISTRIES_X_TOKEN: dummy\n' >"${TMP_DIR}/t-env"
splice "${GOOD_CI}" "${TMP_DIR}/ci-registry-env.yml" after '    runs-on: ubuntu-latest' "${TMP_DIR}/t-env"
case_ "CARGO_REGISTR* word is rejected (B2)" 1 B2 --ci "${TMP_DIR}/ci-registry-env.yml"
# shellcheck disable=SC2016 # sed の式なので、シェルに展開させない
variant "${GOOD_CI}" "${TMP_DIR}/ci-pat.yml" 's|echo "\${RUNNER_TEMP}/bin"|echo "${MY_PAT}"|'
case_ "_PAT word is rejected (B2)" 1 B2 --ci "${TMP_DIR}/ci-pat.yml"
# shellcheck disable=SC2016 # sed の式なので、シェルに展開させない
variant "${GOOD_CI}" "${TMP_DIR}/ci-token.yml" 's|echo "\${RUNNER_TEMP}/bin"|echo "${GITHUB_TOKEN}"|'
case_ "GITHUB_TOKEN word is rejected (B2)" 1 B2 --ci "${TMP_DIR}/ci-token.yml"
# shellcheck disable=SC2016 # sed の式なので、シェルに展開させない
variant "${GOOD_CI}" "${TMP_DIR}/ci-fetch.yml" 's|echo "\${RUNNER_TEMP}/bin"|echo git-fetch-with-cli|'
case_ "git-fetch-with-cli word is rejected (B2)" 1 B2 --ci "${TMP_DIR}/ci-fetch.yml"
variant "${GOOD_CI}" "${TMP_DIR}/ci-tag.yml" "s|@${SHA} # v7.0.1|@v7|"
case_ "tag-pinned uses is rejected (B3)" 1 B3 --ci "${TMP_DIR}/ci-tag.yml"
variant "${GOOD_CI}" "${TMP_DIR}/ci-owner.yml" 's|actions/checkout|someone/checkout|'
case_ "uses from a non-allowed owner is rejected (B3)" 1 B3 --ci "${TMP_DIR}/ci-owner.yml"
variant "${GOOD_CI}" "${TMP_DIR}/ci-reusable.yml" "s|actions/checkout@${SHA}|Fandhe-AI/actions/.github/workflows/rust-base-ci.yml@${SHA}|"
case_ "reusable workflow is rejected (B3)" 1 B3 --ci "${TMP_DIR}/ci-reusable.yml"
# B1 は本物の check-workflow-secrets.sh で確かめる（ruby を要する）
# shellcheck disable=SC2016 # sed の式なので、シェルに展開させない
variant "${GOOD_CI}" "${TMP_DIR}/ci-secret.yml" 's|persist-credentials: false|persist-credentials: ${{ secrets.X }}|'
rc=0
"${CHECK}" --ci "${TMP_DIR}/ci-secret.yml" >/dev/null 2>&1 || rc=$?
if command -v ruby >/dev/null 2>&1; then
  if [ "${rc}" = 1 ]; then echo "ok - secrets.X in ci.yml is rejected via B1 (exit 1)"; passed=$((passed + 1)); else echo "not ok - B1 (expected exit 1, got ${rc})"; failed=$((failed + 1)); fi
  rc=0
  "${CHECK}" >/dev/null 2>&1 || rc=$?
  if [ "${rc}" = 0 ]; then echo "ok - the real repo passes with the real B1 (exit 0)"; passed=$((passed + 1)); else echo "not ok - real repo with real B1 (expected exit 0, got ${rc})"; failed=$((failed + 1)); fi
else
  # ruby が無ければ B1 は完了できない。合格にせず 2 で止まる（fail-closed）ことを確かめる
  if [ "${rc}" = 2 ]; then echo "ok - B1 without ruby stops with exit 2 (fail-closed)"; passed=$((passed + 1)); else echo "not ok - B1 without ruby (expected exit 2, got ${rc})"; failed=$((failed + 1)); fi
fi
# stub が 2 を返したら、合格にせず 2 で止まる
printf '#!/bin/sh\nexit 2\n' >"${TMP_DIR}/stub2.sh"
chmod +x "${TMP_DIR}/stub2.sh"
rc=0
"${CHECK}" --secrets-checker "${TMP_DIR}/stub2.sh" >/dev/null 2>&1 || rc=$?
if [ "${rc}" = 2 ]; then echo "ok - a secrets checker failure (exit 2) is not a pass (exit 2)"; passed=$((passed + 1)); else echo "not ok - secrets checker exit 2 (expected 2, got ${rc})"; failed=$((failed + 1)); fi

echo "# C: deploy detachment"
variant "${GOOD_CI}" "${TMP_DIR}/ci-write.yml" 's|contents: read|contents: write|'
case_ "write permission in ci.yml is rejected (C1)" 1 C1 --ci "${TMP_DIR}/ci-write.yml"
variant "${GOOD_CI}" "${TMP_DIR}/ci-writeall.yml" 's|^permissions:|permissions: write-all|'
case_ "write-all in ci.yml is rejected (C1)" 1 C1 --ci "${TMP_DIR}/ci-writeall.yml"
printf '    environment: prod\n' >"${TMP_DIR}/t-env"
splice "${GOOD_CI}" "${TMP_DIR}/ci-env.yml" after '    runs-on: ubuntu-latest' "${TMP_DIR}/t-env"
case_ "environment in ci.yml is rejected (C1)" 1 C1 --ci "${TMP_DIR}/ci-env.yml"
variant "${GOOD_CI}" "${TMP_DIR}/ci-deploy-job.yml" 's|^  check:|  deploy:|'
case_ "deploy job in ci.yml is rejected (C3)" 1 C3 --ci "${TMP_DIR}/ci-deploy-job.yml"
for trig in pull_request pull_request_target push schedule workflow_run; do
  printf '  %s:\n' "${trig}" >"${TMP_DIR}/t-trig"
  splice "${GOOD_DEPLOY}" "${TMP_DIR}/deploy-${trig}.yml" after '  workflow_dispatch:' "${TMP_DIR}/t-trig"
  case_ "deploy.yml with ${trig} trigger is rejected (C2)" 1 C2 --deploy "${TMP_DIR}/deploy-${trig}.yml"
done
variant "${GOOD_DEPLOY}" "${TMP_DIR}/deploy-inline-on.yml" 's|^on:$|on: [push, workflow_dispatch]|;/^  workflow_dispatch:$/d'
case_ "deploy.yml with an inline on: is rejected (C2)" 1 C2 --deploy "${TMP_DIR}/deploy-inline-on.yml"
printf 'permissions:\n  contents: read\n' >"${TMP_DIR}/t-perm"
splice "${GOOD_DEPLOY}" "${TMP_DIR}/deploy-perm.yml" eq 'permissions: {}' "${TMP_DIR}/t-perm"
case_ "deploy.yml with non-empty top-level permissions is rejected (C2)" 1 C2 --deploy "${TMP_DIR}/deploy-perm.yml"
variant "${GOOD_DEPLOY}" "${TMP_DIR}/deploy-noguard.yml" '/^    if: /d'
case_ "deploy job without the fork guard is rejected (C2)" 1 C2 --deploy "${TMP_DIR}/deploy-noguard.yml"
variant "${GOOD_DEPLOY}" "${TMP_DIR}/deploy-otherguard.yml" "s|Fandhe-AI/fandhe-workshop|someone/else|"
case_ "deploy job guarded by another repo is rejected (C2)" 1 C2 --deploy "${TMP_DIR}/deploy-otherguard.yml"
{ cat "${GOOD_DEPLOY}"; printf '  second:\n    runs-on: ubuntu-latest\n    steps:\n      - run: exit 1\n'; } >"${TMP_DIR}/deploy-second.yml"
case_ "a second deploy job without the guard is rejected (C2)" 1 C2 --deploy "${TMP_DIR}/deploy-second.yml"

echo "# D: ENGINE_REV"
printf 'abc\n' >"${TMP_DIR}/rev-short"
case_ "short ENGINE_REV is rejected (D1)" 1 D1 --pack-workflow "${GOOD_PACK}" --engine-rev "${TMP_DIR}/rev-short"
printf '%s\n' "0123456789ABCDEF0123456789ABCDEF01234567" >"${TMP_DIR}/rev-upper"
case_ "uppercase ENGINE_REV is rejected (D1)" 1 D1 --pack-workflow "${GOOD_PACK}" --engine-rev "${TMP_DIR}/rev-upper"
printf '%s \n' "${REV}" >"${TMP_DIR}/rev-space"
case_ "ENGINE_REV with trailing space is rejected (D1)" 1 D1 --pack-workflow "${GOOD_PACK}" --engine-rev "${TMP_DIR}/rev-space"
printf '%s\n%s\n' "${REV}" "${REV}" >"${TMP_DIR}/rev-two"
case_ "two-line ENGINE_REV is rejected (D1)" 1 D1 --pack-workflow "${GOOD_PACK}" --engine-rev "${TMP_DIR}/rev-two"
# shellcheck disable=SC2016 # sed の式なので、シェルに展開させない
variant "${GOOD_PACK}" "${TMP_DIR}/pack-main.yml" 's|ref: \${{ steps.rev.outputs.rev }}|ref: main|'
case_ "ref: main is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-main.yml" --engine-rev "${GOOD_REV}"
variant "${GOOD_PACK}" "${TMP_DIR}/pack-noref.yml" '/ref: /d'
case_ "engine checkout without ref is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-noref.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # sed の式なので、シェルに展開させない
variant "${GOOD_PACK}" "${TMP_DIR}/pack-latest.yml" 's|ref: \${{ steps.rev.outputs.rev }}|ref: latest|'
case_ "ref: latest is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-latest.yml" --engine-rev "${GOOD_REV}"
variant "${GOOD_PACK}" "${TMP_DIR}/pack-otherrev.yml" "s|\${{ steps.rev.outputs.rev }}|1111111111111111111111111111111111111111|"
case_ "literal ref different from ENGINE_REV is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-otherrev.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # sed の式なので、シェルに展開させない
variant "${GOOD_PACK}" "${TMP_DIR}/pack-noread.yml" 's|\$(cat ENGINE_REV)|abc|'
case_ "ref from a step that does not read ENGINE_REV is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-noread.yml" --engine-rev "${GOOD_REV}"
variant "${GOOD_PACK}" "${TMP_DIR}/pack-norepo.yml" 's|repository: Fandhe-AI/fandhe-workshop|repository: Fandhe-AI/pack-x|'
case_ "no engine checkout is rejected (D2)" 1 D2 --pack-workflow "${TMP_DIR}/pack-norepo.yml" --engine-rev "${GOOD_REV}"
{ cat "${GOOD_PACK}"; printf '      - name: Clone engine\n        run: git clone https://github.com/Fandhe-AI/fandhe-workshop engine2\n'; } >"${TMP_DIR}/pack-clone.yml"
case_ "git clone of the engine without ENGINE_REV is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-clone.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # 見本の文字列なので、シェルに展開させない
{ cat "${GOOD_PACK}"; printf '      - name: Clone engine\n        run: |\n          git clone https://github.com/Fandhe-AI/fandhe-workshop engine2\n          git -C engine2 checkout "$(cat ENGINE_REV)"\n'; } >"${TMP_DIR}/pack-clone-ok.yml"
case_ "git clone of the engine that reads ENGINE_REV is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-clone-ok.yml" --engine-rev "${GOOD_REV}"

# D4: ENGINE_REV を表示するだけの行は版の指定とみなさない
# shellcheck disable=SC2016 # 見本の文字列なので、シェルに展開させない
{ cat "${GOOD_PACK}"; printf '      - name: Clone engine\n        run: |\n          git clone https://github.com/Fandhe-AI/fandhe-workshop engine2\n          cat ENGINE_REV\n'; } >"${TMP_DIR}/pack-clone-cat.yml"
case_ "git clone followed by a bare cat ENGINE_REV is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-clone-cat.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # 見本の文字列なので、シェルに展開させない
{ cat "${GOOD_PACK}"; printf '      - name: Clone engine\n        run: git clone https://github.com/Fandhe-AI/fandhe-workshop engine2 && echo "$(cat ENGINE_REV)"\n'; } >"${TMP_DIR}/pack-clone-echo.yml"
case_ "git clone with ENGINE_REV only echoed is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-clone-echo.yml" --engine-rev "${GOOD_REV}"

# D3: 別ジョブ・後ろの step の出力は承認しない。step ID が "outputs" を含んでも誤って分割しない
cat >"${TMP_DIR}/pack-otherjob.yml" <<EOF
name: Build pack
on:
  push:
jobs:
  prep:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@${SHA} # v7.0.1
        with:
          persist-credentials: false
      - name: Read engine revision
        id: rev
        run: echo "rev=\$(cat ENGINE_REV)" >> "\$GITHUB_OUTPUT"
  build:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@${SHA} # v7.0.1
        with:
          persist-credentials: false
      - name: Checkout engine
        uses: actions/checkout@${SHA} # v7.0.1
        with:
          repository: Fandhe-AI/fandhe-workshop
          ref: \${{ steps.rev.outputs.rev }}
          persist-credentials: false
EOF
case_ "ref from a step of another job is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-otherjob.yml" --engine-rev "${GOOD_REV}"
cat >"${TMP_DIR}/pack-later.yml" <<EOF
name: Build pack
on:
  push:
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@${SHA} # v7.0.1
        with:
          persist-credentials: false
      - name: Checkout engine
        uses: actions/checkout@${SHA} # v7.0.1
        with:
          repository: Fandhe-AI/fandhe-workshop
          ref: \${{ steps.rev.outputs.rev }}
          path: engine
          persist-credentials: false
      - name: Read engine revision
        id: rev
        run: echo "rev=\$(cat ENGINE_REV)" >> "\$GITHUB_OUTPUT"
EOF
case_ "ref from a step that runs after the checkout is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-later.yml" --engine-rev "${GOOD_REV}"
variant "${GOOD_PACK}" "${TMP_DIR}/pack-id-outputs.yml" 's|id: rev|id: read_outputs|;s|steps\.rev\.outputs\.rev|steps.read_outputs.outputs.rev|'
case_ "a step id containing 'outputs' passes (D3)" 0 - --pack-workflow "${TMP_DIR}/pack-id-outputs.yml" --engine-rev "${GOOD_REV}"
variant "${GOOD_PACK}" "${TMP_DIR}/pack-dotless.yml" 's|steps\.rev\.outputs\.rev|steps.rev-outputs-rev|'
case_ "a ref with a character in place of the dot is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-dotless.yml" --engine-rev "${GOOD_REV}"
variant "${GOOD_PACK}" "${TMP_DIR}/pack-dotany.yml" 's|id: rev|id: readXoutputsXrev|;s|steps\.rev\.outputs\.rev|steps.readXoutputsXrev.outputs.rev|'
case_ "a step id with X in place of dots passes only by its own output (D3)" 0 - --pack-workflow "${TMP_DIR}/pack-dotany.yml" --engine-rev "${GOOD_REV}"

echo "# review follow-ups"
variant "${GOOD_DEPLOY}" "${TMP_DIR}/deploy-ortrue.yml" "s/^\\(    if: .*\\)\$/\\1 || true/"
case_ "fork guard with '|| true' is rejected (C2)" 1 C2 --deploy "${TMP_DIR}/deploy-ortrue.yml"
# 同じ step に別の値の出力を足す見本（id: rev の step 内に書く）
# shellcheck disable=SC2016 # sed の式なので、シェルに展開させない
printf '        run: |\n          echo "rev=main" >> "${GITHUB_OUTPUT}"\n          echo "rev=$(cat ENGINE_REV)" >> "${GITHUB_OUTPUT}"\n' >"${TMP_DIR}/t-run"
splice "${GOOD_PACK}" "${TMP_DIR}/pack-taint2.yml" prefix '        run: echo "rev=' "${TMP_DIR}/t-run"
case_ "output also written with another value is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-taint2.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # sed の式なので、シェルに展開させない
sed 's|^\(        run: echo "rev=.*\)$|        run: echo "rev=main" >> "${GITHUB_OUTPUT}" # ENGINE_REV|' "${GOOD_PACK}" >"${TMP_DIR}/pack-commentonly.yml"
case_ "ENGINE_REV only in a comment is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-commentonly.yml" --engine-rev "${GOOD_REV}"
# D4: ENGINE_REV を保存先の位置に置くだけの形は版の指定とみなさない
# shellcheck disable=SC2016 # 見本の文字列なので、シェルに展開させない
{ cat "${GOOD_PACK}"; printf '      - name: Clone engine\n        run: git clone https://github.com/Fandhe-AI/fandhe-workshop "$(cat ENGINE_REV)"\n'; } >"${TMP_DIR}/pack-clone-dest.yml"
case_ "git clone with ENGINE_REV as the destination is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-clone-dest.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # 見本の文字列なので、シェルに展開させない
{ cat "${GOOD_PACK}"; printf '      - name: Install engine\n        run: cargo install --git https://github.com/Fandhe-AI/fandhe-workshop --rev "$(cat ENGINE_REV)" fandhe-pack\n'; } >"${TMP_DIR}/pack-cargo-rev.yml"
case_ "cargo install with --rev ENGINE_REV is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-cargo-rev.yml" --engine-rev "${GOOD_REV}"

# D3: ブロック形式の run のコメントにある ENGINE_REV は出力の根拠にしない
# shellcheck disable=SC2016 # 見本の文字列なので、シェルに展開させない
printf '        run: |\n          echo "rev=main" >> "${GITHUB_OUTPUT}" # rev=$(cat ENGINE_REV) >>\n' >"${TMP_DIR}/t-run2"
splice "${GOOD_PACK}" "${TMP_DIR}/pack-blockcomment.yml" prefix '        run: echo "rev=' "${TMP_DIR}/t-run2"
case_ "ENGINE_REV only in a block-run comment is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-blockcomment.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # 見本の文字列なので、シェルに展開させない
printf '        run: |\n          echo "rev=$(cat ENGINE_REV)" >> "${GITHUB_OUTPUT}" # pinned\n' >"${TMP_DIR}/t-run3"
splice "${GOOD_PACK}" "${TMP_DIR}/pack-blockok.yml" prefix '        run: echo "rev=' "${TMP_DIR}/t-run3"
case_ "block run with a trailing comment on the pinned write is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-blockok.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # 見本の文字列なので、シェルに展開させない
printf '        run: |\n          echo "rev=$(cat ENGINE_REV)" >> "${GITHUB_OUTPUT}"; echo "rev=main" >> "${GITHUB_OUTPUT}"\n' >"${TMP_DIR}/t-run4"
splice "${GOOD_PACK}" "${TMP_DIR}/pack-oneline.yml" prefix '        run: echo "rev=' "${TMP_DIR}/t-run4"
case_ "two writes of one output on one line are rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-oneline.yml" --engine-rev "${GOOD_REV}"
# review follow-ups 2: shell quoting, clone destinations, YAML-quoted permissions
# shellcheck disable=SC2016 # 見本の文字列なので、シェルに展開させない
printf '        run: |\n          echo '"'"'rev=$(cat ENGINE_REV)'"'"' >> "${GITHUB_OUTPUT}"\n' >"${TMP_DIR}/t-run5"
splice "${GOOD_PACK}" "${TMP_DIR}/pack-sq-block.yml" prefix '        run: echo "rev=' "${TMP_DIR}/t-run5"
case_ "single-quoted command substitution in a block run is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-sq-block.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # 見本の文字列なので、シェルに展開させない
printf '        run: echo '"'"'rev=$(cat ENGINE_REV)'"'"' >> "${GITHUB_OUTPUT}"\n' >"${TMP_DIR}/t-run6"
splice "${GOOD_PACK}" "${TMP_DIR}/pack-sq-inline.yml" prefix '        run: echo "rev=' "${TMP_DIR}/t-run6"
case_ "single-quoted command substitution in an inline run is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-sq-inline.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # 見本の文字列なので、シェルに展開させない
printf '        run: |\n          echo rev=$(cat ENGINE_REV) >> "${GITHUB_OUTPUT}"\n' >"${TMP_DIR}/t-run7"
splice "${GOOD_PACK}" "${TMP_DIR}/pack-unquoted.yml" prefix '        run: echo "rev=' "${TMP_DIR}/t-run7"
case_ "unquoted command substitution in a block run is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-unquoted.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # 見本の文字列なので、シェルに展開させない
printf '        run: "echo \\"rev=$(cat ENGINE_REV)\\" >> \\"${GITHUB_OUTPUT}\\""\n' >"${TMP_DIR}/t-run8"
splice "${GOOD_PACK}" "${TMP_DIR}/pack-yamlq.yml" prefix '        run: echo "rev=' "${TMP_DIR}/t-run8"
case_ "YAML-quoted run with a double-quoted substitution is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-yamlq.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # 見本の文字列なので、シェルに展開させない
{ cat "${GOOD_PACK}"; printf '      - name: Clone engine\n        run: |\n          git clone https://github.com/Fandhe-AI/fandhe-workshop engine2\n          git -C other checkout "$(cat ENGINE_REV)"\n'; } >"${TMP_DIR}/pack-clone-other.yml"
case_ "pin applied to another directory than the clone is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-clone-other.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # 見本の文字列なので、シェルに展開させない
{ cat "${GOOD_PACK}"; printf '      - name: Clone engine\n        run: |\n          git -C engine2 checkout "$(cat ENGINE_REV)"\n          git clone https://github.com/Fandhe-AI/fandhe-workshop engine2\n'; } >"${TMP_DIR}/pack-clone-before.yml"
case_ "pin written before the clone is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-clone-before.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # 見本の文字列なので、シェルに展開させない
{ cat "${GOOD_PACK}"; printf '      - name: Clone engine\n        run: |\n          git clone https://github.com/Fandhe-AI/fandhe-workshop engine2\n          git clone https://github.com/Fandhe-AI/fandhe-workshop engine3\n          git -C engine2 checkout "$(cat ENGINE_REV)"\n'; } >"${TMP_DIR}/pack-clone-two.yml"
case_ "two clones with only one pinned are rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-clone-two.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # 見本の文字列なので、シェルに展開させない
{ cat "${GOOD_PACK}"; printf '      - name: Clone engine\n        run: |\n          git clone https://github.com/Fandhe-AI/fandhe-workshop engine2\n          cd engine2 && git checkout "$(cat ENGINE_REV)"\n'; } >"${TMP_DIR}/pack-clone-cd.yml"
case_ "clone then cd into it and checkout ENGINE_REV is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-clone-cd.yml" --engine-rev "${GOOD_REV}"
variant "${GOOD_CI}" "${TMP_DIR}/ci-write-dq.yml" 's|contents: read|contents: "write"|'
case_ "double-quoted write permission is rejected (C1)" 1 C1 --ci "${TMP_DIR}/ci-write-dq.yml"
variant "${GOOD_CI}" "${TMP_DIR}/ci-write-sq.yml" "s|contents: read|contents: 'write'|"
case_ "single-quoted write permission is rejected (C1)" 1 C1 --ci "${TMP_DIR}/ci-write-sq.yml"
variant "${GOOD_CI}" "${TMP_DIR}/ci-write-flow.yml" 's|^permissions:$|permissions: {contents: "write"}|;/^  contents: read$/d'
case_ "flow-style quoted write permission is rejected (C1)" 1 C1 --ci "${TMP_DIR}/ci-write-flow.yml"
variant "${GOOD_CI}" "${TMP_DIR}/ci-write-quoted-all.yml" 's|^permissions:$|permissions: "write-all"|'
case_ "quoted write-all is rejected (C1)" 1 C1 --ci "${TMP_DIR}/ci-write-quoted-all.yml"

printf '{"packages":{"node_modules/a":{"resolved":"https:%suser:pw%snpm.example.invalid/a.tgz"}}}\n' "//" "${AT}" >"${TMP_DIR}/npm-cred.json"
"${CHECK}" --secrets-checker "${STUB}" --package-lock "${TMP_DIR}/npm-cred.json" >/dev/null 2>"${TMP_DIR}/err" || true
if grep -q 'pw' "${TMP_DIR}/err"; then
  echo "not ok - A2 output leaks the resolved value"
  failed=$((failed + 1))
else
  echo "ok - A2 output does not contain the resolved value"
  passed=$((passed + 1))
fi

# grep が 2 で失敗したら、$(...) の中でも握りつぶさず 2 で止まる（A3 の grep だけを失敗させる shim）
mkdir -p "${TMP_DIR}/shim"
REAL_GREP="$(command -v grep)"
# shellcheck disable=SC2016 # shim の本文なので、シェルに展開させない
printf '#!/bin/sh\ncase "$*" in *_authtoken*) exit 2 ;; esac\nexec "%s" "$@"\n' "${REAL_GREP}" >"${TMP_DIR}/shim/grep"
chmod +x "${TMP_DIR}/shim/grep"
rc=0
PATH="${TMP_DIR}/shim:${PATH}" "${CHECK}" --secrets-checker "${STUB}" --npmrc "${TMP_DIR}/npmrc-ok" >/dev/null 2>&1 || rc=$?
if [ "${rc}" = 2 ]; then echo "ok - a grep failure inside a report is not a pass (exit 2)"; passed=$((passed + 1)); else echo "not ok - grep failure (expected exit 2, got ${rc})"; failed=$((failed + 1)); fi

# review follow-ups 3: single quotes, non-switching operations, conditional output steps, clone destinations
URL='https://github.com/Fandhe-AI/fandhe-workshop'
d4_n=0
# d4_case <name> <expected-rc> <run-lines...>: 良い見本に run のステップを足して流す
d4_case() {
  local name="$1" want="$2" f="${TMP_DIR}/d4-$((++d4_n)).yml" tag="D4"
  shift 2
  [ "${want}" = 0 ] && tag="-"
  { cat "${GOOD_PACK}"; printf '      - name: Clone engine\n        run: |\n'; printf '          %s\n' "$@"; } >"${f}"
  case_ "${name}" "${want}" "${tag}" --pack-workflow "${f}" --engine-rev "${GOOD_REV}"
}
# shellcheck disable=SC2016 # 見本の本文なので、シェルに展開させない
{
  d4_case "single-quoted command substitution is rejected (D4)" 1 "git clone ${URL} engine2" "git -C engine2 checkout '\$(cat ENGINE_REV)'"
  d4_case "double-quoted command substitution after a clone is rejected (D4)" 1 "git clone ${URL} engine2" "git -C engine2 checkout \"\$(cat ENGINE_REV)\""
  d4_case "fetch alone does not pin (D4)" 1 "git clone ${URL} engine2" "git -C engine2 fetch origin \"\$(cat ENGINE_REV)\""
  d4_case "reset without --hard does not pin (D4)" 1 "git clone ${URL} engine2" "git -C engine2 reset \"\$(cat ENGINE_REV)\""
  d4_case "reset --hard after a clone is rejected (D4)" 1 "git clone ${URL} engine2" "git -C engine2 reset --hard \"\$(cat ENGINE_REV)\""
  d4_case "checkout -- <path form> does not pin (D4)" 1 "git clone ${URL} engine2" "git -C engine2 checkout -- \"\$(cat ENGINE_REV)\""
  d4_case "switch --detach after a clone is rejected (D4)" 1 "git clone ${URL} engine2" "git -C engine2 switch --detach \"\$(cat ENGINE_REV)\""
  d4_case "clone into . then checkout ENGINE_REV is rejected (D4)" 1 "git clone ${URL} ." "git checkout \"\$(cat ENGINE_REV)\""
  d4_case "clone into . without a pin is rejected (D4)" 1 "git clone ${URL} ."
  d4_case "clone with flags after the URL is rejected (D4)" 1 "git clone ${URL} --depth 1 engine2" "git -C engine2 checkout \"\$(cat ENGINE_REV)\""
  d4_case "clone into the default directory is rejected (D4)" 1 "git clone ${URL} --recursive" "git -C fandhe-workshop checkout \"\$(cat ENGINE_REV)\""
  d4_case "clone flag without a destination is not pinned elsewhere (D4)" 1 "git clone ${URL} --recursive" "git -C --recursive checkout \"\$(cat ENGINE_REV)\""
}
# 出力元の step が条件つきなら、出力は固定済みと認めない
{ sed -e '/^        id: rev$/a\
        if: ${{ false }}' "${GOOD_PACK}"; } >"${TMP_DIR}/pack-if-false.yml"
case_ "output step with an if condition is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-if-false.yml" --engine-rev "${GOOD_REV}"

# review follow-ups 4: 制御構造・条件演算子・取得先ごとの固定の解除
# shellcheck disable=SC2016 # 見本の本文なので、シェルに展開させない
{
  d4_case "conditional checkout after && false is rejected (D4)" 1 "git clone ${URL} engine2" "false && git -C engine2 checkout \"\$(cat ENGINE_REV)\""
  d4_case "checkout after || is rejected (D4)" 1 "git clone ${URL} engine2" "true || git -C engine2 checkout \"\$(cat ENGINE_REV)\""
  d4_case "checkout inside an if is rejected (D4)" 1 "git clone ${URL} engine2" "if false; then git -C engine2 checkout \"\$(cat ENGINE_REV)\"; fi"
  d4_case "checkout main after the pin unpins the destination (D4)" 1 "git clone ${URL} engine2" "git -C engine2 checkout \"\$(cat ENGINE_REV)\"" "git -C engine2 checkout main"
  d4_case "reset to another ref after the pin unpins the destination (D4)" 1 "git clone ${URL} engine2" "git -C engine2 checkout \"\$(cat ENGINE_REV)\"" "git -C engine2 reset --hard origin/main"
  d4_case "pin after switching to main again is rejected (D4)" 1 "git clone ${URL} engine2" "git -C engine2 checkout main" "git -C engine2 checkout \"\$(cat ENGINE_REV)\""
  d4_case "checkout main in another directory after a pin is rejected (D4)" 1 "git clone ${URL} engine2" "git -C engine2 checkout \"\$(cat ENGINE_REV)\"" "git -C other checkout main"
}
d3_n=0
# d3_case <name> <expected-rc> <run-lines...>: 出力を書く step の run を差し替えた見本を流す
d3_case() {
  local name="$1" want="$2" f="${TMP_DIR}/d3-$((++d3_n)).yml" tag="D3"
  shift 2
  [ "${want}" = 0 ] && tag="-"
  {
    printf 'name: Build pack\non:\n  push:\njobs:\n  build:\n    runs-on: ubuntu-latest\n    steps:\n'
    printf '      - name: Checkout\n        uses: actions/checkout@%s # v7.0.1\n        with:\n          persist-credentials: false\n' "${SHA}"
    printf '      - name: Read engine revision\n        id: rev\n        run: |\n'
    printf '          %s\n' "$@"
    # shellcheck disable=SC2016 # YAML の式なので、シェルに展開させない
    printf '      - name: Checkout engine\n        uses: actions/checkout@%s # v7.0.1\n        with:\n          repository: Fandhe-AI/fandhe-workshop\n          ref: ${{ steps.rev.outputs.rev }}\n          path: engine\n          persist-credentials: false\n' "${SHA}"
  } >"${f}"
  case_ "${name}" "${want}" "${tag}" --pack-workflow "${f}" --engine-rev "${GOOD_REV}"
}
# shellcheck disable=SC2016 # 見本の本文なので、シェルに展開させない
{
  d3_case "canonical output line in a block run passes (D3)" 0 'echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"'
  d3_case "output line with braces around GITHUB_OUTPUT is rejected (D3)" 1 'echo "rev=$(cat ENGINE_REV)" >> "${GITHUB_OUTPUT}"'
  d3_case "pinned output inside an if is rejected (D3)" 1 'if false; then' '  echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"' 'fi'
  d3_case "pinned output inside a case is rejected (D3)" 1 'case x in y) echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT" ;; esac'
  d3_case "pinned output after && false is rejected (D3)" 1 'false && echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"'
  d3_case "pinned output after an exit is rejected (D3)" 1 'exit 0' 'echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"'
  d3_case "canonical line inside a here-document is rejected (D3)" 1 "cat <<'EOF'" 'echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"' 'EOF'
}

# review follow-ups 5: 行をまたぐ条件・後続 step による版の変更
# d4_steps <name> <expected-rc> <run-lines...>: 良い見本（path: engine の checkout）に run の step を足して流す。
# 引数 "--" で次の step に移る。
d4_steps() {
  local name="$1" want="$2" f="${TMP_DIR}/d4-$((++d4_n)).yml" tag="D4" a
  shift 2
  [ "${want}" = 0 ] && tag="-"
  {
    cat "${GOOD_PACK}"
    printf '      - name: Engine step\n        run: |\n'
    for a in "$@"; do
      if [ "${a}" = "--" ]; then printf '      - name: Engine step\n        run: |\n'; else printf '          %s\n' "${a}"; fi
    done
  } >"${f}"
  case_ "${name}" "${want}" "${tag}" --pack-workflow "${f}" --engine-rev "${GOOD_REV}"
}
# shellcheck disable=SC2016 # 見本の本文なので、シェルに展開させない
PIN='"$(cat ENGINE_REV)"'
# shellcheck disable=SC2016 # 見本の本文なので、シェルに展開させない
{
  d4_case "checkout after a line ending with || is rejected (D4)" 1 "git clone ${URL} engine2" "true ||" "  git -C engine2 checkout ${PIN}"
  d4_case "checkout after a line ending with && is rejected (D4)" 1 "git clone ${URL} engine2" "false &&" "  git -C engine2 checkout ${PIN}"
  d4_case "checkout on a line starting with || is rejected (D4)" 1 "git clone ${URL} engine2" "true" "|| git -C engine2 checkout ${PIN}"
  d4_case "checkout inside a here-document is rejected (D4)" 1 "git clone ${URL} engine2" "cat <<EOF" "git -C engine2 checkout ${PIN}" "EOF"
  d4_case "checkout inside a multi-line quote is rejected (D4)" 1 "git clone ${URL} engine2" "echo '" "git -C engine2 checkout ${PIN}" "'"
  d4_case "backslash-continued && chain is rejected (D4)" 1 "git clone ${URL} engine2 && \\" "  git -C engine2 checkout ${PIN}"
  d4_steps "later step checks out main in the pinned clone (D4)" 1 "git clone ${URL} engine2" "git -C engine2 checkout ${PIN}" -- "git -C engine2 checkout main"
  d4_steps "later step checks out main in the engine checkout path (D4)" 1 "git -C engine checkout main"
  d4_steps "later step pulls after cd into the engine checkout (D4)" 1 "cd engine && git pull"
  d4_steps "later step fetches inside a nested shell (D4)" 1 "sh -c 'cd engine && git fetch origin main'"
  d4_steps "later git operation on an undetermined directory is rejected (D4)" 1 'git -C "$DIR" checkout main'
  d4_steps "later git operation via an alias is rejected (D4)" 1 "git -C engine -c alias.co=checkout co main"
  d4_steps "later read-only git command on the engine is rejected (D4)" 1 "git -C engine status" "git -C engine log --oneline -1"
  d4_steps "later git checkout in another directory is rejected (D4)" 1 "git -C other checkout main"
  d4_steps "later read-only command on a pinned clone is rejected (D4)" 1 "git clone ${URL} engine2" "git -C engine2 checkout ${PIN}" -- "git -C engine2 rev-parse HEAD"
}
# 折り返す YAML スカラー（>）は行の区切りが実行と食い違うので、固定の形でも拒否する
# shellcheck disable=SC2016 # 見本の本文なので、シェルに展開させない
{ cat "${GOOD_PACK}"; printf '      - name: Clone engine\n        run: >\n          git clone %s engine2 &&\n          git -C engine2 checkout "$(cat ENGINE_REV)"\n' "${URL}"; } >"${TMP_DIR}/pack-folded.yml"
case_ "folded run scalar that fetches the engine is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-folded.yml" --engine-rev "${GOOD_REV}"
# 後続の actions/checkout が取得済みのエンジンの取得先へ書く
{ cat "${GOOD_PACK}"; printf '      - name: Overwrite engine\n        uses: actions/checkout@%s # v7.0.1\n        with:\n          repository: Fandhe-AI/pack-x\n          path: engine\n          persist-credentials: false\n' "${SHA}"; } >"${TMP_DIR}/pack-overwrite.yml"
case_ "later checkout into the engine path is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-overwrite.yml" --engine-rev "${GOOD_REV}"
{ cat "${GOOD_PACK}"; printf '      - name: Pack\n        uses: actions/checkout@%s # v7.0.1\n        with:\n          repository: Fandhe-AI/pack-x\n          path: pack\n          persist-credentials: false\n' "${SHA}"; } >"${TMP_DIR}/pack-sibling.yml"
# 後続の checkout は保存先を解釈せず一律に拒否する（許可リスト）
case_ "later checkout into a sibling path is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-sibling.yml" --engine-rev "${GOOD_REV}"

# review follow-ups 6: 正準の形の許可リスト（表示だけの git・here-document・作業ディレクトリ・式の権限）
echo "# canonical form"
# shellcheck disable=SC2016 # 見本の本文なので、シェルに展開させない
{
  d4_case "display-only echo git after a clone is rejected (D4)" 1 "git clone ${URL} engine2" "echo git -C engine2 checkout ${PIN}"
  d4_case "no-op ': git' after a clone is rejected (D4)" 1 "git clone ${URL} engine2" ": git -C engine2 checkout ${PIN}"
  d4_steps "display-only echo git in a later step is rejected (D4)" 1 "echo git -C engine checkout ${PIN}"
  d4_steps "later step that mentions ENGINE_REV is rejected (D4)" 1 "echo main > ENGINE_REV"
  d4_steps "later step that refers to the engine path is rejected (D4)" 1 "cargo build --manifest-path engine/Cargo.toml"
  d4_steps "later unrelated step passes (D4)" 0 "cargo build --release"
}
# shellcheck disable=SC2016 # YAML の式なので、シェルに展開させない
{ cat "${GOOD_PACK}"; printf '      - name: Cache\n        uses: actions/cache@%s # v4\n        with:\n          path: ${{ runner.temp }}/x\n' "${SHA}"; } >"${TMP_DIR}/pack-expr-path.yml"
case_ "later step with an expression in path is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-expr-path.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # YAML の式なので、シェルに展開させない
{ cat "${GOOD_PACK}"; printf '  build2:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@%s # v7.0.1\n        with:\n          repository: ${{ vars.ENGINE }}\n          ref: main\n          persist-credentials: false\n' "${SHA}"; } >"${TMP_DIR}/pack-expr-repo.yml"
case_ "a job whose checkout repository is an expression is checked and rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-expr-repo.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # YAML の式なので、シェルに展開させない
{ cat "${GOOD_PACK}"; printf '      - uses: actions/checkout@%s # v7.0.1\n        with:\n          repository: ${{ vars.ENGINE }}\n          path: other\n          persist-credentials: false\n' "${SHA}"; } >"${TMP_DIR}/pack-expr-repo-later.yml"
case_ "later checkout whose repository is an expression is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-expr-repo-later.yml" --engine-rev "${GOOD_REV}"
{ cat "${GOOD_PACK}"; printf '  lint:\n    runs-on: ubuntu-latest\n    env:\n      X: y\n    steps:\n      - run: git status\n'; } >"${TMP_DIR}/pack-other-job.yml"
case_ "a job that does not refer to the engine is not checked (D)" 0 - --pack-workflow "${TMP_DIR}/pack-other-job.yml" --engine-rev "${GOOD_REV}"
printf 'name: Build pack\non:\n  push:\njobs:\n  lint:\n    runs-on: ubuntu-latest\n    steps:\n      - run: make check\n' >"${TMP_DIR}/pack-no-engine.yml"
case_ "workflow without an engine job is rejected (D2)" 1 D2 --pack-workflow "${TMP_DIR}/pack-no-engine.yml" --engine-rev "${GOOD_REV}"
# pack_insert <name> <out> <mode> <pattern> <text>: 良い見本へ text を差し込んだ見本で D3 の拒否を確かめる（mode は splice と同じ）
pack_insert() {
  printf '%s' "$5" >"${TMP_DIR}/t-ins"
  splice "${GOOD_PACK}" "$2" "$3" "$4" "${TMP_DIR}/t-ins"
  case_ "$1" 1 D3 --pack-workflow "$2" --engine-rev "${GOOD_REV}"
}
pack_insert "output step with working-directory is rejected (D3)" "${TMP_DIR}/pack-wd.yml" after '        id: rev' '        working-directory: other
'
pack_insert "output step with shell is rejected (D3)" "${TMP_DIR}/pack-shell.yml" after '        id: rev' '        shell: bash
'
pack_insert "output step with env is rejected (D3)" "${TMP_DIR}/pack-step-env.yml" after '        id: rev' '        env:
          BASH_ENV: x
'
pack_insert "job-level defaults are rejected (D3)" "${TMP_DIR}/pack-job-defaults.yml" after '    runs-on: ubuntu-latest' '    defaults:
      run:
        working-directory: other
'
pack_insert "workflow-level defaults are rejected (D3)" "${TMP_DIR}/pack-wf-defaults.yml" eq 'jobs:' 'defaults:
  run:
    working-directory: other
jobs:
'
pack_insert "job-level env is rejected (D3)" "${TMP_DIR}/pack-job-env.yml" after '    runs-on: ubuntu-latest' '    env:
      BASH_ENV: x
'
pack_insert "workflow-level env is rejected (D3)" "${TMP_DIR}/pack-wf-env.yml" eq 'jobs:' 'env:
  BASH_ENV: x
jobs:
'
# shellcheck disable=SC2016 # 見本の本文なので、シェルに展開させない
pack_insert "unrelated step before the output step is rejected (D3)" "${TMP_DIR}/pack-before.yml" eq '      - name: Read engine revision' '      - name: Env
        run: echo "BASH_ENV=/tmp/x" >> "$GITHUB_ENV"
      - name: Read engine revision
'
pack_insert "engine checkout with an extra input is rejected (D3)" "${TMP_DIR}/pack-extra-input.yml" after '          path: engine' '          fetch-depth: 0
'
awk '{ print } /persist-credentials: false/ && !done { print "          path: pack"; done = 1 }' "${GOOD_PACK}" >"${TMP_DIR}/pack-own-path.yml"
case_ "checkout of this repository with a path is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-own-path.yml" --engine-rev "${GOOD_REV}"
variant "${GOOD_PACK}" "${TMP_DIR}/pack-no-own.yml" '/^      - name: Checkout$/,/^          persist-credentials: false$/d'
case_ "missing checkout of this repository is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-no-own.yml" --engine-rev "${GOOD_REV}"
variant "${GOOD_PACK}" "${TMP_DIR}/pack-pc-true.yml" '/repository: Fandhe-AI/,/persist-credentials/s|false|true|'
case_ "engine checkout with persist-credentials true is rejected (D3)" 1 D3 --pack-workflow "${TMP_DIR}/pack-pc-true.yml" --engine-rev "${GOOD_REV}"
# shellcheck disable=SC2016 # sed の式なので、シェルに展開させない
variant "${GOOD_CI}" "${TMP_DIR}/ci-write-expr.yml" "s|contents: read|contents: \${{ github.event_name == 'push' \\&\\& 'write' \\|\\| 'read' }}|"
case_ "write inside an expression in a permission value is rejected (C1)" 1 C1 --ci "${TMP_DIR}/ci-write-expr.yml"
# shellcheck disable=SC2016 # sed の式なので、シェルに展開させない
variant "${GOOD_CI}" "${TMP_DIR}/ci-perm-expr.yml" 's|^permissions:$|permissions: ${{ fromJSON(vars.PERMS) }}|;/^  contents: read$/d'
case_ "an expression as the permissions value is rejected (C1)" 1 C1 --ci "${TMP_DIR}/ci-perm-expr.yml"
variant "${GOOD_CI}" "${TMP_DIR}/ci-perm-none.yml" 's|contents: read|contents: none|'
case_ "a none permission passes (C1)" 0 - --ci "${TMP_DIR}/ci-perm-none.yml"
variant "${GOOD_CI}" "${TMP_DIR}/ci-perm-flow.yml" 's|^permissions:$|permissions: {contents: read, actions: none}|;/^  contents: read$/d'
case_ "a flow-style read-only permissions map passes (C1)" 0 - --ci "${TMP_DIR}/ci-perm-flow.yml"

# review follow-ups 7: path を省いた後続の checkout・トップレベルの permissions の欠落
# path を省いた checkout は作業領域のルートを clean し、取得済みのエンジンごと消す
{ cat "${GOOD_PACK}"; printf '      - name: Checkout again\n        uses: actions/checkout@%s # v7.0.1\n        with:\n          persist-credentials: false\n' "${SHA}"; } >"${TMP_DIR}/pack-root-checkout.yml"
case_ "later checkout without a path (workspace root) is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-root-checkout.yml" --engine-rev "${GOOD_REV}"
{ cat "${GOOD_PACK}"; printf '      - uses: "Actions/Checkout@%s"\n' "${SHA}"; } >"${TMP_DIR}/pack-root-checkout-quoted.yml"
case_ "later quoted, mixed-case checkout without with is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-root-checkout-quoted.yml" --engine-rev "${GOOD_REV}"
# uses を折り返し・ブロックのスカラーで書くと、同じ行の値には checkout が現れない
{ cat "${GOOD_PACK}"; printf '      - uses: >-\n          actions/checkout@%s\n' "${SHA}"; } >"${TMP_DIR}/pack-folded-checkout.yml"
case_ "later checkout in a folded uses scalar is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-folded-checkout.yml" --engine-rev "${GOOD_REV}"
{ cat "${GOOD_PACK}"; printf '      - uses: |\n          actions/checkout@%s\n' "${SHA}"; } >"${TMP_DIR}/pack-block-checkout.yml"
case_ "later checkout in a block uses scalar is rejected (D4)" 1 D4 --pack-workflow "${TMP_DIR}/pack-block-checkout.yml" --engine-rev "${GOOD_REV}"
variant "${GOOD_CI}" "${TMP_DIR}/ci-no-perm.yml" '/^permissions:$/d;/^  contents: read$/d'
case_ "ci.yml without top-level permissions is rejected (C1)" 1 C1 --ci "${TMP_DIR}/ci-no-perm.yml"
printf '    permissions:\n      contents: read\n' >"${TMP_DIR}/t-job-perm"
splice "${TMP_DIR}/ci-no-perm.yml" "${TMP_DIR}/ci-job-perm-only.yml" after '    runs-on: ubuntu-latest' "${TMP_DIR}/t-job-perm"
case_ "ci.yml with only job-level permissions is rejected (C1)" 1 C1 --ci "${TMP_DIR}/ci-job-perm-only.yml"
variant "${GOOD_CI}" "${TMP_DIR}/ci-perm-empty.yml" 's|^permissions:$|permissions: {}|;/^  contents: read$/d'
case_ "top-level permissions: {} passes (C1)" 0 - --ci "${TMP_DIR}/ci-perm-empty.yml"

# review follow-ups 8: 字句で確定できない key（Y）・run からの submodule の取得（A5）
echo "# YAML key notation and submodule fetches in run"
ci_n=0
# ci_case <name> <expected-rc> <expected-tag|-> <text>: 良い見本の ci.yml の末尾に text（step 等）を足して流す
ci_case() {
  local f="${TMP_DIR}/ci-y-$((++ci_n)).yml"
  { cat "${GOOD_CI}"; printf '%s\n' "$4"; } >"${f}"
  case_ "$1" "$2" "$3" --ci "${f}"
}
ci_case "double-quoted uses key is rejected (Y1)" 1 Y1 '      - "uses": someone/action@main'
ci_case "single-quoted uses key is rejected (Y1)" 1 Y1 "      - 'uses': someone/action@main"
printf '          "submodules": recursive\n' >"${TMP_DIR}/t-qsub"
splice "${GOOD_CI}" "${TMP_DIR}/ci-qsub.yml" after 'persist-credentials: false' "${TMP_DIR}/t-qsub"
case_ "double-quoted submodules key is rejected (Y1)" 1 Y1 --ci "${TMP_DIR}/ci-qsub.yml"
printf '    "environment": production\n' >"${TMP_DIR}/t-qenv"
splice "${GOOD_CI}" "${TMP_DIR}/ci-qenv.yml" after '    runs-on: ubuntu-latest' "${TMP_DIR}/t-qenv"
case_ "double-quoted environment key is rejected (Y1)" 1 Y1 --ci "${TMP_DIR}/ci-qenv.yml"
ci_case "complex key for uses is rejected (Y2)" 1 Y2 '      - ? uses
        : someone/action@main'
ci_case "flow-style step is rejected (Y3)" 1 Y3 '      - {uses: someone/action@main}'
ci_case "flow-style steps sequence is rejected (Y3)" 1 Y3 '  other:
    runs-on: ubuntu-latest
    steps: [{uses: someone/action@main}]'
ci_case "tagged uses key is rejected (Y4)" 1 Y4 '      - !!str uses: someone/action@main'
ci_case "anchored uses key is rejected (Y4)" 1 Y4 '      - &k uses: someone/action@main'
ci_case "run given as an alias is rejected (Y5)" 1 Y5 '      - name: &cmd git submodule update --init --checkout docs/spec
        run: *cmd'
printf 'name: CI\npermissions: {}\njobs: {check: {runs-on: ubuntu-latest, steps: [{run: make check}]}}\n' >"${TMP_DIR}/ci-flow-jobs.yml"
case_ "flow-style jobs mapping is rejected (Y3)" 1 Y3 --ci "${TMP_DIR}/ci-flow-jobs.yml"
printf '%s\n' '      - "uses": someone/action@main' >"${TMP_DIR}/t-qpack"
splice "${GOOD_PACK}" "${TMP_DIR}/pack-quoted.yml" after '          persist-credentials: false' "${TMP_DIR}/t-qpack"
case_ "quoted key in the pack workflow is rejected (Y1)" 1 Y1 --pack-workflow "${TMP_DIR}/pack-quoted.yml" --engine-rev "${GOOD_REV}"
printf '%s\n' '"on":' '  push:' >"${TMP_DIR}/t-qon"
splice "${GOOD_DEPLOY}" "${TMP_DIR}/deploy-quoted.yml" eq 'on:' "${TMP_DIR}/t-qon"
case_ "quoted key in deploy.yml is rejected (Y1)" 1 Y1 --deploy "${TMP_DIR}/deploy-quoted.yml"
ci_case "inline run with git submodule update is rejected (A5)" 1 A5 '      - run: git submodule update --init --checkout docs/spec'
ci_case "block run with git submodule update is rejected (A5)" 1 A5 '      - name: Spec
        run: |
          make check
          git submodule update --init --checkout docs/spec'
ci_case "submodule.recurse config in run is rejected (A5)" 1 A5 '      - run: git -c submodule.recurse=true pull'
ci_case "git clone --recurse-submodules is rejected (A5)" 1 A5 '      - run: git clone --recurse-submodules https://github.com/Fandhe-AI/x.git'
ci_case "git clone --recursive is rejected (A5)" 1 A5 '      - run: git clone --recursive https://github.com/Fandhe-AI/x.git'
ci_case "abbreviated --recurs is rejected (A5)" 1 A5 '      - run: git clone --recurs https://github.com/Fandhe-AI/x.git'
ci_case "--recursive on a continued line of a git command is rejected (A5)" 1 A5 '      - run: |
          git clone \
            --recursive https://github.com/Fandhe-AI/x.git'
# shellcheck disable=SC2016 # 見本の本文なので、シェルに展開させない
ci_case "run body with braces, quotes and URLs passes" 0 - '      - name: Submodule-free build
        run: |
          "${CARGO:-cargo}" build --release
          { echo "https://example.invalid/a:b"; } > out.txt
          cp --recursive a b'

echo "# misuse and fail-closed (exit 2)"
case_ "missing Cargo.lock stops with exit 2" 2 - --cargo-lock "${TMP_DIR}/none.lock"
case_ "missing ci.yml stops with exit 2" 2 - --ci "${TMP_DIR}/none.yml"
case_ "missing deploy.yml stops with exit 2" 2 - --deploy "${TMP_DIR}/none.yml"
printf 'not json' >"${TMP_DIR}/bad.json"
case_ "unparsable package-lock.json stops with exit 2" 2 - --package-lock "${TMP_DIR}/bad.json"
case_ "unknown option stops with exit 2" 2 - --bogus x
case_ "option without a value stops with exit 2" 2 - --ci
case_ "--pack-workflow without --engine-rev stops with exit 2" 2 - --pack-workflow "${GOOD_PACK}"
case_ "missing ENGINE_REV file stops with exit 2" 2 - --pack-workflow "${GOOD_PACK}" --engine-rev "${TMP_DIR}/none-rev"

echo ""
echo "${passed} passed, ${failed} failed"
if [ "${failed}" -gt 0 ]; then
  exit 1
fi
