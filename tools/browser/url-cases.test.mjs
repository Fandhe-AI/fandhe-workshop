// url-cases.mjs の単体試験（ブラウザ不要。TASK-147 2/2）。
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { loadUrlCases, buildRuns, parseUrlCases, verifySha, DEFAULT_FILE } from './url-cases.mjs';

const raw = () => JSON.parse(fs.readFileSync(DEFAULT_FILE, 'utf8'));
const xss = (ids) => new Map(ids.map((id) => [id, `payload-${id}`]));

test('既定の写しから 7・8・5 件と 61 回が組み立てられる', () => {
  const c = loadUrlCases();
  assert.equal(c.positions.length, 7);
  assert.equal(c.payloads.length, 8);
  assert.equal(c.irFixed.length, 5);
  assert.equal(buildRuns(c).length, 61);
  const ids = c.payloads.map((p) => p.id);
  for (const id of ['XSS-01', 'XSS-02', 'XSS-07']) assert.ok(ids.includes(id), id);
});

test('query の値は URLSearchParams で、hash の値は decodeURIComponent で元のペイロードに戻る', () => {
  for (const r of buildRuns(loadUrlCases()).filter((x) => !x.rawIr)) {
    if (r.where === 'query') assert.equal(new URLSearchParams(r.query).get(r.name), r.expected, r.runId);
    else assert.equal(decodeURIComponent(r.hash.slice(1)), r.expected, r.runId);
  }
});

test('?ir= の固定の値は再符号化されず、IR-5 の %23 が残る', () => {
  const runs = buildRuns(loadUrlCases()).filter((x) => x.rawIr);
  assert.equal(runs.length, 5);
  assert.ok(runs.find((r) => r.caseId === 'IR-5').query.endsWith('min-3step.json%23x'));
  assert.ok(runs.find((r) => r.caseId === 'IR-2').query.includes('https://example.invalid/XSS-IR2'));
});

test('件数違い・未知の ID・未知の名前・sha256 不一致は例外になる', () => {
  const ids = raw().e6_payloads;
  const j = raw();
  assert.throws(() => parseUrlCases({ ...j, e6_payloads: ids.slice(1) }, xss(ids)), /8 e6_payloads/);
  assert.throws(() => parseUrlCases(j, xss(ids.slice(1))), /payload not found/);
  const renamed = { ...j, e6_positions: j.e6_positions.map((p) => (p.name === 'x' ? { ...p, name: 'y' } : p)) };
  assert.throws(() => parseUrlCases(renamed, xss(ids)), /unexpected query names/);
  const twoHash = { ...j, e6_positions: j.e6_positions.map((p) => (p.name === 'x' ? { id: 'h2', where: 'hash' } : p)) };
  assert.throws(() => parseUrlCases(twoHash, xss(ids)), /unexpected query names/);
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'url-cases-'));
  try {
    const f = path.join(tmp, 'extra-cases.json');
    fs.writeFileSync(f, '{}');
    const sha = path.join(tmp, 'x.sha256');
    fs.writeFileSync(sha, `${'0'.repeat(64)}  extra-cases.json\n`);
    assert.throws(() => verifySha(f, sha), /mismatch/);
    fs.writeFileSync(sha, '');
    assert.throws(() => verifySha(f, sha), /not found/);
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});
