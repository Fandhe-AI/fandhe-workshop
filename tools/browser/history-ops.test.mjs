// 履歴の書き出しと読み込みの操作を枠に置く試験（TASK-531 1/2。SHELL-15 の自動ブラウザの部分）。
//
// 入力: FANDHE_WEB_TEST_DIR（試験の設定）と FANDHE_WEB_DIR（配信する設定）。未設定・不在は skip にせず失敗にする。
// 方針: 試験用の口（exportClick・importInstall）を使わず、画面の操作だけで書き出しと読み込みを通す足場を作る。
//   書き出しは「履歴を書き出す」ボタン → 現れたリンクのクリックを Playwright の `Download` で受ける。
//   読み込みは `shell-import` の中の input[type=file] へ `setInputFiles` で渡す（OS のファイル画面は使わない）。
// 確かめること: (1) 書き出しのリンクで履歴のファイルが得られ、JSON として読める。もう一度押すとリンクが新しい URL で作り直され、href がその URL と一致する。(2) 別のオリジンでそのファイルを選ぶと開く。
//   (3) ファイル画面を開く（選ばずに閉じる）クリックで読み込みの結果・通知が変わらない（空の本文が届かない）。
//   (4) 書き換えたファイル・空のファイルは開かず、状態を変えず、通知が出る。(5) 外部への要求・CSP 違反・ページのエラー・ダイアログ・
//   ポップアップが 0 件で、URL に状態が載らない。(6) 配信する設定（試験用の口なし）でも、画面の操作だけで書き出しの文が出る。
// 限界: 同じ状態の詳しい照合と書き出しのバイト単位の一致は #724、macOS のファイル画面は人の確認（TASK-342）が持つ。Chromium だけ。
// 試験の出力・失敗の文に、履歴の中身を出さない（真偽・件数・固定のコードだけを比べる）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { launchBrowser, channelFromEnv, withServer, openPage, waitMounted, summarize } from './harness.mjs';

const BASE = '/fandhe-ops/';
const EXPORT_BTN = '[data-testid="shell-export"]';
const LINK = '[data-testid="export-download"]';
const STATUS = '[data-testid="export-status"]';
const FILE_INPUT = '[data-testid="shell-import"] input[type="file"]';
const NOTICE_ITEM = '[data-testid="shell-notice-item"]';
let browser;
let tmpDir;

before(async () => {
  for (const name of ['FANDHE_WEB_TEST_DIR', 'FANDHE_WEB_DIR']) {
    const dir = process.env[name];
    assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to a built web directory`);
  }
  browser = await launchBrowser({ channel: channelFromEnv() });
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-history-ops-'));
});

after(async () => {
  if (browser) await browser.close();
  if (tmpDir) fs.rmSync(tmpDir, { recursive: true, force: true });
});

const call = (page, fn, ...args) => page.evaluate(([f, a]) => window.__fandheTest[f](...a), [fn, args]);
const engine = async (page) => JSON.parse(await call(page, 'stateJson')).engine;
const urlState = (page) => page.evaluate(() => ({ href: location.href, hash: location.hash, length: history.length, state: history.state }));

async function openOrigin(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  return opened;
}

async function waitImport(page, verdict) {
  await page.waitForFunction(
    (v) => {
      const e = JSON.parse(window.__fandheTest.stateJson()).engine;
      return e && e.lastImport && e.lastImport.verdict === v;
    },
    verdict,
    { timeout: 10000 },
  );
}

// 進行が止まる（予約済みの宛先が無い）まで待つ。
async function settle(page) {
  await page.waitForFunction(() => window.__fandheTest.nextTick() === 'null', null, { timeout: 15000 });
}

function assertQuiet(rec, label) {
  const s = summarize(rec);
  assert.equal(s.external, 0, `${label}: no external request`);
  assert.equal(s.csp, 0, `${label}: no CSP violation`);
  assert.equal(s.pageErrors, 0, `${label}: no page error`);
  assert.equal(s.dialogs, 0, `${label}: no dialog`);
  assert.equal(s.popups, 0, `${label}: no popup`);
}

// 画面の操作だけで書き出し、保存したファイルの経路を返す。
async function exportByUi(page) {
  await page.locator(EXPORT_BTN).click();
  const link = page.locator(LINK);
  await link.waitFor({ state: 'visible', timeout: 10000 });
  const [download] = await Promise.all([page.waitForEvent('download'), link.click()]);
  const file = path.join(tmpDir, `${Date.now()}-${Math.random().toString(16).slice(2)}.json`);
  await download.saveAs(file);
  return { file, name: download.suggestedFilename() };
}

test('書き出しのリンクで履歴のファイルが得られ、別のオリジンで選ぶと開く。URL に状態は載らない', async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  await withServer({ dir, base: BASE }, (srvA) =>
    withServer({ dir, base: BASE }, async (srvB) => {
      assert.notEqual(srvA.origin, srvB.origin, 'two distinct origins');
      const A = await openOrigin(srvA);
      const B = await openOrigin(srvB);

      // 空の画面では押してもリンクは出ず、固定の文が出る。
      await A.page.locator(EXPORT_BTN).click();
      await A.page.locator(STATUS).waitFor({ state: 'visible', timeout: 10000 });
      assert.equal(await A.page.locator(LINK).count(), 0, 'no link without a history');

      // 履歴を作る（受講中の記録の代わりに、契約の試料から作った開ける包みを開く）。
      const text = await call(A.page, 'loadableHistoryText');
      assert.ok(text.length > 0, 'loadable history exists');
      assert.equal(await call(A.page, 'importText', text), true);
      await waitImport(A.page, 'open');
      await settle(A.page);
      const urlBefore = await urlState(A.page);

      const { file, name } = await exportByUi(A.page);
      assert.match(name, /^fandhe-workshop-history(-[a-z0-9-]+)?\.json$/, 'file name has the fixed shape');
      const size = fs.statSync(file).size;
      assert.ok(size > 0, 'downloaded file is not empty');
      const parsed = JSON.parse(fs.readFileSync(file, 'utf8'));
      assert.ok(Array.isArray(parsed.ops), 'file has ops');
      assert.deepEqual(await urlState(A.page), urlBefore, 'URL does not carry the state');

      // もう一度押すと、リンクは新しい URL で作り直され（前の URL は破棄済み）、`app.js` が写した href は新しい URL と一致する。
      // 写した href が view の `data-export-url` と乖離しないこと（同じ要素が残って古い href を指し続けないこと）を見る。
      const linkUrls = (page) =>
        page.evaluate((sel) => [...document.querySelectorAll(sel)].map((a) => [a.getAttribute('data-export-url'), a.getAttribute('href')]), LINK);
      const [[firstUrl, firstHref]] = await linkUrls(A.page);
      assert.equal(firstHref, firstUrl, 'first link: href equals data-export-url');
      await A.page.locator(EXPORT_BTN).click();
      await A.page.waitForFunction(
        ([sel, prev]) => {
          const a = document.querySelector(sel);
          return a !== null && a.getAttribute('data-export-url') !== prev && a.getAttribute('href') === a.getAttribute('data-export-url');
        },
        [LINK, firstUrl],
        { timeout: 10000 },
      );
      const second = await linkUrls(A.page);
      assert.equal(second.length, 1, 'one link after the second export');
      assert.notEqual(second[0][0], firstUrl, 'second link has a new URL');
      assert.equal(second[0][1], second[0][0], 'second link: href equals data-export-url');
      const [download2] = await Promise.all([A.page.waitForEvent('download'), A.page.locator(LINK).click()]);
      const file2 = path.join(tmpDir, `${Date.now()}-second.json`);
      await download2.saveAs(file2);
      assert.equal(JSON.parse(fs.readFileSync(file2, 'utf8')).ops.length, parsed.ops.length, 'second download has the same ops length');
      assertQuiet(A.rec, 'origin A');

      // B: 画面のファイル入力へ渡す。
      assert.equal((await engine(B.page)).lastImport ?? null, null, 'B has not imported');
      await B.page.setInputFiles(FILE_INPUT, file);
      await waitImport(B.page, 'open');
      await settle(B.page);
      const a = await engine(A.page);
      const b = await engine(B.page);
      assert.equal(b.logLen, a.logLen, 'B reaches the same log length');
      assert.equal(JSON.stringify(b.stateWithoutLog), JSON.stringify(a.stateWithoutLog), 'B reaches the same state');
      assert.equal(await B.page.locator(NOTICE_ITEM).count(), 0, 'no notice for a loadable file');
      assertQuiet(B.rec, 'origin B');
      await A.context.close();
      await B.context.close();
    }),
  );
});

test('ファイル画面を開いて選ばずに閉じても、読み込みの結果・通知は変わらない（空の本文が届かない）', async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  await withServer({ dir, base: BASE }, async (srv) => {
    const P = await openOrigin(srv);
    const before = JSON.stringify((await engine(P.page)).lastImport ?? null);
    const noticesBefore = await P.page.locator(NOTICE_ITEM).count();

    // label のクリック（ファイル画面が開く）。選ばずに閉じる。
    const [chooser] = await Promise.all([
      P.page.waitForEvent('filechooser'),
      P.page.locator('[data-testid="shell-import"] label').click(),
    ]);
    await chooser.setFiles([]);
    // input の直接のクリックでも同じ。
    const [chooser2] = await Promise.all([P.page.waitForEvent('filechooser'), P.page.locator(FILE_INPUT).click()]);
    await chooser2.setFiles([]);
    // 配送は set_timeout(0) を経るため、届くとしても届くだけの時間を置く。
    await P.page.waitForTimeout(300);

    assert.equal(JSON.stringify((await engine(P.page)).lastImport ?? null), before, 'lastImport unchanged');
    assert.equal(await P.page.locator(NOTICE_ITEM).count(), noticesBefore, 'no notice appears');
    assertQuiet(P.rec, 'file chooser cancel');
    await P.context.close();
  });
});

test('書き換えたファイルと空のファイルは開かず、状態を変えず、通知が出る', async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  await withServer({ dir, base: BASE }, async (srv) => {
    const P = await openOrigin(srv);
    const text = await call(P.page, 'loadableHistoryText');
    assert.equal(await call(P.page, 'importText', text), true);
    await waitImport(P.page, 'open');
    await settle(P.page);
    const { file } = await exportByUi(P.page);
    const stateBefore = JSON.stringify((await engine(P.page)).stateWithoutLog);
    const logBefore = (await engine(P.page)).logLen;

    const empty = path.join(tmpDir, 'empty.json');
    fs.writeFileSync(empty, '');
    const broken = path.join(tmpDir, 'broken.json');
    fs.writeFileSync(broken, '{not json');
    const tampered = path.join(tmpDir, 'tampered.json');
    const obj = JSON.parse(fs.readFileSync(file, 'utf8'));
    obj.snapshot = { tampered: true };
    fs.writeFileSync(tampered, JSON.stringify(obj));

    // 文字列の中に不正な UTF-8（0xFF）を 1 バイト入れた、元は開ける履歴。置換されると正常な JSON になる形（SAFE。入力の検証）。
    const badUtf8 = path.join(tmpDir, 'bad-utf8.json');
    const raw = fs.readFileSync(file);
    const marker = Buffer.from('"ops"');
    const at = raw.indexOf(marker);
    assert.ok(at > 0, 'file has an ops key to anchor the corruption');
    fs.writeFileSync(badUtf8, Buffer.concat([raw.subarray(0, at), Buffer.from([0x22, 0x78, 0xff, 0x22, 0x3a, 0x30, 0x2c]), raw.subarray(at)]));

    for (const [label, f] of [['empty', empty], ['broken', broken], ['tampered', tampered], ['bad-utf8', badUtf8]]) {
      await P.page.setInputFiles(FILE_INPUT, f);
      await P.page.locator(NOTICE_ITEM).first().waitFor({ state: 'visible', timeout: 10000 });
      const e = await engine(P.page);
      assert.ok(e.lastImport && e.lastImport.verdict !== 'open', `${label}: not opened`);
      if (label === 'bad-utf8') assert.equal(e.lastImport.reason, 'not_json', 'invalid UTF-8 is refused as not_json');
      assert.equal(JSON.stringify(e.stateWithoutLog), stateBefore, `${label}: state unchanged`);
      assert.equal(e.logLen, logBefore, `${label}: log unchanged`);
    }
    assertQuiet(P.rec, 'refused files');
    await P.context.close();
  });
});

// arrayBuffer() の完了を操作する。大きいファイル（200 バイト超）は gate を解くまで待たせ、reject なら全部拒否する。
async function patchRead(page) {
  await page.evaluate(() => {
    const orig = Blob.prototype.arrayBuffer;
    window.__readGate = { release: null, reject: false };
    Blob.prototype.arrayBuffer = async function () {
      if (window.__readGate.reject) throw new Error('forced read failure');
      const buf = await orig.call(this);
      if (this.size > 200) await new Promise((r) => (window.__readGate.release = r));
      return buf;
    };
  });
}

test('先に選んだファイルの読み取りが後から終わっても、後に選んだファイルの結果を差し替えない', async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  await withServer({ dir, base: BASE }, async (srv) => {
    const P = await openOrigin(srv);
    const text = await call(P.page, 'loadableHistoryText');
    assert.equal(await call(P.page, 'importText', text), true);
    await waitImport(P.page, 'open');
    await settle(P.page);
    const { file } = await exportByUi(P.page);
    const broken = path.join(tmpDir, 'order-broken.json');
    fs.writeFileSync(broken, '{not json');
    assert.ok(fs.statSync(file).size > 200, 'the good file is larger than the gate threshold');
    await patchRead(P.page);
    const stateBefore = JSON.stringify((await engine(P.page)).stateWithoutLog);

    await P.page.setInputFiles(FILE_INPUT, file); // 読み取りが止まる
    await P.page.setInputFiles(FILE_INPUT, broken); // 先に終わり、断られる
    await P.page.locator(NOTICE_ITEM).first().waitFor({ state: 'visible', timeout: 10000 });
    await P.page.waitForFunction(() => window.__readGate.release !== null, null, { timeout: 10000 });
    await P.page.evaluate(() => window.__readGate.release());
    await P.page.waitForTimeout(500);

    const e = await engine(P.page);
    assert.notEqual(e.lastImport.verdict, 'open', 'the stale selection is not applied');
    assert.equal(JSON.stringify(e.stateWithoutLog), stateBefore, 'state unchanged');
    assert.equal(await P.page.locator(NOTICE_ITEM).count(), 1, 'the notice of the latest selection stays');
    assertQuiet(P.rec, 'stale read');
    await P.context.close();
  });
});

test('ファイルの読み取りに失敗すると、状態と保存先を変えず通知が 1 件出る', async () => {
  const dir = process.env.FANDHE_WEB_TEST_DIR;
  await withServer({ dir, base: BASE }, async (srv) => {
    const P = await openOrigin(srv);
    const text = await call(P.page, 'loadableHistoryText');
    assert.equal(await call(P.page, 'importText', text), true);
    await waitImport(P.page, 'open');
    await settle(P.page);
    const f = path.join(tmpDir, 'read-fail.json');
    fs.writeFileSync(f, text);
    const stateBefore = JSON.stringify((await engine(P.page)).stateWithoutLog);
    const logBefore = (await engine(P.page)).logLen;
    const storeBefore = await P.page.evaluate(() => JSON.stringify(Object.entries(localStorage)));
    await patchRead(P.page);
    await P.page.evaluate(() => { window.__readGate.reject = true; });

    await P.page.setInputFiles(FILE_INPUT, f);
    await P.page.locator(NOTICE_ITEM).first().waitFor({ state: 'visible', timeout: 10000 });
    await P.page.waitForTimeout(300);
    const e = await engine(P.page);
    assert.notEqual(e.lastImport.verdict, 'open', 'not opened');
    assert.equal(await P.page.locator(NOTICE_ITEM).count(), 1, 'one notice');
    assert.equal(JSON.stringify(e.stateWithoutLog), stateBefore, 'state unchanged');
    assert.equal(e.logLen, logBefore, 'log unchanged');
    assert.equal(await P.page.evaluate(() => JSON.stringify(Object.entries(localStorage))), storeBefore, 'storage unchanged');
    const body = await P.page.locator(NOTICE_ITEM).first().innerText();
    assert.ok(!body.includes('read-fail') && !body.includes('forced'), 'no file name or exception message');
    assertQuiet(P.rec, 'read failure');
    await P.context.close();
  });
});

test('配信する設定（試験用の口なし）でも、画面の操作だけで書き出しの文が出る', async () => {
  const dir = process.env.FANDHE_WEB_DIR;
  await withServer({ dir, base: BASE }, async (srv) => {
    const P = await openOrigin(srv);
    assert.equal(await P.page.locator('[data-testid="shell-import"] input[type="file"]').count(), 1, 'import input is on the screen');
    await P.page.locator(EXPORT_BTN).click();
    await P.page.locator(STATUS).waitFor({ state: 'visible', timeout: 10000 });
    assert.equal(await P.page.locator(LINK).count(), 0, 'no link without a history');
    // 読み込みの経路（app.js → import_history）が配信物で動く錨: 選んでも例外・外部への要求が出ない。
    const empty = path.join(tmpDir, 'delivery-empty.json');
    fs.writeFileSync(empty, '');
    await P.page.setInputFiles(FILE_INPUT, empty);
    await P.page.waitForTimeout(300);
    assertQuiet(P.rec, 'delivery config');
    await P.context.close();
  });
});
