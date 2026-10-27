#!/usr/bin/env bash
# 測定の結果（JSON）を組み立て・検証・出力し、測定前のビルドとログ解析を行う共有ヘルパー。直接実行せず source して使う（実行権限は付けない）:
#   source "${DIR}/../../scripts/lib.sh"   # ROOT_DIR・log・require_cmd を先に読み込む
#   source "${DIR}/lib.sh"
#
# 存在理由: 基点（commit・wasm-opt・機能の範囲・圧縮の設定）の付かない値は、基点の違う値どうしの誤った比較を招く
# （docs/measurement.md）。結果を出す経路を measure_emit / measure_emit_run だけにして、基点が欠けた結果を構造的に出さない。
# 呼び出し元: tools/measure/wasm-size.sh・first-paint.sh・redraw.sh・all.sh。ビルドとログ解析・commit の前後比較も
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

# measure_pack_json <ir-file>: 測った教材の基点 {"id","version","nodes","sha256"} を返す。log-redraw・log-slope・input-paint の基点の「教材」。
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

# measure_build <tmpdir>: tools/build-web.sh を release で <tmpdir>/web へビルドし（dist/ は上書きしない）、ログを解析する。
# ビルドログは標準エラーへ流す。TEST_HOOKS・SKIP_WASM_OPT は呼び出し元の環境を build-web.sh へそのまま渡し、IR_FILE は空にする。
# 成功時に MEASURE_CONFIG（delivery / test）・MEASURE_WASM_OPT_APPLIED（true / false）・MEASURE_WASM_OPT_DETAIL・
# MEASURE_LOG_RAW・MEASURE_LOG_GZ を設定する（呼び出し元が参照する）。
# 戻り値: 0 = 成功 / 1 = 失敗 / 2 = build-web.sh の誤用（そのまま返す）。各ログ行が厳密に 1 行でなければ失敗する（fail-closed）。
measure_build() {
  local tmp="$1" rc=0 config_line wasm_opt_line size_line
  log "building (release) via tools/build-web.sh"
  OUT_DIR="${tmp}/web" IR_FILE="" "${ROOT_DIR}/tools/build-web.sh" release >"${tmp}/build.log" 2>&1 || rc=$?
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

# measure_browser_result <kind> <commit-json> <tmpdir>: ビルド（measure_build）→ ブラウザの測定（browser.mjs）→ commit の前後比較 →
# v1 の結果の組み立て → measure_emit、までを行う。first-paint.sh と redraw.sh の共通の流れ。
# <kind> は first-paint / redraw / log-redraw / log-slope / input-paint。log-redraw・log-slope・input-paint は第 4 引数に追加の基点（{"pack": ..., "device": ...} の JSON）を取り、
# 結果の baseline へ足す。この 3 つでこれが欠ける・pack か device が無いときは失敗にする。<tmpdir> は呼び出し元が mktemp -d で作った空のディレクトリ（終了時の削除も呼び出し元が行う）。
# browser.mjs の出力は jq で型を確かめてから使い、JSON は --arg / --slurpfile だけで組み立てる（文字列連結で作らない）。
# 戻り値: 0 = 成功 / 1 = 失敗（標準出力は空） / 2 = 誤用（build-web.sh の 2 を引き継ぐ）。
measure_browser_result() {
  local kind="$1" commit_json="$2" tmp="$3" extra="${4:-}" rc=0 features need_extra
  if [ -z "${extra}" ]; then extra='{}'; fi
  case "${kind}" in
    log-redraw | log-slope | input-paint) need_extra=1 ;;
    *) need_extra=0 ;;
  esac
  if [ "${need_extra}" -eq 1 ] && ! jq -e '
    (.pack.id | type == "string" and length > 0) and (.pack.sha256 | type == "string" and test("^[0-9a-f]{64}$"))
    and (.device.os | type == "string" and length > 0) and (.device.cpu | type == "string" and length > 0)
    and (.device.logical_cores | type == "number") and (.device.memory_bytes | type == "number")
    and (.device.apple_silicon_macos | type == "boolean")' <<<"${extra}" >/dev/null 2>&1; then
    echo "error: ${kind} needs the pack and device baseline" >&2
    return 1
  fi
  measure_build "${tmp}" || rc=$?
  if [ "${rc}" -ne 0 ]; then return "${rc}"; fi
  features="$(measure_features_json "${MEASURE_CONFIG}")" || return 1

  log "measuring ${kind} with Playwright (this takes a while)"
  if ! node "${ROOT_DIR}/tools/measure/browser.mjs" --kind "${kind}" --dir "${tmp}/web" >"${tmp}/browser.json"; then
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

  measure_assert_commit_unchanged "${commit_json}" || return 1

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
    --slurpfile b "${tmp}/browser.json" \
    '$b[0] as $b | {
      schema: "fandhe-workshop/measure/v1",
      measured_at: $measured_at,
      baseline: {
        commit: $commit,
        wasm_opt: {applied: $applied, detail: $detail},
        features: {config: $config, cargo_features: $features, profile: "release"},
        compression: $b.compression
      } + $extra,
      toolchain: {rustc: $rustc, wasm_bindgen: $wbg, browser: $b.browser, playwright: $b.playwright},
      conditions: $b.conditions,
      measurements: [$b.measurements[] | {name: .name, file: "index.html", unit: .unit, value: .value}]
    }' >"${tmp}/result.json"

  measure_emit "${tmp}/result.json"
}
