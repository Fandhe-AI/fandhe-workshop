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
  installValueWriteProbe, readValueWriteProbe, clearValueWriteProbe,
  MIS_SEND_MARK, misSendCount, missedCount, stopCount, cancelLogCount, assertNoMisSend,
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

test(`書き込みの器は変換中の value の書き込みを composing に、変換していないときを idle に数える（${SIM_LABEL}）`, async () => {
  const context = await browser.newContext();
  await installValueWriteProbe(context, T);
  const page = await context.newPage();
  await page.goto('data:text/html,<textarea id=t></textarea>');
  await page.focus(T);
  const cdp = await openIme(page);
  assert.deepEqual(await readValueWriteProbe(page), { composing: 0, idle: 0 }, SIM_LABEL);
  await page.evaluate(() => { document.querySelector('#t').value = 'x'; });
  assert.deepEqual(await readValueWriteProbe(page), { composing: 0, idle: 1 }, SIM_LABEL);
  await clearValueWriteProbe(page);
  await imeCompose(cdp, 'へん');
  await page.evaluate(() => { document.querySelector('#t').value = 'y'; });
  assert.deepEqual(await readValueWriteProbe(page), { composing: 1, idle: 0 }, SIM_LABEL);
  await imeCancel(cdp);
  await context.close();
});

test(`書き込みの器は対象の selector に合わない要素の書き込みを数えない（${SIM_LABEL}）`, async () => {
  const context = await browser.newContext();
  await installValueWriteProbe(context, T);
  const page = await context.newPage();
  await page.goto('data:text/html,<textarea id=t></textarea><textarea id=u></textarea>');
  await page.evaluate(() => { document.querySelector('#u').value = 'z'; });
  assert.deepEqual(await readValueWriteProbe(page), { composing: 0, idle: 0 }, SIM_LABEL);
  await context.close();
});

// 数え方の自己確認（ime-sentences・ime-screen が共有する関数。0 件が空振りでなく、1 件を数えられること）。
test('数え方は誤送信・送信漏れ・停止・取り消しの記録を 1 件と数え、正常なら 0 を数える', () => {
  assert.equal(misSendCount(0), 0);
  assert.equal(misSendCount(1), 1);
  assert.equal(missedCount(1, 'abc', 'abc'), 0);
  assert.equal(missedCount(1, 'abd', 'abc'), 1);
  assert.equal(missedCount(0, 'abc', 'abc'), 1);
  assert.equal(stopCount({ generation: 1, runStatus: 'busy' }, { generation: 1, runStatus: 'busy' }), 0);
  assert.equal(stopCount({ generation: 1, runStatus: 'busy' }, { generation: 2, runStatus: 'busy' }), 1);
  assert.equal(stopCount({ generation: 1, runStatus: 'busy' }, { generation: 1, runStatus: 'stopped' }), 1);
  assert.equal(cancelLogCount(3, 3), 0);
  assert.equal(cancelLogCount(3, 4), 1);
});

test('誤送信の表明は印と件数を失敗の文言に出す', () => {
  assertNoMisSend(0, 'x');
  assert.throws(() => assertNoMisSend(1, 'x'), (e) => e.message.includes(`${MIS_SEND_MARK} 誤送信 1 件`));
});
