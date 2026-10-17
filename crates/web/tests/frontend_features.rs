//! 表示層の基盤（fandhe-frontend）の採用版と feature の宣言を守る試験（TASK-346 / FEND-1）。
//!
//! `fandhe-frontend-wasm-full` は `action-keydown` が無いと、上流の `Runtime::mount` が keydown の配線を
//! 登録しない。`data-action-keydown` と `data-keys` の属性は、エラーも出さずに効かなくなる。
//! 上流の公開 API は feature の有無を native から観測できないため、ここでは manifest の宣言を直接検査し、
//! 動作は `tools/browser/keydown-action.test.mjs` が確かめる。
//!
//! 読み込みは `CARGO_MANIFEST_DIR` からの相対パスで行う。変異注入（`scripts/check-mutations.sh`）の
//! 一時コピーでも、コピー側のファイルが読まれる。

use toml::Table;

const ROOT_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml"));
const WEB_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
const LOCK: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock"));

/// FEND-1 の採用版（D-30）。版を上げるときは、この表と `docs/testing.md` 第 6 節を同時に更新する（TASK-101）。
const ADOPTED: [(&str, &str); 6] = [
    ("fandhe-frontend-core", "0.4.3"),
    ("fandhe-frontend-interactive", "0.2.7"),
    ("fandhe-frontend-wasm-client", "0.6.1"),
    ("fandhe-frontend-wasm-full", "0.46.0"),
    ("fandhe-frontend-headless-ui", "0.69.2"),
    ("fandhe-frontend-wireframe-ui", "0.52.0"),
];

fn parse(src: &str) -> Table {
    src.parse::<Table>().expect("TOML として読めること")
}

#[test]
fn wasm_full_は_既定_feature_なしで_action_keydown_を明示している() {
    let root = parse(ROOT_MANIFEST);
    let dep = &root["workspace"]["dependencies"]["fandhe-frontend-wasm-full"];
    assert_eq!(
        dep.get("default-features").and_then(|v| v.as_bool()),
        Some(false),
        "default-features = false が必要: {dep}"
    );
    let features: Vec<&str> = dep
        .get("features")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    assert!(
        features.contains(&"action-keydown"),
        "action-keydown が無いと data-action-keydown が黙って効かない（FEND-1）: {dep}"
    );
}

#[test]
fn web_の_fandhe_frontend_依存は_workspace_の宣言をそのまま使う() {
    let web = parse(WEB_MANIFEST);
    let deps = web["dependencies"].as_table().expect("dependencies");
    let mut seen = 0;
    for (name, dep) in deps
        .iter()
        .filter(|(n, _)| n.starts_with("fandhe-frontend-"))
    {
        seen += 1;
        let t = dep
            .as_table()
            .unwrap_or_else(|| panic!("{name} は表形式であること"));
        assert_eq!(
            t.get("workspace").and_then(|v| v.as_bool()),
            Some(true),
            "{name}"
        );
        assert!(
            !t.contains_key("features"),
            "{name} は features を上書きしない"
        );
        assert!(
            !t.contains_key("default-features"),
            "{name} は default-features を上書きしない"
        );
    }
    assert_eq!(seen, 6, "fandhe-frontend-* の依存は 6 件");
}

#[test]
fn cargo_lock_の解決済みの版が採用版と一致する() {
    let lock = parse(LOCK);
    let packages = lock["package"].as_array().expect("package");
    for (name, version) in ADOPTED {
        let found: Vec<&str> = packages
            .iter()
            .filter(|p| p["name"].as_str() == Some(name))
            .filter_map(|p| p["version"].as_str())
            .collect();
        assert_eq!(
            found,
            vec![version],
            "{name} の解決済みの版（版上げ時は ADOPTED を更新する）"
        );
    }
}
