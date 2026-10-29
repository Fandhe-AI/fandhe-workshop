// 受講中の版の URL で続けられることの試験（TASK-178 1/2。VER-9・VER-10・DLV-26・D-52・D-56）。
//
// 版つき URL は「同じ配信の区間（pathname の先頭 1 区間。D-56）の下に置いた、旧版の配信物の凍結した写し」である。エンジンは
// 版つき URL を知らず、IR の取得先も `./ir/main.json` に固定のまま（SAFE-33）。そのため本試験は、1 つの配信元に次の 3 つの配信物を並べる。
//   /repo/      最新版（pack.version 2.0.0。Step の題を 1 つ変えた IR）
//   /repo/v1/   旧版の写し（pack.version 1.0.0）。最新版と同じ先頭区間 `repo` の下なので履歴の鍵が一致する
//   /repo-v1/   旧版の写し。先頭区間が `repo-v1` で別になる形（採らない形の負の対照）
// 3 つとも同じオリジン（同じ保存領域）で、同じ browser context から順に開く（別ポートは別オリジンで保存領域が分かれ、誤った理由で通るため）。
//
// 確かめること（CSP あり・なし。配信する設定。試験用の口なし。DOM だけの観察）:
//   A /repo/v1/ を新しい文脈で開き、実クリックで数手進める。保存先に履歴の鍵がちょうど 1 本できる。
//   B 同じ文脈で /repo/ を開くと表示だけの通知（content_major）が出る。入力・クリックを送り、再読み込みしても、保存先の値は A のまま。
//   C /repo/v1/ に戻ると A で到達した Step が旧版の題で出る（その版の IR で表示している証拠）。続きを 1 手進めると保存先の値が変わり、
//     保存した履歴の教材の版は 1.0.0 のまま。再読み込みで続けた位置に戻る。続けた後に /repo/ を開いても表示だけで、値は変わらない。
//   D 負の対照 /repo-v1/ では履歴が見えず最初の Step から始まり、保存先は変わらない（先頭区間が違うと鍵が分かれる）。
//   全場面で配信元以外への通信・CSP 違反・ページのエラー・dialog が 0 件。
//
// 確かめないこと（限界。docs/testing.md 5.379）:
//   - 旧版が残っていないときの表示のみへの落とし方・15 件のうち 5 件の扱い（TASK-178 2/2）。最新版の画面から旧版の URL への案内（REVIEW-8）。
//   - 本物の配信構成（教材 repo の TASK-125）。ここでは写しを合成して並べるだけ。
//   - 根への配信（先頭区間が無く、/v1/ が別の区間になって鍵が分かれる）。
// 失敗の文には ID・件数・真偽だけを出す（履歴の中身・本文を出さない）。
// 入力: FANDHE_WEB_AUTHOR_DIR（IR_FILE つき・TEST_HOOKS なしの配信物）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, stripCspMeta,
} from './harness.mjs';
import { choiceSel, T } from './ch0-3-journey.mjs';
import { domObserver03, TITLES } from './author-preview.mjs';

const HISTORY_PREFIX = 'fandhe-workshop/history/';
const NOTICE = '[data-testid="view-only-notice"]';
const DRAFT = '[data-testid="draft-input"]';
const CHOICE = '[data-testid="choice-btn"]';
const OLD = '1.0.0';
const NEW = '2.0.0';
const AT_A = 'main/st-0-2a';
const AFTER_A = 'main/st-0-2b';
const FIRST = 'main/st-0-1';
const NEW_TITLE_MARK = '版が新しくなった確認';

let browser;
let authorDir;

before(async () => {
  authorDir = process.env.FANDHE_WEB_AUTHOR_DIR;
  assert.ok(authorDir && fs.statSync(authorDir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_AUTHOR_DIR must point to a web directory built with IR_FILE and without TEST_HOOKS');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 配信物の写しを作り、ir/main.json の教材の版（と、最新版だけ Step の題）を書き換える。wasm などは同一で、再ビルドはしない。
function copyWith(dest, version, { retitle = false, csp }) {
  fs.cpSync(authorDir, dest, { recursive: true });
  const irPath = path.join(dest, 'ir', 'main.json');
  const ir = JSON.parse(fs.readFileSync(irPath, 'utf8'));
  ir.pack.version = version;
  if (retitle) ir.nodes.find((n) => n.uid === AT_A).step.title = NEW_TITLE_MARK;
  fs.writeFileSync(irPath, JSON.stringify(ir));
  if (!csp) {
    const index = path.join(dest, 'index.html');
    const { html, removed } = stripCspMeta(fs.readFileSync(index, 'utf8'));
    assert.equal(removed, 1, 'exactly one CSP meta is removed');
    fs.writeFileSync(index, html);
  }
}

async function withSite(csp, fn) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-versioned-url-'));
  try {
    fs.mkdirSync(path.join(root, 'repo'), { recursive: true });
    // 旧版の写しは最新版を置く前に作る（最新版の配信物の下に入れ子で写さない）。
    copyWith(path.join(root, 'repo', 'v1'), OLD, { csp });
    copyWith(path.join(root, 'repo-v1'), OLD, { csp });
    const latest = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-versioned-latest-'));
    try {
      copyWith(latest, NEW, { retitle: true, csp });
      for (const name of fs.readdirSync(latest)) fs.cpSync(path.join(latest, name), path.join(root, 'repo', name), { recursive: true });
    } finally {
      fs.rmSync(latest, { recursive: true, force: true });
    }
    return await withServer({ dir: root, base: '/' }, fn);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
}

const historyKeys = (page) => page.evaluate((prefix) => {
  const keys = [];
  for (let i = 0; i < localStorage.length; i += 1) {
    const k = localStorage.key(i);
    if (k.startsWith(prefix)) keys.push(k);
  }
  return keys.sort();
}, HISTORY_PREFIX);
// 値そのものは node へ出さず、page の中で SHA-256 の 16 進だけを返す。
const digestOf = (page, key) => page.evaluate(async (k) => {
  const v = localStorage.getItem(k);
  if (v === null) return null;
  const buf = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(v));
  return Array.from(new Uint8Array(buf), (b) => b.toString(16).padStart(2, '0')).join('');
}, key);
const savedContentVersion = (page, key) => page.evaluate((k) => {
  try {
    return JSON.parse(localStorage.getItem(k)).versions.content;
  } catch {
    return null;
  }
}, key);

async function open(page, srv, pathname) {
  await page.goto(`${srv.origin}${pathname}`);
  await waitMounted(page);
}

const observe = domObserver03();
// 復元した画面の確認。復元の直後はログに現在の Step の行があり、観察の 'entered'（最後の行が別の Step）は成り立たない。
// そのため Step の題（その版の IR の題）と処理中でないことだけを見る。
const shows = (page, uid) => page.waitForFunction(([t, sel]) => document.querySelector(sel)?.textContent === t
  && document.querySelector('[data-testid="run-status"]')?.getAttribute('data-state') === 'idle',
[TITLES[uid], '[data-testid="step-title"]'], { timeout: T });

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`旧版の URL で続けた履歴を最新版は壊さず、旧版の URL に戻ると続きが出る（CSP ${label}）`, { timeout: 300000 }, async () => {
    await withSite(csp, async (srv) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      try {
        // A: 旧版の URL で数手進める。
        await open(page, srv, '/repo/v1/');
        assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', 'delivery build has no test hooks');
        await observe.at(page, FIRST, 'entered');
        assert.deepEqual(await historyKeys(page), [], 'new context has no saved history');
        await page.click(choiceSel('opt-right'), { timeout: T });
        await observe.at(page, FIRST, 'accepted');
        await page.click('[data-testid="part-artifact-view"] [data-row="cards/notice"]', { timeout: T });
        await observe.at(page, AT_A, 'entered');
        await page.waitForFunction((p) => Object.keys(localStorage).some((k) => k.startsWith(p)), HISTORY_PREFIX, { timeout: T });
        const keys = await historyKeys(page);
        assert.equal(keys.length, 1, 'one history key after the old URL');
        const key = keys[0];
        const digestA = await digestOf(page, key);
        assert.equal(typeof digestA, 'string', 'saved value exists');
        assert.equal(await savedContentVersion(page, key), OLD, 'saved content version is the old one');

        // B: 最新版の URL。表示だけで、何を送っても保存先は変わらない。
        await open(page, srv, '/repo/');
        const notice = await page.locator(NOTICE).innerText();
        assert.ok(notice.includes('content_major'), 'view-only notice with content_major');
        if (await page.locator(DRAFT).count() > 0) {
          await page.fill(DRAFT, '/help');
          await page.press(DRAFT, 'Enter').catch(() => {});
        }
        const choices = page.locator(CHOICE);
        if (await choices.count() > 0) await choices.first().click({ timeout: 2000 }).catch(() => {});
        assert.equal(await digestOf(page, key), digestA, 'latest URL does not write the saved value');
        await page.reload();
        await waitMounted(page);
        assert.ok((await page.locator(NOTICE).innerText()).includes('content_major'), 'notice after reload');
        assert.deepEqual(await historyKeys(page), [key], 'latest URL adds no key');
        assert.equal(await digestOf(page, key), digestA, 'saved value after reload on the latest URL');

        // C: 旧版の URL に戻ると、A で到達した Step が旧版の題で出る。続きを進めると保存される。
        await open(page, srv, '/repo/v1/');
        await shows(page, AT_A);
        assert.ok(!(await page.evaluate(() => document.body.innerText)).includes('版が新しくなった確認'), 'old IR is shown, not the new one');
        assert.equal(await page.locator(NOTICE).count() === 0 || (await page.locator(NOTICE).innerText()) === '', true, 'no view-only notice on the old URL');
        await page.click(choiceSel('opt-read-a'), { timeout: T });
        await observe.at(page, AT_A, 'accepted');
        await page.click(choiceSel('ans-a-ok'), { timeout: T });
        await observe.at(page, AFTER_A, 'entered');
        await page.waitForFunction(async ([k, d]) => {
          const v = localStorage.getItem(k);
          if (v === null) return false;
          const buf = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(v));
          return Array.from(new Uint8Array(buf), (b) => b.toString(16).padStart(2, '0')).join('') !== d;
        }, [key, digestA], { timeout: T });
        assert.equal(await savedContentVersion(page, key), OLD, 'continued history keeps the old content version');
        assert.deepEqual(await historyKeys(page), [key], 'continuing adds no key');
        await page.reload();
        await waitMounted(page);
        await shows(page, AFTER_A);
        // 再読み込みの復元が記録を足して保存することがあるため、保存の待ち（SAVE-7 は 1 秒以内）を過ぎてから値を取る。
        await page.waitForTimeout(1500);
        const digestR = await digestOf(page, key);
        assert.notEqual(digestR, digestA, 'continued value differs from the first one');

        // 続けた後に最新版を開いても表示だけで、値は変わらない。
        await open(page, srv, '/repo/');
        assert.ok((await page.locator(NOTICE).innerText()).includes('content_major'), 'notice on the latest URL after continuing');
        // 旧版のページを離れるときの pagehide の保存は旧版の書き込みなので、最新版のページが開いてから値を取り、その後の不変を見る。
        await page.waitForTimeout(1500);
        const digestL = await digestOf(page, key);
        if (await page.locator(DRAFT).count() > 0) {
          await page.fill(DRAFT, '/help');
          await page.press(DRAFT, 'Enter').catch(() => {});
        }
        await page.reload();
        await waitMounted(page);
        await page.waitForTimeout(1500);
        assert.equal(await digestOf(page, key), digestL, 'latest URL keeps the continued value');
        assert.equal(await savedContentVersion(page, key), OLD, 'the latest URL leaves the old content version');

        // D: 負の対照。先頭区間が違う形では履歴が見えず最初の Step から始まる。
        await open(page, srv, '/repo-v1/');
        await observe.at(page, FIRST, 'entered');
        assert.equal(TITLES[FIRST].length > 0, true);
        assert.equal(await digestOf(page, key), digestL, 'the other first segment does not touch the saved value');
        assert.deepEqual(await historyKeys(page), [key], 'an unoperated other segment writes nothing');

        assert.equal(rec.external.length, 0, 'external requests');
        assert.equal(rec.csp.length, 0, 'csp violations');
        assert.equal(rec.pageErrors.length, 0, 'page errors');
        assert.equal(rec.dialogs.length, 0, 'dialogs');
        assert.equal(rec.crashed, false, 'crashed');
      } finally {
        await context.close();
      }
    });
  });
}
