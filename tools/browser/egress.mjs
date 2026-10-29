// 受講者の入力・履歴を外部へ送らないことの通信の計数器（SAFE-16。TASK-153）。
//
// 役割: harness.mjs の記録器 rec（route を通った要求だけを数える）が見落とす経路を補う。
//   - 通信の API の「呼び出し」を init script で数える（fetch・XMLHttpRequest・WebSocket・EventSource・sendBeacon・RTCPeerConnection・
//     ServiceWorker の登録・Worker・SharedWorker）。CSP ありの回は止められた試みが route に届かないため、完了した要求ではなく
//     呼び出しの件数で「送ろうとしない」ことを確かめる。
//   - WebSocket・Worker・ServiceWorker はブラウザ側のイベント（page.on('websocket')・page.on('worker')・context.serviceWorkers()）でも数える。
//   - 配信元への要求のうち、GET・HEAD 以外（静的配信への送信＝提出の形）と、配信物の構成要素以外のパス・サブリソースのクエリ
//     （入力・履歴を URL に載せて送る形）を数える。
// 呼び出し元: egress-count.test.mjs（場面 G1・G2・C）。純関数は egress.test.mjs が表で試す。
// 記録するのは API 名・宛先の分類・件数だけで、要求の本文・URL の値・履歴・下書きは Node 側へ送らず、出力もしない。
// 契約: harness.mjs の safetyCounts のキー名は変えない。ここは別モジュールとして rec と合わせた表を返す。
// 変更時の連動: 配信物の構成要素（ALLOWED_PATHS）は scripts/check-no-egress.sh の DIST_FILES・DIST_OPTIONAL と、
// 試験の設定の test-hooks.js は tools/build-web.sh と揃える。

const BINDING = '__fandheEgress';

// 配信元（base の下）で要求してよいパス。空文字は base 直下（ディレクトリ）。
export const ALLOWED_PATHS = new Set([
  '', 'index.html', 'app.js', 'app.css', 'icon.svg',
  'wasm/fandhe_workshop_web.js', 'wasm/fandhe_workshop_web_bg.wasm', 'ir/main.json', 'test-hooks.js',
]);
// 実行時の fetch で許す宛先（配信元への GET だけ）。wasm の取得は wasm-bindgen の初期化、IR は app.js。
export const FETCH_PATHS = new Set(['wasm/fandhe_workshop_web_bg.wasm', 'ir/main.json']);

// 相対パス（base より後ろ）を返す。base の外は null。
export function relPath(pathname, base) {
  if (!pathname.startsWith(base)) return null;
  return pathname.slice(base.length);
}

// 試験が開くページの固定クエリの鍵。値は短い識別子の形に限る（履歴などの長い値を載せた文書要求を通さない）。
const PAGE_QUERY_KEYS = new Set(['fixture', 'start', 'ir', 'tick', 'receptor']);
const PAGE_QUERY_VALUE = /^[A-Za-z0-9._\/-]{0,64}$/;

// クエリつきの要求を許すのは、メインフレームの文書要求で、試験のページ（'' か index.html）への固定の鍵・短い値だけ。
// サブフレーム（iframe の src）や他のパスへの文書要求は許さない。
function isTestPageQuery(u, resourceType, isMainFrame, rel) {
  if (resourceType !== 'document' || !isMainFrame) return false;
  if (rel !== '' && rel !== 'index.html') return false;
  for (const [k, v] of u.searchParams) {
    if (!PAGE_QUERY_KEYS.has(k) || !PAGE_QUERY_VALUE.test(v)) return false;
  }
  return true;
}

// 配信元への要求（Playwright の Request 由来の値）を分類する純関数。
// 戻り値: 'ok' | 'nonGet'（GET・HEAD 以外）| 'offAllowlist'（構成要素以外のパス、またはサブリソースのクエリ）| 'offOrigin'。
// data:・blob: は同一文書内の資源で通信ではないため 'ok'。favicon.ico はクエリなしの GET・HEAD に限り 'ok'（取得が揺れるため。クエリつきは通常の分類に進む）。
export function classifyRequest({ url, method, resourceType, isMainFrame = false }, { origin, base }) {
  if (url.startsWith('data:') || url.startsWith('blob:')) return 'ok';
  let u;
  try {
    u = new URL(url);
  } catch {
    return 'offAllowlist';
  }
  if (u.origin !== origin) return 'offOrigin';
  if (method !== 'GET' && method !== 'HEAD') return 'nonGet';
  if (u.pathname === '/favicon.ico' && u.search === '' && u.hash === '') return 'ok';
  const rel = relPath(u.pathname, base);
  if (rel === null || !ALLOWED_PATHS.has(rel)) return 'offAllowlist';
  if (u.search !== '' && !isTestPageQuery(u, resourceType, isMainFrame, rel)) return 'offAllowlist';
  return 'ok';
}

// ページ内で記録した呼び出し（{ api, off, special, unparsed, pathname, hasQuery, method }）を分類する純関数。
// 戻り値: 'allowedFetch'（配信元への GET で wasm か IR）| 'offOrigin' | 'other'（許可外の呼び出し）。
export function classifyCall(call, { base }) {
  if (call.off) return 'offOrigin';
  if (call.api !== 'fetch' || call.unparsed || call.special) return 'other';
  if ((call.method ?? 'GET').toUpperCase() !== 'GET' || call.hasQuery) return 'other';
  const rel = relPath(call.pathname ?? '', base);
  return rel !== null && FETCH_PATHS.has(rel) ? 'allowedFetch' : 'other';
}

// ページのスクリプトより先に動く包み。元の関数へそのまま渡し、エンジンの振る舞いは変えない。
// Node 側へ送るのは API 名と URL の分類だけ（URL の値は送らず、パス名はクエリを含まない形で分類にだけ使う）。
function installWrappers(binding) {
  const report = (api, url, method) => {
    let rec;
    try {
      const u = new URL(String(url), location.href);
      const special = u.protocol === 'data:' || u.protocol === 'blob:';
      rec = {
        api, off: !special && u.origin !== location.origin, special, unparsed: false, pathname: u.pathname, hasQuery: u.search !== '', method,
      };
    } catch {
      rec = { api, off: false, special: false, unparsed: true, pathname: '', hasQuery: false, method };
    }
    try {
      const r = window[binding](rec);
      if (r && typeof r.catch === 'function') r.catch(() => {});
    } catch { /* 記録できない場合は何もしない */ }
  };
  const ofetch = window.fetch;
  if (typeof ofetch === 'function') {
    window.fetch = function fetchWrapped(input, init) {
      const url = typeof input === 'string' ? input : (input && input.url) || String(input);
      report('fetch', url, (init && init.method) || (input && input.method) || 'GET');
      return ofetch.apply(this, arguments); // eslint-disable-line prefer-rest-params
    };
  }
  const xopen = window.XMLHttpRequest && window.XMLHttpRequest.prototype.open;
  if (xopen) {
    window.XMLHttpRequest.prototype.open = function openWrapped(method, url) {
      report('XMLHttpRequest', url, method);
      return xopen.apply(this, arguments); // eslint-disable-line prefer-rest-params
    };
  }
  for (const name of ['WebSocket', 'EventSource', 'Worker', 'SharedWorker', 'RTCPeerConnection']) {
    const C = window[name];
    if (typeof C !== 'function') continue;
    window[name] = new Proxy(C, {
      construct(target, args, newTarget) {
        report(name, name === 'RTCPeerConnection' ? '' : args[0], 'GET');
        return Reflect.construct(target, args, newTarget);
      },
    });
  }
  const beacon = navigator.sendBeacon;
  if (typeof beacon === 'function') {
    navigator.sendBeacon = function sendBeaconWrapped(url) {
      report('sendBeacon', url, 'POST');
      return beacon.apply(this, arguments); // eslint-disable-line prefer-rest-params
    };
  }
  const SWC = window.ServiceWorkerContainer;
  if (SWC && SWC.prototype.register) {
    const reg = SWC.prototype.register;
    SWC.prototype.register = function registerWrapped(url) {
      report('serviceWorker.register', url, 'GET');
      return reg.apply(this, arguments); // eslint-disable-line prefer-rest-params
    };
  }
}

// context に記録器を付ける。page を作った後、最初の goto の前に呼ぶ（init script は以後の遷移から効く）。
// origin は配信元（http://127.0.0.1:port）、base はサブパス（'/name/'）。
export async function attachEgress(context, { origin, base }) {
  const eg = {
    calls: 0, allowedFetch: 0, offOriginCalls: 0, otherCalls: 0, ws: 0, workers: 0, nonGet: 0, offAllowlist: 0, offOriginRequests: 0,
  };
  await context.exposeBinding(BINDING, (_src, call) => {
    eg.calls += 1;
    const kind = classifyCall(call, { base });
    if (kind === 'allowedFetch') eg.allowedFetch += 1;
    else if (kind === 'offOrigin') eg.offOriginCalls += 1;
    else eg.otherCalls += 1;
  });
  await context.addInitScript(installWrappers, BINDING);
  const watch = (p) => {
    p.on('websocket', () => { eg.ws += 1; });
    p.on('worker', () => { eg.workers += 1; });
  };
  for (const p of context.pages()) watch(p);
  context.on('page', watch);
  context.on('request', (req) => {
    const kind = classifyRequest({ url: req.url(), method: req.method(), resourceType: req.resourceType(), isMainFrame: req.frame()?.parentFrame() === null }, { origin, base });
    if (kind === 'nonGet') eg.nonGet += 1;
    else if (kind === 'offAllowlist') eg.offAllowlist += 1;
    else if (kind === 'offOrigin') eg.offOriginRequests += 1;
  });
  eg.context = context;
  return eg;
}

// 区分ごとの件数の表。0 であるべき区分だけを持つ（allowedFetch は 0 を求めない情報として別に返す）。
// rec は harness の記録器（route で abort した外部要求・ポップアップ・CSP 違反）。
export function egressCounts(eg, rec) {
  return {
    external: rec.external.length,
    popups: rec.popups.length,
    csp: rec.csp.length,
    offOriginCalls: eg.offOriginCalls,
    otherCalls: eg.otherCalls,
    offOriginRequests: eg.offOriginRequests,
    ws: eg.ws,
    workers: eg.workers,
    serviceWorkers: eg.context.serviceWorkers().length,
    nonGet: eg.nonGet,
    offAllowlist: eg.offAllowlist,
  };
}

export const EGRESS_KEYS = [
  'external', 'popups', 'csp', 'offOriginCalls', 'otherCalls', 'offOriginRequests', 'ws', 'workers', 'serviceWorkers', 'nonGet', 'offAllowlist',
];

// 0 でない区分の名前と件数だけを文字列で返す（値・本文は出さない）。
export function egressFailures(counts, keys = EGRESS_KEYS) {
  return keys.filter((k) => counts[k] !== 0).map((k) => `${k}=${counts[k]}`);
}

// 結果の 1 行（場面・CSP・各区分の件数・fetch の件数）。様式は docs/testing.md 5.371。
export function egressLine(label, csp, counts, eg, extra = '') {
  const cells = EGRESS_KEYS.map((k) => `${k}=${counts[k]}`).join(' ');
  return `egress ${label} (CSP ${csp ? 'あり' : 'なし'}): ${cells} fetch=${eg.allowedFetch}/${eg.calls}${extra ? ` ${extra}` : ''}`;
}
