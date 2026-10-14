//! 教材パックのディレクトリを読み込む層（構造定義・本文・練習データの確認）。
//!
//! `compile`（`crate::compile`）の前段で、ディスクに触れるのはここだけである。教材データは信頼しない
//! 入力として扱い、(1) 中身を読む前に上限・シンボリックリンク・入れ子の深さを確かめ、(2) 本文と練習データの
//! パスは安全な相対パス（`..`・絶対パス・`\` を含まない）に限り、(3) 読むときは上限 + 1 バイトで打ち切る。
//! 診断のメッセージにはパスと ID だけを入れ、本文・練習データの中身は入れない。
//!
//! 上限の値（`MAX_*`）は PoC-7 の仮の値で、公開しない。値と拒否理由の契約は TASK-199 で固定する。
//! 形式 B の厳格化（文字列への型変換・重複キー・行と桁）は TASK-40、モジュールの展開は TASK-50 が扱う。

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use fandhe_workshop_contract::ir::{FixtureDef, PackInfo, Start, Step};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use crate::diag::{DiagCode, Diagnostic};

/// Markdown 1 ファイルの上限（バイト）。仮の値（TASK-199 で固定）。
const MAX_MARKDOWN_BYTES: u64 = 512 * 1024;
/// パック全体の上限（バイト）。仮の値（TASK-199 で固定）。
const MAX_PACK_BYTES: u64 = 20 * 1024 * 1024;
/// ディレクトリの入れ子の上限。仮の値（TASK-199 で固定）。
const MAX_DEPTH: usize = 32;

/// 人が書く形式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// 形式 A: Markdown と TOML。
    Toml,
    /// 形式 B: Markdown と YAML。
    Yaml,
}

impl Format {
    /// 定義ファイルの拡張子。
    pub const fn ext(self) -> &'static str {
        match self {
            Format::Toml => "toml",
            Format::Yaml => "yaml",
        }
    }

    /// 構文・型を確かめて読む。失敗は位置だけのメッセージにする。パーサーのエラー文字列には、ソースの行や
    /// 型が合わない入力値が入るので、そのまま診断へ渡さない。
    fn parse<T: DeserializeOwned>(self, text: &str) -> Result<T, String> {
        match self {
            Format::Toml => toml::from_str(text).map_err(|e| {
                let pos = e.span().map(|r| line_col(text, r.start));
                parse_failure(pos)
            }),
            Format::Yaml => yaml_serde::from_str(text).map_err(|e| {
                let pos = e.location().map(|l| (l.line(), l.column()));
                parse_failure(pos)
            }),
        }
    }
}

/// `workshop.*` の内容。構造に関わる欄だけ型を付け、語彙の中身は通す。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkshopSrc {
    /// 教材の基本情報。
    pub pack: PackInfo,
    /// 参照元の版。
    #[serde(default)]
    pub refs: Vec<Value>,
    /// コマンドの表。
    #[serde(default)]
    pub commands: BTreeMap<String, Value>,
    /// ビューの表。
    #[serde(default)]
    pub views: BTreeMap<String, Value>,
    /// 練習データの宣言。
    #[serde(default)]
    pub fixtures: BTreeMap<String, FixtureDef>,
    /// ヒントの表。
    #[serde(default)]
    pub hints: BTreeMap<String, Value>,
    /// 拒否の文面の表。
    #[serde(default)]
    pub rejections: BTreeMap<String, Value>,
    /// 問い返しの文面の表。
    #[serde(default)]
    pub clarifications: BTreeMap<String, Value>,
    /// 拒否の分類ごとの既定の文面の ID。
    #[serde(default)]
    pub defaults: BTreeMap<String, String>,
}

/// 章またはモジュールの定義ファイルの内容。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeSrc {
    /// 章・モジュールの ID。
    pub id: String,
    /// 題名。
    pub title: String,
    /// モジュールの引数と既定値。
    #[serde(default)]
    pub params: BTreeMap<String, String>,
    /// 途中開始。
    #[serde(default)]
    pub starts: Vec<Start>,
    /// Step（ファイルの中の順）。
    pub steps: Vec<Step>,
}

/// 教材テスト（`tests/playthrough.*`）。IR には載せない。中身の解釈は TASK-404 以降が行う。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestsSrc {
    /// 模範解答の通し試験。
    #[serde(default)]
    pub playthroughs: Vec<Value>,
    /// 誤入力の分類。
    #[serde(default)]
    pub error_inputs: Vec<Value>,
    /// 文面の確認。
    #[serde(default)]
    pub text_checks: Vec<Value>,
    /// 案内の確認。
    #[serde(default)]
    pub guidance_checks: Vec<Value>,
}

/// 章かモジュールか。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// `chapters/<名前>/chapter.*`。
    Chapter,
    /// `modules/<名前>/module.*`。
    Module,
}

/// 読み込んだ章・モジュール。
#[derive(Debug, Clone)]
pub struct LoadedScope {
    /// 章かモジュールか。
    pub kind: ScopeKind,
    /// 定義ファイルのパック根からの相対パス（`/` 区切り）。
    pub file: String,
    /// 定義ファイルのあるディレクトリのパック根からの相対パス（本文の参照の基準）。
    pub dir: String,
    /// 定義の内容。
    pub src: ScopeSrc,
}

/// 読み込んだ教材パック。`compile` が要るものをすべてメモリに載せる。
#[derive(Debug)]
pub struct LoadedPack {
    /// 人が書く形式。
    pub format: Format,
    /// `workshop.*` のパック根からの相対パス。
    pub workshop_file: String,
    /// `workshop.*` の内容。
    pub workshop: WorkshopSrc,
    /// 章（名前の昇順）、続いてモジュール（名前の昇順）。
    pub scopes: Vec<LoadedScope>,
    /// 本文。キーはパック根からの相対パス。
    pub bodies: BTreeMap<String, String>,
    /// 教材テストのパック根からの相対パス。
    pub tests_file: Option<String>,
    /// 教材テストの内容。
    pub tests: Option<TestsSrc>,
}

/// バイト位置を 1 始まりの行・桁（文字数）にする。範囲外は末尾として扱う。
fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let mut end = offset.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let before = &text[..end];
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, col)
}

/// 入力値を含まない構文エラーのメッセージ。位置が分かれば行と桁を添える。
fn parse_failure(pos: Option<(usize, usize)>) -> String {
    match pos {
        Some((line, col)) => format!("invalid syntax or structure at line {line} column {col}"),
        None => "invalid syntax or structure".to_string(),
    }
}

/// 診断を作る。`file` が空のときは `.`（パックの根）に直す。直した後は必ず作れる。
fn diag(file: &str, item: &str, code: DiagCode, message: impl Into<String>) -> Diagnostic {
    let file = if file.trim().is_empty() { "." } else { file };
    let item = if item.trim().is_empty() { "file" } else { item };
    // file と item を空でない値に直したので、Diagnostic::new は失敗しない。
    Diagnostic::new(file, item, code, message).expect("file and item are non-empty")
}

/// パック根からの相対パスを `/` 区切りで書く。根自身は `.`。
fn rel(root: &Path, p: &Path) -> String {
    let parts: Vec<String> = p
        .strip_prefix(root)
        .unwrap_or(p)
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

/// 教材データの参照パスとして安全か。空・絶対パス・`\` を含む・`..` や `.` を含むものは拒否する。
fn is_safe_rel(p: &str) -> bool {
    if p.is_empty() || p.contains('\\') || p.contains('\0') {
        return false;
    }
    Path::new(p)
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
}

/// 読み込みの失敗。
enum ReadError {
    Io,
    TooLarge,
    NotUtf8,
}

/// 上限 + 1 バイトで打ち切って読む。確認の後にファイルが入れ替えられても、上限を超えて読まない。
fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, ReadError> {
    let f = File::open(path).map_err(|_| ReadError::Io)?;
    let mut buf = Vec::new();
    f.take(limit + 1)
        .read_to_end(&mut buf)
        .map_err(|_| ReadError::Io)?;
    if buf.len() as u64 > limit {
        return Err(ReadError::TooLarge);
    }
    Ok(buf)
}

fn read_text(path: &Path, limit: u64) -> Result<String, ReadError> {
    String::from_utf8(read_limited(path, limit)?).map_err(|_| ReadError::NotUtf8)
}

/// 通常のファイルで、シンボリックリンクでなく、実体がパック根の下にあることを確かめる。
fn is_plain_file_under(root_canon: &Path, path: &Path) -> bool {
    let Ok(md) = fs::symlink_metadata(path) else {
        return false;
    };
    if md.file_type().is_symlink() || !md.is_file() {
        return false;
    }
    match path.canonicalize() {
        Ok(c) => c.starts_with(root_canon),
        Err(_) => false,
    }
}

/// ディレクトリの走査に失敗したことの診断。握りつぶすと、教材の一部が欠けた IR が成功として返る。
fn scan_failure(file: &str) -> Diagnostic {
    diag(
        file,
        "file",
        DiagCode::Structure,
        "cannot scan directory or read entry metadata",
    )
}

/// 中身を読む前の走査。シンボリックリンク・入れ子の深さ・Markdown の大きさ・パック全体の大きさを数える。
/// リンクは辿らない。診断は相対パスの順に並べる。
fn check_limits(root: &Path) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut total: u64 = 0;
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((d, depth)) = stack.pop() {
        let rd = match fs::read_dir(&d) {
            Ok(rd) => rd,
            Err(_) => {
                diags.push(scan_failure(&rel(root, &d)));
                continue;
            }
        };
        for e in rd {
            let e = match e {
                Ok(e) => e,
                Err(_) => {
                    diags.push(scan_failure(&rel(root, &d)));
                    continue;
                }
            };
            let p = e.path();
            let file = rel(root, &p);
            let md = match fs::symlink_metadata(&p) {
                Ok(md) => md,
                Err(_) => {
                    diags.push(scan_failure(&file));
                    continue;
                }
            };
            if md.file_type().is_symlink() {
                diags.push(diag(
                    &file,
                    "file",
                    DiagCode::LimitSymlink,
                    "symbolic links are not allowed",
                ));
            } else if md.is_dir() {
                if depth < MAX_DEPTH {
                    stack.push((p, depth + 1));
                } else {
                    diags.push(diag(
                        &file,
                        "file",
                        DiagCode::LimitDepth,
                        format!("directory nesting exceeds {MAX_DEPTH} levels"),
                    ));
                }
            } else if md.is_file() {
                total = total.saturating_add(md.len());
                if p.extension().is_some_and(|x| x == "md") && md.len() > MAX_MARKDOWN_BYTES {
                    diags.push(diag(
                        &file,
                        "file",
                        DiagCode::LimitMdSize,
                        format!(
                            "markdown file exceeds {MAX_MARKDOWN_BYTES} bytes ({} bytes)",
                            md.len()
                        ),
                    ));
                }
            }
        }
    }
    if total > MAX_PACK_BYTES {
        diags.push(diag(
            ".",
            "pack",
            DiagCode::LimitPackSize,
            format!("pack exceeds {MAX_PACK_BYTES} bytes ({total} bytes)"),
        ));
    }
    diags.sort_by(|a, b| (a.file(), a.code().as_str()).cmp(&(b.file(), b.code().as_str())));
    diags
}

/// 構造定義を読んで構文を確かめる。失敗は `E-PARSE` として `diags` に積む。
fn read_parse<T: DeserializeOwned>(
    format: Format,
    path: &Path,
    file: &str,
    item: &str,
    diags: &mut Vec<Diagnostic>,
) -> Option<T> {
    let text = match read_text(path, MAX_PACK_BYTES) {
        Ok(t) => t,
        Err(ReadError::NotUtf8) => {
            diags.push(diag(file, item, DiagCode::Parse, "file is not valid UTF-8"));
            return None;
        }
        Err(ReadError::TooLarge) => {
            diags.push(diag(file, item, DiagCode::Parse, "file is too large"));
            return None;
        }
        Err(ReadError::Io) => {
            diags.push(diag(file, item, DiagCode::Parse, "cannot read file"));
            return None;
        }
    };
    match format.parse::<T>(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            diags.push(diag(file, item, DiagCode::Parse, e));
            None
        }
    }
}

/// 直下のディレクトリを名前の昇順で返す。ディレクトリが無いこと（省略可能）は空とする。それ以外の
/// I/O エラーは、走査できなかった相対パスを診断にして `diags` へ積む。
fn sorted_subdirs(root: &Path, p: &Path, diags: &mut Vec<Diagnostic>) -> Vec<PathBuf> {
    let rd = match fs::read_dir(p) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(_) => {
            diags.push(scan_failure(&rel(root, p)));
            return Vec::new();
        }
    };
    let mut v: Vec<PathBuf> = Vec::new();
    for e in rd {
        let Ok(e) = e else {
            diags.push(scan_failure(&rel(root, p)));
            continue;
        };
        let path = e.path();
        match fs::symlink_metadata(&path) {
            Ok(m) if m.is_dir() => v.push(path),
            Ok(_) => {}
            Err(_) => diags.push(scan_failure(&rel(root, &path))),
        }
    }
    v.sort();
    v
}

/// 形式を決める。両方ある・どちらも無い場合は `E-PARSE`。
fn detect_format(root: &Path) -> Result<(Format, PathBuf), Vec<Diagnostic>> {
    let toml_p = root.join("workshop.toml");
    let yaml_p = root.join("workshop.yaml");
    let exists = |p: &Path| fs::symlink_metadata(p).is_ok_and(|m| m.is_file());
    match (exists(&toml_p), exists(&yaml_p)) {
        (true, false) => Ok((Format::Toml, toml_p)),
        (false, true) => Ok((Format::Yaml, yaml_p)),
        (true, true) => Err(vec![diag(
            "workshop.toml",
            "pack",
            DiagCode::Parse,
            "both workshop.toml and workshop.yaml exist; use one format",
        )]),
        (false, false) => Err(vec![diag(
            "workshop.toml",
            "pack",
            DiagCode::Parse,
            "neither workshop.toml nor workshop.yaml exists",
        )]),
    }
}

/// 1 つの本文を読み、`bodies` に入れる。失敗は診断にする。
fn load_body(
    root: &Path,
    root_canon: &Path,
    scope: &LoadedScope,
    step_id: &str,
    body_ref: &str,
    bodies: &mut BTreeMap<String, String>,
    diags: &mut Vec<Diagnostic>,
) {
    let item = format!("step:{step_id}");
    if !is_safe_rel(body_ref) {
        diags.push(diag(
            &scope.file,
            &item,
            DiagCode::RefBody,
            format!("unsafe body path: {body_ref}"),
        ));
        return;
    }
    let key = format!("{}/{body_ref}", scope.dir);
    if bodies.contains_key(&key) {
        return;
    }
    let path = root.join(&key);
    if !is_plain_file_under(root_canon, &path) {
        diags.push(diag(
            &scope.file,
            &item,
            DiagCode::RefBody,
            format!("body file not found: {body_ref}"),
        ));
        return;
    }
    match read_text(&path, MAX_MARKDOWN_BYTES) {
        Ok(t) => {
            bodies.insert(key, t);
        }
        Err(ReadError::TooLarge) => diags.push(diag(
            &key,
            "file",
            DiagCode::LimitMdSize,
            format!("markdown file exceeds {MAX_MARKDOWN_BYTES} bytes"),
        )),
        Err(ReadError::NotUtf8) => diags.push(diag(
            &key,
            "file",
            DiagCode::Parse,
            "body is not valid UTF-8",
        )),
        Err(ReadError::Io) => diags.push(diag(
            &scope.file,
            &item,
            DiagCode::RefBody,
            format!("cannot read body: {body_ref}"),
        )),
    }
}

/// 練習データの宣言を確かめる。中身は読まない。
fn check_fixtures(
    root_canon: &Path,
    root: &Path,
    workshop_file: &str,
    ws: &WorkshopSrc,
    diags: &mut Vec<Diagnostic>,
) {
    for (id, f) in &ws.fixtures {
        let item = format!("fixture:{id}");
        let under_fixtures = Path::new(&f.path)
            .components()
            .next()
            .is_some_and(|c| c == Component::Normal("fixtures".as_ref()));
        let ok = is_safe_rel(&f.path)
            && under_fixtures
            && is_plain_file_under(root_canon, &root.join(&f.path))
            && fs::metadata(root.join(&f.path)).is_ok_and(|m| m.len() <= MAX_PACK_BYTES);
        if !ok {
            diags.push(diag(
                workshop_file,
                &item,
                DiagCode::RefFixture,
                format!("fixture path is invalid or missing: {}", f.path),
            ));
        }
    }
}

/// 教材パックを読み込む。
///
/// 構文・未知の欄・参照の欠け・上限の超過は、診断の一覧として返す。ファイルシステムに触れる唯一の
/// 入口で、`compile` はここで読んだ `LoadedPack` だけを見る。
pub fn load(dir: &Path) -> Result<LoadedPack, Vec<Diagnostic>> {
    if !fs::metadata(dir).is_ok_and(|m| m.is_dir()) {
        return Err(vec![diag(
            ".",
            "pack",
            DiagCode::Parse,
            "pack path is not a directory",
        )]);
    }
    let over = check_limits(dir);
    if !over.is_empty() {
        return Err(over);
    }
    let (format, wpath) = detect_format(dir)?;
    let root_canon = dir.canonicalize().map_err(|_| {
        vec![diag(
            ".",
            "pack",
            DiagCode::Parse,
            "pack directory cannot be resolved",
        )]
    })?;

    let mut diags = Vec::new();
    let workshop_file = rel(dir, &wpath);
    let workshop: Option<WorkshopSrc> =
        read_parse(format, &wpath, &workshop_file, "pack", &mut diags);

    let mut scopes = Vec::new();
    for (sub, name, kind) in [
        ("chapters", "chapter", ScopeKind::Chapter),
        ("modules", "module", ScopeKind::Module),
    ] {
        for d in sorted_subdirs(dir, &dir.join(sub), &mut diags) {
            let p = d.join(format!("{name}.{}", format.ext()));
            let dir_rel = rel(dir, &d);
            if !is_plain_file_under(&root_canon, &p) {
                diags.push(diag(
                    &dir_rel,
                    "scope",
                    DiagCode::Structure,
                    format!("{name}.{} is missing", format.ext()),
                ));
                continue;
            }
            let file = rel(dir, &p);
            if let Some(src) = read_parse::<ScopeSrc>(format, &p, &file, "scope", &mut diags) {
                scopes.push(LoadedScope {
                    kind,
                    file,
                    dir: dir_rel,
                    src,
                });
            }
        }
    }

    let tp = dir
        .join("tests")
        .join(format!("playthrough.{}", format.ext()));
    // 試験は省略できる。省略と認めるのは「存在しない」ときだけで、ディレクトリ・特殊ファイル・
    // シンボリックリンク・NotFound 以外のメタデータ取得失敗は、読み込みの失敗として診断にする。
    let tests_rel = rel(dir, &tp);
    let (tests_file, tests) = match fs::symlink_metadata(&tp) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (None, None),
        Err(_) => {
            diags.push(diag(
                &tests_rel,
                "tests",
                DiagCode::Structure,
                "cannot read test file metadata",
            ));
            (None, None)
        }
        Ok(_) if !is_plain_file_under(&root_canon, &tp) => {
            diags.push(diag(
                &tests_rel,
                "tests",
                DiagCode::Structure,
                "test file must be a regular file under the pack root",
            ));
            (None, None)
        }
        Ok(_) => {
            let t = read_parse::<TestsSrc>(format, &tp, &tests_rel, "tests", &mut diags);
            (Some(tests_rel), t)
        }
    };

    let mut bodies = BTreeMap::new();
    for scope in &scopes {
        for step in &scope.src.steps {
            let refs = step.body.iter().chain(step.body_levels.values());
            for b in refs {
                load_body(
                    dir,
                    &root_canon,
                    scope,
                    &step.id,
                    b,
                    &mut bodies,
                    &mut diags,
                );
            }
        }
    }
    if let Some(ws) = &workshop {
        check_fixtures(&root_canon, dir, &workshop_file, ws, &mut diags);
    }

    match workshop {
        Some(workshop) if diags.is_empty() => Ok(LoadedPack {
            format,
            workshop_file,
            workshop,
            scopes,
            bodies,
            tests_file,
            tests,
        }),
        _ => Err(diags),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_errors_do_not_echo_input_values() {
        let toml_src = "[pack]\nid = 12345\ntitle = \"SECRET-VALUE\"\nbogus = SECRET-VALUE\n";
        let e = Format::Toml.parse::<WorkshopSrc>(toml_src).unwrap_err();
        assert!(!e.contains("SECRET-VALUE") && e.contains("line"), "{e}");
        let yaml_src = "pack:\n  id: [SECRET-VALUE]\n";
        let e = Format::Yaml.parse::<WorkshopSrc>(yaml_src).unwrap_err();
        assert!(!e.contains("SECRET-VALUE"), "{e}");
    }

    #[test]
    fn missing_optional_dir_is_empty_but_other_errors_are_diagnosed() {
        let tmp = std::env::temp_dir().join(format!("fw-src-{}", std::process::id()));
        fs::create_dir_all(&tmp).unwrap();
        let mut diags = Vec::new();
        assert!(sorted_subdirs(&tmp, &tmp.join("chapters"), &mut diags).is_empty());
        assert!(diags.is_empty());
        // ディレクトリでないパスは NotFound 以外のエラーになる。
        fs::write(tmp.join("modules"), "x").unwrap();
        assert!(sorted_subdirs(&tmp, &tmp.join("modules"), &mut diags).is_empty());
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].file(), "modules");
        fs::remove_dir_all(&tmp).unwrap();
    }
}
