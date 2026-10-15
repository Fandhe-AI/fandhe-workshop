//! Step の本文と練習データの内容を、ビルド時に変えた安全な木（方式 (c)、IR 版 2）の型。
//!
//! pack の `markdown` が Markdown からこの木を作って IR に載せ、core の `load_ir` が読み、web が画面の部品へ
//! 変える。3 者が共有する型なので contract に置く（`docs/architecture.md` 第 6 節の判定規則 1）。
//! 実行時に Markdown は解釈しない。IR に載った木は未信頼の入力として読み込み、語彙の外の種別・未知の欄は
//! `deny_unknown_fields` と内部タグの `enum` で拒否する。HTML を表す種別は持たない（生の HTML は変換器が
//! テキストにする）。
//!
//! 出力に `null` を入れない（`Block::Code` の `lang` は無いとき書き出さない。IR 版 2 の golden が `null` を
//! 含まない形を固定する）。木の版は逆直列化の段で検査する（[`Doc::version`]）。描画の深さの上限と URL の
//! 検査は、描画側（web の `doc_view`）が受け持つ。

use serde::{Deserialize, Serialize};

/// 木の版。実行時に読む IR は未信頼の入力なので、版も検査する（逆直列化の段で `Doc` が拒否する）。
pub const TREE_VERSION: u32 = 1;

/// 変換器が木に入れる入れ子の深さの上限。超えた部分は構造を捨て、テキストだけの段落にする。
///
/// 値は、IR の JSON の入れ子の上限（`limits::MAX_JSON_DEPTH` = 64）から導く。木の 1 段は JSON では最大 4 段に
/// 当たる（表: ブロック・`rows`・行・セル・インライン、リスト: ブロック・`items`・項目・ブロック）。木が最も深く
/// 埋まる位置は `Ir` → `nodes` → `Node` → `body_levels_doc` → `Doc` → `blocks` → `Block` で 7 段目から始まる。
/// 7 + 4 * 12 + 余り 2（葉のインライン）= 57 が 64 以下に収まる。13 以上だと最悪の形で 64 を超えうる。
/// 超えると `compile` の出力検査が `E-LIMIT` にするため、12 とする。
///
/// 描画側の深さの上限（web の `MAX_RENDER_DEPTH` = 32 段。SAFE-12）とは別の値で、描画側が自分で検査する。
pub const MAX_DOC_DEPTH: usize = 12;

/// 木の版を読み、[`TREE_VERSION`] 以外なら拒否する（`Doc::version` の逆直列化）。
fn deserialize_tree_version<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = u32::deserialize(deserializer)?;
    if v == TREE_VERSION {
        Ok(v)
    } else {
        Err(serde::de::Error::custom("unsupported tree version"))
    }
}

/// 本文または練習データの内容の木。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Doc {
    /// 木の版（[`TREE_VERSION`]）。違う値は逆直列化の段で拒否する。
    ///
    /// `Doc` の型に置くのは、`load_json::<Doc>`・`load_ir`（`body_doc`・`body_levels_doc`・`fixture_docs`）など
    /// 木を読むすべての口に、呼び出し箇所を足さずに効かせるため（取りこぼしを作らない。fail-closed）。
    /// 拒否の分類は型・内容の不一致（`LoadError::Malformed`・`IrLoadError::Json`）で、版専用の分類は作らない。
    /// 診断の細かさより網羅性を取り、公開の型も変えない。エラーの文言に入力の値は含めない。
    #[serde(deserialize_with = "deserialize_tree_version")]
    pub version: u32,
    /// 最上位のブロック。
    pub blocks: Vec<Block>,
}

/// ブロック。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case", deny_unknown_fields)]
pub enum Block {
    /// 見出し。
    Heading {
        /// 見出しの水準（1〜6）。
        level: u8,
        /// 見出しの中身。
        children: Vec<Inline>,
    },
    /// 段落。
    Paragraph {
        /// 段落の中身。
        children: Vec<Inline>,
    },
    /// 引用。
    Quote {
        /// 引用の中のブロック。
        children: Vec<Block>,
    },
    /// リスト。
    List {
        /// 番号つきか。
        ordered: bool,
        /// 項目。各項目はブロックの列。
        items: Vec<Vec<Block>>,
    },
    /// コードの囲み。
    Code {
        /// 言語名（空白の前の最初の語だけ）。無いときは書き出さない。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lang: Option<String>,
        /// コードの全文。
        text: String,
    },
    /// 表。
    Table {
        /// 見出しの行のセル。
        head: Vec<Vec<Inline>>,
        /// 本体の行。各行はセルの列。
        rows: Vec<Vec<Vec<Inline>>>,
    },
    /// 区切り線。
    Rule,
}

/// インライン。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case", deny_unknown_fields)]
pub enum Inline {
    /// テキスト。
    Text {
        /// 文字列。
        text: String,
    },
    /// 行内のコード。
    Code {
        /// コードの文字列。
        text: String,
    },
    /// 強調。
    Emph {
        /// 中身。
        children: Vec<Inline>,
    },
    /// 強い強調。
    Strong {
        /// 中身。
        children: Vec<Inline>,
    },
    /// リンク。`href` は検査せずに入れる（スキームの許可は表示側。TASK-149）。
    Link {
        /// 宛先。
        href: String,
        /// 中身。
        children: Vec<Inline>,
    },
    /// 画像。`src` は検査せずに入れる（TASK-149）。
    Image {
        /// 画像の URL。
        src: String,
        /// 代替テキスト。
        alt: String,
    },
    /// 強制改行。
    Break,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_node_kind_is_rejected() {
        let bad = r#"{"version":1,"blocks":[{"t":"script","text":"x"}]}"#;
        assert!(serde_json::from_str::<Doc>(bad).is_err());
        let bad2 = r#"{"version":1,"blocks":[{"t":"paragraph","children":[],"onclick":"x"}]}"#;
        assert!(serde_json::from_str::<Doc>(bad2).is_err());
        let bad3 = r#"{"version":1,"blocks":[],"onclick":"x"}"#;
        assert!(serde_json::from_str::<Doc>(bad3).is_err());
    }

    #[test]
    fn tree_version_other_than_current_is_rejected() {
        for v in [
            "0",
            "2",
            "4294967295",
            "4294967296",
            "-1",
            "1.5",
            "\"1\"",
            "null",
        ] {
            let json = format!(r#"{{"version":{v},"blocks":[]}}"#);
            assert!(serde_json::from_str::<Doc>(&json).is_err(), "{v}");
        }
        assert!(serde_json::from_str::<Doc>(r#"{"version":1,"blocks":[]}"#).is_ok());
    }

    #[test]
    fn code_without_lang_omits_the_field() {
        let b = Block::Code {
            lang: None,
            text: "x".into(),
        };
        let j = serde_json::to_string(&b).unwrap();
        assert_eq!(j, r#"{"t":"code","text":"x"}"#);
        let back: Block = serde_json::from_str(&j).unwrap();
        assert_eq!(back, b);
        let with = Block::Code {
            lang: Some("rust".into()),
            text: "x".into(),
        };
        assert!(serde_json::to_string(&with)
            .unwrap()
            .contains("\"lang\":\"rust\""));
    }

    #[test]
    fn unit_variants_roundtrip() {
        let d = Doc {
            version: TREE_VERSION,
            blocks: vec![Block::Rule],
        };
        let j = serde_json::to_string(&d).unwrap();
        assert_eq!(j, r#"{"version":1,"blocks":[{"t":"rule"}]}"#);
        assert_eq!(serde_json::from_str::<Doc>(&j).unwrap(), d);
    }
}
