#!/usr/bin/env bash
# fork が secret を設定せずに CI を流しても、ビルドと試験が通る構成であることを静的に検査する
# （`make check-fork-ci` と ci.yml の check ジョブから呼ばれる。TASK-96、BUILD-22）。
#
# 方針: ファイルを字句（awk・grep）ではなく実際のパーサで復号し、復号した値で判定する。
# 引用符つきのキー・エスケープ・フロー形式・複数行の値・空白なしの `key="value"` といった表記の違いで検査を
# 抜けられないようにするため（表記の回避が収束しなかった先行の字句実装の反省）。
#   - YAML（ci.yml・deploy.yml）: ruby 標準の Psych でノード木にし、復号済みのスカラーで判定する
#   - Cargo の依存元: `cargo metadata --locked` が返す JSON を jq で判定する（Cargo.lock は cargo が復号する）
#   - package-lock.json: jq
#   - .gitmodules: git 自身の設定パーサ（`git config --file -z --list`）
#   - .npmrc: パーサを使わず、行の完全一致の許可リストで判定する
#   - .cargo/config(.toml): 復号せず、存在自体を拒否する
#
# 判定項目（エラー行の `<項目>` がこの名前で出る。回帰テストは項目名で「正しい理由で失敗したか」を確かめる）
#   A private な依存が 0 件で、submodule を取得しない
#     A1 Cargo の全 package が crates.io 由来（source 無しは workspace の member だけ）
#     A2 package-lock.json の全項目が npm registry の resolved と integrity を持つ（git・file・link は拒否）
#     A3 .npmrc は空行・コメント・`engine-strict=true` だけ（registry・認証情報を書かせない）
#     A4 .cargo/config(.toml) が無い（`[source]` の置き換えは cargo metadata に現れないため、存在自体を拒否する）
#     A5 .gitmodules の全 submodule が `update = none`。ci.yml が submodule を取得しない
#        （checkout の submodules が false 以外・run に submodule の語・env の GIT_ キーを拒否する）
#   B ci.yml からの secret の参照が 0 件
#     B1 scripts/check-workflow-secrets.sh を呼ぶ（検査を二重に書かない）
#     B2 トークン・PAT・git-fetch-with-cli の語、許可外の CARGO_ の env キーを拒否する
#     B3 uses は許可した所有者の 40 桁 SHA 固定だけ（docker://・./・タグ固定・reusable workflow は拒否）
#   C 配信の切り離し
#     C1 ci.yml のトップレベルの permissions が読み取り専用で、environment を持たない
#     C2 deploy.yml は workflow_dispatch だけで動き、全ジョブが fork で skip される
#     C3 ci.yml に配信のジョブ（名前に deploy を含む）を置かない
#   D 教材 repo の workflow が、固定したエンジンの版（`ENGINE_REV`。C-14）でエンジンを取得する
#     （`--pack-workflow` と `--engine-rev` を渡したときだけ動く。本 repo の CI の既定の実行には入らない）
#     D1 ENGINE_REV が 40 桁の小文字の 16 進を 1 行だけ持つ（末尾の改行は 1 つまで）
#     D2 正準の 3 step で始まるエンジンのジョブが 1 つ以上ある
#     D3 エンジンのジョブが正準の 3 step で始まり、ジョブと workflow のキーが許可リストの中だけ
#     D4 4 番目以降の step が actions/checkout を使わず、git の語・エンジンの repo・ENGINE_REV・エンジンの path に触れない
#   Y YAML の曖昧な表記を解釈せず拒否する（別名・アンカー・明示のタグ・複数文書・指令・マージキー・
#     スカラーでないキー・重複キー）。重複キーは大文字小文字を区別せずに比べる
#     （`with:` の入力は大文字の環境変数になるため、`Submodules:` も有効なキーになる）
#
# 正準の 3 step（D の合格はこの形 1 つだけ。キーは綴りの完全一致で、`name` だけは各 step に足してよい）
#     - uses: actions/checkout@<40 桁 SHA>
#       with:
#         persist-credentials: false
#     - id: <id>
#       run: echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"
#     - uses: actions/checkout@<40 桁 SHA>
#       with:
#         repository: Fandhe-AI/fandhe-workshop
#         ref: ${{ steps.<id>.outputs.rev }}
#         path: <dir>
#         persist-credentials: false
# D はシェルの意味（clone 先の追跡・`&&`・here-doc 等）を解釈しない。正準の形に外れたものと、4 番目以降の step が
# 触れたものを、復号した値の許可リストで一律に拒否する（先行の字句的な実装が回避の指摘で収束しなかった反省）。
# エンジンは教材 repo 自身のビルド定義（Cargo.toml の path 依存・Makefile）から使う前提で、後続の step は触れない。
#
# 検査の限界（README にも書く）
#   - 自分たちが書く構成の誤りを防ぐための検査であり、悪意ある変更を防ぐ境界ではない
#   - `run` の中のシェルの意味は解釈しない。`submodule` の語の有無だけを見る（難読化・変数での組み立て・eval は
#     見抜けない）
#   - `run` から呼ぶスクリプト（scripts/*.sh・make のターゲット）の中身は解釈しない
#   - runner・利用者の環境の設定（~/.npmrc・$CARGO_HOME/config.toml・global の git 設定）は対象外
#   - D: git clone・cargo install --git 等による取得は正準に含めず一律に拒否する。エンジンに触れないジョブは検査しない。
#     文字列を組み立てた URL・action の内部の動き・artifact の受け渡しは見抜けない。`--engine-rev` には workflow が読む
#     のと同じ（教材 repo の直下の）ENGINE_REV を渡す前提である。TMPL-9 で案 B を採る場合の作り直しは TASK-765 の範囲
#   - 実際の fork での実行はしていない。D は試験の見本でだけ確かめており、実際の教材 repo（TASK-36）への適用もしていない
#     （D-29 の行わない確認。2026-10-19 時点）
#
# 出力: 違反は `error: <file>:<行>:<項目> <理由>`（YAML 以外は行が無く `error: <file>:<項目> <理由>`）を標準エラーへ
# 1 行ずつ出す。値は出さない（復号した値・resolved の URL・source の URL・.npmrc の行・cargo と git の標準エラー）。
# 出すのは行番号・項目名・固定の理由と、name@version・lock のキーといった識別子だけ。
#
# 使い方: scripts/check-fork-ci.sh [オプション]
#   引数なし: 本 repo の全ファイルを検査する。下のオプションを 1 つでも渡すと、渡したものだけを検査する
#   --ci <file>                  ci.yml（A5・B・C1・C3 と Y。B1 を含む）
#   --deploy <file>              deploy.yml（C2 と Y）
#   --cargo-manifest <Cargo.toml> A1 を cargo metadata --locked で判定する
#   --cargo-metadata-json <file> A1 を、cargo metadata の出力 JSON で判定する（回帰テストだけが使う。
#                                オフラインでは解決できない git・別 registry の source の見本を作るため）
#   --package-lock <file>        A2
#   --npmrc <file>               A3
#   --gitmodules <file>          A5（.gitmodules の部分）
#   --cargo-config-dir <dir>     A4（<dir>/config と <dir>/config.toml の存在を拒否する）
#   --pack-workflow <file>       D2〜D4 と Y（教材 repo の workflow。--engine-rev と組で渡す。片方だけは誤用）
#   --engine-rev <file>          D1（教材 repo の ENGINE_REV）
#   --secrets-checker <script>   B1 の実体の差し替え（回帰テストだけが使う）
# 終了コード: 0 = 合格 / 1 = 違反あり / 2 = 誤用、または検査を完了できない（ファイルが無い・道具が無い・
#   パースに失敗・B1 が 2・cargo の失敗）。2 でも合格にしない（fail-closed）。
# 回帰テスト: scripts/test-check-fork-ci.sh
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

# uses に許す所有者（カンマ区切り）。サードパーティ action を足すときは、この定数と SHA 固定を同時に見直す
ALLOWED_USES_OWNERS="actions"
# 全ジョブの if: に要求する配信元リポジトリ。fork 自身の github.repository を許可値にすると制限の目的が
# 失われるため、固定の設定値として判定処理（ruby）へ引数で渡す
UPSTREAM_REPO="Fandhe-AI/fandhe-workshop"
# D が教材 repo の workflow に要求するエンジンの repo。UPSTREAM_REPO と値は同じだが、意味（fork 判定の配信元と取得対象）が
# 違うため別の定数にする
ENGINE_REPO="Fandhe-AI/fandhe-workshop"
CRATES_REGISTRY="registry+https://github.com/rust-lang/crates.io-index"
NPM_REGISTRY_PREFIX="https://registry.npmjs.org/"

usage() {
  echo "usage: $(basename "$0") [--ci <file>] [--deploy <file>] [--cargo-manifest <Cargo.toml>] [--cargo-metadata-json <file>]" \
    "[--package-lock <file>] [--npmrc <file>] [--gitmodules <file>] [--cargo-config-dir <dir>]" \
    "[--pack-workflow <file> --engine-rev <file>] [--secrets-checker <script>]" >&2
  exit 2
}

CI_FILE=""
DEPLOY_FILE=""
CARGO_MANIFEST=""
CARGO_META_JSON=""
PKG_LOCK=""
NPMRC_FILE=""
GITMODULES_FILE=""
CARGO_CFG_DIR=""
PACK_WORKFLOW=""
ENGINE_REV_FILE=""
SECRETS_CHECKER="${DIR}/check-workflow-secrets.sh"
SELECTED=0
while [ "$#" -gt 0 ]; do
  [ "$#" -ge 2 ] || usage
  case "$1" in
    --ci) CI_FILE="$2" ;;
    --deploy) DEPLOY_FILE="$2" ;;
    --cargo-manifest) CARGO_MANIFEST="$2" ;;
    --cargo-metadata-json) CARGO_META_JSON="$2" ;;
    --package-lock) PKG_LOCK="$2" ;;
    --npmrc) NPMRC_FILE="$2" ;;
    --gitmodules) GITMODULES_FILE="$2" ;;
    --cargo-config-dir) CARGO_CFG_DIR="$2" ;;
    --pack-workflow) PACK_WORKFLOW="$2" ;;
    --engine-rev) ENGINE_REV_FILE="$2" ;;
    --secrets-checker) SECRETS_CHECKER="$2"; shift 2; continue ;;
    *) usage ;;
  esac
  SELECTED=$((SELECTED + 1))
  shift 2
done
if [ "${SELECTED}" -eq 0 ]; then
  CI_FILE="${ROOT_DIR}/.github/workflows/ci.yml"
  DEPLOY_FILE="${ROOT_DIR}/.github/workflows/deploy.yml"
  CARGO_MANIFEST="${ROOT_DIR}/Cargo.toml"
  PKG_LOCK="${ROOT_DIR}/package-lock.json"
  NPMRC_FILE="${ROOT_DIR}/.npmrc"
  GITMODULES_FILE="${ROOT_DIR}/.gitmodules"
  CARGO_CFG_DIR="${ROOT_DIR}/.cargo"
fi

# D は 2 つのファイルが揃って意味を持つ。片方だけでは検査を完了できないため、誤用として止める
if [ -n "${PACK_WORKFLOW}" ] && [ -z "${ENGINE_REV_FILE}" ]; then usage; fi
if [ -z "${PACK_WORKFLOW}" ] && [ -n "${ENGINE_REV_FILE}" ]; then usage; fi

# 検査を流す前に、必要な道具を確かめる（無ければ exit 2。合格にしない）
if [ -n "${CI_FILE}" ] || [ -n "${DEPLOY_FILE}" ] || [ -n "${PACK_WORKFLOW}" ]; then require_cmd ruby || exit 2; fi
if [ -n "${CARGO_MANIFEST}" ] || [ -n "${CARGO_META_JSON}" ] || [ -n "${PKG_LOCK}" ]; then require_cmd jq || exit 2; fi
if [ -n "${CARGO_MANIFEST}" ] && [ -z "${CARGO_META_JSON}" ]; then require_cmd cargo || exit 2; fi
if [ -n "${GITMODULES_FILE}" ]; then require_cmd git || exit 2; fi

VIOLATIONS=0
report() {
  echo "error: $*" >&2
  VIOLATIONS=$((VIOLATIONS + 1))
}

need_file() {
  if [ ! -f "$1" ] || [ ! -r "$1" ]; then
    echo "error: cannot read $1" >&2
    exit 2
  fi
}

# 識別子として出してよい文字だけを残す（name@version・lock のキー・submodule 名用）
safe_id() {
  printf '%s' "$1" | tr -cd 'A-Za-z0-9@._+/:-'
}

# ---- YAML（Psych のノード木）----
# 値をシェルに埋め込まず、ファイルのパスだけを引数で渡す。キーは小文字化して照合する。
# スカラーは bool・数値へ変換せず復号済みの文字列のまま比べる（`false` と `"false"` はどちらも文字列 false）。
# Psych.safe_load が返すハッシュは使わない: YAML 1.1 として `on` を true に変換し、`"on"` で引くと見落とすため。
# 出力は `<行>:<項目> <理由>`。ruby の終了コード 0 = 合格 / 1 = 違反あり / 2 = 解析できない。
# `read -d ''` はヒアドキュメントの末尾（EOF）で必ず 1 を返すため、`|| true` は握りつぶしではなく
# 変数への取り込みの定型である（$(cat <<...) は bash 3.2 でヒアドキュメント内の引用符を誤解析するので使わない）。
# shellcheck disable=SC2016 # ruby のプログラムなので、シェルに展開させない
read -r -d '' RUBY_PROG <<'RUBY' || true
require "psych"

mode = ARGV[0]
file = ARGV[1]
owners = (ARGV[2] || "").split(",")
upstream = ARGV[3].to_s
abort("upstream repository is not given") if upstream.empty?
engine = ARGV[4].to_s
GUARD = "github.repository == '#{upstream}'".freeze
CARGO_ENV_ALLOWED = %w[cargo_term_color cargo_incremental cargo_profile_dev_debug cargo_target_dir].freeze
N = Psych::Nodes

$viol = []

def add(node, item, msg)
  line = node.respond_to?(:start_line) && node.start_line ? node.start_line + 1 : 0
  $viol << [line, item, msg]
end

def finish
  $viol.sort_by { |l, i, m| [l, i, m] }.each { |l, i, m| puts "#{l}:#{i} #{m}" }
  exit($viol.empty? ? 0 : 1)
end

begin
  src = File.binread(file).force_encoding("UTF-8")
  unless src.valid_encoding?
    warn "error: #{file} is not valid UTF-8"
    exit 2
  end
  stream = Psych.parse_stream(src)
rescue Psych::SyntaxError => e
  warn "error: cannot parse #{file} as YAML (line #{e.line}, column #{e.column}): #{e.problem}"
  exit 2
rescue StandardError => e
  warn "error: cannot read or parse #{file}: #{e.class}"
  exit 2
end

docs = stream.children
if docs.size != 1
  add(docs[1] || stream, "Y", "the file must hold exactly one YAML document")
  finish
end
doc = docs[0]
add(doc, "Y", "YAML directive is not accepted") if doc.version && !doc.version.empty?
add(doc, "Y", "TAG directive is not accepted") unless doc.tag_directives.empty?
root = doc.root
unless root.is_a?(N::Mapping)
  add(doc, "Y", "the document root must be a mapping")
  finish
end

# ---- 項目 Y: 表記そのものを受け付けない ----
def walk(n)
  if n.is_a?(N::Alias)
    add(n, "Y", "alias is not accepted")
    return
  end
  add(n, "Y", "anchor is not accepted") if n.anchor
  add(n, "Y", "explicit tag is not accepted") if n.tag
  case n
  when N::Mapping
    seen = {}
    n.children.each_slice(2) do |k, v|
      if k.is_a?(N::Scalar)
        kv = k.value.downcase
        add(k, "Y", "merge key is not accepted") if kv == "<<"
        add(k, "Y", "duplicate key (compared case-insensitively)") if seen[kv]
        seen[kv] = true
      else
        add(k, "Y", "non-scalar key is not accepted")
      end
      walk(k)
      walk(v)
    end
  when N::Sequence
    n.children.each { |c| walk(c) }
  end
end
walk(root)

# ---- 補助 ----
def pairs(m)
  m.is_a?(N::Mapping) ? m.children.each_slice(2).to_a : []
end

def mget(m, key)
  pairs(m).each { |k, v| return v if k.is_a?(N::Scalar) && k.value.downcase == key }
  nil
end

def mkey(m, key)
  pairs(m).each { |k, _| return k if k.is_a?(N::Scalar) && k.value.downcase == key }
  nil
end

def str(n)
  n.is_a?(N::Scalar) ? n.value : nil
end

def each_node(n, &blk)
  yield n
  n.children.each { |c| each_node(c, &blk) } if n.children
end

# permissions は、空のマッピングか、全ての値が read / none のマッピングだけを許す
# （スカラー形の read-all・write-all、式、write は拒否）
def perm_ok?(n)
  n.is_a?(N::Mapping) && pairs(n).all? { |_, v| %w[read none].include?(str(v)) }
end

# ---- ci.yml ----
def check_ci(root, owners)
  uses_re = /\A(?:#{owners.map { |o| Regexp.escape(o) }.join("|")})\/[A-Za-z0-9._-]+@[0-9a-f]{40}\z/
  # C1 トップレベルの permissions
  perm = mget(root, "permissions")
  if perm.nil?
    add(root, "C1", "top-level permissions is required")
  elsif !perm_ok?(perm)
    add(perm, "C1", "top-level permissions must be read-only (read / none)")
  end

  jobs = mget(root, "jobs")
  if jobs.is_a?(N::Mapping)
    pairs(jobs).each do |jk, jv|
      add(jk, "C3", "a job whose key contains deploy must not be in ci.yml") if str(jk).to_s.downcase.include?("deploy")
      unless jv.is_a?(N::Mapping)
        add(jk, "C3", "a job must be a mapping")
        next
      end
      jp = mget(jv, "permissions")
      add(jp, "C1", "job permissions must be read-only (read / none)") if jp && !perm_ok?(jp)
      add(mkey(jv, "environment"), "C1", "environment is not allowed in ci.yml") if mget(jv, "environment")
      add(mkey(jv, "uses"), "B3", "reusable workflow is not allowed in ci.yml") if mget(jv, "uses")
    end
  else
    add(root, "C3", "jobs must be a mapping")
  end

  # 階層を問わず、キーと値の対で判定する
  each_node(root) do |n|
    if n.is_a?(N::Scalar)
      lv = n.value.downcase
      if lv =~ /_(?:token|pat)(?![a-z0-9_])/ || lv.include?("git-fetch-with-cli")
        add(n, "B2", "token / PAT / git-fetch-with-cli word is not allowed")
      end
    end
    next unless n.is_a?(N::Mapping)
    pairs(n).each do |k, v|
      next unless k.is_a?(N::Scalar)
      case k.value.downcase
      when "uses"
        u = str(v)
        add(k, "B3", "uses must be #{owners.join("/")}/<name>@<40-hex SHA>") unless u && u =~ uses_re
      when "submodules"
        add(k, "A5", "submodules must be false") unless str(v) == "false"
      when "run"
        s = str(v)
        add(k, "A5", "run must not mention submodule") if s && s.downcase.include?("submodule")
      when "env"
        if v.is_a?(N::Mapping)
          pairs(v).each do |ek, _|
            next unless ek.is_a?(N::Scalar)
            e = ek.value.downcase
            add(ek, "A5", "GIT_ env key is not allowed") if e.start_with?("git_")
            add(ek, "B2", "CARGO_ env key is not in the allowlist") if e.start_with?("cargo_") && !CARGO_ENV_ALLOWED.include?(e)
          end
        else
          add(k, "A5", "env must be a mapping")
        end
      end
    end
  end
end

# ---- deploy.yml ----
def check_deploy(root)
  on = mget(root, "on")
  ok =
    case on
    when N::Scalar then on.value == "workflow_dispatch"
    when N::Sequence then on.children.size == 1 && str(on.children[0]) == "workflow_dispatch"
    when N::Mapping then pairs(on).size == 1 && str(pairs(on)[0][0]) == "workflow_dispatch"
    else false
    end
  add(on || root, "C2", "on must be workflow_dispatch only") unless ok

  perm = mget(root, "permissions")
  add(perm || root, "C2", "top-level permissions must be an empty mapping") unless perm.is_a?(N::Mapping) && perm.children.empty?

  jobs = mget(root, "jobs")
  unless jobs.is_a?(N::Mapping)
    add(root, "C2", "jobs must be a mapping")
    return
  end
  pairs(jobs).each do |jk, jv|
    guard = jv.is_a?(N::Mapping) ? str(mget(jv, "if")) : nil
    g = guard.to_s.strip
    add(jk, "C2", "every job needs the fork guard: if: #{GUARD}") unless g == GUARD || g == "${{ #{GUARD} }}"
  end
end

# ---- 教材 repo の workflow（D2〜D4）----
# 正準の 3 step の照合は、キーの綴りの完全一致と復号した値で行う（正準は 1 つ）。
# 後続の step の拒否（D4）だけは、大文字小文字を区別せず広く拾う。
CANON_RUN = 'echo "rev=$(cat ENGINE_REV)" >> "$GITHUB_OUTPUT"'.freeze
PACK_TOP_KEYS = %w[name run-name on permissions concurrency jobs].freeze
PACK_JOB_KEYS = %w[name runs-on permissions timeout-minutes needs if steps strategy concurrency continue-on-error outputs].freeze
CHECKOUT_RE = %r{\Aactions/checkout@[0-9a-f]{40}\z}
ID_RE = /\A[A-Za-z_][A-Za-z0-9_-]*\z/
PATH_RE = /\A[A-Za-z0-9_-]+\z/
GIT_WORD_RE = /(?<![a-z0-9])git(?![a-z0-9])/

# 綴りそのままのキーの一覧。マッピングでない、またはキーにスカラーでないものがあるときは nil
def keyset(m)
  return nil unless m.is_a?(N::Mapping)
  ks = pairs(m).map { |k, _| str(k) }
  ks.include?(nil) ? nil : ks
end

# 綴りの完全一致でキーを引く（mget は小文字化して照合するので、正準の照合には使わない）
def sget(m, key)
  pairs(m).each { |k, v| return v if str(k) == key }
  nil
end

# step のキーが need に name を足した集合ちょうどで、name があればスカラーであること
def step_keys_ok?(s, need)
  ks = keyset(s)
  return false unless ks
  return false unless (ks - ["name"]).sort == need.sort
  nm = sget(s, "name")
  nm.nil? || nm.is_a?(N::Scalar)
end

def uses_checkout?(s)
  str(sget(s, "uses")).to_s.match?(CHECKOUT_RE)
end

def own_checkout?(s)
  return false unless step_keys_ok?(s, %w[uses with]) && uses_checkout?(s)
  w = sget(s, "with")
  ks = keyset(w)
  !ks.nil? && ks.sort == ["persist-credentials"] && str(sget(w, "persist-credentials")) == "false"
end

# 出力の step なら id を、そうでなければ nil を返す
def output_step?(s)
  return nil unless step_keys_ok?(s, %w[id run])
  id = str(sget(s, "id"))
  run = str(sget(s, "run"))
  return nil unless id && id.match?(ID_RE) && run && run.strip == CANON_RUN
  id
end

def engine_checkout?(s, id, engine)
  return false unless id && step_keys_ok?(s, %w[uses with]) && uses_checkout?(s)
  w = sget(s, "with")
  ks = keyset(w)
  return false unless ks && ks.sort == %w[repository ref path persist-credentials].sort
  str(sget(w, "repository")) == engine &&
    str(sget(w, "ref")) == "${{ steps.#{id}.outputs.rev }}" &&
    str(sget(w, "path")).to_s.match?(PATH_RE) &&
    str(sget(w, "persist-credentials")) == "false"
end

# エンジンを取得しうるジョブか。エンジンの repo か ENGINE_REV に触れるか、repository を式や非スカラーで決める
def engine_job?(jv, engine)
  hit = false
  each_node(jv) do |n|
    if n.is_a?(N::Scalar)
      t = n.value.downcase
      hit = true if t.include?(engine.downcase) || t.include?("engine_rev")
    elsif n.is_a?(N::Mapping)
      pairs(n).each do |k, v|
        next unless k.is_a?(N::Scalar) && k.value.downcase == "repository"
        hit = true if !v.is_a?(N::Scalar) || v.value.include?("${{")
      end
    end
  end
  hit
end

# 4 番目以降の step が触れてはならないものに触れているか。ep は正準の 3 番目の path（無ければ nil）
def later_step_bad?(s, engine, ep)
  return true if str(mget(s, "uses")).to_s.downcase.start_with?("actions/checkout")
  skip = mget(s, "name")
  hit = false
  each_node(s) do |n|
    if n.is_a?(N::Scalar)
      next if n.equal?(skip)
      t = n.value.downcase
      hit = true if t.match?(GIT_WORD_RE) || t.include?("actions/checkout") || t.include?(engine.downcase) ||
                    t.include?("engine_rev") || (ep && t.include?(ep.downcase))
    elsif n.is_a?(N::Mapping)
      pairs(n).each do |k, v|
        next unless k.is_a?(N::Scalar) && %w[repository ref path].include?(k.value.downcase)
        hit = true if !v.is_a?(N::Scalar) || v.value.include?("${{")
      end
    end
  end
  hit
end

def check_pack(root, engine)
  jobs = mget(root, "jobs")
  unless jobs.is_a?(N::Mapping)
    add(root, "D2", "jobs must be a mapping")
    return
  end
  ok_jobs = 0
  engine_jobs = 0
  pairs(jobs).each do |jk, jv|
    next unless engine_job?(jv, engine)
    engine_jobs += 1
    unless jv.is_a?(N::Mapping)
      add(jk, "D3", "an engine job must be a mapping")
      next
    end
    bad = false
    pairs(jv).each do |k, _|
      kn = str(k)
      next if kn && PACK_JOB_KEYS.include?(kn)
      add(k, "D3", "a key of an engine job is not in the allowlist (#{PACK_JOB_KEYS.join(", ")})")
      bad = true
    end
    steps = sget(jv, "steps")
    unless steps.is_a?(N::Sequence)
      add(steps || jk, "D3", "an engine job must have steps as a sequence")
      next
    end
    st = steps.children
    s1, s2, s3 = st[0], st[1], st[2]
    id = s2 && output_step?(s2)
    unless s1 && own_checkout?(s1)
      add(s1 || steps, "D3", "step 1 of an engine job must be the canonical checkout of this repository (uses: actions/checkout@<40-hex>, with: persist-credentials: false only)")
      bad = true
    end
    unless id
      add(s2 || steps, "D3", "step 2 of an engine job must be exactly: id: <id> and run: echo \"rev=$(cat ENGINE_REV)\" >> \"$GITHUB_OUTPUT\"")
      bad = true
    end
    ep = nil
    if s3 && engine_checkout?(s3, id, engine)
      ep = str(sget(sget(s3, "with"), "path"))
    else
      add(s3 || steps, "D3", "step 3 of an engine job must be the canonical engine checkout (repository: #{engine}, ref: ${{ steps.<id of step 2>.outputs.rev }}, path: <dir>, persist-credentials: false)")
      bad = true
    end
    (st[3..-1] || []).each_with_index do |s, i|
      next unless later_step_bad?(s, engine, ep)
      add(s, "D4", "step #{i + 4} of an engine job uses actions/checkout or refers to git, the engine repo, ENGINE_REV or the engine path; only the three canonical steps may")
      bad = true
    end
    ok_jobs += 1 unless bad
  end
  if engine_jobs > 0
    pairs(root).each do |k, _|
      kn = str(k)
      next if kn && PACK_TOP_KEYS.include?(kn)
      add(k, "D3", "a top-level key is not in the allowlist (#{PACK_TOP_KEYS.join(", ")}) while an engine job exists")
    end
  end
  add(jobs, "D2", "no job fetches the engine with the canonical three steps") if ok_jobs.zero?
end

case mode
when "ci" then check_ci(root, owners)
when "pack" then check_pack(root, engine)
when "deploy" then check_deploy(root)
else
  warn "error: unknown mode"
  exit 2
end
finish
RUBY

run_yaml() {
  local mode="$1" file="$2" out line rc=0
  need_file "${file}"
  out="$(ruby -e "${RUBY_PROG}" -- "${mode}" "${file}" "${ALLOWED_USES_OWNERS}" "${UPSTREAM_REPO}" "${ENGINE_REPO}")" || rc=$?
  case "${rc}" in
    0) return 0 ;;
    1)
      while IFS= read -r line; do
        report "${file}:${line}"
      done <<<"${out}"
      ;;
    *)
      echo "error: YAML check of ${file} could not complete (exit ${rc})" >&2
      exit 2
      ;;
  esac
}

# ---- A1 Cargo ----
# cargo metadata は [source.crates-io] の置き換え後でも元の source を返すため、置き換えは A4 が別に拒否する。
# source が null なのは workspace の member（と workspace の外の path 依存）。member だけを許す。
check_cargo() {
  local meta out rc=0 dir
  if [ -n "${CARGO_META_JSON}" ]; then
    need_file "${CARGO_META_JSON}"
    meta="$(cat "${CARGO_META_JSON}")"
  else
    need_file "${CARGO_MANIFEST}"
    dir="$(dirname "${CARGO_MANIFEST}")"
    # cargo の標準エラーは URL に認証情報を含み得るため出さない。rust-toolchain.toml を効かせるため manifest の場所で流す
    meta="$(cd "${dir}" && cargo metadata --locked --format-version 1 --manifest-path "$(basename "${CARGO_MANIFEST}")" 2>/dev/null)" || rc=$?
    if [ "${rc}" -ne 0 ]; then
      echo "error: cargo metadata failed (exit ${rc}); is Cargo.lock up to date with --locked?" >&2
      exit 2
    fi
  fi
  rc=0
  # shellcheck disable=SC2016 # jq のプログラムなので、シェルに展開させない
  out="$(printf '%s' "${meta}" | jq -r --arg reg "${CRATES_REGISTRY}" '
    (.workspace_members // error("no workspace_members")) as $ws
    | (.packages // error("no packages"))[]
    | . as $p
    | select((($p.source == $reg) or ($p.source == null and ($ws | any(. == $p.id)))) | not)
    | "\($p.name)@\($p.version)"
  ')" || rc=$?
  if [ "${rc}" -ne 0 ]; then
    echo "error: cannot evaluate the cargo metadata JSON (jq exit ${rc})" >&2
    exit 2
  fi
  local pkg
  while IFS= read -r pkg; do
    [ -n "${pkg}" ] || continue
    report "$(basename "${CARGO_MANIFEST:-cargo-metadata}"):A1 $(safe_id "${pkg}") is not from crates.io and not a workspace member"
  done <<<"${out}"
}

# ---- A2 package-lock.json ----
check_package_lock() {
  local out rc=0 ent
  need_file "${PKG_LOCK}"
  # shellcheck disable=SC2016 # jq のプログラムなので、シェルに展開させない
  out="$(jq -r --arg pre "${NPM_REGISTRY_PREFIX}" '
    (if .lockfileVersion != 3 then "-\tlockfileVersion must be 3" else empty end),
    ((.packages // error("no packages")) | to_entries[] | select(.key != "")
      | select(
          (.value | has("link"))
          or ((.value.resolved | type) != "string")
          or ((.value.resolved | type) == "string" and (.value.resolved | startswith($pre) | not))
          or ((.value.integrity | type) != "string")
        )
      | "\(.key)\tmust have an npm registry resolved URL and an integrity, and must not be a link")
  ' "${PKG_LOCK}")" || rc=$?
  if [ "${rc}" -ne 0 ]; then
    echo "error: cannot evaluate ${PKG_LOCK} (jq exit ${rc})" >&2
    exit 2
  fi
  while IFS= read -r ent; do
    [ -n "${ent}" ] || continue
    report "${PKG_LOCK}:A2 $(safe_id "${ent%%$'\t'*}"): ${ent#*$'\t'}"
  done <<<"${out}"
}

# ---- A3 .npmrc ----
check_npmrc() {
  local line n=0
  need_file "${NPMRC_FILE}"
  while IFS= read -r line || [ -n "${line}" ]; do
    n=$((n + 1))
    case "${line}" in
      "" | "#"* | ";"* | "engine-strict=true") ;;
      *) report "${NPMRC_FILE}:${n}:A3 only blank lines, comments and engine-strict=true are allowed" ;;
    esac
  done <"${NPMRC_FILE}"
}

# ---- A4 .cargo/config ----
check_cargo_config() {
  local f
  for f in "${CARGO_CFG_DIR}/config" "${CARGO_CFG_DIR}/config.toml"; do
    if [ -e "${f}" ] || [ -L "${f}" ]; then
      report "${f}:A4 cargo config must not exist (a [source] replacement is invisible to cargo metadata)"
    fi
  done
}

# ---- A5 .gitmodules ----
check_gitmodules() {
  local f="${GITMODULES_FILE}" listing ent key rest name names="" upd rc cnt
  need_file "${f}"
  rc=0
  # NUL 区切りの出力を、command substitution を通せる文字（RS = 0x1e）に置き換えて読む
  listing="$(git config --file "${f}" -z --list | tr '\0' '\036')" || rc=$?
  if [ "${rc}" -ne 0 ]; then
    echo "error: cannot parse ${f} with git config (exit ${rc})" >&2
    exit 2
  fi
  while IFS= read -r -d $'\036' ent; do
    key="${ent%%$'\n'*}"
    case "${key}" in
      submodule.*.*) ;;
      *)
        report "${f}:A5 only submodule.<name>.<key> entries are allowed"
        continue
        ;;
    esac
    rest="${key#submodule.}"
    name="${rest%.*}"
    if ! printf '%s\n' "${names}" | grep -Fxq -- "${name}"; then
      names="${names}${name}"$'\n'
    fi
  done <<<"${listing}"
  while IFS= read -r name; do
    [ -n "${name}" ] || continue
    rc=0
    upd="$(git config --file "${f}" --get-all -- "submodule.${name}.update")" || rc=$?
    if [ "${rc}" -ne 0 ] && [ "${rc}" -ne 1 ]; then
      echo "error: cannot parse ${f} with git config (exit ${rc})" >&2
      exit 2
    fi
    cnt=0
    [ -z "${upd}" ] || cnt="$(printf '%s\n' "${upd}" | wc -l | tr -d ' ')"
    if [ "${cnt}" -ne 1 ] || [ "${upd}" != "none" ]; then
      report "${f}:A5 submodule $(safe_id "${name}") must have exactly one update = none"
    fi
  done <<<"${names}"
}

# ---- D1 ENGINE_REV ----
# 中身は出さない。ruby の判定が 0 = 合格 / 1 = 不合格。それ以外（ruby の異常終了）は検査を完了できない
check_engine_rev() {
  local rc=0
  need_file "${ENGINE_REV_FILE}"
  ruby -e 'exit(File.binread(ARGV[0]).match?(/\A[0-9a-f]{40}\n?\z/) ? 0 : 1)' -- "${ENGINE_REV_FILE}" || rc=$?
  case "${rc}" in
    0) ;;
    1) report "${ENGINE_REV_FILE}:1:D1 ENGINE_REV must be a single line of 40 lowercase hex digits" ;;
    *)
      echo "error: D1 check of ${ENGINE_REV_FILE} could not complete (exit ${rc})" >&2
      exit 2
      ;;
  esac
}

# ---- B1 ----
check_secrets() {
  local f="$1" rc=0
  need_file "${f}"
  # 出力には式の本文が含まれ得るため、そのまま出さない
  "${SECRETS_CHECKER}" --file "${f}" >/dev/null 2>&1 || rc=$?
  case "${rc}" in
    0) ;;
    1) report "${f}:B1 check-workflow-secrets reported secret references (run scripts/check-workflow-secrets.sh --file to see them)" ;;
    *)
      echo "error: check-workflow-secrets could not complete (exit ${rc})" >&2
      exit 2
      ;;
  esac
}

[ -z "${CARGO_MANIFEST}${CARGO_META_JSON}" ] || check_cargo
[ -z "${PKG_LOCK}" ] || check_package_lock
[ -z "${NPMRC_FILE}" ] || check_npmrc
[ -z "${CARGO_CFG_DIR}" ] || check_cargo_config
[ -z "${GITMODULES_FILE}" ] || check_gitmodules
if [ -n "${CI_FILE}" ]; then
  check_secrets "${CI_FILE}"
  run_yaml ci "${CI_FILE}"
fi
[ -z "${DEPLOY_FILE}" ] || run_yaml deploy "${DEPLOY_FILE}"
if [ -n "${PACK_WORKFLOW}" ]; then
  check_engine_rev
  run_yaml pack "${PACK_WORKFLOW}"
fi

if [ "${VIOLATIONS}" -ne 0 ]; then
  echo "error: ${VIOLATIONS} violation(s) in the fork CI check" >&2
  exit 1
fi
echo "ok: fork CI check passed" >&2
