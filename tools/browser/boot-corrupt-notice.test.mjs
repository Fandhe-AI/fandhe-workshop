// 起動時に壊れた履歴の案内が、配信する設定の画面で DOM だけの観察で出ることの試験（SHELL-17。TASK-533 1/2）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。boot-corrupt-history（試験の設定。試験用の口で保存先を操作する）と違い、
// 本試験は試験用の口の無い配信する設定（IR_FILE つき・TEST_HOOKS なし）で、実際の操作と localStorage の読み書きだけで確かめる。
// 入力: FANDHE_WEB_AUTHOR_DIR・FANDHE_AUTHOR_IR。未設定・不在は skip にせず失敗にする。
// 確かめること（CSP あり・なしの両方。壊れた値は、アプリを起動しない同じオリジンの静的資源へ移って保存先へ置き、開き直して起動する）:
//   壊れた履歴 5 通り（B-2・B-3・B-5・B-6・B-7。作り方は testdata/broken-histories の表と同じ規則）:
//     corrupt-history の通知がちょうど 1 件で理由が表のコードと一致し、文に理由の文と案内の文（ボタン名を含む）が入り、
//     「捨てて始める」のボタンが 1 個で見え、通知に閉じる操作が無く、`import-refused` は 0 件、最初の Step で始まる（ログ 0 行）。
//     画面の操作を 1 回行っても保存先の履歴の鍵と値のハッシュが変わらず、開き直しても同じ通知が出て値が不変である。
//     幅 360px で成果物のタブを選んでも通知とボタンが見えている。入力に入れた番兵が DOM・コンソールに無い。
//     外部通信・CSP 違反・ページのエラー・ダイアログが 0 件。
//   対照: 壊していない値で開き直すと通知は 0 件で、最初の Step の状態にならない（復元される）。
//   「捨てて始める」を押した後（TASK-533 2/2。#902。上の 5 通り × CSP 2 通り）:
//     通知とボタンが 0 件になり最初の Step で始まり、壊れた今の鍵だけが消えて別の開始点・別の教材の鍵 2 本が不変で、
//     未操作の開き直しでは鍵が作られず、実操作 1 回で壊れた値と異なる JSON の値が今の鍵へ書かれ（番兵なし）、再読み込みで復元される。
//     B-2 はキーボード（Enter）で押す。
// 範囲外: 受講者が任意に行う履歴の削除（SHELL-38・TASK-539）。
// 限界: B-1（submit の書き換え）は作者の教材の最初の操作が選択肢で submit を持たないため、B-4（10MB 超）は localStorage に置けないため除く。
//   その 2 通りは boot-corrupt-history が試験の設定で確かめる。保存先は localStorage の範囲（IndexedDB に入った場合は失敗にする）。Chromium のみ。
//   作り方が表と違う理由になった場合は失敗にする（表を黙って直さない）。
// 失敗の文・出力には、件数・固定のコード・真偽・ハッシュの先頭だけを出す（履歴の中身と番兵の値は出さない）。

import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';
import { stepOrder, inspectDom, domObserver } from './l2-journey.mjs';
import { mouseDriver } from './ch0-3-journey.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const BASE = '/fandhe-boot-corrupt-notice/';
const PREFIX = 'fandhe-workshop/history/';
const BOOT_MARK = 'fandhe-workshop/meta/boot/';
const T = 30000;
const NOTICE = (kind) => `[data-testid="shell-notice-item"][data-notice-kind="${kind}"]`;
const DISCARD = '[data-testid="shell-discard-start"]';
const GUIDE = '「保存された履歴を捨てて始める」を押すと';

const EXPECTATIONS = JSON.parse(fs.readFileSync(path.join(ROOT, 'testdata/broken-histories/expectations.json'), 'utf8'));
const expectCode = (id) => EXPECTATIONS.rows.find((r) => r.id === id).expect.code;

const edit = (base, fn) => {
  const v = JSON.parse(base);
  fn(v);
  return JSON.stringify(v);
};
const CASES = [
  { id: 'B-2', fragment: '途中で切れて', make: (s, b) => b.slice(0, Math.floor(b.length * 0.55)) },
  { id: 'B-3', fragment: 'JSON として読めません', make: (s, b) => { const at = b.indexOf('"ops":[') + '"ops":['.length; return `${b.slice(0, at)}}${b.slice(at)}`; } },
  { id: 'B-5', fragment: '版の欄がありません', make: (s, b) => edit(b, (v) => { delete v.versions; }) },
  { id: 'B-6', fragment: '別の教材で記録された', make: (s, b) => edit(b, (v) => { v.pack_id = s; }) },
  { id: 'B-7', fragment: '決められた形と合いません', make: (s, b) => edit(b, (v) => { v.snapshot.run.generation = s; }) },
];

let browser;
let webDir;
let order;
let first;

before(async () => {
  webDir = process.env.FANDHE_WEB_AUTHOR_DIR;
  assert.ok(webDir && fs.statSync(webDir, { throwIfNoEntry: false })?.isDirectory(), 'FANDHE_WEB_AUTHOR_DIR must point to a web directory built with IR_FILE and without TEST_HOOKS');
  const irFile = process.env.FANDHE_AUTHOR_IR;
  assert.ok(irFile && fs.statSync(irFile, { throwIfNoEntry: false })?.isFile(), 'FANDHE_AUTHOR_IR must point to the compiled IR');
  order = stepOrder(fs.readFileSync(irFile, 'utf8'));
  [first] = order;
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const sha = (s) => crypto.createHash('sha256').update(s).digest('hex');

/** 履歴の鍵（起動の印を除く）と値の長さ・ハッシュの対を鍵で整列して返す。 */
async function historyEntries(page) {
  const raw = await page.evaluate(([prefix, boot]) => {
    const out = [];
    for (let i = 0; i < localStorage.length; i += 1) {
      const k = localStorage.key(i);
      if (k !== null && k.startsWith(prefix) && !k.startsWith(boot)) out.push([k, localStorage.getItem(k) ?? '']);
    }
    return out;
  }, [PREFIX, BOOT_MARK]);
  return raw.map(([k, v]) => ({ key: k, len: v.length, hash: sha(v) })).sort((a, b) => (a.key < b.key ? -1 : 1));
}

const waitHistoryKeys = (page, n) => page.waitForFunction(([prefix, boot, k]) => {
  let c = 0;
  for (let i = 0; i < localStorage.length; i += 1) {
    const key = localStorage.key(i) ?? '';
    if (key.startsWith(prefix) && !key.startsWith(boot)) c += 1;
  }
  return c === k;
}, [PREFIX, BOOT_MARK, n], { timeout: T });

const logRows = (page) => page.locator('[data-testid="part-log-list"] li').count();

function assertQuiet(rec, label) {
  const s = summarize(rec);
  assert.equal(s.crashed, 0, `${label}: tab must not crash`);
  assert.equal(s.external, 0, `${label}: no external request`);
  assert.equal(s.csp, 0, `${label}: no CSP violation`);
  assert.equal(s.pageErrors, 0, `${label}: no page error`);
  assert.equal(s.dialogs, 0, `${label}: no dialog`);
  assert.equal(s.popups, 0, `${label}: no popup`);
}

async function sentinelAbsent(opened, sentinel) {
  const html = await opened.page.evaluate(() => document.documentElement.outerHTML);
  const text = await opened.page.evaluate(() => document.body.innerText);
  return !html.includes(sentinel) && !text.includes(sentinel) && !opened.consoleTexts.some((t) => t.includes(sentinel));
}

/** 初期の Step（ログ 0 行）で始まっているか。 */
async function isInitial(page) {
  return (await page.evaluate(inspectDom, [order, first, 'entered'])).ok && (await logRows(page)) === 0;
}

/** 保存済みの履歴を 1 本作る（実操作 2 回。保存の値が落ち着くまで待つ）。保存先の値を返す。 */
async function saveHistory(opened, srv) {
  const { page } = opened;
  await page.goto(srv.url);
  await waitMounted(page);
  assert.ok(await isInitial(page), 'starts at the first step');
  const d = mouseDriver(page);
  await d.choose('opt-right');
  await domObserver(order).at(page, first, 'accepted');
  await d.point('cards/notice');
  await waitHistoryKeys(page, 1);
  let prev = '';
  for (let n = 0; n < 50; n += 1) {
    const cur = JSON.stringify(await historyEntries(page));
    if (cur === prev) break;
    prev = cur;
    await page.waitForTimeout(100);
  }
  await page.waitForTimeout(1500);
}

/** アプリを起動せず同じオリジンの静的資源へ移る（起動の記録で保存先が変わらない読み書きのため）。 */
async function offApp(page, srv) {
  const res = await page.goto(`${srv.url}icon.svg`);
  assert.equal(res?.status(), 200, 'static asset is served');
}

async function boot(page, srv) {
  await page.goto(srv.url);
  await waitMounted(page);
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';
  for (const c of CASES) {
    test(`壊れた履歴 ${c.id}: 配信する設定で理由と案内が出て新しい状態で始まり、保存先が変わらない（CSP ${label}）`, async (t) => {
      await withCspMode({ dir: webDir, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const opened = await openPage(browser, { origin: srv.origin });
        opened.consoleTexts = [];
        opened.page.on('console', (m) => opened.consoleTexts.push(m.text()));
        const { page } = opened;
        await saveHistory(opened, srv);
        await offApp(page, srv);
        const saved = await historyEntries(page);
        assert.equal(saved.length, 1, 'exactly one history key in localStorage');
        const [{ key }] = saved;
        const base = await page.evaluate((k) => localStorage.getItem(k), key);
        const sentinel = `SHELL17-SENTINEL-${c.id}`;
        const broken = c.make(sentinel, base);
        assert.notEqual(broken, base, 'the value is actually broken');
        await page.evaluate(([k, v]) => localStorage.setItem(k, v), [key, broken]);
        const placed = await historyEntries(page);
        t.diagnostic(`case=${c.id} keys=${placed.length} bytes=${placed[0].len} hash=${placed[0].hash.slice(0, 12)}`);

        await boot(page, srv);
        const notice = page.locator(NOTICE('corrupt-history'));
        assert.equal(await notice.count(), 1, 'one corrupt-history notice');
        assert.equal(await notice.getAttribute('data-notice-reason'), expectCode(c.id), 'reason code matches the table');
        const body = await notice.textContent();
        assert.ok(body.includes(c.fragment), 'notice has the reason text');
        assert.ok(body.includes(GUIDE), 'notice has the guide text');
        assert.equal(await page.locator(NOTICE('import-refused')).count(), 0, 'no import-refused');
        assert.equal(await page.locator(DISCARD).count(), 1, 'one discard button');
        assert.equal(await page.locator(DISCARD).isVisible(), true, 'discard button is visible');
        assert.equal(await page.locator(`${NOTICE('corrupt-history')} [data-testid="shell-dismiss"]`).count(), 0, 'not dismissable');
        assert.ok(await isInitial(page), 'starts fresh at the first step');
        assert.equal(await sentinelAbsent(opened, sentinel), true, 'input value is not shown or logged');

        // 操作しても保存先は変わらない（壊れた値を上書きしない）。本試験ではボタンを押さない（押した後は下の「捨てて始める」の試験）。
        await mouseDriver(page).choose('opt-right');
        await page.waitForTimeout(1500);
        await offApp(page, srv);
        assert.deepEqual(await historyEntries(page), placed, 'history keys and hashes unchanged after an operation');

        await boot(page, srv);
        assert.equal(await page.locator(NOTICE('corrupt-history')).count(), 1, 'notice again after reload');
        assert.equal(await page.locator(DISCARD).count(), 1, 'discard button again after reload');
        assert.ok(await isInitial(page), 'fresh again after reload');
        await offApp(page, srv);
        assert.deepEqual(await historyEntries(page), placed, 'unchanged after reload');

        // 幅 360px では成果物のタブを選んでも通知とボタンが見えている（SHELL-1）。
        await boot(page, srv);
        await page.setViewportSize({ width: 360, height: 800 });
        const tab = page.locator('[role="tab"]');
        if (await tab.count() > 1) await tab.nth(0).click();
        assert.equal(await page.locator(NOTICE('corrupt-history')).isVisible(), true, 'notice visible at 360px');
        assert.equal(await page.locator(DISCARD).isVisible(), true, 'button visible at 360px');
        assert.equal(await sentinelAbsent(opened, sentinel), true, 'no sentinel at the end');
        assertQuiet(opened.rec, c.id);
        await opened.context.close();
      }));
    });
  }

  for (const c of CASES) {
    test(`壊れた履歴 ${c.id}: 配信する設定で「捨てて始める」を押すと今の鍵だけが消え、次の操作から保存が再開する（CSP ${label}）`, async (t) => {
      await withCspMode({ dir: webDir, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const opened = await openPage(browser, { origin: srv.origin });
        opened.consoleTexts = [];
        opened.page.on('console', (m) => opened.consoleTexts.push(m.text()));
        const { page } = opened;
        await saveHistory(opened, srv);
        await offApp(page, srv);
        const saved = await historyEntries(page);
        assert.equal(saved.length, 1, 'exactly one history key in localStorage');
        const [{ key }] = saved;
        const base = await page.evaluate((k) => localStorage.getItem(k), key);
        const sentinel = `SHELL17-SENTINEL-${c.id}`;
        const broken = c.make(sentinel, base);
        assert.notEqual(broken, base, 'the value is actually broken');
        // 対照の鍵 2 本（別の開始点・別の教材）。消してはいけない（A01）。
        const parts = key.split('/');
        assert.equal(parts.length, 5, 'key has five segments');
        const otherStart = [...parts.slice(0, 4), 'other-start'].join('/');
        const otherPack = [...parts.slice(0, 3), 'other-pack', parts[4]].join('/');
        await page.evaluate(([k, v, a, b, w]) => {
          localStorage.setItem(k, v);
          localStorage.setItem(a, w);
          localStorage.setItem(b, w);
        }, [key, broken, otherStart, otherPack, base]);
        const placed = await historyEntries(page);
        assert.equal(placed.length, 3, 'three history keys placed');
        const controls = placed.filter((e) => e.key !== key);
        assert.equal(controls.length, 2, 'two control keys');
        const brokenHash = placed.find((e) => e.key === key).hash;
        t.diagnostic(`case=${c.id} keys=${placed.length} broken=${brokenHash.slice(0, 12)}`);

        await boot(page, srv);
        assert.equal(await page.locator(NOTICE('corrupt-history')).count(), 1, 'one corrupt-history notice');
        assert.equal(await page.locator(DISCARD).count(), 1, 'one discard button');
        // B-2 だけキーボード（focus して Enter）で押す。他はマウス。
        if (c.id === 'B-2') {
          await page.locator(DISCARD).focus();
          await page.keyboard.press('Enter');
        } else {
          await page.locator(DISCARD).click();
        }
        await page.locator(NOTICE('corrupt-history')).waitFor({ state: 'detached', timeout: T });
        assert.equal(await page.locator(DISCARD).count(), 0, 'discard button gone');
        assert.ok(await isInitial(page), 'starts fresh at the first step after discard');
        await waitHistoryKeys(page, 2);
        await offApp(page, srv);
        assert.deepEqual(await historyEntries(page), controls, 'only the current key is removed; control keys unchanged');

        // 未操作の起動では鍵を作らず、通知も出ない。
        await boot(page, srv);
        assert.equal(await page.locator(NOTICE('corrupt-history')).count(), 0, 'no notice after discard');
        assert.ok(await isInitial(page), 'fresh after reopen');
        await offApp(page, srv);
        assert.deepEqual(await historyEntries(page), controls, 'no key created without an operation');

        // 保存の再開: 実操作 1 回で今の鍵が書き直される。
        await boot(page, srv);
        await mouseDriver(page).choose('opt-right');
        await domObserver(order).at(page, first, 'accepted');
        await waitHistoryKeys(page, 3);
        let prev = '';
        for (let n = 0; n < 50; n += 1) {
          const cur = JSON.stringify(await historyEntries(page));
          if (cur === prev) break;
          prev = cur;
          await page.waitForTimeout(100);
        }
        await page.waitForTimeout(1500);
        await offApp(page, srv);
        const after = await historyEntries(page);
        const mine = after.find((e) => e.key === key);
        assert.ok(mine, 'current key written again');
        assert.notEqual(mine.hash, brokenHash, 'new value differs from the broken one');
        assert.deepEqual(after.filter((e) => e.key !== key), controls, 'control keys unchanged after resume');
        const v = await page.evaluate(([k, s]) => {
          const raw = localStorage.getItem(k) ?? '';
          let parses = true;
          try { JSON.parse(raw); } catch { parses = false; }
          return { parses, hasSentinel: raw.includes(s) };
        }, [key, sentinel]);
        assert.equal(v.parses, true, 'saved value parses as JSON');
        assert.equal(v.hasSentinel, false, 'saved value does not carry the broken input');

        // 再読み込みで復元される。
        await boot(page, srv);
        assert.equal(await page.locator(NOTICE('corrupt-history')).count(), 0, 'no notice after resume');
        assert.equal(await page.locator(DISCARD).count(), 0, 'no discard button after resume');
        assert.equal(await isInitial(page), false, 'restored after reload, not fresh');
        assert.equal(await sentinelAbsent(opened, sentinel), true, 'no sentinel at the end');
        assertQuiet(opened.rec, `${c.id}-discard`);
        await opened.context.close();
      }));
    });
  }

  test(`対照: 壊していない値で開き直すと通知は 0 件で復元される（CSP ${label}）`, async () => {
    await withCspMode({ dir: webDir, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const opened = await openPage(browser, { origin: srv.origin });
      const { page } = opened;
      await saveHistory(opened, srv);
      await boot(page, srv);
      assert.equal(await page.locator(NOTICE('corrupt-history')).count(), 0, 'openable value: no corrupt-history');
      assert.equal(await page.locator(DISCARD).count(), 0, 'no discard button');
      assert.equal(await isInitial(page), false, 'restored, not fresh');
      assertQuiet(opened.rec, 'control');
      await opened.context.close();
    }));
  });
}
