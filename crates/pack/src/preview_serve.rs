//! `fandhe-pack preview` の配信（AUTHOR-22、TASK-429 2/3）。
//!
//! 呼び出し元は `cli::run_preview`。関門（`preview::gate`）を通った教材パックの IR と、同梱したエンジンの配信物
//! （`preview::EMBEDDED_WEB`）を、手元（127.0.0.1 だけ）の `/preview/` の下で GET・HEAD に限って配る。
//! 作成者の端末に Node も cargo も無い前提のため、標準ライブラリだけで書いた最小の HTTP/1.1 サーバである
//! （HTTP の crate は `scripts/check-no-egress.sh` の拒否の一覧にある）。
//!
//! 守ること（利用者が送るバイト列は信頼しない）
//! - 待ち受けは 127.0.0.1 の固定で、host・port を変える手段を持たない（`bind_loopback`）
//! - 経路は同梱の表との完全一致だけで引き、ファイルシステムを読まない（パストラバーサルが起きない）
//! - `Host` ヘッダが `127.0.0.1:<port>` と完全一致しない要求は 400（DNS rebinding で別のサイトから読まれないため）
//! - 誤りの本文は固定の文字列で、要求の経路・ヘッダを写さない。`nosniff` と拡張子の表の `Content-Type` を付ける
//! - リリースは `panic = abort` のため、接続のスレッドの panic は配信の全体を落とす。要求の解析では `unwrap`・
//!   範囲外の添字を使わず、誤りは応答の誤りか接続の切断で扱う
//! - 要求のヘッダの上限・読み書きのタイムアウト・同時の接続数の上限を置く。要求ごとのログは出さない

use std::borrow::Cow;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::preview::Asset;

/// 配信のサブパス。URL は `http://127.0.0.1:<port>/preview/`。
pub const SUBPATH: &str = "/preview/";
/// 要求のヘッダ全体の上限（バイト）。
pub const MAX_HEAD_BYTES: usize = 8 * 1024;
/// 同時に処理する接続の上限。超えた接続は 503 で閉じる。
pub const MAX_CONNECTIONS: usize = 32;
/// 読み書きのタイムアウト。
const IO_TIMEOUT: Duration = Duration::from_secs(10);

/// 配るファイルの表。経路（`/preview/` の後ろ）と完全一致で引く。
#[derive(Debug)]
pub struct Site {
    files: Vec<(&'static str, Cow<'static, [u8]>)>,
}

impl Site {
    /// 同梱の配信物に、関門の IR（`ir/main.json`）を足して表を作る。
    pub fn new(assets: &'static [Asset], ir: Vec<u8>) -> Self {
        let mut files: Vec<(&'static str, Cow<'static, [u8]>)> = assets
            .iter()
            .filter(|a| a.path != "ir/main.json")
            .map(|a| (a.path, Cow::Borrowed(a.body)))
            .collect();
        files.push(("ir/main.json", Cow::Owned(ir)));
        Site { files }
    }

    fn lookup(&self, rest: &str) -> Option<(&'static str, &[u8])> {
        let key = if rest.is_empty() { "index.html" } else { rest };
        self.files
            .iter()
            .find(|(p, _)| *p == key)
            .map(|(p, b)| (*p, b.as_ref()))
    }
}

/// 127.0.0.1 の空きポートで待ち受ける。ポートは OS に割り当てさせる。
pub fn bind_loopback() -> io::Result<TcpListener> {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
}

/// 組み立て済みの応答。
#[derive(Debug, PartialEq, Eq)]
pub struct Response {
    /// 状態コード。
    pub status: u16,
    /// `Content-Type`。
    pub content_type: &'static str,
    /// 本文。HEAD では送らない。
    pub body: Vec<u8>,
    /// 405 のときの `Allow`。
    pub allow: bool,
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

fn error(status: u16) -> Response {
    // 本文は固定の文字列。要求の内容は写さない。
    Response {
        status,
        content_type: "text/plain; charset=utf-8",
        body: format!("{} {}\n", status, reason(status)).into_bytes(),
        allow: status == 405,
    }
}

fn content_type_of(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "json" => "application/json",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// 経路を表の鍵に直す。`/preview/` の後ろだけを受け、危険な形は `None`（404）にする。
fn rest_of(target: &str) -> Option<&str> {
    let path = target.split(['?', '#']).next().unwrap_or("");
    let rest = path.strip_prefix(SUBPATH)?;
    if rest.contains(['%', '\\']) || rest.contains("//") {
        return None;
    }
    if rest.split('/').any(|seg| seg == "." || seg == "..") {
        return None;
    }
    Some(rest)
}

/// 要求のヘッダ部（最後の空行の前まで）から応答を決める。入出力に触れない純関数。
/// 返す `bool` は HEAD かどうか（本文を送らない）。
pub fn respond(site: &Site, head: &[u8], port: u16) -> (Response, bool) {
    let Ok(text) = std::str::from_utf8(head) else {
        return (error(400), false);
    };
    let mut lines = text.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return (error(400), false);
    };
    if version != "HTTP/1.1" && version != "HTTP/1.0" {
        return (error(400), false);
    }
    let is_head = method == "HEAD";
    // Host は完全一致だけを受ける（ちょうど 1 個）。
    let want = format!("127.0.0.1:{port}");
    let mut hosts = lines
        .filter_map(|l| l.split_once(':'))
        .filter(|(k, _)| k.eq_ignore_ascii_case("host"))
        .map(|(_, v)| v.trim());
    match (hosts.next(), hosts.next()) {
        (Some(h), None) if h == want => {}
        _ => return (error(400), is_head),
    }
    if method != "GET" && method != "HEAD" {
        return (error(405), false);
    }
    if !target.starts_with('/') {
        return (error(404), is_head);
    }
    match rest_of(target).and_then(|r| site.lookup(r)) {
        Some((path, body)) => (
            Response {
                status: 200,
                content_type: content_type_of(path),
                body: body.to_vec(),
                allow: false,
            },
            is_head,
        ),
        None => (error(404), is_head),
    }
}

fn write_response(stream: &mut TcpStream, resp: &Response, head_only: bool) -> io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n",
        resp.status,
        reason(resp.status),
        resp.content_type,
        resp.body.len()
    );
    if resp.allow {
        head.push_str("Allow: GET, HEAD\r\n");
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    if !head_only {
        stream.write_all(&resp.body)?;
    }
    stream.flush()
}

/// 要求のヘッダを上限まで読む。`Ok(None)` は上限超え、`Err` は切断・タイムアウト。
fn read_head(stream: &mut TcpStream) -> io::Result<Option<Vec<u8>>> {
    let deadline = Instant::now() + IO_TIMEOUT;
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            if pos > MAX_HEAD_BYTES {
                return Ok(None);
            }
            buf.truncate(pos);
            return Ok(Some(buf));
        }
        if buf.len() > MAX_HEAD_BYTES {
            return Ok(None);
        }
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        buf.extend_from_slice(chunk.get(..n).unwrap_or(&[]));
    }
}

fn handle(mut stream: TcpStream, site: &Site, port: u16) -> io::Result<()> {
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let (resp, head_only) = match read_head(&mut stream)? {
        None => (error(431), false),
        Some(head) => respond(site, &head, port),
    };
    write_response(&mut stream, &resp, head_only)?;
    // 読み残しがあるまま閉じると RST で応答が相手に届かないことがある（上限超えの 431 など）。
    // 送り終えたことを伝えてから、短い時間だけ残りを捨てる。
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
    let mut sink = [0u8; 4096];
    let mut left = 64 * 1024usize;
    while left > 0 {
        match stream.read(&mut sink) {
            Ok(0) | Err(_) => break,
            Ok(n) => left = left.saturating_sub(n),
        }
    }
    Ok(())
}

/// 接続の数え上げ。drop で必ず戻す。
struct Slot(Arc<AtomicUsize>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// 受け付けを続ける。止まるのはプロセスが終わるときだけ（Ctrl+C。シグナルの処理は書かない）。
/// 接続ごとにスレッドを立て、同時の上限を超えた接続は 503 で閉じる。
pub fn serve(listener: TcpListener, site: Arc<Site>) -> ! {
    let port = listener.local_addr().map_or(0, |a| a.port());
    let active = Arc::new(AtomicUsize::new(0));
    loop {
        let mut stream = match listener.accept() {
            Ok((s, _)) => s,
            Err(_) => {
                // 一時的な失敗（fd 不足など）で空回りしない。
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        if active.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            active.fetch_sub(1, Ordering::SeqCst);
            let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
            let _ = write_response(&mut stream, &error(503), false);
            continue;
        }
        let slot = Slot(Arc::clone(&active));
        let site = Arc::clone(&site);
        let spawned = std::thread::Builder::new().spawn(move || {
            let _slot = slot;
            let _ = handle(stream, &site, port);
        });
        if spawned.is_err() {
            // スレッドを立てられなかった。クロージャごと落ちて Slot も戻る。
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static ASSETS: [Asset; 2] = [
        Asset {
            path: "index.html",
            body: b"<html></html>",
        },
        Asset {
            path: "wasm/x_bg.wasm",
            body: b"\0asm",
        },
    ];

    fn site() -> Site {
        Site::new(&ASSETS, b"{}\n".to_vec())
    }

    fn req(method: &str, target: &str) -> Vec<u8> {
        format!("{method} {target} HTTP/1.1\r\nHost: 127.0.0.1:9\r\nAccept: */*").into_bytes()
    }

    #[test]
    fn serves_index_and_ir() {
        let s = site();
        for t in ["/preview/", "/preview/index.html", "/preview/?start=a#x"] {
            let (r, h) = respond(&s, &req("GET", t), 9);
            assert_eq!((r.status, h), (200, false), "{t}");
            assert_eq!(r.body, b"<html></html>");
        }
        let (r, _) = respond(&s, &req("GET", "/preview/ir/main.json"), 9);
        assert_eq!(r.content_type, "application/json");
        assert_eq!(r.body, b"{}\n");
        let (r, _) = respond(&s, &req("GET", "/preview/wasm/x_bg.wasm"), 9);
        assert_eq!(r.content_type, "application/wasm");
    }

    #[test]
    fn rejects_bad_paths_without_echo() {
        let s = site();
        for t in [
            "/",
            "/preview",
            "/preview/..",
            "/preview/../index.html",
            "/preview/%2e%2e/index.html",
            "/preview//index.html",
            "/preview/wasm\\x",
            "/preview/missing",
            "http://127.0.0.1:9/preview/",
            "preview/",
        ] {
            let (r, _) = respond(&s, &req("GET", t), 9);
            assert_eq!(r.status, 404, "{t}");
            assert!(!String::from_utf8_lossy(&r.body).contains("preview"), "{t}");
        }
    }

    #[test]
    fn method_and_host_rules() {
        let s = site();
        let (r, _) = respond(&s, &req("POST", "/preview/"), 9);
        assert!(r.status == 405 && r.allow);
        let (r, h) = respond(&s, &req("HEAD", "/preview/"), 9);
        assert!(r.status == 200 && h);
        for head in [
            "GET /preview/ HTTP/1.1\r\nHost: localhost:9",
            "GET /preview/ HTTP/1.1\r\nHost: 127.0.0.1:8",
            "GET /preview/ HTTP/1.1\r\nAccept: */*",
            "GET /preview/ HTTP/1.1\r\nHost: 127.0.0.1:9\r\nHost: 127.0.0.1:9",
            "GET /preview/ HTTP/1.1\r\nHost: evil.example",
        ] {
            let (r, _) = respond(&s, head.as_bytes(), 9);
            assert_eq!(r.status, 400, "{head}");
        }
    }

    #[test]
    fn malformed_requests_do_not_panic() {
        let s = site();
        for head in [
            &b""[..],
            b"GET",
            b"GET /preview/",
            b"GET /preview/ HTTP/2 extra",
            b"\xff\xfe\r\n",
            b"GET /preview/ HTTP/9.9\r\nHost: 127.0.0.1:9",
        ] {
            let (r, _) = respond(&s, head, 9);
            assert!(r.status >= 400);
        }
    }

    #[test]
    fn bind_is_loopback() {
        let l = bind_loopback().unwrap();
        let a = l.local_addr().unwrap();
        assert_eq!(a.ip(), std::net::IpAddr::V4(Ipv4Addr::LOCALHOST));
    }
}
