//! 形式 B（YAML）の値の読み方を厳密にする前処理（PACK-4）。非公開。
//!
//! `source::Format::parse`（YAML の分岐）と `source` の章・モジュールの読み込みから呼ばれる。YAML は同じ表に
//! 同じキーを 2 回書いても後の値が黙って勝つ（`BTreeMap` や `serde_json::Value` へ読むときは特に）ため、
//! 型付きの読み込みの前に木を辿って重複キーを拒否する。また契約の型 `Step` の `Option<String>`（`body`・`do`）は
//! 引用符なしの `null`・`~` を `None` として読むため、同じテキストを文字列として読み直して字面を補う。
//! 契約の型は IR の JSON の読み込みと共有しているので変えない。
//!
//! キーの同一性は文字列で比べる（`true:` と `'true':` は同じキー）。構造定義のキーは最終的にすべて `String` に
//! 読まれ、YAML 上で別のキーでも 1 つに潰れて後勝ちになるためである。

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt;

use serde::de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;

use crate::source::safe_ident;

/// 前処理の失敗。
#[derive(Debug)]
pub(crate) enum StrictError {
    /// 同じ表に同じキーがある。`name` は安全な識別子のときだけ持つ。
    Duplicate {
        name: Option<String>,
        pos: Option<(usize, usize)>,
    },
    /// 重複以外（構文エラー等）。元のエラーの文字列と位置。
    Other {
        raw: String,
        pos: Option<(usize, usize)>,
    },
}

type DupSlot = RefCell<Option<Option<String>>>;

/// 重複キーを検査する。値は捨て、木を辿るだけである。
pub(crate) fn check_duplicate_keys(text: &str) -> Result<(), StrictError> {
    let slot: DupSlot = RefCell::new(None);
    let result = Seed { slot: &slot }.deserialize(yaml_serde::Deserializer::from_str(text));
    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            let pos = e.location().map(|l| (l.line(), l.column()));
            match slot.into_inner() {
                Some(name) => Err(StrictError::Duplicate { name, pos }),
                None => Err(StrictError::Other {
                    raw: e.to_string(),
                    pos,
                }),
            }
        }
    }
}

/// 値を辿る種。重複は共有の `slot` に記録する（エラーの文字列を解析しないため）。
struct Seed<'a> {
    slot: &'a DupSlot,
}

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_any(Walk { slot: self.slot })
    }
}

struct Walk<'a> {
    slot: &'a DupSlot,
}

impl<'de> Visitor<'de> for Walk<'_> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any YAML value")
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E: de::Error>(self, _: &str) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_none<E: de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_some<D: de::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        Seed { slot: self.slot }.deserialize(d)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq.next_element_seed(Seed { slot: self.slot })?.is_some() {}
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut seen = BTreeSet::new();
        while map
            .next_key_seed(KeySeed {
                seen: &mut seen,
                slot: self.slot,
            })?
            .is_some()
        {
            map.next_value_seed(Seed { slot: self.slot })?;
        }
        Ok(())
    }
    fn visit_enum<A: de::EnumAccess<'de>>(self, data: A) -> Result<(), A::Error> {
        // タグ付きの値。タグの中の表の重複も拒否する。型として受けるかは型付きの読み込みに任せる。
        use de::VariantAccess;
        let (_, variant) = data.variant::<IgnoredAny>()?;
        variant.newtype_variant_seed(Seed { slot: self.slot })
    }
}

/// 表のキーの種。重複はキーを読んだ場所でエラーにし、パーサーがキー自身の行・桁を付けられるようにする。
struct KeySeed<'a> {
    seen: &'a mut BTreeSet<String>,
    slot: &'a DupSlot,
}

impl<'de> DeserializeSeed<'de> for KeySeed<'_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        d.deserialize_str(self)
    }
}

impl<'de> Visitor<'de> for KeySeed<'_> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string key")
    }
    fn visit_str<E: de::Error>(self, key: &str) -> Result<(), E> {
        if self.seen.insert(key.to_string()) {
            Ok(())
        } else {
            // キー名は安全な識別子のときだけ後で写す。エラーの文字列には入れない。
            *self.slot.borrow_mut() = Some(safe_ident(key).then(|| key.to_string()));
            Err(E::custom("duplicate key"))
        }
    }
}

/// 人が書く層の `Option<String>` の欄を文字列として読み直した結果（Step ごと）。
///
/// 契約の人が書く層に `Option<String>` を足したら、ここにも足す。
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct StepStrings {
    pub body: Option<String>,
    pub do_: Option<String>,
}

#[derive(Deserialize)]
struct ScopeProbe {
    #[serde(default)]
    steps: Vec<StepProbe>,
}

#[derive(Deserialize)]
struct StepProbe {
    #[serde(default, deserialize_with = "as_str")]
    body: Option<String>,
    #[serde(default, rename = "do", deserialize_with = "as_str")]
    do_: Option<String>,
}

/// `deserialize_str` を強いて、`null`・`~` も字面の文字列として読む。
fn as_str<'de, D: de::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    String::deserialize(d).map(Some)
}

/// 章・モジュールの Step の `body`・`do` を文字列として読み直す。失敗は `Err`（呼び出し側が拒否する）。
pub(crate) fn string_probe(text: &str) -> Result<Vec<StepStrings>, ()> {
    let p: ScopeProbe = yaml_serde::from_str(text).map_err(|_| ())?;
    Ok(p.steps
        .into_iter()
        .map(|s| StepStrings {
            body: s.body,
            do_: s.do_,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dup(text: &str) -> (Option<String>, Option<(usize, usize)>) {
        match check_duplicate_keys(text) {
            Err(StrictError::Duplicate { name, pos }) => (name, pos),
            other => panic!("expected duplicate, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_at_top_level_reports_key_position() {
        let (n, p) = dup("a: 1\nb: 2\na: 3\n");
        assert_eq!(n.as_deref(), Some("a"));
        assert_eq!(p.map(|x| x.0), Some(3));
    }

    #[test]
    fn duplicate_in_nested_flow_and_sequence() {
        assert_eq!(dup("m:\n  k: 1\n  k: 2\n").0.as_deref(), Some("k"));
        assert_eq!(dup("m: {k: 1, k: 2}\n").0.as_deref(), Some("k"));
        assert_eq!(dup("s:\n  - x: 1\n    x: 2\n").0.as_deref(), Some("x"));
        assert_eq!(dup("a: !t {k: 1, k: 2}\n").0.as_deref(), Some("k"));
    }

    #[test]
    fn quoting_does_not_make_keys_distinct() {
        assert_eq!(dup("true: 1\n'true': 2\n").0.as_deref(), Some("true"));
    }

    #[test]
    fn unsafe_key_names_are_not_recorded() {
        assert_eq!(dup("\"a b\": 1\n\"a b\": 2\n").0, None);
        let long = "a".repeat(65);
        assert_eq!(dup(&format!("{long}: 1\n{long}: 2\n")).0, None);
    }

    #[test]
    fn clean_documents_pass() {
        assert!(check_duplicate_keys("a: 1\nb: {a: 1}\nc: [{a: 1}, {a: 2}]\n").is_ok());
        assert!(check_duplicate_keys("x: &a {k: 1}\ny: *a\n").is_ok());
        assert!(check_duplicate_keys("").is_ok());
    }

    #[test]
    fn syntax_errors_are_other() {
        match check_duplicate_keys("a: [1\n") {
            Err(StrictError::Other { pos, .. }) => assert!(pos.is_some()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn string_probe_reads_null_like_values_as_text() {
        let t = "steps:\n  - {id: a, do: null, body: ~}\n  - {id: b, do: 'null'}\n  - {id: c, do: }\n  - {id: d}\n";
        let v = string_probe(t).expect("probe");
        assert_eq!(v[0].do_.as_deref(), Some("null"));
        assert_eq!(v[0].body.as_deref(), Some("~"));
        assert_eq!(v[1].do_.as_deref(), Some("null"));
        assert_eq!(v[2].do_.as_deref(), Some(""));
        assert_eq!(v[3], StepStrings::default());
    }
}
