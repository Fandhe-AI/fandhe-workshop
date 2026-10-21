// ページの入口。wasm を読み込んで Hello をマウントし、結果を html の data-mounted に残す。
// ブラウザ試験はこの属性を待つ。試験用の meta がある設定でだけ test-hooks.js を読む。
import init, { mount_app } from "./wasm/fandhe_workshop_web.js";

const root = document.documentElement;

try {
  await init();
  // OS の推定（修飾キーの表示。D-48）のため、ブラウザの User-Agent を渡す。Rust 側はブラウザの値を読まない。
  await mount_app("app", navigator.userAgent ?? "");
  if (document.querySelector('meta[name="fandhe-test-hooks"]')) {
    const hooks = await import("./test-hooks.js");
    window.__fandheTest = hooks;
  }
  root.dataset.mounted = "true";
} catch (e) {
  console.error(e);
  root.dataset.mounted = "error";
}
