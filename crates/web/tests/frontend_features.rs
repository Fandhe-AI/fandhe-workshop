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

/// `web-sys` の feature の一覧と用途（FEND-20・TASK-362 2/2。2026-10-20 時点で 24 個）。
///
/// FEND-20 は web-sys の feature の数を記録し、増えたら理由を求める。個数だけでなく中身も比べるので、
/// 追加にも入れ替えにも気づける。足すときは、ここに用途を書き、`docs/testing.md` 5.166 の表を同時に更新する。
/// spec の「9 個」は決定時点の PoC の値で、その後に保存先（10 個）・書き出し（3 個）などが加わった（5.92・5.166）。
const WEB_SYS_FEATURES: [(&str, &str); 24] = [
    (
        "Document",
        "キー入力等: root の外の Esc の keydown の登録先",
    ),
    (
        "Node",
        "キー入力等: target が root の中かの判定（contains）",
    ),
    ("Element", "キー入力等: 要素の取得と属性の書き込み"),
    ("Event", "キー入力等: イベントの受け取り"),
    ("EventTarget", "キー入力等: リスナーの登録"),
    ("HtmlElement", "キー入力等: click"),
    ("HtmlTextAreaElement", "キー入力等: textarea の値の反映"),
    ("HtmlLiElement", "キー入力等: 左の行の識別"),
    ("InputEvent", "キー入力等: 変換中の印"),
    ("KeyboardEvent", "キー入力等: keydown の受け取り"),
    ("Window", "キー入力等: タイマー・保存先・離脱時の保存の入口"),
    ("Location", "保存先: 起動時の URL"),
    ("Storage", "保存先: localStorage"),
    ("DomException", "保存先: 失敗の種類の読み取り"),
    ("IdbFactory", "保存先: IndexedDB を開く"),
    ("IdbOpenDbRequest", "保存先: IndexedDB の open の要求"),
    ("IdbRequest", "保存先: IndexedDB の要求"),
    ("IdbDatabase", "保存先: IndexedDB の接続"),
    ("IdbTransaction", "保存先: IndexedDB のトランザクション"),
    ("IdbTransactionMode", "保存先: トランザクションの種類"),
    ("IdbObjectStore", "保存先: IndexedDB のオブジェクトストア"),
    ("Blob", "書き出し: 履歴のファイル"),
    ("BlobPropertyBag", "書き出し: ファイルの種類の指定"),
    ("Url", "書き出し: object URL"),
];

#[test]
fn web_sys_は_既定_feature_なしで宣言している() {
    let root = parse(ROOT_MANIFEST);
    let dep = &root["workspace"]["dependencies"]["web-sys"];
    assert_eq!(
        dep.get("default-features").and_then(|v| v.as_bool()),
        Some(false),
        "web-sys は default-features = false が必要: {dep}"
    );
}

#[test]
fn web_sys_の_feature_は一覧と一致する() {
    let root = parse(ROOT_MANIFEST);
    let mut actual: Vec<&str> = root["workspace"]["dependencies"]["web-sys"]["features"]
        .as_array()
        .expect("web-sys の features")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    actual.sort_unstable();
    let mut expected: Vec<&str> = WEB_SYS_FEATURES.iter().map(|(f, _)| *f).collect();
    expected.sort_unstable();
    let rule = "web-sys の feature が変わった（FEND-20）。足す・外すときは WEB_SYS_FEATURES に用途（理由）を書き、docs/testing.md 5.166 の表を同時に更新する";
    assert_eq!(actual, expected, "{rule}");
    for (f, purpose) in WEB_SYS_FEATURES {
        assert!(!purpose.is_empty(), "{f}: 用途が空");
    }
}

#[test]
fn web_の_web_sys_依存は_workspace_の宣言をそのまま使う() {
    let web = parse(WEB_MANIFEST);
    // web-sys は wasm32 専用の依存（`[target.'cfg(target_arch = "wasm32")'.dependencies]`）にだけ置く。
    assert!(web["dependencies"].get("web-sys").is_none());
    let dep = web["target"]["cfg(target_arch = \"wasm32\")"]["dependencies"]["web-sys"]
        .as_table()
        .expect("web-sys は表形式であること");
    assert_eq!(dep.get("workspace").and_then(|v| v.as_bool()), Some(true));
    assert!(
        !dep.contains_key("features"),
        "crate 側で feature を足さない（一覧はルートの 1 か所で固定する）"
    );
    assert!(!dep.contains_key("default-features"));
}
