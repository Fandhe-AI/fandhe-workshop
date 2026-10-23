// 選択肢が表示名で出ることを、`st-1-4`・`st-2-7` を含む 20 Step の全場面で数える試験（TASK-446。TERM-25）。
//
// 解釈: 本物の教材（教材 repo の TASK-615・TASK-617・TASK-627）は本 repo に無い。そのため CH-0〜CH-3 の形の試験用の教材
// （`crates/web/tests/data/ch0_3/ir.json`。`?fixture=ch0-3`）の全 Step を通しで進め、選択肢のボタンが出る場面ごとに、
// 見える文字・アクセシブルな名前・一覧の ariaSnapshot にその Step の選択肢の ID が出る件数を数える。
// 本物の教材の全 Step の確認は、教材が揃った後に同じ試験を本物の IR で流し直す（記録は `docs/testing.md` 5.271）。
//
// 確かめること:
//   - `?fixture=ch0-3` を CSP あり・なしで通し、選択肢のボタン（入力方式 `choice` の入力待ち・確認 `answer` の確認待ち）が
//     出る全場面で、その Step の選択肢の ID が文字として出る件数が 0 件である
//   - `main/st-1-4`・`main/st-2-7` の確認待ちで、ボタンの文字の並びが IR の `label` の並びと一致する
//   - 期待値は IR から独自に数える（画面の data-* を読み返して合格にしない）。IR が持つ場面を 1 つも飛ばさない
//   - 対照 `?fixture=ch0-3-nolabel`（確認 `answer` の `label` を外した IR）では、数え方が ID の表示を検出する
//     （`answer` の選択肢の総数以上の件数）。これは決定（D-67）の前の「ID のまま出る」現状の観察を兼ねる
//
// 確かめないこと:
//   - 操作のログの受付の行。選んだ後の行は選択肢の ID を本文に出す（`log_lines.rs`）。観察値として件数だけ出し assert しない（範囲外）
//   - `label` の無い教材パックが断られること（`crates/pack/tests/answer_label_required.rs` が受け持つ）
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
  CHOICE, LOG, engine, ledger, mouseDriver, journey, install,
} from './ch0-3-journey.mjs';
import { IR, LAST } from './shell-journey.mjs';

const BASE = '/fandhe-choice-labels/';
const LIST = '[data-bind-list="offer_choices"]';
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

// IR から数えた期待。キーは `uid:phase`。確認 `answer` は確認待ち（accepted）、入力方式 `choice` は入力待ち（entered）。
function expectations() {
  const exp = new Map();
  for (const n of IR.nodes) {
    const input = n.step.input;
    if (input?.kind === 'choice') exp.set(`${n.uid}:entered`, { kind: 'input', options: input.options });
    const confirm = n.step.confirm;
    if (confirm?.kind === 'answer') exp.set(`${n.uid}:accepted`, { kind: 'answer', options: confirm.options });
  }
  return exp;
}

const EXPECTED = expectations();
const ANSWER_OPTIONS = [...EXPECTED.values()].filter((e) => e.kind === 'answer').reduce((a, e) => a + e.options.length, 0);

// 出ているボタンの見える文字・アクセシブルな名前・title と、一覧の ariaSnapshot を読む。
async function readButtons(page) {
  const buttons = await page.evaluate((sel) => [...document.querySelectorAll(sel)].map((b) => ({
    text: b.textContent ?? '',
    aria: b.getAttribute('aria-label') ?? '',
    title: b.getAttribute('title') ?? '',
  })), CHOICE);
  const list = page.locator(LIST);
  const snapshot = (await list.count()) > 0 ? await list.first().ariaSnapshot() : '';
  return { buttons, snapshot };
}

// 観察のみ。操作のログの行に選択肢の ID が出た行の数。
async function logIdHits(page, ids) {
  const lines = await page.evaluate((sel) => [...document.querySelectorAll(`${sel} li`)].map((li) => li.textContent ?? ''), LOG);
  return lines.filter((t) => ids.some((id) => t.includes(id))).length;
}

// 1 回の通し。計数を返す。
async function run({ csp, query, name, withLabel }) {
  let result;
  await withCspMode({ dir: process.env.FANDHE_WEB_TEST_DIR, csp }, (dir) => withServer({ dir, base: BASE }, async (srv) => {
    const { page, context, rec } = await openPage(browser, { origin: srv.origin });
    const L = ledger();
    await page.goto(`${srv.url}${query}`);
    await waitMounted(page);
    assert.equal(await page.evaluate(() => typeof window.__fandheTest?.stateJson), 'function', 'test hooks are missing');
    await install(page);
    const seen = new Set();
    const st = { buttons: 0, idHits: 0, answerIdHits: 0, st14: 'ng', st27: 'ng', logIdHits: 0 };
    const onStep = async (uid, phase) => {
      const key = `${uid}:${phase}`;
      const exp = EXPECTED.get(key);
      if (!exp) return;
      seen.add(key);
      const ids = exp.options.map((o) => o.id);
      const { buttons, snapshot } = await readButtons(page);
      st.buttons += buttons.length;
      L.check(name, `${key} ボタンの数が選択肢の数`, buttons.length === exp.options.length);
      let hits = 0;
      for (const id of ids) {
        for (const b of buttons) hits += [b.text, b.aria, b.title].filter((t) => t.includes(id)).length;
        if (snapshot.includes(id)) hits += 1;
      }
      st.idHits += hits;
      if (exp.kind === 'answer') st.answerIdHits += hits;
      const texts = buttons.map((b) => b.text);
      const wantLabels = exp.options.map((o) => o.label);
      if (withLabel) {
        L.check(name, `${key} ボタンの文字が label の並びと一致`, JSON.stringify(texts) === JSON.stringify(wantLabels));
      } else if (exp.kind === 'answer') {
        L.check(name, `${key} label が無いと ID の文字が出る（対照）`, JSON.stringify(texts) === JSON.stringify(ids));
      }
      const same = JSON.stringify(texts) === JSON.stringify(wantLabels) ? 'ok' : 'ng';
      if (key === 'main/st-1-4:accepted') st.st14 = same;
      if (key === 'main/st-2-7:accepted') st.st27 = same;
      st.logIdHits = Math.max(st.logIdHits, await logIdHits(page, ids));
    };
    await journey(page, mouseDriver(page), name, L, null, { onStep });
    L.check(name, 'IR が持つ場面をすべて通った', [...EXPECTED.keys()].every((k) => seen.has(k)) && seen.size === EXPECTED.size);
    const e = await engine(page);
    L.check(name, '最後の Step に着いた', e.current === LAST);
    L.check(name, 'エンジンの警告が 0 件', e.log.filter((r) => r.kind === 'warning').length === 0);
    L.check(name, 'エンジンの無視が 0 件', e.log.filter((r) => r.kind === 'ignored').length === 0);
    const counts = await collectSafety(page, rec);
    for (const k of nonZero(counts)) L.check(name, `安全の計数 ${k} が 0`, false);
    L.check(name, '安全の計数がすべて 0', nonZero(counts).length === 0);
    const fixture = query.split('=')[1];
    console.log(`CHOICE-LABELS fixture=${fixture} csp=${csp ? 'on' : 'off'} scanned=${seen.size}/${EXPECTED.size} buttons=${st.buttons} id-hits=${st.idHits} st-1-4=${st.st14} st-2-7=${st.st27} log-id-hits=${st.logIdHits} ledger=${L.summary()}`);
    assert.deepEqual(L.failed(), [], JSON.stringify({ rec }));
    result = st;
    await context.close();
  }));
  return result;
}

for (const csp of [true, false]) {
  test(`全 Step の選択肢のボタンに ID の文字が出ない（CSP ${csp ? 'あり' : 'なし'}）`, async () => {
    const st = await run({ csp, query: '?fixture=ch0-3', name: 'labels', withLabel: true });
    assert.equal(st.idHits, 0, 'ID の文字が選択肢に出た');
    assert.equal(st.st14, 'ok');
    assert.equal(st.st27, 'ok');
  });
}

test('対照: answer の label を外した教材では数え方が ID の表示を検出する（CSP あり）', async () => {
  const st = await run({ csp: true, query: '?fixture=ch0-3-nolabel', name: 'nolabel', withLabel: false });
  assert.ok(st.answerIdHits >= ANSWER_OPTIONS, `検出が足りない: ${st.answerIdHits} < ${ANSWER_OPTIONS}`);
  assert.equal(st.st14, 'ng', 'label を外すと st-1-4 は label の並びにならない');
  assert.equal(st.st27, 'ng', 'label を外すと st-2-7 は label の並びにならない');
});
