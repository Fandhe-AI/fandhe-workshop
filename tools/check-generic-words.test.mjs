// tools/check-generic-words.mjs（教材固有の語の検査。W-22）の回帰テスト（TASK-471 3/3）。
//
// 作業ツリーは変えない。一時ディレクトリへ小さな crates/ と docs/vocabulary/vocabulary.md を組み立てて検査を流す
// （実 repo に対しては読むだけ）。語を足した写しで失敗すること（負の対照）と、判定できない入力で 2 になること
// （fail-closed）を主張する。
import { test, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const TOOL = fileURLToPath(new URL('./check-generic-words.mjs', import.meta.url));
const REPO = fileURLToPath(new URL('..', import.meta.url));

const APPENDIX = `# 語彙

## 付録 A. 固有の語の一覧

| 出所 | 語 | 照合 |
|---|---|---|
| 掲示板 | \`bbs\` | 部分一致 |
| 掲示板 | \`thread\` | 語の一致 |
| GitHub | \`github\` | 部分一致 |
| GitHub | \`pr\` | 語の一致 |
| GitHub | \`issue\` | 語の一致 |
| Claude Code | \`skill\` | 語の一致 |

- 注記
`;

// command_class.rs の例外（skill が 3 件）を満たす最小の写し。
const COMMAND_CLASS = 'enum K { Skill }\nfn f() { let _ = "skill"; let _ = "skill"; }\n';

const dirs = [];
afterEach(() => {
  while (dirs.length > 0) fs.rmSync(dirs.pop(), { recursive: true, force: true });
});

// 一時 root を作る。files は { 相対パス: 内容 }。既定で付録 A と例外の写しを置く。
function makeRoot(files = {}, { appendix = APPENDIX, commandClass = COMMAND_CLASS } = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'cgw-'));
  dirs.push(root);
  const put = (rel, body) => {
    fs.mkdirSync(path.dirname(path.join(root, rel)), { recursive: true });
    fs.writeFileSync(path.join(root, rel), body);
  };
  if (appendix !== null) put('docs/vocabulary/vocabulary.md', appendix);
  put('crates/core/src/lib.rs', 'pub fn ok() {}\n');
  put('crates/core/src/command_class.rs', commandClass);
  for (const [rel, body] of Object.entries(files)) put(rel, body);
  return root;
}

function run(root, extra = []) {
  const r = spawnSync(process.execPath, [TOOL, '--root', root, ...extra], { encoding: 'utf8' });
  return { code: r.status, out: r.stdout + r.stderr };
}

test('実 repo は違反 0 で終わる', () => {
  const r = spawnSync(process.execPath, [TOOL, '--root', REPO], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stdout + r.stderr);
});

test('既定の root（引数なし）でも実 repo を検査して 0 になる', () => {
  const r = spawnSync(process.execPath, [TOOL], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stdout + r.stderr);
});

test('写しの基準（語なし）は 0', () => {
  assert.equal(run(makeRoot()).code, 0);
});

test('識別子に語を足すと 1: 部分一致・CamelCase の語の一致・snake_case の語の一致', () => {
  for (const body of ['fn github_url() {}', 'struct IssueId;', 'fn open_pr() {}', 'fn bbsx() {}']) {
    const r = run(makeRoot({ 'crates/x/src/a.rs': `${body}\n` }));
    assert.equal(r.code, 1, `${body}: ${r.out}`);
    assert.match(r.out, /crates\/x\/src\/a\.rs:1: /, r.out);
  }
});

test('文字列リテラルに語を足すと 1。違反の出力は文字列の値そのものを含まない', () => {
  const r = run(makeRoot({ 'crates/x/src/a.rs': 'const A: &str = "open pr now";\n' }));
  assert.equal(r.code, 1, r.out);
  assert.match(r.out, /a\.rs:1: pr/);
  assert.ok(!r.out.includes('open pr now'), r.out);
});

test('語の一致は語の境界だけ: 一般語に含まれる部分文字列は違反にしない', () => {
  const r = run(makeRoot({ 'crates/x/src/a.rs': 'fn spring() {} fn prompt() {} fn reissue_x() {}\n' }));
  assert.equal(r.code, 0, r.out);
});

test('コメントにだけ書いた語は 0（行・ブロック・doc・入れ子）', () => {
  const body = [
    '// github issue pr',
    '/// bbs thread',
    '//! skill',
    '/* github /* nested pr */ still comment issue */',
    'fn f() {}',
    '',
  ].join('\n');
  assert.equal(run(makeRoot({ 'crates/x/src/a.rs': body })).code, 0);
});

test('コメントの記号に見える文字列の中の語は、文字列として検査される', () => {
  const r = run(makeRoot({ 'crates/x/src/a.rs': 'const A: &str = "// github";\n' }));
  assert.equal(r.code, 1, r.out);
});

test('生文字列・バイト列・文字リテラル・ライフタイムを正しく読む', () => {
  const ok = [
    "fn f<'a>(x: &'a str) -> &'a str { let _c = 'x'; let _q = '\\''; x }",
    "fn g() { let _ = b'a'; let _ = br\"ok\"; }",
    'fn h() { let _ = r#"say "hi" there"#; }',
    'fn i() { let r#type = 1; let _ = r#type; }',
    '',
  ].join('\n');
  assert.equal(run(makeRoot({ 'crates/x/src/a.rs': ok })).code, 0);
  const bad = run(makeRoot({ 'crates/x/src/a.rs': 'fn f() { let _ = r#"a "q" github"#; }\n' }));
  assert.equal(bad.code, 1, bad.out);
});

test('tests/ に書いた語は対象外で 0', () => {
  const r = run(makeRoot({ 'crates/x/tests/a.rs': 'fn github_pr_issue() { let _ = "bbs-tutorial"; }\n' }));
  assert.equal(r.code, 0, r.out);
});

test('static の js・html・svg・css と Cargo.toml も対象。コメントは除く', () => {
  const cases = {
    'crates/x/static/a.js': ["const a = 'github';\n", '// github\n/* issue */ const b = 1;\n'],
    'crates/x/static/a.html': ['<p id="pr">x</p>\n', '<!-- github -->\n<p>x</p>\n'],
    'crates/x/static/a.svg': ['<g id="bbs"/>\n', '<!-- bbs -->\n<g/>\n'],
    'crates/x/static/a.css': ['.thread { color: red; }\n', '/* thread */ .a { color: red; }\n'],
    'crates/x/Cargo.toml': ['[package]\nname = "github-x"\n', '# github\n[package]\nname = "x"\n'],
  };
  for (const [rel, [bad, good]] of Object.entries(cases)) {
    assert.equal(run(makeRoot({ [rel]: bad })).code, 1, rel);
    assert.equal(run(makeRoot({ [rel]: good })).code, 0, rel);
  }
});

test('例外の件数: command_class.rs の skill を 1 増やしても 1 減らしても 1', () => {
  assert.equal(run(makeRoot({}, { commandClass: `${COMMAND_CLASS}const A: &str = "skill";\n` })).code, 1);
  assert.equal(run(makeRoot({}, { commandClass: 'enum K { Skill }\nfn f() { let _ = "skill"; }\n' })).code, 1);
  // 例外は command_class.rs の skill だけ。別のファイルの skill は違反。
  assert.equal(run(makeRoot({ 'crates/x/src/a.rs': 'enum K { Skill }\n' })).code, 1);
});

test('許可は字句の完全一致だけ: thread_local は 0、thread_pool は 1', () => {
  assert.equal(run(makeRoot({ 'crates/x/src/a.rs': 'thread_local! { static X: u8 = 0; }\n' })).code, 0);
  assert.equal(run(makeRoot({ 'crates/x/src/a.rs': 'fn thread_pool() {}\n' })).code, 1);
  assert.equal(run(makeRoot({ 'crates/x/src/a.rs': 'enum A { SlashCount }\n' })).code, 0);
});

test('fail-closed: 付録 A が無い・0 語・未知の照合の値は 2', () => {
  assert.equal(run(makeRoot({}, { appendix: null })).code, 2);
  assert.equal(run(makeRoot({}, { appendix: '# 語彙\n\n## その他\n' })).code, 2);
  assert.equal(run(makeRoot({}, { appendix: '## 付録 A. 一覧\n\n| 出所 | 語 | 照合 |\n|---|---|---|\n' })).code, 2);
  const unknown = APPENDIX.replace('| 部分一致 |\n| 掲示板 | `thread`', '| 前方一致 |\n| 掲示板 | `thread`');
  assert.notEqual(unknown, APPENDIX);
  assert.equal(run(makeRoot({}, { appendix: unknown })).code, 2);
});

test('fail-closed: 閉じていない文字列・ブロックコメント・crates 無しは 2', () => {
  assert.equal(run(makeRoot({ 'crates/x/src/a.rs': 'const A: &str = "abc;\n' })).code, 2);
  assert.equal(run(makeRoot({ 'crates/x/src/a.rs': '/* never closed\n' })).code, 2);
  assert.equal(run(makeRoot({ 'crates/x/static/a.js': "const a = 'abc;\n" })).code, 2);
  assert.equal(run(makeRoot({ 'crates/x/static/a.html': '<!-- never closed\n' })).code, 2);
  const empty = fs.mkdtempSync(path.join(os.tmpdir(), 'cgw-'));
  dirs.push(empty);
  assert.equal(run(empty).code, 2);
});

test('fail-closed: シンボリックリンクは辿らず 2', () => {
  const root = makeRoot();
  fs.symlinkSync(path.join(root, 'crates/core/src/lib.rs'), path.join(root, 'crates/core/src/link.rs'));
  assert.equal(run(root).code, 2);
});

test('Rust のバイト文字: 16 進・単純なエスケープは正当で 0', () => {
  const body = "fn f() { let _ = [b'a', b'\\x41', b'\\n', b'\\'', b'\\\\']; }\n";
  assert.equal(run(makeRoot({ 'crates/x/src/a.rs': body })).code, 0);
});

test('fail-closed: crates/<name>・static・Cargo.toml のリンクも 2', () => {
  const r1 = makeRoot();
  fs.mkdirSync(path.join(r1, 'other'));
  fs.symlinkSync(path.join(r1, 'other'), path.join(r1, 'crates/linked'));
  assert.equal(run(r1).code, 2);
  const r2 = makeRoot();
  fs.mkdirSync(path.join(r2, 'other'));
  fs.symlinkSync(path.join(r2, 'other'), path.join(r2, 'crates/core/static'));
  assert.equal(run(r2).code, 2);
  const r3 = makeRoot();
  fs.writeFileSync(path.join(r3, 'real.toml'), '[package]\n');
  fs.symlinkSync(path.join(r3, 'real.toml'), path.join(r3, 'crates/core/Cargo.toml'));
  assert.equal(run(r3).code, 2);
});

test('複数行の文字列の違反は実際の行で報告する', () => {
  const cases = {
    'crates/x/src/a.rs': 'fn f() {\n  let _ = "a\nb\n  github";\n  let _ = "x \\\n  github";\n  let _ = r"p\nq\ngithub";\n}\n',
    'crates/x/static/a.js': 'const a = `p\nq\ngithub`;\nconst b = "x\\\n github";\n',
    'crates/x/Cargo.toml': '[package]\nd = """\na\ngithub"""\n',
  };
  const expect = { 'crates/x/src/a.rs': [4, 6, 9], 'crates/x/static/a.js': [3, 5], 'crates/x/Cargo.toml': [4] };
  for (const [rel, body] of Object.entries(cases)) {
    const r = run(makeRoot({ [rel]: body }));
    assert.equal(r.code, 1, r.out);
    const lines = [...r.out.matchAll(new RegExp(`${rel.replace(/\./g, '\\.')}:(\\d+): github`, 'g'))].map((m) => Number(m[1]));
    assert.deepEqual(lines, expect[rel], r.out);
  }
});

test('誤用は 2: 未知の引数・--root の値なし', () => {
  const root = makeRoot();
  assert.equal(run(root, ['--unknown']).code, 2);
  const r = spawnSync(process.execPath, [TOOL, '--root'], { encoding: 'utf8' });
  assert.equal(r.status, 2);
});

test('テンプレートリテラルの補間と入れ子の中の語を検査する', () => {
  const bodies = [
    'const a = `ok ${"//"} github`;\n',
    'const a = `x ${`y ${"github"}`} z`;\n',
    'const a = `x ${ {k: 1}.k + "github" }`;\n',
    'const a = `x ${f(`github`)}`;\n',
  ];
  for (const body of bodies) {
    const r = run(makeRoot({ 'crates/x/static/a.js': body }));
    assert.equal(r.code, 1, `${body}: ${r.out}`);
  }
  assert.equal(run(makeRoot({ 'crates/x/static/a.js': 'const a = `x ${f(1)} y`;\n' })).code, 0);
});

test('閉じない補間は 2', () => {
  assert.equal(run(makeRoot({ 'crates/x/static/a.js': 'const a = `x ${f(1`;\n' })).code, 2);
});

test('引用符の中のコメント記号をコメントとして扱わない（CSS・HTML・SVG）', () => {
  const cases = {
    'crates/x/static/a.css': 'a::before { content: "/* github */"; }\n',
    'crates/x/static/a.html': '<a title="<!-- github -->">x</a>\n',
    'crates/x/static/a.svg': "<svg><text data-x='<!-- github -->'>x</text></svg>\n",
  };
  for (const [rel, body] of Object.entries(cases)) {
    const r = run(makeRoot({ [rel]: body }));
    assert.equal(r.code, 1, `${rel}: ${r.out}`);
  }
  const ok = {
    'crates/x/static/a.css': '/* github */ a { color: red; }\n',
    'crates/x/static/a.html': '<!-- github --><a>x</a>\n',
  };
  for (const [rel, body] of Object.entries(ok)) assert.equal(run(makeRoot({ [rel]: body })).code, 0, rel);
});

test('エスケープを復号してから照合する（Rust・JS・TOML・CSS・HTML）', () => {
  const cases = {
    'crates/x/src/a.rs': 'const A: &str = "git\\x68ub";\n',
    'crates/x/src/b.rs': 'const A: &str = "git\\u{68}ub";\n',
    'crates/x/static/a.js': 'const a = "git\\x68ub";\n',
    'crates/x/static/b.js': 'const a = `git\\u0068ub`;\n',
    'crates/x/Cargo.toml': '[package]\nd = "git\\u0068ub"\n',
    'crates/x/static/a.css': 'a::before { content: "git\\68 ub"; }\n',
    'crates/x/static/a.html': '<a title="git&#104;ub">x</a>\n',
  };
  for (const [rel, body] of Object.entries(cases)) {
    const r = run(makeRoot({ [rel]: body }));
    assert.equal(r.code, 1, `${rel}: ${r.out}`);
  }
});

test('未対応のエスケープは検査不能として 2', () => {
  const cases = {
    'crates/x/src/a.rs': 'const A: &str = "a\\qb";\n',
    'crates/x/static/a.js': 'const a = "a\\1b";\n',
    'crates/x/Cargo.toml': '[package]\nd = "a\\qb"\n',
  };
  for (const [rel, body] of Object.entries(cases)) {
    assert.equal(run(makeRoot({ [rel]: body })).code, 2, rel);
  }
});

test('正規表現リテラルの `//` をコメントと取り違えず、後続の語を検出する', () => {
  const body = 'const re = /https?:\\/\\/\\//;\nconst re2 = /https?://///;\nconst source = "github";\n';
  const r = run(makeRoot({ 'crates/x/static/a.js': body }));
  assert.equal(r.code, 1, r.out);
  assert.match(r.out, /a\.js:3: github/);
  // 除算は正規表現にしない
  assert.equal(run(makeRoot({ 'crates/x/static/b.js': 'const q = (a + b) / 2 / 3;\n' })).code, 0);
});

test('閉じない正規表現リテラルは検査不能として 2', () => {
  assert.equal(run(makeRoot({ 'crates/x/static/a.js': 'const re = /abc;\nconst s = "github";\n' })).code, 2);
});

test('識別子の Unicode エスケープを復号して照合する', () => {
  for (const body of ['const git\\u0068ub = 1;\n', 'const git\\u{68}ub = 1;\n']) {
    const r = run(makeRoot({ 'crates/x/static/a.js': body }));
    assert.equal(r.code, 1, `${body}: ${r.out}`);
  }
  assert.equal(run(makeRoot({ 'crates/x/static/a.js': 'const a\\u00zz = 1;\n' })).code, 2);
});

test('セミコロンの無い HTML の数値文字参照も復号する', () => {
  for (const body of ['<a title="git&#104ub">x</a>\n', '<a title="git&#x68ub">x</a>\n']) {
    const r = run(makeRoot({ 'crates/x/static/a.html': body }));
    assert.equal(r.code, 1, `${body}: ${r.out}`);
  }
});

test('正規表現リテラルの文字を表すエスケープを復号して照合する。読めない形は 2', () => {
  for (const body of ['const re = /git\\u0068ub/;\n', 'const re = /git\\u{68}ub/;\n', 'const re = /git\\x68ub/;\n']) {
    const r = run(makeRoot({ 'crates/x/static/a.js': body }));
    assert.equal(r.code, 1, `${body}: ${r.out}`);
  }
  // 文字を表さないエスケープは語を作らない
  assert.equal(run(makeRoot({ 'crates/x/static/b.js': 'const re = /\\d+\\s\\w/;\n' })).code, 0);
  for (const body of ['const re = /\\u00zz/;\n', 'const re = /\\xg1/;\n', 'const re = /\\u{110000}/;\n']) {
    assert.equal(run(makeRoot({ 'crates/x/static/c.js': body })).code, 2, body);
  }
});

test('HTML の script の中身は JS として読み、文字列の `<!-- -->` を落とさない', () => {
  const body = '<p>x</p>\n<script>\nconst x = "<!-- github -->";\n</script>\n';
  const r = run(makeRoot({ 'crates/x/static/a.html': body }));
  assert.equal(r.code, 1, r.out);
  assert.match(r.out, /a\.html:3: github/);
  // script の外の HTML コメントは従来どおり除く。script の中の JS コメントも除く
  assert.equal(run(makeRoot({ 'crates/x/static/b.html': '<!-- github -->\n<script>// github\nconst a = 1;</script>\n' })).code, 0);
  // 閉じない JS の文字列は検査不能として 2
  assert.equal(run(makeRoot({ 'crates/x/static/c.html': '<script>const a = "x;</script>\n' })).code, 2);
});
