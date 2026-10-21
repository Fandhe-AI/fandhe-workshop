//! 秘密情報らしい文字列の検出（SAFE-27・TASK-164 1/3）。
//!
//! 呼び出し元は web の `secret_notice` で、履歴を保存先へ書く直前の値を調べ、見つかった形の名前だけを警告に出す。
//! 教材を取り込むときの検査（TASK-420）も、pack からこの module を再利用する（pack から core への向きは許可されている）。
//! 検出は純関数で、I/O・時計・ブラウザを持たない。正規表現の crate は足さず、接頭辞と文字種・長さで判定する。
//!
//! 見つかった値は、値そのものも参照も返さない。[`Finding`] が持つのは形と 32 ビットの指紋だけで、指紋は後続（TASK-164 3/3）が
//! 「確認済みの値か」を照合するために置く。画面・ログ・保存先のどこにも出力しない。
//!
//! 限界（確認した事実）:
//! - 規則は固定の 15 形（[`SecretShape::ALL`]。SAFE-27 の 10 形と、SAFE-30・決定 D-45 で足した 5 形）に合わせたもので、
//!   一般の秘密情報の検出率を保証しない。見逃しも過剰な検出もありうる。追加の 5 形の選定は 2026-10-21 にユーザーが確定した（決定 D-66）。
//! - 見送った形（旧い接頭辞なしの長いキー・Webhook の URL・区切りが複数ある形・OAuth のクライアントシークレット・
//!   一般の高エントロピー文字列）は検出しない。値を分けて入力した場合や符号化した値も見逃す。
//! - 接頭辞のあとに下限以上の文字種が続く説明文や試験用の値は、過剰に検出しうる（受講者は「このまま保存する」で保存できる）。
//! - 秘密鍵は、BEGIN 行と種類の語（`PRIVATE KEY-----`）が揃えば END 行が無くても検出する（切り詰めた貼り付けも止める）。
//! - 英数字に接した接頭辞は検出しない（`xAKIA...`・`aghp_...` など、直前が英数字のものは語の途中として扱う。秘密鍵の枠と
//!   接続文字列を除く形に当たる）。その結果、二重エンコードの JSON 中の `\n` の直後に続くトークン（`\nghp_...`）のような
//!   エスケープ残りも検出できない。
//! - 接頭辞の大文字小文字は区別する。名前つきの形（`aws_secret_access_key`）の名前だけは、ASCII の大文字小文字を区別しない。
//! - 履歴ファイルの読み込み（インポート）の内容は調べない（D-45 で拡大を見送った）。
//! - 費用は値の大きさに比例する（接頭辞の反復でも二次にならない。見つけた範囲の中の候補は数え直さず、連なりの終端は再利用する）。WASM での検査時間は、10MB に近い履歴で保存の直前の関門（直列化を除く検査）が約 50〜70ms、変更から保存の完了までの p95 が 340ms 以下だった（2026-10-21。`docs/testing.md` 5.186。基準端末 A では未測定）。
//!
//! 形の ID と名前は、秘密情報の検出の規則を識別する固定の名前であり、教材の知識ではない。

/// 検出する形。順序は [`SecretShape::ALL`] で固定する。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SecretShape {
    /// 接頭辞つきの英数字の連なりのトークン（ID と名前は下の表）。
    PrefixedToken,
    /// 接頭辞つきの細粒度トークン。
    FineGrainedToken,
    /// Slack 風のトークン。
    SlackToken,
    /// AWS 風のアクセスキー。
    AwsAccessKey,
    /// AWS 風のシークレットキー（名前つきの 40 文字）。
    AwsSecretKey,
    /// Stripe 風の本番キー。
    StripeLiveKey,
    /// Google 風の API キー。
    GoogleApiKey,
    /// JWT 風の 3 部の連なり。
    Jwt,
    /// 秘密鍵の枠。
    PrivateKey,
    /// 資格情報つきの接続文字列。
    ConnectionString,
    /// `sk-ant-` 接頭辞の API キー（SAFE-30 で足した形。名前は接頭辞の形で表す）。
    SkAntKey,
    /// `sk-proj-`・`sk-svcacct-`・`sk-admin-` 接頭辞の API キー（SAFE-30）。
    SkProjKey,
    /// GitLab 風のトークン（SAFE-30）。
    GitLabToken,
    /// npm 風のトークン（SAFE-30）。
    NpmToken,
    /// PyPI 風のトークン（SAFE-30）。
    PypiToken,
}

impl SecretShape {
    /// 固定の順序の全形。
    pub const ALL: [SecretShape; 15] = [
        SecretShape::PrefixedToken,
        SecretShape::FineGrainedToken,
        SecretShape::SlackToken,
        SecretShape::AwsAccessKey,
        SecretShape::AwsSecretKey,
        SecretShape::StripeLiveKey,
        SecretShape::GoogleApiKey,
        SecretShape::Jwt,
        SecretShape::PrivateKey,
        SecretShape::ConnectionString,
        SecretShape::SkAntKey,
        SecretShape::SkProjKey,
        SecretShape::GitLabToken,
        SecretShape::NpmToken,
        SecretShape::PypiToken,
    ];

    /// 形の ID（ASCII。試験データ `secret-shapes.json` の `expect` と同じ）。
    pub fn id(self) -> &'static str {
        match self {
            SecretShape::PrefixedToken => "github_token",
            SecretShape::FineGrainedToken => "github_pat",
            SecretShape::SlackToken => "slack_token",
            SecretShape::AwsAccessKey => "aws_access_key",
            SecretShape::AwsSecretKey => "aws_secret_key",
            SecretShape::StripeLiveKey => "stripe_live_key",
            SecretShape::GoogleApiKey => "google_api_key",
            SecretShape::Jwt => "jwt",
            SecretShape::PrivateKey => "private_key",
            SecretShape::ConnectionString => "connection_string",
            SecretShape::SkAntKey => "sk_ant_api_key",
            SecretShape::SkProjKey => "sk_proj_api_key",
            SecretShape::GitLabToken => "gitlab_token",
            SecretShape::NpmToken => "npm_token",
            SecretShape::PypiToken => "pypi_token",
        }
    }

    /// 画面に出す名前（値を含まない固定の文）。
    pub fn label(self) -> &'static str {
        match self {
            SecretShape::PrefixedToken => "GitHub 風のトークン",
            SecretShape::FineGrainedToken => "GitHub 風の細粒度トークン",
            SecretShape::SlackToken => "Slack 風のトークン",
            SecretShape::AwsAccessKey => "AWS 風のアクセスキー",
            SecretShape::AwsSecretKey => "AWS 風のシークレットキー",
            SecretShape::StripeLiveKey => "Stripe 風の本番キー",
            SecretShape::GoogleApiKey => "Google 風の API キー",
            SecretShape::Jwt => "JWT 風のトークン",
            SecretShape::PrivateKey => "秘密鍵",
            SecretShape::ConnectionString => "資格情報つきの接続文字列",
            SecretShape::SkAntKey => "sk-ant- 形式の API キー",
            SecretShape::SkProjKey => "sk-proj- 形式の API キー",
            SecretShape::GitLabToken => "GitLab 風のトークン",
            SecretShape::NpmToken => "npm 風のトークン",
            SecretShape::PypiToken => "PyPI 風のトークン",
        }
    }

    /// ID から形を引く。完全一致だけを受ける。
    pub fn from_id(id: &str) -> Option<SecretShape> {
        SecretShape::ALL.into_iter().find(|s| s.id() == id)
    }
}

/// 検出の 1 件。値も値への参照も持たない（`Debug` の派生でも値は出ない）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Finding {
    /// 見つかった形。
    pub shape: SecretShape,
    /// 見つかった範囲の FNV-1a 32 ビット。どこにも出力しない（確認済みの照合だけに使う）。
    pub fingerprint: u32,
}

fn fnv1a(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5_u32, |h, b| {
        (h ^ u32::from(*b)).wrapping_mul(0x0100_0193)
    })
}

fn is_alnum(b: u8) -> bool {
    b.is_ascii_alphanumeric()
}

fn is_b64url(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn is_upper_digit(b: u8) -> bool {
    b.is_ascii_uppercase() || b.is_ascii_digit()
}

fn is_b64(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'/' | b'+' | b'=')
}

/// `from` から `class` に当たる ASCII のバイトが続く長さ。`from` は文字の境界でなくてよい（ASCII だけを数える）。
fn run_len(s: &str, from: usize, class: fn(u8) -> bool) -> usize {
    s.as_bytes()
        .get(from..)
        .map_or(0, |rest| rest.iter().take_while(|b| class(**b)).count())
}

/// 直前に数えた連なり（`start..end`）の記憶。同じ連なりの中から始まる問い合わせは再走査せず終端を返す。
///
/// 接頭辞の反復（`-eyJ` を繰り返す入力など）で、各候補が残り全体を数え直す二次時間を避ける。
#[derive(Default)]
struct RunCache {
    start: usize,
    end: usize,
}

impl RunCache {
    /// [`run_len`] と同じ値を返す。`from` が記憶した連なりの中なら、その終端までの長さを再利用する。
    /// 1 つの記憶は 1 つの文字の種類にだけ使う（呼び出し側が種類ごとに別の記憶を持つ）。
    fn run_len(&mut self, s: &str, from: usize, class: fn(u8) -> bool) -> usize {
        if from >= self.start && from < self.end {
            return self.end - from;
        }
        let n = run_len(s, from, class);
        self.start = from;
        self.end = from + n;
        n
    }
}

/// `idx` の直前が英数字でないか（接頭辞が語の途中でないこと）。`idx` は `match_indices` が返す文字の境界。
fn boundary_before(s: &str, idx: usize) -> bool {
    s.get(..idx)
        .and_then(|head| head.chars().next_back())
        .is_none_or(|c| !c.is_ascii_alphanumeric())
}

fn push(out: &mut Vec<Finding>, shape: SecretShape, s: &str, start: usize, end: usize) {
    let span = s.as_bytes().get(start..end).unwrap_or_default();
    out.push(Finding {
        shape,
        fingerprint: fnv1a(span),
    });
}

/// 接頭辞のあとに、`class` の文字が `min` 個以上続くものを集める。
fn prefixed(
    out: &mut Vec<Finding>,
    s: &str,
    shape: SecretShape,
    prefixes: &[&str],
    class: fn(u8) -> bool,
    min: usize,
) {
    for prefix in prefixes {
        // 見つけた範囲の中にある同じ接頭辞は、その範囲に含まれるため数え直さない（指紋の計算も含めて線形にする）。
        let mut covered = 0;
        for (i, _) in s.match_indices(prefix) {
            if i < covered || !boundary_before(s, i) {
                continue;
            }
            let body = i + prefix.len();
            let n = run_len(s, body, class);
            if n >= min {
                push(out, shape, s, i, body + n);
                covered = body + n;
            }
        }
    }
}

fn skip_quote_space(s: &str, mut p: usize) -> usize {
    let b = s.as_bytes();
    while p < b.len() && matches!(b[p], b' ' | b'"' | b'\'') {
        p += 1;
    }
    p
}

fn scan_aws_secret(out: &mut Vec<Finding>, s: &str) {
    const NAME: &str = "aws_secret_access_key";
    // 名前は環境変数の `AWS_SECRET_ACCESS_KEY` などの大文字・混在も受ける。ASCII だけを小文字にした写しで位置を探し、
    // 長さと文字の境界が変わらないため、以後の判定と指紋は元の `s` で行う。
    let lower = s.to_ascii_lowercase();
    let mut covered = 0;
    for (i, _) in lower.match_indices(NAME) {
        if i < covered || !boundary_before(s, i) {
            continue;
        }
        let p = skip_quote_space(s, i + NAME.len());
        if !matches!(s.as_bytes().get(p), Some(b'=' | b':')) {
            continue;
        }
        let p = skip_quote_space(s, p + 1);
        let n = run_len(s, p, is_b64);
        if n >= 40 {
            push(out, SecretShape::AwsSecretKey, s, i, p + n);
            covered = p + n;
        }
    }
}

fn scan_jwt(out: &mut Vec<Finding>, s: &str) {
    const HEAD: &str = "eyJ";
    // 3 部それぞれの連なりを別々に記憶する。同じ連なりの中の候補が、同じ終端を何度も数え直さないため。
    let (mut c1, mut c2, mut c3) = (
        RunCache::default(),
        RunCache::default(),
        RunCache::default(),
    );
    let mut covered = 0;
    for (i, _) in s.match_indices(HEAD) {
        if i < covered || !boundary_before(s, i) {
            continue;
        }
        let b = s.as_bytes();
        let mut p = i + HEAD.len();
        let n1 = c1.run_len(s, p, is_b64url);
        if n1 < 8 {
            continue;
        }
        p += n1;
        if b.get(p) != Some(&b'.') || b.get(p + 1..p + 1 + HEAD.len()) != Some(HEAD.as_bytes()) {
            continue;
        }
        p += 1 + HEAD.len();
        let n2 = c2.run_len(s, p, is_b64url);
        if n2 < 8 {
            continue;
        }
        p += n2;
        if b.get(p) != Some(&b'.') {
            continue;
        }
        p += 1;
        let n3 = c3.run_len(s, p, is_b64url);
        if n3 >= 10 {
            push(out, SecretShape::Jwt, s, i, p + n3);
            covered = p + n3;
        }
    }
}

fn scan_private_key(out: &mut Vec<Finding>, s: &str) {
    const BEGIN: &str = "-----BEGIN ";
    const TAIL: &str = "PRIVATE KEY-----";
    const END: &str = "-----END ";
    // 種類の語（RSA・EC など）の上限。TAIL を探す範囲をここまでに限り、BEGIN 行の反復で残り全体を探さない。
    const KIND_MAX: usize = 16;
    // 見つけた範囲の中の BEGIN は数え直さない。
    let mut covered = 0;
    // END 行の探索の記憶（探し始めた位置、見つかった絶対位置。見つからなければ None）。
    let mut end_cache: Option<(usize, Option<usize>)> = None;
    for (i, _) in s.match_indices(BEGIN) {
        if i < covered {
            continue;
        }
        let from = i + BEGIN.len();
        let Some(rest) = s.as_bytes().get(from..) else {
            continue;
        };
        let window = rest
            .get(..rest.len().min(KIND_MAX + TAIL.len()))
            .unwrap_or_default();
        let Some(t) = window
            .windows(TAIL.len())
            .position(|w| w == TAIL.as_bytes())
        else {
            continue;
        };
        let kind_ok = window.get(..t).is_some_and(|k| {
            k.len() <= KIND_MAX && k.iter().all(|b| b.is_ascii_uppercase() || *b == b' ')
        });
        if !kind_ok {
            continue;
        }
        let after = from + t + TAIL.len();
        let found = match end_cache {
            Some((searched, pos)) if after >= searched && pos.is_none_or(|e| after <= e) => pos,
            _ => {
                let pos = s.get(after..).and_then(|r| r.find(END)).map(|e| after + e);
                end_cache = Some((after, pos));
                pos
            }
        };
        match found {
            Some(e) => {
                push(out, SecretShape::PrivateKey, s, i, e + END.len());
                covered = e + END.len();
            }
            // END 行が無い切り詰め・途中までの貼り付けも、鍵本体が続く前提として止める。
            None => {
                push(out, SecretShape::PrivateKey, s, i, after);
                covered = after;
            }
        }
    }
}

fn scan_connection(out: &mut Vec<Finding>, s: &str) {
    const SEP: &str = "://";
    let b = s.as_bytes();
    for (i, _) in s.match_indices(SEP) {
        // 直前の scheme（英字で始まる英数字・`+`・`.`・`-`）。
        let mut st = i;
        while st > 0
            && (b[st - 1].is_ascii_alphanumeric() || matches!(b[st - 1], b'+' | b'.' | b'-'))
        {
            st -= 1;
        }
        if st == i || !b[st].is_ascii_alphabetic() {
            continue;
        }
        let u = i + SEP.len();
        let user = run_len(s, u, |c| {
            !c.is_ascii_whitespace() && !matches!(c, b':' | b'/' | b'@')
        });
        if user == 0 || b.get(u + user) != Some(&b':') {
            continue;
        }
        let p = u + user + 1;
        let pass = run_len(s, p, |c| {
            !c.is_ascii_whitespace() && !matches!(c, b'/' | b'@')
        });
        if pass > 0 && b.get(p + pass) == Some(&b'@') {
            push(out, SecretShape::ConnectionString, s, st, p + pass + 1);
        }
    }
}

/// 文字列 1 本を調べる。
pub fn scan_text(s: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    let gh = ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"];
    prefixed(&mut out, s, SecretShape::PrefixedToken, &gh, is_alnum, 36);
    prefixed(
        &mut out,
        s,
        SecretShape::FineGrainedToken,
        &["github_pat_"],
        is_word,
        40,
    );
    let slack = ["xoxb-", "xoxa-", "xoxp-", "xoxr-", "xoxs-"];
    prefixed(&mut out, s, SecretShape::SlackToken, &slack, is_b64url, 10);
    prefixed(
        &mut out,
        s,
        SecretShape::AwsAccessKey,
        &["AKIA", "ASIA"],
        is_upper_digit,
        16,
    );
    scan_aws_secret(&mut out, s);
    prefixed(
        &mut out,
        s,
        SecretShape::StripeLiveKey,
        &["sk_live_", "rk_live_"],
        is_alnum,
        24,
    );
    prefixed(
        &mut out,
        s,
        SecretShape::GoogleApiKey,
        &["AIza"],
        is_b64url,
        35,
    );
    scan_jwt(&mut out, s);
    scan_private_key(&mut out, s);
    scan_connection(&mut out, s);
    scan_added_shapes(&mut out, s);
    out
}

/// SAFE-30（決定 D-45）で足した 5 形。接頭辞と文字種・長さだけで判定する。
///
/// 形の選定は 2026-10-21 にユーザーが確定した（決定 D-66。限界は `docs/known-gaps.md` の SAFE-30）。
/// 既存の 10 形と接頭辞が重ならないため、順序と指紋は既存の形に影響しない。
fn scan_added_shapes(out: &mut Vec<Finding>, s: &str) {
    prefixed(out, s, SecretShape::SkAntKey, &["sk-ant-"], is_b64url, 40);
    let proj = ["sk-proj-", "sk-svcacct-", "sk-admin-"];
    prefixed(out, s, SecretShape::SkProjKey, &proj, is_b64url, 40);
    prefixed(out, s, SecretShape::GitLabToken, &["glpat-"], is_b64url, 20);
    prefixed(out, s, SecretShape::NpmToken, &["npm_"], is_alnum, 36);
    let pypi = ["pypi-AgEIcHlwaS5vcmc"];
    prefixed(out, s, SecretShape::PypiToken, &pypi, is_b64url, 50);
}

/// JSON の値を調べる。文字列の値だけを見て、オブジェクトのキーは見ない。
pub fn scan_json_value(v: &serde_json::Value) -> Vec<Finding> {
    let mut out = Vec::new();
    collect(v, &mut out);
    out
}

fn collect(v: &serde_json::Value, out: &mut Vec<Finding>) {
    match v {
        serde_json::Value::String(s) => out.extend(scan_text(s)),
        serde_json::Value::Array(a) => a.iter().for_each(|x| collect(x, out)),
        serde_json::Value::Object(m) => m.values().for_each(|x| collect(x, out)),
        _ => {}
    }
}

/// JSON の文章を調べる。JSON として読めなければ、全体を 1 つの文字列として調べる（安全側）。
///
/// 改行などは JSON の文字列ではエスケープになるため、値として読み直してから調べる。
pub fn scan_json_text(text: &str) -> Vec<Finding> {
    match serde_json::from_str::<serde_json::Value>(text) {
        Ok(v) => scan_json_value(&v),
        Err(_) => scan_text(text),
    }
}

/// 検出から形だけを、重複なしで固定の順序に並べる。
pub fn shapes_of(findings: &[Finding]) -> Vec<SecretShape> {
    SecretShape::ALL
        .into_iter()
        .filter(|s| findings.iter().any(|f| f.shape == *s))
        .collect()
}
