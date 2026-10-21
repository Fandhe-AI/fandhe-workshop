// ページの入口。wasm を読み込んで Hello をマウントし、結果を html の data-mounted に残す。
// ブラウザ試験はこの属性を待つ。試験用の meta がある設定でだけ test-hooks.js を読む。
import init, { mount_app } from "./wasm/fandhe_workshop_web.js";

const root = document.documentElement;

try {
  await init();
  // 直接リンク ?start=<開始点 ID>（D-68・TASK-109）。読むのは start だけで、値は Rust 側が IR の starts と完全一致で照合する。
  // 2 件以上の指定は、どの開始点とも一致し得ない値（空でない NUL 入りの文字列）に倒して断らせる（fail-closed）。値は出力しない。
  const starts = new URLSearchParams(location.search).getAll("start");
  const start = starts.length === 0 ? undefined : starts.length === 1 ? starts[0] : "\u0000";
  // OS の推定（修飾キーの表示。D-48）のため、ブラウザの User-Agent も渡す。Rust 側はブラウザの値を読まない。
  await mount_app("app", navigator.userAgent ?? "", start);
  if (document.querySelector('meta[name="fandhe-test-hooks"]')) {
    const hooks = await import("./test-hooks.js");
    window.__fandheTest = hooks;
  }
  root.dataset.mounted = "true";
} catch (e) {
  console.error(e);
  root.dataset.mounted = "error";
}
