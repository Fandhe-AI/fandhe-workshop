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
ycase "allowed CARGO_ env keys pass" 0 - ci "$(ci_doc - "" $'    env:\n      CARGO_TERM_COLOR: always\n      CARGO_INCREMENTAL: "0"' "")"
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
