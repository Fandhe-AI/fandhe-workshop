// ビューの切り替えと更新で、マウント根の要素が同一でフォーカスが保たれることの試験（TASK-470 2/2。ART-5・ART-12）。
//
// 試験の設定（TEST_HOOKS=1）の画面へ、見えない `raw-trigger` の固定の接頭辞 `load-fixture:views` で、Step ごとに
// 複数の種類のビューを宣言する教材（st-1: file_tree・card_list・check_list / st-2: card_list・file_tree /
// st-3: card_list）へ差し替えてから測る。保持した要素（根・部品の根・ビューの list 7 本・選択の親・入力欄）は、
// 操作の後も `===` で同一でなければならない。上流は dirty な field を束縛点に解決できないと根の子をまとめて
// 差し替える（全再描画）ため、同一性はその検出になる。
// - V1: 選択のボタンが宣言の順に出る。押したビューの list だけが見え、他の 6 本は項目 0 件で hidden を持つ（ART-12）
// - V2: ボタンの実クリックで 3 種を 2 往復しても、保持した要素が同一で、フォーカスが押したボタンに残る（ART-5）
// - V3: 入力欄にフォーカスを置いたまま、ビューの切り替え・同じ Step の更新・Step の移動をしても、保持した要素が
//   同一で、フォーカスが入力欄に残る。移動の後は新しい Step の宣言の先頭が選ばれる（ART-5・ART-12）
// - V4: 選択の属性（ボタンの選択状態・list の hidden・data-view・data-testid）が、再描画 101 回の後も
//   最初の写しと差 0 回である（ART-12）。ログの行は 4,096 件に届かない（上限の試験は TASK-359）
// - N1: 負の対照。代役の部品（fixture_control）で部品を項目の外に置くと、切り替えで根の子が置き換わる。
//   項目の中なら同一のまま。製品の画面には、外に置く口を作らない（作ると登録簿の契約に例外ができる）
// - P1: 前後の切り替え（ART-2 の Phase）の口は製品の画面に無いため、代役の部品で根の子の同一性を確かめる
// 呼び出し元は make test-browser（scripts/test-browser.sh）。1 件ごとに新しい context を使う。
//
// 入力: 環境変数 FANDHE_WEB_TEST_DIR（TEST_HOOKS=1 の tools/build-web.sh の出力）。未設定・不在は失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';

const BASE = '/fandhe-view-switch/';
const RAW = '[data-testid="raw-trigger"]';
const DRAFT = '[data-testid="draft-input"]';
const BTN = '[data-testid="view-select-button"]';
const LOG = '[data-testid="part-log-list"] > li';
const KINDS = ['file_tree', 'card_list', 'check_list'];
const REPEAT = 101;
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

// 文字列としてだけ渡し、コードの文字列を組み立てない。
function sendRaw(page, payload) {
  return page.evaluate(([sel, p]) => {
    const el = document.querySelector(sel);
    el.setAttribute('data-payload', p);
    el.click();
  }, [RAW, payload]);
}

// 画面を開き、教材を差し替えて st-1 の選択のボタンが出るまで待つ。
async function withViewsPage(fn) {
  await withServer({ dir: process.env.FANDHE_WEB_TEST_DIR, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    try {
      await page.goto(srv.url);
      await waitMounted(page);
      await sendRaw(page, 'load-fixture:views');
      await page.waitForFunction((b) => document.querySelectorAll(b).length === 3, BTN);
      await fn(page, rec);
      assert.deepEqual(nonZero(await collectSafety(page, rec)), []);
    } finally {
      await context.close();
    }
  });
}

const goTo = (page, step) => sendRaw(page, JSON.stringify({ control: { go_to: step } }));
const waitStep = (page, step) => page.waitForFunction((s) => JSON.parse(window.__fandheTest.stateJson()).engine.current === s, step);
const logCount = (page) => page.evaluate((s) => document.querySelectorAll(s).length, LOG);
const waitLogAbove = (page, n) => page.waitForFunction(([s, c]) => document.querySelectorAll(s).length > c, [LOG, n]);
const waitSelected = (page, kind) => page.waitForFunction(
  ([b, k]) => document.querySelector(`${b}[data-payload="${k}"]`).dataset.selected === 'true',
  [BTN, kind],
);

// 保持する要素を window.__held へ控える。比較は画面の中で行い、違う要素の名前の列を返す。
// 製品の画面（#app）の中だけを測る。代役の部品の list（#app の外）と混ざらない。
function hold(page) {
  return page.evaluate(() => {
    const q = (s) => document.querySelector(`#app ${s}`);
    const held = {
      root: document.querySelector('#app > [data-testid="workshop-root"]'),
      part: q('[data-testid="part-artifact-view"]'),
      select: q('[data-testid="view-select"]'),
      draft: q('[data-testid="draft-input"]'),
    };
    for (const l of document.querySelectorAll('#app [data-testid="view-list"]')) held[`list:${l.dataset.view}`] = l;
    window.__held = held;
    return Object.keys(held);
  });
}

function changed(page) {
  return page.evaluate(() => {
    const q = (s) => document.querySelector(`#app ${s}`);
    const now = {
      root: document.querySelector('#app > [data-testid="workshop-root"]'),
      part: q('[data-testid="part-artifact-view"]'),
      select: q('[data-testid="view-select"]'),
      draft: q('[data-testid="draft-input"]'),
    };
    for (const l of document.querySelectorAll('#app [data-testid="view-list"]')) now[`list:${l.dataset.view}`] = l;
    return Object.keys(window.__held).filter((k) => window.__held[k] !== now[k] || !window.__held[k].isConnected);
  });
}

const activeIs = (page, sel) => page.evaluate((s) => document.activeElement === document.querySelector(s), sel);

// 選択の属性の写し。ボタンの順・選択状態と、list 7 本の data-view・hidden・data-testid・項目数。
function selection(page) {
  return page.evaluate(() => JSON.stringify({
    buttons: [...document.querySelectorAll('#app [data-testid="view-select-button"]')]
      .map((b) => [b.dataset.payload, b.dataset.selected ?? null]),
    lists: [...document.querySelectorAll('#app [data-testid="view-list"]')]
      .map((l) => [l.dataset.view, l.hasAttribute('hidden'), l.dataset.testid, l.children.length]),
  }));
}

const shown = (page) => page.evaluate(() => [...document.querySelectorAll('#app [data-testid="view-list"]:not([hidden])')].map((l) => l.dataset.view));

test('V1: 選択のボタンは宣言の順に出て 押したビューの list だけが見える', async () => {
  await withViewsPage(async (page) => {
    const payloads = () => page.evaluate((b) => [...document.querySelectorAll(b)].map((x) => x.dataset.payload), BTN);
    assert.deepEqual(await payloads(), KINDS);
    for (const kind of [...KINDS, KINDS[0]]) {
      await page.locator(`${BTN}[data-payload="${kind}"]`).click();
      await waitSelected(page, kind);
      const s = JSON.parse(await selection(page));
      assert.deepEqual(await shown(page), [kind]);
      assert.deepEqual(s.buttons.filter(([, sel]) => sel === 'true').map(([p]) => p), [kind]);
      assert.equal(s.lists.length, 7);
      for (const [view, hidden, , n] of s.lists) {
        if (view !== kind) assert.deepEqual([hidden, n], [true, 0], view);
      }
    }
  });
});

test('V2: ボタンの実クリックで 3 種を 2 往復しても 要素が同一でフォーカスがボタンに残る', async () => {
  await withViewsPage(async (page) => {
    assert.equal((await hold(page)).length, 4 + 7);
    for (const kind of [...KINDS, ...KINDS, KINDS[0]]) {
      const sel = `${BTN}[data-payload="${kind}"]`;
      await page.locator(sel).click();
      await waitSelected(page, kind);
      assert.deepEqual(await changed(page), [], kind);
      assert.equal(await activeIs(page, sel), true, `${kind}: フォーカスがボタンに残る`);
      assert.equal(await page.evaluate((s) => document.querySelector(s).isConnected, sel), true);
    }
  });
});

test('V3: 入力欄にフォーカスを置いたまま 切り替え・更新・移動をしても要素が同一でフォーカスが残る', async () => {
  await withViewsPage(async (page) => {
    await page.locator(DRAFT).focus();
    await hold(page);
    const check = async (label) => {
      assert.deepEqual(await changed(page), [], label);
      assert.equal(await activeIs(page, DRAFT), true, `${label}: フォーカスが入力欄に残る`);
    };
    // (i) フォーカスを動かさない click で切り替える。
    for (const kind of [...KINDS, KINDS[0]]) {
      await page.evaluate(([b, k]) => document.querySelector(`${b}[data-payload="${k}"]`).click(), [BTN, kind]);
      await waitSelected(page, kind);
      await check(`select ${kind}`);
    }
    // (ii) 同じ Step・同じビューのまま中身（ログ）が変わる。
    let n = await logCount(page);
    await sendRaw(page, JSON.stringify({ input: { command: '/make-note Bad_Name' } }));
    await waitLogAbove(page, n);
    await check('same step update');
    // (iii) 入力欄から誤った入力を送る。
    n = await logCount(page);
    await page.fill(DRAFT, '/make-note Bad_Name');
    await page.keyboard.press('Enter');
    await waitLogAbove(page, n);
    await check('submit from the draft');
    // (iv) Step の移動。新しい Step の宣言の先頭が選ばれる。
    await page.evaluate((b) => document.querySelector(`${b}[data-payload="check_list"]`).click(), BTN);
    await waitSelected(page, 'check_list');
    await goTo(page, 'main/st-2');
    await waitStep(page, 'main/st-2');
    await check('move to st-2');
    assert.deepEqual(await shown(page), ['card_list']);
    // 宣言の集合が変わるため、ボタンも新しい Step の宣言に追従する。dirty の漏れそのものは native の試験
    // （view_select・変異 ART12-3）が固定する。ブラウザの本試験は、その漏れを検出できなかった（手元で確かめた。docs/testing.md 5.97）。
    const payloads = () => page.evaluate((b) => [...document.querySelectorAll(b)].map((x) => x.dataset.payload), BTN);
    assert.deepEqual(await payloads(), ['card_list', 'file_tree']);
    await goTo(page, 'main/st-3');
    await waitStep(page, 'main/st-3');
    await check('move to st-3');
    assert.deepEqual(await shown(page), ['card_list']);
    assert.deepEqual(await payloads(), []);
    await goTo(page, 'main/st-1');
    await waitStep(page, 'main/st-1');
    await check('back to st-1');
    assert.deepEqual(await shown(page), ['file_tree']);
    assert.deepEqual(await payloads(), KINDS);
  });
});

test(`V4: 選択の属性は再描画 ${REPEAT} 回の後も最初の写しと差 0 回`, async () => {
  await withViewsPage(async (page) => {
    await page.locator(`${BTN}[data-payload="card_list"]`).click();
    await waitSelected(page, 'card_list');
    await page.locator(DRAFT).focus();
    await hold(page);
    const first = await selection(page);
    assert.deepEqual(await shown(page), ['card_list']);
    let diffs = 0;
    let replaced = 0;
    for (let i = 0; i < REPEAT; i++) {
      const n = await logCount(page);
      await page.fill(DRAFT, '/make-note Bad_Name');
      await page.keyboard.press('Enter');
      await waitLogAbove(page, n);
      if ((await selection(page)) !== first) diffs += 1;
      if ((await changed(page)).length > 0) replaced += 1;
    }
    assert.equal(diffs, 0, `選択の属性の差 ${diffs} 回`);
    assert.equal(replaced, 0, `要素の置換 ${replaced} 回`);
    const total = await logCount(page);
    assert.ok(total >= REPEAT && total < 4096, `ログの行 ${total}`);
  });
});

// --- 代役の部品（#app の外の root）---

async function withFixtureRoot(fn) {
  await withViewsPage(async (page, rec) => {
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.viewFixtureMount), 'function', 'test hooks are missing');
    await page.evaluate(() => {
      const box = document.createElement('div');
      box.id = 'view-fixture-root';
      document.body.append(box);
    });
    const res = JSON.parse(await page.evaluate(() => window.__fandheTest.viewFixtureMount('view-fixture-root')));
    assert.deepEqual(res, { ok: true });
    await fn(page, rec);
  });
}

async function fixtureDispatch(page, name, payload) {
  const r = JSON.parse(await page.evaluate(([n, p]) => window.__fandheTest.viewFixtureDispatch(n, p), [name, payload]));
  assert.equal(r.outcome, 'applied', `${name}:${payload}`);
}

const holdFixtureRoot = (page) => page.evaluate(() => {
  window.__fixtureRoot = document.querySelector('#view-fixture-root').firstElementChild;
});
const fixtureRootSame = (page) => page.evaluate(() => {
  const now = document.querySelector('#view-fixture-root').firstElementChild;
  return now === window.__fixtureRoot && now.isConnected;
});

for (const [control, same] of [['inside', true], ['outside', false]]) {
  test(`N1: 部品が項目の${same ? '中' : '外'}にあると 切り替えで根の子は${same ? '同一のまま' : '置き換わる'}`, async () => {
    await withFixtureRoot(async (page) => {
      await hold(page);
      await fixtureDispatch(page, 'fixture_select', 'card_list:a');
      await fixtureDispatch(page, 'fixture_control', control);
      await holdFixtureRoot(page);
      await fixtureDispatch(page, 'fixture_phase', 'after');
      await fixtureDispatch(page, 'fixture_select', 'file_tree:b');
      await fixtureDispatch(page, 'fixture_select', 'card_list:a');
      assert.equal(await fixtureRootSame(page), same);
      // 製品の画面の根は、代役の部品の状態に関わらず同一のままである（同じ測り方で別の根を測る）。
      assert.deepEqual(await changed(page), []);
    });
  });
}

test('P1: 代役の部品で前後を 2 往復しても 根の子は同一', async () => {
  await withFixtureRoot(async (page) => {
    await fixtureDispatch(page, 'fixture_select', 'file_tree:b');
    await fixtureDispatch(page, 'fixture_control', 'inside');
    await holdFixtureRoot(page);
    for (const phase of ['after', 'before', 'after', 'before']) {
      await fixtureDispatch(page, 'fixture_phase', phase);
      assert.equal(await fixtureRootSame(page), true, phase);
    }
  });
});
