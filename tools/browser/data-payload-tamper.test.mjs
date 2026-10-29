// 操作要素の `data-payload` を書き換えて click する試験（TASK-148 1/2。SAFE-6）。
//
// 受講者が開発者ツールで属性を書き換えても、実行 0・外部通信 0・形が合わない値は受け付けられず状態が変わらず、タブが落ちないことを確かめる。
// 値は spec の PoC-7 の追補 4 の写し（`e5_data_payload`。XSS 6 件 + 壊れた値 7 件の 13 値）を data-payload-cases.mjs が読む。
// `submit-input` の `data-action` の書き換え（試験の設定・配信する設定。TASK-148 2/2。docs/testing.md 5.372）と、全体の click が PoC の 377 回以上の照合もここで行う。
//
// 対象は spec の 10 種ではなく現行の画面の口（対応は docs/testing.md 5.369）。場面ごとに root 内の `[data-payload]` の `data-action` の集合が
// 閉じた一覧とちょうど一致することを確かめ（新しい口が試験されずに紛れ込まない）、各種類の全インスタンスを 13 値で書き換える。
// 期待は decode_action の実装から分類した D（復号で断る・全て同一）・J（core が断る・状態は同一でログは rejected だけ）・I（payload を読まない・対照と同じ）。
// 試験の設定だけの `raw-trigger` は正当に状態を変えるため対象から名前つきで除く。配信する設定には 0 件で、E4 は除く理由の記録。
// 場面への到達は実際の click だけで行い、`raw-trigger` は使わない。試験の出力に入力した値は出さない（label だけ）。
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）・FANDHE_WEB_DIR・FANDHE_WEB_AUTHOR_DIR（IR を同梱した配信する設定）。どちらも未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, summarize, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import {
  ledger, mouseDriver, journey, engine,
} from './ch0-3-journey.mjs';
import { loadDataPayloadCases } from './data-payload-cases.mjs';
import {
  ROOT, RAW_TESTID, CLASS_OF, diffInventory, inventory, absentCounts, observe, dangerCount, sameState, tamperClick, tamperGroup,
  ACTION_TAMPER_NAMES, sameStateWithDraft, tamperActionClick,
} from './data-payload-tamper.mjs';

const BASE = '/fandhe-data-payload/';
const QUERY = '?fixture=ch0-3-chapters';
const SAVE = '[data-testid="checkpoint-save"]';
const RESTORE = '[data-testid="checkpoint-restore"]';
const DRAFT = '[data-testid="draft-input"]';
const SUBMIT = '[data-testid="submit-input"]';
const CAND = '[data-testid="candidate-btn"]';
const FIREFOX = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:125.0) Gecko/20100101 Firefox/125.0';
const BASE_ACTIONS = ['save-checkpoint', 'select-pane', 'select-view'];
let browser;
let cases;
const tally = { clicks: 0, skipped: 0, actionClicks: 0, actionScenes: 0 };
// PoC-7 の 29 要素 × 13 値。現行の画面に合わせた目標ではなく `data-payload` の書き換えの下限（実測は 2,458 回）。
const POC_CLICKS = 377;
const ACTION_SCENES = 6;
// 受け付けられない文字の下書き。書き換えない送信の対照では入力欄・ログが変わる。
const DRAFT_TEXT = '/x';
// 試験の設定で受け付けられる正しい `Op`（st-0-1 の正答）。`raw` と組むと試験の設定だけ状態が変わる。
const VALID_OP = '{"input":{"choice":"opt-right"}}';

function assertDir(name, hint) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to ${hint}`);
}

before(async () => {
  assertDir('FANDHE_WEB_DIR', 'a web directory built without TEST_HOOKS');
  assertDir('FANDHE_WEB_TEST_DIR', 'a web directory built with TEST_HOOKS=1');
  assertDir('FANDHE_WEB_AUTHOR_DIR', 'a web directory built with IR_FILE and without TEST_HOOKS');
  cases = loadDataPayloadCases();
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
  console.log(`data-payload の書き換え: click ${tally.clicks} 回（値が元と一致して除いた組 ${tally.skipped}）。data-action の書き換え: click ${tally.actionClicks} 回`);
});

const groupSelector = (action) => `${ROOT} [data-payload][data-action="${action}"]:not([data-testid="${RAW_TESTID}"])`;
const tuples = (inv) => JSON.stringify(inv.items.map((i) => [i.action, i.payload]));

// 場面の全体: 閉じた一覧との一致 → 全種類の書き換え → 書き換えた値が残っていないこと。
async function sweep(page, L, hooks, scene, expected, extra = []) {
  const inv = await inventory(page);
  const diff = diffInventory(inv.items.map((i) => i.action), expected);
  if (diff.missing.length || diff.extra.length) console.log(`${scene}: 一覧の差 ${JSON.stringify(diff)}`);
  L.check(scene, 'data-action の集合が閉じた一覧と一致', diff.missing.length === 0 && diff.extra.length === 0);
  L.check(scene, hooks ? 'raw-trigger がちょうど 1 件（対象から除く）' : 'raw-trigger が 0 件', inv.rawCount === (hooks ? 1 : 0));
  L.check(scene, '現行の画面に置かない口（tick・esc・ヒント）が 0 件', (await absentCounts(page)).every((n) => n === 0));
  const groups = expected.map((a) => ({ name: a, selector: groupSelector(a), cls: CLASS_OF[a] }));
  for (const g of [...groups, ...extra]) {
    assert.ok(g.cls, `${g.name}: 分類が無い`);
    const r = await tamperGroup(page, hooks, { ...g, values: cases.values });
    tally.clicks += r.clicks;
    tally.skipped += r.skipped;
    if (r.failures.length) console.log(`${scene}/${g.name}: ${r.failures.slice(0, 8).join(' | ')}`);
    L.check(scene, `${g.name}（${g.cls}）: ${r.instances} 要素 × 値で期待どおり`, r.failures.length === 0 && r.instances > 0 && r.clicks > 0);
  }
  const post = await inventory(page);
  if (tuples(post) !== tuples(inv)) {
    const a = new Set(inv.items.map((i) => `${i.action}|${i.payload}`));
    const b = new Set(post.items.map((i) => `${i.action}|${i.payload}`));
    console.log(`${scene}: 口の差 ${JSON.stringify([[...a].filter((x) => !b.has(x)), [...b].filter((x) => !a.has(x))])}`);
  }
  L.check(scene, '書き換えた値が残っていない', tuples(post) === tuples(inv));
}

async function openScene(csp, { dir = process.env.FANDHE_WEB_TEST_DIR, query = QUERY, viewport = { width: 1280, height: 800 }, userAgent, hooks = true }, fn) {
  await withCspMode({ dir, csp }, (d) => withServer({ dir: d, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin, viewport, userAgent });
    const L = ledger();
    await page.goto(`${srv.url}${query}`);
    await waitMounted(page);
    await fn(page, L, rec);
    const s = summarize(rec);
    L.check('共通', 'ページ内のエラーが 0 件', s.pageErrors === 0);
    L.check('共通', 'CSP 違反が 0 件', s.csp === 0);
    L.check('共通', 'タブが落ちていない', !page.isClosed() && s.crashed === 0);
    assert.deepEqual(nonZero(await collectSafety(page, rec)), [], JSON.stringify({ rec: s }));
    console.log(`data-payload (CSP ${csp ? 'あり' : 'なし'}, hooks ${hooks}): ${L.summary()} 項目`);
    assert.deepEqual(L.failed(), [], JSON.stringify({ rec: s }));
    await context.close();
  }));
}

const SUBMIT_ALL = `${ROOT} [data-testid="submit-input"]`;

// `submit-input` の `data-action` を 4 つの名前に書き換え、click と Enter の 2 経路で押す。期待は D（画面・状態・ログ・保存先・入力欄が同一）。
// 戻り値は押した回数。hooks が無い配信する設定では状態とログは読まず、画面・保存先・入力欄を比べる。
async function actionScene(page, L, scene, hooks) {
  L.check(scene, 'submit-input がちょうど 1 件で data-action が submit', await page.evaluate((sel) => {
    const e = document.querySelectorAll(sel);
    return e.length === 1 && e[0].getAttribute('data-action') === 'submit';
  }, SUBMIT_ALL));
  await page.fill(DRAFT, DRAFT_TEXT);
  let n = 0;
  for (const action of ACTION_TAMPER_NAMES) {
    for (const via of ['click', 'enter']) {
      const r = await tamperActionClick(page, hooks, SUBMIT, { action, via });
      assert.ok(!r.gone, `${scene}/${action}/${via}: 要素が消えた`);
      n += 1;
      L.check(scene, `data-action=${action}（${via}）: 画面・状態・ログ・保存先・入力欄が同一`, sameStateWithDraft(r.before, r.after));
      L.check(scene, `data-action=${action}（${via}）: 禁止の要素が増えない`, r.danger.before === r.danger.after);
    }
  }
  tally.actionClicks += n;
  L.check(scene, '書き換えた data-action が残っていない', await page.evaluate((sel) => [...document.querySelectorAll(sel)].every((e) => e.getAttribute('data-action') === 'submit'), SUBMIT_ALL));
  return n;
}

for (const csp of [true, false]) {
  const mode = `CSP ${csp ? 'あり' : 'なし'}`;

  test(`開いた直後（選択肢・保存・タブ・ビューの切り替え）の書き換え（${mode}）`, { timeout: 600000 }, async () => {
    await openScene(csp, {}, (page, L) => sweep(page, L, true, 'S1', [...BASE_ACTIONS, 'choose']));
  });

  test(`幅 959px の tablist の書き換え（${mode}）`, { timeout: 600000 }, async () => {
    await openScene(csp, { viewport: { width: 959, height: 800 } }, async (page, L) => {
      const visible = await page.evaluate(() => {
        const e = document.querySelector('[data-testid="pane-tabs"]');
        return !!e && getComputedStyle(e).display !== 'none';
      });
      L.check('S1w', 'tablist が見えている', visible);
      await sweep(page, L, true, 'S1w', [...BASE_ACTIONS, 'choose']);
    });
  });

  test(`章の終わり・保存後の復帰・指せる行・送信・候補の書き換え（${mode}）`, { timeout: 600000 }, async () => {
    await openScene(csp, {}, async (page, L) => {
      const real = mouseDriver(page);
      const d = {
        ...real,
        async nextChapter() {
          await sweep(page, L, true, 'N', [...BASE_ACTIONS, 'next-chapter', 'ui']);
          return real.nextChapter();
        },
      };
      const onStep = async (uid, status) => {
        if (uid !== 'main/st-1-1' || status !== 'entered') return;
        await page.click(SAVE);
        await page.waitForSelector(`${RESTORE}[data-checkpoint="1"]`);
      };
      await journey(page, d, 'A', L, 'main/st-1-4', { chaptered: true, onStep });
      await sweep(page, L, true, 'A2', [...BASE_ACTIONS, 'ui', 'restore-checkpoint'], [
        { name: 'submit-input', selector: SUBMIT, cls: 'I' },
      ]);
      // 直前の送信の確認で積んだ履歴の保存（遅延して書かれる）が、候補の対照の click の前後に混ざらないよう待つ。
      await page.waitForTimeout(1500);
      await page.fill(DRAFT, '/');
      await page.waitForSelector(CAND);
      const r = await tamperGroup(page, true, { name: 'candidate-btn', selector: CAND, cls: 'I', values: cases.values, draft: { selector: DRAFT, text: '/' } });
      tally.clicks += r.clicks;
      if (r.failures.length) console.log(`A2/candidate-btn: ${r.failures.slice(0, 8).join(' | ')}`);
      L.check('A2', `candidate-btn（I）: ${r.instances} 要素 × 値で期待どおり`, r.failures.length === 0 && r.instances > 0);
    });
  });

  test(`承認と「直す」の対象の書き換え（${mode}）`, { timeout: 600000 }, async () => {
    await openScene(csp, { query: '?fixture=ch0-3-revise&start=s-2-6' }, (page, L) => sweep(page, L, true, 'S3', [...BASE_ACTIONS, 'decide', 'revise-target']));
  });

  test(`枠の通知を閉じる口の書き換え（${mode}）`, { timeout: 600000 }, async () => {
    await openScene(csp, { userAgent: FIREFOX }, async (page, L) => {
      L.check('S4', '通知が 1 件出ている', (await page.locator('[data-testid="shell-notice-item"]').count()) === 1);
      await sweep(page, L, true, 'S4', [...BASE_ACTIONS, 'choose', 'dismiss-notice']);
    });
  });

  test(`submit-input の data-action の書き換え（試験の設定。${mode}）`, { timeout: 600000 }, async () => {
    await openScene(csp, {}, async (page, L) => {
      L.check('AS', '開いた直後は main/st-0-1', (await engine(page)).current === 'main/st-0-1');
      L.check('AS', '試験の設定には raw-trigger がある', (await page.locator(`[data-testid="${RAW_TESTID}"]`).count()) === 1);
      L.check('AS', '4 つの名前をそれぞれ click と Enter で押した', (await actionScene(page, L, 'AS', true)) === ACTION_TAMPER_NAMES.length * 2);
      tally.actionScenes += 1;
    });
  });

  test(`submit-input の data-action の書き換え（配信する設定・IR あり。${mode}）`, { timeout: 600000 }, async () => {
    await openScene(csp, { dir: process.env.FANDHE_WEB_AUTHOR_DIR, query: '', hooks: false }, async (page, L) => {
      L.check('AP', '__fandheTest が無い', (await page.evaluate(() => typeof window.__fandheTest)) === 'undefined');
      L.check('AP', 'raw-trigger が 0 件', (await page.locator(`[data-testid="${RAW_TESTID}"]`).count()) === 0);
      L.check('AP', '4 つの名前をそれぞれ click と Enter で押した', (await actionScene(page, L, 'AP', false)) === ACTION_TAMPER_NAMES.length * 2);
      // 配信する設定には raw の分岐が無い。正しい Op を payload に置いても何も起きない（SHELL-35 の振る舞いの面の証拠）。
      const r = await tamperActionClick(page, false, SUBMIT, { action: 'raw', payload: VALID_OP, via: 'click' });
      tally.actionClicks += 1;
      L.check('AP', 'raw と正しい Op の組でも画面・保存先・入力欄が同一', !r.gone && sameStateWithDraft(r.before, r.after));
      tally.actionScenes += 1;
    });
  });

  test(`submit-input の data-action の書き換え（配信する設定・IR なし。${mode}）`, { timeout: 600000 }, async () => {
    await openScene(csp, { dir: process.env.FANDHE_WEB_DIR, query: '', hooks: false }, async (page, L) => {
      const n = await page.locator(SUBMIT_ALL).count();
      console.log(`IR なしの配信する設定の submit-input: ${n} 件`);
      L.check('AN', 'submit-input の件数が実測（1 件）と一致', n === 1);
      L.check('AN', '__fandheTest が無い', (await page.evaluate(() => typeof window.__fandheTest)) === 'undefined');
      if (n === 1) await actionScene(page, L, 'AN', false);
      tally.actionScenes += 1;
    });
  });

  test(`配信する設定（IR なし）の口の書き換え（${mode}）`, { timeout: 600000 }, async () => {
    await openScene(csp, { dir: process.env.FANDHE_WEB_DIR, query: '', hooks: false }, async (page, L) => {
      const inv = await inventory(page);
      L.check('P', 'raw-trigger が 0 件', inv.rawCount === 0);
      L.check('P', 'tick・esc・ヒントの口が 0 件', (await absentCounts(page)).every((n) => n === 0));
      L.check('P', '__fandheTest が無い', (await page.evaluate(() => typeof window.__fandheTest)) === 'undefined');
      const actions = [...new Set(inv.items.map((i) => i.action))];
      for (const a of actions) assert.ok(CLASS_OF[a] === 'D', `${a}: 配信する設定で想定外の口`);
      for (const a of actions) {
        const r = await tamperGroup(page, false, { name: a, selector: groupSelector(a), cls: 'D', values: cases.values });
        tally.clicks += r.clicks;
        L.check('P', `${a}: 画面と保存先が変わらない`, r.failures.length === 0);
      }
    });
  });
}

test('対照: 変化の検出・文字だけの判定・閉じた一覧・raw-trigger の除外の意味', { timeout: 600000 }, async () => {
  await openScene(true, {}, async (page, L) => {
    // E1: 正規の payload の click は「変わった」と検出される（同じ観察の関数）。
    const ok = await tamperClick(page, true, groupSelector('save-checkpoint'), 0, null);
    L.check('E1', '保存の click で画面が変わると検出する', !sameState(ok.before, ok.after));
    const ch = await tamperClick(page, true, groupSelector('choose'), 0, null);
    L.check('E1', '選択肢の click でログが増えると検出する', ch.after.eng.kinds.length > ch.before.eng.kinds.length);

    // E2: 解釈される入力は禁止の要素として数える（切り離した要素。画面には入れない）。
    L.check('E2', '解釈される入力で禁止の要素を 3 件数える', (await dangerCount(page, '<img><svg></svg><iframe></iframe>')) === 3);
    L.check('E2', '文字だけの入力は 0 件', (await dangerCount(page, '&lt;img&gt;')) === 0);

    // E3: 未知の data-action は閉じた一覧との不一致になる。
    const inv = await inventory(page);
    const withUnknown = [...inv.items.map((i) => i.action), 'zzz-unknown'];
    L.check('E3', '未知の口で一覧の不一致を検出する', diffInventory(withUnknown, [...BASE_ACTIONS, 'choose']).extra.includes('zzz-unknown'));
    await page.evaluate((root) => {
      const b = document.createElement('button');
      b.setAttribute('data-action', 'zzz-unknown');
      b.setAttribute('data-payload', '');
      document.querySelector(root).appendChild(b);
    }, ROOT);
    const live = await inventory(page);
    L.check('E3', '足した未知の口を実際の画面でも数える', diffInventory(live.items.map((i) => i.action), [...BASE_ACTIONS, 'choose']).extra.includes('zzz-unknown'));

    // E4: raw-trigger の除外の意味（記録のみ。raw の挙動自体は固定しない）。
    const raw = await page.evaluate((id) => {
      const b = document.querySelector(`[data-testid="${id}"]`);
      return !!b;
    }, RAW_TESTID);
    L.check('E4', '試験の設定には raw-trigger がある（配信する設定は P で 0 件）', raw);
    const e0 = await engine(page);
    const o0 = await observe(page, true);
    await page.evaluate((id) => {
      const b = document.querySelector(`[data-testid="${id}"]`);
      b.setAttribute('data-payload', '{"command":"/x"}');
      b.click();
    }, RAW_TESTID);
    await page.waitForTimeout(200);
    const o1 = await observe(page, true);
    console.log(`E4 診断: raw-trigger の書き換えの click で状態が${sameState(o0, o1) ? '変わらなかった' : '変わった'}（ログ ${e0.log.length} -> ${o1.eng.kinds.length}）`);
  });
});

test('対照: 書き換えない送信は変化として検出され、試験の設定の raw と正しい Op は状態を変える', { timeout: 600000 }, async () => {
  await openScene(true, {}, async (page, L) => {
    // EA1: 同じ下書きで書き換えない送信を押すと、同じ観察の関数が変わったと検出する（D の判定の素通りの防止）。
    await page.fill(DRAFT, DRAFT_TEXT);
    const base = await observe(page, true);
    await page.evaluate((sel) => document.querySelector(sel).click(), SUBMIT);
    await page.waitForTimeout(500);
    const sent = await observe(page, true);
    L.check('EA1', '書き換えない送信で入力欄・ログが変わると検出する', !sameStateWithDraft(base, sent));
    // EA2: 試験の設定では raw と正しい Op の組が状態を変える。AP で何も起きないのは配信する設定に口が無いため、と示す。
    const r = await tamperActionClick(page, true, SUBMIT, { action: 'raw', payload: VALID_OP, via: 'click' });
    L.check('EA2', '試験の設定の raw と正しい Op で状態またはログが変わる', !r.gone && !sameState(r.before, r.after));
  });
});

// 全体の照合: 場面がすべて完了し、data-payload の click が PoC の 377 回以上、data-action の click が加わる。
// 各場面は collectSafety・CSP・ページ内エラーの 0 件を個別に assert しているため、場面がすべて通れば全体で実行 0・外部通信 0。
// --test-name-pattern で絞った実行ではこの照合が失敗する（fail-closed）。
test('総数: click の全体が 377 回以上で、すべての場面が完了した', () => {
  console.log(`click 合計: data-payload ${tally.clicks} 回 + data-action ${tally.actionClicks} 回 = ${tally.clicks + tally.actionClicks} 回`);
  assert.equal(tally.actionScenes, ACTION_SCENES);
  assert.ok(tally.clicks >= POC_CLICKS, 'data-payload の click が PoC の 377 回に届かない');
  assert.ok(tally.clicks + tally.actionClicks >= POC_CLICKS);
});
