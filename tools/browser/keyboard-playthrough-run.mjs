// 教材の通しをキーボードだけで流す入口（TASK-92 2/2。BUILD-17）。
//
// 役割: 配信する設定の配信物（IR 同梱。試験用の口なし）を開き、計数（installInputCounter・guardPage）つきで keyboard-playthrough.mjs の
// runPlaythrough を流し、項目の名前と合否・件数だけを返す。呼び出し元は scripts/check-keyboard-playthrough.sh（CLI として）と
// keyboard-playthrough.test.mjs（runKeyboardPlaythrough を import）。教材には依存しない。
//
// 環境変数（CLI）: FANDHE_WEB_PACK_DIR（IR 同梱の配信物）・FANDHE_PACK_IR（compile した IR）・FANDHE_PLAYTHROUGH_JSON
// （playthrough_emit.rs の出力）・FANDHE_BROWSER_CHANNEL（任意）。未設定・不在は失敗にする。幅は 1280px、CSP あり。
// 終了コード: 0 = 合格 / 1 = 不一致・未完走 / 2 = 誤用・入力不足。出力は項目の名前・件数・ID・合否だけで、入力値と本文は出さない。
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, cspConsoleMessages,
} from './harness.mjs';
import {
  installInputCounter, guardPage, keyOnlyDriver, ALLOWED_KEYS,
} from './keyboard-only.mjs';
import { runPlaythrough, buildIndex } from './keyboard-playthrough.mjs';

const BASE = '/fandhe-keyboard-playthrough/';

// 通しを流して項目を返す。start が IR の既定の開始点と違うときは `?start=` で開く（ナビゲーションであり click ではない）。
// requiredKeys は、通しが押すはずのキー（指定のキーが 1 回以上押されたことを確かめる）。
export async function runKeyboardPlaythrough({ browser, webDir, ir, playthrough, csp = true, requiredKeys = ['Tab', 'Enter'] }) {
  const items = [];
  const check = (name, ok) => items.push({ name, ok: ok === true });
  let result;
  await withCspMode({ dir: webDir, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 900 } });
    const counter = await installInputCounter(context);
    const first = playthrough.segments[0].start;
    const idx = buildIndex(ir);
    const query = first === ir.starts[0]?.id || !idx.startNode.has(first) ? '' : `?start=${encodeURIComponent(first)}`;
    await page.goto(`${srv.url}${query}`);
    await waitMounted(page);
    const g = guardPage(page);
    result = await runPlaythrough({ page: g.page, driver: keyOnlyDriver(g.page), ir, playthrough });
    const c = await counter.read(page);
    check('全操作の期待が一致し、終わりの条件が成り立つ', result.complete && result.mismatches.length === 0);
    check('次の章へ進むを区間の数 - 1 回だけ押した', result.nextPresses === playthrough.segments.length - 1);
    check('ページ内のマウスのイベントが 0', c.mouse === 0);
    check('許さないキーの keydown が 0', c.forbidden.length === 0);
    check('ドライバ側の監視が 0', g.log.calls === 0);
    check('押したキーが許す集合の中だけ', Object.keys(c.keys).every((k) => ALLOWED_KEYS.includes(k)));
    for (const k of requiredKeys) check(`ページ内で ${k} が 1 回以上`, (c.keys[k] ?? 0) >= 1);
    check('ページ内エラーが 0', rec.pageErrors.length === 0);
    check('CSP の違反・警告が 0', rec.csp.length === 0 && cspConsoleMessages(rec).length === 0);
    check('外部通信が 0', rec.external.length === 0);
    const counts = await collectSafety(page, rec);
    check('安全の計数が 0', nonZero(counts).length === 0);
    result.keys = c.keys;
    result.driverKeys = { ...g.keyboard.used };
    await context.close();
  }));
  return { result, items };
}

function report({ result, items }) {
  const lines = [];
  lines.push(`区間 ${result.segments}・操作 ${result.ops}（一致 ${result.matched}）・次の章へ進む ${result.nextPresses} 回`);
  lines.push(`キー（ページ内の計数）: ${JSON.stringify(result.keys)}`);
  lines.push(`asserts の未判定: ${result.unjudgedAsserts} 件（判定しない。合格に数えない）`);
  for (const m of result.mismatches) lines.push(`不一致: 区間 ${m.seg}・操作 ${m.op}・期待 ${m.expected}・観察 ${m.observed}`);
  for (const i of items) lines.push(`${i.ok ? 'PASS' : 'FAIL'} ${i.name}`);
  return lines.join('\n');
}

async function main() {
  const need = (name) => {
    const v = process.env[name];
    if (!v || !fs.existsSync(v)) {
      process.stderr.write(`error: ${name} must point to an existing path\n`);
      process.exit(2);
    }
    return v;
  };
  const webDir = need('FANDHE_WEB_PACK_DIR');
  const ir = JSON.parse(fs.readFileSync(need('FANDHE_PACK_IR'), 'utf8'));
  const playthrough = JSON.parse(fs.readFileSync(need('FANDHE_PLAYTHROUGH_JSON'), 'utf8'));
  if (!Array.isArray(playthrough.segments) || playthrough.segments.length === 0) {
    process.stderr.write('error: playthrough has no segments\n');
    process.exit(2);
  }
  const browser = await launchBrowser({ channel: channelFromEnv() });
  try {
    const out = await runKeyboardPlaythrough({ browser, webDir, ir, playthrough });
    process.stdout.write(`${report(out)}\n`);
    process.exit(out.items.every((i) => i.ok) ? 0 : 1);
  } finally {
    await browser.close();
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();
