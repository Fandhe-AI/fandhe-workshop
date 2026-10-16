// PoC-8 計測用の大きな内容を決定的に生成する（乱数の種は固定。大きなファイルそのものはリポジトリに置かない）。
// 使い方: node generate.mjs <出力ディレクトリ>
// 出力: doc-large-before.md / doc-large-after.md（200 行）、diff-500.json、tasks-50.json、filetree-200.json、
//       cards-20.json、checks-20.json、diff-4095.json / diff-4096.json / diff-4097.json（keyed list の上限の確認用）
import fs from "node:fs";
import path from "node:path";

const out = process.argv[2];
if (!out) { console.error("使い方: node generate.mjs <出力ディレクトリ>"); process.exit(2); }
fs.mkdirSync(out, { recursive: true });

// xorshift32（種は固定）
function rng(seed) { let s = seed >>> 0 || 1; return () => { s ^= s << 13; s >>>= 0; s ^= s >>> 17; s ^= s << 5; s >>>= 0; return s / 4294967296; }; }
const cell = (label, detail = "", state = "", tags = []) => ({ label, detail, state, tags });
const write = (name, v) => fs.writeFileSync(path.join(out, name), typeof v === "string" ? v : JSON.stringify(v));

// ---- 文書 200 行（Markdown）。after は段落 10 行に文言を足し、一覧 5 行に文を足し、引用 5 行を別の文に置き換える（行数は 200 のまま。行の追加・削除はなく、変わったブロックは `-` と `+` の組になる）
{
  const lines = [];
  let ch = 0;
  while (lines.length < 200) {
    ch++;
    lines.push(`## 第 ${ch} 章 メモの扱い`, "");
    lines.push(`第 ${ch} 章では、メモの保存と一覧の手順を説明します。`, "");
    lines.push(`1. ノート ${ch}-1 を開きます。`, `2. ノート ${ch}-2 に書き込みます。`, `3. 保存を押します。`, "");
    lines.push(`> 注意 ${ch}: 保存の前に内容を確かめます。`, "");
  }
  const before = lines.slice(0, 200);
  const r = rng(20261008);
  const after = before.slice();
  const idx = (pred) => after.map((l, i) => (pred(l) ? i : -1)).filter((i) => i >= 0);
  const para = idx((l) => l.startsWith("第 "));
  for (let k = 0; k < 10; k++) { const i = para[Math.floor(r() * para.length)]; after[i] = after[i].replace("説明します", "説明します（更新）"); }
  const items = idx((l) => /^\d\. ノート/.test(l));
  for (let k = 0; k < 5 && items.length; k++) { const i = items.splice(Math.floor(r() * items.length), 1)[0]; after[i] = after[i] + " 追加の確認をします。"; }
  const quotes = idx((l) => l.startsWith("> 注意"));
  for (let k = 0; k < 5; k++) { const i = quotes.splice(Math.floor(r() * quotes.length), 1)[0]; after[i] = "> 補足: 確認の結果を記録します。"; }
  write("doc-large-before.md", before.join("\n") + "\n");
  write("doc-large-after.md", after.join("\n") + "\n");
}

// ---- 差分（行の数 n。n 行の和集合。after では約 10% が変わる）
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
write("diff-500.json", diffRows(500, 1001));
for (const n of [4095, 4096, 4097]) write(`diff-${n}.json`, diffRows(n, 1000 + n));

// ---- タスク 50 件（深さ 0〜2）
{
  const r = rng(2002);
  const states = ["open", "progress", "done"];
  const rows = [];
  for (let i = 1; i <= 50; i++) {
    const depth = i === 1 ? 0 : Math.min(2, Math.floor(r() * 3));
    const s0 = states[Math.floor(r() * 3)];
    const x = r();
    const label = `#${i} 作業項目 ${i}`;
    const who = r() < 0.5 ? "担当: 受講者" : "担当: AI";
    const id = `t${i}`;
    if (x < 0.06) rows.push({ id, depth, kind: "task", before: null, after: cell(label, who, s0) });
    else if (x < 0.12) rows.push({ id, depth, kind: "task", before: cell(label, who, s0), after: null });
    else if (x < 0.40) rows.push({ id, depth, kind: "task", before: cell(label, who, s0), after: cell(label, who, states[(states.indexOf(s0) + 1) % 3]) });
    else rows.push({ id, depth, kind: "task", before: cell(label, who, s0), after: cell(label, who, s0) });
  }
  write("tasks-50.json", { view: "tasks", title: "generated tasks 50", rows });
}

// ---- ファイルツリー 200 件
{
  const r = rng(3003);
  const rows = [];
  let n = 0;
  for (let d = 1; n < 200; d++) {
    rows.push({ id: `d${d}`, depth: 0, kind: "dir", before: cell(`dir${d}/`), after: cell(`dir${d}/`) }); n++;
    const k = 5 + Math.floor(r() * 6);
    for (let f = 1; f <= k && n < 200; f++) {
      const x = r(); const id = `d${d}/f${f}`; const name = `file${d}_${f}.txt`;
      if (x < 0.05) rows.push({ id, depth: 1, kind: "file", before: null, after: cell(name, "3 行") });
      else if (x < 0.10) rows.push({ id, depth: 1, kind: "file", before: cell(name, "3 行"), after: null });
      else if (x < 0.25) rows.push({ id, depth: 1, kind: "file", before: cell(name, "3 行"), after: cell(name, "5 行") });
      else rows.push({ id, depth: 1, kind: "file", before: cell(name, "3 行"), after: cell(name, "3 行") });
      n++;
    }
  }
  write("filetree-200.json", { view: "filetree", title: "generated filetree 200", rows });
}

// ---- カード 20 件、チェック 20 件
{
  const r = rng(4004);
  const cs = ["draft", "open", "merged", "closed"];
  const rows = [];
  for (let i = 1; i <= 20; i++) {
    const s = cs[Math.floor(r() * 4)];
    rows.push({ id: `pr${i}`, depth: 0, kind: "card", before: cell(`#${i} 変更 ${i}`, `feat/x${i} → main / AI`, s, ["タグ"]), after: cell(`#${i} 変更 ${i}`, `feat/x${i} → main / AI`, r() < 0.4 ? cs[(cs.indexOf(s) + 1) % 4] : s, ["タグ"]) });
  }
  write("cards-20.json", { view: "cards", title: "generated cards 20", rows });
  const ks = ["success", "failure", "running", "pending", "skipped"];
  const rows2 = [];
  for (let i = 1; i <= 20; i++) {
    const s = ks[Math.floor(r() * 5)];
    rows2.push({ id: `c${i}`, depth: 0, kind: "check", before: cell(`check-${i}`, "1 秒", s), after: cell(`check-${i}`, "2 秒", r() < 0.5 ? ks[Math.floor(r() * 5)] : s) });
  }
  write("checks-20.json", { view: "checks", title: "generated checks 20", rows: rows2 });
}
