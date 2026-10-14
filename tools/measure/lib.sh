#!/usr/bin/env bash
# 測定の結果（JSON）を組み立て・検証・出力する共有ヘルパー。直接実行せず source して使う（実行権限は付けない）:
#   source "${DIR}/../../scripts/lib.sh"   # ROOT_DIR・log・require_cmd を先に読み込む
#   source "${DIR}/lib.sh"
#
# 存在理由: 基点（commit・wasm-opt・機能の範囲・圧縮の設定）の付かない値は、基点の違う値どうしの誤った比較を招く
# （docs/measurement.md）。結果を出す経路を measure_emit だけにして、基点が欠けた結果を構造的に出さない。
# 呼び出し元: tools/measure/wasm-size.sh。#92 の初回表示・再描画の測定も再利用する。
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

# measure_validate <json-file>: 様式 v1 を満たすか検査する。1 つでも外れたら非 0（理由は出さず、emit が一括で報告する）。
measure_validate() {
  local file="$1"
  [ -r "${file}" ] || return 1
  # -s で入力全体を配列に受け、ちょうど 1 件であることを確かめてから検証する。jq -e は最後の出力値だけで
  # 終了状態を決めるため、複数の JSON 値（基点の欠けた値の後ろに正常な値を連結した入力）が通るのを防ぐ。
  jq -e -s '
    def nes: type == "string" and length > 0;
    length == 1 and (.[0] |
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
      and (.value | type == "number" and . > 0 and . == floor))))
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
