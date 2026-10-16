// 固定の後に足したもの（理由は ../README.md の「固定の後に足したもの」節）。差分の行数を変えて、再描画の時間の傾きを測るための入力。
// generate.mjs の diffRows と同じ生成（乱数の種は 1000 + n）。使い方: node generate-sweep.mjs <出力ディレクトリ>
import fs from "node:fs";
import path from "node:path";
const out = process.argv[2];
if (!out) { console.error("使い方: node generate-sweep.mjs <出力ディレクトリ>"); process.exit(2); }
fs.mkdirSync(out, { recursive: true });
function rng(seed) { let s = seed >>> 0 || 1; return () => { s ^= s << 13; s >>>= 0; s ^= s >>> 17; s ^= s << 5; s >>>= 0; return s / 4294967296; }; }
const cell = (label, detail = "", state = "", tags = []) => ({ label, detail, state, tags });
function diffRows(n, seed) {
  const r = rng(seed);
  const rows = [];
  for (let i = 1; i <= n; i++) {
    const x = r();
    const id = `l${i}`;
    const text = i % 17 === 0 ? "" : `    let value_${i} = compute(${i});`;
    if (x < 0.04) rows.push({ id, depth: 0, kind: "line", before: null, after: cell(text, `:${i}`, "add") });
    else if (x < 0.08) rows.push({ id, depth: 0, kind: "line", before: cell(text, `${i}:`, "del"), after: null });
    else if (x < 0.10) rows.push({ id, depth: 0, kind: "line", before: cell(text, `${i}:${i}`, "ctx"), after: cell(text + " // 更新", `${i}:${i}`, "ctx") });
    else rows.push({ id, depth: 0, kind: "line", before: cell(text, `${i}:${i}`, "ctx"), after: cell(text, `${i}:${i}`, "ctx") });
  }
  return { view: "diff", title: `generated diff ${n}`, rows };
}
for (const n of [1000, 2000, 3000]) fs.writeFileSync(path.join(out, `diff-${n}.json`), JSON.stringify(diffRows(n, 1000 + n)));
