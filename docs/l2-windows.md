# Windows 向けの L2 の配布物の作成と起動の手順

版 2（2026-10-31。TASK-198。DLV-22。#1932）。

Windows（x64）向けの L2 の配布物（zip）を `tools/build-l2.mjs` から再現可能に作る手順と、受講者の起動の手順を示します。
Windows の実機での起動確認と OS の警告の記録は未実施で、TASK-120（人の確認）で行います。DLV-22 は「検討中」のままです。

教材を更新したときは、`L2_ZIP=dist/l2/fandhe-workshop-l2-windows-x64.zip make update-l2` で `site/` だけを差し替えます
（作業は Linux・macOS 上で行います）。手順は `docs/l2-update-procedure.md` にあります。

## 1. 配布物の中身

```text
fandhe-workshop-l2/
├── start.cmd        # 起動スクリプト（zip の中では CRLF）
├── miniserve.exe    # miniserve 0.35.0 の Windows x64 版（sha256 を固定値と照合）
└── site/            # 配信する設定の配信物
```

macOS・Linux の配布物との違いは、`start.sh` が `start.cmd` に、`miniserve` が `miniserve.exe` になる点だけです。
`site/` は同じ入力から同じ中身になります。

## 2. 作成の手順

1. `make build-web` で配信する設定の `dist/web` を作る
2. `https://github.com/svenstaro/miniserve/releases/download/v0.35.0/miniserve-0.35.0-x86_64-pc-windows-msvc.exe` を取得する
3. 次を実行する。`tools/l2/miniserve.sha256` の固定値と一致しなければ zip は作られない

```bash
L2_PLATFORM=windows-x64 L2_SERVER=<取得した exe> make build-l2
```

出力は `dist/l2/fandhe-workshop-l2-windows-x64.zip`。同じ入力から同じバイト列になる（決定的）。
`start.cmd` は repo では LF で持ち、zip に入れるときに CRLF へ変換する。ASCII だけでない・CR を含む場合は作成を止める。
Windows で展開できない名前（禁止文字・予約名・末尾の `.`・大文字小文字だけが違う重複）が `site/` にあるときも止める。

## 3. 受講者の起動の手順

1. zip をエクスプローラーの「すべて展開」で展開する。zip の中を開いたまま `start.cmd` を実行しない（`site/` が無い場所で動くため）
2. 展開先の `start.cmd` をダブルクリックする。または PowerShell・コマンドプロンプトで展開先へ移り `.\start.cmd` を実行する
3. 表示された `http://127.0.0.1:8787/` を Chrome で開く。`localhost` と混ぜない（別オリジンになり、保存した履歴が見えない）

- 待ち受けの番号の変更: コマンドプロンプトは `set PORT=8800`、PowerShell は `$env:PORT=8800` の後に `start.cmd` を実行する。1〜65535 の数字だけを受け付け、それ以外は終了コード 2
- 終了: Ctrl-C（「バッチ ジョブを終了しますか」には `Y`）
- ブラウザは自動では開かない。待ち受けは 127.0.0.1 だけで、他の機器からは届かない

## 4. 限界と未確認

- 未確認: `start.cmd` の実行時の振る舞い、SmartScreen・Defender・ファイアウォールの警告の有無と対処。Linux の試験では実行できないため、試験は `start.cmd` の中身と `start.sh` の引数の一致を照合するだけ。TASK-120 の実機の確認で記録する
- 範囲外: 配布物への案内文の同梱（TASK-117）、ライセンス文の同梱（TASK-137）
