// 前提バイナリ fandhe-pack の受け渡しの試験（`make test-browser` から呼ばれる。BUILD-16。TASK-91）。
//
// scripts/test-browser.sh が scripts/build-pack-bin.sh で解決した位置を FANDHE_PACK_BIN で受け取り、
// その実行ファイルが実行できること（引数なしは usage を stderr へ出して終了コード 2）だけを確かめる。
// compile の出力内容は assert しない（本タスクの範囲外）。ブラウザは起動しない。
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { packBinFromEnv } from './harness.mjs';

test('FANDHE_PACK_BIN の fandhe-pack が実行でき、引数なしは usage で終了コード 2', () => {
  const bin = packBinFromEnv();
  const r = spawnSync(bin, [], { encoding: 'utf8' });
  assert.equal(r.error, undefined);
  assert.equal(r.status, 2);
  assert.match(r.stderr, /^usage: fandhe-pack/);
});
