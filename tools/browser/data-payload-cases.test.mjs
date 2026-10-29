// data-payload-cases.mjs の試験（ブラウザ不要。TASK-148 1/2）。件数・ID の解決・sha256 の照合が fail-closed であること。
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  loadDataPayloadCases, parseDataPayloadCases, verifySha, DEFAULT_FILE, DEFAULT_SHA_FILE,
} from './data-payload-cases.mjs';

const json = () => JSON.parse(fs.readFileSync(DEFAULT_FILE, 'utf8'));
const xss = () => new Map(['XSS-01', 'XSS-07', 'XSS-13', 'XSS-63', 'XSS-53', 'XSS-49'].map((id) => [id, `p-${id}`]));

test('写しを読むと対象 10 件・値 13 件（XSS 6 + 壊れた値 7）になる', () => {
  const c = loadDataPayloadCases();
  assert.equal(c.targets.length, 10);
  assert.equal(c.values.length, 13);
  assert.deepEqual(c.values.slice(0, 6).map((v) => v.label), ['XSS-01', 'XSS-07', 'XSS-13', 'XSS-63', 'XSS-53', 'XSS-49']);
  assert.ok(c.values.slice(0, 6).every((v) => v.value !== '' && v.value !== v.label));
});

test('対照: 件数が違う・XSS の ID が引けない・欄が無いと例外', () => {
  const j = json();
  j.e5_data_payload.targets.pop();
  assert.throws(() => parseDataPayloadCases(j, xss()), /targets/);
  const k = json();
  k.e5_data_payload.raw_values.push('x');
  assert.throws(() => parseDataPayloadCases(k, xss()), /raw_values/);
  const m = xss();
  m.delete('XSS-63');
  assert.throws(() => parseDataPayloadCases(json(), m), /XSS-63/);
  assert.throws(() => parseDataPayloadCases({}, xss()), /e5_data_payload/);
});

test('対照: sha256 が一致しない写しは例外', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'dp-cases-'));
  try {
    const f = path.join(dir, 'extra-cases.json');
    fs.writeFileSync(f, `${fs.readFileSync(DEFAULT_FILE, 'utf8')} `);
    assert.throws(() => verifySha(f, DEFAULT_SHA_FILE), /sha256/);
    verifySha(DEFAULT_FILE, DEFAULT_SHA_FILE);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
