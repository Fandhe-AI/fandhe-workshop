// 試験用クエリの無効化の試験（TASK-547 2/3。SHELL-35）。配信する設定の画面は、PoC 期の試験用クエリ 7 種
// （persist・store・manual・tick・os・osmode・flood）を付けて開いても、付けないときと挙動が変わらない。
//
// 入力: FANDHE_WEB_DIR（配信する設定。IR なし）と FANDHE_WEB_AUTHOR_DIR（配信する設定。compile した CH-0〜CH-3 の IR を同梱）。
// 未設定・不在は skip にせず失敗にする。CSP あり・なしの両方で流す。
//
// 確かめること（基準はクエリなしで開いた同じ配信物）:
//   Q1 マウント直後の画面（DOM だけの観察。readScreen）が基準と一致し、window.__fandheTest が無い。
//   Q2 取得した資源（メソッドとパス名の列）と localStorage のキーの集合が基準と一致する。
//   Q3 要求したタイマーの遅延に、クエリの数値（5・1000）が基準に無いのに現れない。
//   Q4 （IR 同梱の配信物）選択肢を 1 回押した後と再読み込みの後の画面・保存先のキーが基準と一致する（persist・store が保存と復元を変えない）。
//   Q5 安全の計数が 0、配信元以外への通信・ページ内エラーが 0 件。
//   対照 配信する設定が実際に読む唯一のクエリ `?start=` では Q1 の比較が不一致になる（比べ方が空振りでないことの証拠）。
//
// 確かめないこと:
//   - CH-0〜CH-3 の全量の通しと部品・配信物の検査（delivery-ports.test.mjs。TASK-547 3/3）。章の境目の件数は author-preview の B が持つ。
//   - 試験の設定の挙動（tick-interval が持つ）、配信物の試験用の口の検査（check-delivery.sh。TASK-82）
//   - URL を読まない構造の静的な証明（native の tick_interval.rs・delivery_ports.rs が持つ）
//
// 出力は名前・件数・合否だけで、DOM の文字と入力値は出さない。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero, summarize,
} from './harness.mjs';
import { readScreen, sha256 } from './screen-snapshot.mjs';

const BASE = '/fandhe-query-inert/';
const MAC_UA = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/153.0.0.0 Safari/537.36';
const NAMES = ['persist', 'store', 'manual', 'tick', 'os', 'osmode', 'flood'];
const QUERIES = [
  '?persist=0',
  '?store=memory',
  '?manual=1',
  '?tick=5',
  '?os=windows',
  '?osmode=windows',
  '?flood=1000',
  `?${NAMES.join('&')}`,
  '?persist=0&store=memory&manual=1&tick=5&os=windows&osmode=windows&flood=1000',
];
let browser;

function requireDir(name) {
  const dir = process.env[name];
  assert.ok(dir && fs.statSync(dir, { throwIfNoEntry: false })?.isDirectory(), `${name} must point to a built web directory`);
  return dir;
}

before(async () => {
  requireDir('FANDHE_WEB_DIR');
  requireDir('FANDHE_WEB_AUTHOR_DIR');
  browser = await launchBrowser({ channel: channelFromEnv() });
});

after(async () => {
  if (browser) await browser.close();
});

// ページが要求したタイマーの遅延を記録する（試験側の計測。配信物には何も足さない）。
const RECORD_TIMERS = () => {
  window.__delays = [];
  for (const name of ['setTimeout', 'setInterval']) {
    const orig = window[name].bind(window);
    window[name] = (fn, delay, ...rest) => {
      window.__delays.push(Number(delay ?? 0));
      return orig(fn, delay, ...rest);
    };
  }
};

const storageKeys = (page) => page.evaluate(async () => {
  const dbs = typeof indexedDB.databases === 'function' ? (await indexedDB.databases()).map((d) => `idb:${d.name}`) : [];
  return [...Object.keys(localStorage), ...dbs].sort();
});

// 1 回開いて観察を返す。interact が真なら、選択肢を 1 回押して落ち着いてから再読み込みの後も観察する。
async function observe(srv, query, { interact }) {
  const { page, context, rec } = await openPage(browser, { origin: srv.origin, userAgent: MAC_UA });
  try {
    await page.addInitScript(RECORD_TIMERS);
    await page.goto(`${srv.url}${query}`);
    await waitMounted(page);
    const out = {
      shot: sha256(JSON.stringify(await page.evaluate(readScreen))),
      hook: await page.evaluate(() => typeof window.__fandheTest),
      delays: await page.evaluate(() => window.__delays),
      keys: await storageKeys(page),
    };
    if (interact) {
      const btn = page.locator('button[data-action="choose"]').first();
      assert.ok((await btn.count()) > 0, '選択肢のボタンが無い');
      await btn.click();
      await page.waitForTimeout(600);
      out.afterClick = sha256(JSON.stringify(await page.evaluate(readScreen)));
      out.keysAfterClick = await storageKeys(page);
      await page.reload();
      await waitMounted(page);
      out.afterReload = sha256(JSON.stringify(await page.evaluate(readScreen)));
    }
    out.requests = rec.requests.map((r) => `${r.url.replace(srv.origin, '').split('?')[0]}:${r.status}`).sort();
    const counts = await collectSafety(page, rec);
    const bad = nonZero(counts);
    assert.deepEqual(bad, [], JSON.stringify(bad));
    const s = summarize(rec);
    const detail = JSON.stringify({ ...s });
    assert.equal(s.external, 0, detail);
    assert.equal(s.pageErrors, 0, detail);
    assert.equal(s.csp, 0, detail);
    return out;
  } finally {
    await context.close();
  }
}

for (const csp of [true, false]) {
  for (const [label, envName, interact] of [
    ['IR なし', 'FANDHE_WEB_DIR', false],
    ['IR 同梱', 'FANDHE_WEB_AUTHOR_DIR', true],
  ]) {
    test(`配信する設定(${label})は試験用クエリを無視する (csp=${csp})`, async () => {
      await withCspMode({ dir: process.env[envName], csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
        const base = await observe(srv, '', { interact });
        assert.equal(base.hook, 'undefined');
        for (const query of QUERIES) {
          const got = await observe(srv, query, { interact });
          const at = `クエリの組 ${QUERIES.indexOf(query)}`;
          assert.equal(got.hook, 'undefined', at);
          assert.equal(got.shot, base.shot, `${at}: Q1 マウント直後の画面`);
          assert.deepEqual(got.keys, base.keys, `${at}: Q2 保存先のキー`);
          assert.deepEqual(got.requests, base.requests, `${at}: Q2 取得した資源`);
          for (const d of [5, 1000]) {
            assert.ok(base.delays.includes(d) || !got.delays.includes(d), `${at}: Q3 タイマーの遅延 ${d}`);
          }
          if (interact) {
            assert.equal(got.afterClick, base.afterClick, `${at}: Q4 操作の後の画面`);
            assert.deepEqual(got.keysAfterClick, base.keysAfterClick, `${at}: Q4 操作の後の保存先`);
            assert.equal(got.afterReload, base.afterReload, `${at}: Q4 再読み込みの後の画面`);
          }
        }
      }));
    });
  }
}

test('対照: 実際に読む ?start= では基準と画面が一致しない', async () => {
  await withServer({ dir: process.env.FANDHE_WEB_AUTHOR_DIR, base: BASE }, async (srv) => {
    const base = await observe(srv, '', { interact: false });
    const moved = await observe(srv, '?start=s-1-1', { interact: false });
    assert.notEqual(moved.shot, base.shot);
  });
});
