#!/usr/bin/env bash
# 変換中の判定の変異 M1・M3 の対照（test-ime-composing-control.sh）が使う判定の関数。source 専用で、直接は実行しない。
# 判定を別ファイルに分けるのは、ビルドもブラウザも要らない回帰テスト（test-lib-ime-control.sh）で、
# 「印の取り違え」「別の理由の失敗を成立と数える」誤りを速く固定するため。
# 判定は fail-closed にする。印の無い失敗・mount の失敗は成立と数えない。
#
# 前提: 呼び出し側が lib.sh と lib-mutations.sh（REAL_ROOT 定義済み）を source している。

# 固定の印。tools/browser/ime-sentences.test.mjs の MIS_SEND_MARK と合わせる。
# テスト名の「誤送信 0 件」に一致しないよう、件数が 1 以上の形まで要求する。
# shellcheck disable=SC2034 # 呼び出し側（test-ime-composing-control.sh）も出力の抜き出しに使う
IME_MIS_SEND_RE='IME-MIS-SEND 誤送信 [1-9][0-9]* 件'

# M3 の検出の証拠: 変換中の Esc の試験の失敗行。
# shellcheck disable=SC2034 # judge_m3 が使う
IME_ESC_FAIL_RE='^[[:space:]]*(✖|not ok).*変換中の Esc'

# load_variant <def-file>: 定義を読み、M_TARGET・M_FIND・M_REPLACE を使える状態にする。
load_variant() {
  parse_def "$1"
}

# judge_m1 <rc> <out-file>: established | not-established | other を標準出力へ。理由は標準エラーへ。
# M1（Enter を除外から外す）は、誤送信の件数つきの印で失敗して初めて成立と数える。
judge_m1() {
  local rc="$1" out="$2"
  if [ "${rc}" -eq 0 ]; then
    echo "M1: the test passed without the Enter exclusion" >&2
    echo not-established
    return 0
  fi
  if grep -Fq 'mount failed' "${out}"; then
    echo "M1: the test failed for another reason (mount failed)" >&2
    echo other
    return 0
  fi
  if ! grep -Eq "${IME_MIS_SEND_RE}" "${out}"; then
    echo "M1: the test failed for another reason (no IME-MIS-SEND mark with a count)" >&2
    echo other
    return 0
  fi
  echo established
}

# judge_m3 <rc> <out-file>: survived | detected | other を標準出力へ。
# M3（Esc を除外から外す）は、上流が変換中の keydown を先に除くため、ブラウザでは生き残るのが記録（docs/testing.md 5.503）。
judge_m3() {
  local rc="$1" out="$2"
  if [ "${rc}" -eq 0 ]; then
    echo survived
    return 0
  fi
  if grep -Fq 'mount failed' "${out}"; then
    echo "M3: the test failed for another reason (mount failed)" >&2
    echo other
    return 0
  fi
  # 検出と数えるのは、Esc の表明（AssertionError）で失敗した証拠がある場合だけにする。
  # Chromium の起動失敗・モジュールの読み込み失敗・タイムアウトは other（fail-closed）。
  if grep -Eq "${IME_ESC_FAIL_RE}" "${out}" && grep -Fq 'AssertionError' "${out}"; then
    echo detected
    return 0
  fi
  echo "M3: the test failed without an Esc assertion failure (launch, module load or timeout?)" >&2
  echo other
}

# combine_rc <m1> <m3>: 終了コードを標準出力へ。0 = 成立かつ記録どおり / 1 = M1 不成立 / 2 = 別の理由 / 3 = M3 が検出された。
# 重い方を返す（2 > 1 > 3）。
combine_rc() {
  local m1="$1" m3="$2"
  if [ "${m1}" = "other" ] || [ "${m3}" = "other" ]; then
    echo 2
  elif [ "${m1}" = "not-established" ]; then
    echo 1
  elif [ "${m3}" = "detected" ]; then
    echo 3
  else
    echo 0
  fi
}
