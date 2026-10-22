// 画面の状態を、配信先に依らない固定の文字列にする直列化（TASK-114 1/2。DLV-15）。
//
// 役割: L1 と L2 の 9 回の実行（l1-l2-match.test.mjs）が、(a) 操作ごとの観察と (b) 最後の画面の状態 `snapshot.json` を
// バイト単位で比べるための部品。配信する設定には `__fandheTest`（試験の口）が無いため、DOM だけを読む。
// 試験ではないため `.test.mjs` にしない（試験ファイルを import すると、試験が二重に登録されるため）。
//
// snapshot の中身: 画面の根（#app）の複製を HTML にした文字列と、HTML に現れない値（下書き・選択・フォーカス）。
//   除外するもの: 書き出しのリンク（data-testid=export-download）の blob: の URL を値に持つ属性（href・data-export-url・data-key）だけ。複製の上で取り除く。
//   正規化するもの: 他の属性の値に含まれる現在の配信元とサブパスの部分だけを `<base>/` に置き換え、残りのパスの違いは比較に残す。
//   自己検査: 直列化の結果に、オリジンやポートに依存する文字列（127.0.0.1・localhost・blob:・http の URL）が残っていれば、
//   9 回の一致が壊れる前に理由つきで例外にする。
// 出力に DOM の文字を出さない（呼び出し側は SHA-256 の先頭と、最初に違った欄の名前だけを出す）。
import { createHash } from 'node:crypto';
import { inspectDom } from './l2-journey.mjs';

/**
 * ページ内で評価する自己完結の関数（外の名前を参照しない。Playwright が文字列にして渡すため。`Function`・`eval` は CSP が止めるので使わない）。
 * 戻りは JSON にできる値だけ。
 */
export function readScreen() {
  const root = document.getElementById('app');
  const clone = root.cloneNode(true);
  // 現在の配信元とサブパス（例: http://127.0.0.1:1234/fandhe-workshop/）。配信先ごとに変わるのはこの部分だけ。
  const baseUrl = new URL('.', location.href).href;
  for (const el of clone.querySelectorAll('*')) {
    const isExportLink = el.getAttribute('data-testid') === 'export-download';
    for (const attr of [...el.attributes]) {
      // 書き出しのリンクの href・data-export-url・data-key だけは、毎回変わる blob の URL を持つため取り除く。
      if (isExportLink && /^blob:/.test(attr.value)) {
        el.removeAttribute(attr.name);
      } else if (attr.value.includes(baseUrl)) {
        // それ以外は、現在の配信元とサブパスの部分だけを置き換え、残りのパスなどの比較を残す。
        el.setAttribute(attr.name, attr.value.split(baseUrl).join('<base>/'));
      }
    }
  }
  const draft = document.querySelector('[data-testid="draft-input"]');
  const os = document.querySelector('[data-testid="os-select"]');
  const active = document.activeElement;
  return {
    mounted: document.documentElement.getAttribute('data-mounted'),
    html: clone.outerHTML,
    draft: draft === null ? null : draft.value,
    os: os === null ? null : os.value,
    active: active === null ? null : (active.getAttribute('data-testid') ?? active.tagName),
  };
}

const ORIGIN_DEPENDENT = [/127\.0\.0\.1/, /localhost/i, /blob:/, /https?:\/\//];

/** 直列化の結果にオリジン依存の文字列が残っていれば、何が残っているか（パターンの名前）の一覧を返す。 */
export function originLeaks(text) {
  return ORIGIN_DEPENDENT.filter((re) => re.test(text)).map((re) => re.source);
}

/** 欄の順が固定の JSON 文字列（`snapshot.json` の中身）を返す。 */
export function serializeScreen(screen) {
  const text = JSON.stringify({
    mounted: screen.mounted,
    active: screen.active,
    draft: screen.draft,
    os: screen.os,
    html: screen.html,
  }, null, 2);
  const leaks = originLeaks(text);
  if (leaks.length > 0) throw new Error(`snapshot has origin-dependent strings: ${leaks.join(', ')}`);
  return `${text}\n`;
}

/** いまの画面の `snapshot.json` の中身を返す。 */
export async function takeSnapshot(page) {
  return serializeScreen(await page.evaluate(readScreen));
}

/**
 * 操作ごとの観察 1 件。journey の onStep が呼ばれるたびに積む。題は画面に描かれていない（shell-title は未結線。TASK-524）ため、
 * IR の並びから導いた Step の ID で代える。値は DOM の語・種類・件数だけで、入力した値は含まない。
 */
export async function observeStep(page, order, uid, status) {
  const { d } = await page.evaluate(inspectDom, [order, uid, status]);
  return { uid, status, ...d };
}

export const sha256 = (data) => createHash('sha256').update(data).digest('hex');

/** 2 つの JSON 文字列を比べ、最初に違った欄のパス（値は出さない）を返す。同じなら null。 */
export function firstDiffPath(a, b) {
  const walk = (x, y, p) => {
    if (x === y) return null;
    const xo = x !== null && typeof x === 'object';
    const yo = y !== null && typeof y === 'object';
    if (!xo || !yo || Array.isArray(x) !== Array.isArray(y)) return p;
    const keys = [...new Set([...Object.keys(x), ...Object.keys(y)])];
    for (const k of keys) {
      const r = walk(x[k], y[k], `${p}.${k}`);
      if (r !== null) return r;
    }
    return null;
  };
  try {
    return walk(JSON.parse(a), JSON.parse(b), '$');
  } catch {
    return a === b ? null : '$(unparsable)';
  }
}
