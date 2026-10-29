// egress.mjs の純関数（要求・呼び出しの分類、失敗の文面）の単体試験（SAFE-16。TASK-153）。ブラウザ不要。
// 分類の表が緩むと egress-count.test.mjs の「0 件」が空振りになるため、境界を表で固定する。
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { classifyRequest, classifyCall, egressFailures, relPath } from './egress.mjs';

const O = 'http://127.0.0.1:4000';
const B = '/fandhe-egress/';
const ctx = { origin: O, base: B };
const req = (url, method = 'GET', resourceType = 'fetch') => classifyRequest({ url, method, resourceType }, ctx);

test('relPath は base の外を null にする', () => {
  assert.equal(relPath('/fandhe-egress/app.js', B), 'app.js');
  assert.equal(relPath('/fandhe-egress/', B), '');
  assert.equal(relPath('/other/app.js', B), null);
});

test('配信物の構成要素への GET は ok', () => {
  for (const p of ['', 'index.html', 'app.js', 'app.css', 'icon.svg', 'wasm/fandhe_workshop_web.js', 'wasm/fandhe_workshop_web_bg.wasm', 'ir/main.json', 'test-hooks.js']) {
    assert.equal(req(`${O}${B}${p}`), 'ok', p);
  }
  assert.equal(req(`${O}${B}`, 'HEAD', 'document'), 'ok');
});

test('文書のクエリは ok、サブリソースのクエリは offAllowlist', () => {
  assert.equal(req(`${O}${B}?fixture=ch0-3-chapters`, 'GET', 'document'), 'ok');
  assert.equal(req(`${O}${B}ir/main.json?x=1`), 'offAllowlist');
});

test('構成要素以外のパスと base の外は offAllowlist', () => {
  assert.equal(req(`${O}${B}api/save`), 'offAllowlist');
  assert.equal(req(`${O}${B}sw.js`), 'offAllowlist');
  assert.equal(req(`${O}/other/app.js`), 'offAllowlist');
});

test('GET・HEAD 以外は nonGet', () => {
  for (const m of ['POST', 'PUT', 'PATCH', 'DELETE']) assert.equal(req(`${O}${B}ir/main.json`, m), 'nonGet', m);
});

test('別オリジンは offOrigin、data: と blob: と favicon は ok', () => {
  assert.equal(req('https://example.invalid/'), 'offOrigin');
  assert.equal(req('http://127.0.0.1:4001/x'), 'offOrigin');
  assert.equal(req('data:text/plain,x'), 'ok');
  assert.equal(req('blob:http://127.0.0.1:4000/abc'), 'ok');
  assert.equal(req(`${O}/favicon.ico`), 'ok');
  assert.equal(req('not a url'), 'offAllowlist');
});

const call = (o) => classifyCall({ api: 'fetch', off: false, special: false, unparsed: false, pathname: '', hasQuery: false, method: 'GET', ...o }, { base: B });

test('呼び出しの分類は wasm と IR の GET の fetch だけを許す', () => {
  assert.equal(call({ pathname: `${B}wasm/fandhe_workshop_web_bg.wasm` }), 'allowedFetch');
  assert.equal(call({ pathname: `${B}ir/main.json` }), 'allowedFetch');
  assert.equal(call({ pathname: `${B}ir/main.json`, method: 'POST' }), 'other');
  assert.equal(call({ pathname: `${B}ir/main.json`, hasQuery: true }), 'other');
  assert.equal(call({ pathname: `${B}app.js` }), 'other');
  assert.equal(call({ pathname: `${B}ir/main.json`, api: 'XMLHttpRequest' }), 'other');
  assert.equal(call({ pathname: `${B}ir/main.json`, api: 'sendBeacon', method: 'POST' }), 'other');
  assert.equal(call({ unparsed: true }), 'other');
  assert.equal(call({ off: true }), 'offOrigin');
  assert.equal(call({ off: true, api: 'WebSocket' }), 'offOrigin');
});

test('egressFailures は 0 でない区分の名前と件数だけを返す', () => {
  const zero = Object.fromEntries(['external', 'popups', 'csp', 'offOriginCalls', 'otherCalls', 'offOriginRequests', 'ws', 'workers', 'serviceWorkers', 'nonGet', 'offAllowlist'].map((k) => [k, 0]));
  assert.deepEqual(egressFailures(zero), []);
  assert.deepEqual(egressFailures({ ...zero, ws: 2, nonGet: 1 }), ['ws=2', 'nonGet=1']);
});
