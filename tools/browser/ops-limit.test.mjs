// `ops` の件数の上限（20,000 件。D-44）のブラウザでの確認（SAFE-19・SAFE-41。TASK-156）。
//
// 呼び出し元は `make test-browser`（scripts/test-browser.sh）である。入力は試験の設定の画面（FANDHE_WEB_TEST_DIR）だけで、
// 教材は既定（min-3step・開始点 s-1）を使う。履歴は試験の口（opsHistoryText）で実行時に作り、repo に置かない。
//
// 確かめること（CSP あり・なしの両方）:
//   - ファイル選択の経路: 20,000 件ちょうど（実操作）は開く。同じ件数の合成の履歴は件数では断られず、後ろの段（適用し直し）で断られる。
//     1 件多い履歴・10 倍（200,000 件。10MB 未満）の履歴は too_many_ops で断られ、理由が通知欄に 1 秒以内に出る。
//     断ったときは状態・入力欄・保存先が変わらず、環境の差し替えが 0 回で、その後も操作が効く
//   - 保存先からの起動の経路: 19,990 件（実操作）の履歴が、再読み込みの開始から 1 秒以内に開く（#app へのマウントの時刻で判定。bootKind が restored）。
//     20,001 件の履歴は 1 秒以内に not_opened になり、操作の後も保存先の値は変わらない（起動時の件数の理由の案内は TASK-533 の範囲）
//   - 外部への要求・CSP 違反・ページのエラー・dialog・popup が 0 件
// 時間はページの中の performance.now() で取る（CDP の往復を含めない）。閾値は緩めない。実測は diagnostic に残す。
// 失敗の文には件数・バイト数・ms・固定のコードだけを出す（履歴の中身は出さない）。通知の文は shell_notice.rs（最終の文面は TASK-549・TASK-534）。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, summarize,
} from './harness.mjs';

const BASE = '/fandhe-ops-limit/';
const FILE_INPUT = '[data-testid="history-file"]';
const ITEM = '[data-testid="shell-notice-item"][data-notice-kind="import-refused"]';
const DISMISS = `${ITEM} [data-testid="shell-dismiss"]`;
const INPUT = '[data-testid="draft-input"]';
const OPERATION = '/make-note second-note';
const PACK = 'min-3step';
const START = 's-1';
const LIMIT = 20000;
const MAX_BYTES = 10 * 1024 * 1024;
const BUDGET_MS = 1000;

let browser;
let tmp;

before(async () => {
  for (const name of ['FANDHE_WEB_DIR', 'FANDHE_WEB_TEST_DIR']) {
    const dir = process.env[name];
    assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to a directory`);
  }
  tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-ops-limit-'));
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
  if (tmp) fs.rmSync(tmp, { recursive: true, force: true });
});

const call = (page, fn, ...args) => page.evaluate(([f, a]) => window.__fandheTest[f](...a), [fn, args]);
const engine = async (page) => JSON.parse(await call(page, 'stateJson')).engine;

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

// 状態・ログ・世代の指紋（真偽の比較にだけ使い、内容は出さない）。
const fingerprint = (e) => JSON.stringify({ s: e.stateWithoutLog, l: e.log, g: e.generation, c: e.current });

async function openStorage(page) {
  await call(page, 'storageOpen');
  await page.waitForFunction(([p, s]) => JSON.parse(window.__fandheTest.storageState(p, s)).opened === true, [PACK, START], { timeout: 10000 });
}

async function freshPage(srv) {
  const opened = await openPage(browser, { origin: srv.origin });
  await opened.page.goto(srv.url);
  await waitMounted(opened.page);
  await settle(opened.page);
  return opened;
}

const writeTmp = (name, text) => {
  const p = path.join(tmp, name);
  fs.writeFileSync(p, text);
  return p;
};

// 上限ちょうど（実操作）の履歴の文字列。ページの口で作る。
const baseHistory = async (page) => {
  const text = await call(page, 'opsHistoryText', LIMIT);
  assert.ok(text.length > 0, 'the base history was built');
  return text;
};

const withOps = (text, ops) => {
  const v = JSON.parse(text);
  v.ops = ops;
  return JSON.stringify(v);
};
const synthetic = (n) => Array.from({ length: n }, () => ({ draft: 'x' }));

// ファイル入力の change から通知の要素が現れるまでをページの中で計る。
async function armNoticeTimer(page) {
  await page.evaluate(([sel, item]) => {
    window.__opsLimitT = { start: null, end: null };
    document.querySelector(sel).addEventListener('change', () => { window.__opsLimitT.start = performance.now(); }, { capture: true, once: true });
    const done = () => {
      if (window.__opsLimitT.end == null && document.querySelector(item)) window.__opsLimitT.end = performance.now();
    };
    new MutationObserver(done).observe(document.body, { childList: true, subtree: true });
    done();
  }, [FILE_INPUT, ITEM]);
}

for (const csp of [true, false]) {
  const label = csp ? 'あり' : 'なし';

  test(`上限ちょうど（実操作）は開き、同じ件数の合成の履歴は件数では断られない（CSP ${label}）`, async (t) => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const opened = await freshPage(srv);
      const { page } = opened;
      const base = await baseHistory(page);
      assert.ok(base.length < MAX_BYTES, `base bytes ${base.length} within the size limit`);
      assert.equal(JSON.parse(base).ops.length, LIMIT, 'ops count');
      await call(page, 'importInstall');

      // 合成（件数ちょうど）を先に入れる。件数では断られず、理由は too_many_ops 以外になる。
      await page.setInputFiles(FILE_INPUT, writeTmp('synthetic.json', withOps(base, synthetic(LIMIT))));
      await page.waitForFunction(() => {
        const e = JSON.parse(window.__fandheTest.stateJson()).engine;
        return e && e.lastImport != null && e.lastImport.verdict !== 'open';
      }, null, { timeout: 15000 });
      const synth = (await engine(page)).lastImport;
      t.diagnostic(`synthetic at-limit: verdict=${synth.verdict} reason=${synth.reason}`);
      assert.notEqual(synth.reason, 'too_many_ops', 'the exact limit is not refused by the count');
      assert.equal(await page.locator(`${ITEM}[data-notice-reason="too_many_ops"]`).count(), 0, 'no too_many_ops notice');

      // 実操作の履歴は開く。
      const before = (await engine(page)).installCount;
      await page.setInputFiles(FILE_INPUT, writeTmp('exact.json', base));
      await page.waitForFunction((n) => {
        const e = JSON.parse(window.__fandheTest.stateJson()).engine;
        return e && e.lastImport != null && e.lastImport.verdict === 'open' && e.installCount >= n;
      }, before, { timeout: 30000 });
      assert.equal((await engine(page)).lastImport.verdict, 'open', 'exact limit history opens');
      assertQuiet(opened.rec, 'exact');
      await opened.context.close();
    }));
  });

  for (const kind of ['over-by-one', 'tenfold']) {
    test(`${kind} の履歴は too_many_ops で 1 秒以内に断られ、状態も保存先も変わらない（CSP ${label}）`, async (t) => {
      await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const opened = await freshPage(srv);
        const { page } = opened;
        const base = await baseHistory(page);
        const text = kind === 'over-by-one'
          ? withOps(base, [...JSON.parse(base).ops, { draft: 'x' }])
          : withOps(base, synthetic(LIMIT * 10));
        assert.ok(text.length < MAX_BYTES, `${kind}: ${text.length} bytes is under the size limit (the count must be the reason)`);
        const file = writeTmp(`${kind}.json`, text);

        await openStorage(page);
        await page.locator(INPUT).fill(OPERATION);
        await page.locator(INPUT).press('Enter');
        await settle(page);
        await page.waitForFunction(([p, s]) => window.__fandheTest.storageRead(p, s) != null, [PACK, START], { timeout: 10000 });
        // 下書きの変更も遅延保存される。書き込みが落ち着いてから基準を読む（途中の値を基準にしない）。
        await page.locator(INPUT).fill('keep');
        await page.waitForTimeout(1500);
        const storedBefore = await call(page, 'storageRead', PACK, START);
        const fpBefore = fingerprint(await engine(page));
        const installBefore = (await engine(page)).installCount;

        await call(page, 'importInstall');
        await armNoticeTimer(page);
        await page.setInputFiles(FILE_INPUT, file);
        await page.waitForFunction(() => window.__opsLimitT.end != null, null, { timeout: 15000 });
        const { start, end } = await page.evaluate(() => window.__opsLimitT);
        const ms = end - start;
        t.diagnostic(`${kind}: bytes=${text.length} notice after ${ms.toFixed(1)}ms`);
        assert.ok(start != null && ms < BUDGET_MS, `${kind}: reason shown within ${BUDGET_MS}ms (${ms}ms)`);

        await settle(page);
        const e = await engine(page);
        assert.equal(e.lastImport.verdict, 'refused', `${kind}: verdict`);
        assert.equal(e.lastImport.reason, 'too_many_ops', `${kind}: reason`);
        assert.equal(await page.locator(ITEM).count(), 1, `${kind}: one notice`);
        assert.equal(await page.locator(ITEM).getAttribute('data-notice-reason'), 'too_many_ops', `${kind}: notice reason`);
        assert.equal((await page.locator(ITEM).textContent()).includes('too_many_ops'), true, `${kind}: notice text carries the code`);
        assert.equal(e.installCount, installBefore, `${kind}: no environment was built`);
        assert.equal(fingerprint(e) === fpBefore, true, `${kind}: state unchanged`);
        assert.equal(await page.locator(INPUT).inputValue(), 'keep', `${kind}: draft unchanged`);
        assert.equal((await call(page, 'storageRead', PACK, START)) === storedBefore, true, `${kind}: stored value untouched`);

        await page.locator(INPUT).fill(OPERATION);
        await page.locator(INPUT).press('Enter');
        await settle(page);
        assert.equal(fingerprint(await engine(page)) !== fpBefore, true, `${kind}: operation still works`);
        await page.locator(DISMISS).click();
        assert.equal(await page.locator(ITEM).count(), 0, `${kind}: notice dismissed`);
        assertQuiet(opened.rec, kind);
        await opened.context.close();
      }));
    });
  }

  // 保存先から起動する経路。再読み込みの開始（ナビゲーションの起点）から bootKind が決まるまでをページの中で計る。
  const bootTest = (name, count, want) => test(`${name}（CSP ${label}）`, async (t) => {
    await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
      const opened = await freshPage(srv);
      const { page } = opened;
      await openStorage(page);
      const written = await page.evaluate(async ([n, pack, start]) => {
        const t = window.__fandheTest;
        // 上限以下は実操作の履歴をそのまま作る（切り詰めると適用し直しの写しと合わなくなる）。超過は末尾に足す。
        let text = t.opsHistoryText(Math.min(n, 20000));
        if (n > 20000) {
          const v = JSON.parse(text);
          v.ops.push(...Array.from({ length: n - v.ops.length }, () => ({ draft: 'x' })));
          text = JSON.stringify(v);
        }
        return { r: await t.storageWrite(pack, start, text), len: text.length };
      }, [count, PACK, START]);
      assert.equal(JSON.parse(written.r).ok, true, 'stored history written');
      assert.ok(written.len < MAX_BYTES, `stored bytes ${written.len} within the size limit`);
      // 大きい値は IndexedDB への非同期の書き込みになりうる。確定を待ってから再読み込みする。
      await page.waitForTimeout(1500);
      const st = JSON.parse(await call(page, 'storageState', PACK, START));
      t.diagnostic(`storage after write: ${JSON.stringify(st)}`);
      assert.equal(st.lastAsyncError ?? null, null, 'no async storage error');

      // 判定の終点は「#app に最初の子が入った時刻」（boot の直後の同じタスクでのマウント）。試験の口（test-hooks.js）の動的 import と
      // rAF の間隔は起動そのものではなく、CI で 1 秒を 26〜34ms 超えた揺れの源だったので区間に入れない（#1956）。
      // 旧い定義（bootKind が決まって rAF で見えた時刻）は poll として diagnostic にだけ残す。
      await page.addInitScript(() => {
        window.__bootT = { kind: '', at: null, mountAt: null };
        const mo = new MutationObserver(() => {
          const app = document.getElementById('app');
          if (app && app.childElementCount > 0 && window.__bootT.mountAt == null) {
            window.__bootT.mountAt = performance.now();
            mo.disconnect();
          }
        });
        mo.observe(document, { childList: true, subtree: true });
        const poll = () => {
          try {
            const k = window.__fandheTest && window.__fandheTest.bootKind();
            if (k === 'restored' || k === 'not_opened') { window.__bootT.kind = k; window.__bootT.at = performance.now(); return; }
          } catch { /* まだ組み込まれていない */ }
          requestAnimationFrame(poll);
        };
        poll();
      });
      await page.reload();
      await waitMounted(page);
      await page.waitForFunction(() => window.__bootT.at != null, null, { timeout: 15000 }).catch(async (err) => {
        const k = await page.evaluate(() => window.__fandheTest.bootKind());
        throw new Error(`boot kind never decided (now: ${JSON.stringify(k)}): ${err.message}`);
      });
      const boot = await page.evaluate(() => window.__bootT);
      t.diagnostic(`${name}: ops=${count} bytes=${written.len} boot=${boot.kind} mount=${boot.mountAt == null ? 'missing' : boot.mountAt.toFixed(1)}ms poll=${boot.at.toFixed(1)}ms from navigation start`);
      assert.equal(boot.kind, want.kind, `boot kind (${boot.kind})`);
      // 印が取れなかったら通さない（fail-closed）。
      assert.ok(typeof boot.mountAt === 'number', 'mount time was captured');
      assert.ok(boot.mountAt < BUDGET_MS, `opened within ${BUDGET_MS}ms (${boot.mountAt}ms)`);

      await settle(page);
      const e = await engine(page);
      if (want.kind === 'restored') {
        assert.equal(e.bootKind, 'restored', 'restored');
        assert.equal(await page.locator(ITEM).count(), 0, 'no refused notice');
      } else {
        assert.equal(e.bootKind, 'not_opened', 'not opened');
        // 起動時の通知は版の判定の段の理由だけで、件数の理由の案内は TASK-533 の範囲。ここでは通知の有無を固定しない。
        t.diagnostic(`boot refused notices: ${await page.locator(ITEM).count()}`);
        await page.locator(INPUT).fill(OPERATION);
        await page.locator(INPUT).press('Enter');
        await settle(page);
        await page.waitForTimeout(1500);
        await openStorage(page);
        const now = await call(page, 'storageRead', PACK, START);
        assert.equal(typeof now, 'string', 'the stored value remains');
        assert.equal(JSON.parse(now).ops.length, count, 'the stored value is untouched');
      }
      assertQuiet(opened.rec, name);
      await opened.context.close();
    }));
  });

  bootTest('19,990 件（実操作）の保存済みの履歴は再読み込みから 1 秒以内に開く', LIMIT - 10, { kind: 'restored' });
  bootTest('20,001 件の保存済みの履歴は 1 秒以内に断られ、保存先は変わらない', LIMIT + 1, { kind: 'not_opened' });
}
