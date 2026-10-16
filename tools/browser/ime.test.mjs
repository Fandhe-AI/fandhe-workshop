// CDP による IME の模擬のヘルパ（ime.mjs）の自己確認（TASK-86 1/2。対照）。
//
// ime-input.test.mjs の「誤送信 0 件」が、合成が起きていなかったための空振りでないことを示す。
// wasm は要らない。最小の <textarea> だけの画面で、合成・確定・取り消し・変換中のキーが
// ブラウザに期待どおりのイベントとして届くことを確かめる。シミュレーターの確認であり、実 IME ではない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { launchBrowser, channelFromEnv } from './harness.mjs';
import {
  SIM_LABEL, loadSentences, openIme, assertFocused, imeCompose, imeCommit, imeCancel, imeEnterKey,
  installKeyProbe, readKeyProbe, clearKeyProbe,
} from './ime.mjs';

const T = '#t';
let browser;

before(async () => {
  browser = await launchBrowser({ channel: channelFromEnv() });
});
after(async () => {
  if (browser) await browser.close();
});

async function setup() {
  const context = await browser.newContext();
  await installKeyProbe(context);
  const page = await context.newPage();
  await page.goto('data:text/html,<textarea id=t></textarea>'); // 初期化スクリプトは遷移で走るため setContent は使わない
  await page.focus(T);
  return { context, page, cdp: await openIme(page) };
}

test(`合成で compositionstart・compositionupdate が出て、値が変換中の文字列になる（${SIM_LABEL}）`, async () => {
  const { context, page, cdp } = await setup();
  await imeCompose(cdp, 'きょう');
  await imeCompose(cdp, '今日');
  const types = (await readKeyProbe(page)).map((r) => r.type);
  assert.deepEqual(types, ['compositionstart', 'compositionupdate', 'compositionupdate'], SIM_LABEL);
  assert.equal(await page.inputValue(T), '今日', SIM_LABEL);
  await context.close();
});

test(`変換中の Enter は isComposing=true・keyCode=229 で届き、確定後の Enter は false・13 になる（${SIM_LABEL}）`, async () => {
  const { context, page, cdp } = await setup();
  await imeCompose(cdp, '今日');
  await clearKeyProbe(page);
  await imeEnterKey(cdp);
  const kd = (await readKeyProbe(page)).filter((r) => r.type === 'keydown');
  assert.equal(kd.length, 1, SIM_LABEL);
  assert.equal(kd[0].isComposing, true, SIM_LABEL);
  assert.equal(kd[0].keyCode, 229, SIM_LABEL);
  await imeCommit(cdp, '今日');
  const all = await readKeyProbe(page);
  assert.ok(all.some((r) => r.type === 'compositionend'), SIM_LABEL);
  assert.equal(await page.inputValue(T), '今日', SIM_LABEL);
  await clearKeyProbe(page);
  await page.keyboard.press('Enter');
  const after = (await readKeyProbe(page)).filter((r) => r.type === 'keydown');
  assert.equal(after.length, 1, SIM_LABEL);
  assert.equal(after[0].isComposing, false, SIM_LABEL);
  assert.equal(after[0].keyCode, 13, SIM_LABEL);
  await context.close();
});

test(`取り消しで値が合成前に戻る（${SIM_LABEL}）`, async () => {
  const { context, page, cdp } = await setup();
  await page.fill(T, 'abc');
  await page.focus(T);
  await imeCompose(cdp, 'ほげ');
  assert.equal(await page.inputValue(T), 'abcほげ', SIM_LABEL);
  await imeCancel(cdp);
  assert.equal(await page.inputValue(T), 'abc', SIM_LABEL);
  await context.close();
});

test('assertFocused は未フォーカスで例外を投げる（fail-closed）', async () => {
  const { context, page } = await setup();
  await assertFocused(page, T);
  await page.evaluate(() => document.activeElement.blur());
  await assert.rejects(() => assertFocused(page, T), /not focused/);
  await context.close();
});

test('loadSentences は 30 文を返し、kind の内訳が仕様どおりである', () => {
  const rows = loadSentences();
  assert.equal(rows.length, 30);
  const by = {};
  for (const r of rows) by[r.kind] = (by[r.kind] ?? 0) + 1;
  assert.deepEqual(by, { kanji: 10, mixed: 5, symbol: 5, candidate: 5, cancel: 3, multi: 2 });
});
