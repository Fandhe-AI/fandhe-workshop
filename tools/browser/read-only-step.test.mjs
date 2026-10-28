// 読むだけの Step・確認の問いだけの Step の画面の操作の回数を固定する試験（TASK-449。TERM-30・PACK-35・D-69）。
//
// 解釈: D-69 は「入力のない確認を表す語は足さず、選択肢が 1 つだけの `choice` に確認 `answer` を続けて書く」を採った。
// このため画面では、1 択を選ぶ操作が 1 回増える（限界として受け入れ済み）。本試験は「足さない案」の現状を固定する。
// 将来、新しい決定なしに「1 択を自動で受け付ける」「確認の問いを最初から出す」変更が入ると落ちる。
// `Session` の層の同じ観察は `crates/pack/tests/read_only_step.rs` の `pack35_one_extra_operation_then_answer` が持つ。
// 本試験は画面の層だけを持つ。
//
// 確かめること（`?fixture=ch0-3` の `main/st-0-2a`〜`c`。IR から機械的に選ぶ。CSP あり・なし × マウス・キーボード）:
//   - (a) Step に入った直後は 1 択のボタンがちょうど 1 件で文字が IR の `label`。確認の選択肢は 0 件
//   - (b) 1 択を 1 回押した後は確認待ちで、確認の選択肢が IR の並びと `label` で出る
//   - (c) 正答を 1 回押すと達成が 1 件ログに出る。1 Step の操作は合計 2 回、余計な操作は 1 回
//   - 対照: 誤答を 1 回挟むと、同じ Step に留まり操作は 3 回になる（数え方が回数の違いを検出できる）
//
// 確かめないこと:
//   - 新しい章の読むだけの Step（ST-6.2・ST-11.1・ST-11.2・ST-12.1）。教材が揃った後に本物の IR で流し直す
//   - `?fixture=ch4-9` の `st-6-3`（確認が `none` で D-69 の形ではない）
//   - 配信する設定（`?fixture=` は試験の設定だけにある）
//   - 試験の出力に、入力した値や本文は出さない（名前・件数・合否だけ）
//
// 入力: FANDHE_WEB_TEST_DIR（TEST_HOOKS=1）。未設定・不在は skip にせず失敗にする。
import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {
  launchBrowser, channelFromEnv, withServer, withCspMode, openPage, waitMounted, collectSafety, nonZero,
} from './harness.mjs';
import {
  CHOICE, APPROVAL, REJECTED, engine, at, ledger, waitCount, mouseDriver, keyboardDriver, ports, install,
} from './ch0-3-journey.mjs';
import { IR } from './shell-journey.mjs';

const BASE = '/fandhe-read-only-step/';
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

// IR から選ぶ対象。1 択の `choice` と確認 `answer` を持つ CH-0 の Step。次の Step は IR の並びで決まる。
const MAIN = IR.nodes.filter((n) => n.scope === 'main');
const TARGETS = MAIN.map((n, i) => ({ n, next: MAIN[i + 1]?.uid }))
  .filter(({ n }) => n.uid.startsWith('main/st-0-')
    && n.step.input?.kind === 'choice' && n.step.input.options.length === 1 && n.step.confirm?.kind === 'answer')
  .map(({ n, next }) => ({
    uid: n.uid,
    next,
    read: n.step.input.options[0],
    answers: n.step.confirm.options,
    right: n.step.confirm.options.find((o) => o.correct).id,
    wrong: n.step.confirm.options.find((o) => !o.correct).id,
  }));
const ST_01 = MAIN.find((n) => n.uid === 'main/st-0-1');

const texts = (page) => page.evaluate((sel) => [...document.querySelectorAll(sel)].map((b) => b.textContent ?? ''), CHOICE);
const dataIds = (page) => page.evaluate((sel) => [...document.querySelectorAll(sel)].map((b) => b.getAttribute('data-choice')), CHOICE);
const achieved = (e, uid) => e.log.filter((r) => r.kind === 'achieved' && r.step === uid).length;
const rejectedRows = (page) => page.locator(REJECTED).count();

// 入力を送る操作（choose・point）を数えるドライバの包み。
function counting(d) {
  const c = { ops: 0 };
  return {
    c,
    d: {
      ...d,
      choose: (...a) => { c.ops += 1; return d.choose(...a); },
      point: (...a) => { c.ops += 1; return d.point(...a); },
    },
  };
}

async function run({ csp, driver, name, miss }) {
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    const L = ledger();
    await page.goto(`${srv.url}?fixture=ch0-3`);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
    await install(page);
    L.check(name, '対象の Step が 3 件', TARGETS.length === 3 && TARGETS.every((t) => t.next));
    const { c, d } = counting(driver(page));
    // st-0-1 は固定の対象外。通すだけ。
    await at(page, 'main/st-0-1', 'entered');
    await d.choose(ST_01.step.input.options.find((o) => o.label === '右').id);
    await at(page, 'main/st-0-1', 'accepted');
    await d.point(ST_01.step.confirm.targets[0].path);
    const perStep = [];
    for (const t of TARGETS) {
      await at(page, t.uid, 'entered');
      const base = c.ops;
      // (a) 入った直後
      L.check(name, `${t.uid} (a) 1 択のボタンが 1 件で label の文字`,
        JSON.stringify(await texts(page)) === JSON.stringify([t.read.label])
        && JSON.stringify(await dataIds(page)) === JSON.stringify([t.read.id]));
      L.check(name, `${t.uid} (a) 確認の選択肢が最初の操作の前は 0 件・承認 0 件`,
        JSON.stringify(await ports(page)) === JSON.stringify([1, 0, 1, 1])
        && (await page.locator(CHOICE).count()) === 1 && (await page.locator(APPROVAL).count()) === 0);
      // (b) 1 択を 1 回押した後
      await d.choose(t.read.id);
      await at(page, t.uid, 'accepted');
      L.check(name, `${t.uid} (b) 操作 1 回で確認待ち`, c.ops - base === 1);
      L.check(name, `${t.uid} (b) 確認の選択肢が IR の並びと label で出る`,
        JSON.stringify(await texts(page)) === JSON.stringify(t.answers.map((o) => o.label))
        && JSON.stringify(await dataIds(page)) === JSON.stringify(t.answers.map((o) => o.id)));
      let e = await engine(page);
      L.check(name, `${t.uid} (b) 達成はまだ 0 件`, achieved(e, t.uid) === 0);
      // 対照: 誤答を挟む
      if (miss === t.uid) {
        await d.choose(t.wrong);
        await waitCount(page, REJECTED, 1);
        e = await engine(page);
        L.check(name, `${t.uid} 誤答は拒否の行 1 件で同じ Step に留まる`, (await rejectedRows(page)) === 1 && e.current === t.uid);
        L.check(name, `${t.uid} 誤答の後も確認の選択肢が残る`, (await page.locator(CHOICE).count()) === t.answers.length);
      }
      // (c) 正答
      await d.choose(t.right);
      await at(page, t.next, 'entered');
      e = await engine(page);
      L.check(name, `${t.uid} (c) 達成が 1 件`, achieved(e, t.uid) === 1);
      const ops = c.ops - base;
      perStep.push(ops);
      const want = miss === t.uid ? 3 : 2;
      L.check(name, `${t.uid} 操作の回数が ${want}`, ops === want);
    }
    const counts = await collectSafety(page, rec);
    for (const k of nonZero(counts)) L.check(name, `安全の計数 ${k} が 0`, false);
    L.check(name, '安全の計数がすべて 0', nonZero(counts).length === 0);
    console.log(`READ-ONLY-STEP ${name} csp=${csp ? 'on' : 'off'} ops-per-step=${perStep.join(',')} ledger=${L.summary()}`);
    assert.deepEqual(L.failed(), [], JSON.stringify({ rec }));
    await context.close();
  }));
}

for (const csp of [true, false]) {
  for (const [label, driver] of [['マウス', mouseDriver], ['キーボード', keyboardDriver]]) {
    test(`読むだけ・確認だけの Step は操作 2 回（${label}・CSP ${csp ? 'あり' : 'なし'}）`, async () => {
      await run({ csp, driver, name: `${label}-${csp ? 'on' : 'off'}`, miss: null });
    });
  }
}

test('対照: 誤答を挟むと操作が 3 回になり数え方が違いを検出する（マウス・CSP あり）', async () => {
  await run({ csp: true, driver: mouseDriver, name: 'miss', miss: 'main/st-0-2b' });
});
