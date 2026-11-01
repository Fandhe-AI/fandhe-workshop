#!/usr/bin/env bash
# scripts/check-fork-ci.sh 自体の回帰テスト（`make test-check-fork-ci` と ci.yml の check ジョブから呼ばれる）。
#
# 「private な依存・secret の参照・配信の取り込みを足すと検査が失敗する」ことを、一時ファイルを検査して自動で
# 確かめる。違反の見本は、終了コードに加えて「正しい理由（検査の項目名）で失敗したか」も確かめる。
# 先行の字句実装が表記の回避（引用符つきのキー・エスケープ・フロー形式・大文字のキー・複数行の値）で抜けられた形は、
# ここで拒否されることを個別に確かめる。
# B1 の実体（check-workflow-secrets.sh）以外の見本は、常に成功する stub に差し替えて流す。
# 道具（ruby・jq・git・cargo）が無い環境では、スキップで成功にせず失敗させる。
# 作業ツリーは変えない。ネットワークは使わない（cargo は --offline で lock を作る）。
#
# 出力は TAP 風（scripts/hooks/test-hooks.sh に合わせる）。終了コード: 0 = 全 pass / 1 = 失敗あり。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"
for c in mktemp rm ruby jq git cargo; do
  require_cmd "${c}" || exit 1
done

CHECK="${DIR}/check-fork-ci.sh"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

SHA="3d3c42e5aac5ba805825da76410c181273ba90b1"
G="github.repository == 'Fandhe-AI/fandhe-workshop'"
REG="registry+https://github.com/rust-lang/crates.io-index"
LEAK="LEAKMARKER0123456789"
# URL の認証情報の見本は、secret-scan.sh に検知されないよう実行時に組み立てる
AT="@"
STUB="${TMP_DIR}/secrets-stub.sh"
printf '#!/bin/sh\nexit 0\n' >"${STUB}"
chmod +x "${STUB}"
STUB2="${TMP_DIR}/secrets-stub2.sh"
printf '#!/bin/sh\nexit 2\n' >"${STUB2}"
chmod +x "${STUB2}"

passed=0
failed=0
seq_no=0

# case_ <name> <expected-rc> <expected-tag|-> <args...>: 検査を B1 の stub つきで流す。
# 失敗の見本は、標準エラーに `:<expected-tag> `（A1 等）が出ていることも確かめる。
case_() {
  local name="$1" want="$2" tag="$3" rc=0 err="${TMP_DIR}/err"
  shift 3
  "${CHECK}" --secrets-checker "${STUB}" "$@" >/dev/null 2>"${err}" || rc=$?
  verdict "${name}" "${want}" "${tag}" "${rc}" "${err}"
}

verdict() {
  local name="$1" want="$2" tag="$3" rc="$4" err="$5"
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
# ci_doc <perm|-> <top> <jobx> <stepx>: ci.yml の見本を標準出力へ出す。
#   perm: トップレベルの permissions のブロック（- なら読み取り専用。空文字なら置かない）
#   top: jobs の前に足す行 / jobx: ジョブの steps の前に足す行（インデント 4）/ stepx: steps の末尾に足す行（インデント 6）
ci_doc() {
  local perm="$1" top="$2" jobx="$3" stepx="$4" on="${5:-  pull_request:}"
  [ "${perm}" != "-" ] || perm=$'permissions:\n  contents: read'
  printf 'name: CI\non:\n%s\n%s\n%s\njobs:\n  check:\n    runs-on: ubuntu-latest\n%s\n    steps:\n      - uses: actions/checkout@%s\n        with:\n          persist-credentials: false\n%s\n' \
    "${on}" "${perm}" "${top}" "${jobx}" "${SHA}" "${stepx}"
}

# deploy_doc <on|-> <perm|-> <jobs|->: deploy.yml の見本を標準出力へ出す
deploy_doc() {
  local on="$1" perm="$2" jobs="$3"
  [ "${on}" != "-" ] || on=$'on:\n  workflow_dispatch:'
  [ "${perm}" != "-" ] || perm='permissions: {}'
  [ "${jobs}" != "-" ] || jobs="  deploy:
    if: ${G}
    runs-on: ubuntu-latest
    steps:
      - run: exit 1"
  printf 'name: Deploy\n%s\n%s\njobs:\n%s\n' "${on}" "${perm}" "${jobs}"
}

# release_doc <on|-> <perm|-> <guard|-> <uses|->: release.yml の見本を標準出力へ出す（C4）
release_doc() {
  local on="$1" perm="$2" guard="$3" uses="$4"
  [ "${on}" != "-" ] || on=$'on:\n  workflow_dispatch:'
  [ "${perm}" != "-" ] || perm='permissions: {}'
  [ "${guard}" != "-" ] || guard="${G}"
  [ "${uses}" != "-" ] || uses="actions/checkout@${SHA}"
  printf 'name: Release\n%s\n%s\njobs:\n  build:\n    if: %s\n    runs-on: macos-latest\n    permissions:\n      contents: read\n    steps:\n      - uses: %s\n' \
    "${on}" "${perm}" "${guard}" "${uses}"
}

# ycase <name> <rc> <tag|-> <ci|deploy> <content>: 内容を一時ファイルに書いて検査する
ycase() {
  local f
  seq_no=$((seq_no + 1))
  f="${TMP_DIR}/y-${seq_no}.yml"
  printf '%s\n' "$5" >"${f}"
  case_ "$1" "$2" "$3" "--$4" "${f}"
}

# meta_json <package2-source-json> [<package2-id>]: cargo metadata の出力の見本
meta_json() {
  local src="$1" id="${2:-reg#serde@1.0.0}"
  printf '{"workspace_members":["path+file:///w#a@0.1.0"],"packages":[{"id":"path+file:///w#a@0.1.0","name":"a","version":"0.1.0","source":null},{"id":"%s","name":"serde","version":"1.0.0","source":%s}]}\n' "${id}" "${src}"
}

mcase() {
  local f
  seq_no=$((seq_no + 1))
  f="${TMP_DIR}/m-${seq_no}.json"
  meta_json "$4" "${5:-reg#serde@1.0.0}" >"${f}"
  case_ "$1" "$2" "$3" --cargo-metadata-json "${f}"
}

# ---- 合格する見本 ----
echo "# positive samples"
case_ "the real repo passes (A-C)" 0 -
ycase "minimal ci sample passes" 0 - ci "$(ci_doc - "" "" "")"
ycase "GITHUB_PATH is not mistaken for a _PAT word" 0 - ci "$(ci_doc - "" "" $'      - run: echo "x" >> "$GITHUB_PATH"')"
ycase "submodules: false passes" 0 - ci "$(ci_doc - "" "" $'      - uses: actions/checkout@'"${SHA}"$'\n        with:\n          submodules: false')"
ycase "permissions: {} at top level passes" 0 - ci "$(ci_doc 'permissions: {}' "" "" "")"
ycase "flow-style read-only permissions pass" 0 - ci "$(ci_doc 'permissions: {contents: read, actions: none}' "" "" "")"
ycase "flow-style branches pass" 0 - ci "$(ci_doc - "" "" "" $'  push:\n    branches: [main]')"
ycase "allowed CARGO_ env keys pass" 0 - ci "$(ci_doc - "" $'    env:\n      CARGO_TERM_COLOR: always\n      CARGO_INCREMENTAL: "0"\n      CARGO_PROFILE_DEV_DEBUG: line-tables-only\n      CARGO_TARGET_DIR: /tmp/t' "")"
ycase "deploy sample passes" 0 - deploy "$(deploy_doc - - -)"
ycase "deploy guard wrapped in an expression passes" 0 - deploy "$(deploy_doc - - "  deploy:
    if: \${{ ${G} }}
    steps:
      - run: exit 1")"
ycase "deploy guard as a folded block scalar passes" 0 - deploy "$(deploy_doc - - "  deploy:
    if: >-
      ${G}
    steps:
      - run: exit 1")"
printf 'engine-strict=true\n' >"${TMP_DIR}/npmrc-ok"
case_ ".npmrc with only engine-strict passes" 0 - --npmrc "${TMP_DIR}/npmrc-ok"
printf '# comment\n\n; another\nengine-strict=true\n' >"${TMP_DIR}/npmrc-ok2"
case_ ".npmrc with comments and blank lines passes" 0 - --npmrc "${TMP_DIR}/npmrc-ok2"
printf '[submodule "docs/spec"]\n\tpath = docs/spec\n\turl = https://example.invalid/x.git\n\tupdate = none\n' >"${TMP_DIR}/gm-ok"
case_ ".gitmodules with update = none passes" 0 - --gitmodules "${TMP_DIR}/gm-ok"
mkdir -p "${TMP_DIR}/no-cargo-cfg"
case_ "no .cargo config passes" 0 - --cargo-config-dir "${TMP_DIR}/no-cargo-cfg"
mcase "crates.io packages and workspace members pass" 0 - "\"${REG}\""
printf '{"lockfileVersion":3,"packages":{"":{"name":"x"},"node_modules/a":{"version":"1.0.0","resolved":"https://registry.npmjs.org/a/-/a-1.0.0.tgz","integrity":"sha512-AAAA"}}}\n' >"${TMP_DIR}/lock-ok.json"
case_ "package-lock from the npm registry passes" 0 - --package-lock "${TMP_DIR}/lock-ok.json"

# ---- A: private な依存 ----
echo "# A: private dependencies"
mcase "cargo git source is rejected (A1)" 1 A1 '"git+https://example.invalid/r#abc"'
mcase "cargo other registry is rejected (A1)" 1 A1 '"registry+https://registry.example.invalid/index"'
mcase "cargo sparse registry is rejected (A1)" 1 A1 '"sparse+https://registry.example.invalid/"'
mcase "cargo path dependency outside the workspace (null source) is rejected (A1)" 1 A1 null "path+file:///elsewhere#b@0.1.0"
lockpkg() { # <resolved-json> <integrity-json> [extra-json]
  printf '{"lockfileVersion":3,"packages":{"":{"name":"x"},"node_modules/a":{"version":"1.0.0"%s%s%s}}}\n' "${1:+,\"resolved\":$1}" "${2:+,\"integrity\":$2}" "${3:+,$3}"
}
npmcase() { # <name> <rc> <tag> <lock content>
  seq_no=$((seq_no + 1))
  printf '%s\n' "$4" >"${TMP_DIR}/lock-${seq_no}.json"
  case_ "$1" "$2" "$3" --package-lock "${TMP_DIR}/lock-${seq_no}.json"
}
npmcase "npm other registry is rejected (A2)" 1 A2 "$(lockpkg '"https://npm.example.invalid/a.tgz"' '"sha512-A"')"
npmcase "npm git dependency is rejected (A2)" 1 A2 "$(lockpkg '"git+ssh://example.invalid/x/y.git"' '"sha512-A"')"
npmcase "npm file: dependency is rejected (A2)" 1 A2 "$(lockpkg '"file:../a"' '"sha512-A"')"
npmcase "npm link entry is rejected (A2)" 1 A2 "$(lockpkg '"https://registry.npmjs.org/a.tgz"' '"sha512-A"' '"link":true')"
npmcase "npm entry without integrity is rejected (A2)" 1 A2 "$(lockpkg '"https://registry.npmjs.org/a.tgz"' '')"
npmcase "npm entry without resolved is rejected (A2)" 1 A2 "$(lockpkg '' '"sha512-A"')"
npmcase "lockfileVersion 2 is rejected (A2)" 1 A2 "$(lockpkg '"https://registry.npmjs.org/a.tgz"' '"sha512-A"' | sed 's/"lockfileVersion":3/"lockfileVersion":2/')"
printf '//npm.example.invalid/:_authToken=%s\n' "${LEAK}" >"${TMP_DIR}/npmrc-token"
case_ ".npmrc credentials are rejected (A3)" 1 A3 --npmrc "${TMP_DIR}/npmrc-token"
printf 'registry=https://npm.example.invalid/\n' >"${TMP_DIR}/npmrc-registry"
case_ ".npmrc registry is rejected (A3)" 1 A3 --npmrc "${TMP_DIR}/npmrc-registry"
printf 'engine-strict=true\nengine-strict = true\n' >"${TMP_DIR}/npmrc-spaced"
case_ ".npmrc with a differently spelled line is rejected (A3)" 1 A3 --npmrc "${TMP_DIR}/npmrc-spaced"
mkdir -p "${TMP_DIR}/cargo-bad" "${TMP_DIR}/cargo-registry"
printf '[net]\ngit-fetch-with-cli = true\n' >"${TMP_DIR}/cargo-bad/config.toml"
case_ ".cargo/config.toml is rejected (A4)" 1 A4 --cargo-config-dir "${TMP_DIR}/cargo-bad"
printf '[source.crates-io]\nreplace-with = "mirror"\n' >"${TMP_DIR}/cargo-registry/config"
case_ ".cargo/config is rejected (A4)" 1 A4 --cargo-config-dir "${TMP_DIR}/cargo-registry"
printf '[submodule "docs/spec"]\n\tpath = docs/spec\n\turl = https://example.invalid/x.git\n' >"${TMP_DIR}/gm-noupdate"
case_ "submodule without update is rejected (A5)" 1 A5 --gitmodules "${TMP_DIR}/gm-noupdate"
printf '[submodule "docs/spec"]\n\tpath = docs/spec\n\turl = https://example.invalid/x.git\n\tupdate = checkout\n' >"${TMP_DIR}/gm-checkout"
case_ "submodule with update = checkout is rejected (A5)" 1 A5 --gitmodules "${TMP_DIR}/gm-checkout"
printf '[submodule "docs/spec"]\n\tpath = docs/spec\n\tupdate = none\n\tupdate = checkout\n' >"${TMP_DIR}/gm-two"
case_ "submodule with two update values is rejected (A5)" 1 A5 --gitmodules "${TMP_DIR}/gm-two"
printf '[submodule "a"]\n\tpath = a\n\tupdate = none\n[submodule "b"]\n\tpath = b\n' >"${TMP_DIR}/gm-second"
case_ "a second submodule without update is rejected (A5)" 1 A5 --gitmodules "${TMP_DIR}/gm-second"
printf '[submodule "a"]\n\tpath = a\n\tupdate = none\n[core]\n\tfoo = bar\n' >"${TMP_DIR}/gm-core"
case_ "a non-submodule key in .gitmodules is rejected (A5)" 1 A5 --gitmodules "${TMP_DIR}/gm-core"

ycase "checkout with submodules: recursive is rejected (A5)" 1 A5 ci "$(ci_doc - "" "" $'      - uses: actions/checkout@'"${SHA}"$'\n        with:\n          submodules: recursive')"
ycase "quoted key 'submodules' is rejected (A5)" 1 A5 ci "$(ci_doc - "" "" $'      - uses: actions/checkout@'"${SHA}"$'\n        with:\n          \'submodules\': recursive')"
ycase "capitalised key Submodules: true is rejected (A5)" 1 A5 ci "$(ci_doc - "" "" $'      - uses: actions/checkout@'"${SHA}"$'\n        with:\n          Submodules: true')"
ycase "flow-style with: {submodules: true} is rejected (A5)" 1 A5 ci "$(ci_doc - "" "" $'      - uses: actions/checkout@'"${SHA}"$'\n        with: {submodules: true}')"
ycase "escaped run mentioning submodule is rejected (A5)" 1 A5 ci "$(ci_doc - "" "" $'      - run: "git \\x73ubmodule update --init docs/spec"')"
ycase "run with --recurse-submodules is rejected (A5)" 1 A5 ci "$(ci_doc - "" "" $'      - run: git clone --recurse-submodules x')"
ycase "run with clone.recurseSubmodules is rejected (A5)" 1 A5 ci "$(ci_doc - "" "" $'      - run: git -c clone.recurseSubmodules=true fetch')"
ycase "run in a block scalar mentioning submodule is rejected (A5)" 1 A5 ci "$(ci_doc - "" "" $'      - run: |\n          echo a\n          git submodule update')"
ycase "job env GIT_CONFIG_KEY_0 is rejected (A5)" 1 A5 ci "$(ci_doc - "" $'    env:\n      GIT_CONFIG_KEY_0: x' "")"
ycase "workflow env GIT_CONFIG_COUNT is rejected (A5)" 1 A5 ci "$(ci_doc - $'env:\n  GIT_CONFIG_COUNT: "1"' "" "")"
ycase "step env git_ in lower case is rejected (A5)" 1 A5 ci "$(ci_doc - "" "" $'      - run: echo a\n        env:\n          git_dir: x')"
ycase "env that is not a mapping is rejected (A5)" 1 A5 ci "$(ci_doc - "" "    env: \${{ fromJSON(x) }}" "")"
ycase "cp --recursive is not mistaken for a submodule (no A5)" 0 - ci "$(ci_doc - "" "" $'      - run: cp --recursive a b')"

# ---- B: secret の参照・uses ----
echo "# B: secrets and uses"
ycase "tag-pinned uses is rejected (B3)" 1 B3 ci "$(ci_doc - "" "" $'      - uses: actions/checkout@v4')"
ycase "other owner is rejected (B3)" 1 B3 ci "$(ci_doc - "" "" $'      - uses: someone/action@'"${SHA}")"
ycase "quoted key \"uses\" with a branch ref is rejected (B3)" 1 B3 ci "$(ci_doc - "" "" $'      - "uses": someone/action@main')"
ycase "docker:// uses is rejected (B3)" 1 B3 ci "$(ci_doc - "" "" $'      - uses: docker://alpine:3')"
ycase "local uses is rejected (B3)" 1 B3 ci "$(ci_doc - "" "" $'      - uses: ./local-action')"
ycase "flow-style steps with a branch ref is rejected (B3)" 1 B3 ci $'name: CI\non: push\npermissions: {contents: read}\njobs:\n  a:\n    steps: [{uses: x/y@main}]'
ycase "uses inside a flow sequence under on is rejected (B3)" 1 B3 ci "$(ci_doc - "" "" "" $'  push:\n    branches: [{uses: x/y@main}]')"
ycase "reusable workflow at job level is rejected (B3)" 1 B3 ci "$(ci_doc - "" "" "" | cat; printf '  second:\n    uses: org/repo/.github/workflows/w.yml@%s\n' "${SHA}")"
ycase "CARGO_REGISTRIES_X_TOKEN env key is rejected (B2)" 1 B2 ci "$(ci_doc - "" $'    env:\n      CARGO_REGISTRIES_X_TOKEN: a' "")"
ycase "CARGO_REGISTRY_DEFAULT env key is rejected (B2)" 1 B2 ci "$(ci_doc - "" $'    env:\n      CARGO_REGISTRY_DEFAULT: a' "")"
ycase "a _PAT word is rejected (B2)" 1 B2 ci "$(ci_doc - "" "" $'      - run: echo "$MY_PAT"')"
ycase "a _TOKEN word is rejected (B2)" 1 B2 ci "$(ci_doc - "" "" $'      - run: echo "$DEPLOY_TOKEN"')"
ycase "GITHUB_TOKEN is rejected (B2)" 1 B2 ci "$(ci_doc - "" $'    env:\n      "GITHUB_TOKEN": x' "")"
ycase "git-fetch-with-cli is rejected (B2)" 1 B2 ci "$(ci_doc - "" "" $'      - run: cargo --config net.git-fetch-with-cli=true build')"
printf 'name: CI\njobs:\n  a:\n    steps:\n      - run: echo %s\n' "\${{ secrets.X }}" >"${TMP_DIR}/ci-secret.yml"
seq_no=$((seq_no + 1))
rc=0
"${CHECK}" --ci "${TMP_DIR}/ci-secret.yml" >/dev/null 2>"${TMP_DIR}/err" || rc=$?
verdict "a secrets reference is rejected by the real checker (B1)" 1 B1 "${rc}" "${TMP_DIR}/err"
rc=0
"${CHECK}" --secrets-checker "${STUB2}" --ci "${TMP_DIR}/ci-secret.yml" >/dev/null 2>"${TMP_DIR}/err" || rc=$?
verdict "B1 checker exiting 2 makes the whole check exit 2" 2 - "${rc}" "${TMP_DIR}/err"

# ---- C: 配信の切り離し ----
echo "# C: delivery isolation"
ycase "ci.yml without top-level permissions is rejected (C1)" 1 C1 ci "$(ci_doc '' "" $'    permissions:\n      contents: read' "")"
ycase "write permission is rejected (C1)" 1 C1 ci "$(ci_doc $'permissions:\n  contents: write' "" "" "")"
ycase "quoted \"write\" is rejected (C1)" 1 C1 ci "$(ci_doc $'permissions:\n  contents: "write"' "" "" "")"
ycase "single-quoted 'write' is rejected (C1)" 1 C1 ci "$(ci_doc $'permissions:\n  contents: \'write\'' "" "" "")"
ycase "flow-style {contents: \"write\"} is rejected (C1)" 1 C1 ci "$(ci_doc 'permissions: {contents: "write"}' "" "" "")"
ycase "write-all is rejected (C1)" 1 C1 ci "$(ci_doc 'permissions: write-all' "" "" "")"
ycase "read-all scalar is rejected (C1)" 1 C1 ci "$(ci_doc 'permissions: read-all' "" "" "")"
ycase "an expression value in permissions is rejected (C1)" 1 C1 ci "$(ci_doc $'permissions:\n  contents: ${{ x && \'write\' || \'read\' }}' "" "" "")"
ycase "an expression as permissions is rejected (C1)" 1 C1 ci "$(ci_doc "permissions: \${{ fromJSON(x) }}" "" "" "")"
ycase "job-level write permission is rejected (C1)" 1 C1 ci "$(ci_doc - "" $'    permissions:\n      id-token: write' "")"
ycase "job-level environment is rejected (C1)" 1 C1 ci "$(ci_doc - "" '    environment: production' "")"
ycase "quoted \"environment\" is rejected (C1)" 1 C1 ci "$(ci_doc - "" '    "environment": production' "")"
ycase "a deploy job in ci.yml is rejected (C3)" 1 C3 ci "$(ci_doc - "" "" "" | cat; printf '  Deploy-Pages:\n    runs-on: ubuntu-latest\n    steps:\n      - run: echo\n')"
ycase "flow-style jobs with a deploy job is rejected (C3)" 1 C3 ci $'name: CI\non: push\npermissions: {contents: read}\njobs: {deploy: {runs-on: ubuntu-latest, steps: []}}'
ycase "deploy guard with || true on the same line is rejected (C2)" 1 C2 deploy "$(deploy_doc - - "  deploy:
    if: ${G} || true
    steps:
      - run: exit 1")"
ycase "deploy guard with || true on a continuation line is rejected (C2)" 1 C2 deploy "$(deploy_doc - - "  deploy:
    if: >-
      ${G}
      || true
    steps:
      - run: exit 1")"
ycase "deploy guard of another repository is rejected (C2)" 1 C2 deploy "$(deploy_doc - - "  deploy:
    if: github.repository == 'someone/else'
    steps:
      - run: exit 1")"
ycase "deploy job without a guard is rejected (C2)" 1 C2 deploy "$(deploy_doc - - "  deploy:
    if: ${G}
    steps:
      - run: exit 1
  second:
    steps:
      - run: exit 1")"
ycase "deploy on with push added is rejected (C2)" 1 C2 deploy "$(deploy_doc $'on:\n  workflow_dispatch:\n  push:' - -)"
ycase "deploy on: pull_request_target is rejected (C2)" 1 C2 deploy "$(deploy_doc $'on:\n  pull_request_target:' - -)"
ycase "deploy on as a sequence with push is rejected (C2)" 1 C2 deploy "$(deploy_doc 'on: [workflow_dispatch, push]' - -)"
ycase "deploy quoted \"on\" with pull_request is rejected (C2)" 1 C2 deploy "$(deploy_doc $'"on":\n  pull_request:' - -)"
ycase "deploy on as a one-element sequence passes" 0 - deploy "$(deploy_doc 'on: [workflow_dispatch]' - -)"
ycase "deploy on as a scalar passes" 0 - deploy "$(deploy_doc 'on: workflow_dispatch' - -)"
ycase "deploy with non-empty top-level permissions is rejected (C2)" 1 C2 deploy "$(deploy_doc - $'permissions:\n  contents: read' -)"
ycase "deploy without the on key is rejected (C2)" 1 C2 deploy "$(deploy_doc '' - -)"
ycase "release sample passes" 0 - release "$(release_doc - - - -)"
ycase "release with push added is rejected (C4)" 1 C4 release "$(release_doc $'on:\n  workflow_dispatch:\n  push:' - - -)"
ycase "release with pull_request is rejected (C4)" 1 C4 release "$(release_doc $'on:\n  pull_request:' - - -)"
ycase "release without a fork guard is rejected (C4)" 1 C4 release "$(release_doc - - "true" -)"
ycase "release guard of another repository is rejected (C4)" 1 C4 release "$(release_doc - - "github.repository == 'someone/else'" -)"
ycase "release with top-level contents: write is rejected (C4)" 1 C4 release "$(release_doc - $'permissions:\n  contents: write' - -)"
ycase "release uses pinned by tag is rejected (C4)" 1 C4 release "$(release_doc - - - 'actions/checkout@v4')"
ycase "release uses of another owner is rejected (C4)" 1 C4 release "$(release_doc - - - "evil/act@${SHA}")"

# ---- D: ENGINE_REV の正準の形（教材 repo の workflow）----
# 先行の字句実装（awk）が回避の指摘で収束しなかった形を、ここで拒否されることとして個別に確かめる。
# 合格するのは正準の 3 step だけ。後ろの step は git の語・エンジンの repo・ENGINE_REV・エンジンの path・
# actions/checkout に触れた時点で一律に拒否される（シェルの意味は解釈しない）。
echo "# D: canonical ENGINE_REV pin"
ENGINE="Fandhe-AI/fandhe-workshop"
REF=$'${{ steps.rev.outputs.rev }}'
GOOD_REV="${TMP_DIR}/ENGINE_REV"
printf '0123456789abcdef0123456789abcdef01234567\n' >"${GOOD_REV}"
CANON_S1=$'      - uses: actions/checkout@'"${SHA}"$'\n        with:\n          persist-credentials: false'
CANON_S2=$'      - id: rev\n        run: echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"'

# eng3 <repository> <ref> <path> [<extra raw line>]: エンジンの checkout の step
eng3() {
  local x=""
  [ -z "${4-}" ] || x=$'\n'"$4"
  printf '      - uses: actions/checkout@%s\n        with:\n          repository: %s\n          ref: %s\n          path: %s\n          persist-credentials: false%s' \
    "${SHA}" "$1" "$2" "$3" "${x}"
}
CANON_S3="$(eng3 "${ENGINE}" "${REF}" engine)"

# pack_doc [<top>] [<jobx>] [<s1>] [<s2>] [<s3>] [<later>] [<after>]: 教材 repo の workflow の見本。
#   s1〜s3 は `-` なら正準の step、空文字なら置かない。later は 4 番目以降の step、after は別のジョブ
pack_doc() {
  local top="${1-}" jobx="${2-}" s1="${3--}" s2="${4--}" s3="${5--}" later="${6-}" after="${7-}"
  [ "${s1}" != "-" ] || s1="${CANON_S1}"
  [ "${s2}" != "-" ] || s2="${CANON_S2}"
  [ "${s3}" != "-" ] || s3="${CANON_S3}"
  printf 'name: Pack\non:\n  pull_request:\npermissions:\n  contents: read\n%s\njobs:\n  build:\n    runs-on: ubuntu-latest\n%s\n    steps:\n%s\n%s\n%s\n%s\n%s\n' \
    "${top}" "${jobx}" "${s1}" "${s2}" "${s3}" "${later}" "${after}"
}

# pcase <name> <rc> <tag|-> <content> [<engine-rev file>]: 教材 repo の workflow を検査する
pcase() {
  local f rev="${5:-${GOOD_REV}}"
  seq_no=$((seq_no + 1))
  f="${TMP_DIR}/p-${seq_no}.yml"
  printf '%s\n' "$4" >"${f}"
  case_ "$1" "$2" "$3" --pack-workflow "${f}" --engine-rev "${rev}"
}
# d3case / dcase: 正準の 3 step を外す見本（D3）と、後ろに足す step の見本（D4）
d3case() { pcase "$1 (D3)" 1 D3 "$2"; }
dcase() { pcase "$1 (D4)" 1 D4 "$(pack_doc "" "" - - - "$2")"; }
rs() { printf '      - run: %s' "$1"; }

pcase "canonical three steps pass" 0 - "$(pack_doc)"
printf '0123456789abcdef0123456789abcdef01234567' >"${TMP_DIR}/rev-nonl"
pcase "ENGINE_REV without a trailing newline passes" 0 - "$(pack_doc)" "${TMP_DIR}/rev-nonl"
pcase "name on each step passes" 0 - "$(pack_doc "" "" "${CANON_S1}"$'\n        name: Checkout' "${CANON_S2}"$'\n        name: Read the pin' "${CANON_S3}"$'\n        name: Engine')"
pcase "output step as a one-line block scalar passes" 0 - "$(pack_doc "" "" - $'      - id: rev\n        run: |\n          echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"')"
# 復号すると正準と同じ文字列になる引用の表記は、A〜C と同じく復号した値で判定するため合格する
pcase "output step as a double-quoted YAML string decoding to the canonical run passes" 0 - "$(pack_doc "" "" - $'      - id: rev\n        run: "echo \\"rev=$(cat ENGINE_REV)\\" >> \\"$GITHUB_OUTPUT\\""')"
# YAML のコメントは値の一部ではない（復号した値が正準と一致する）ため、run の行末のコメントは合格する
pcase "a YAML comment after the output run is not part of its value and passes" 0 - "$(pack_doc "" "" - $'      - id: rev\n        run: echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT" # note')"
pcase "step id containing the word outputs passes" 0 - "$(pack_doc "" "" - $'      - id: read_outputs\n        run: echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"' "$(eng3 "${ENGINE}" $'${{ steps.read_outputs.outputs.rev }}' engine)")"
pcase "allowed job and workflow keys pass" 0 - "$(pack_doc $'concurrency:\n  group: pack' $'    timeout-minutes: 30')"
pcase "an unrelated job that uses git is not inspected" 0 - "$(pack_doc "" "" - - - "" $'  lint:\n    runs-on: ubuntu-latest\n    steps:\n      - run: git status')"
pcase "a later step that builds passes" 0 - "$(pack_doc "" "" - - - "$(rs 'cargo build --release')")"
pcase "a later upload-artifact step passes" 0 - "$(pack_doc "" "" - - - $'      - uses: actions/upload-artifact@'"${SHA}"$'\n        with:\n          path: dist')"

# D1 ENGINE_REV の形
revcase() { # <name> <printf format>
  local f="${TMP_DIR}/rev-$((seq_no += 1))"
  # shellcheck disable=SC2059 # 書式そのものが見本の内容
  printf "$2" >"${f}"
  pcase "$1 (D1)" 1 D1 "$(pack_doc)" "${f}"
}
revcase "a short ENGINE_REV is rejected" 'abc123\n'
revcase "an upper-case ENGINE_REV is rejected" '0123456789ABCDEF0123456789ABCDEF01234567\n'
revcase "a trailing space in ENGINE_REV is rejected" '0123456789abcdef0123456789abcdef01234567 \n'
revcase "two lines in ENGINE_REV are rejected" '0123456789abcdef0123456789abcdef01234567\nmain\n'
revcase "CRLF in ENGINE_REV is rejected" '0123456789abcdef0123456789abcdef01234567\r\n'
revcase "two trailing newlines in ENGINE_REV are rejected" '0123456789abcdef0123456789abcdef01234567\n\n'
revcase "an empty ENGINE_REV is rejected" ''
revcase "a branch name in ENGINE_REV is rejected" 'main\n'

# D2 エンジンを正準の形で取得するジョブが無い
pcase "no job fetches the engine (D2)" 1 D2 $'name: P\non: push\njobs:\n  b:\n    runs-on: ubuntu-latest\n    steps:\n      - run: cargo build'
pcase "an engine job fetching another repository is rejected (D2)" 1 D2 "$(pack_doc "" "" - - "$(eng3 someone/else "${REF}" engine)")"

# D3 正準の 3 step・キーの許可リストから外れる
d3case "ref main" "$(pack_doc "" "" - - "$(eng3 "${ENGINE}" main engine)")"
d3case "no ref" "$(pack_doc "" "" - - $'      - uses: actions/checkout@'"${SHA}"$'\n        with:\n          repository: '"${ENGINE}"$'\n          path: engine\n          persist-credentials: false')"
d3case "ref latest" "$(pack_doc "" "" - - "$(eng3 "${ENGINE}" latest engine)")"
d3case "a literal SHA as ref (even the pinned value)" "$(pack_doc "" "" - - "$(eng3 "${ENGINE}" 0123456789abcdef0123456789abcdef01234567 engine)")"
d3case "ref of another step id" "$(pack_doc "" "" - - "$(eng3 "${ENGINE}" $'${{ steps.other.outputs.rev }}' engine)")"
d3case "ref with the dot replaced" "$(pack_doc "" "" - - "$(eng3 "${ENGINE}" $'${{ steps.rev.outputs-rev }}' engine)")"
d3case "output run reading another file" "$(pack_doc "" "" - $'      - id: rev\n        run: echo "rev=$(cat OTHER_REV)" >> "$GITHUB_OUTPUT"')"
d3case "output run with braces around GITHUB_OUTPUT" "$(pack_doc "" "" - $'      - id: rev\n        run: echo "rev=$(cat ENGINE_REV)" >> "${GITHUB_OUTPUT}"')"
d3case "output run with a single-quoted substitution" "$(pack_doc "" "" - $'      - id: rev\n        run: echo \'rev=$(cat ENGINE_REV)\' >> "$GITHUB_OUTPUT"')"
d3case "output run with an unquoted substitution" "$(pack_doc "" "" - $'      - id: rev\n        run: echo rev=$(cat ENGINE_REV) >> "$GITHUB_OUTPUT"')"
d3case "output run mentioning ENGINE_REV only in a comment" "$(pack_doc "" "" - $'      - id: rev\n        run: echo "rev=main" >> "$GITHUB_OUTPUT" # ENGINE_REV')"
d3case "output run written twice (block)" "$(pack_doc "" "" - $'      - id: rev\n        run: |\n          echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"\n          echo "rev=main" >> "$GITHUB_OUTPUT"')"
d3case "output run written twice (semicolon)" "$(pack_doc "" "" - $'      - id: rev\n        run: echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"; echo "rev=main" >> "$GITHUB_OUTPUT"')"
d3case "output run inside an if" "$(pack_doc "" "" - $'      - id: rev\n        run: |\n          if false; then\n            echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"\n          fi')"
d3case "output run after && false" "$(pack_doc "" "" - $'      - id: rev\n        run: false && echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"')"
d3case "output run after exit 0" "$(pack_doc "" "" - $'      - id: rev\n        run: |\n          exit 0\n          echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"')"
d3case "output run inside a here-doc" "$(pack_doc "" "" - $'      - id: rev\n        run: |\n          cat <<EOF\n          echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"\n          EOF')"
for k in 'if: true' 'shell: bash' 'working-directory: sub' $'env:\n          A: b' 'continue-on-error: true'; do
  d3case "output step with an extra key (${k%%:*})" "$(pack_doc "" "" - "${CANON_S2}"$'\n        '"${k}")"
done
d3case "job env" "$(pack_doc "" $'    env:\n      A: b')"
d3case "job defaults" "$(pack_doc "" $'    defaults:\n      run:\n        shell: bash')"
d3case "job container" "$(pack_doc "" $'    container: ubuntu')"
d3case "workflow env" "$(pack_doc $'env:\n  A: b')"
d3case "workflow defaults" "$(pack_doc $'defaults:\n  run:\n    shell: bash')"
d3case "another step before the output step" "$(pack_doc "" "" "${CANON_S1}"$'\n      - run: cargo build' "${CANON_S2}" "${CANON_S3}")"
d3case "no output step" "$(pack_doc "" "" - "")"
d3case "output step after the engine checkout" "$(pack_doc "" "" - "${CANON_S3}" "${CANON_S2}")"
d3case "no checkout of this repository" "$(pack_doc "" "" "")"
d3case "checkout of this repository with a path" "$(pack_doc "" "" $'      - uses: actions/checkout@'"${SHA}"$'\n        with:\n          path: self\n          persist-credentials: false')"
d3case "checkout of this repository with persist-credentials: true" "$(pack_doc "" "" $'      - uses: actions/checkout@'"${SHA}"$'\n        with:\n          persist-credentials: true')"
d3case "engine checkout with an extra input" "$(pack_doc "" "" - - "$(eng3 "${ENGINE}" "${REF}" engine '          fetch-depth: 0')")"
d3case "engine checkout with persist-credentials: true" "$(pack_doc "" "" - - $'      - uses: actions/checkout@'"${SHA}"$'\n        with:\n          repository: '"${ENGINE}"$'\n          ref: '"${REF}"$'\n          path: engine\n          persist-credentials: true')"
d3case "engine checkout with a tag-pinned uses" "$(pack_doc "" "" - - $'      - uses: actions/checkout@v4\n        with:\n          repository: '"${ENGINE}"$'\n          ref: '"${REF}"$'\n          path: engine\n          persist-credentials: false')"
d3case "capitalised REF key" "$(pack_doc "" "" - - $'      - uses: actions/checkout@'"${SHA}"$'\n        with:\n          repository: '"${ENGINE}"$'\n          REF: '"${REF}"$'\n          path: engine\n          persist-credentials: false')"
d3case "capitalised Uses key" "$(pack_doc "" "" - - $'      - Uses: actions/checkout@'"${SHA}"$'\n        with:\n          repository: '"${ENGINE}"$'\n          ref: '"${REF}"$'\n          path: engine\n          persist-credentials: false')"
d3case "an engine checkout whose repository is an expression" "$(pack_doc "" "" - $'      - uses: actions/checkout@'"${SHA}"$'\n        with:\n          repository: ${{ inputs.repo }}' "")"

# D4 正準の後ろの step がエンジンの取得・版の変更に触れる
dcase "git clone of the engine" "$(rs 'git clone https://example.invalid/Fandhe-AI/fandhe-workshop engine2')"
dcase "git clone to the current directory" "$(rs 'git clone https://example.invalid/x .')"
dcase "git clone with a trailing flag" "$(rs 'git clone https://example.invalid/x --depth 1')"
dcase "git clone to the default directory" "$(rs 'git clone https://example.invalid/x')"
dcase "git checkout of the pinned value" "$(rs $'git checkout "$(cat ENGINE_REV)"')"
dcase "git fetch" "$(rs 'git fetch origin main')"
dcase "git reset" "$(rs 'git reset origin/main')"
dcase "git reset --hard" "$(rs 'git reset --hard origin/main')"
dcase "git switch --detach" "$(rs 'git switch --detach main')"
dcase "git checkout -- a path" "$(rs 'git checkout -- engine')"
dcase "cd then git checkout" "$(rs 'cd engine && git checkout main')"
dcase "git checkout main after the pin" "$(rs 'git checkout main')"
dcase "git -C engine checkout main" "$(rs 'git -C engine checkout main')"
dcase "git -C with a variable" "$(rs $'git -C "$DIR" checkout main')"
dcase "git -C engine status" "$(rs 'git -C engine status')"
dcase "cd engine and git pull" "$(rs 'cd engine && git pull')"
dcase "sh -c with cd and git fetch" "$(rs "sh -c 'cd engine && git fetch origin'")"
dcase "an alias of git" "$(rs 'alias g=git')"
dcase "echo git (display only)" "$(rs 'echo git clone x')"
dcase "a colon command mentioning git" $'      - run: ": git status"'
dcase "writing main into ENGINE_REV" "$(rs 'echo main > ENGINE_REV')"
dcase "cargo build with the engine manifest path" "$(rs 'cargo build --manifest-path engine/Cargo.toml')"
dcase "cargo build with the engine path in another case" "$(rs 'cargo build --manifest-path ENGINE/Cargo.toml')"
dcase "cargo install --git with the pinned value" "$(rs $'cargo install --git https://example.invalid/x --rev "$(cat ENGINE_REV)"')"
dcase "git after && in a block" $'      - run: |\n          true && git clone https://example.invalid/x engine2'
dcase "git after || in a block" $'      - run: |\n          false || git clone https://example.invalid/x engine2'
dcase "git inside an if in a block" $'      - run: |\n          if true; then\n            git clone https://example.invalid/x engine2\n          fi'
dcase "git after a trailing && continuation" $'      - run: |\n          true &&\n            git clone https://example.invalid/x engine2'
dcase "git after a backslash continuation" $'      - run: |\n          true \\\n            && git clone https://example.invalid/x engine2'
dcase "git inside a here-doc" $'      - run: |\n          cat <<EOF\n          git clone https://example.invalid/x engine2\n          EOF'
dcase "git inside a multi-line quote" $'      - run: |\n          sh -c "\n            git clone https://example.invalid/x engine2\n          "'
dcase "git in a folded scalar" $'      - run: >-\n          git clone https://example.invalid/x engine2'
dcase "clone then checkout of a different version" $'      - run: |\n          git clone https://example.invalid/x engine2\n          cd engine2\n          git checkout main'
dcase "checkout without a path" $'      - uses: actions/checkout@'"${SHA}"
dcase "checkout with the engine path" $'      - uses: actions/checkout@'"${SHA}"$'\n        with:\n          path: engine'
dcase "checkout with a sibling path" $'      - uses: actions/checkout@'"${SHA}"$'\n        with:\n          path: other'
dcase "checkout with a quoted uses value" $'      - uses: "actions/checkout@'"${SHA}"$'"'
dcase "checkout with a capitalised Uses key" $'      - Uses: actions/checkout@'"${SHA}"
dcase "checkout with a folded uses value" $'      - uses: >-\n          actions/checkout@'"${SHA}"
dcase "checkout with a block uses value" $'      - uses: |\n          actions/checkout@'"${SHA}"
dcase "checkout with a quoted key" $'      - "uses": actions/checkout@'"${SHA}"
dcase "checkout as a flow-style step" $'      - {uses: actions/checkout@'"${SHA}"$'}'
dcase "checkout whose repository is an expression" $'      - uses: actions/checkout@'"${SHA}"$'\n        with:\n          repository: ${{ inputs.repo }}'
dcase "checkout whose ref is an expression" $'      - uses: actions/checkout@'"${SHA}"$'\n        with:\n          ref: ${{ inputs.ref }}'
dcase "checkout whose path is an expression" $'      - uses: actions/checkout@'"${SHA}"$'\n        with:\n          path: ${{ inputs.path }}'
dcase "another action whose path input is an expression" $'      - uses: actions/upload-artifact@'"${SHA}"$'\n        with:\n          path: ${{ inputs.path }}'
dcase "an escaped git in a double-quoted run (decoded before judging)" $'      - run: "\\x67it clone https://example.invalid/x engine2"'
dcase "a capitalised Run key with git" $'      - RUN: git clone https://example.invalid/x engine2'

# 復号した値で判定する（表記の違いで抜けられない）ことの確認
pcase "an anchor on the output run is rejected (Y)" 1 Y "$(pack_doc "" "" - $'      - id: rev\n        run: &r echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"')"
pcase "an alias is rejected (Y)" 1 Y "$(pack_doc $'x: &a 1\ny: *a')"
pcase "a duplicate with key is rejected (Y)" 1 Y "$(pack_doc "" "" - - "${CANON_S3}"$'\n        with:\n          path: x')"
pcase "a merge key is rejected (Y)" 1 Y "$(pack_doc "" $'    x:\n      <<: {a: 1}')"

# 出力に復号した値を出さない
pack_out() { # <name> <workflow content> [<engine-rev file>]: 違反の出力に LEAK が含まれないこと
  local f="${TMP_DIR}/pout-$((seq_no += 1)).yml" rc=0 rev="${3:-${GOOD_REV}}"
  printf '%s\n' "$2" >"${f}"
  "${CHECK}" --secrets-checker "${STUB}" --pack-workflow "${f}" --engine-rev "${rev}" >"${TMP_DIR}/out" 2>"${TMP_DIR}/err" || rc=$?
  if [ "${rc}" = 1 ] && ! grep -q -- "${LEAK}" "${TMP_DIR}/err" "${TMP_DIR}/out"; then
    echo "ok - ${1}"
    passed=$((passed + 1))
  else
    echo "not ok - ${1} (exit ${rc}, or the value leaked)"
    sed 's/^/# /' "${TMP_DIR}/err"
    failed=$((failed + 1))
  fi
}
pack_out "a later step's run value is not echoed" "$(pack_doc "" "" - - - "      - run: git clone https://${LEAK}.invalid/x")"
pack_out "a job key is not echoed" "$(pack_doc "" "    ${LEAK}: x")"
printf '%s\n' "${LEAK}" >"${TMP_DIR}/rev-leak"
pack_out "the ENGINE_REV content is not echoed" "$(pack_doc)" "${TMP_DIR}/rev-leak"

# 誤用・検査を完了できない（終了コード 2）
printf '%s\n' "$(pack_doc)" >"${TMP_DIR}/pack-ok.yml"
case_ "--pack-workflow without --engine-rev exits 2" 2 - --pack-workflow "${TMP_DIR}/pack-ok.yml"
case_ "--engine-rev without --pack-workflow exits 2" 2 - --engine-rev "${GOOD_REV}"
case_ "a missing pack workflow exits 2" 2 - --pack-workflow "${TMP_DIR}/none.yml" --engine-rev "${GOOD_REV}"
case_ "a missing ENGINE_REV exits 2" 2 - --pack-workflow "${TMP_DIR}/pack-ok.yml" --engine-rev "${TMP_DIR}/none"
printf 'a: [unclosed\n' >"${TMP_DIR}/pack-broken.yml"
case_ "a YAML syntax error in the pack workflow exits 2" 2 - --pack-workflow "${TMP_DIR}/pack-broken.yml" --engine-rev "${GOOD_REV}"

# ---- Y: YAML の曖昧な表記 ----
echo "# Y: ambiguous YAML notation"
ycase "alias is rejected (Y)" 1 Y ci "$(ci_doc - $'x: &a 1\ny: *a' "" "")"
ycase "anchor is rejected (Y)" 1 Y ci "$(ci_doc - 'x: &a 1' "" "")"
ycase "explicit tag is rejected (Y)" 1 Y ci "$(ci_doc - 'x: !!str 1' "" "")"
ycase "merge key is rejected (Y)" 1 Y ci "$(ci_doc - $'x:\n  <<: {a: 1}' "" "")"
ycase "duplicate key is rejected (Y)" 1 Y ci "$(ci_doc - $'x: 1\nx: 2' "" "")"
ycase "case-insensitive duplicate key is rejected (Y)" 1 Y ci "$(ci_doc - $'x: 1\nX: 2' "" "")"
ycase "complex key is rejected (Y)" 1 Y ci "$(ci_doc - $'? [a]\n: b' "" "")"
ycase "multiple documents are rejected (Y)" 1 Y ci "$(ci_doc - "" "" ""; printf -- '---\nname: second\n')"
ycase "a %YAML directive is rejected (Y)" 1 Y ci "$(printf '%%YAML 1.1\n---\n'; ci_doc - "" "" "")"
ycase "a non-mapping root is rejected (Y)" 1 Y ci '- a'

# ---- 出力に値を出さない ----
echo "# output never contains decoded values"
ycase_out() { # <name> <ci content>: 違反の出力に LEAK が含まれないこと
  local f="${TMP_DIR}/out-$((seq_no += 1)).yml" rc=0
  printf '%s\n' "$2" >"${f}"
  "${CHECK}" --secrets-checker "${STUB}" --ci "${f}" >"${TMP_DIR}/out" 2>"${TMP_DIR}/err" || rc=$?
  if [ "${rc}" = 1 ] && ! grep -q -- "${LEAK}" "${TMP_DIR}/err" "${TMP_DIR}/out"; then
    echo "ok - ${1}"
    passed=$((passed + 1))
  else
    echo "not ok - ${1} (exit ${rc}, or the value leaked)"
    sed 's/^/# /' "${TMP_DIR}/err"
    failed=$((failed + 1))
  fi
}
ycase_out "a violating run value is not echoed" "$(ci_doc - "" "" "      - run: echo ${LEAK} submodule")"
ycase_out "a violating uses value is not echoed" "$(ci_doc - "" "" "      - uses: ${LEAK}/x@main")"
ycase_out "a violating env key is not echoed" "$(ci_doc - "" "    env:
      GIT_${LEAK}: x" "")"
rc=0
printf '{"lockfileVersion":3,"packages":{"node_modules/a":{"resolved":"https://user:%s%shost.invalid/a.tgz","integrity":"x","link":true}}}\n' "${LEAK}" "${AT}" >"${TMP_DIR}/lock-leak.json"
"${CHECK}" --package-lock "${TMP_DIR}/lock-leak.json" >"${TMP_DIR}/out" 2>"${TMP_DIR}/err" || rc=$?
if [ "${rc}" = 1 ] && ! grep -q -- "${LEAK}" "${TMP_DIR}/err"; then
  echo "ok - an npm resolved URL with credentials is not echoed"
  passed=$((passed + 1))
else
  echo "not ok - an npm resolved URL with credentials is not echoed (exit ${rc})"
  failed=$((failed + 1))
fi
rc=0
meta_json "\"git+https://user:${LEAK}${AT}host.invalid/r#abc\"" >"${TMP_DIR}/meta-leak.json"
"${CHECK}" --cargo-metadata-json "${TMP_DIR}/meta-leak.json" >"${TMP_DIR}/out" 2>"${TMP_DIR}/err" || rc=$?
if [ "${rc}" = 1 ] && ! grep -q -- "${LEAK}" "${TMP_DIR}/err"; then
  echo "ok - a cargo source URL with credentials is not echoed"
  passed=$((passed + 1))
else
  echo "not ok - a cargo source URL with credentials is not echoed (exit ${rc})"
  failed=$((failed + 1))
fi
rc=0
printf '//host.invalid/:_authToken=%s\n' "${LEAK}" >"${TMP_DIR}/npmrc-leak"
"${CHECK}" --npmrc "${TMP_DIR}/npmrc-leak" >"${TMP_DIR}/out" 2>"${TMP_DIR}/err" || rc=$?
if [ "${rc}" = 1 ] && ! grep -q -- "${LEAK}" "${TMP_DIR}/err"; then
  echo "ok - an .npmrc credential line is not echoed"
  passed=$((passed + 1))
else
  echo "not ok - an .npmrc credential line is not echoed (exit ${rc})"
  failed=$((failed + 1))
fi

# ---- fail-closed（終了コード 2）----
echo "# fail-closed (exit 2)"
case_ "missing ci file exits 2" 2 - --ci "${TMP_DIR}/none.yml"
case_ "missing deploy file exits 2" 2 - --deploy "${TMP_DIR}/none.yml"
case_ "missing package-lock exits 2" 2 - --package-lock "${TMP_DIR}/none.json"
case_ "missing .npmrc exits 2" 2 - --npmrc "${TMP_DIR}/none"
case_ "missing .gitmodules exits 2" 2 - --gitmodules "${TMP_DIR}/none"
case_ "missing cargo metadata JSON exits 2" 2 - --cargo-metadata-json "${TMP_DIR}/none.json"
printf 'a: [unclosed\n' >"${TMP_DIR}/broken.yml"
case_ "YAML syntax error in ci exits 2" 2 - --ci "${TMP_DIR}/broken.yml"
case_ "YAML syntax error in deploy exits 2" 2 - --deploy "${TMP_DIR}/broken.yml"
printf 'a: "\xff\xfe"\n' >"${TMP_DIR}/bad-utf8.yml"
case_ "invalid UTF-8 exits 2" 2 - --ci "${TMP_DIR}/bad-utf8.yml"
printf '{ not json' >"${TMP_DIR}/broken.json"
case_ "broken package-lock.json exits 2" 2 - --package-lock "${TMP_DIR}/broken.json"
case_ "invalid cargo metadata JSON exits 2" 2 - --cargo-metadata-json "${TMP_DIR}/broken.json"
printf '{"packages":[]}\n' >"${TMP_DIR}/meta-nows.json"
case_ "cargo metadata JSON without workspace_members exits 2" 2 - --cargo-metadata-json "${TMP_DIR}/meta-nows.json"
printf '[submodule "a"\n' >"${TMP_DIR}/gm-broken"
case_ "unparsable .gitmodules exits 2" 2 - --gitmodules "${TMP_DIR}/gm-broken"
case_ "unknown option exits 2" 2 - --bogus x
case_ "option without a value exits 2" 2 - --ci
case_ "positional argument exits 2" 2 - stray

# 道具が無いとき（ruby を含まない PATH）に、合格にせず 2 で止まること
mkdir -p "${TMP_DIR}/nobin"
for c in bash dirname basename cat tr grep wc jq git sed awk uname cd; do
  p="$(command -v "${c}" 2>/dev/null || true)"
  case "${p}" in /*) ln -sf "${p}" "${TMP_DIR}/nobin/${c}" ;; esac
done
rc=0
printf 'name: CI\n' >"${TMP_DIR}/min.yml"
env PATH="${TMP_DIR}/nobin" "${TMP_DIR}/nobin/bash" "${CHECK}" --secrets-checker "${STUB}" --ci "${TMP_DIR}/min.yml" >/dev/null 2>"${TMP_DIR}/err" || rc=$?
verdict "missing ruby exits 2" 2 - "${rc}" "${TMP_DIR}/err"
rc=0
env PATH="${TMP_DIR}/nobin" "${TMP_DIR}/nobin/bash" "${CHECK}" --secrets-checker "${STUB}" --pack-workflow "${TMP_DIR}/pack-ok.yml" --engine-rev "${GOOD_REV}" >/dev/null 2>"${TMP_DIR}/err" || rc=$?
verdict "missing ruby exits 2 for --pack-workflow" 2 - "${rc}" "${TMP_DIR}/err"

# ---- 実際の cargo を通す ----
echo "# real cargo"
CR="${TMP_DIR}/cr"
mkdir -p "${CR}/ext/src" "${CR}/app/src"
printf '[package]\nname = "ext"\nversion = "0.1.0"\nedition = "2021"\n' >"${CR}/ext/Cargo.toml"
: >"${CR}/ext/src/lib.rs"
printf '[package]\nname = "app"\nversion = "0.1.0"\nedition = "2021"\n\n[dependencies]\next = { path = "../ext" }\n' >"${CR}/app/Cargo.toml"
printf 'fn main() {}\n' >"${CR}/app/src/main.rs"
cp "${ROOT_DIR}/rust-toolchain.toml" "${CR}/app/rust-toolchain.toml"
if (cd "${CR}/app" && cargo generate-lockfile --offline >/dev/null 2>&1); then
  case_ "a path dependency outside the workspace is rejected by real cargo (A1)" 1 A1 --cargo-manifest "${CR}/app/Cargo.toml"
  # lock と manifest がずれていれば --locked が失敗し、2 で止まる
  mkdir -p "${CR}/ext2/src"
  printf '[package]\nname = "ext2"\nversion = "0.1.0"\nedition = "2021"\n' >"${CR}/ext2/Cargo.toml"
  : >"${CR}/ext2/src/lib.rs"
  printf 'ext2 = { path = "../ext2" }\n' >>"${CR}/app/Cargo.toml"
  case_ "an outdated Cargo.lock makes cargo --locked fail and exits 2" 2 - --cargo-manifest "${CR}/app/Cargo.toml"
else
  echo "not ok - cargo generate-lockfile --offline failed for the sample crate"
  failed=$((failed + 1))
fi

echo "# ${passed} passed, ${failed} failed"
[ "${failed}" -eq 0 ]
