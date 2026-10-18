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

GOOD_PACK="${TMP_DIR}/pack.yml"
cat >"${GOOD_PACK}" <<EOF
name: Build pack
on:
  push:
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - name: Read engine revision
        id: rev
        run: echo "rev=\$(cat ENGINE_REV)" >> "\${GITHUB_OUTPUT}"
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
case_ "pack workflow reading ENGINE_REV passes (D)" 0 - --pack-workflow "${GOOD_PACK}" --engine-rev "${GOOD_REV}"
variant "${GOOD_PACK}" "${TMP_DIR}/pack-literal.yml" "s|\${{ steps.rev.outputs.rev }}|${REV}|"
case_ "pack workflow with the literal ENGINE_REV ref passes (D3)" 0 - --pack-workflow "${TMP_DIR}/pack-literal.yml" --engine-rev "${GOOD_REV}"

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
variant "${GOOD_CI}" "${TMP_DIR}/ci-submodules.yml" 's|persist-credentials: false|persist-credentials: false\n          submodules: recursive|'
case_ "checkout with submodules is rejected (A5)" 1 A5 --ci "${TMP_DIR}/ci-submodules.yml"
variant "${GOOD_CI}" "${TMP_DIR}/ci-submodules-false.yml" 's|persist-credentials: false|persist-credentials: false\n          submodules: false|'
case_ "checkout with submodules: false passes" 0 - --ci "${TMP_DIR}/ci-submodules-false.yml"

echo "# B: secret references in the build/test workflow"
variant "${GOOD_CI}" "${TMP_DIR}/ci-registry-env.yml" 's|    runs-on: ubuntu-latest|    runs-on: ubuntu-latest\n    env:\n      CARGO_REGISTRIES_X_TOKEN: dummy|'
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
variant "${GOOD_CI}" "${TMP_DIR}/ci-env.yml" 's|    runs-on: ubuntu-latest|    runs-on: ubuntu-latest\n    environment: prod|'
case_ "environment in ci.yml is rejected (C1)" 1 C1 --ci "${TMP_DIR}/ci-env.yml"
variant "${GOOD_CI}" "${TMP_DIR}/ci-deploy-job.yml" 's|^  check:|  deploy:|'
case_ "deploy job in ci.yml is rejected (C3)" 1 C3 --ci "${TMP_DIR}/ci-deploy-job.yml"
for trig in pull_request pull_request_target push schedule workflow_run; do
  variant "${GOOD_DEPLOY}" "${TMP_DIR}/deploy-${trig}.yml" "s|^  workflow_dispatch:|  workflow_dispatch:\n  ${trig}:|"
  case_ "deploy.yml with ${trig} trigger is rejected (C2)" 1 C2 --deploy "${TMP_DIR}/deploy-${trig}.yml"
done
variant "${GOOD_DEPLOY}" "${TMP_DIR}/deploy-inline-on.yml" 's|^on:$|on: [push, workflow_dispatch]|;/^  workflow_dispatch:$/d'
case_ "deploy.yml with an inline on: is rejected (C2)" 1 C2 --deploy "${TMP_DIR}/deploy-inline-on.yml"
variant "${GOOD_DEPLOY}" "${TMP_DIR}/deploy-perm.yml" 's|^permissions: {}|permissions:\n  contents: read|'
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
case_ "git clone of the engine that reads ENGINE_REV passes (D4)" 0 - --pack-workflow "${TMP_DIR}/pack-clone-ok.yml" --engine-rev "${GOOD_REV}"

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
