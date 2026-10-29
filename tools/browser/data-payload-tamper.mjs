// `data-payload` の書き換え試験（TASK-148。SAFE-6）の補助。書き換え → click → 安定待ち → 観察 → 元の値へ戻す、と期待の分類を持つ。
//
// 呼び出し元は data-payload-tamper.test.mjs と、`submit` の `data-action` の書き換えを足す TASK-148 2/2。試験でないモジュールにして、
// `.test.mjs` を import したときの二重登録を避ける。入力した値は戻り値・失敗の文に載せず、ケースは label（XSS の ID・`raw[n]`）だけで示す。
// 観察は試験の設定の `window.__fandheTest`（hooks）があれば状態とログも比べ、無ければ（配信する設定）画面と保存先だけを比べる。
import { DANGER_TAGS } from './harness.mjs';

export const ROOT = '[data-testid="workshop-root"]';
// 試験の設定だけの口。payload の JSON を `Op` として送るため正当に状態を変える。書き換えの対象から名前つきで除く（E4 で除く理由を示す）。
export const RAW_TESTID = 'raw-trigger';
// 現行の画面に置かない口（FEND-4・ヒントの画面は未実装）。全場面で 0 件を確かめる。
export const ABSENT_TESTIDS = ['tick-trigger', 'esc-trigger', 'hint-btn'];

// `crates/web/src/host.rs` の `decode_action` から分類した期待。
//   D  復号で断る。画面・保存先・状態・ログがすべて同一
//   J  復号は通り core／update が断る。世界状態・現在の Step は同一で、ログの増分は受信（received）と rejected の判定だけ。
//      受信は履歴の ops に残るため保存先は変わりうる（入力として記録する設計）
//   I  payload を読まない。書き換えない click の対照と同じ結果（状態・保存先・ログの増分で比べる。画面の細部は候補の整えなどで揺れるため比べない）
export const CLASS_OF = {
  'save-checkpoint': 'D',
  'restore-checkpoint': 'D',
  'select-pane': 'D',
  'select-view': 'D',
  decide: 'D',
  'dismiss-notice': 'D',
  ui: 'D',
  choose: 'J',
  'revise-target': 'J',
  'next-chapter': 'J',
};

// 場面で出ているはずの data-action の集合と実測の差。どちらかが空でなければ不一致。
export function diffInventory(actual, expected) {
  const a = new Set(actual);
  const e = new Set(expected);
  return { missing: [...e].filter((x) => !a.has(x)).sort(), extra: [...a].filter((x) => !e.has(x)).sort() };
}

// root 内の `[data-payload]` の一覧（raw-trigger は除く）。raw の件数は別に返す。
export function inventory(page) {
  return page.evaluate(([root, raw]) => {
    const all = [...document.querySelectorAll(`${root} [data-payload]`)];
    const keep = all.filter((e) => e.getAttribute('data-testid') !== raw);
    return {
      items: keep.map((e) => ({ action: e.getAttribute('data-action') ?? '', payload: e.getAttribute('data-payload') })),
      rawCount: all.length - keep.length,
    };
  }, [ROOT, RAW_TESTID]);
}

export const absentCounts = (page) => page.evaluate(([ids]) => ids.map((id) => document.querySelectorAll(`[data-testid="${id}"]`).length), [ABSENT_TESTIDS]);

// 画面の root の中身・保存先（localStorage）・状態とログの観察。hooks があれば状態とログも読む。
export function observe(page, hooks) {
  return page.evaluate(([root, withHooks]) => {
    const r = document.querySelector(root);
    const ls = {};
    try {
      for (let i = 0; i < localStorage.length; i += 1) {
        const k = localStorage.key(i);
        ls[k] = localStorage.getItem(k);
      }
    } catch { /* 読めない文脈は空として比べる */ }
    let eng = null;
    if (withHooks) {
      const e = JSON.parse(window.__fandheTest.stateJson()).engine;
      eng = {
        current: e.current,
        state: JSON.stringify(e.stateWithoutLog),
        kinds: e.log.map((x) => x.kind),
        lines: e.log.map((x) => x.line),
        tick: window.__fandheTest.nextTick(),
      };
    }
    // 入力欄の value（innerHTML には出ない）。候補の click の主な結果は入力欄への補完のため、比較に含める。
    const d = document.querySelector('[data-testid="draft-input"]');
    return { dom: r ? r.innerHTML : null, ls: JSON.stringify(ls), eng, draft: d ? d.value : null };
  }, [ROOT, hooks]);
}

// root の中の禁止の要素の数。html を渡すと、切り離した要素にその文字列を解釈させて数える（E2 の対照）。
export function dangerCount(page, html = null) {
  return page.evaluate(([root, tags, h]) => {
    const sel = tags.join(',');
    if (h !== null) {
      const tmp = document.createElement('div');
      tmp.innerHTML = h;
      return tmp.querySelectorAll(sel).length;
    }
    return document.querySelector(root)?.querySelectorAll(sel).length ?? -1;
  }, [ROOT, DANGER_TAGS, html]);
}

// 処理中でなく、画面・ログ・保存先が連続 3 回の読みで変わらないのを待つ（固定の sleep だけには頼らない）。
export async function settle(page, hooks) {
  let prev = null;
  let same = 0;
  for (let n = 0; n < 200 && same < 3; n += 1) {
    const o = await observe(page, hooks);
    const sig = JSON.stringify([o.dom, o.ls, o.eng?.kinds.length ?? 0, o.eng?.state ?? '']);
    const idle = !hooks || o.eng.tick === 'null';
    same = idle && sig === prev ? same + 1 : 0;
    prev = sig;
    await page.waitForTimeout(30);
  }
  if (same < 3) throw new Error('settle: 画面が安定しなかった');
}

export function sameState(a, b) {
  const engSame = (!a.eng && !b.eng) || (a.eng.state === b.eng.state && a.eng.current === b.eng.current
    && JSON.stringify(a.eng.kinds) === JSON.stringify(b.eng.kinds));
  return a.dom === b.dom && a.ls === b.ls && engSame;
}

// 世界状態・slice・現在の Step が同一（ログ・画面・保存先は問わない）。
function sameCore(a, b) {
  return !a.eng || (a.eng.state === b.eng.state && a.eng.current === b.eng.current);
}

function addedKinds(a, b) {
  if (!a.eng) return [];
  return b.eng.kinds.slice(a.eng.kinds.length);
}

// J の許す増分。受け付けた値は「受信（received）」と「判定（judged）が rejected」の 2 行になる。受付・達成・進入・復帰の行は許さない。
const J_KINDS = new Set(['input_received', 'judged', 'rejected']);
function jLogOk(a, b) {
  if (!a.eng) return true;
  const n = a.eng.kinds.length;
  const kinds = b.eng.kinds.slice(n);
  const lines = b.eng.lines.slice(n);
  return b.eng.kinds.length >= n && kinds.every((k, i) => J_KINDS.has(k)
    && (k === 'judged' ? lines[i] === 'rejected' : k === 'rejected' ? true : lines[i] === 'received'));
}

// 1 回の書き換え。`index` 番目の `selector` の要素に value を `data-payload` として置き（無ければ足す）、click し、安定を待ち、元へ戻す。
// value が null なら書き換えずに click する（対照）。戻り値は { before, after, danger: { before, after }, gone }。
export async function tamperClick(page, hooks, selector, index, value) {
  const before = await observe(page, hooks);
  const dangerBefore = await dangerCount(page);
  const orig = await page.evaluate(([sel, i, v]) => {
    const el = document.querySelectorAll(sel)[i];
    if (!el) return { gone: true };
    const o = el.getAttribute('data-payload');
    // 値が元と一致する組は書き換えではない（正規の click になる）ため押さない。
    if (v !== null && o === v) return { gone: false, o, same: true };
    if (v !== null) el.setAttribute('data-payload', v);
    el.click();
    return { gone: false, o };
  }, [selector, index, value]);
  if (orig.gone) return { gone: true };
  if (orig.same) return { gone: false, same: true, orig: orig.o };
  await settle(page, hooks);
  // 元の値へ戻す。再描画で別の要素になっていれば属性は元の値で出ているため、書き換えた値が残っている要素だけを直す。
  await page.evaluate(([sel, i, v, o]) => {
    const el = document.querySelectorAll(sel)[i];
    if (!el || v === null || el.getAttribute('data-payload') !== v) return;
    if (o === null) el.removeAttribute('data-payload');
    else el.setAttribute('data-payload', o);
  }, [selector, index, value, orig.o]);
  const after = await observe(page, hooks);
  return { gone: false, before, after, danger: { before: dangerBefore, after: await dangerCount(page) }, orig: orig.o };
}

// 1 つの口の種類（selector）の全インスタンス × 全値の書き換え。期待の分類（cls）で判定し、失敗は label だけで返す。
//   D: 同一。J: 状態は同一でログの増分は rejected だけ。I: 直前の対照の click と同じ変化。
// 戻り値: { clicks, skipped, failures }。skipped は値が元の payload と一致する組（書き換えではないため除く）。
//   draft: { selector, text } を渡すと、I の対照と書き換えの click の直前に入力欄をその下書きへ戻して同じ条件から始める（候補の補完の比較用）。
export async function tamperGroup(page, hooks, { name, selector, cls, values, draft = null }) {
  const n = await page.evaluate((sel) => document.querySelectorAll(sel).length, selector);
  const out = { clicks: 0, skipped: 0, failures: [], instances: n };
  const resetDraft = async () => {
    if (!draft) return;
    await page.fill(draft.selector, draft.text);
    await page.waitForFunction(([sel, c]) => document.querySelectorAll(sel).length >= c, [selector, n]);
  };
  for (let i = 0; i < n; i += 1) {
    // I は最初の click が初回だけの変化（フォーカス・一覧の整え）を起こしうるため、記録しない対照の click を先に 1 回行う。
    if (cls === 'I') {
      await resetDraft();
      await tamperClick(page, hooks, selector, i, null);
      await page.waitForTimeout(800);
    }
    for (const v of values) {
      let control = null;
      if (cls === 'I') {
        await resetDraft();
        control = await tamperClick(page, hooks, selector, i, null);
        if (control.gone) { out.failures.push(`${name}[${i}] ${v.label}: 対照の click の要素が消えた`); continue; }
      }
      if (cls === 'I') await resetDraft();
      const r = await tamperClick(page, hooks, selector, i, v.value);
      if (r.gone) { out.failures.push(`${name}[${i}] ${v.label}: 要素が消えた`); continue; }
      if (r.same) { out.skipped += 1; continue; }
      out.clicks += 1;
      const fail = (why) => out.failures.push(`${name}[${i}] ${v.label}: ${why}`);
      if (r.danger.after !== r.danger.before) fail('禁止の要素が増えた');
      if (cls === 'D') {
        if (!sameState(r.before, r.after)) fail('画面・状態・ログ・保存先のどれかが変わった');
      } else if (cls === 'J') {
        if (!sameCore(r.before, r.after)) fail('世界状態または現在の Step が変わった');
        else if (!jLogOk(r.before, r.after)) fail('受信と rejected の判定以外のログが増えた');
      } else if (cls === 'I') {
        // 世界状態・現在の Step の変化と保存内容の変化は別々の真偽値で比べ、増えたログの種類と行も照合する。
        const sig = (x) => JSON.stringify({
          core: !sameCore(x.before, x.after),
          ls: x.before.ls !== x.after.ls,
          kinds: addedKinds(x.before, x.after),
          draft: x.after.draft,
          lines: x.before.eng ? x.after.eng.lines.slice(x.before.eng.lines.length) : [],
        });
        if (sig(r) !== sig(control)) fail(`書き換えない click と結果が違う（${sig(r)} / 対照 ${sig(control)}）`);
      } else {
        fail(`未知の分類 ${cls}`);
      }
    }
  }
  return out;
}
