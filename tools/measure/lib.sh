#!/usr/bin/env bash
# 測定の結果（JSON）を組み立て・検証・出力し、測定前のビルドとログ解析を行う共有ヘルパー。直接実行せず source して使う（実行権限は付けない）:
#   source "${DIR}/../../scripts/lib.sh"   # ROOT_DIR・log・require_cmd を先に読み込む
#   source "${DIR}/lib.sh"
#
# 存在理由: 基点（commit・wasm-opt・機能の範囲・圧縮の設定）の付かない値は、基点の違う値どうしの誤った比較を招く
# （docs/measurement.md）。結果を出す経路を measure_emit / measure_emit_run だけにして、基点が欠けた結果を構造的に出さない。
# 呼び出し元: tools/measure/wasm-size.sh・first-paint.sh・first-paint-direct.sh・redraw.sh・all.sh（log-redraw.sh・input-paint.sh・view-redraw.sh も measure_browser_result を使う）。ビルドとログ解析・commit の前後比較も
# 3 つの測定で同じ手順にするため、ここへ集約する。
# 結果の様式の定義は docs/measurement.md。JSON は jq の --arg / --argjson だけで組み立て、文字列連結で作らない。

# measure_commit_json: 基点の commit を {"sha": ..., "dirty": ...} で標準出力へ返す。失敗は非 0。
measure_commit_json() {
  local sha status
  sha="$(git -C "${ROOT_DIR}" rev-parse HEAD)" || return 1
  case "${sha}" in
    *[!0-9a-f]* | "") return 1 ;;
  esac
  [ "${#sha}" -eq 40 ] || return 1
  status="$(git -C "${ROOT_DIR}" status --porcelain)" || return 1
  if [ -n "${status}" ]; then
    jq -n --arg sha "${sha}" '{sha: $sha, dirty: true}'
  else
    jq -n --arg sha "${sha}" '{sha: $sha, dirty: false}'
  fi
}

# _measure_sha256 <file>: ファイルの SHA-256（16 進 64 桁）を返す。sha256sum（GNU）か shasum（macOS）のどちらか有る方を使い、
# どちらも無い・64 桁でなければ失敗する。
_measure_sha256() {
  local out
  if command -v sha256sum >/dev/null 2>&1; then
    out="$(sha256sum -- "$1")" || return 1
  elif command -v shasum >/dev/null 2>&1; then
    out="$(shasum -a 256 -- "$1")" || return 1
  else
    return 1
  fi
  out="${out%% *}"
  case "${out}" in
    *[!0-9a-f]* | "") return 1 ;;
  esac
  [ "${#out}" -eq 64 ] || return 1
  printf '%s\n' "${out}"
}

# measure_pack_json <ir-file>: 測った教材の基点 {"id","version","nodes","sha256"} を返す。log-redraw・log-slope・log-rounds・input-paint の基点の「教材」。
# id・version は IR の pack 欄、nodes は IR の nodes の数（Step の数）。読めない・欠けていれば失敗する。
measure_pack_json() {
  local file="$1" sha
  [ -r "${file}" ] || return 1
  sha="$(_measure_sha256 "${file}")" || return 1
  jq -e --arg sha "${sha}" '
    def nes: type == "string" and length > 0;
    select((.pack.id | nes) and (.pack.version | nes) and (.nodes | type == "array"))
    | {id: .pack.id, version: .pack.version, nodes: (.nodes | length), sha256: $sha}
  ' "${file}"
}

# measure_device_json: 測った端末の基点を返す。OS・アーキテクチャ・CPU の型番・論理コアの数・メモリの量だけを入れる
# （public の repo に残るため、ホスト名・ユーザー名・パスは入れない）。Linux と macOS で動かし、bash 3.2 と BSD の道具で書く。
# apple_silicon_macos は基準端末 A（Apple シリコンの macOS）の目印。取れない欄があれば失敗する（fail-closed）。
measure_device_json() {
  local os release arch cpu cores mem apple=false
  os="$(uname -s)" || return 1
  release="$(uname -r)" || return 1
  arch="$(uname -m)" || return 1
  case "${os}" in
    Linux)
      cpu="$(sed -n 's/^model name[[:space:]]*:[[:space:]]*//p' /proc/cpuinfo | head -n 1)"
      cores="$(getconf _NPROCESSORS_ONLN)" || return 1
      mem="$(awk '/^MemTotal:/ { print $2 * 1024 }' /proc/meminfo)"
      ;;
    Darwin)
      cpu="$(sysctl -n machdep.cpu.brand_string)" || return 1
      cores="$(sysctl -n hw.ncpu)" || return 1
      mem="$(sysctl -n hw.memsize)" || return 1
      if [ "${arch}" = "arm64" ]; then apple=true; fi
      ;;
    *) return 1 ;;
  esac
  case "${cores}${mem}" in
    *[!0-9]* | "") return 1 ;;
  esac
  [ -n "${cpu}" ] || return 1
  jq -n --arg os "${os}" --arg release "${release}" --arg arch "${arch}" --arg cpu "${cpu}" \
    --argjson cores "${cores}" --argjson mem "${mem}" --argjson apple "${apple}" \
    '{os: $os, os_release: $release, arch: $arch, cpu: $cpu, logical_cores: $cores, memory_bytes: $mem, apple_silicon_macos: $apple}'
}

# measure_device_class_json: 端末の区分だけを返す（first-paint-direct 用。公開に向かない CPU の型番・メモリの量・ホスト名は入れない）。
# class は Apple シリコンの macOS なら A、それ以外は other。A は基準端末 A であることの証明ではなく、記録する人が確かめる（docs/measurement.md）。
measure_device_class_json() {
  local os arch class=other apple=false
  os="$(uname -s)" || return 1
  arch="$(uname -m)" || return 1
  if [ "${os}" = "Darwin" ] && [ "${arch}" = "arm64" ]; then
    class=A
    apple=true
  fi
  jq -n --arg os "${os}" --arg arch "${arch}" --arg class "${class}" --argjson apple "${apple}" \
    '{class: $class, os: $os, arch: $arch, apple_silicon_macos: $apple}'
}

# measure_first_paint_direct_verdict <result.json>: first-paint-direct の結果を検証し、3 秒の判定を返す。
# 戻り値: 0 = 3 秒以内 / 3 = 測れたが超過 / 1 = 形の不正（理由は標準エラー）。中央値・最大・判定は values_us から jq で計算し直して照合する。
measure_first_paint_direct_verdict() {
  local file="$1" within
  [ -r "${file}" ] || return 1
  measure_validate "${file}" || { echo "error: first-paint-direct result is not a valid v1 result" >&2; return 1; }
  if ! jq -e -s '
    def med: sort | if length % 2 == 1 then .[length / 2 | floor] else ((.[length / 2 - 1] + .[length / 2]) / 2) end | round;
    def one(n): [.measurements[] | select(.name == n)] | length == 1;
    def val(n): .measurements[] | select(.name == n) | .value;
    .[0] as $r | $r.conditions as $c |
    length == 1
    and ($c.values_us | type == "array" and length == 10 and all(type == "number" and . > 0 and . == floor))
    and ($c.mounted_values_us | type == "array" and length == 10 and all(type == "number" and . > 0 and . == floor))
    and ($c.threshold_us == 3000000)
    and ($r | one("first_paint_direct_operable_median") and one("first_paint_direct_operable_max")
      and one("first_paint_direct_mounted_median") and one("first_paint_direct_wasm_transfer") and one("first_paint_direct_ir_transfer"))
    and ($r | val("first_paint_direct_operable_median")) == ($c.values_us | med)
    and ($r | val("first_paint_direct_operable_max")) == ($c.values_us | max)
    and ($r | val("first_paint_direct_mounted_median")) == ($c.mounted_values_us | med)
    and ($c.judgement.median_within == (($c.values_us | med) <= 3000000))
    and ($c.judgement.max_within == (($c.values_us | max) <= 3000000))
    and ($c.judgement.within == ($c.judgement.median_within and $c.judgement.max_within))
    and ($r.toolchain.browser | type == "string" and length > 0)
    and ($c.browser.channel == "chromium" or $c.browser.channel == "chrome")
    and ($c.browser.headless | type == "boolean")
    and ($r.baseline.device.class == "A" or $r.baseline.device.class == "other")
    and ($r.baseline.device | has("cpu") or has("memory_bytes") | not)
    and ($r.baseline.pack.sha256 | type == "string" and test("^[0-9a-f]{64}$"))
  ' "${file}" >/dev/null 2>&1; then
    echo "error: first-paint-direct result is malformed or inconsistent" >&2
    return 1
  fi
  within="$(jq -r '.conditions.judgement.within' "${file}")" || return 1
  if [ "${within}" = "true" ]; then return 0; fi
  return 3
}

# measure_data_json: 固定データの測定（view-redraw）の基点 {"id","manifest_sha256","files"} を返す。教材の代わりに、測った固定データの
# 出所（testdata/artifact-view-parts/test-data.sha256 の SHA-256）と、埋め込んだ規模のデータの写しのファイルごとの SHA-256 を残す。
# 1 つでも読めない・SHA-256 が取れなければ失敗する（fail-closed）。JSON は --arg だけで組み立てる。
measure_data_json() {
  local manifest dir name sha out
  manifest="$(_measure_sha256 "${ROOT_DIR}/testdata/artifact-view-parts/test-data.sha256")" || return 1
  dir="${ROOT_DIR}/crates/web/tests/data/artifact_view_scale"
  out='{}'
  for name in diff-500.json tasks-50.json filetree-200.json doc-large-before.json doc-large-after.json doc-large-before.md doc-large-after.md; do
    sha="$(_measure_sha256 "${dir}/${name}")" || return 1
    out="$(jq -c --arg n "${name}" --arg s "${sha}" '. + {($n): $s}' <<<"${out}")" || return 1
  done
  jq -n --arg id "artifact-view-parts" --arg m "${manifest}" --argjson files "${out}" \
    '{id: $id, manifest_sha256: $m, files: $files}'
}

# jq の述語。様式 v1 の 1 件の検査（valid_v1）を、単体（measure_validate）と束（measure_validate_run）で共有する。
MEASURE_JQ_VALID_V1='
  def nes: type == "string" and length > 0;
  def valid_v1:
    .schema == "fandhe-workshop/measure/v1"
    and (.measured_at | nes)
    and (.baseline.commit.sha | type == "string" and test("^[0-9a-f]{40}$"))
    and (.baseline.commit.dirty | type == "boolean")
    and (.baseline.wasm_opt.applied | type == "boolean")
    and (.baseline.wasm_opt.detail | nes)
    and (.baseline.features.config | . == "delivery" or . == "test")
    and (.baseline.features.cargo_features | type == "array")
    and (.baseline.features.profile | nes)
    and (.baseline.compression.command | nes)
    and (.baseline.compression.implementation | nes)
    and (.measurements | type == "array" and length >= 1)
    and (.measurements | all(
      (.name | nes) and (.file | nes) and (.unit | nes)
      and (.value | type == "number" and . > 0 and . == floor)));
'

# measure_validate <json-file>: 様式 v1 を満たすか検査する。1 つでも外れたら非 0（理由は出さず、emit が一括で報告する）。
measure_validate() {
  local file="$1"
  [ -r "${file}" ] || return 1
  # -s で入力全体を配列に受け、ちょうど 1 件であることを確かめてから検証する。jq -e は最後の出力値だけで
  # 終了状態を決めるため、複数の JSON 値（基点の欠けた値の後ろに正常な値を連結した入力）が通るのを防ぐ。
  jq -e -s "${MEASURE_JQ_VALID_V1}"'
    length == 1 and (.[0] | valid_v1)
  ' "${file}" >/dev/null 2>&1
}

# measure_validate_run <json-file>: 一括実行の束（fandhe-workshop/measure-run/v1）を検査する。
# results はちょうど 3 件で、各要素が v1 を満たし、測定の種別（wasm_gzip・first_paint_mounted_median・redraw_median）が
# 1 件ずつあり、3 件の baseline.commit（sha と dirty）が一致すること。同じ commit で測ったことの保証になる。
# 複数の JSON 値の入力は単体と同じく拒否する。
measure_validate_run() {
  local file="$1"
  [ -r "${file}" ] || return 1
  jq -e -s "${MEASURE_JQ_VALID_V1}"'
    def kinds: [.measurements[].name | select(. == "wasm_gzip" or . == "first_paint_mounted_median" or . == "redraw_median")];
    length == 1 and (.[0] |
      .schema == "fandhe-workshop/measure-run/v1"
      and (.results | type == "array" and length == 3)
      and (.results | all(valid_v1))
      and ([.results[] | kinds[]] | sort == ["first_paint_mounted_median", "redraw_median", "wasm_gzip"])
      and ([.results[].baseline.commit] | unique | length == 1))
  ' "${file}" >/dev/null 2>&1
}

# measure_emit <json-file>: 検証に通ったときだけ標準出力へ出す。通らなければ何も出さず 1 を返す。
measure_emit() {
  local file="$1"
  if ! measure_validate "${file}"; then
    echo "error: result is missing baseline fields" >&2
    return 1
  fi
  jq . "${file}"
}

# measure_emit_run <json-file>: 束の検証に通ったときだけ標準出力へ出す。通らなければ何も出さず 1 を返す。
measure_emit_run() {
  local file="$1"
  if ! measure_validate_run "${file}"; then
    echo "error: run result is malformed or missing baseline fields" >&2
    return 1
  fi
  jq . "${file}"
}

# _measure_log_line <log> <prefix>: ログからその接頭辞の行をちょうど 1 行取り出し、接頭辞を除いた残りを返す。
_measure_log_line() {
  local logfile="$1" prefix="$2" n
  n="$(grep -c -F -- "[build-web.sh] ${prefix}" "${logfile}" || true)"
  if [ "${n}" != "1" ]; then
    echo "error: expected exactly one '${prefix}' line in the build log (got ${n})" >&2
    return 1
  fi
  grep -F -- "[build-web.sh] ${prefix}" "${logfile}" | sed -e "s/^\[build-web.sh\] ${prefix} *//"
}

# measure_build <tmpdir> [ir-file]: tools/build-web.sh を release で <tmpdir>/web へビルドし（dist/ は上書きしない）、ログを解析する。
# ビルドログは標準エラーへ流す。TEST_HOOKS・SKIP_WASM_OPT は呼び出し元の環境を build-web.sh へそのまま渡す。
# IR_FILE は第 2 引数（IR を開く測定 ir-memory 用。ir/main.json として同梱される）で、省略すると空にする。
# 第 2 引数の IR を渡すと配信する設定に同梱し（first-paint-direct。IR_FILE は <tmpdir>/web の外に置くこと）、ログの `ir:` 行が
# ちょうど 1 行で config が delivery であることを確かめる。IR を渡さない従来の呼び出しは動きが変わらない。
# 成功時に MEASURE_CONFIG（delivery / test）・MEASURE_WASM_OPT_APPLIED（true / false）・MEASURE_WASM_OPT_DETAIL・
# MEASURE_LOG_RAW・MEASURE_LOG_GZ を設定する（呼び出し元が参照する）。
# 戻り値: 0 = 成功 / 1 = 失敗 / 2 = build-web.sh の誤用（そのまま返す）。各ログ行が厳密に 1 行でなければ失敗する（fail-closed）。
measure_build() {
  local tmp="$1" ir_file="${2:-}" rc=0 config_line wasm_opt_line size_line ir_line
  log "building (release) via tools/build-web.sh"
  OUT_DIR="${tmp}/web" IR_FILE="${ir_file}" "${ROOT_DIR}/tools/build-web.sh" release >"${tmp}/build.log" 2>&1 || rc=$?
  if [ "${rc}" -ne 0 ]; then
    cat "${tmp}/build.log" >&2
    echo "error: build-web.sh failed" >&2
    if [ "${rc}" -eq 2 ]; then return 2; fi
    return 1
  fi
  cat "${tmp}/build.log" >&2

  config_line="$(_measure_log_line "${tmp}/build.log" "config:")" || return 1
  wasm_opt_line="$(_measure_log_line "${tmp}/build.log" "wasm-opt:")" || return 1
  size_line="$(_measure_log_line "${tmp}/build.log" "wasm size:")" || return 1

  # 選択（\|）は BSD sed（macOS 標準）の BRE で動かないため、case で完全一致を判定する。
  case "${config_line}" in
    "delivery / profile: release") MEASURE_CONFIG="delivery" ;;
    "test / profile: release") MEASURE_CONFIG="test" ;;
    *)
      echo "error: unexpected config line in the build log" >&2
      return 1
      ;;
  esac
  case "${wasm_opt_line}" in
    "not applied ("*")") MEASURE_WASM_OPT_APPLIED=false; MEASURE_WASM_OPT_DETAIL="${wasm_opt_line#not applied (}" ;;
    "applied ("*")") MEASURE_WASM_OPT_APPLIED=true; MEASURE_WASM_OPT_DETAIL="${wasm_opt_line#applied (}" ;;
    *)
      echo "error: unexpected wasm-opt line in the build log" >&2
      return 1
      ;;
  esac
  MEASURE_WASM_OPT_DETAIL="${MEASURE_WASM_OPT_DETAIL%)}"

  if [ -n "${ir_file}" ]; then
    ir_line="$(_measure_log_line "${tmp}/build.log" "ir:")" || return 1
    if [ "${ir_line}" != "ir/main.json (from IR_FILE)" ] || [ "${MEASURE_CONFIG}" != "delivery" ]; then
      echo "error: the IR was not bundled into a delivery build" >&2
      return 1
    fi
  fi

  MEASURE_LOG_RAW="$(sed -n 's/^raw \([0-9][0-9]*\) bytes, gzip -9 \([0-9][0-9]*\) bytes$/\1/p' <<<"${size_line}")"
  MEASURE_LOG_GZ="$(sed -n 's/^raw \([0-9][0-9]*\) bytes, gzip -9 \([0-9][0-9]*\) bytes$/\2/p' <<<"${size_line}")"
  if [ -z "${MEASURE_LOG_RAW}" ] || [ -z "${MEASURE_LOG_GZ}" ]; then
    echo "error: unexpected wasm size line in the build log" >&2
    return 1
  fi
}

# measure_features_json <config>: 設定名（delivery / test）に対応する cargo_features の JSON 配列を返す。
measure_features_json() {
  case "$1" in
    test) echo '["test-hooks"]' ;;
    delivery) echo '[]' ;;
    *) return 1 ;;
  esac
}

# measure_assert_commit_unchanged <commit-json-before>: 測定中に commit と作業ツリーの状態（dirty）が動いていないことを確かめる。
# 開始後の編集で、変更済みのソースに元の SHA と dirty:false が付くのを防ぐ。動いていれば理由を出して非 0。
measure_assert_commit_unchanged() {
  local before="$1" after
  after="$(measure_commit_json)" || after=""
  if [ -z "${after}" ] || [ "$(jq -r '.sha' <<<"${after}")" != "$(jq -r '.sha' <<<"${before}")" ]; then
    echo "error: HEAD moved during the measurement" >&2
    return 1
  fi
  if [ "$(jq -c . <<<"${before}")" != "$(jq -c . <<<"${after}")" ]; then
    echo "error: working tree state changed during the measurement" >&2
    return 1
  fi
}

# measure_result_json <commit-json> <features-json> <extra-json> <browser-json-file>: ブラウザの測定の出力（browser.mjs の JSON）と基点から、
# 様式 v1 の結果を組み立てて標準出力へ返す。measure_browser_result と scripts/test-measure-result.sh（ビルドなしの回帰試験）から呼ばれる。
# MEASURE_WASM_OPT_APPLIED・MEASURE_WASM_OPT_DETAIL・MEASURE_CONFIG は measure_build が設定した値を読む。
# jq 1.7 系（macOS 同梱・Ubuntu 24.04）の文法では、オブジェクトの値の位置に置く二項演算（`{...} + $extra`）を括弧で囲む必要がある
# （囲まないとコンパイルが失敗し、4 種の測定がすべて止まる。#1822）。JSON は --arg / --argjson / --slurpfile だけで組み立てる。
measure_result_json() {
  local commit_json="$1" features="$2" extra="$3" browser_file="$4"
  jq -n \
    --arg measured_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --argjson commit "${commit_json}" \
    --argjson applied "${MEASURE_WASM_OPT_APPLIED}" \
    --arg detail "${MEASURE_WASM_OPT_DETAIL}" \
    --arg config "${MEASURE_CONFIG}" \
    --argjson features "${features}" \
    --arg rustc "$(rustc -V 2>/dev/null || true)" \
    --arg wbg "$(wasm-bindgen --version 2>/dev/null || true)" \
    --argjson extra "${extra}" \
    --slurpfile b "${browser_file}" \
    '$b[0] as $b | {
      schema: "fandhe-workshop/measure/v1",
      measured_at: $measured_at,
      baseline: ({
        commit: $commit,
        wasm_opt: {applied: $applied, detail: $detail},
        features: {config: $config, cargo_features: $features, profile: "release"},
        compression: $b.compression
      } + $extra),
      toolchain: {rustc: $rustc, wasm_bindgen: $wbg, browser: $b.browser, playwright: $b.playwright},
      conditions: $b.conditions,
      measurements: [$b.measurements[] | {name: .name, file: "index.html", unit: .unit, value: .value}]
    }'
}

# measure_ir_memory_check <browser-json-file> <rc>: ir-memory の browser.mjs の出力が様式どおりか（cases が 3 件で各欄の型が合い、
# crashes_total が件数の和）、終了コード（0 または 3）が crash の件数と一致するかを確かめる。食い違えば理由を出して 1 を返す。
# crash があるときの 3 だけを受け入れる（crash なしの 3 や、crash ありの 0 は出力を信用できないため断る）。
measure_ir_memory_check() {
  local file="$1" rc="$2"
  if [ "${rc}" != "0" ] && [ "${rc}" != "3" ]; then
    echo "error: unexpected exit code from browser.mjs (ir-memory)" >&2
    return 1
  fi
  if ! jq -e -s --argjson rc "${rc}" '
    def nes: type == "string" and length > 0;
    def count: type == "number" and . >= 0 and . == floor;
    length == 1 and (.[0].conditions |
      (.cases | type == "array" and length == 3
        and ([.[].name] == ["base", "pack_title", "hint_text"])
        and all((.field | nes) and (.chars | count) and (.ir_bytes | count) and (.ir_sha256 | type == "string" and test("^[0-9a-f]{64}$"))
          and (.crashes | count) and (.notices | count) and (.reason == null or (.reason | nes))
          and (.trial_peaks_kib | type == "array" and length >= 1 and all(type == "number" and . > 0))))
      and (.crashes_total | count) and (.crashes_total == ([.cases[].crashes] | add))
      and ((.crashes_total > 0) == ($rc == 3))
      and (.low_memory_device == "unverified"))
  ' "${file}" >/dev/null 2>&1; then
    echo "error: ir-memory output is malformed or its exit code does not match the crash count" >&2
    return 1
  fi
}

# measure_browser_result <kind> <commit-json> <tmpdir>: ビルド（measure_build）→ ブラウザの測定（browser.mjs）→ commit の前後比較 →
# v1 の結果の組み立て → measure_emit、までを行う。first-paint.sh と redraw.sh の共通の流れ。
# 第 5 引数 [ir-file] は ir-memory だけが使う（measure_build の IR_FILE へ渡す。ir-memory で空なら失敗）。ir-memory の browser.mjs は crash を
# 観察すると終了コード 3 で JSON を出す。このときだけ measure_ir_memory_check を通れば検証・出力まで進み、3 を返す（他の種別の 3 は失敗）。
# <kind> は first-paint / first-paint-direct / redraw / log-redraw / log-slope / log-rounds / input-paint / view-redraw / product-redraw / ir-memory / view-redraw-cpu4x / product-redraw-cpu4x（後ろ 2 つは記録だけの CPU 4 倍。TASK-485 3/3）。log-redraw・log-slope・log-rounds・input-paint・product-redraw・product-redraw-cpu4x は第 4 引数に追加の基点（{"pack": ..., "device": ...} の JSON）を取り、
# 結果の baseline へ足す。ir-memory も pack と device を要る。この 4 つでこれが欠ける・pack か device が無いときは失敗にする。view-redraw と view-redraw-cpu4x は pack の代わりに data（measure_data_json）と device を要る。<tmpdir> は呼び出し元が mktemp -d で作った空のディレクトリ（終了時の削除も呼び出し元が行う）。
# first-paint-direct は第 5 引数に IR を要り、extra は pack と device の区分（measure_device_class_json）。
# browser.mjs の出力は jq で型を確かめてから使い、JSON は --arg / --slurpfile だけで組み立てる（文字列連結で作らない）。
# 戻り値: 0 = 成功 / 1 = 失敗（標準出力は空） / 2 = 誤用（build-web.sh の 2 を引き継ぐ） / 3 = ir-memory で crash を観察（JSON は出す）。
measure_browser_result() {
  local kind="$1" commit_json="$2" tmp="$3" extra="${4:-}" ir_file="${5:-}" rc=0 brc=0 features need_extra
  if [ -z "${extra}" ]; then extra='{}'; fi
  case "${kind}" in
    log-redraw | log-slope | log-rounds | input-paint | product-redraw | product-redraw-cpu4x | ir-memory) need_extra=1 ;;
    view-redraw | view-redraw-cpu4x) need_extra=2 ;;
    first-paint-direct) need_extra=3 ;;
    *) need_extra=0 ;;
  esac
  # IR は first-paint-direct と ir-memory だけが取る（必須）。他の種別で渡されたら断る。ビルドの前に判定する。
  if { [ "${kind}" = "first-paint-direct" ] || [ "${kind}" = "ir-memory" ]; } && [ -z "${ir_file}" ]; then
    echo "error: ${kind} needs an IR file" >&2
    return 1
  fi
  if [ "${kind}" != "first-paint-direct" ] && [ "${kind}" != "ir-memory" ] && [ -n "${ir_file}" ]; then
    echo "error: ${kind} does not take an IR file" >&2
    return 1
  fi
  if [ "${need_extra}" -eq 3 ] && ! jq -e '
    (.pack.id | type == "string" and length > 0) and (.pack.sha256 | type == "string" and test("^[0-9a-f]{64}$"))
    and (.device.class == "A" or .device.class == "other") and (.device | has("cpu") or has("memory_bytes") | not)' <<<"${extra}" >/dev/null 2>&1; then
    echo "error: ${kind} needs the pack and device class baseline" >&2
    return 1
  fi
  if [ "${need_extra}" -eq 2 ] && ! jq -e '
    (.data.id | type == "string" and length > 0) and (.data.manifest_sha256 | type == "string" and test("^[0-9a-f]{64}$"))
    and (.data.files | type == "object" and length > 0)
    and (.device.os | type == "string" and length > 0) and (.device.cpu | type == "string" and length > 0)
    and (.device.logical_cores | type == "number") and (.device.memory_bytes | type == "number")
    and (.device.apple_silicon_macos | type == "boolean")' <<<"${extra}" >/dev/null 2>&1; then
    echo "error: ${kind} needs the data and device baseline" >&2
    return 1
  fi
  if [ "${need_extra}" -eq 1 ] && ! jq -e '
    (.pack.id | type == "string" and length > 0) and (.pack.sha256 | type == "string" and test("^[0-9a-f]{64}$"))
    and (.device.os | type == "string" and length > 0) and (.device.cpu | type == "string" and length > 0)
    and (.device.logical_cores | type == "number") and (.device.memory_bytes | type == "number")
    and (.device.apple_silicon_macos | type == "boolean")' <<<"${extra}" >/dev/null 2>&1; then
    echo "error: ${kind} needs the pack and device baseline" >&2
    return 1
  fi
  measure_build "${tmp}" "${ir_file}" || rc=$?
  if [ "${rc}" -ne 0 ]; then return "${rc}"; fi
  features="$(measure_features_json "${MEASURE_CONFIG}")" || return 1

  log "measuring ${kind} with Playwright (this takes a while)"
  node "${ROOT_DIR}/tools/measure/browser.mjs" --kind "${kind}" --dir "${tmp}/web" >"${tmp}/browser.json" || brc=$?
  if [ "${brc}" -ne 0 ] && { [ "${kind}" != "ir-memory" ] || [ "${brc}" -ne 3 ]; }; then
    echo "error: browser measurement failed (${kind})" >&2
    return 1
  fi
  if ! jq -e -s '
    def nes: type == "string" and length > 0;
    length == 1 and (.[0] |
      (.measurements | type == "array" and length >= 1)
      and (.measurements | all((.name | nes) and (.unit | nes) and (.value | type == "number")))
      and (.compression.command | nes) and (.compression.implementation | nes)
      and (.conditions | type == "object")
      and (.browser | nes) and (.playwright | nes))
  ' "${tmp}/browser.json" >/dev/null 2>&1; then
    echo "error: unexpected output from browser.mjs" >&2
    return 1
  fi

  if [ "${kind}" = "ir-memory" ]; then
    measure_ir_memory_check "${tmp}/browser.json" "${brc}" || return 1
  fi

  measure_assert_commit_unchanged "${commit_json}" || return 1

  measure_result_json "${commit_json}" "${features}" "${extra}" "${tmp}/browser.json" >"${tmp}/result.json" || return 1

  measure_emit "${tmp}/result.json" || return 1
  # crash を観察した測定は、検証に通った JSON を出したうえで 3 を返す（呼び出し元はそのまま終了コードにする）。
  if [ "${brc}" -eq 3 ]; then return 3; fi
}

# ---- 大きさの測定（native。fandhe-workshop/measure-native/v1。history-size.sh。TASK-216）----
# 端末・wasm-opt・圧縮を持たない測定のため、v1 とは別の様式にする（偽の値で v1 の必須欄を埋めない。docs/measurement.md）。

# jq の述語。様式 measure-native/v1 の 1 件の検査。
MEASURE_JQ_VALID_NATIVE_V1='
  def nes: type == "string" and length > 0;
  def valid_native_v1:
    .schema == "fandhe-workshop/measure-native/v1"
    and (.measured_at | nes)
    and (.baseline.commit.sha | type == "string" and test("^[0-9a-f]{40}$"))
    and (.baseline.commit.dirty | type == "boolean")
    and (.baseline.cargo_profile | nes)
    and (.baseline.pack.id | nes) and (.baseline.pack.version | nes)
    and (.baseline.pack.ir_sha256 | type == "string" and test("^[0-9a-f]{64}$"))
    and (.baseline.pack.content_hash | type == "string" and test("^[0-9a-f]{64}$"))
    and (.baseline.ops_sequence.id | nes)
    and (.baseline.ops_sequence.ops_sha256 | type == "string" and test("^[0-9a-f]{64}$"))
    and (.baseline.ops_sequence.goto_substitutions | type == "number")
    and (.toolchain.rustc | nes)
    and (.conditions | type == "object")
    and (.measurements | type == "array" and length >= 1)
    and (.measurements | all(
      (.name | nes) and (.unit | nes)
      and (.value | type == "number" and . > 0 and . == floor)));
'

# measure_validate_native <json-file>: 様式 measure-native/v1 を満たすか検査する。ちょうど 1 件の JSON でなければ非 0。
measure_validate_native() {
  local file="$1"
  [ -r "${file}" ] || return 1
  jq -e -s "${MEASURE_JQ_VALID_NATIVE_V1}"'
    length == 1 and (.[0] | valid_native_v1)
  ' "${file}" >/dev/null 2>&1
}

# measure_emit_native <json-file>: 検証に通ったときだけ標準出力へ出す。通らなければ何も出さず 1 を返す。
measure_emit_native() {
  local file="$1"
  if ! measure_validate_native "${file}"; then
    echo "error: native result is malformed or missing baseline fields" >&2
    return 1
  fi
  jq . "${file}"
}

# measure_native_result_json <commit-json> <line-json-file>: 試験の出力（HISTORY-SIZE の行の JSON）と基点から、
# 様式 measure-native/v1 の結果を組み立てて標準出力へ返す。history-size.sh と scripts/test-measure-result.sh から呼ばれる。
# JSON は --arg / --argjson / --slurpfile だけで組み立てる（オブジェクトの値の位置に二項演算を置かない。jq 1.7 の注意は #1822）。
measure_native_result_json() {
  local commit_json="$1" line_file="$2"
  jq -n \
    --arg measured_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --argjson commit "${commit_json}" \
    --arg rustc "$(rustc -V 2>/dev/null || true)" \
    --slurpfile l "${line_file}" \
    '$l[0] as $l | {
      schema: "fandhe-workshop/measure-native/v1",
      measured_at: $measured_at,
      baseline: {
        commit: $commit,
        cargo_profile: "test",
        pack: $l.pack,
        ops_sequence: $l.ops_sequence
      },
      toolchain: {rustc: $rustc},
      conditions: {
        condition: "A",
        checkpoints_at: ["round1_end", "round2_end"],
        scope_note: "no wrong inputs; some steps skipped with GoTo; conditions B/C and the 5 MB judgment belong to TASK-336"
      },
      measurements: [
        {name: "history_total_bytes", unit: "bytes", value: $l.size.total_bytes},
        {name: "history_ops_bytes", unit: "bytes", value: $l.size.ops_bytes},
        {name: "history_snapshot_bytes", unit: "bytes", value: $l.size.snapshot_bytes},
        {name: "history_ops", unit: "count", value: $l.size.ops},
        {name: "history_log_entries", unit: "count", value: $l.size.log_entries},
        {name: "history_changed_entries", unit: "count", value: $l.size.changed_entries},
        {name: "history_changed_bytes", unit: "bytes", value: $l.size.changed_bytes},
        {name: "history_changed_ratio_ppm", unit: "ppm", value: $l.size.changed_ratio_ppm},
        {name: "history_checkpoints", unit: "count", value: $l.size.checkpoints},
        {name: "history_checkpoint_bytes", unit: "bytes", value: $l.size.checkpoint_bytes}
      ]
    }'
}

# measure_native_log_rounds_result_json <commit-json> <line-json-file>: 試験の出力（LOG-ROUNDS の行の JSON）と基点から、
# 様式 measure-native/v1 の結果を組み立てて標準出力へ返す（TASK-454 1/2）。log-items.sh と scripts/test-measure-result.sh から呼ばれる。
# measurements は正の整数だけ（検証の述語の条件）。0 になりうる畳み・印の数は conditions に置く。
# JSON は --arg / --argjson / --slurpfile だけで組み立てる（jq 1.7 の注意は #1822）。
measure_native_log_rounds_result_json() {
  local commit_json="$1" line_file="$2"
  jq -n \
    --arg measured_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --argjson commit "${commit_json}" \
    --arg rustc "$(rustc -V 2>/dev/null || true)" \
    --slurpfile l "${line_file}" \
    '$l[0] as $l | ($l.log.items_max <= $l.log.target) as $within | {
      schema: "fandhe-workshop/measure-native/v1",
      measured_at: $measured_at,
      baseline: {
        commit: $commit,
        cargo_profile: "test",
        pack: $l.pack,
        ops_sequence: $l.ops_sequence
      },
      toolchain: {rustc: $rustc},
      conditions: {
        condition: "A",
        target: $l.log.target,
        within_target: $within,
        folded_steps: $l.log.folded_steps,
        limit_marks: $l.log.limit_marks,
        scope_note: "native render of the right log (li count), not the browser DOM; no wrong inputs; some steps skipped with GoTo; safety valve and over-limit view belong to TASK-454 2/2"
      },
      measurements: [
        {name: "log_rounds_items_max", unit: "count", value: $l.log.items_max},
        {name: "log_rounds_items_round2_end", unit: "count", value: $l.log.items_round2_end},
        {name: "log_rounds_events_round2_end", unit: "count", value: $l.log.events_round2_end},
        {name: "log_rounds_groups_round2_end", unit: "count", value: $l.log.groups_round2_end},
        {name: "log_rounds_steps_observed", unit: "count", value: $l.log.steps_observed}
      ]
    }'
}
