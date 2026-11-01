//! `fandhe-pack preview` の配信の適合試験（AUTHOR-22、TASK-429 2/3）。合成の同梱の表で、プロセス内の配信を確かめる。
//!
//! `cli::run` を診断 0 件の教材パックに対して呼ばない（呼ぶと配信に入って戻らない）。配信は `preview_serve::serve`
//! を別スレッドで動かし、127.0.0.1 の実ソケットに要求を送る。

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::sync::Arc;

use fandhe_workshop_pack::preview::Asset;
use fandhe_workshop_pack::preview_serve::{bind_loopback, serve, Site, MAX_HEAD_BYTES};

static ASSETS: [Asset; 3] = [
    Asset {
        path: "index.html",
        body: b"<!doctype html><title>t</title>",
    },
    Asset {
        path: "app.js",
        body: b"console.log(1)",
    },
    Asset {
        path: "wasm/x_bg.wasm",
        body: b"\0asm\x01\0\0\0",
    },
];
const IR: &[u8] = b"{\"ir\":true}\n";

fn start() -> u16 {
    let l = bind_loopback().unwrap();
    let a = l.local_addr().unwrap();
    assert_eq!(a.ip(), std::net::IpAddr::V4(Ipv4Addr::LOCALHOST));
    let site = Arc::new(Site::new(&ASSETS, IR.to_vec()));
    std::thread::spawn(move || serve(l, site));
    a.port()
}

fn raw(port: u16, bytes: &[u8]) -> Vec<u8> {
    let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    s.write_all(bytes).unwrap();
    // 送り終えたことを伝える（途中で切れた要求でも、配信側が待たされずに閉じる）。
    let _ = s.shutdown(std::net::Shutdown::Write);
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    out
}

fn get(port: u16, method: &str, target: &str) -> (String, Vec<u8>) {
    let req = format!("{method} {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n");
    split(raw(port, req.as_bytes()))
}

fn split(resp: Vec<u8>) -> (String, Vec<u8>) {
    let pos = resp
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("response has a head");
    (
        String::from_utf8_lossy(&resp[..pos]).into_owned(),
        resp[pos + 4..].to_vec(),
    )
}

#[test]
fn author22_serves_assets_and_ir_byte_for_byte() {
    let port = start();
    for t in ["/preview/", "/preview/index.html", "/preview/?start=x"] {
        let (h, b) = get(port, "GET", t);
        assert!(h.starts_with("HTTP/1.1 200 "), "{h}");
        assert!(h.contains("Content-Type: text/html; charset=utf-8"));
        assert_eq!(b, ASSETS[0].body);
    }
    let (h, b) = get(port, "GET", "/preview/wasm/x_bg.wasm");
    assert!(h.contains("Content-Type: application/wasm"), "{h}");
    assert_eq!(b, ASSETS[2].body);
    let (h, b) = get(port, "GET", "/preview/ir/main.json");
    assert!(h.contains("Content-Type: application/json"));
    assert!(h.contains("Cache-Control: no-store"));
    assert!(h.contains("X-Content-Type-Options: nosniff"));
    assert!(h.contains(&format!("Content-Length: {}", IR.len())));
    assert_eq!(b, IR);
}

#[test]
fn author22_head_has_no_body_and_other_methods_are_405() {
    let port = start();
    let (h, b) = get(port, "HEAD", "/preview/");
    assert!(h.starts_with("HTTP/1.1 200 "));
    assert!(b.is_empty());
    let (h, _) = get(port, "POST", "/preview/");
    assert!(
        h.starts_with("HTTP/1.1 405 ") && h.contains("Allow: GET, HEAD"),
        "{h}"
    );
}

#[test]
fn author22_unknown_and_traversal_paths_are_404_without_echo() {
    let port = start();
    for t in [
        "/",
        "/preview",
        "/preview/nope",
        "/preview/../app.js",
        "/preview/%2e%2e/app.js",
        "/preview//app.js",
        "/preview/wasm\\x",
    ] {
        let (h, b) = get(port, "GET", t);
        assert!(h.starts_with("HTTP/1.1 404 "), "{t}: {h}");
        assert!(!String::from_utf8_lossy(&b).contains("nope"));
    }
}

#[test]
fn author22_wrong_or_missing_host_is_400() {
    let port = start();
    for req in [
        "GET /preview/ HTTP/1.1\r\nHost: evil.example\r\n\r\n".to_string(),
        "GET /preview/ HTTP/1.1\r\nHost: localhost\r\n\r\n".to_string(),
        "GET /preview/ HTTP/1.1\r\n\r\n".to_string(),
    ] {
        let (h, _) = split(raw(port, req.as_bytes()));
        assert!(h.starts_with("HTTP/1.1 400 "), "{h}");
    }
}

#[test]
fn author22_oversized_head_is_431_and_garbage_does_not_stop_the_server() {
    let port = start();
    let big = format!(
        "GET /preview/ HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX: {}\r\n\r\n",
        "a".repeat(MAX_HEAD_BYTES + 100)
    );
    let (h, _) = split(raw(port, big.as_bytes()));
    assert!(h.starts_with("HTTP/1.1 431 "), "{h}");
    // 途中で切れた要求・不正なバイト列の後も応える。
    let _ = raw(port, b"GET /preview/ HT");
    let _ = raw(port, b"\xff\xfe\x00\r\n\r\n");
    let (h, _) = get(port, "GET", "/preview/");
    assert!(h.starts_with("HTTP/1.1 200 "));
}
