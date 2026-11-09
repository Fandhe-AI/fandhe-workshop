// tab-focus.mjs（Tab の出入りの観察の共通の部品）の自己確認（TASK-373）。
//
// 固定の HTML を page.setContent で置くだけなので、サーバもビルドも要らない。
// 列挙の選び方（隠れた要素・disabled・tabindex=-1 を除く）と、押下ごとの位置・defaultPrevented の記録が正しいことを確かめる。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { launchBrowser, channelFromEnv } from './harness.mjs';
import {
  listFocusables, focusIndex, activeTestId, installTabProbe, pressTracked, focusAt,
} from './tab-focus.mjs';

const HTML = `<!doctype html><body>
<button data-testid="a">a</button>
<textarea data-testid="b"></textarea>
<button data-testid="hidden" style="display:none">h</button>
<button data-testid="off" disabled>d</button>
<button data-testid="skip" tabindex="-1">s</button>
<button data-testid="c">c</button></body>`;

let browser;
before(async () => { browser = await launchBrowser({ channel: channelFromEnv() }); });
after(async () => { if (browser) await browser.close(); });

async function fresh() {
  const page = await browser.newPage();
  await page.setContent(HTML);
  await installTabProbe(page);
  return page;
}

test('列挙は見える・有効・tabindex が 0 以上の要素を DOM の順に返す', async () => {
  const page = await fresh();
  assert.deepEqual(await listFocusables(page), { ids: ['a', 'b', 'c'], positive: 0 });
  await page.close();
});

test('押す前後の位置と defaultPrevented が記録され、末尾の次は OUT になる', async () => {
  const page = await fresh();
  await listFocusables(page);
  await focusAt(page, 0);
  assert.equal(await activeTestId(page), 'a');
  assert.deepEqual(await pressTracked(page, 'Tab'), { before: 0, after: 1, prevented: [false] });
  await focusAt(page, 2);
  const out = await pressTracked(page, 'Tab');
  assert.equal(out.before, 2);
  assert.equal(await focusIndex(page), out.after);
  await page.close();
});

test('preventDefault する listener を足すと prevented: true が記録される', async () => {
  const page = await fresh();
  await listFocusables(page);
  await page.evaluate(() => {
    document.querySelector('[data-testid="a"]').addEventListener('keydown', (e) => e.preventDefault());
  });
  await focusAt(page, 0);
  assert.deepEqual(await pressTracked(page, 'Tab'), { before: 0, after: 0, prevented: [true] });
  await page.close();
});
