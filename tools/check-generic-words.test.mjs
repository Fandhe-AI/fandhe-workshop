// tools/check-generic-words.mjs（教材固有の語の検査。W-22）の回帰テスト（TASK-471 3/3）。
//
// 作業ツリーは変えない。一時ディレクトリへ小さな crates/ と docs/vocabulary/vocabulary.md を組み立てて検査を流す
// （実 repo に対しては読むだけ）。語を足した写しで失敗すること（負の対照）と、判定できない入力で 2 になること
// （fail-closed）を主張する。static（JS・HTML・SVG・CSS）は字句解析をせず、コメントを含む素の文字列と、エスケープを
// 全体で復号した文字列で照合する。字句の読み違いで語を見逃した過去の指摘の例は、どれも 1 になることを確かめる。
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
| GitHub | \`gh\` | 語の一致 |
| GitHub | \`pr\` | 語の一致 |
| GitHub | \`issue\` | 語の一致 |
| Claude Code | \`skill\` | 語の一致 |

- 注記
`;

// command_class.rs の例外（skill が 3 件）を満たす最小の写し。
const COMMAND_CLASS = 'enum K { Skill }\nfn f() { let _ = "skill"; let _ = "skill"; }\n';

// secret_scan.rs の例外（github が 5 件・gh が 2 件）を満たす最小の写し。
const SECRET_SCAN = 'const A: &str = "github";\n'.repeat(5) + 'const B: &str = "gh";\n'.repeat(2);

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
  put('crates/core/src/secret_scan.rs', SECRET_SCAN);
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

test('static の js・html・svg・css と Cargo.toml も対象。static はコメントの中の語も 1、Cargo.toml のコメントは 0', () => {
  const cases = {
    'crates/x/static/a.js': ["const a = 'github';\n", 'const b = 1;\n', ['// github\n', '/* issue */ const b = 1;\n']],
    'crates/x/static/a.mjs': ["const a = 'github';\n", 'const b = 1;\n', ['// github\n']],
    'crates/x/static/a.html': ['<p id="pr">x</p>\n', '<p>x</p>\n', ['<!-- github -->\n<p>x</p>\n']],
    'crates/x/static/a.svg': ['<g id="bbs"/>\n', '<g/>\n', ['<!-- bbs -->\n<g/>\n']],
    'crates/x/static/a.css': ['.thread { color: red; }\n', '.a { color: red; }\n', ['/* thread */ .a { color: red; }\n']],
  };
  for (const [rel, [bad, good, comments]] of Object.entries(cases)) {
    assert.equal(run(makeRoot({ [rel]: bad })).code, 1, rel);
    assert.equal(run(makeRoot({ [rel]: good })).code, 0, rel);
    for (const body of comments) assert.equal(run(makeRoot({ [rel]: body })).code, 1, `${rel}: ${body}`);
  }
  assert.equal(run(makeRoot({ 'crates/x/Cargo.toml': '[package]\nname = "github-x"\n' })).code, 1);
  assert.equal(run(makeRoot({ 'crates/x/Cargo.toml': '# github\n[package]\nname = "x"\n' })).code, 0);
});

test('static の違反は同じ行・語を読みの数だけ重ねず、行ごとの件数で報告する', () => {
  const r = run(makeRoot({ 'crates/x/static/a.js': "const a = 'github';\nconst b = 'github github';\n" }));
  assert.equal(r.code, 1, r.out);
  const lines = [...r.out.matchAll(/a\.js:(\d+): github/g)].map((m) => Number(m[1]));
  assert.deepEqual(lines, [1, 2, 2], r.out);
});

test('例外の件数: command_class.rs の skill を 1 増やしても 1 減らしても 1', () => {
  assert.equal(run(makeRoot({}, { commandClass: `${COMMAND_CLASS}const A: &str = "skill";\n` })).code, 1);
  assert.equal(run(makeRoot({}, { commandClass: 'enum K { Skill }\nfn f() { let _ = "skill"; }\n' })).code, 1);
  // 例外は command_class.rs の skill だけ。別のファイルの skill は違反。
  assert.equal(run(makeRoot({ 'crates/x/src/a.rs': 'enum K { Skill }\n' })).code, 1);
});

test('例外の件数: secret_scan.rs の github と gh は増減で 1、別のファイルの github は違反', () => {
  const rs = (n, w) => `const A: &str = "${w}";\n`.repeat(n);
  const root = (github, gh) => makeRoot({ 'crates/core/src/secret_scan.rs': rs(github, 'github') + rs(gh, 'gh') });
  assert.equal(run(root(5, 2)).code, 0);
  assert.equal(run(root(6, 2)).code, 1);
  assert.equal(run(root(4, 2)).code, 1);
  assert.equal(run(root(5, 3)).code, 1);
  assert.equal(run(root(5, 1)).code, 1);
  assert.equal(run(makeRoot({ 'crates/x/src/a.rs': 'const A: &str = "github";\n' })).code, 1);
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

test('fail-closed: .rs の閉じていない文字列・ブロックコメント、crates 無しは 2', () => {
  assert.equal(run(makeRoot({ 'crates/x/src/a.rs': 'const A: &str = "abc;\n' })).code, 2);
  assert.equal(run(makeRoot({ 'crates/x/src/a.rs': '/* never closed\n' })).code, 2);
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

test('static は字句が閉じなくても 2 にせず、素の文字列で照合する（語が無ければ 0、あれば 1）', () => {
  for (const body of ["const a = 'abc;\n", 'const a = `x ${f(1`;\n', 'const re = /abc;\n']) {
    assert.equal(run(makeRoot({ 'crates/x/static/a.js': body })).code, 0, body);
    assert.equal(run(makeRoot({ 'crates/x/static/b.js': `${body}github\n` })).code, 1, body);
  }
  assert.equal(run(makeRoot({ 'crates/x/static/a.html': '<!-- never closed\n' })).code, 0);
  assert.equal(run(makeRoot({ 'crates/x/static/b.html': '<!-- never closed github\n' })).code, 1);
  assert.equal(run(makeRoot({ 'crates/x/static/a.css': '/* never closed\n' })).code, 0);
});

test('引用符の中のコメント記号の後の語も照合する（CSS・HTML・SVG）', () => {
  const cases = {
    'crates/x/static/a.css': 'a::before { content: "/* github */"; }\n',
    'crates/x/static/a.html': '<a title="<!-- github -->">x</a>\n',
    'crates/x/static/a.svg': "<svg><text data-x='<!-- github -->'>x</text></svg>\n",
  };
  for (const [rel, body] of Object.entries(cases)) {
    const r = run(makeRoot({ [rel]: body }));
    assert.equal(r.code, 1, `${rel}: ${r.out}`);
  }
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

test('.rs・Cargo.toml の未対応のエスケープは検査不能として 2', () => {
  const cases = {
    'crates/x/src/a.rs': 'const A: &str = "a\\qb";\n',
    'crates/x/Cargo.toml': '[package]\nd = "a\\qb"\n',
  };
  for (const [rel, body] of Object.entries(cases)) {
    assert.equal(run(makeRoot({ [rel]: body })).code, 2, rel);
  }
});

test('正規表現の中の `//` の後の語も検出する（`/` の意味を判定しない）', () => {
  const body = 'const re = /https?:\\/\\/\\//;\nconst re2 = /https?://///;\nconst source = "github";\n';
  const r = run(makeRoot({ 'crates/x/static/a.js': body }));
  assert.equal(r.code, 1, r.out);
  assert.match(r.out, /a\.js:3: github/);
  // 除算の `/` だけなら語は無く 0
  assert.equal(run(makeRoot({ 'crates/x/static/b.js': 'const q = (a + b) / 2 / 3;\n' })).code, 0);
});

test('閉じない正規表現リテラルの後の語も照合する', () => {
  const r = run(makeRoot({ 'crates/x/static/a.js': 'const re = /abc;\nconst s = "github";\n' }));
  assert.equal(r.code, 1, r.out);
  assert.match(r.out, /a\.js:2: github/);
});

test('識別子の Unicode エスケープを復号して照合する', () => {
  for (const body of ['const git\\u0068ub = 1;\n', 'const git\\u{68}ub = 1;\n']) {
    const r = run(makeRoot({ 'crates/x/static/a.js': body }));
    assert.equal(r.code, 1, `${body}: ${r.out}`);
  }
  // 読めないエスケープは文字のまま残し、2 にしない
  assert.equal(run(makeRoot({ 'crates/x/static/a.js': 'const a\\u00zz = 1;\n' })).code, 0);
});

test('セミコロンの無い HTML の数値文字参照も復号する', () => {
  for (const body of ['<a title="git&#104ub">x</a>\n', '<a title="git&#x68ub">x</a>\n']) {
    const r = run(makeRoot({ 'crates/x/static/a.html': body }));
    assert.equal(r.code, 1, `${body}: ${r.out}`);
  }
});

test('正規表現リテラルの文字を表すエスケープを復号して照合する。読めない形は 2 にしない', () => {
  for (const body of ['const re = /git\\u0068ub/;\n', 'const re = /git\\u{68}ub/;\n', 'const re = /git\\x68ub/;\n']) {
    const r = run(makeRoot({ 'crates/x/static/a.js': body }));
    assert.equal(r.code, 1, `${body}: ${r.out}`);
  }
  // 文字を表さないエスケープは語を作らない
  assert.equal(run(makeRoot({ 'crates/x/static/b.js': 'const re = /\\d+\\s\\w/;\n' })).code, 0);
  for (const body of ['const re = /\\u00zz/;\n', 'const re = /\\xg1/;\n', 'const re = /\\u{110000}/;\n']) {
    assert.equal(run(makeRoot({ 'crates/x/static/c.js': body })).code, 0, body);
  }
});

test('HTML の script の中の `<!-- -->` の語を落とさない', () => {
  const body = '<p>x</p>\n<script>\nconst x = "<!-- github -->";\n</script>\n';
  const r = run(makeRoot({ 'crates/x/static/a.html': body }));
  assert.equal(r.code, 1, r.out);
  assert.match(r.out, /a\.html:3: github/);
  // script の外の HTML コメント・script の中の JS コメントの語も照合する
  assert.equal(run(makeRoot({ 'crates/x/static/b.html': '<!-- github -->\n<p>x</p>\n' })).code, 1);
  assert.equal(run(makeRoot({ 'crates/x/static/b2.html': '<script>// github\nconst a = 1;</script>\n' })).code, 1);
  // 閉じない JS の文字列も 2 にしない
  assert.equal(run(makeRoot({ 'crates/x/static/c.html': '<script>const a = "x;</script>\n' })).code, 0);
});

test('`++`・`--` の前後の `/` は、語が無ければ 0、あれば 1', () => {
  for (const body of ['let n = x++ / 2;\n', 'let n = x-- / 2 / 3;\n', 'let m = a++ /b/ c;\n']) {
    assert.equal(run(makeRoot({ 'crates/x/static/a.js': body })).code, 0, body);
  }
  // 前置の ++ の後の正規表現の語は照合する
  assert.equal(run(makeRoot({ 'crates/x/static/b.js': 'let n = 1; ++n; let r = /github/;\n' })).code, 1);
});

test('正規表現リテラルの identity escape は文字そのものとして照合する', () => {
  // Unicode フラグなしでは /git\hub/ の `\h` は h を表すため、github に一致する
  const r = run(makeRoot({ 'crates/x/static/a.js': 'const re = /git\\hub/;\n' }));
  assert.equal(r.code, 1, r.out);
  // u フラグで意味が変わる \p・\k も 2 にしない（文字のまま残す）
  assert.equal(run(makeRoot({ 'crates/x/static/c.js': 'const re = /\\p{L}/u;\n' })).code, 0);
});

test('`}` の後の `/` は、語が無ければ 0、あれば 1', () => {
  for (const body of ['const n = {} / 2;\n', 'const n = ({}) / 2;\n', 'const o = { a: {} / 2 };\n']) {
    assert.equal(run(makeRoot({ 'crates/x/static/a.js': body })).code, 0, body);
  }
  // 文ブロックの後の正規表現の語は照合する
  assert.equal(run(makeRoot({ 'crates/x/static/b.js': 'if (a) { b(); }\n/github/.test(c);\n' })).code, 1);
  assert.equal(run(makeRoot({ 'crates/x/static/c.js': 'function f() {}\n/github/.test(c);\n' })).code, 1);
});

test('HTML の style 要素と style 属性の CSS もエスケープを復号して照合する', () => {
  const cases = [
    '<style>a::after { content: "git\\68 ub"; }</style>\n',
    '<p style="content: \'git\\68 ub\'">x</p>\n',
    '<p style=\'content: "git\\68 ub"\'>x</p>\n',
  ];
  for (const body of cases) {
    const r = run(makeRoot({ 'crates/x/static/a.html': body }));
    assert.equal(r.code, 1, `${body}: ${r.out}`);
  }
  // 語を含まない style は通る。CSS のコメントの中の語は照合する
  assert.equal(run(makeRoot({ 'crates/x/static/b.html': '<style>a { color: red; }</style><p style="color:red">x</p>\n' })).code, 0);
  assert.equal(run(makeRoot({ 'crates/x/static/c.html': '<style>/* github */ a { color: red; }</style>\n' })).code, 1);
});

test('TOML の複数行文字列の終端の引用符が 4 個・5 個でも閉じる', () => {
  for (const q of ['"', "'"]) {
    for (const extra of ['', q, q + q]) {
      const body = `[package]\na = ${q.repeat(3)}value${extra}${q.repeat(3)}\nb = ${q.repeat(3)}github${extra}${q.repeat(3)}\n`;
      const r = run(makeRoot({ 'crates/x/Cargo.toml': body }));
      assert.equal(r.code, 1, `${body}: ${r.out}`);
      assert.match(r.out, /Cargo\.toml:3: github/);
    }
  }
  assert.equal(run(makeRoot({ 'crates/x/Cargo.toml': 'a = """value""""\nb = 1\n' })).code, 0);
});

test('制御文の括弧の閉じの後の正規表現も、中の `//` の後の語を照合する', () => {
  // Codex の指摘の例: 字句解析で if の括弧の後の正規表現を除算と誤認すると、`//` 以降の github を読み飛ばしていた
  const r = run(makeRoot({ 'crates/x/static/a.js': 'if (ok) /[//]github/.test(x);\n' }));
  assert.equal(r.code, 1, r.out);
  assert.match(r.out, /a\.js:1: github/);
  for (const body of [
    'while (ok) /[//]github/.test(x);\n',
    'for (;;) /[//]github/.test(x);\n',
    'with (o) /[//]github/.test(x);\n',
    'if (f(a)) /[//]github/.test(x);\n',
    'do x(); while (ok) /[//]github/.test(x);\n',
  ]) {
    const r2 = run(makeRoot({ 'crates/x/static/b.js': body }));
    assert.equal(r2.code, 1, `${body}: ${r2.out}`);
  }
  // 制御文でない括弧・プロパティ名の for の後の除算は、語が無ければ 0
  for (const body of ['const q = f(a) / 2 / 3;\n', 'const q = x.for(a) / 2 / 3;\n', 'const q = x.if / 2 / 3;\n']) {
    assert.equal(run(makeRoot({ 'crates/x/static/c.js': body })).code, 0, body);
  }
  // 制御文の括弧の後の文ブロックの後の正規表現の語も照合する
  assert.equal(run(makeRoot({ 'crates/x/static/d.js': 'if (a) { b(); } /github/.test(c);\n' })).code, 1);
});

test('HTML の数値文字参照は先頭の 0 を含む数字列全体を読んで復号する', () => {
  const cases = [
    '<p title="git&#000000000104;ub">x</p>\n',
    '<p title="git&#x0000000068;ub">x</p>\n',
    '<p>git&#00000000000000000000104ub</p>\n',
    '<p style="content: \'git&#000000000104;ub\'">x</p>\n',
  ];
  for (const body of cases) {
    const r = run(makeRoot({ 'crates/x/static/a.html': body }));
    assert.equal(r.code, 1, `${body}: ${r.out}`);
  }
});

test('範囲外・C1 の HTML の数値文字参照は置換文字として読み、2 にしない', () => {
  for (const ref of ['&#0;', '&#1114112;', '&#00000000001114112;', '&#x110000;', '&#xD800;', '&#128;', '&#x9F;']) {
    for (const body of [`<p>a${ref}b</p>\n`, `<p style="content: 'a${ref}b'">x</p>\n`, `const s = "a${ref}b";\n`]) {
      assert.equal(run(makeRoot({ 'crates/x/static/a.html': body })).code, 0, body);
    }
    // 置換文字は語をつながず、前後の語はそれぞれ照合する
    assert.equal(run(makeRoot({ 'crates/x/static/b.html': `<p>open${ref}pr</p>\n` })).code, 1, ref);
  }
  // 数字の無い `&#`・`&#x` は参照ではなく文字のまま
  assert.equal(run(makeRoot({ 'crates/x/static/b.html': '<p>&# &#x; &#q</p>\n' })).code, 0);
});

test('HTML の名前つき文字参照: `;` 無しの旧来の名前の後の語と、英数字や `_` になる参照を照合する', () => {
  // `&ltpr` は本文で `<pr` と表示される（`;` 無しで復号される旧来の名前 lt の後に pr が続く）
  for (const body of ['<p>&ltpr x</p>\n', '<p>&amppr</p>\n', '<p title="&copyPrIssue">x</p>\n']) {
    const r = run(makeRoot({ 'crates/x/static/a.html': body }));
    assert.equal(r.code, 1, `${body}: ${r.out}`);
  }
  // 正当な名前つき参照だけなら語を作らない
  assert.equal(run(makeRoot({ 'crates/x/static/b.html': '<p>&ltimes; &lt; &amp; &nbsp; &copy;</p>\n' })).code, 0);
  // 英数字・`_` に復号される参照（fjlig は fj、lowbar・UnderBar は `_`）は復号してから照合する
  const appendix = APPENDIX.replace('- 注記', '').trimEnd() + '\n| 試験 | `fjord` | 部分一致 |\n| 試験 | `git_hub` | 部分一致 |\n\n- 注記\n';
  for (const body of ['<p>&fjlig;ord</p>\n', '<p>git&lowbar;hub</p>\n', '<p title="git&UnderBar;hub">x</p>\n']) {
    const r = run(makeRoot({ 'crates/x/static/c.html': body }, { appendix }));
    assert.equal(r.code, 1, `${body}: ${r.out}`);
  }
});

test('JS の `\\u{...}` は先頭の 0 が多くても復号して照合する', () => {
  for (const body of [
    'const s = "git\\u{0000000068}ub";\n',
    'const git\\u{0000000068}ub = 1;\n',
    'const re = /git\\u{0000000068}ub/;\n',
  ]) {
    const r = run(makeRoot({ 'crates/x/static/a.js': body }));
    assert.equal(r.code, 1, `${body}: ${r.out}`);
  }
  // 範囲外は 2 にしない
  assert.equal(run(makeRoot({ 'crates/x/static/b.js': 'const s = "\\u{00000110000}";\n' })).code, 0);
});

test('style 属性の `=` の前後に長い空白があっても CSS として復号して照合する', () => {
  const pad = ' '.repeat(20);
  for (const body of [
    `<p style\n${pad}=\n${pad}"content: 'git\\68 ub'">x</p>\n`,
    `<p STYLE${pad}=${pad}'content: "git\\68 ub"'>x</p>\n`,
  ]) {
    const r = run(makeRoot({ 'crates/x/static/a.html': body }));
    assert.equal(r.code, 1, `${body}: ${r.out}`);
  }
  // 名前が style でない属性の値の中の CSS のコメントの形の語も照合する
  const r = run(makeRoot({ 'crates/x/static/b.html': '<p data-style="/* github */">x</p>\n' }));
  assert.equal(r.code, 1, r.out);
});

test('Codex の指摘の例（字句の読み違いで語を見逃した書き方）はどれも 1', () => {
  const cases = [
    // 最新の 2 件: for await の await が制御文の情報を上書きした例と、引用符の無い style 属性の CSS エスケープ
    ['a.js', 'async function f() { for await (const x of xs) /[//]github/.test(x); }\n'],
    ['a.html', '<p style=font-family:git\\68ub>x</p>\n'],
    // `)`・`}`・`++` の後の `/` の判定
    ['b.js', 'if (ok) /[//]github/.test(x);\n'],
    ['c.js', 'if (a) { b(); }\n/[//]github/.test(c);\n'],
    ['d.js', 'let n = 1;\n++n\n/[//]github/.test(s);\n'],
    // <script> の中の <!-- -->
    ['b.html', '<script>\n<!-- github -->\n</script>\n'],
    ['c.html', '<script>const x = "<!-- github -->";</script>\n'],
    // `;` の無い数値文字参照・先頭の 0 の多い参照
    ['d.html', '<p>git&#104ub</p>\n'],
    ['e.html', '<p>git&#x68ub</p>\n'],
    ['f.html', '<p>git&#00000000000000000000104;ub</p>\n'],
    ['g.html', '<p title="git&#x0000000068;ub">x</p>\n'],
    // <style> 要素の CSS エスケープ
    ['h.html', '<style>a::after { content: "git\\68 ub"; }</style>\n'],
    ['a.css', 'a::after { content: "git\\68 ub"; }\n'],
  ];
  for (const [name, body] of cases) {
    const r = run(makeRoot({ [`crates/x/static/${name}`]: body }));
    assert.equal(r.code, 1, `${body}: ${r.out}`);
    assert.match(r.out, /: github/, r.out);
  }
});

test('復号の組み合わせ: HTML→CSS・JS→HTML・行の継続・8 進のエスケープも照合する', () => {
  const cases = [
    // HTML の参照で書いた `\` の後の CSS エスケープ
    ['a.html', '<p style="content: \'git&#92;68 ub\'">x</p>\n'],
    // JS の文字列で書いた HTML の参照（innerHTML に入れると h になる）
    ['a.js', 'el.innerHTML = "git\\x26#104;ub";\n'],
    // JS の行の継続
    ['b.js', 'const a = "git\\\nhub";\n'],
    // 旧来の 8 進のエスケープ（\150 は h）と identity escape
    ['c.js', 'const a = "git\\150ub";\n'],
    ['d.js', 'const a = "git\\hub";\n'],
    // CSS の 1 文字のエスケープ
    ['a.css', '.git\\hub { color: red; }\n'],
  ];
  for (const [name, body] of cases) {
    const r = run(makeRoot({ [`crates/x/static/${name}`]: body }));
    assert.equal(r.code, 1, `${body}: ${r.out}`);
    assert.match(r.out, /:1: github/, r.out);
  }
});

test('正当な JS・HTML・CSS は、エスケープや参照が読めない形でも 2 にしない', () => {
  const cases = {
    'crates/x/static/a.js': '// \\u00zz \\u{110000} \\x \\8\nconst re = /(?<n>a)\\k<n>\\p{L}/u;\nconst re2 = /\\u{110000}/;\nconst s = "a\\0b";\n',
    'crates/x/static/a.html': '<p>&# &#x; &#q &unknownname; &#0; &#xD800;</p>\n',
    'crates/x/static/a.css': 'a::after { content: "\\0 \\110000 \\D800 \\"; }\n',
  };
  for (const [rel, body] of Object.entries(cases)) assert.equal(run(makeRoot({ [rel]: body })).code, 0, rel);
});

test('fail-closed: UTF-8 でない static（BOM つきの UTF-16・不正なバイト列）は 2。BOM つきの UTF-8 と NUL を含む JS は読む', () => {
  const root1 = makeRoot();
  fs.mkdirSync(path.join(root1, 'crates/x/static'), { recursive: true });
  fs.writeFileSync(path.join(root1, 'crates/x/static/a.js'), Buffer.from('\ufeffconst a = "github";\n', 'utf16le'));
  assert.equal(run(root1).code, 2);
  const root2 = makeRoot();
  fs.mkdirSync(path.join(root2, 'crates/x/static'), { recursive: true });
  fs.writeFileSync(path.join(root2, 'crates/x/static/a.css'), Buffer.from([0x61, 0xff, 0x62, 0x0a]));
  assert.equal(run(root2).code, 2);
  const bom = run(makeRoot({ 'crates/x/static/a.js': '\ufeffconst a = "github";\n' }));
  assert.equal(bom.code, 1, bom.out);
  assert.match(bom.out, /a\.js:1: github/);
  assert.equal(run(makeRoot({ 'crates/x/static/b.js': 'const a = "x\0y";\n' })).code, 0);
});

test('.rs のコメントの中の語は今までどおり 0（static だけがコメントも照合する）', () => {
  const body = '// github\n/* issue */\nfn f() {}\n';
  assert.equal(run(makeRoot({ 'crates/x/src/a.rs': body })).code, 0);
  assert.equal(run(makeRoot({ 'crates/x/static/a.js': body })).code, 1);
});
