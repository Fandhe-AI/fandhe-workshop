// 受け口を有効にしても安全性が変わらないことのブラウザ試験（EXT-10・D-53。TASK-191 2/2。記録は docs/testing.md 5.383）。
//
// 配信する設定には有効の状態が存在しない（有効にする経路は試験の設定の `?receptor=probe` だけ。受け口の実体も無い）。
// そこで試験の設定で、同じ入力・同じ観察を「無効（既定）」と「有効（`?receptor=probe`。代役の受け口を許可し同意した状態）」で流し、
// 観察が完全に一致すること・実行 0・外部通信 0・CSP 違反 0 を確かめる。配信する設定での SAFE-5・SAFE-7 の既存の試験
// （input-field-xss・url-xss）はそのまま残り、「有効にする経路が無い側」の基準になる。CSP は CSP なし（止められないぶん厳しい側）を必須にする。
//
//   A  錨と完全一致   有効の判定は `receptor=probe` の完全一致の 1 件だけ。配信する設定では試験の口が無く通常どおり起動する。
//   B  SAFE-5 の差分  E5 の 49 件 x 3 通りを本物の入力欄から送り、各回の観察が無効と有効で一致する。
//   C  SAFE-7 の差分  URL の注入 61 回を無効・有効で開き、観察が一致する。
//   D  外部通信の計数  20 Step の通しと再読み込みで、通信の試みが全区分 0 件。
//   E  対照           有効の回が空振りでないこと（錨が実際に違う値を返す）と、観察が禁止の要素を数えられること。
//
// 入力: FANDHE_WEB_DIR（配信する設定）・FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
// 失敗の文には回の ID と区分だけを出し、ペイロード・URL の値の全文は出さない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, summarize, DANGER_TAGS,
} from './harness.mjs';
import { attachEgress, egressCounts, egressFailures, EGRESS_KEYS } from './egress.mjs';
import { DRAFT, LOG, ledger } from './ch0-3-journey.mjs';
import { runShellJourney } from './shell-journey.mjs';
import { e5Payloads } from './xss-cases.mjs';
import { loadUrlCases, buildRuns, EXPECTED_RUNS } from './url-cases.mjs';

const BASE = '/fandhe-receptor/';
const OFF = '?fixture=ch0-3-links&start=s-1-4';
const ON = `${OFF}&receptor=probe`;
const ITEMS = `${LOG} > li`;
const RECEIVED = `${ITEMS}[data-kind="received"]`;
const BODY = '[data-testid="line-body"]';
const TITLE = '[data-testid="step-title"]';
const T = 30000;
const FORBIDDEN = [
  'a', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'pre', 'code', 'strong', 'em', 'b', 'i', 'blockquote', 'ul', 'ol', 'table', 'hr',
  'details', ...DANGER_TAGS,
].join(', ');
const MODES = [
  { name: 'そのまま', make: (p) => p },
  { name: '/ の後ろ', make: (p) => `/${p}` },
  { name: 'コマンドの引数', make: (p) => `/new-idea ${p}` },
];

let browser;

function assertDir(name) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to a built web directory`);
}

before(async () => {
  assertDir('FANDHE_WEB_DIR');
  assertDir('FANDHE_WEB_TEST_DIR');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

const short = (e) => String(e?.message ?? e).replaceAll(/\s+/g, ' ').slice(0, 300);
const activation = (page) => page.evaluate(() => window.__fandheTest.receptorActivation());

async function openTest(page, srv, query) {
  await page.goto(`${srv.url}${query}`);
  await waitMounted(page);
  assert.equal(await page.evaluate(() => typeof window.__fandheTest?.receptorActivation), 'function', 'test hooks are missing');
}

// 最後の受付の行とその次の行、Step の題、状態の JSON を 1 回で読む（値は比べるだけで出力しない）。
const observe = (page) => page.evaluate(([items, received, body, forbidden, title]) => {
  const describe = (li) => (li ? {
    kind: li.dataset.kind,
    input: li.dataset.input ?? null,
    children: [...li.children].map((c) => c.dataset.testid),
    text: li.querySelector(body)?.textContent ?? li.textContent,
    forbidden: li.querySelectorAll(forbidden).length,
  } : null);
  const recs = [...document.querySelectorAll(received)];
  const li = recs[recs.length - 1];
  return {
    received: recs.length,
    total: document.querySelectorAll(items).length,
    line: describe(li),
    next: describe(li ? li.nextElementSibling : null),
    title: document.querySelector(title)?.textContent ?? null,
    state: window.__fandheTest.stateJson(),
  };
}, [ITEMS, RECEIVED, BODY, FORBIDDEN, TITLE]);

async function sendOnce(page, value, before) {
  await page.fill(DRAFT, value);
  const sent = await page.inputValue(DRAFT);
  await page.press(DRAFT, 'Enter');
  await page.waitForFunction(([sel, n]) => document.querySelectorAll(sel).length === n, [RECEIVED, before + 1], { timeout: T });
  await page.waitForFunction(([items]) => {
    const n = document.querySelectorAll(items).length;
    if (window.__rcLast === n) window.__rcSame = (window.__rcSame ?? 0) + 1; else { window.__rcLast = n; window.__rcSame = 0; }
    return window.__rcSame >= 3;
  }, [ITEMS], { polling: 50, timeout: T });
  return { sent, obs: await observe(page) };
}

test('A: 有効は receptor=probe の完全一致の 1 件だけで、配信する設定には試験の口が無い', async () => {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp: false }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    for (const [q, want] of [
      ['', 'not_configured'],
      [OFF, 'not_configured'],
      ['?receptor=PROBE', 'not_configured'],
      ['?receptor=', 'not_configured'],
      ['?receptor=on', 'not_configured'],
      ['?receptor=probe&receptor=probe', 'not_configured'],
      ['?receptor=probe', 'enabled'],
      [ON, 'enabled'],
      [`?receptor=probe&${OFF.slice(1)}`, 'enabled'],
    ]) {
      const { page, context } = await openPage(browser, { origin: srv.origin });
      try {
        await openTest(page, srv, q);
        assert.equal(await activation(page), want, `query form ${JSON.stringify(q)}`);
      } finally {
        await context.close();
      }
    }
  }));
  await withCspMode({ dir: process.env.FANDHE_WEB_DIR, csp: false }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(`${srv.url}?receptor=probe`);
      await waitMounted(page);
      assert.equal(await page.evaluate(() => typeof window.__fandheTest), 'undefined', 'delivery config has no test hooks');
    } finally {
      await context.close();
    }
  }));
});

test('B: E5 の 49 件 x 3 通りの入力欄からの送信で、観察が無効と有効で一致し、実行・通信・危険な要素が 0 件', { timeout: 900000 }, async (t) => {
  const cases = e5Payloads();
  assert.equal(cases.length, 49);
  const failures = [];
  const logs = { off: [], on: [] };
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp: false }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    for (const [name, query] of [['off', OFF], ['on', ON]]) {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      try {
        const eg = await attachEgress(context, { origin: srv.origin, base: BASE });
        await openTest(page, srv, query);
        assert.equal(await activation(page), name === 'on' ? 'enabled' : 'not_configured');
        let n = (await observe(page)).received;
        for (const c of cases) {
          for (const m of MODES) {
            try {
              const { sent, obs } = await sendOnce(page, m.make(c.payload), n);
              n += 1;
              assert.equal(obs.line?.text, sent, 'received text equals sent');
              assert.equal(obs.line?.forbidden, 0, 'forbidden elements in received');
              assert.equal(obs.next?.forbidden ?? 0, 0, 'forbidden elements in next');
              logs[name].push(JSON.stringify(obs));
            } catch (e) {
              failures.push(`${name} ${c.id} ${m.name}: ${short(e)}`);
              n = (await observe(page)).received;
              logs[name].push('error');
            }
          }
        }
        assert.deepEqual(rec.hits, [], `${name}: executed`);
        assert.deepEqual(nonZero(await collectSafety(page, rec)), [], `${name}: safety counts`);
        assert.deepEqual(egressFailures(egressCounts(eg, rec)), [], `${name}: egress`);
        assert.equal(await activation(page), name === 'on' ? 'enabled' : 'not_configured', `${name}: activation unchanged by input`);
      } catch (e) {
        failures.push(`run ${name}: ${short(e)}`);
      } finally {
        await context.close();
      }
    }
  }));
  t.diagnostic(`sent ${cases.length * MODES.length} x 2 (off/on)`);
  assert.deepEqual(failures, []);
  assert.equal(logs.off.length, cases.length * MODES.length);
  const diffs = logs.off.map((x, i) => (x === logs.on[i] ? null : i)).filter((x) => x !== null);
  assert.deepEqual(diffs, [], 'observations differ between off and on (indices)');
});

test(`C: URL の注入 ${EXPECTED_RUNS} 回の観察が無効と有効で一致し、実行・通信・CSP 違反が 0 件`, { timeout: 900000 }, async () => {
  const runs = buildRuns(loadUrlCases());
  assert.equal(runs.length, EXPECTED_RUNS);
  const failures = [];
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp: false }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const one = async (run, on) => {
      const { page, context, rec } = await openPage(browser, { origin: srv.origin });
      try {
        const eg = await attachEgress(context, { origin: srv.origin, base: BASE });
        const query = on ? `${run.query ? `${run.query}&` : '?'}receptor=probe` : run.query;
        await page.goto(`${srv.url}${query}${run.hash}`);
        await waitMounted(page);
        await page.waitForSelector(TITLE, { timeout: T });
        await page.waitForLoadState('networkidle');
        assert.equal(await activation(page), on ? 'enabled' : 'not_configured', 'activation anchor');
        assert.deepEqual(rec.hits, [], 'executed');
        assert.deepEqual(nonZero(await collectSafety(page, rec)), [], 'safety counts');
        // 文書の要求の URL は注入の値を含むため、基準の許可の形に合わず offAllowlist になる（無効と有効で同じ）。それ以外の区分は 0。
        const counts = egressCounts(eg, rec);
        assert.deepEqual(egressFailures(counts, EGRESS_KEYS.filter((k) => k !== 'offAllowlist')), [], 'egress');
        const shown = await page.evaluate((title) => ({
          title: document.querySelector(title)?.textContent ?? null,
          text: document.body.innerText,
          html: document.documentElement.outerHTML.length,
        }), TITLE);
        return JSON.stringify({ shown, requests: summarize(rec), counts });
      } finally {
        await context.close();
      }
    };
    for (const run of runs) {
      try {
        const off = await one(run, false);
        const on = await one(run, true);
        assert.equal(on, off, 'observation differs between off and on');
      } catch (e) {
        failures.push(`${run.runId}: ${short(e)}`);
      }
    }
  }));
  assert.deepEqual(failures, []);
});

test('D: 有効のまま 20 Step を通し再読み込みしても、通信の試みが全区分 0 件で有効の状態が保たれる', { timeout: 600000 }, async () => {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp: false }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport: { width: 1280, height: 800 } });
    try {
      const eg = await attachEgress(context, { origin: srv.origin, base: BASE });
      await openTest(page, srv, '?fixture=ch0-3-chapters&receptor=probe');
      assert.equal(await activation(page), 'enabled');
      const L = ledger();
      const result = await runShellJourney(page, { width: 1280, scene: 'receptor', L, hooks: {} });
      assert.deepEqual(L.failed(), []);
      assert.equal(result.achieved, 20);
      assert.equal(await activation(page), 'enabled');
      assert.equal(summarize(rec).pageErrors, 0);
      assert.deepEqual(nonZero(await collectSafety(page, rec)), []);
      assert.deepEqual(egressFailures(egressCounts(eg, rec)), [], 'journey egress');
      await page.reload();
      await waitMounted(page);
      assert.equal(await activation(page), 'enabled', 'enabled again after reload (consent is re-granted by the probe, never stored)');
      assert.deepEqual(egressFailures(egressCounts(eg, rec)), [], 'reload egress');
    } finally {
      await context.close();
    }
  }));
});

test('E: 対照。観察は禁止の要素を数えられ、錨は無効と有効で実際に違う値を返す', async () => {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp: false }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context } = await openPage(browser, { origin: srv.origin });
    try {
      await openTest(page, srv, OFF);
      const n = await page.evaluate((forbidden) => {
        const li = document.createElement('li');
        li.innerHTML = '<img><svg></svg><a href="#x">l</a>';
        return li.querySelectorAll(forbidden).length;
      }, FORBIDDEN);
      assert.equal(n, 3);
      const a = await activation(page);
      await openTest(page, srv, ON);
      assert.notEqual(a, await activation(page));
    } finally {
      await context.close();
    }
  }));
});
