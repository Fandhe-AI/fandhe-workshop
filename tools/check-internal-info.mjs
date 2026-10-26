#!/usr/bin/env node
// 公開前の確認（LIC-12。D-40・D-77・D-78・D-81。TASK-139。`make check-internal-info`）の照合のスクリプト。
//
// 目的: 公開する repo に、社内固有の情報（社内 runner の名前・private repo の名前・社内の手順・社内の人名）と
// 認証情報（トークン・鍵）が 0 件であることを、機械で確かめる。紛らわしい架空の名前の判断と最終の承認は人が行う
// （D-78）。結果の記録は docs/publish-checks.md。
//
// 2 つの照合:
//   1. 社内固有の語: `--list <file>` の一覧（1 行 1 語。`#` 行と空行は読み飛ばす）で照合する。一覧の正は private の
//      spec repo にあり、ユーザーが更新する（D-77）。この repo へ写さない。一覧が git で追跡されていたら断る
//      （public の repo へ語を持ち込む経路を閉じる）。`--list` の既定値は持たない。
//      対象: docs/examples/**・testdata/**・crates/web/static/**・.github/workflows/**（LIC-12 の 4 種類を本 repo に
//      当てはめたもの。教材パック・練習データ・配信物の画面と試験用教材・CI 設定）。教材 repo と講師資料は本 repo の
//      外で、TASK-794 で照合する。`--all` で追跡中の全ファイルを対象にする（対象外の箇所の洗い出し用）。
//   2. 認証情報の形: `--secrets-only` で、追跡中の全ファイル（外部取得のスキルを含む）を照合する。判定の正は
//      crates/core/src/secret_scan.rs で、JS へ写さず crates/pack/examples/secret_scan_files.rs を呼ぶ。
//      意図した試験用の偽の値は SECRET_EXCEPTIONS に、確かめたファイル内容の SHA-256 で固定して持つ（内容が変われば失敗する）。
//
// `--list` も `--secrets-only` も無ければ 2 で止める（語の照合を黙って飛ばさない）。`--secrets-only` では出力に
// 「words: not checked」を出す。
//
// 照合の規則: 行と語を NFKC で正規化して小文字にし、文字列の部分一致で見る（正規表現・eval・シェルには渡さない）。
// 出力: 違反 1 件につき `パス:行番号: term#<番号>` または `パス:行番号: secret:<形の ID>`。語・値は出さない。
// 標準エラーに `list: terms=<n> sha256=<hash>` を出す（記録に一覧の版を残す。中身は出さない）。
// 終了コード: 0 = 違反なし / 1 = 違反あり / 2 = 誤用、または検査を完了できない（合格にしない）。
// 使い方: node tools/check-internal-info.mjs (--list <file> | --secrets-only) [--root <dir>] [--all]
//   --root は回帰テスト用（この repo 以外を指すときは固定した例外を使わない）（既定はこのファイルの親の親）。
//   環境変数 FANDHE_TERM_LIST に一覧のパスを渡せる（`--list` が無いときだけ使う。make はこちらで渡し、パスをシェルの構文にしない）。
//   環境変数 FANDHE_SECRET_SCAN_BIN にビルド済みの example を渡すと cargo run を省く。
// 回帰テスト: tools/check-internal-info.test.mjs（scripts/test-check-internal-info.sh）。
// CI・make check・lefthook には組み込まない（継続の照合は TASK-420 以降。失敗で止めるガードは人間の承認点）。
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const TOOL_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

// 語の照合の既定の対象（相対パスの接頭辞）。
export const WORD_SCOPE = ['docs/examples/', 'testdata/', 'crates/web/static/', '.github/workflows/'];

// 認証情報の形の固定した例外。キーは相対パス、値は目で確かめたときのファイル内容の SHA-256。意図した試験用の偽の値だけを入れる。
// 内容が 1 バイトでも変わったら例外を適用せず、そのファイルの当たりを違反として出す（同じ件数の別の値や、同じ行への
// 同型の資格情報の追加を通さないため。件数は使わない）。再確認したら新しいハッシュに書き直す。
export const SECRET_EXCEPTIONS = new Map([
  // 外部取得のスキル（.agents/skills。skills-lock.json 管理で、この repo では編集しない）の例示・試験の値。
  // 2026-10-26 に目で確かめた。いずれも `u:p@h`・`${TOKEN}`・RFC 7519 の例の JWT のようなダミーか変数の展開で、
  // 実在の資格情報ではない。スキルの日次更新で内容が変わったら、新しい当たりを同じ手順で確かめ直す。
  ['.agents/skills/create-commit/SKILL.md', '8fbc57f725711d87768e8c782b32dcd99f1de4bab2482a90ca289b1d31d3ba03'],
  ['.agents/skills/implement-issue-tree/tests/base-merge-unit.test.mjs', '3c69a58c4445231a93051608a94de187c65800fd2a038bb75707da9c2989cfa4'],
  ['.agents/skills/implement-issue-tree/tests/lib/base-merge-suite.mjs', 'c839fa3bae8f9df121ed68d4ef6d5e727060b7f94dc31027d121adc0c4e9195e'],
  ['.agents/skills/implement-issue-tree/tests/lib/pr-push-suite.mjs', '1436147017db731bdfafe6ac11a3e4e7e537155238b1b6c7ef674f07f32b9406'],
]);

class CheckError extends Error {}

function git(root, args) {
  const r = spawnSync('git', ['-C', root, ...args], { encoding: 'buffer', maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new CheckError(`git ${args[0]} failed`);
  return r.stdout;
}

// 追跡中の通常ファイル（gitlink と symlink は除く）の相対パス。
export function trackedFiles(root) {
  const out = git(root, ['ls-files', '-s', '-z']).toString('utf8');
  const files = [];
  for (const rec of out.split('\0')) {
    if (rec === '') continue;
    const tab = rec.indexOf('\t');
    const mode = rec.slice(0, 6);
    if (mode !== '100644' && mode !== '100755') continue;
    files.push(rec.slice(tab + 1));
  }
  return files;
}

const norm = (s) => s.normalize('NFKC').toLowerCase();

// 語の一覧を読む。読めない・UTF-8 でない・0 語・repo で追跡されている場合は CheckError。
export function loadList(file, root) {
  let buf;
  try {
    buf = fs.readFileSync(file);
  } catch {
    throw new CheckError('term list is not readable');
  }
  let text;
  try {
    text = new TextDecoder('utf-8', { fatal: true }).decode(buf);
  } catch {
    throw new CheckError('term list is not UTF-8');
  }
  const terms = text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l !== '' && !l.startsWith('#'))
    .map(norm);
  if (terms.length === 0) throw new CheckError('term list has no terms');
  const dir = path.dirname(path.resolve(file));
  const a = spawnSync('git', ['-C', dir, 'rev-parse', '--show-toplevel'], { encoding: 'utf8' });
  if (a.status === 0) {
    const mine = spawnSync('git', ['-C', root, 'rev-parse', '--show-toplevel'], { encoding: 'utf8' });
    if (mine.status === 0 && fs.realpathSync(a.stdout.trim()) === fs.realpathSync(mine.stdout.trim())) {
      const t = spawnSync('git', ['-C', dir, 'ls-files', '--error-unmatch', path.basename(file)], { encoding: 'utf8' });
      if (t.status === 0) throw new CheckError('term list is tracked by this repository');
    }
  }
  return { terms, sha256: crypto.createHash('sha256').update(buf).digest('hex') };
}

// 語の照合。違反は {file, line, id}。
export function scanWords(root, files, terms) {
  const hits = [];
  for (const rel of files) {
    const text = fs.readFileSync(path.join(root, rel)).toString('utf8');
    text.split('\n').forEach((line, i) => {
      const n = norm(line);
      terms.forEach((t, k) => {
        if (n.includes(t)) hits.push({ file: rel, line: i + 1, id: `term#${k + 1}` });
      });
    });
  }
  return hits;
}

// 認証情報の形の照合。example を呼び、当たった {file, line, id} を返す。
export function scanSecrets(root, files) {
  const abs = files.map((f) => path.join(path.resolve(root), f));
  const input = abs.join('\0') + '\0';
  const bin = process.env.FANDHE_SECRET_SCAN_BIN;
  const r = bin
    ? spawnSync(bin, [], { input, encoding: 'utf8', maxBuffer: 1 << 28 })
    : spawnSync(
        'cargo',
        ['run', '--quiet', '--locked', '-p', 'fandhe-workshop-pack', '--example', 'secret_scan_files'],
        { input, encoding: 'utf8', cwd: TOOL_ROOT, maxBuffer: 1 << 28 },
      );
  if (r.status !== 0 && r.status !== 1) throw new CheckError('secret scan could not complete');
  const prefix = path.join(path.resolve(root), path.sep);
  const hits = [];
  for (const l of r.stdout.split('\n')) {
    if (l === '') continue;
    const m = /^(.*):(\d+): ([a-z_]+)$/.exec(l);
    if (!m || !m[1].startsWith(prefix)) throw new CheckError('unexpected scanner output');
    hits.push({ file: m[1].slice(prefix.length), line: Number(m[2]), id: `secret:${m[3]}` });
  }
  return hits;
}

// 固定した例外を引き、残りの違反と、内容の不一致の報告を返す。hashOf(相対パス) はファイル内容の SHA-256。
// 内容のハッシュが承認したものと一致するファイルの当たりだけを引く。
export function applyExceptions(hits, exceptions, hashOf) {
  const rest = [];
  const drift = new Set();
  for (const h of hits) {
    const want = exceptions.get(h.file);
    if (want !== undefined && hashOf(h.file) === want) continue;
    if (want !== undefined) drift.add(`exception content changed: ${h.file} (re-verify and update the hash)`);
    rest.push(h);
  }
  return { rest, drift: [...drift] };
}

function parseArgs(argv) {
  const o = { list: null, secretsOnly: false, root: TOOL_ROOT, all: false };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--secrets-only') o.secretsOnly = true;
    else if (a === '--all') o.all = true;
    else if ((a === '--list' || a === '--root') && i + 1 < argv.length) {
      i += 1;
      if (a === '--list') o.list = argv[i];
      else o.root = argv[i];
    } else throw new CheckError(`unknown argument: ${a}`);
  }
  if (o.list === null && !o.secretsOnly && process.env.FANDHE_TERM_LIST) o.list = process.env.FANDHE_TERM_LIST;
  if (o.list === null && !o.secretsOnly) throw new CheckError('either --list <file> or --secrets-only is required');
  return o;
}

function main() {
  const o = parseArgs(process.argv.slice(2));
  const files = trackedFiles(o.root);
  const hits = [];
  if (o.list !== null) {
    const { terms, sha256 } = loadList(o.list, o.root);
    process.stderr.write(`list: terms=${terms.length} sha256=${sha256}\n`);
    const scope = o.all ? files : files.filter((f) => WORD_SCOPE.some((p) => f.startsWith(p)));
    hits.push(...scanWords(o.root, scope, terms));
    process.stdout.write(`words: checked ${scope.length} files\n`);
  } else {
    process.stdout.write('words: not checked (--secrets-only)\n');
  }
  const { rest, drift } = applyExceptions(scanSecrets(o.root, files), path.resolve(o.root) === TOOL_ROOT ? SECRET_EXCEPTIONS : new Map(),
    (rel) => crypto.createHash('sha256').update(fs.readFileSync(path.join(o.root, rel))).digest('hex'),
  );
  hits.push(...rest);
  process.stdout.write(`secrets: checked ${files.length} files\n`);
  for (const h of hits) process.stdout.write(`${h.file}:${h.line}: ${h.id}\n`);
  for (const d of drift) process.stdout.write(`${d}\n`);
  process.exit(hits.length + drift.length > 0 ? 1 : 0);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (e) {
    process.stderr.write(`error: ${e instanceof CheckError ? e.message : 'unexpected failure'}\n`);
    process.exit(2);
  }
}
