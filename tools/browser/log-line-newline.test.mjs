// 右のログの行が、送った複数行の改行を見えるまま出すことの試験（#2189。TERM-2・TERM-19 の送った後の見え方）。
//
// 記録（行の本文 `line-body` の文字）には改行が残っていたが、既定の `white-space: normal` で改行が空白に畳まれ、
// 1 行に見えていた。ここでは画面の見え方を固定する: Shift+Enter で書いた 2 行の本文が、1 行の本文の 2 倍近い高さで
// 描かれ、innerText にも改行が残る。本文の要素だけを `normal` に戻した対照で 1 行に戻ることも確かめ、高さの比較が
// 畳まれた状態を見分けることを示す。
//
// 画面: 試験の設定（TEST_HOOKS=1）の `?fixture=ch0-3-chapters&start=s-1-1`。最初の Step は自由な文を断るが、
// 送った文は受付の行（`received`）としてログに出るため、Step を進めずに何度でも送れる。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted } from './harness.mjs';
import { DRAFT, LOG, T, waitCount } from './ch0-3-journey.mjs';

const BASE = '/fandhe-log-line-newline/';
const QUERY = '?fixture=ch0-3-chapters&start=s-1-1';
const BODY = `${LOG} > li[data-kind="received"] [data-testid="line-body"]`;
// 入力欄の下の送信済みの一覧（hello の `log-list`）。同じ文が出るため、同じ見え方を確かめる。
const SENT = '[data-testid="log-list"] > li';
let browser;

before(async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  assert.ok(
    dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(),
    'FANDHE_WEB_TEST_DIR must point to a web directory built with TEST_HOOKS=1',
  );
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 受付の行の本文を、送った順に { text, shown, ws, h } で読む。
const bodies = (page, sel = BODY) => page.$$eval(sel, (els) => els.map((b) => ({
  text: b.textContent,
  shown: b.innerText,
  ws: getComputedStyle(b).whiteSpace,
  h: b.getBoundingClientRect().height,
})));

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;
  test(`複数行の入力が右のログで改行のまま見える（${mode}）`, async () => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const { page, context } = await openPage(browser, { origin: srv.origin });
      await page.goto(`${srv.url}${QUERY}`);
      await waitMounted(page);
      const input = page.locator(DRAFT);

      await input.click();
      await page.keyboard.type('一行目のメモ');
      await page.keyboard.press('Enter');
      await waitCount(page, BODY, 1);

      await input.fill('');
      await page.keyboard.type('一行目のメモ');
      await page.keyboard.press('Shift+Enter');
      await page.keyboard.type('二行目のメモ');
      await page.keyboard.press('Enter');
      await waitCount(page, BODY, 2);

      const [one, two] = await bodies(page);
      assert.equal(two.text, '一行目のメモ\n二行目のメモ', '記録の文字に改行が残る');
      assert.equal(two.shown, '一行目のメモ\n二行目のメモ', '見える文字にも改行が残る');
      assert.equal(two.ws, 'pre-wrap');
      assert.ok(two.h >= one.h * 1.8, `2 行の本文は 1 行の約 2 倍の高さ: ${two.h} / ${one.h}`);

      const sent = await bodies(page, SENT);
      assert.equal(sent.length, 2, '送信済みの一覧に 2 件');
      assert.equal(sent[1].shown, '一行目のメモ\n二行目のメモ', '送信済みの一覧でも改行が見える');
      assert.ok(sent[1].h >= sent[0].h * 1.8, `送信済みの一覧の 2 行の高さ: ${sent[1].h} / ${sent[0].h}`);

      // 対照: 本文を既定の normal に戻すと 1 行に畳まれ、上の高さの比較で見分けられる。
      await page.$$eval(`${BODY}, ${SENT}`, (els) => { for (const b of els) b.style.whiteSpace = 'normal'; });
      const [oneN, twoN] = await bodies(page);
      assert.equal(twoN.shown, '一行目のメモ 二行目のメモ', 'normal では改行が空白に畳まれる');
      assert.ok(twoN.h < oneN.h * 1.5, `normal では 1 行の高さ: ${twoN.h} / ${oneN.h}`);
      const sentN = await bodies(page, SENT);
      assert.ok(sentN[1].h < sentN[0].h * 1.5, `normal では送信済みの一覧も 1 行: ${sentN[1].h} / ${sentN[0].h}`);

      await context.close();
    }));
  }, { timeout: T * 2 });
}
