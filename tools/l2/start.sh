#!/usr/bin/env bash
# L2 の配布物（fandhe-workshop-l2）の起動スクリプト。zip の展開先にそのまま入る（D-58・D-71・DLV-13）。
#
# 目的: 受講者の操作を「展開・./start.sh・URL を開く」の 3 つにする。miniserve を 127.0.0.1 だけで待ち受け、
# site/（配信する設定の配信物）を配る（DLV-14）。tools/build-l2.mjs が配布物へ写す。
# 使い方: ./start.sh（PORT=<1〜65535> で待ち受けの番号を変えられる。既定 8787。FANDHE_NO_OPEN=1 で自動で開かない）
# 終了コード: 2 = PORT が不正 / 1 = 展開先へ移れない / それ以外は miniserve の終了状態（exec で置き換える）
set -euo pipefail

cd "$(dirname "$0")" || exit 1

PORT="${PORT:-8787}"
case "${PORT}" in
  '' | *[!0-9]* | 0*)
    echo "error: PORT must be a number from 1 to 65535: ${PORT}" >&2
    exit 2
    ;;
esac
if [ "${#PORT}" -gt 5 ] || [ "${PORT}" -gt 65535 ]; then
  echo "error: PORT must be a number from 1 to 65535: ${PORT}" >&2
  exit 2
fi

URL="http://127.0.0.1:${PORT}/"
echo "起動します。Chrome で次の URL を開いてください: ${URL}"
echo "終了は Ctrl-C です。"

if [ -z "${FANDHE_NO_OPEN:-}" ] && command -v open >/dev/null 2>&1; then
  (
    sleep 1
    open "${URL}" >/dev/null 2>&1
  ) &
fi

# 全アドレスで待ち受ける既定を避けるため、-i で 127.0.0.1 に限る。引数は PoC-10 で完走を確かめた形のまま。
exec ./miniserve --index index.html -i 127.0.0.1 -p "${PORT}" site
