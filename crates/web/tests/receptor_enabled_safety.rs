//! 受け口を有効にしても安全性が変わらないことの試験（TASK-191 2/2。EXT-10・D-53。SAFE-1〜SAFE-7 の native 側）。
//!
//! 受け口の実体（有効を読んで振る舞いを変える処理）はまだ無い。そのため安全性が変わらない根拠を 2 つに分ける。
//! (1) 静的: 関門の結果（`Activation::Enabled`）を読む識別子の出現を、定義と試験の口の固定の件数に限る
//!     （新しい消費者を足すと失敗する。足すときは件数と `docs/extension-point-procedure.md` の手順を見直す）。
//! (2) 挙動: 有効にしたホストと既定のホストへ、入力欄からの注入（E5 の 49 件）を同じ手順で送り、判定・ログ・状態・書き出しが
//!     完全に一致する。ブラウザでの同じ比較は `tools/browser/receptor-safety.test.mjs`（`docs/testing.md` 5.383）。
//!
//! 試験では `unwrap` を使ってよい。

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::{count_ident, strip_comments, web_src_sources};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::receptor_gate::{Activation, ReceptorId, ReceptorSettings};
use fandhe_workshop_core::session::{RunState, Session, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;

const R: ReceptorId = ReceptorId::new("test-receptor");
const EXPECTED_E5: usize = 49;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

// ---- (1) 静的 ----

/// 関門の結果を読む識別子。コメントを除いたソースでの出現を数える。
const GATE_READERS: [&str; 4] = [
    "receptor_activation",
    "is_enabled",
    "Activation::Enabled",
    "receptor_gate::activation",
];

fn core_sources() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../core/src");
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "rs") {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            out.push((name, std::fs::read_to_string(&p).unwrap()));
        }
    }
    out.sort();
    out
}

#[test]
fn ext10_web_has_no_gate_consumer_beyond_the_fixed_sites() {
    let files = web_src_sources();
    // (識別子, ファイル, 件数)。ここに無い組は 0 件でなければならない。
    let allowed: [(&str, &str, usize); 4] = [
        // 定義（ホストのアクセサ）とその中の関門の呼び出し。
        ("receptor_activation", "host.rs", 1),
        ("receptor_gate::activation", "host.rs", 1),
        // 試験の設定の読み取りの錨（`debug_receptor_activation` の経路）。
        ("receptor_activation", "glue.rs", 1),
        ("Activation::Enabled", "test_hooks.rs", 1),
    ];
    let mut seen = 0;
    for needle in GATE_READERS {
        for (name, src) in &files {
            let want = allowed
                .iter()
                .find(|(n, f, _)| *n == needle && f == name)
                .map_or(0, |a| a.2);
            let got = count_ident(&strip_comments(src), needle);
            assert_eq!(got, want, "{name}: {needle} の出現");
            seen += got;
        }
    }
    // 空振り防止: 固定の 4 件が実際に見つかっている。
    assert_eq!(seen, 4);
    // 読み取りの錨は試験の設定の口（glue の定義と test_hooks の呼び出し）だけ。
    for (name, src) in &files {
        let want = usize::from(name == "glue.rs" || name == "test_hooks.rs");
        let got = count_ident(&strip_comments(src), "receptor_activation_probe");
        assert_eq!(got, want, "{name}: receptor_activation_probe");
    }
}

#[test]
fn ext10_core_gate_is_referenced_only_by_itself() {
    let files = core_sources();
    assert!(files.iter().any(|(n, _)| n == "receptor_gate.rs"));
    for (name, src) in &files {
        if name == "receptor_gate.rs" {
            continue;
        }
        let code = strip_comments(src);
        for needle in GATE_READERS {
            assert_eq!(count_ident(&code, needle), 0, "{name}: {needle}");
        }
        let want = usize::from(name == "lib.rs");
        assert_eq!(count_ident(&code, "receptor_gate"), want, "{name}");
    }
}

// ---- (2) 挙動 ----

fn ir() -> Ir {
    let dir = root().join("testdata/input-field-ime/test-data/ch1-pack");
    let bytes = compile_dir(&dir).unwrap_or_else(|d| panic!("compile: {d:?}"));
    load_ir(&bytes).expect("load_ir")
}

fn st14_state() -> SessionState {
    let uid = StepUid("main/st-1-4".to_owned());
    let mut slice = Slice::default();
    slice.status.insert(uid.clone(), StepStatus::Entered);
    let mut slices = BTreeMap::new();
    slices.insert(uid.root(), slice);
    SessionState {
        run: RunState {
            current: Some(uid),
            slices,
            ..RunState::default()
        },
        ..SessionState::default()
    }
}

fn host(enabled: bool) -> EngineHost {
    let mut h =
        EngineHost::with_session(Session::from_state(ir(), st14_state()), standard_registry())
            .unwrap();
    if enabled {
        h = h.with_receptor_settings(ReceptorSettings::default().allow(R));
        h.grant_receptor_consent(R);
        assert_eq!(h.receptor_activation(R), Activation::Enabled);
    } else {
        assert_ne!(h.receptor_activation(R), Activation::Enabled);
    }
    h
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

fn send(h: &mut EngineHost, text: &str) {
    h.update(act("draft", text));
    h.update(act("submit", ""));
}

/// 観察。ログ・状態・書き出しの全体を 1 つの文字列にする（比べるだけで出力しない）。
fn observe(h: &EngineHost) -> String {
    format!(
        "{:?}\n{}\n{:?}",
        h.model().session().unwrap().log(),
        serde_json::to_string(h.model().session().unwrap().state()).unwrap(),
        h.export_live()
    )
}

/// `\u{HEX}` を 1 文字へ展開する（`tools/browser/xss-cases.mjs` と同じ規則）。不正は panic（fail-closed）。
fn expand(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("\\u{") {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 3..];
        let end = tail.find('}').expect("\\u{ の閉じ");
        let hex = &tail[..end];
        assert!((1..=6).contains(&hex.len()), "桁数");
        let cp = u32::from_str_radix(hex, 16).unwrap();
        out.push(char::from_u32(cp).expect("コードポイント"));
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

/// testdata の E5（入力欄）の 49 件のペイロード。件数が違えば panic（0 件のまま通る空振りを防ぐ）。
fn e5_payloads() -> Vec<String> {
    let path = root().join("testdata/untrusted-input-safety/test-data/xss-cases.md");
    let text = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    let mut out = Vec::new();
    for part in text.split("\n### XSS-").skip(1) {
        let e5 = part
            .lines()
            .any(|l| l.starts_with("- 入り口:") && l.split([',', ':']).any(|t| t.trim() == "E5"));
        if !e5 {
            continue;
        }
        let lines: Vec<&str> = part.lines().collect();
        let fences: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.starts_with("~~~~~~"))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(fences.len(), 2, "ペイロードの区画");
        let payload = expand(&lines[fences[0] + 1..fences[1]].join("\n"));
        assert!(!payload.is_empty());
        out.push(payload);
    }
    assert_eq!(out.len(), EXPECTED_E5, "E5 の件数");
    out
}

#[test]
fn ext10_enabling_does_not_change_judgment_log_state_or_export() {
    let payloads = e5_payloads();
    for shape in 0..3 {
        let mut off = host(false);
        let mut on = host(true);
        let empty = observe(&off);
        for p in &payloads {
            let text = match shape {
                0 => p.clone(),
                1 => format!("/{p}"),
                _ => format!("/new-idea {p}"),
            };
            send(&mut off, &text);
            send(&mut on, &text);
            assert_eq!(
                observe(&off),
                observe(&on),
                "形 {shape}: 有効で結果が変わった"
            );
        }
        // 観察が空振りでない（ログが伸びている）。
        assert_ne!(observe(&off), empty, "形 {shape}: 観察が空振り");
        // 有効の状態は操作で変わらない（同意は操作・入力から作れない）。
        assert_eq!(on.receptor_activation(R), Activation::Enabled);
        assert_ne!(off.receptor_activation(R), Activation::Enabled);
    }
}
