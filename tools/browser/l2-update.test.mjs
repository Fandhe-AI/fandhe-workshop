// 更新した L2 の配布物（tools/update-l2.mjs の出力）を、Rust のない模擬環境で展開・起動して開く試験
// （TASK-116。D-58・DLV-18。`make test-l2-run` から呼ばれる。`make test-browser` には入れない）。
//
// 目的: 「site/ の差し替え 1 操作」で更新した配布物でも、受講者の操作（展開・./start.sh・URL を開く）だけで
// 新しい教材が動くことを、実物の miniserve で確かめる（合成の実行ファイルでの確認は tools/update-l2.test.mjs）。
// 確かめること:
//   (1) 更新後の zip を PATH=/usr/bin:/bin だけの環境で展開して ./start.sh で起動でき、/ir/main.json が新しい IR とバイト単位で一致する
//   (2) 対照: 旧 zip を起動すると、/ir/main.json は新しい IR と一致せず旧い IR と一致する（差し替えが効いていることの確認）
//   (3) 更新後の start.sh・miniserve が旧 zip のものとバイト単位で一致する（site/ 以外を引き継いでいる）
//   (4) 自動ブラウザで開くと html[data-mounted="true"] になり、ページ内エラー・CSP の違反・コンソールの error と warning・
//       2xx 以外の応答・配信元以外への要求が 0 件
// 完走（CH-0〜CH-1 の操作）は流さない。旧 zip での完走は l2-run が確かめている。
//
// 入力（環境変数。未設定・不在は skip にせず失敗にする）:
//   FANDHE_L2_ZIP          更新前の zip（scripts/test-l2-run.sh が作る）
//   FANDHE_L2_UPDATED_ZIP  tools/update-l2.mjs が作った更新後の zip
//   FANDHE_L2_IR           旧 zip に同梱した IR の JSON
//   FANDHE_L2_IR_B         更新で入れた新しい IR の JSON（旧と内容が異なること）
// 出力に入力した値は載せない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { launchBrowser, channelFromEnv, openPage, waitMounted, summarize, cspConsoleMessages } from './harness.mjs';
import { startL2, stopL2, unzipTo } from './l2-server.mjs';

let browser;
let zipOld;
let zipNew;
let irA;
let irB;

function file(name) {
  const f = process.env[name];
  assert.ok(f && fs.statSync(f, { throwIfNoEntry: false })?.isFile(), `${name} must point to an existing file`);
  return f;
}

before(async () => {
  zipOld = file('FANDHE_L2_ZIP');
  zipNew = file('FANDHE_L2_UPDATED_ZIP');
  irA = fs.readFileSync(file('FANDHE_L2_IR'));
  irB = fs.readFileSync(file('FANDHE_L2_IR_B'));
  assert.ok(!irA.equals(irB), 'the two IRs must differ, or the update would be invisible');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

async function fetchIr(url) {
  const res = await fetch(`${url}ir/main.json`);
  assert.equal(res.status, 200);
  return Buffer.from(await res.arrayBuffer());
}

test('(3) 更新後の start.sh と miniserve は旧 zip のものとバイト単位で一致する', () => {
  const w1 = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-l2-update-'));
  const w2 = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-l2-update-'));
  try {
    const a = unzipTo(zipOld, w1);
    const b = unzipTo(zipNew, w2);
    for (const f of ['start.sh', 'miniserve']) {
      assert.ok(fs.readFileSync(path.join(a, f)).equals(fs.readFileSync(path.join(b, f))), f);
      assert.equal(fs.statSync(path.join(a, f)).mode & 0o777, fs.statSync(path.join(b, f)).mode & 0o777, `${f} mode`);
    }
  } finally {
    fs.rmSync(w1, { recursive: true, force: true });
    fs.rmSync(w2, { recursive: true, force: true });
  }
});

test('(2) 対照: 旧 zip を起動すると /ir/main.json は旧い IR で、新しい IR とは一致しない', async () => {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-l2-update-'));
  let run = null;
  try {
    run = await startL2(zipOld, work);
    const got = await fetchIr(run.url);
    assert.ok(got.equals(irA), 'old zip serves the old IR');
    assert.ok(!got.equals(irB), 'old zip must not serve the new IR');
  } finally {
    await stopL2(run);
    fs.rmSync(work, { recursive: true, force: true });
  }
});

test('(1)(4) 更新後の zip を展開・./start.sh・URL を開くだけで新しい IR が配られ、画面がマウントされる', async () => {
  const work = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-l2-update-'));
  let run = null;
  let context = null;
  try {
    run = await startL2(zipNew, work);
    assert.ok(run.out.includes(run.url), 'start.sh prints the URL');
    const got = await fetchIr(run.url);
    assert.ok(got.equals(irB), 'updated zip serves the new IR byte for byte');
    assert.ok(!got.equals(irA), 'updated zip must not serve the old IR');

    const origin = `http://127.0.0.1:${run.port}`;
    const opened = await openPage(browser, { origin });
    context = opened.context;
    const { page, rec } = opened;
    await page.goto(run.url);
    await waitMounted(page);
    const c = summarize(rec);
    // 200 で届いた IR の取得が後から net::ERR_ABORTED と通知される競合だけを除く（l1-l2-origins と同じ。docs/testing.md 5.268）。
    const raced = (f) => f.error === 'net::ERR_ABORTED' && /\/ir\/main\.json$/.test(new URL(f.url).pathname)
      && rec.requests.some((r) => r.id === f.id && r.url === f.url && r.status === 200);
    assert.deepEqual(
      { pageErrors: c.pageErrors, csp: c.csp, consoleErrors: c.consoleErrors, external: c.external, dialogs: c.dialogs, popups: c.popups, crashed: c.crashed },
      { pageErrors: 0, csp: 0, consoleErrors: 0, external: 0, dialogs: 0, popups: 0, crashed: 0 },
    );
    assert.equal(rec.consoleWarnings.length, 0);
    assert.equal(cspConsoleMessages(rec).length, 0);
    assert.deepEqual(rec.failed.filter((f) => !raced(f)).map((f) => f.error), []);
    assert.ok(rec.requests.every((r) => r.status >= 200 && r.status < 300), 'no non-2xx response');
    console.log(`L2 更新後の起動: 要求 ${c.requests} 件・起動 ${run.startMs.toFixed(0)} ms`);
  } finally {
    if (context) await context.close().catch(() => {});
    await stopL2(run);
    fs.rmSync(work, { recursive: true, force: true });
  }
});
