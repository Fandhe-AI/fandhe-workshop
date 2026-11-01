//! 同梱つき（feature `embed-web`）の `fandhe-pack preview` を実行ファイルで確かめる（AUTHOR-22、TASK-429 2/3）。
//!
//! `scripts/test-browser.sh` が `cargo test --features embed-web --test preview_embedded` で流す
//! （同梱の元 `dist/web-embed/` が要る）。URL が標準出力のパイプ越しに読めること（`flush`）、配った 6 ファイルが同梱の
//! 元とバイト単位で一致すること、IR が `compile` の出力と一致すること、待ち受けが 127.0.0.1 だけであることを確かめる。
#![cfg(feature = "embed-web")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

mod common;
use common::minimal;

struct Guard(Child);
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn http_get(port: u16, path: &str) -> (String, Vec<u8>) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(s, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").unwrap();
    let mut out = Vec::new();
    s.read_to_end(&mut out).unwrap();
    let pos = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    (
        String::from_utf8_lossy(&out[..pos]).into_owned(),
        out[pos + 4..].to_vec(),
    )
}

#[test]
fn author22_embedded_preview_prints_url_and_serves_bundle() {
    let pack = minimal();
    let mut child = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .arg("preview")
        .arg(&pack.0)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let _guard = Guard(child);
    let mut lines = BufReader::new(stdout).lines();
    let ok = lines.next().unwrap().unwrap();
    assert!(ok.starts_with("OK: "), "{ok}");
    let url = lines.next().unwrap().unwrap();
    let rest = url.strip_prefix("URL: http://127.0.0.1:").expect(&url);
    let port: u16 = rest.strip_suffix("/preview/").expect(&url).parse().unwrap();

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist/web-embed");
    for f in [
        "index.html",
        "app.js",
        "app.css",
        "icon.svg",
        "wasm/fandhe_workshop_web.js",
        "wasm/fandhe_workshop_web_bg.wasm",
    ] {
        let (h, b) = http_get(port, &format!("/preview/{f}"));
        assert!(h.starts_with("HTTP/1.1 200 "), "{f}: {h}");
        assert_eq!(b, std::fs::read(root.join(f)).unwrap(), "{f}");
    }
    let c = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .arg("compile")
        .arg(&pack.0)
        .output()
        .unwrap();
    let (_, ir) = http_get(port, "/preview/ir/main.json");
    assert_eq!(ir, c.stdout);

    // 127.0.0.1 以外の自分のアドレス（Linux の 127.0.0.2）には開かない。
    if cfg!(target_os = "linux") {
        assert!(TcpStream::connect(("127.0.0.2", port)).is_err());
    }
}
