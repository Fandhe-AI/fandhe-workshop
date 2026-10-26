//! 教材パックの値が秘密情報らしい形をしていないかの検査（`E-SECRET-SHAPE`。AUTHOR-18・D-81。TASK-420 1/2）。
//!
//! 呼び出し元は [`crate::validate::validate`] だけで、`compile` は行わない（`test` の関門は `validate` を呼ぶので効く）。
//! 教材パックは public の repo に載るため、本物と紛らわしい値の混入を公開の前に作成者へ知らせる。
//! 形の判定は core の `secret_scan`（受講中の保存の直前の警告と同じ 15 形）を再利用し、規則を二重に持たない。
//!
//! 出すのは形の ID（ASCII の固定名）だけで、値・値の一部・指紋・日本語の名前は診断のどこにも写さない。
//! 項目の ID は `safe_ident` を通ったものだけを写し、通らない、または秘密情報の形をしていれば位置（`#<n>`）にする。
//!
//! 限界（SAFE-30 と同じ。`docs/known-gaps.md`）:
//! - 固定の 15 形だけで、一般の秘密情報の検出率は保証しない。
//! - 読み込んだ後の値だけを調べる。YAML・TOML のコメント、map の鍵、参照されていない本文・練習データは調べない。
//!   これらは公開前の確認（LIC-12。`make check-internal-info-secrets`）が全ファイルを行ごとに照合して拾う。
//! - モジュールの `use` の引数と定義の字面はそれぞれ調べる。置き換えて合成した後に初めて現れる値は調べない。
//! - 社内固有の語・実在の名前との紛らわしさは調べない（D-81。公開前の確認の受け持ち）。

use std::collections::{BTreeMap, BTreeSet};

use fandhe_workshop_core::secret_scan::{scan_json_value, scan_text, shapes_of, Finding};
use serde::Serialize;
use serde_json::Value;

use crate::diag::{DiagCode, Diagnostic};
use crate::source::{safe_ident, LoadedPack};
use crate::validate::diag;

/// 重複（ファイル・項目・形）を 1 件にしながら診断を集める。
struct Collector {
    out: Vec<Diagnostic>,
    seen: BTreeSet<(String, String, &'static str)>,
}

impl Collector {
    fn add(&mut self, file: &str, item: &str, findings: &[Finding]) {
        // 重複排除の鍵は書き換え前のパス。伏せた名前で畳むと、別々のパスの診断が 1 件になる。
        let shown = file_name(file);
        for shape in shapes_of(findings) {
            let id = shape.id();
            if self.seen.insert((file.to_string(), item.to_string(), id)) {
                self.out.push(diag(
                    &shown,
                    item,
                    DiagCode::SecretShape,
                    format!(
                        "secret-like value ({id}) found; replace it with an obviously fictional value"
                    ),
                ));
            }
        }
    }

    fn text(&mut self, file: &str, item: &str, s: &str) {
        self.add(file, item, &scan_text(s));
    }

    fn json(&mut self, file: &str, item: &str, v: &Value) {
        self.add(file, item, &scan_json_value(v));
    }

    /// 直列化して調べる。直列化に失敗したら黙って通さず診断にする（fail-closed）。
    fn serialized<T: Serialize>(&mut self, file: &str, item: &str, v: &T) {
        match serde_json::to_value(v) {
            Ok(v) => self.json(file, item, &v),
            Err(_) => self.out.push(diag(
                &file_name(file),
                item,
                DiagCode::SecretShape,
                "could not scan for secret-like values",
            )),
        }
    }
}

/// 診断のファイル欄。パス自体が秘密情報の形をしていれば、値を写さない固定の語にする。
pub(crate) fn file_name(file: &str) -> String {
    if scan_text(file).is_empty() {
        file.to_string()
    } else {
        "<path-redacted>".to_string()
    }
}

/// 項目名。ID が安全に写せるときだけ `<kind>:<id>`、そうでなければ `<kind>:#<n>`。
pub(crate) fn item_name(kind: &str, id: &str, n: usize) -> String {
    // 秘密情報の形をした ID は、診断が値を写さないよう位置にする。
    if safe_ident(id) && scan_text(id).is_empty() {
        format!("{kind}:{id}")
    } else {
        format!("{kind}:#{n}")
    }
}

/// パックの本文・練習データ・構造定義・教材テストの値を調べる。診断の順序は走査順で固定。
pub(crate) fn check_secret_shapes(pack: &LoadedPack) -> Vec<Diagnostic> {
    let mut c = Collector {
        out: Vec::new(),
        seen: BTreeSet::new(),
    };
    let ws_file = pack.workshop_file.as_str();

    for (path, body) in &pack.bodies {
        c.text(path, "file", body);
    }
    for (n, (id, text)) in pack.fixture_texts.iter().enumerate() {
        let file = pack
            .workshop
            .fixtures
            .get(id)
            .map_or(ws_file, |f| f.path.as_str());
        c.text(file, &item_name("fixture", id, n), text);
    }

    for scope in &pack.scopes {
        let file = scope.file.as_str();
        let src = &scope.src;
        // 章・モジュールの ID も読み込み済みの文字列値なので走査する（診断は値を写さない）。
        c.text(file, "file", &src.id);
        c.text(file, "file", &src.title);
        for v in src.params.values() {
            c.text(file, "file", v);
        }
        // enum の候補も作成者が書いた字面なので、既定値と別に走査する。
        for decl in src.param_types.values() {
            for v in decl.values.iter().flatten() {
                c.text(file, "file", v);
            }
        }
        for (n, st) in src.starts.iter().enumerate() {
            c.serialized(file, &item_name("start", &st.id, n), st);
        }
        for (n, st) in src.steps.iter().enumerate() {
            c.serialized(file, &item_name("step", &st.id, n), st);
        }
    }

    let w = &pack.workshop;
    let tables: [(&str, &BTreeMap<String, Value>); 5] = [
        ("command", &w.commands),
        ("view", &w.views),
        ("hint", &w.hints),
        ("rejection", &w.rejections),
        ("clarification", &w.clarifications),
    ];
    for (kind, map) in tables {
        for (n, (id, v)) in map.iter().enumerate() {
            c.json(ws_file, &item_name(kind, id, n), v);
        }
    }
    c.serialized(ws_file, "pack", &w.pack);
    for v in &w.refs {
        c.json(ws_file, "pack", v);
    }
    for v in w.defaults.values() {
        c.text(ws_file, "pack", v);
    }

    if let (Some(tests), Some(file)) = (&pack.tests, &pack.tests_file) {
        for (kind, list) in [
            ("playthrough", &tests.playthroughs),
            ("error_input", &tests.error_inputs),
        ] {
            for (n, v) in list.iter().enumerate() {
                let id = v.get("id").and_then(Value::as_str).unwrap_or("");
                c.json(file, &item_name(kind, id, n), v);
            }
        }
        for v in tests.text_checks.iter().chain(&tests.guidance_checks) {
            c.json(file, "file", v);
        }
    }
    c.out
}
