#!/usr/bin/env bash
# ビルド・試験のワークフロー（既定は .github/workflows/ci.yml）が secret を参照していないことを検査する
# （`make check-workflow-secrets` から呼ばれる）。
#
# 理由: ci.yml は pull_request で動くため、secret を参照すると PR の起点から secret に届く経路になる。
# secret を扱ってよいのは、PR の起点を持たない配信側（deploy.yml）だけにする（TASK-33）。
#
# 検出対象は、Actions 式 `${{ ... }}` の中（複数行にまたがる式を含む）にある次の参照と、`secrets: inherit`
# （キー・値を引用符で囲んだ形も含む。`if` キーの引用符付きも同様）。
#   - `secrets` コンテキスト（`secrets.X`・`secrets['X']`・`secrets . X`・`toJSON(secrets)`。大文字小文字・空白を問わない）
#   - `github.token`・`github` への添字アクセス `github[...]`（中身を問わない。`github[env.KEY]` のように添字が動的な式で
#     `token` を指せるため、`github['event_name']` も含めて拒否する）（空白・大文字小文字を問わない）と、
#     `toJSON(github)`・`github.*` のような github 全体の参照
#   - `if:` の値（`${{ }}` を省略した形と、継続行を含む）。値全体を式として同じ規則で検査する
# 式の中の文字列リテラル（`'...'`。引用符は `''` でエスケープ）にある `}}` は終端として扱わない。
# 上の行ベースの検査に加え、ruby（Psych）で YAML を構文木に解析し、記法に依存せず次を検査する。
#   - `secrets` キーのスカラー値（値を別行に置いた形・`>-` などのブロックスカラー・フロー形式を含む）
#   - 全スカラーの復号後の値にある Actions 式（二重引用符内の `\x73ecrets` 等のエスケープ表記を復号して検査する）
#   - `if` キーの値（フロー形式 `{if: ...}` を含む）。行頭の `if:` の形に頼らず、キーと値の対として検査する
# 解析できない YAML・エイリアス（`*name`。参照先を追わないため）・`if` のスカラーでない値は、検査を完了できない
# ものとして終了コード 2 にする（fail-closed）。
# コメント行も除外しない。YAML のコメントと、run のブロックスカラー（文字列。中の式は評価される）を
# 行頭の `#` だけでは区別できないため、fail-closed で全行を対象にする。式の外の文章（`secrets を参照しない` 等）は対象外。
#
# 使い方: scripts/check-workflow-secrets.sh [--file <path>]
#   --file は回帰テスト（scripts/test-check-workflow-secrets.sh）が一時ファイルを検査するために使う。
# 終了コード: 0 = 参照 0 件 / 1 = 参照あり / 2 = 誤用、または検査を完了できない（ファイルが無い・読めない・awk の失敗）。
#   2 でも合格にしない（fail-closed）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

TARGET="${ROOT_DIR}/.github/workflows/ci.yml"
if [ "$#" -eq 2 ] && [ "$1" = "--file" ]; then
  TARGET="$2"
elif [ "$#" -ne 0 ]; then
  echo "usage: $(basename "$0") [--file <path>]" >&2
  exit 2
fi

require_cmd awk || exit 2
require_cmd ruby || exit 2

if [ ! -r "${TARGET}" ] || [ ! -f "${TARGET}" ]; then
  echo "error: cannot read ${TARGET}" >&2
  exit 2
fi

# awk で式を取り出して判定する。式の開始行番号と式の本文（複数行は空白で連結）を `行:本文` で出す。
# 引用符は八進エスケープ（\047 = ' / \042 = "）で書き、シェルのクォートと衝突させない。
# shellcheck disable=SC2016 # awk のプログラムなので、シェルに展開させない
AWK_PROG='
function check(body, start,    lb) {
  lb = tolower(body)
  if (lb ~ /(^|[^a-z0-9_-])secrets([^a-z0-9_-]|$)/ ||
      lb ~ /(^|[^a-z0-9_-])github[ \t]*(\.[ \t]*token|\[)/ ||
      lb ~ /(^|[^a-z0-9_-])github[ \t]*(\.[ \t]*\*|[),]|$)/) {
    print start ":" body
  }
}
function flush_if() {
  if (ifctx) check(ifbuf, ifstart)
  ifctx = 0
}
{
  line = $0
  if (tolower(line) ~ /secrets[\047\042]?[ \t]*:[ \t]*[\047\042]?inherit/) {
    print NR ":" line
  }
  # `if:` の値は `${{ }}` を省略しても式として評価されるため、区切りの有無を問わず値全体を式として検査する。
  # 値は継続行（key より深いインデント）まで含める。
  # 式が開いたまま始まる行も値の一部なので、インデントでは終端せず必ず取り込む。
  if (ifctx) {
    if (!inexpr && match(line, /[^ \t]/) && RSTART - 1 <= ifind) flush_if()
    else ifbuf = ifbuf line " "
  }
  if (!ifctx && !inexpr && match(line, /^[ \t]*(-[ \t]+)?[\047\042]?if[\047\042]?[ \t]*:/)) {
    ifind = index(line, "if") - 1
    if (substr(line, ifind, 1) ~ /[\047\042]/) ifind--
    ifbuf = substr(line, RLENGTH + 1) " "
    ifstart = NR
    ifctx = 1
  }
  # 式の中の文字列リテラル（単一引用符。中の引用符は連続 2 個でエスケープ）にある `}}` は終端として扱わない。
  n = length(line)
  i = 1
  while (i <= n) {
    c = substr(line, i, 1)
    if (!inexpr) {
      if (substr(line, i, 3) == "${{") { inexpr = 1; instr = 0; buf = ""; start = NR; i += 3 }
      else i++
    } else if (instr) {
      buf = buf c
      if (c == "\047") {
        if (substr(line, i + 1, 1) == "\047") { buf = buf "\047"; i += 2 }
        else { instr = 0; i++ }
      } else i++
    } else if (c == "\047") {
      instr = 1; buf = buf c; i++
    } else if (substr(line, i, 2) == "}}") {
      inexpr = 0; check(buf, start); i += 2
    } else {
      buf = buf c; i++
    }
  }
  if (inexpr) buf = buf " "
}
END {
  if (inexpr) check(buf, start)
  flush_if()
}
'

# YAML を構文木にして、キーと値の対を検査する。出力は awk と同じ `行:本文`。
# 引用符は \x27 / \x22 で書き、シェルのクォートと衝突させない。
# shellcheck disable=SC2016 # ruby のプログラムなので、シェルに展開させない
RUBY_PROG='
require "psych"
file = ARGV[0]
begin
  stream = Psych.parse_stream(File.read(file, mode: "rb").force_encoding("UTF-8"))
rescue StandardError, Psych::SyntaxError => e
  warn "error: cannot parse #{file} as YAML: #{e.message.lines.first}"
  exit 2
end
# 復号後の値は改行・CR・タブを含み得る（二重引用符内の `\n` 等）。空白は `\s` で見る。`^`・`$` は行頭・行末でも一致し、
# 検出が増えるだけで見逃しは増えない。ただし github 全体参照の末尾は、後続の `.event_name` を誤検出しないよう `\z` にする。
risky = lambda do |v|
  lb = v.downcase
  lb =~ /(^|[^a-z0-9_-])secrets([^a-z0-9_-]|$)/ ||
    lb =~ /(^|[^a-z0-9_-])github\s*(\.\s*token|\[)/ ||
    lb =~ /(^|[^a-z0-9_-])github\s*(\.\s*\*|[),]|\z)/
end
# 復号後のスカラー値から Actions 式 `${{ ... }}` の本文を取り出す。awk と同じく、式の中の文字列リテラル
# （単一引用符。中の引用符は連続 2 個でエスケープ）にある `}}` は終端として扱わない。閉じない式は残りを本文にする。
expr_bodies = lambda do |s|
  bodies = []
  i = 0
  n = s.length
  while i < n
    if s[i, 3] == "${{"
      i += 3
      buf = +""
      instr = false
      closed = false
      while i < n
        c = s[i]
        if instr
          buf << c
          if c == "\x27"
            if s[i + 1] == "\x27"
              buf << "\x27"
              i += 2
            else
              instr = false
              i += 1
            end
          else
            i += 1
          end
        elsif c == "\x27"
          instr = true
          buf << c
          i += 1
        elsif s[i, 2] == "}}"
          closed = true
          i += 2
          break
        else
          buf << c
          i += 1
        end
      end
      bodies << buf
    else
      i += 1
    end
  end
  bodies
end
hits = []
unsupported = []
walk = lambda do |node|
  # 引用符内のエスケープ（\x73ecrets 等）は Psych が復号した値で見る。行ベースの検査が見逃す記法の受け皿になる。
  if node.is_a?(Psych::Nodes::Scalar)
    expr_bodies.call(node.value).each do |body|
      hits << "#{node.start_line + 1}:expression: #{body.strip}" if risky.call(body)
    end
  end
  case node
  when Psych::Nodes::Alias
    unsupported << (node.start_line + 1)
  when Psych::Nodes::Mapping
    node.children.each_slice(2) do |k, v|
      if k.is_a?(Psych::Nodes::Scalar)
        key = k.value.strip.downcase
        if key == "secrets" && v.is_a?(Psych::Nodes::Scalar)
          hits << "#{k.start_line + 1}:secrets: #{v.value}"
        elsif key == "if"
          if !v.is_a?(Psych::Nodes::Scalar)
            unsupported << (k.start_line + 1)
          elsif risky.call(v.value)
            hits << "#{k.start_line + 1}:if: #{v.value.strip}"
          end
        end
      end
      walk.call(k)
      walk.call(v)
    end
  else
    (node.children || []).each { |c| walk.call(c) }
  end
end
walk.call(stream)
unless unsupported.empty?
  warn "error: #{file}: unsupported YAML notation (alias or non-scalar if) at line #{unsupported.sort.join(",")}"
  exit 2
end
puts hits
'

rc=0
hits="$(awk "${AWK_PROG}" "${TARGET}")" || rc=$?
if [ "${rc}" -ne 0 ]; then
  echo "error: awk failed on ${TARGET}" >&2
  exit 2
fi

rc=0
yaml_hits="$(ruby -e "${RUBY_PROG}" -- "${TARGET}")" || rc=$?
if [ "${rc}" -ne 0 ] && [ -z "${hits}" ]; then
  # 行ベースの検査で参照が見つかっていれば、そちらの失敗（1）を優先して報告する。
  echo "error: YAML structural check failed on ${TARGET}" >&2
  exit 2
fi
if [ -n "${yaml_hits}" ]; then
  hits="${hits:+${hits}
}${yaml_hits}"
fi

if [ -n "${hits}" ]; then
  while IFS= read -r line; do
    echo "error: ${TARGET}:${line}" >&2
  done <<EOT
${hits}
EOT
  echo "workflow secret check failed: the build/test workflow must not reference secrets" >&2
  exit 1
fi
echo "workflow secret check passed (0 references in ${TARGET})"
