# L2 の配布物の更新手順

```text
手順書の版: 2
決定日: 2026-10-28
更新日: 2026-10-31（Windows 向けの zip を追加。#1932）
状態: 固定（契約）
```

## 1. 位置づけと版

- 本文書は、教材を更新したときに L2 の配布物（受講者が Rust なしで開く zip）を作り直す手順を 1 か所に固定する（TASK-116、DLV-18）。根拠は D-58 と D-71 である
- 対象は配布する側の作業である。受講者の側は従来どおり、Rust・node を使わず「展開・`./start.sh`・URL を開く」の 3 操作だけで済む
- 仕様は private のため、本文を転記せず ID だけで参照する
- 版の規則: 手順を変えるときは版を上げ、変更履歴に記録する

## 2. 配布の形の決定

- L2 の配布の形は案 (ii)（汎用の静的サーバ miniserve 0.35.0 ＋静的ファイル）に決まった（D-58 は 2026-10-12、D-71 は 2026-10-22）。配布物は `fandhe-workshop-l2/{start.sh, miniserve, site/}` の zip である（Windows 向けは `{start.cmd, miniserve.exe, site/}`。TASK-198・DLV-22。手順は `docs/l2-windows.md`）
- 案 (i)（`dist-server` 系の単一ファイル）は採らなかった。理由は、上流の workspace の複製（約 106MB）が要ることである（PoC-10）。したがって、案 (i) の「単一ファイルの作り直し 1 回」に当たる更新の道具は作らない
- 案 (ii) の利点は、教材を更新したときの L2 固有の作業が `site/` の差し替え 1 操作で済むことである。本文書の更新の手順はその操作を道具にしたものである
- TASK-12 の決定は、spec の 04（振る舞い）の DLV-18 に反映済みである。確認した版は `docs/testing.md` の 5.359 に記録する

## 3. 前提

- node と `unzip`（`/usr/bin/unzip`）が使えること。Rust・通信・miniserve の実行ファイルの再取得は要らない（更新では取得済みの miniserve を旧 zip から引き継ぐ）
- 更新前の zip があること。初回は 4 節で作る

## 4. 初回の作成

1. 教材を `fandhe-pack compile` し、出力の IR を `IR_FILE` に渡して `make build-web` を流す（配信する設定の `dist/web` ができる。L1 の配信と共通の作業）
2. miniserve 0.35.0 の実行ファイルを取得し、`L2_SERVER` に渡して `make build-l2` を流す。実行ファイルの sha256 は `tools/l2/miniserve.sha256` の固定値と照合される。通れば `dist/l2/fandhe-workshop-l2-<platform>.zip` ができる

## 5. 教材を更新したとき

1. 教材を直し、4 節の 1 と同じに `compile` と `make build-web` をやり直す。これは L1 と共通の作業である
2. L2 に固有の作業は次の 1 回だけである

    ```bash
    L2_ZIP=dist/l2/fandhe-workshop-l2-macos-arm64.zip make update-l2
    ```

    `L2_SITE`（既定 `dist/web`）と `L2_OUT`（既定 `dist/l2`）は環境変数で変えられる。出力は `fandhe-workshop-l2-<platform>.zip` で、`<platform>` は旧 zip にある実行ファイルの名前（`miniserve` か `miniserve.exe`）で配置を決め、その sha256 を固定値と照合して決まる。Windows 向けの zip（`fandhe-workshop-l2-windows-x64.zip`）も同じ操作で更新できる（作業は Linux・macOS 上で行う。`/usr/bin/unzip` が要る）。出力先が入力と同じ場所なら上書きされる（書き込みは一時ファイルから rename するため、失敗しても元の zip は壊れない）
3. できた zip を配る。受講者の操作は変わらない

### 5.1 引き継ぐもの・入れ替えるもの

| 対象 | 扱い |
|---|---|
| `start.sh`・`miniserve`（macOS・Linux） | 旧 zip のバイト列と mode をそのまま引き継ぐ。mode 755 の通常のファイルであることを求める |
| `start.cmd`・`miniserve.exe`（Windows） | 旧 zip のバイト列と mode をそのまま引き継ぐ（`start.cmd` は CRLF のまま）。実行の権限は求めず、通常のファイルであることだけを求める |
| `site/` 以外の同梱物（ライセンス文・告知など。TASK-137） | 旧 zip のままバイト単位で引き継ぐ |
| `site/` | 新しい配信物に丸ごと入れ替える。旧にだけあったファイルは残らない |

出力は決定的である。同じ旧 zip と同じ配信物から、同じバイト列ができる。`site/` 以外が同じなら、`update-l2` の出力は `build-l2` の出力と同じバイト列になる。

### 5.2 検査（fail-closed）

どれかに当たると何も書かず失敗する（終了コード 1）。

- 旧 zip の `miniserve` の sha256 が固定表のどの資産とも一致しない
- 旧 zip が不正: `fandhe-workshop-l2/` の外・`..`・絶対パス・バックスラッシュ・非 ASCII・重複した名前、リンク、許さない mode、必須エントリ（`start.sh`・`miniserve`・`site/`、Windows は `start.cmd`・`miniserve.exe`・`site/`）の欠落、両方の配置を含む・混在する配置、実行ファイルの名前と固定値の食い違い、申告の合計または実際に読み出した合計が 512 MiB を超える（申告値は信用せず、実バイト数で打ち切る）、壊れている
- Windows 向けの zip では、新しい配信物に Windows で展開できない名前（予約名・禁止文字・大文字小文字だけ違う重複）がある（`build-l2` と同じ規則）
- 新しい配信物が `tools/check-delivery.sh`（試験用の口・Raw の接頭辞・試験用 IR・試験用クエリの印・`.d.ts`・リンクが無いこと）か、`ir/main.json` が JSON であることの検査に落ちる

誤用（`L2_ZIP` の未設定・不在・通常のファイルでない、位置引数あり、存在せず `dist/` の外の `L2_OUT`）は終了コード 2 である。

### 5.3 手で行う同等の操作（参考）

展開して `site/` を入れ替え、再び zip にしても同じ内容になる。ただし、決定的なバイト列・固定表との照合・配信物の検査は失われるため、通常は使わない。

## 6. 確認の方法

- `make test-update-l2`: 合成の配信物での回帰テスト（速い。ビルドなし）
- `make test-l2-run`: 実物の miniserve で、別の IR で更新した zip を模擬環境（`PATH=/usr/bin:/bin`）で展開・起動し、新しい IR が配られることと画面がマウントされることを確かめる（`L2_SERVER` が要る。遅く、CI の `browser-test` が流す）

## 7. 限界

- `start.sh`・`miniserve` 自体を更新するとき（miniserve の版上げなど）は、`update-l2` では済まず `build-l2` のやり直しが要る
- macOS の実機での起動・quarantine の扱いは TASK-117・118・119 で扱う。ライセンス文の同梱は TASK-137 で扱う（同梱されても `update-l2` は `site/` 以外を引き継ぐ）
- 受講者がすでに展開した木で `site/` だけを差し替える運用と、その後の履歴の扱いは確かめていない。配るのは zip ごとにする
- Windows 向けの zip も更新できる（#1932）。ただし Windows の実機での起動は未確認（TASK-120）で、`make test-l2-run` は linux-x64 だけを流す。`update-l2` 自体を Windows 上で動かすことは対象外である（`/usr/bin/unzip` 前提）

## 8. 変更履歴

| 版 | 日付 | 内容 |
|---|---|---|
| 1 | 2026-10-28 | 初版（TASK-116） |
| 2 | 2026-10-31 | Windows 向けの zip の更新を追加（#1932） |
