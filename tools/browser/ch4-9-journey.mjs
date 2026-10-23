// CH-4〜CH-9 の形の 17 Step の誘導の補助モジュール（TASK-674 1/2）。
//
// 役割: ch4-9-run.test.mjs（通しの合否・配信する設定での通し・DOM の観察の較正・パスワード欄の検出器の対照）と、
// 後続の csp-runs への登録（TASK-674 2/2）が、同じ誘導の操作を共有する。`.test.mjs` を import すると試験が二重に登録されるため、
// 試験でない本モジュールに置く。教材は `crates/web/tests/data/ch4_9/ir.json`（`?fixture=ch4-9`）。本物の CH-4〜CH-9 ではなく、
// 章ごとの入力方式・確認の種類・拒否・処理中・ビューの種類の形を写した試験用の教材である（同 README.md）。
// ドライバ・台帳・口の数え方は ch0-3-journey.mjs のものをそのまま使う（二重に書かない。ch0-3-journey.mjs は変えない）。
//
// 観察は 2 種。hookObserver 相当（試験の設定。`__fandheTest.stateJson` を読む）と、domObserver49（配信する設定。DOM だけを読む。
// 配信する設定には `__fandheTest` が無い）。DOM の目印の正しさは、較正の試験が stateJson との一致で確かめる。
// `Function`・`eval` は使わない（CSP が止める）。入力した値は台帳にも出力にも載せない。
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  NEXT, T, engine, ports, waitCount, hookObserver, REJECTED,
} from './ch0-3-journey.mjs';

export { hookObserver, install, ledger, mouseDriver, keyboardDriver } from './ch0-3-journey.mjs';

export const FIXTURE_QUERY = '?fixture=ch4-9';

// 教材の節点（uid）・題・章の ID・章の中の位置。DOM の観察と上部の位置の表示（「章の ID　Step M / K」）の期待値に使う。
// 題は ir.json と一致させる（較正の試験が崩れを検出する）。
export const STEPS = [
  ['main/st-4-1a', '設定の範囲を確かめて承認する'], ['main/st-4-1b', '直した設定を再承認する'], ['main/st-4-2', '再実行が止まる理由を答える'],
  ['main/st-5-1', '名前つきの設定を作る'], ['main/st-5-2', '止める対象を選ぶ'],
  ['main/st-6-1', '要件を複数行で渡す'], ['main/st-6-2a', '分け方の案を承認する'], ['main/st-6-2b', '直した分け方を再承認する'], ['main/st-6-3', '状況を読む'],
  ['main/st-7-1', '守りを足す'], ['main/st-7-2', '処理を流して待つ'], ['main/st-7-3', '止まり方を見分ける'], ['main/st-7-4', '結果を人がまとめる'],
  ['main/st-8-1', '名前を選んで投稿する'], ['main/st-8-2', '3 枚の指摘を仕分ける'], ['main/st-8-3', '修正の計画を承認する'],
  ['main/st-9-1', '初版を保存する'],
];
export const TITLES = new Map(STEPS);
export const ORDER = STEPS.map(([uid]) => uid);
// 章の最後の Step（`final`）。この達成の後に「次の章へ進む」が 1 個出る（最後の CH-9 を除く）。
export const CHAPTER_ENDS = ['main/st-4-2', 'main/st-5-2', 'main/st-6-3', 'main/st-7-4', 'main/st-8-3'];
export const LAST = 'main/st-9-1';


/**
 * パスワードに当たる入力欄の数（ART-24）。ページ内で評価する自己完結の関数（外の名前を参照しない）。
 * `draft-input`（コマンド・自然文の入力欄）も数える。パスワードの欄ではない前提で除外しない。
 * 数えるのは、type=password（大文字小文字違いを含む）、autocomplete に password を含む要素、
 * placeholder・aria-label・関連づけた label の文字に「パスワード」か password（大文字小文字を区別しない）を含む input・textarea。
 */
export function passwordFields() {
  const word = (s) => /パスワード|password/i.test(s ?? '');
  const hit = new Set();
  for (const el of document.querySelectorAll('input, textarea, select')) {
    const type = (el.getAttribute('type') ?? '').toLowerCase();
    if (type === 'password') hit.add(el);
    if (word(el.getAttribute('autocomplete'))) hit.add(el);
    if (el.tagName === 'SELECT') continue;
    if (word(el.getAttribute('placeholder')) || word(el.getAttribute('aria-label'))) hit.add(el);
    for (const l of el.labels ?? []) if (word(l.textContent)) hit.add(l.control ?? el);
  }
  for (const el of document.querySelectorAll('[autocomplete]')) if (word(el.getAttribute('autocomplete'))) hit.add(el);
  return hit.size;
}

/** 現在の位置の表示（上部。「章の ID　Step M / K」）の文字。 */
export const positionText = (page) => page.evaluate(() => document.querySelector('[data-testid="shell-position"]')?.textContent ?? '');

/**
 * 画面を読み、`uid` が `status`（entered・accepted・achieved）かを判定する。ページ内で評価する自己完結の関数。
 * 戻りは JSON にできる値だけ（`d` は読み取り、`ok` は判定。末尾の `waiting` が真なら `ok` だけ）。
 * 目印: Step の題（節点ごとに一意）、処理中でない（run-status が idle）、ログの最後の行の `data-step`・`data-kind`、step-status の語。
 * 実画面の観察（2026-10-23）: ログに「達成」の行は出ない（受付は received、拒否は rejected）。step-status は入力待ちで空（章の先頭）か
 * 直前の達成の語が残る（章の途中）。そのため、確認待ちは語が「受付済み（確認待ち）」で最後の行が当の Step、
 * 達成（章の最後の Step）は語が「達成」で最後の行が当の Step の拒否でない行、入力待ちは「語が確認待ちでなく最後の行が当の Step でない」で見る。
 * 入力待ちの判定は、拒否の後の同じ Step には使わない（その確認は stillAt が受け持つ）。
 */
export function inspectDom49([titles, uid, status, waiting = false]) {
  const rows = [...document.querySelectorAll('[data-testid="part-log-list"] li')];
  const last = rows.length === 0 ? null : rows[rows.length - 1];
  const d = {
    title: document.querySelector('[data-testid="step-title"]')?.textContent ?? '',
    word: document.querySelector('[data-testid="step-status"]')?.textContent ?? '',
    run: document.querySelector('[data-testid="run-status"]')?.getAttribute('data-state') ?? '',
    rows: rows.length,
    lastStep: last?.getAttribute('data-step') ?? null,
    lastKind: last?.getAttribute('data-kind') ?? null,
  };
  const want = titles[uid];
  let ok = false;
  if (d.run === 'idle' && d.title === want) {
    if (status === 'accepted') ok = d.word === '受付済み（確認待ち）' && d.lastStep === uid;
    else if (status === 'achieved') ok = d.word === '達成' && d.lastStep === uid && d.lastKind !== 'rejected';
    else ok = d.word !== '受付済み（確認待ち）' && d.lastStep !== uid;
  }
  return waiting ? ok : { d, ok };
}

/** 配信する設定の観察（DOM だけ）。 */
export function domObserver49() {
  const titles = Object.fromEntries(STEPS);
  return {
    async at(page, uid, status, timeout = T) {
      await page.waitForFunction(inspectDom49, [titles, uid, status, true], { timeout })
        .catch((e) => { throw new Error(`DOM が ${uid} の ${status} を示さない: ${String(e.message).slice(0, 200)}`); });
    },
    async stillAt(page, uid) {
      const { d } = await page.evaluate(inspectDom49, [titles, uid, 'entered']);
      return d.title === titles[uid] && d.lastStep === uid && (d.lastKind === 'rejected' || d.lastKind === 'clarified');
    },
    // 状態の欄（stateJson）が無いので、世代の確認は置かない。
    hasState: false,
  };
}

/** 試験の設定の観察。 */
export const hookObserver49 = { ...hookObserver, hasState: true };

// 17 Step の通し。`d` の操作で教材を上から進め、CH-9 の最後の Step の達成で返す。
// opts.observe: 観察、opts.onStep: 各 Step に入った（または確認待ち・達成になった）直後に呼ぶ口（Step の uid と状態を渡す）。
// 章の境目は「次の章へ進む」を 1 回押して越える（CH-4〜CH-8 の 5 回）。押した回数は戻り値の presses。
export async function journey49(page, d, scene, L, { observe = hookObserver49, onStep = null } = {}) {
  const at = (uid, status) => observe.at(page, uid, status);
  let rejected = 0;
  let presses = 0;
  // 口の数は (選択肢, 承認, 入力欄, 送信)。承認の数は候補つきの Step で教材に従うため、null は数を確かめない。
  const portsOk = async (expected) => {
    const got = await ports(page);
    return expected.every((n, i) => n === null || n === got[i]);
  };
  const enter = async (uid, expected) => {
    await at(uid, 'entered');
    L.check(scene, `${uid} 入力待ちの口`, await portsOk(expected));
    L.check(scene, `${uid} の途中で次の章へ進むが 0 個`, (await page.locator(NEXT).count()) === 0);
    if (onStep) await onStep(uid, 'entered');
  };
  const wait = async (uid, expected) => {
    await at(uid, 'accepted');
    L.check(scene, `${uid} 確認待ちの口`, await portsOk(expected));
    if (onStep) await onStep(uid, 'accepted');
  };
  const reject = async (uid) => {
    rejected += 1;
    await waitCount(page, REJECTED, rejected);
    L.check(scene, `${uid} 拒否で進まない`, await observe.stillAt(page, uid));
  };
  // 章の最後の達成。`last` の達成を待ち、ボタンがちょうど 1 個・文字が契約の形・「移動」ボタンが 0 件であることを台帳に積み、押す。
  const cross = async (last, next) => {
    await at(last, 'achieved');
    if (onStep) await onStep(last, 'achieved');
    await waitCount(page, NEXT, 1);
    const info = await page.evaluate((sel) => ({
      texts: [...document.querySelectorAll(sel)].map((b) => b.textContent ?? ''),
      moves: [...document.querySelectorAll('button')].filter((b) => (b.textContent ?? '').startsWith('移動')).length,
    }), NEXT);
    L.check(scene, `${last} の完了後に次の章へ進むが 1 個`, info.texts.length === 1 && info.texts[0].startsWith('次の章へ進む: '));
    L.check(scene, `${last} の完了後の文字に次の章の先頭の題`, info.texts[0]?.endsWith(TITLES.get(next) ?? '\u0000'));
    L.check(scene, `${last} の完了後に移動ボタンが 0 件`, info.moves === 0);
    const before = observe.hasState ? (await engine(page)).stateWithoutLog.run.generation : null;
    await d.nextChapter();
    presses += 1;
    await at(next, 'entered');
    if (observe.hasState) {
      L.check(scene, `${next} へ移って世代が進む`, (await engine(page)).stateWithoutLog.run.generation > before);
    }
    await waitCount(page, NEXT, 0);
  };

  const plain = [0, 0, 1, 1];
  // CH-4: 承認（候補つきの直す）→ 再承認 → 選択肢と答える確認。
  await enter('main/st-4-1a', [0, null, 1, 1]);
  await d.decide('revise');
  await reject('main/st-4-1a');
  await d.decide('approve');
  await reject('main/st-4-1a');
  await d.revise('t-scope');
  await wait('main/st-4-1a', [0, null, 1, 1]);
  await d.decide('approve');
  await enter('main/st-4-1b', [0, null, 1, 1]);
  await d.decide('revise');
  await reject('main/st-4-1b');
  await d.decide('approve');
  await wait('main/st-4-1b', plain);
  await d.point('files/config');
  await enter('main/st-4-2', [1, 0, 1, 1]);
  await d.choose('opt-rerun');
  await wait('main/st-4-2', [2, 0, 1, 1]);
  await d.choose('ans-empty');
  await reject('main/st-4-2');
  await d.choose('ans-exists');
  await cross('main/st-4-2', 'main/st-5-1');

  // CH-5: 引数つきのコマンド（形違いの拒否）→ 選択肢（誤答）。
  await enter('main/st-5-1', plain);
  await d.send('/setup-config BadName');
  await reject('main/st-5-1');
  await d.send('/setup-config main-area');
  await wait('main/st-5-1', plain);
  await d.point('checks/config');
  await enter('main/st-5-2', [2, 0, 1, 1]);
  await d.choose('opt-stop-ng');
  await reject('main/st-5-2');
  await d.choose('opt-stop-ok');
  await cross('main/st-5-2', 'main/st-6-1');

  // CH-6: 複数行の自然文 → 分け方の差し戻しと再承認 → 読むだけの 1 択。
  await enter('main/st-6-1', plain);
  await d.sendMultiline('requirement line 1', 'requirement line 2');
  await enter('main/st-6-2a', [0, null, 1, 1]);
  await d.decide('revise');
  await reject('main/st-6-2a');
  await d.revise('t-split');
  await wait('main/st-6-2a', [0, null, 1, 1]);
  await d.decide('approve');
  await enter('main/st-6-2b', [0, null, 1, 1]);
  await d.decide('approve');
  await enter('main/st-6-3', [1, 0, 1, 1]);
  await d.choose('opt-read-status');
  await cross('main/st-6-3', 'main/st-7-1');

  // CH-7: 必須の引数が無い拒否 → 処理中 → 答える確認 → 画面操作。
  await enter('main/st-7-1', plain);
  await d.send('/add-guards');
  await reject('main/st-7-1');
  await d.send('/add-guards all-areas');
  await wait('main/st-7-1', plain);
  await d.point('checks/guards');
  await enter('main/st-7-2', plain);
  await d.send('/run-tasks');
  await wait('main/st-7-2', plain);
  await d.point('cards/job-1');
  await enter('main/st-7-3', [1, 0, 1, 1]);
  await d.choose('opt-view');
  await wait('main/st-7-3', [2, 0, 1, 1]);
  await d.choose('ans-all');
  await reject('main/st-7-3');
  await d.choose('ans-stopped');
  await enter('main/st-7-4', plain);
  await d.point('cards/job-2');
  await reject('main/st-7-4');
  await d.point('cards/job-1');
  await cross('main/st-7-4', 'main/st-8-1');

  // CH-8: 模擬ログイン（名前を選ぶだけ）→ 仕分け → 修正の承認と差分の行。
  await enter('main/st-8-1', plain);
  await d.point('demo/name-pick');
  await wait('main/st-8-1', plain);
  await d.point('demo/post');
  await enter('main/st-8-2', [2, 0, 1, 1]);
  await d.choose('opt-sort-ng');
  await reject('main/st-8-2');
  await d.choose('opt-sort-ok');
  await enter('main/st-8-3', [0, null, 1, 1]);
  await d.decide('approve');
  await wait('main/st-8-3', plain);
  await d.point('diff/fixed');
  await cross('main/st-8-3', 'main/st-9-1');

  // CH-9: チェックポイントのカード。次の章が無いので、達成の後に「次の章へ進む」は 0 個。
  await enter('main/st-9-1', plain);
  await d.point('cards/checkpoint-1');
  await at(LAST, 'achieved');
  if (onStep) await onStep(LAST, 'achieved');
  L.check(scene, 'CH-9 の完了後に次の章へ進むが 0 個', (await page.locator(NEXT).count()) === 0);
  return { presses };
}

// 配信する設定の一時の写しに fandhe-ir の meta と ir/ を足して fn に渡し、成否にかかわらず消す（pack-body-xss と同じ足し方）。
// ch4-9-run.test.mjs の B と csp-runs.test.mjs の D2 が共有する（.test.mjs は import できないため本モジュールに置く）。
export async function withDeliveryCopy(fn) {
  const copy = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-ch49-web-'));
  try {
    fs.cpSync(process.env.FANDHE_WEB_DIR, copy, { recursive: true });
    const index = path.join(copy, 'index.html');
    const html = fs.readFileSync(index, 'utf8');
    assert.equal(html.split('</head>').length - 1, 1, 'exactly one </head>');
    assert.ok(!/name\s*=\s*["']?fandhe-ir/i.test(html), 'delivery index has no fandhe-ir meta');
    assert.ok(!/name\s*=\s*["']?fandhe-test-hooks/i.test(html), 'delivery index has no test-hooks meta');
    assert.equal((html.match(/http-equiv\s*=\s*["']?content-security-policy/gi) ?? []).length, 1, 'exactly one CSP meta');
    fs.writeFileSync(index, html.replace('</head>', '<meta name="fandhe-ir" content="1"></head>'));
    fs.mkdirSync(path.join(copy, 'ir'), { recursive: true });
    return await fn(copy);
  } finally {
    fs.rmSync(copy, { recursive: true, force: true });
  }
}
