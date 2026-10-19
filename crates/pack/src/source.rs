//! 教材パックのディレクトリを読み込む層（構造定義・本文・練習データの内容）。
//!
//! `compile`（`crate::compile`）の前段で、ディスクに触れるのはここと `metrics`（走査と読み込みは本モジュールの関数を使う）だけである。教材データは信頼しない
//! 入力として扱い、(1) 中身を読む前に上限・シンボリックリンク・入れ子の深さを確かめ、(2) 本文と練習データの
//! パスは安全な相対パス（`..`・絶対パス・`\` を含まない）に限り、(3) 読むときは上限 + 1 バイトで打ち切る。
//! 診断のメッセージにはパスと ID だけを入れ、本文・練習データの中身は入れない。
//!
//! 上限の値と拒否理由の文面は契約（TASK-199、`contract::limits`）の共有定数・`PackLimitCode` に従う。
//! 形式の判定（`workshop.yaml` と `workshop.toml` の両方・どちらも無いは拒否。PACK-2）と、語彙にない欄の
//! 拒否（`unknown field`。PACK-3）は本モジュールが扱う。引用符なしの値の文字列としての読み取り・重複キーの拒否・
//! 構文エラーの行と桁（PACK-4）も本モジュールと `yaml_strict` が扱う。モジュールの展開は `compile`（`expand`）が行う。
//! 形式 B のアンカー・エイリアス・タグ・複数文書の拒否（PACK-5）も `yaml_strict` が行い、ここで診断文にする。

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use fandhe_workshop_contract::ir::{is_supported_ir_format, FixtureDef, PackInfo, Start, Step};
use fandhe_workshop_contract::limits::{self, PackLimitCode};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use crate::diag::{DiagCode, Diagnostic};
use crate::yaml_strict::{self, StrictError};

/// 上限の値は契約（TASK-199）の共有定数を使い、独自に持たない。
const MAX_MARKDOWN_BYTES: u64 = limits::MAX_MARKDOWN_BYTES;
const MAX_PACK_BYTES: u64 = limits::MAX_PACK_BYTES;
pub(crate) const MAX_DEPTH: usize = limits::MAX_PACK_DIR_DEPTH;

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
            Format::Toml => {
                // 10 進でない整数は `toml` が黙って数に読むので、型付きの読み込みの前に拒否する（PACK-5）。
                if let Some(pos) = toml_radix_int(text) {
                    return Err(non_decimal_int_message(Some(pos)));
                }
                toml::from_str(text).map_err(|e| {
                    let pos = e.span().map(|r| line_col(text, r.start));
                    if e.message() == "duplicate key" {
                        return duplicate_key_message(None, pos);
                    }
                    classify_parse_error(e.message(), pos)
                })
            }
            Format::Yaml => {
                // 型付きの読み込みの前に重複キーを拒否する（PACK-4）。重複のある文書は構文として信頼できない
                // ので、`ir_format` の版の検査（`probe_format_version`）より前に `E-PARSE` で止まる。
                match yaml_strict::check_duplicate_keys(text) {
                    Ok(()) => {}
                    Err(StrictError::Duplicate { name, pos }) => {
                        return Err(duplicate_key_message(name.as_deref(), pos));
                    }
                    Err(StrictError::Other { raw, pos }) => {
                        return Err(classify_parse_error(&raw, pos));
                    }
                    Err(StrictError::NonDecimalInt { .. } | StrictError::Feature { .. }) => {}
                }
                // アンカー・エイリアス・タグ・複数文書は `yaml_serde` が黙って展開・解決するので拒否する（PACK-5）。
                match yaml_strict::check_yaml_features(text) {
                    Ok(()) => {}
                    Err(StrictError::Feature { kind, pos }) => {
                        return Err(yaml_feature_message(kind, pos));
                    }
                    Err(StrictError::Other { raw, pos }) => {
                        return Err(classify_parse_error(&raw, pos));
                    }
                    Err(StrictError::Duplicate { name, pos }) => {
                        return Err(duplicate_key_message(name.as_deref(), pos));
                    }
                    Err(StrictError::NonDecimalInt { pos }) => {
                        return Err(non_decimal_int_message(pos));
                    }
                }
                // 10 進でない整数（`0x1f` 等）は `yaml_serde` が黙って数に読むので拒否する（PACK-5）。
                match yaml_strict::check_radix_ints(text) {
                    Ok(()) => {}
                    Err(StrictError::NonDecimalInt { pos }) => {
                        return Err(non_decimal_int_message(pos));
                    }
                    Err(StrictError::Other { raw, pos }) => {
                        return Err(classify_parse_error(&raw, pos));
                    }
                    Err(StrictError::Duplicate { name, pos }) => {
                        return Err(duplicate_key_message(name.as_deref(), pos));
                    }
                    Err(StrictError::Feature { kind, pos }) => {
                        return Err(yaml_feature_message(kind, pos));
                    }
                }
                yaml_serde::from_str(text).map_err(|e| {
                    let pos = e.location().map(|l| (l.line(), l.column()));
                    classify_parse_error(&e.to_string(), pos)
                })
            }
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

/// `workshop.*` から `pack.ir_format` だけを先に読むための型。`deny_unknown_fields` を付けない。
///
/// 版が違う教材パックは、版 1 の語彙で解釈すると `E-PARSE` などの別の診断に化ける。版を最初に
/// 判定し、1 でなければ残りを解釈せずに止める（失敗を安全側へ倒す）。
#[derive(Deserialize)]
struct VersionProbe {
    pack: Option<PackProbe>,
}

#[derive(Deserialize)]
struct PackProbe {
    ir_format: Option<Value>,
}

/// `pack.ir_format` が整数の 1 または 2 でないときの診断。構文として読めない・`pack` が無いときは `None`
/// （型付きの読み込みが `E-PARSE` にする）。診断に入力値は写さず、整数として読めたときだけ値を出す。
fn probe_format_version(format: Format, text: &str, file: &str) -> Option<Diagnostic> {
    let probe = format.parse::<VersionProbe>(text).ok()?;
    let pack = probe.pack?;
    let message = match pack.ir_format.as_ref().and_then(Value::as_i64) {
        Some(v) if u32::try_from(v).is_ok_and(is_supported_ir_format) => return None,
        Some(v) => format!("ir_format {v} is not supported (expected 1 or 2)"),
        None => "ir_format must be the integer 1 or 2 (missing or not an integer)".to_string(),
    };
    Some(diag(
        file,
        "pack:ir_format",
        DiagCode::FormatVersion,
        message,
    ))
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
    /// 練習データの内容。キーは `workshop.fixtures` の ID。`compile` が木にする（TASK-60）。
    pub fixture_texts: BTreeMap<String, String>,
    /// 教材テストのパック根からの相対パス。
    pub tests_file: Option<String>,
    /// 教材テストの内容。
    pub tests: Option<TestsSrc>,
    /// `pack.vocab` が欄ごと省略ではなく、値のない `null` として明示されている（形式 B のみ。TOML に null は無い）。
    /// `PackInfo.vocab` は省略も `null` も `None` に読むため、読み込み時に区別して残す（PACK-31）。
    pub vocab_explicit_null: bool,
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

/// 10 進でない整数の診断文。値も欄の名前も写さず、位置だけを添える。
fn non_decimal_int_message(pos: Option<(usize, usize)>) -> String {
    match pos {
        Some((line, col)) => {
            format!("integer at line {line} column {col} must be written in decimal")
        }
        None => "integer must be written in decimal".to_string(),
    }
}

/// アンカー・エイリアス・タグ・複数文書の診断文。アンカー名・タグの字面・値は写さず、位置だけを添える。
fn yaml_feature_message(kind: yaml_strict::YamlFeature, pos: Option<(usize, usize)>) -> String {
    use yaml_strict::YamlFeature;
    let what = match kind {
        YamlFeature::Anchor | YamlFeature::Alias => "anchors and aliases",
        YamlFeature::Tag => "tags",
        YamlFeature::MultipleDocuments => "multiple documents",
    };
    match pos {
        Some((line, col)) => format!("{what} are not allowed at line {line} column {col}"),
        None => format!("{what} are not allowed"),
    }
}

/// 形式 A で、10 進でない整数の最初の位置（行・桁）。構文として読めない文書は `None`
/// （既存の `toml::from_str` が構文エラーとして扱う）。
fn toml_radix_int(text: &str) -> Option<(usize, usize)> {
    use toml::de::{DeTable, DeValue};
    fn walk(text: &str, v: &toml::Spanned<DeValue<'_>>) -> Option<(usize, usize)> {
        match v.get_ref() {
            DeValue::Integer(i) if i.radix() != 10 => Some(line_col(text, v.span().start)),
            DeValue::Array(a) => a.iter().find_map(|x| walk(text, x)),
            DeValue::Table(t) => t.values().find_map(|x| walk(text, x)),
            _ => None,
        }
    }
    let table = DeTable::parse(text).ok()?;
    table.get_ref().values().find_map(|v| walk(text, v))
}

/// 入力値を含まない構文エラーのメッセージ。位置が分かれば行と桁を添える。
fn parse_failure(pos: Option<(usize, usize)>) -> String {
    match pos {
        Some((line, col)) => format!("invalid syntax or structure at line {line} column {col}"),
        None => "invalid syntax or structure".to_string(),
    }
}

/// 欄・キーの名前を診断へ写してよいか。作成者の手がかりになるが任意の文字列になりうるため、短い識別子
/// （`[A-Za-z0-9_.-]{1,64}`）のときだけ写す。未知の欄と重複キー（`yaml_strict`）で共有する。
pub(crate) fn safe_ident(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

/// 重複キーの診断文。名前は呼び出し側が `safe_ident` を通したものだけを渡す。
fn duplicate_key_message(name: Option<&str>, pos: Option<(usize, usize)>) -> String {
    let mut msg = match name {
        Some(n) => format!("duplicate key `{n}`"),
        None => "duplicate key".to_string(),
    };
    if let Some((line, col)) = pos {
        msg.push_str(&format!(" at line {line} column {col}"));
    }
    msg
}

/// パーサーの文言が serde の未知の欄（PACK-3）なら `unknown field` の診断文にし、それ以外は
/// `parse_failure` にする。欄の名前は作成者が直す手がかりになるが任意の文字列になりうるため、短い識別子
/// （`[A-Za-z0-9_.-]{1,64}`）のときだけ写す。入力の値は写さない。
fn classify_parse_error(raw: &str, pos: Option<(usize, usize)>) -> String {
    const MARK: &str = "unknown field `";
    // 文言の全文から部分一致で探さない。型が合わない入力値（``invalid type: string "unknown field `x`..."``）
    // にも同じ語が入りうるためである。未知の欄の文言は、先頭（または `パス: ` の直後）から始まるものだけを
    // 認める。パスは `[A-Za-z0-9_.\[\]-]` だけで、それ以外が前に付くものは位置だけの診断に戻す。
    let Some(start) = raw.find(MARK) else {
        return parse_failure(pos);
    };
    let prefix = &raw[..start];
    let path_ok = prefix.is_empty()
        || prefix.strip_suffix(": ").is_some_and(|p| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-' | '[' | ']'))
        });
    if !path_ok {
        return parse_failure(pos);
    }
    let start = start + MARK.len();
    // serde の文言は ``unknown field `名前`, expected ...`` / ``..., there are no fields``。名前に
    // バッククォートが入っても途中で切れないよう、後ろの区切りまで取り、見つからなければ写さない。
    let rest = &raw[start..];
    let end = ["`, expected", "`, there are no fields"]
        .iter()
        .filter_map(|d| rest.find(d))
        .min();
    let name = end.map_or("", |i| &rest[..i]);
    let safe = safe_ident(name);
    let mut msg = if safe {
        format!("unknown field `{name}`")
    } else {
        "unknown field".to_string()
    };
    if let Some((line, col)) = pos {
        msg.push_str(&format!(" at line {line} column {col}"));
    }
    msg
}

/// 診断を作る。`file` が空のときは `.`（パックの根）に直す。直した後は必ず作れる。
fn diag(file: &str, item: &str, code: DiagCode, message: impl Into<String>) -> Diagnostic {
    let file = if file.trim().is_empty() { "." } else { file };
    let item = if item.trim().is_empty() { "file" } else { item };
    // file と item を空でない値に直したので、Diagnostic::new は失敗しない。
    Diagnostic::new(file, item, code, message).expect("file and item are non-empty")
}

/// シンボリックリンク・入れ子の深さの上限超過の診断（`metrics` の走査が使う）。
pub(crate) fn limit_diag(file: &str, code: DiagCode) -> Diagnostic {
    let msg = match code {
        DiagCode::LimitSymlink => PackLimitCode::Symlink.message(),
        _ => PackLimitCode::DirDepth.message(),
    };
    diag(file, "file", code, msg)
}

/// パック根からの相対パスを `/` 区切りで書く。根自身は `.`。
pub(crate) fn rel(root: &Path, p: &Path) -> String {
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
pub(crate) fn scan_failure(file: &str) -> Diagnostic {
    diag(
        file,
        "file",
        DiagCode::Structure,
        "cannot scan directory or read entry metadata",
    )
}

/// 走査するエントリ（ファイル・ディレクトリ・リンクなど、種類を問わない）の数の上限。
///
/// 0 バイトのファイル・ディレクトリ・リンクはパック全体の大きさを増やさないため、大きさの予算だけでは
/// 走査の量と診断の確保が入力の幅に比例して増える。spec（SAFE-22・SAFE-43）と契約は件数の上限を
/// 定めていないので、契約で唯一の要素数の上限（`MAX_COLLECTION_ELEMENTS`。SAFE-25・D-45 の、IR の
/// 1 つのコレクションの要素数）を流用した仮の値とする。教材パックの件数としての根拠は無く、spec・契約に
/// 値が入ったら置き換える。testdata の教材パックは 1 つあたり 60 エントリ未満（2026-10-15 計測）である。
const MAX_SCAN_ENTRIES: usize = limits::MAX_COLLECTION_ELEMENTS;

/// 中身を読む前の走査で積む診断の数の上限。達したら走査をやめる。
///
/// 上限の診断は 1 件でもあればパックを読まずに拒否するので、すべてを数え上げる必要は無い。spec に値が無く、
/// 作成者が 1 回の実行で直せる量として 100 件にした仮の値である。
const MAX_SCAN_DIAGS: usize = 100;

/// 中身を読む前の走査。シンボリックリンク・入れ子の深さ・Markdown の大きさ・パック全体の大きさを数える。
/// リンクは辿らない。診断は相対パスの順に並べる。
pub(crate) fn check_limits(root: &Path) -> Vec<Diagnostic> {
    check_limits_with(root, MAX_SCAN_ENTRIES, MAX_SCAN_DIAGS)
}

/// エントリ数が多すぎることの診断。
fn too_many_entries(max_entries: usize) -> Diagnostic {
    diag(
        ".",
        "pack",
        DiagCode::Limit,
        format!("pack has more than {max_entries} files and directories"),
    )
}

/// `check_limits` の本体。上限を引数で受け、試験で小さな値を渡せるようにする。
///
/// エントリ数が `max_entries` を超えたら、その時点で上限超過の診断 1 件だけを返す。診断が `max_diags` 件に
/// 達したら走査をやめる。どちらで打ち切っても結果がファイルシステムの列挙順に依らないよう、各ディレクトリの
/// エントリは名前の順に処理する。
fn check_limits_with(root: &Path, max_entries: usize, max_diags: usize) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut total: u64 = 0;
    let mut seen = 0usize;
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    'walk: while let Some((d, depth)) = stack.pop() {
        if diags.len() >= max_diags {
            break;
        }
        let rd = match fs::read_dir(&d) {
            Ok(rd) => rd,
            Err(_) => {
                diags.push(scan_failure(&rel(root, &d)));
                continue;
            }
        };
        // 確保の前に数えるので、1 つのディレクトリが巨大でも `max_entries` を超えて確保しない。
        let mut entries = Vec::new();
        for e in rd {
            seen += 1;
            if seen > max_entries {
                return vec![too_many_entries(max_entries)];
            }
            match e {
                Ok(e) => entries.push(e.path()),
                Err(_) => diags.push(scan_failure(&rel(root, &d))),
            }
        }
        entries.sort();
        let mut subdirs = Vec::new();
        for p in entries {
            if diags.len() >= max_diags {
                break 'walk;
            }
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
                    PackLimitCode::Symlink.message(),
                ));
            } else if md.is_dir() {
                if depth < MAX_DEPTH {
                    subdirs.push((p, depth + 1));
                } else {
                    diags.push(diag(
                        &file,
                        "file",
                        DiagCode::LimitDepth,
                        PackLimitCode::DirDepth.message(),
                    ));
                }
            } else if md.is_file() {
                total = total.saturating_add(md.len());
                if p.extension().is_some_and(|x| x == "md") && md.len() > MAX_MARKDOWN_BYTES {
                    diags.push(diag(
                        &file,
                        "file",
                        DiagCode::LimitMdSize,
                        PackLimitCode::MdSize.message(),
                    ));
                }
            }
        }
        // 名前の順に辿るため、逆順に積む。
        stack.extend(subdirs.into_iter().rev());
    }
    if total > MAX_PACK_BYTES && diags.len() < max_diags {
        diags.push(diag(
            ".",
            "pack",
            DiagCode::LimitPackSize,
            PackLimitCode::PackSize.message(),
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
    let text = read_diag_text(path, file, item, diags)?;
    parse_diag(format, &text, file, item, diags)
}

/// 章・モジュールの定義を読む。形式 B では、型付きの読み込みが `null`・`~` を `None` にしてしまう
/// `body`・`do` を、同じテキストの文字列としての読み直しで補う（PACK-4）。読み直しに失敗した、または
/// Step の件数が食い違うときは黙って飛ばさず `E-PARSE` にする（fail-closed）。
fn read_scope(
    format: Format,
    path: &Path,
    file: &str,
    diags: &mut Vec<Diagnostic>,
) -> Option<ScopeSrc> {
    let text = read_diag_text(path, file, "scope", diags)?;
    let mut src = parse_diag::<ScopeSrc>(format, &text, file, "scope", diags)?;
    if format == Format::Yaml {
        match yaml_strict::string_probe(&text) {
            Ok(strs) if strs.len() == src.steps.len() => {
                for (step, s) in src.steps.iter_mut().zip(strs) {
                    // 空の値（`do:` だけ）は型の変換ではないので省略のままにする。
                    if step.body.is_none() {
                        step.body = s.body.filter(|v| !v.is_empty());
                    }
                    if step.do_.is_none() {
                        step.do_ = s.do_.filter(|v| !v.is_empty());
                    }
                }
            }
            _ => {
                diags.push(diag(
                    file,
                    "scope",
                    DiagCode::Parse,
                    "string fields could not be re-read",
                ));
                return None;
            }
        }
    }
    Some(src)
}

/// 上限つきで読み、失敗は診断にする。
pub(crate) fn read_diag_text(
    path: &Path,
    file: &str,
    item: &str,
    diags: &mut Vec<Diagnostic>,
) -> Option<String> {
    match read_text(path, MAX_PACK_BYTES) {
        Ok(t) => Some(t),
        Err(ReadError::NotUtf8) => {
            diags.push(diag(file, item, DiagCode::Parse, "file is not valid UTF-8"));
            None
        }
        Err(ReadError::TooLarge) => {
            diags.push(diag(file, item, DiagCode::Parse, "file is too large"));
            None
        }
        Err(ReadError::Io) => {
            diags.push(diag(file, item, DiagCode::Parse, "cannot read file"));
            None
        }
    }
}

/// 読んだ文字列を型付きで読み、失敗は診断にする。
fn parse_diag<T: DeserializeOwned>(
    format: Format,
    text: &str,
    file: &str,
    item: &str,
    diags: &mut Vec<Diagnostic>,
) -> Option<T> {
    match format.parse::<T>(text) {
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
pub(crate) fn detect_format(root: &Path) -> Result<(Format, PathBuf), Vec<Diagnostic>> {
    let toml_p = root.join("workshop.toml");
    let yaml_p = root.join("workshop.yaml");
    // 種類が違うエントリ（ディレクトリなど）を「無い」と扱うと、もう片方の形式が黙って選ばれうる。
    // 名前のエントリがあるかで判定し、通常のファイルでなければ拒否する。
    let exists = |p: &Path| fs::symlink_metadata(p).is_ok();
    let is_file = |p: &Path| fs::symlink_metadata(p).is_ok_and(|m| m.is_file());
    let not_file = |name: &str| {
        Err(vec![diag(
            name,
            "pack",
            DiagCode::Parse,
            format!("{name} is not a regular file"),
        )])
    };
    match (exists(&toml_p), exists(&yaml_p)) {
        (true, false) if !is_file(&toml_p) => not_file("workshop.toml"),
        (false, true) if !is_file(&yaml_p) => not_file("workshop.yaml"),
        (true, false) => Ok((Format::Toml, toml_p)),
        (false, true) => Ok((Format::Yaml, yaml_p)),
        (true, true) => Err(vec![diag(
            "workshop.toml",
            "pack",
            DiagCode::Parse,
            "both workshop.toml and workshop.yaml exist; use one format",
        )]),
        (false, false) => Err(vec![diag(
            "workshop.yaml",
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
            PackLimitCode::MdSize.message(),
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

/// 練習データの宣言を確かめ、内容を読んで `texts` に入れる。
///
/// 読むのは、安全な相対パス・`fixtures/` の下・通常のファイル（シンボリックリンクとパック根の外を除く）を
/// 通ったものだけである。上限は拡張子が `.md` なら本文と同じ `MAX_MARKDOWN_BYTES`、それ以外は
/// `MAX_PACK_BYTES` で、上限 + 1 バイトで打ち切って読む。診断には ID とパスだけを入れ、内容は入れない。
fn check_fixtures(
    root_canon: &Path,
    root: &Path,
    workshop_file: &str,
    ws: &WorkshopSrc,
    texts: &mut BTreeMap<String, String>,
    diags: &mut Vec<Diagnostic>,
) {
    check_fixtures_with(
        root_canon,
        root,
        workshop_file,
        ws,
        texts,
        diags,
        limits::MAX_IR_BYTES,
    );
}

/// `check_fixtures` の本体。保持する内容の合計の上限を引数で受け、試験で小さな値を渡せるようにする。
fn check_fixtures_with(
    root_canon: &Path,
    root: &Path,
    workshop_file: &str,
    ws: &WorkshopSrc,
    texts: &mut BTreeMap<String, String>,
    diags: &mut Vec<Diagnostic>,
    max_retained: u64,
) {
    // 保持する内容の累積量。異なる ID が同じファイルを指しても ID ごとに別の String を持つので、
    // 実ファイルの大きさ（パック全体の上限）では増幅を防げない。読む前に数えて、超える前に打ち切る。
    let mut retained: u64 = 0;
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
            continue;
        }
        // `ok` で取得に成功しているので、読む前の大きさを累積量に足す。
        let size = fs::metadata(root.join(&f.path)).map_or(0, |m| m.len());
        retained = retained.saturating_add(size);
        if retained > max_retained {
            diags.push(diag(
                workshop_file,
                &item,
                DiagCode::Limit,
                "fixtures to load exceed the total size limit",
            ));
            return;
        }
        let is_md = Path::new(&f.path)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("md"));
        match read_text(
            &root.join(&f.path),
            if is_md {
                MAX_MARKDOWN_BYTES
            } else {
                MAX_PACK_BYTES
            },
        ) {
            Ok(t) => {
                texts.insert(id.clone(), t);
            }
            Err(ReadError::TooLarge) if is_md => diags.push(diag(
                &f.path,
                "file",
                DiagCode::LimitMdSize,
                PackLimitCode::MdSize.message(),
            )),
            Err(ReadError::TooLarge) => diags.push(diag(
                &f.path,
                &item,
                DiagCode::Parse,
                "fixture is too large",
            )),
            Err(ReadError::NotUtf8) => diags.push(diag(
                &f.path,
                &item,
                DiagCode::Parse,
                "fixture is not valid UTF-8",
            )),
            Err(ReadError::Io) => diags.push(diag(
                workshop_file,
                &item,
                DiagCode::RefFixture,
                format!("cannot read fixture: {}", f.path),
            )),
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
    let mut workshop: Option<WorkshopSrc> = None;
    let mut vocab_explicit_null = false;
    if let Some(text) = read_diag_text(&wpath, &workshop_file, "pack", &mut diags) {
        if let Some(d) = probe_format_version(format, &text, &workshop_file) {
            return Err(vec![d]);
        }
        workshop = parse_diag(format, &text, &workshop_file, "pack", &mut diags);
        if workshop.is_some() && format == Format::Yaml {
            vocab_explicit_null = yaml_serde::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.get("pack").and_then(|p| p.get("vocab")).cloned())
                .is_some_and(|v| v.is_null());
        }
    }

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
            if let Some(src) = read_scope(format, &p, &file, &mut diags) {
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
    let mut fixture_texts = BTreeMap::new();
    if let Some(ws) = &workshop {
        check_fixtures(
            &root_canon,
            dir,
            &workshop_file,
            ws,
            &mut fixture_texts,
            &mut diags,
        );
    }

    match workshop {
        Some(workshop) if diags.is_empty() => Ok(LoadedPack {
            format,
            workshop_file,
            workshop,
            scopes,
            bodies,
            fixture_texts,
            tests_file,
            tests,
            vocab_explicit_null,
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
    fn classify_unknown_field_names() {
        let p = Some((3, 5));
        let raw = "unknown field `script`, expected one of `id`, `title`";
        assert_eq!(
            classify_parse_error(raw, p),
            "unknown field `script` at line 3 column 5"
        );
        assert_eq!(
            classify_parse_error("steps[0]: unknown field `exec`, expected `id`", None),
            "unknown field `exec`"
        );
        let long = format!("unknown field `{}`, expected x", "a".repeat(65));
        assert_eq!(
            classify_parse_error(&long, p),
            "unknown field at line 3 column 5"
        );
        assert_eq!(
            classify_parse_error("unknown field `a b`, expected x", p),
            "unknown field at line 3 column 5"
        );
        // 型の不一致のエラーに入った入力値は、未知の欄として写さない。
        for raw in [
            "pack: invalid type: string \"unknown field `SECRET-VALUE`, expected x\", expected struct",
            "invalid type: string \"unknown field `SECRET-VALUE`, expected x\", expected a map",
            "pack: \"unknown field `SECRET-VALUE`, expected x",
        ] {
            let m = classify_parse_error(raw, p);
            assert!(!m.contains("SECRET-VALUE"), "{m}");
            assert_eq!(m, "invalid syntax or structure at line 3 column 5");
        }
        assert_eq!(
            classify_parse_error("invalid type: string", p),
            "invalid syntax or structure at line 3 column 5"
        );
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

    /// 試験ごとの一時ディレクトリ（空で作る）。
    fn scan_tmp(tag: &str) -> PathBuf {
        let tmp = std::env::temp_dir().join(format!("fw-scan-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        tmp
    }

    #[test]
    fn scan_stops_at_entry_budget_even_for_empty_files_and_dirs() {
        // 0 バイトのファイルと空のディレクトリは大きさを増やさないが、件数には数える。
        let tmp = scan_tmp("entries");
        fs::create_dir_all(tmp.join("d")).unwrap();
        for i in 0..4 {
            fs::write(tmp.join(format!("d/e{i}")), b"").unwrap();
        }
        // d と e0〜e3 の 5 件。ちょうどは通し、1 件少ない上限では断る。
        assert!(check_limits_with(&tmp, 5, MAX_SCAN_DIAGS).is_empty());
        let d = check_limits_with(&tmp, 4, MAX_SCAN_DIAGS);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code(), DiagCode::Limit);
        assert_eq!(d[0].file(), ".");
        fs::remove_dir_all(&tmp).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn scan_caps_diagnostics_and_keeps_the_first_by_name() {
        let tmp = scan_tmp("links");
        for i in 0..10 {
            std::os::unix::fs::symlink("nowhere", tmp.join(format!("l{i}"))).unwrap();
        }
        let d = check_limits_with(&tmp, MAX_SCAN_ENTRIES, 3);
        let files: Vec<&str> = d.iter().map(Diagnostic::file).collect();
        assert_eq!(files, ["l0", "l1", "l2"]);
        assert!(d.iter().all(|x| x.code() == DiagCode::LimitSymlink));
        // 既定の上限でも、リンクの数だけ診断を積み続けない。
        for i in 10..(MAX_SCAN_DIAGS + 10) {
            std::os::unix::fs::symlink("nowhere", tmp.join(format!("m{i}"))).unwrap();
        }
        assert_eq!(check_limits(&tmp).len(), MAX_SCAN_DIAGS);
        fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn fixtures_sharing_one_file_are_capped_before_reading_all() {
        let tmp = scan_tmp("fixtures");
        fs::create_dir_all(tmp.join("fixtures")).unwrap();
        fs::write(tmp.join("fixtures/a.txt"), vec![b'x'; 100]).unwrap();
        let yaml = "pack: {id: p, title: t, version: '1', ir_format: 1, guidance: A}\nfixtures:\n"
            .to_string()
            + &(0..10)
                .map(|i| format!("  f{i}: {{path: fixtures/a.txt, kind: text}}\n"))
                .collect::<String>();
        let ws: WorkshopSrc = yaml_serde::from_str(&yaml).unwrap();
        let root = tmp.canonicalize().unwrap();
        let (mut texts, mut diags) = (BTreeMap::new(), Vec::new());
        check_fixtures_with(
            &root,
            &tmp,
            "workshop.yaml",
            &ws,
            &mut texts,
            &mut diags,
            350,
        );
        // 3 件（300 バイト）まで読み、4 件目で打ち切る。
        assert_eq!(texts.len(), 3);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code(), DiagCode::Limit);
        let (mut texts, mut diags) = (BTreeMap::new(), Vec::new());
        check_fixtures_with(
            &root,
            &tmp,
            "workshop.yaml",
            &ws,
            &mut texts,
            &mut diags,
            1000,
        );
        assert_eq!(texts.len(), 10);
        assert!(diags.is_empty());
        fs::remove_dir_all(&tmp).unwrap();
    }
}
