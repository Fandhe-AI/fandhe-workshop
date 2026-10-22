// ページの入口。wasm を読み込んで Hello をマウントし、結果を html の data-mounted に残す。
// ブラウザ試験はこの属性を待つ。試験用の meta がある設定でだけ test-hooks.js を読む。
import init, { mount_app, import_history, history_read_limit } from "./wasm/fandhe_workshop_web.js";

const root = document.documentElement;

try {
  await init();
  // 直接リンク ?start=<開始点 ID>（D-68・TASK-109）。読むのは start だけで、値は Rust 側が IR の starts と完全一致で照合する。
  // 2 件以上の指定は、どの開始点とも一致し得ない値（空でない NUL 入りの文字列）に倒して断らせる（fail-closed）。値は出力しない。
  const starts = new URLSearchParams(location.search).getAll("start");
  const start = starts.length === 0 ? undefined : starts.length === 1 ? starts[0] : "\u0000";
  // OS の推定（修飾キーの表示。D-48）のため、ブラウザの User-Agent も渡す。Rust 側はブラウザの値を読まない。
  // 同梱の IR（TASK-112 2/2）。印の meta がある配信物でだけ、固定の ./ir/main.json を取得して渡す（URL の値から場所は作らない。SAFE-33）。
  // 取得に失敗したら空のバイト列を渡し、Rust 側が断りの画面に倒す（fail-closed）。印が無ければ取得せず、IR を持たないまま起動する。
  let ir;
  if (document.querySelector('meta[name="fandhe-ir"]')) {
    try {
      const r = await fetch("./ir/main.json");
      ir = r.ok ? new Uint8Array(await r.arrayBuffer()) : new Uint8Array(0);
    } catch {
      ir = new Uint8Array(0);
    }
  }
  await mount_app("app", navigator.userAgent ?? "", start, ir);
  if (document.querySelector('meta[name="fandhe-test-hooks"]')) {
    const hooks = await import("./test-hooks.js");
    window.__fandheTest = hooks;
  }
  // 履歴の読み込み: 枠の shell-import の中の input[type=file] で選んだファイルを読み、バイト列のまま Rust へ渡す（文字列にすると不正な UTF-8 が置換されるため）（SHELL-15）。
  // 読むのは上限 + 1 バイトまで（大きさの断りは Rust 側）。同じファイルを選び直せるよう値は戻す。本文・ファイル名・例外の message は出力しない。
  // 読み取りの完了順で古い選択が適用されないよう、選択のたびに要求番号を進め、完了時に最新の要求だけを配送する（古い要求は捨てる）。
  // 読み取りが失敗したときは、状態を変えずに固定の「途切れた履歴」（1 バイトの `{`）を配送し、開かない通知を 1 件出させる。
  let importSeq = 0;
  document.addEventListener("change", async (ev) => {
    const input = ev.target;
    if (!(input instanceof HTMLInputElement) || input.type !== "file") return;
    if (!input.closest('[data-testid="shell-import"]')) return;
    const file = input.files && input.files[0];
    input.value = "";
    if (!file) return;
    const seq = ++importSeq;
    let bytes;
    try {
      bytes = new Uint8Array(await file.slice(0, history_read_limit()).arrayBuffer());
    } catch (_) {
      bytes = new Uint8Array([0x7b]);
    }
    if (seq !== importSeq) return;
    import_history(bytes);
  });
  // 書き出しのリンク: 上流の属性の検査は blob: の href を落とすため、URL は data-export-url に載る。自分のオリジンの blob: だけを href へ写す
  // （setAttribute は使わない）。リンクは履歴が変わるたびに作り直され、写し損ねたものだけを探す。
  const blobPrefix = `blob:${window.origin}/`;
  const resultBox = document.querySelector(".sh-history-result");
  const wireExportLink = () => {
    for (const a of (resultBox ?? document).querySelectorAll('[data-testid="export-download"]:not([href])')) {
      const url = a.getAttribute("data-export-url") ?? "";
      if (url.startsWith(blobPrefix)) a.href = url;
    }
  };
  new MutationObserver(wireExportLink).observe(resultBox ?? document.body, { childList: true, subtree: true });
  root.dataset.mounted = "true";
} catch (e) {
  console.error(e);
  root.dataset.mounted = "error";
}
