// `data-payload` の書き換え試験（TASK-148。SAFE-6）の補助。書き換え → click → 安定待ち → 観察 → 元の値へ戻す、と期待の分類を持つ。
//
// 呼び出し元は data-payload-tamper.test.mjs（`data-payload` の書き換え = TASK-148 1/2 と、`submit` の `data-action` の書き換え = 2/2）。試験でないモジュールにして、
// `.test.mjs` を import したときの二重登録を避ける。入力した値は戻り値・失敗の文に載せず、ケースは label（XSS の ID・`raw[n]`）だけで示す。
// 観察は試験の設定の `window.__fandheTest`（hooks）があれば状態とログも比べる。hooks の有無に関わらず、画面だけから読める `ui`
// （Step の題・状態の語・位置・処理中の表示・ログの行の種類と Step・左の成果物）も読み、配信する設定（hooks なし）ではこれで J・I を判定する。
// hooks なしで判定が素通りになる経路（eng が無いと常に真）は置かない。
import { DANGER_TAGS } from './harness.mjs';

export const ROOT = '[data-testid="workshop-root"]';
// 試験の設定だけの口。payload の JSON を `Op` として送るため正当に状態を変える。書き換えの対象から名前つきで除く（E4 で除く理由を示す）。
export const RAW_TESTID = 'raw-trigger';
// 現行の画面に置かない口（FEND-4。ヒントの試作の名前 hint-btn は製品の画面の hint-open と別）。全場面で 0 件を確かめる。
export const ABSENT_TESTIDS = ['tick-trigger', 'esc-trigger', 'hint-btn'];

// `crates/web/src/host.rs` の `decode_action` から分類した期待。
//   D  復号で断る。画面・保存先・状態・ログがすべて同一
//   J  復号は通り core／update が断る。世界状態・現在の Step は同一で、ログの増分は受信（received）と rejected の判定だけ。
//      受信は履歴の ops に残るため保存先は変わりうる（入力として記録する設計）
//   I  payload を読まない。書き換えない click の対照と同じ結果（状態・保存先・ログの増分で比べる。画面の細部は候補の整えなどで揺れるため比べない）
export const CLASS_OF = {
  'save-checkpoint': 'D',
  'open-hint': 'D',
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
    // 画面だけから読める観察。hooks が無い配信する設定の J・I の判定の根拠で、hooks があるときは eng の判定に重ねる。
    const t = (id) => document.querySelector(`[data-testid="${id}"]`);
    const rows = [...document.querySelectorAll('[data-testid="part-log-list"] li')];
    const ui = {
      title: t('step-title')?.textContent ?? '',
      status: t('step-status')?.textContent ?? '',
      position: t('shell-position')?.textContent ?? '',
      run: t('run-status')?.getAttribute('data-state') ?? '',
      logKinds: rows.map((x) => x.getAttribute('data-kind') ?? ''),
      logSteps: rows.map((x) => x.getAttribute('data-step') ?? ''),
      // 左の成果物。受け付けた入力の数を数える欄（part-artifact-consumed）は、受信を記録する J でも増えるため除く。
      artifact: (() => {
        const v = t('part-artifact-view');
        if (!v) return '';
        const c = v.cloneNode(true);
        c.querySelectorAll('[data-testid="part-artifact-consumed"]').forEach((e) => e.remove());
        return c.innerHTML;
      })(),
    };
    // 入力欄の value（innerHTML には出ない）。候補の click の主な結果は入力欄への補完のため、比較に含める。
    const d = document.querySelector('[data-testid="draft-input"]');
    return { dom: r ? r.innerHTML : null, ls: JSON.stringify(ls), eng, ui, draft: d ? d.value : null };
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
    const sig = JSON.stringify([o.dom, o.ls, o.eng?.kinds.length ?? 0, o.eng?.state ?? '', o.ui.logKinds.length]);
    // 処理中（busy）の Step の途中を安定と見なさない。run-status が無い画面（IR なし）は '' で、処理中ではない。
    const idle = o.ui.run !== 'busy' && (!hooks || o.eng.tick === 'null');
    same = idle && sig === prev ? same + 1 : 0;
    prev = sig;
    await page.waitForTimeout(30);
  }
  if (same < 3) throw new Error('settle: 画面が安定しなかった');
}

export function sameState(a, b) {
  const engSame = (!a.eng && !b.eng) || (a.eng.state === b.eng.state && a.eng.current === b.eng.current
    && JSON.stringify(a.eng.kinds) === JSON.stringify(b.eng.kinds));
  return a.dom === b.dom && a.ls === b.ls && engSame && JSON.stringify(a.ui) === JSON.stringify(b.ui);
}

// 画面から読める範囲で、現在の Step（題・状態の語・位置）と左の成果物が同一（ログ・保存先は問わない）。
// 状態の語だけは 1 つの例外を許す。前の Step の達成の直後に入った Step では、ログの末尾が前の Step の達成のため「達成」が出ていて、
// 次の入力の行（拒否を含む）が積まれると語なしに戻る（crates/web/src/step_status.rs の規則 3。世界状態は変わらない）。
// よって、前が「達成」・後が語なしで、増えた行に拒否があるときに限り同一と見なす。それ以外の語の変化は変化として扱う。
const ACHIEVED_WORD = '達成';
function statusSame(a, b) {
  if (a.ui.status === b.ui.status) return true;
  return a.ui.status === ACHIEVED_WORD && b.ui.status === '' && b.ui.logKinds.slice(a.ui.logKinds.length).includes('rejected');
}
export function domCoreSame(a, b) {
  return a.ui.title === b.ui.title && statusSame(a, b) && a.ui.position === b.ui.position && a.ui.artifact === b.ui.artifact;
}

// 世界状態・slice・現在の Step が同一（ログ・画面・保存先は問わない）。hooks があれば eng も比べ、画面の判定は常に重ねる。
export function sameCore(a, b) {
  const engSame = !a.eng || (a.eng.state === b.eng.state && a.eng.current === b.eng.current);
  return engSame && domCoreSame(a, b);
}

// sameCore が偽のときに違った観察の欄の名前（失敗の文に載せる。値は載せない）。
function coreDiff(a, b) {
  const out = [];
  if (a.eng && a.eng.state !== b.eng.state) out.push('eng.state');
  if (a.eng && a.eng.current !== b.eng.current) out.push('eng.current');
  for (const k of ['title', 'status', 'position', 'artifact']) if (a.ui[k] !== b.ui[k]) out.push(`ui.${k}`);
  return out;
}

function addedKinds(a, b) {
  return b.ui.logKinds.slice(a.ui.logKinds.length);
}

// J の許す増分。受け付けた値は「受信（input_received）」と「判定（judged）が rejected」の 2 行になる。受付・達成・進入・復帰の行は許さない。
const J_KINDS = new Set(['input_received', 'judged', 'rejected']);
// 画面の行の種類での許す増分。受信（received）と拒否（rejected）だけで、既存の行は変わらない。
const J_DOM_KINDS = new Set(['received', 'rejected']);
export function domLogOk(a, b) {
  const n = a.ui.logKinds.length;
  const prefix = b.ui.logKinds.length >= n && a.ui.logKinds.every((k, i) => k === b.ui.logKinds[i])
    && a.ui.logSteps.every((k, i) => k === b.ui.logSteps[i]);
  return prefix && b.ui.logKinds.slice(n).every((k) => J_DOM_KINDS.has(k));
}
export function jLogOk(a, b) {
  if (!domLogOk(a, b)) return false;
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
        if (!sameCore(r.before, r.after)) fail(`世界状態または現在の Step が変わった（${coreDiff(r.before, r.after).join('・')}）`);
        else if (!jLogOk(r.before, r.after)) fail('受信と rejected の判定以外のログが増えた');
      } else if (cls === 'I') {
        // 世界状態・現在の Step の変化と保存内容の変化は別々の真偽値で比べ、増えたログの種類と行も照合する。
        const sig = (x) => JSON.stringify({
          core: !sameCore(x.before, x.after),
          ls: x.before.ls !== x.after.ls,
          kinds: x.before.eng ? x.after.eng.kinds.slice(x.before.eng.kinds.length) : [],
          draft: x.after.draft,
          lines: x.before.eng ? x.after.eng.lines.slice(x.before.eng.lines.length) : [],
          domLog: addedKinds(x.before, x.after),
        });
        if (sig(r) !== sig(control)) fail(`書き換えない click と結果が違う（${sig(r)} / 対照 ${sig(control)}）`);
      } else {
        fail(`未知の分類 ${cls}`);
      }
    }
  }
  return out;
}

// `submit-input` の `data-action` の書き換え（TASK-148 2/2。SAFE-6）。値は spec の SAFE-6 と PoC-7 README の E5 の行が挙げる 4 つの名前
// （testdata に構造化された写しは無いため、ここで固定する）。`raw` は試験の設定だけに口がある action で、配信する設定にはコンパイルされない。
export const ACTION_TAMPER_NAMES = ['raw', 'debug', 'unknown', '__proto__'];

// 入力欄の value まで含めて同一か（`sameState` は画面・保存先・状態・ログだけを比べる）。
export function sameStateWithDraft(a, b) {
  return sameState(a, b) && a.draft === b.draft;
}

// 1 回の書き換え。`selector` の唯一の要素の `data-action` を `action` に置き（`payload` が null でなければ `data-payload` も置く）、
// `via` が click なら要素を click、enter なら入力欄で Enter を押す（glue が submit-input を click する経路）。安定を待ち、書き換えた値が
// 残っている要素だけ元へ戻す。戻り値は { gone, before, after, danger: { before, after } }。値は戻り値に載せない。
export async function tamperActionClick(page, hooks, selector, { action, payload = null, via = 'click', draftSelector = '[data-testid="draft-input"]' }) {
  // Enter は入力欄にフォーカスがあって初めて送信になる。フォーカス（候補の整えなど）の変化を書き換えの結果に混ぜないよう、観察の前に済ませる。
  if (via === 'enter') {
    await page.focus(draftSelector);
    await settle(page, hooks);
  }
  const before = await observe(page, hooks);
  const dangerBefore = await dangerCount(page);
  const orig = await page.evaluate(([sel, a, p]) => {
    const el = document.querySelector(sel);
    if (!el) return null;
    const o = { action: el.getAttribute('data-action'), payload: el.getAttribute('data-payload') };
    el.setAttribute('data-action', a);
    if (p !== null) el.setAttribute('data-payload', p);
    return o;
  }, [selector, action, payload]);
  if (!orig) return { gone: true };
  if (via === 'enter') {
    await page.keyboard.press('Enter');
  } else {
    await page.evaluate((sel) => document.querySelector(sel)?.click(), selector);
  }
  await settle(page, hooks);
  await page.evaluate(([sel, a, p, o]) => {
    const el = document.querySelector(sel);
    if (!el) return;
    if (el.getAttribute('data-action') === a) {
      if (o.action === null) el.removeAttribute('data-action');
      else el.setAttribute('data-action', o.action);
    }
    if (p !== null && el.getAttribute('data-payload') === p) {
      if (o.payload === null) el.removeAttribute('data-payload');
      else el.setAttribute('data-payload', o.payload);
    }
  }, [selector, action, payload, orig]);
  const after = await observe(page, hooks);
  return { gone: false, before, after, danger: { before: dangerBefore, after: await dangerCount(page) } };
}
