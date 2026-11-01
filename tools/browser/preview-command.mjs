// `fandhe-pack preview` を子プロセスとして起動する補助（preview-command.test.mjs。AUTHOR-22。TASK-429 3/3）。
//
// 役割: 試験本体が「実行して URL を読む」「診断ありで終わるまで待つ」「教材の写しを作って直す」を同じ手順で行えるようにする。
// 呼び出し元: preview-command.test.mjs だけ。子の環境は PATH だけに絞る（cargo の見えない環境で動くことも兼ねて確かめる。
// preview_embedded.rs と同じ）。シェルは通さず、配列引数で起動する。
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const CHILD_ENV = { PATH: '/usr/bin:/bin' };
const URL_RE = /^URL: (http:\/\/127\.0\.0\.1:(\d+))\/preview\/$/;

/** 教材パックの写しを一時ディレクトリに作る。戻りの dispose で消す。元には書かない。 */
export function copyPack(src) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'fandhe-preview-command-'));
  const dir = path.join(root, 'pack');
  fs.cpSync(src, dir, { recursive: true });
  return { dir, dispose: () => fs.rmSync(root, { recursive: true, force: true }) };
}

function bodyLines(file) {
  const lines = fs.readFileSync(file, 'utf8').split('\n');
  if (!lines[0].startsWith('# ') || lines[1] !== '' || !lines[2]) throw new Error('unexpected step body layout');
  return lines;
}

/** Step の Markdown の 3 行目（本文の 1 文）を返す。 */
export const sentenceOf = (file) => bodyLines(file)[2];

/** 3 行目だけを別の文に置き換える。見出し（1 行目）は変えない。 */
export function rewriteSentence(file, sentence) {
  const lines = bodyLines(file);
  lines[2] = sentence;
  fs.writeFileSync(file, lines.join('\n'));
}

/** `preview` を起動し、OK 行と URL 行を読んで待ち受けの情報を返す。stop() で止めて終了を待つ。 */
export function startPreview(bin, packDir, { timeoutMs = 30000 } = {}) {
  const child = spawn(bin, ['preview', packDir], { env: CHILD_ENV, stdio: ['ignore', 'pipe', 'pipe'] });
  let buf = '';
  let stderrBytes = 0;
  const exited = new Promise((resolve) => child.once('close', (code, signal) => resolve({ code, signal })));
  // 読み切らないとパイプが詰まって子が止まるため、捨てながら大きさだけ数える。
  child.stderr.on('data', (d) => { stderrBytes += d.length; });
  const stop = async () => {
    if (child.exitCode === null && child.signalCode === null) child.kill('SIGTERM');
    return exited;
  };
  return new Promise((resolve, reject) => {
    let settled = false;
    const timer = setTimeout(() => {
      stop().then(() => { if (!settled) { settled = true; reject(new Error('preview did not print a URL in time')); } });
    }, timeoutMs);
    child.stdout.on('data', (d) => {
      buf += d;
      const lines = buf.split('\n');
      if (settled || lines.length < 3) return;
      settled = true;
      clearTimeout(timer);
      const m = URL_RE.exec(lines[1]);
      if (!lines[0].startsWith('OK: ') || !m) {
        stop().then(() => reject(new Error(`unexpected preview output (stderr ${stderrBytes} bytes)`)));
        return;
      }
      resolve({ url: `${m[1]}/preview/`, origin: m[1], port: Number(m[2]), stop });
    });
    exited.then(({ code }) => {
      clearTimeout(timer);
      if (!settled) {
        settled = true;
        reject(new Error(`preview exited before printing a URL (exit ${code})`));
      }
    });
  });
}

/** 診断ありの経路: 待ち受けに入らず終わるのを待つ。時間切れは kill してから失敗にする。 */
export function runPreviewToExit(bin, packDir, timeoutMs = 30000) {
  return new Promise((resolve, reject) => {
    const child = spawn(bin, ['preview', packDir], { env: CHILD_ENV, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (d) => { stdout += d; });
    child.stderr.on('data', (d) => { stderr += d; });
    const timer = setTimeout(() => {
      child.kill('SIGKILL');
      reject(new Error('preview did not exit in time (it must not serve a pack with diagnostics)'));
    }, timeoutMs);
    child.once('close', (status) => {
      clearTimeout(timer);
      resolve({ status, stdout, stderr });
    });
  });
}
