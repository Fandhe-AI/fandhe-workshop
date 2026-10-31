// 上部のチェックポイントの保存と復帰の操作の試験（SHELL-13。TASK-529 1/2）。
//
// 試験用の教材 `ch0-3`（`?fixture=ch0-3-chapters`）で、実際のボタンの click だけで「保存 → 先へ進む → 復帰」を通し、
// 復帰の後に左の成果物と右の位置（現在の Step・上部の位置の表示）が保存した時点に戻ることを確かめる。保存と復帰に `raw-trigger` は使わない。
//   場面 A  CH-1 の最初の Step で保存し、4 番目の Step まで進めてから復帰する。位置の期待値は IR から独自に数える
//   場面 B  復帰のボタンの payload を書き換えた改ざん（存在しない ID・正規形でない値）が状態とログを変えない
//   場面 D  配信する設定（IR なし）では項目が 0 件。幅 360px・959px でも保存のボタンが見え、横スクロールが無い
// 表示だけで開いた履歴での 0 件は host の層の試験（crates/web/tests/checkpoint_ops_host.rs）が確かめる。
// 差分の印（mark）の属性は復帰で変わりうる表示の状態なので比べない（restore-cancel と同じ）。試験の出力に、入力した値は出さない。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）・FANDHE_WEB_DIR（配信する設定）。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, summarize, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import {
  ledger, mouseDriver, journey, engine, ports,
} from './ch0-3-journey.mjs';

const BASE = '/fandhe-checkpoint-restore/';
const QUERY = '?fixture=ch0-3-chapters';
const SAVE = '[data-testid="checkpoint-save"]';
const RESTORE = '[data-testid="checkpoint-restore"]';
const POSITION = '[data-testid="shell-position"]';
const ROWS = '[data-testid="part-artifact-view"] [data-row]';
let browser;

const IR = JSON.parse(fs.readFileSync(
  path.join(path.dirname(fileURLToPath(import.meta.url)), '../../crates/web/tests/data/ch0_3/ir.json'),
  'utf8',
));

// IR から独自に数えた位置の文字。
function expectedPosition(uid) {
  const node = IR.nodes.find((n) => n.uid === uid);
  const list = IR.nodes.filter((n) => n.scope === 'main' && n.unit === node.unit);
  return `${node.unit}　Step ${list.findIndex((n) => n.uid === uid) + 1} / ${list.length}`;
}

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// 左の行の列（data-row と文字）と上部の位置の文字・属性。
const observe = (page) => page.evaluate(([rows, pos]) => {
  const p = document.querySelector(pos);
  return {
    rows: [...document.querySelectorAll(rows)].map((r) => [r.getAttribute('data-row'), r.textContent]),
    position: p ? [p.textContent, p.getAttribute('data-position-m'), p.getAttribute('data-position-k')] : null,
  };
}, [ROWS, POSITION]);

// 手動の保存を押し、増えた 1 件の ID を返す。章の終わりの自動保存（SAVE-31）が先に ID を使うため、ID は固定の 1 にできない。
const listedIds = (page) => page.evaluate((sel) => [...document.querySelectorAll(sel)].map((b) => b.getAttribute('data-checkpoint')), RESTORE);
async function manualSave(page) {
  const before = await listedIds(page);
  const n = before.length;
  await page.click(SAVE);
  await page.waitForFunction(([sel, k]) => document.querySelectorAll(sel).length === k + 1, [RESTORE, n]);
  const fresh = (await listedIds(page)).filter((i) => !before.includes(i));
  assert.equal(fresh.length, 1, '手動の保存で増えた項目が 1 件');
  return fresh[0];
}

const checkpointCount = async (page) => Object.keys((await engine(page)).stateWithoutLog.checkpoints).length;

async function openScene(csp, viewport, query, fn, { dir = process.env.FANDHE_WEB_TEST_DIR, hooks = true } = {}) {
  await withCspMode({ dir, csp }, (d) => withServer({ dir: d, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport });
    const L = ledger();
    await page.goto(`${srv.url}${query}`);
    await waitMounted(page);
    await fn(page, L);
    const s = summarize(rec);
    L.check('共通', 'ページ内のエラーが 0 件', s.pageErrors === 0);
    L.check('共通', 'CSP 違反が 0 件', s.csp === 0);
    if (hooks) assert.deepEqual(nonZero(await collectSafety(page, rec)), [], JSON.stringify({ rec }));
    console.log(`チェックポイント (CSP ${csp ? 'あり' : 'なし'}, ${viewport.width}px): ${L.summary()} 項目`);
    assert.deepEqual(L.failed(), [], JSON.stringify({ rec }));
    await context.close();
  }));
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`保存 → 先へ進む → 復帰で、左の成果物と右の位置が保存の時点に戻る（${mode}）`, { timeout: 300000 }, async () => {
    await openScene(csp, { width: 1280, height: 800 }, QUERY, async (page, L) => {
      let saved = null;
      let portsAtSave = null;
      let cpId = null;
      let autoN = 0;
      const onStep = async (uid, status) => {
        if (uid !== 'main/st-1-1' || status !== 'entered') return;
        L.check('A', '保存のボタンが 1 個・復帰のボタンは章の終わりの自動保存の分だけ',
          (await page.locator(SAVE).count()) === 1);
        const place = await page.evaluate((s) => {
          const b = document.querySelector(s);
          return { inTop: !!b.closest('[data-testid="shell-top"]'), inPanel: !!b.closest('[role="tabpanel"]') };
        }, SAVE);
        L.check('A', '保存のボタンが shell-top の中・tabpanel の外', place.inTop && !place.inPanel);
        const autoCount = (await engine(page)).stateWithoutLog.checkpoints;
        autoN = Object.keys(autoCount).length;
        cpId = await manualSave(page);
        saved = await observe(page);
        portsAtSave = await ports(page);
        L.check('A', '保存の時点の位置が IR から数えた形', saved.position?.[0] === expectedPosition('main/st-1-1'));
        L.check('A', '保存でチェックポイントが 1 件増える', (await checkpointCount(page)) === autoN + 1);
      };
      await journey(page, mouseDriver(page), 'A', L, 'main/st-1-4', { chaptered: true, onStep });
      assert.ok(saved, '保存の時点の観察が取れていない');
      const moved = await observe(page);
      L.check('A', '対照: 進めた後は位置と成果物が保存の時点と違う',
        moved.position?.[0] !== saved.position[0] && JSON.stringify(moved.rows) !== JSON.stringify(saved.rows));
      const e0 = await engine(page);
      const restored0 = e0.log.filter((r) => r.kind === 'restored').length;
      await page.click(`${RESTORE}[data-checkpoint="${cpId}"]`);
      await page.waitForFunction(() => JSON.parse(window.__fandheTest.stateJson()).engine.current === 'main/st-1-1');
      const e = await engine(page);
      const back = await observe(page);
      L.check('A', '現在の Step が保存の時点', e.current === 'main/st-1-1');
      L.check('A', '位置の文字と属性が保存の時点', JSON.stringify(back.position) === JSON.stringify(saved.position)
        && back.position[0] === expectedPosition('main/st-1-1'));
      L.check('A', '左の行の列と文字が保存の時点', JSON.stringify(back.rows) === JSON.stringify(saved.rows));
      L.check('A', '世代が進む', e.stateWithoutLog.run.generation > e0.stateWithoutLog.run.generation);
      L.check('A', '復帰の記録が 1 件増える', e.log.filter((r) => r.kind === 'restored').length === restored0 + 1);
      L.check('A', '復帰のボタンが残る', (await page.locator(`${RESTORE}[data-checkpoint="${cpId}"]`).count()) === 1
        && (await checkpointCount(page)) >= autoN + 1);
      L.check('A', '入力の口の数が保存の時点', JSON.stringify(await ports(page)) === JSON.stringify(portsAtSave));
    });
  });

  test(`改ざんした復帰の payload は状態とログを変えない（${mode}）`, { timeout: 300000 }, async () => {
    await openScene(csp, { width: 1280, height: 800 }, QUERY, async (page, L) => {
      await journey(page, mouseDriver(page), 'B', L, 'main/st-1-1', { chaptered: true });
      await manualSave(page);
      for (const bad of ['999', '01', '0', '', '1x', '4294967296']) {
        const before = await engine(page);
        await page.evaluate(([sel, v]) => {
          const b = document.querySelector(sel);
          b.setAttribute('data-payload', v);
          b.click();
        }, [RESTORE, bad]);
        await page.waitForTimeout(50);
        const after = await engine(page);
        L.check('B', `payload ${JSON.stringify(bad)} で状態とログが変わらない`,
          JSON.stringify(after.stateWithoutLog) === JSON.stringify(before.stateWithoutLog) && after.log.length === before.log.length);
      }
    });
  });
}

test('幅 360px と 959px で保存のボタンが見え、横スクロールが無い', { timeout: 300000 }, async () => {
  for (const width of [360, 959]) {
    await openScene(true, { width, height: 800 }, QUERY, async (page, L) => {
      const m = await page.evaluate((s) => {
        const b = document.querySelector(s);
        const r = b?.getBoundingClientRect();
        return {
          visible: !!r && r.width > 0 && r.height > 0,
          scroll: document.documentElement.scrollWidth <= document.documentElement.clientWidth,
        };
      }, SAVE);
      L.check(`D${width}`, '保存のボタンが見える', m.visible);
      L.check(`D${width}`, '横スクロールが無い', m.scroll);
    });
  }
});

test('配信する設定（IR なし）では保存・復帰のボタンが 0 件', { timeout: 300000 }, async () => {
  await openScene(true, { width: 1280, height: 800 }, '', async (page, L) => {
    L.check('D', '保存・復帰のボタンが 0 件', (await page.locator(`${SAVE}, ${RESTORE}`).count()) === 0);
  }, { dir: process.env.FANDHE_WEB_DIR, hooks: false });
});
