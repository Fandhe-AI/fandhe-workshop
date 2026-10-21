// CSP の meta の配置と内容の検査（SAFE-13。TASK-151 1/2）。
// <meta http-equiv> の CSP は要素より後の内容にしか効かないため、配信する index.html（配信・試験の両設定）で
// charset の meta の直後にあり、内容が仕様の文字列と完全一致することを、失敗で止める検査として確かめる。
// ブラウザは起動しない。呼び出し元は scripts/test-browser.sh（make test-browser。CI の browser-test ジョブ）。
// 環境変数が無い・ファイルが無い場合は skip せず失敗にする（fail-closed）。
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { DELIVERY_CSP, inspectCspMeta, stripCspMeta } from './harness.mjs';

function readIndex(envName) {
  const dir = process.env[envName];
  assert.ok(dir, `${envName} is not set`);
  return fs.readFileSync(path.join(dir, 'index.html'), 'utf8');
}

for (const [name, envName] of [['delivery', 'FANDHE_WEB_DIR'], ['test', 'FANDHE_WEB_TEST_DIR']]) {
  test(`csp-placement: ${name} 設定の index.html は CSP が charset の直後にあり内容が一致する`, () => {
    const r = inspectCspMeta(readIndex(envName));
    assert.deepEqual(r.problems, [], `${name}: ${r.problems.join('; ')}`);
    assert.equal(r.ok, true);
  });
}

// 対照: 実物を加工した写しが、期待の理由で落ちること（検査が空振りしないことの記録）。
const CSP_LINE = /[ \t]*<meta\b[^>]*http-equiv=["']Content-Security-Policy["'][^>]*>\n?/i;
const CHARSET_LINE = /[ \t]*<meta charset="utf-8">\n/i;

function mutate(html, fn) {
  const cspLine = CSP_LINE.exec(html)?.[0];
  assert.ok(cspLine, 'CSP meta line not found in the real index.html');
  assert.ok(CHARSET_LINE.test(html), 'charset meta line not found in the real index.html');
  return fn(html, cspLine);
}

test('csp-placement: 正の例（加工前の実物）が通る', () => {
  assert.equal(inspectCspMeta(readIndex('FANDHE_WEB_DIR')).ok, true);
});

const cases = [
  ['charset と CSP の間に viewport を挟む', /not immediately after charset/, (h, c) =>
    h.replace(c, '  <meta name="viewport" content="width=device-width">\n' + c)],
  ['指令から wasm-unsafe-eval を外す', /CSP content mismatch/, (h) =>
    h.replace(" 'wasm-unsafe-eval'", '')],
  ['img-src を * にする', /CSP content mismatch/, (h) => h.replace("img-src 'self'", 'img-src *')],
  ['CSP の meta を消す', /CSP meta count is 0/, (h) => stripCspMeta(h).html],
  ['CSP の meta を 2 つにする', /CSP meta count is 2/, (h, c) => h.replace(c, c + c)],
  ['CSP の meta を charset より前に置く', /not immediately after charset/, (h, c) =>
    h.replace(c, '').replace(CHARSET_LINE, (m) => c + m)],
  ['http-equiv を data-http-equiv にする', /CSP meta count is 0/, (h) =>
    h.replace(/\bhttp-equiv=/i, 'data-http-equiv=')],
  ['http-equiv の値の前後に空白を足す', /CSP meta count is 0/, (h) =>
    h.replace(/\bhttp-equiv=(["'])Content-Security-Policy\1/i, 'http-equiv=$1 Content-Security-Policy $1')],
  ['content を data-content にする', /no content attribute/, (h, c) =>
    h.replace(c, c.replace(/\scontent=/i, ' data-content='))],
  ['charset と CSP の間にコメントを挟む', /not immediately after charset/, (h, c) =>
    h.replace(c, '  <!-- x -->\n' + c)],
  ['charset と CSP をまとめてコメントで囲む', /charset meta count is 0/, (h, c) =>
    h.replace(CHARSET_LINE, (m) => '<!-- ' + m).replace(c, c + ' -->')],
  ['CSP だけをコメントで囲む', /CSP meta count is 0/, (h, c) => h.replace(c, '<!-- ' + c + ' -->')],
  ['CSP を script の中身に置く', /CSP meta count is 0/, (h, c) => h.replace(c, '<script>"' + c.replace(/"/g, "'") + '"</script>')],
  ['charset と CSP をまとめて template で囲む', /charset meta count is 0/, (h, c) =>
    h.replace(CHARSET_LINE, (m) => '<template>' + m).replace(c, c + '</template>')],
  ['charset と CSP をまとめて noscript で囲む', /charset meta count is 0/, (h, c) =>
    h.replace(CHARSET_LINE, (m) => '<noscript>' + m).replace(c, c + '</noscript>')],
  ['CSP だけを noscript で囲む', /CSP meta count is 0/, (h, c) => h.replace(c, '<noscript>' + c + '</noscript>')],
  ['CSP だけを入れ子の template で囲む', /CSP meta count is 0/, (h, c) =>
    h.replace(c, '<template><template></template>' + c + '</template>')],
  // head を暗黙に閉じる形（ブラウザでは後続の meta が body に入り CSP が効かない。2026-10-21 のレビュー指摘）と、
  // head の開始の直後を charset に限る厳しい側の規則（title は head を閉じないが charset より前には置かせない）。
  ['head の開始の直後に div を置く', /charset meta is not the first content of head/, (h) =>
    h.replace(/<head>/i, '<head><div></div>')],
  ['head の開始の直後に文字を置く', /charset meta is not the first content of head/, (h) => h.replace(/<head>/i, '<head>x')],
  ['head の開始の直後に NBSP を置く', /charset meta is not the first content of head/, (h) =>
    h.replace(/<head>/i, '<head>\u00a0')],
  ['head の開始と charset の間に title を置く', /charset meta is not the first content of head/, (h) =>
    h.replace(/<head>/i, '<head><title>t</title>')],
  ['head より前に body を始める', /unexpected content before head/, (h) =>
    h.replace(/<head>/i, '<body><head>')],
  ['head より前に文字を置く', /unexpected content before head/, (h) => h.replace(/<head>/i, 'x<head>')],
  ['head を空にして閉じ直す', /head element count is 1\/2/, (h) => h.replace(/<head>/i, '<head></head>')],
];
for (const [label, reason, fn] of cases) {
  test(`csp-placement: 対照 ${label}`, () => {
    const r = inspectCspMeta(mutate(readIndex('FANDHE_WEB_DIR'), fn));
    assert.equal(r.ok, false);
    assert.match(r.problems.join('; '), reason);
  });
}

test('csp-placement: 期待の CSP の文字列が仕様どおりである', () => {
  assert.equal(
    DELIVERY_CSP,
    "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; img-src 'self'",
  );
});
