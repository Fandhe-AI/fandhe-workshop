// L2 の配布物（zip）を展開し、模擬環境で ./start.sh を起動・停止する足場（TASK-112 2/2・TASK-113・TASK-114 1/2）。
//
// 役割: 完走試験（l2-run.test.mjs）と、L1・L2 の一致の試験（l1-l2-match.test.mjs。DLV-15）が同じ起動の手順を使うための共有部品。
// 試験ではないため `.test.mjs` にしない（試験ファイルを import すると、試験が二重に登録されるため）。
// 模擬環境は `PATH=/usr/bin:/bin` だけで、環境変数は PATH・HOME・PORT・FANDHE_NO_OPEN だけを渡す（親の環境を引き継がない）。
import { spawn, spawnSync } from 'node:child_process';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';

export const SIM_PATH = '/usr/bin:/bin';

/** 127.0.0.1 の空いているポートを 1 つ返す。 */
export function freePort() {
  return new Promise((resolve, reject) => {
    const s = net.createServer();
    s.once('error', reject);
    s.listen(0, '127.0.0.1', () => {
      const { port } = s.address();
      s.close(() => resolve(port));
    });
  });
}

/** `url` が 200 を返すまで待つ。時間切れは例外。 */
export async function waitOk(url, ms = 20000, interval = 100) {
  const end = Date.now() + ms;
  for (;;) {
    try {
      const r = await fetch(url);
      if (r.status === 200) return;
    } catch {
      // まだ待ち受けていない。
    }
    if (Date.now() > end) throw new Error('server did not become ready');
    await new Promise((r) => setTimeout(r, interval));
  }
}

/** zip を `work` へ展開し、展開先の最上位（fandhe-workshop-l2）を返す。 */
export function unzipTo(zip, work) {
  const un = spawnSync('/usr/bin/unzip', ['-q', zip, '-d', work], { env: { PATH: SIM_PATH }, encoding: 'utf8' });
  assert.equal(un.status, 0, 'unzip failed');
  return path.join(work, 'fandhe-workshop-l2');
}

/** 展開して start.sh を模擬環境で起動する。`mutate` があれば start.sh の本文を書き換える（対照用。展開先の写しだけ）。 */
export async function startL2(zip, work, { mutate } = {}) {
  const top = unzipTo(zip, work);
  if (mutate) {
    const f = path.join(top, 'start.sh');
    fs.writeFileSync(f, mutate(fs.readFileSync(f, 'utf8')));
  }
  const port = await freePort();
  const t0 = process.hrtime.bigint();
  const child = spawn('./start.sh', [], {
    cwd: top,
    env: { PATH: SIM_PATH, HOME: work, PORT: String(port), FANDHE_NO_OPEN: '1' },
    stdio: ['ignore', 'pipe', 'pipe'],
    detached: true,
  });
  const run = { child, port, url: `http://127.0.0.1:${port}/`, out: '', startMs: 0 };
  child.stdout.on('data', (d) => { run.out += d; });
  child.stderr.on('data', (d) => { run.out += d; });
  try {
    await waitOk(run.url, 20000, 10);
  } catch (e) {
    // 呼び出し側は run を受け取れないため、ここで子の群を止めてから投げ直す。
    await stopL2(run);
    throw e;
  }
  run.startMs = Number(process.hrtime.bigint() - t0) / 1e6;
  return run;
}

/** 起動した子の群（start.sh と miniserve）を止め、終了を待つ。`run` が null・未起動なら何もしない。 */
export async function stopL2(run) {
  if (!run?.child?.pid) return;
  const { child } = run;
  try {
    process.kill(-child.pid, 'SIGTERM');
  } catch {
    // すでに終了している。
  }
  await new Promise((r) => {
    if (child.exitCode !== null || child.signalCode !== null) r();
    else child.once('exit', r);
  });
}
