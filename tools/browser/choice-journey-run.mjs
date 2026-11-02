// 入力欄のない教材の完走の道具（choice-journey.mjs）を設定ファイルで呼ぶ CLI（TASK-748 2/2。SECOND-4・SECOND-7）。
//
// 役割: 教材 repo や CI のジョブなど、node:test の外から「設定ファイル 1 本 = 教材 1 つ」で道具を流す入口。
// 設定ごと・CSP あり／なしごとに、IR を作り（choice-journey-config.mjs の buildIr）、配信する設定の写しを配って完走させ、
// 設定に書いた期待と照合して PASS／FAIL の行を出す。呼び出し元: scripts/check-choice-journey.sh（`make check-choice-journey`。
// CI の browser-test ジョブ）。道具本体は教材固有の値を持たず、教材の違いは設定ファイルだけにある。
//
// 使い方: node tools/browser/choice-journey-run.mjs --config <file> [--config <file> ...]
// 環境変数: FANDHE_PACK_BIN（fandhe-pack の実行ファイル）・FANDHE_WEB_AUTHOR_DIR（IR 同梱の配信する設定。試験用の口なし）・
//           FANDHE_BROWSER_CHANNEL（chromium | chrome）
// 終了コード: 0 = 全件合格 / 1 = 不合格または実行の失敗 / 2 = 誤用（引数・環境変数・設定の形）
// 出力は名前・件数・合否・欄名・数値だけで、教材の文面・入力値・パスの全体を含めない。
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { parseArgs } from 'node:util';

const usage = () => {
  console.error('usage: choice-journey-run.mjs --config <file> [--config <file> ...]');
  process.exit(2);
};

function misuse(msg) {
  console.error(`error: ${msg}`);
  process.exit(2);
}

// playwright が無いとき（npm ci 前）に、取り込みの失敗の長い出力ではなく手順を示して誤用（2）で終える。本体の取り込みは後ろに回す。
try {
  createRequire(import.meta.url).resolve('playwright');
} catch {
  misuse('playwright is not installed; run npm ci && npx playwright install chromium');
}
const { launchBrowser, channelFromEnv, withServer, withCspMode } = await import('./harness.mjs');
const {
  stepsOf, prepareDelivery, runJourney, summaryLine,
} = await import('./choice-journey.mjs');
const {
  loadConfig, buildIr, judge, ConfigError,
} = await import('./choice-journey-config.mjs');

let values;
try {
  ({ values } = parseArgs({ options: { config: { type: 'string', multiple: true } }, allowPositionals: false, strict: true }));
} catch {
  usage();
}
if (!values.config || values.config.length === 0) usage();

const bin = process.env.FANDHE_PACK_BIN;
const webDir = process.env.FANDHE_WEB_AUTHOR_DIR;
const channel = channelFromEnv();
if (channel !== 'chromium' && channel !== 'chrome') misuse('FANDHE_BROWSER_CHANNEL must be chromium or chrome');
if (!bin || !fs.statSync(bin, { throwIfNoEntry: false })?.isFile()) misuse('FANDHE_PACK_BIN must point to the fandhe-pack executable');
if (!webDir || !fs.statSync(webDir, { throwIfNoEntry: false })?.isDirectory()) misuse('FANDHE_WEB_AUTHOR_DIR must point to a delivery web directory built with IR_FILE and without TEST_HOOKS');

// 設定は先にすべて読んで検証する（途中で形の誤りに気づいてブラウザを無駄に起動しない。fail-closed）。
const configs = [];
try {
  for (const f of values.config) configs.push(loadConfig(path.resolve(f)));
} catch (e) {
  if (e instanceof ConfigError) misuse(e.message);
  throw e;
}
const names = configs.map((c) => c.name);
if (new Set(names).size !== names.length) misuse('config names must be unique');

let pass = 0;
let failed = 0;
const browser = await launchBrowser({ channel });
try {
  for (const config of configs) {
    let irText;
    try {
      irText = buildIr({ bin, config });
    } catch (e) {
      if (e instanceof ConfigError) misuse(`${config.name}: ${e.message}`);
      console.log(`FAIL ${config.name} build: ${String(e.message).slice(0, 80)}`);
      failed += config.csp.length;
      continue;
    }
    for (const csp of config.csp) {
      const tag = `${config.name} csp=${csp ? 'on' : 'off'}`;
      let copy;
      try {
        const ir = JSON.parse(irText);
        const info = stepsOf(ir);
        copy = prepareDelivery({ webDir, irText });
        let out;
        await withCspMode({ dir: copy.dir, csp }, (dir) => withServer({ dir, base: config.base }, async (srv) => {
          out = await runJourney({
            browser, srv, ir, sequence: config.sequence, expect: config.sequence.map((s) => s.expect), initial: config.initial,
            titles: info.titles, finals: info.finals, viewport: config.viewport,
          });
        }));
        console.log(summaryLine(config.name, csp, out));
        const why = judge(config, out);
        if (why.length === 0) {
          pass += 1;
          console.log(`PASS ${tag}`);
        } else {
          failed += 1;
          console.log(`FAIL ${tag} ${why.join(',')}`);
        }
      } catch (e) {
        failed += 1;
        console.log(`FAIL ${tag} run: ${String(e.message).slice(0, 80).replace(/[\r\n]+/g, ' ')}`);
      } finally {
        copy?.cleanup();
      }
    }
  }
} finally {
  await browser.close();
}
console.log(`CHOICE-JOURNEY-TOTAL configs=${configs.length} pass=${pass} fail=${failed}`);
process.exit(failed === 0 ? 0 : 1);
