// tools/browser/harness.mjs の安全性の計数（TASK-143 1/2）の試験（`make test-browser` から呼ばれる）。
//
// 合成した静的ページで、各計数（記録用の関数・ダイアログ・ポップアップ・画面の走査・CSP の切り替え）が
// 実際に 1 件以上数えられることを示す。後続の試験（TASK-144 以降・TASK-143 2/2）の「0 件」が空振りでないことの裏づけになる。
// SAFE-8 の値との一致は TASK-143 2/2 が実際にビルドした画面で確かめる。Chromium は要るが、wasm のビルドは要らない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, stripCspMeta, collectSafety, nonZero,
  SAFETY_KEYS, HIT_FN,
} from './harness.mjs';

const CSP = "default-src 'self'; script-src 'self'; connect-src 'self'; img-src 'self'";
const META = `<meta http-equiv="Content-Security-Policy" content="${CSP}">`;
let tmp;
let browser;

function doc(head, body) {
  return `<!doctype html><html><head><meta charset="utf-8">${head}</head><body>${body}</body></html>`;
}
function write(name, html) {
  fs.writeFileSync(path.join(tmp, name), html);
}

before(async () => {
  tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-safety-test-'));
  write('clean.html', doc(META, '<p>ok</p>'));
  write('hit.js', `window.${HIT_FN}('a');\n`);
  write('hit.html', doc('', '<p>x</p><script src="./hit.js"></script>'));
  write('dialog.js', 'alert("x");\n');
  write('dialog.html', doc('', '<p>x</p><script src="./dialog.js"></script>'));
  write('popup.js', 'window.open("./clean.html");\n');
  write('popup.html', doc('', '<p>x</p><script src="./popup.js"></script>'));
  write('scan.html', doc('', [
    '<p onclick="void 0" style="color:red">x</p>',
    '<a href="javascript:void 0">j</a>',
    '<img src="https://example.invalid/x">',
    '<iframe srcdoc="<p>z</p>"></iframe>',
  ].join('')));
  write('entry1.html', doc('', '<script src="./app.js"></script>'));
  write('entry2.html', doc('', '<script src="./app.js"></script><script src="./app.js"></script>'));
  write('app.js', '\n');
  write('provoke.html', doc('', `<p id="m" onmouseover="${HIT_FN}('m')">x</p>`));
  // index.html: CSP の meta とインラインの script。目印の要素は script が実行されたかと無関係に描かれる。
  write('index.html', doc(META, `<p id="mark">mark</p><script>window.${HIT_FN}('i')</script>`));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
  if (tmp) fs.rmSync(tmp, { recursive: true, force: true });
});

// file を開いて集計する。gzip は有効のまま配信する。
async function visit(file, { dir = tmp, scanOpts } = {}) {
  return withServer({ dir, base: '/s/' }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(`${srv.url}${file}`);
    await page.waitForTimeout(300);
    const counts = await collectSafety(page, rec, scanOpts);
    const mark = await page.locator('#mark').count();
    await context.close();
    return { counts, rec, mark };
  });
}

test('stripCspMeta: 大文字小文字を問わず 1 件を除去する', () => {
  const r = stripCspMeta(`<head><META HTTP-EQUIV='content-security-policy' content="x"><meta charset="utf-8"></head>`);
  assert.equal(r.removed, 1);
  assert.equal(r.html, '<head><meta charset="utf-8"></head>');
  assert.equal(stripCspMeta('<p>none</p>').removed, 0);
});

test('withCspMode: csp が真偽値以外なら例外', async () => {
  for (const v of [undefined, null, 'false', 0]) {
    await assert.rejects(() => withCspMode({ dir: tmp, csp: v }, async () => {}), /csp must be a boolean/);
  }
});

test('withCspMode: CSP の meta が 0 件・2 件なら例外（写しは残らない）', async () => {
  for (const [name, html, n] of [['zero', doc('', '<p>x</p>'), 0], ['two', doc(META + META, '<p>x</p>'), 2]]) {
    const d = fs.mkdtempSync(path.join(os.tmpdir(), `fandhe-safety-${name}-`));
    try {
      fs.writeFileSync(path.join(d, 'index.html'), html);
      let copy = null;
      await assert.rejects(
        () => withCspMode({ dir: d, csp: false }, async (c) => { copy = c; }),
        new RegExp(`found ${n}`),
      );
      assert.equal(copy, null);
    } finally {
      fs.rmSync(d, { recursive: true, force: true });
    }
  }
});

test('withCspMode: csp true は元のまま、false は meta の無い写しを渡し、fn が失敗しても写しを消す', async () => {
  await withCspMode({ dir: tmp, csp: true }, async (d) => assert.equal(d, tmp));
  let copy = null;
  await assert.rejects(() => withCspMode({ dir: tmp, csp: false }, async (d) => {
    copy = d;
    assert.notEqual(d, tmp);
    assert.equal(stripCspMeta(fs.readFileSync(path.join(d, 'index.html'), 'utf8')).removed, 0);
    assert.ok(fs.readFileSync(path.join(tmp, 'index.html'), 'utf8').includes('Content-Security-Policy'));
    throw new Error('boom');
  }), /boom/);
  assert.ok(copy && !fs.existsSync(copy));
});

test('対照: 何も違反しないページは全キー 0 で、走査は実際に動く', async () => {
  const { counts } = await visit('clean.html');
  assert.deepEqual(nonZero(counts), [], JSON.stringify(counts));
  assert.deepEqual(Object.keys(counts).sort(), [...SAFETY_KEYS].sort());
});

test('対照: 記録用の関数の呼び出しが数えられる', async () => {
  const { counts, rec } = await visit('hit.html');
  assert.equal(counts.hits, 1);
  assert.deepEqual(rec.hits, ['a']);
});

test('対照: ダイアログが数えられ、試験は止まらない', async () => {
  const { counts } = await visit('dialog.html');
  assert.equal(counts.dialogs, 1);
});

test('対照: ポップアップが数えられ、閉じられる', async () => {
  const { counts, rec } = await visit('popup.html');
  assert.equal(counts.popups, 1);
  assert.ok(rec.popups[0].includes('/s/'), rec.popups[0]);
});

test('対照: 画面の走査が各区分を数える', async () => {
  const { counts } = await visit('scan.html');
  assert.equal(counts.onAttrs, 1);
  assert.equal(counts.styleAttrs, 1);
  assert.equal(counts.nonHttp, 2); // javascript: の href と srcdoc
  assert.equal(counts.extHttp, 1);
  assert.equal(counts.danger, 2); // iframe・img
  assert.equal(counts.external, 1); // img は配信元以外のため abort され、通信として数えられる
  assert.equal(counts.scanError, 0);
});

test('除外の規則: 画面自身の script は 1 件だけ除外し、2 件目は危険に数える', async () => {
  assert.equal((await visit('entry1.html')).counts.danger, 0);
  assert.equal((await visit('entry2.html')).counts.danger, 1);
});

test('能動の段: onmouseover が発火して記録される', async () => {
  const { counts, rec } = await visit('provoke.html');
  assert.equal(counts.hits, 1);
  assert.deepEqual(rec.hits, ['m']);
  assert.equal((await visit('provoke.html', { scanOpts: { provoke: false } })).counts.hits, 0);
});

test('CSP の切り替え: あり=違反が数えられ実行されない、なし=実行され、どちらも本文は描かれる', async () => {
  const on = await withCspMode({ dir: tmp, csp: true }, (d) => visit('index.html', { dir: d }));
  assert.ok(on.counts.csp >= 1, JSON.stringify(on.counts));
  assert.equal(on.counts.hits, 0);
  assert.equal(on.mark, 1);
  const off = await withCspMode({ dir: tmp, csp: false }, (d) => visit('index.html', { dir: d }));
  assert.equal(off.counts.csp, 0, JSON.stringify(off.counts));
  assert.equal(off.counts.hits, 1);
  assert.equal(off.mark, 1);
});

test('走査の失敗は 0 件にせず scanError に数える', async () => {
  await withServer({ dir: tmp, base: '/s/' }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    await page.goto(`${srv.url}clean.html`);
    await page.close();
    const counts = await collectSafety(page, rec, { settleMs: 0 });
    assert.equal(counts.scanError, 1);
    await context.close();
  });
});

test('対照: タブの異常終了（chrome://crash）が数えられる', async () => {
  const { page, context, rec } = await openPage(browser, { origin: 'http://127.0.0.1:1' });
  await page.goto('about:blank');
  await page.goto('chrome://crash').catch(() => {});
  await new Promise((resolve) => setTimeout(resolve, 1000));
  assert.equal(rec.crashed, true);
  const counts = await collectSafety(page, rec, { settleMs: 0 });
  assert.equal(counts.crashed, 1);
  assert.equal(counts.scanError, 1); // 落ちたタブの走査は 0 件でなく失敗として数える
  await context.close();
});
