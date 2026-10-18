//! 形式 B（YAML）の値の読み方を厳密にする前処理（PACK-4）。非公開。
//!
//! `source::Format::parse`（YAML の分岐）と `source` の章・モジュールの読み込みから呼ばれる。YAML は同じ表に
//! 同じキーを 2 回書いても後の値が黙って勝つ（`BTreeMap` や `serde_json::Value` へ読むときは特に）ため、
//! 型付きの読み込みの前に木を辿って重複キーを拒否する。また契約の型 `Step` の `Option<String>`（`body`・`do`）は
//! 引用符なしの `null`・`~` を `None` として読むため、同じテキストを文字列として読み直して字面を補う。
//! 契約の型は IR の JSON の読み込みと共有しているので変えない。
//!
//! 10 進でない整数（`0x1f`・`0o17`・`0b11`。PACK-5、D-43）は、`yaml_serde` が黙って 31 などの数に読むため、
//! `check_radix_ints` が 2 回の木の走査で字面を確かめて拒否する。数値の欄の多くは `serde_json::Value` の領域に
//! あり、`Value` へ読んだ時点で字面が失われるので、型付きの読み込みの側では見られない。また欄の名前の許可リスト
//! は使わない（後から足した数値の欄が黙って漏れる fail-open になるため）。
//!
//! アンカー・エイリアス・タグ・複数文書（PACK-5、D-43）も拒否する。`yaml_serde` はアンカーとエイリアスを透過に
//! 展開し、`!!int` などの中核のタグを内部で解決して消すため、型付きの読み込みや serde の木の走査では見えない。
//! そこで `check_yaml_features` が、安全な Rust だけの別のイベントパーサ（`saphyr-parser`）でイベントを直接見る。
//! 2 つのパーサの解釈が食い違ってイベントパーサがエラーを返したときも、通さずに拒否する（fail-closed）。
//!
//! キーの同一性は文字列で比べる（`true:` と `'true':` は同じキー）。構造定義のキーは最終的にすべて `String` に
//! 読まれ、YAML 上で別のキーでも 1 つに潰れて後勝ちになるためである。

use std::cell::{Cell, RefCell};
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
    /// 10 進でない整数（`0x`・`0o`・`0b`）を書いた。値は持たない（作成者の入力を診断へ写さない）。
    NonDecimalInt { pos: Option<(usize, usize)> },
    /// アンカー・エイリアス・タグ・複数文書のどれかを書いた（PACK-5）。名前・字面は持たない（作成者の入力を診断へ
    /// 写さない）。
    Feature {
        kind: YamlFeature,
        pos: Option<(usize, usize)>,
    },
    /// 重複以外（構文エラー等）。元のエラーの文字列と位置。
    Other {
        raw: String,
        pos: Option<(usize, usize)>,
    },
}

/// 拒否する YAML の機能の種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum YamlFeature {
    /// アンカー（`&x`）。
    Anchor,
    /// エイリアス（`*x`。マージキー `<<: *x` を含む）。
    Alias,
    /// 明示されたタグ（`!t`・`!!str`・`!!int` など）。
    Tag,
    /// 2 つ目以降の文書。
    MultipleDocuments,
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

/// 10 進でない整数のスカラーを拒否する（PACK-5）。値は捨て、木を 2 回辿るだけである。
///
/// 1 回目は `deserialize_any` で、整数として読まれた節を前順の番号で控える。2 回目は同じ順に辿り、整数の節だけ
/// `deserialize_str` で字面を受けて `0x`・`0o`・`0b`（符号を 1 つ外した後）を判定する。表のキーは文字列として
/// 読まれるので数えない。2 回の節の数が食い違えば、通さずに失敗にする（fail-closed）。
pub(crate) fn check_radix_ints(text: &str) -> Result<(), StrictError> {
    let flags: RefCell<Vec<bool>> = RefCell::new(Vec::new());
    MarkSeed { flags: &flags }
        .deserialize(yaml_serde::Deserializer::from_str(text))
        .map_err(|e| StrictError::Other {
            raw: e.to_string(),
            pos: e.location().map(|l| (l.line(), l.column())),
        })?;
    let flags = flags.into_inner();
    let slot: Cell<bool> = Cell::new(false);
    let next = Cell::new(0usize);
    let result = RadixSeed {
        flags: &flags,
        next: &next,
        slot: &slot,
    }
    .deserialize(yaml_serde::Deserializer::from_str(text));
    match result {
        Ok(()) if next.get() == flags.len() => Ok(()),
        Ok(()) => Err(StrictError::Other {
            raw: "node count mismatch".to_string(),
            pos: None,
        }),
        Err(e) => {
            let pos = e.location().map(|l| (l.line(), l.column()));
            if slot.get() {
                Err(StrictError::NonDecimalInt { pos })
            } else {
                Err(StrictError::Other {
                    raw: e.to_string(),
                    pos,
                })
            }
        }
    }
}

/// アンカー・エイリアス・タグ・複数文書を拒否する（PACK-5、D-43）。最初の 1 件で止める。
///
/// `check_duplicate_keys` が構文エラーと複数文書を先に拒否するので、ここへは `yaml_serde` が受け付けた文書だけが
/// 届く。複数文書の判定は多重の防御である。位置は 1 始まりの行・桁（`yaml_serde` の診断と同じ基準）。
pub(crate) fn check_yaml_features(text: &str) -> Result<(), StrictError> {
    use saphyr_parser::{Event, Parser};

    let mut documents = 0usize;
    for item in Parser::new_from_str(text) {
        let (event, span) = match item {
            Ok(pair) => pair,
            Err(_) => {
                // パーサの文言は写さない（入力の断片が混ざりうる）。
                return Err(StrictError::Other {
                    raw: "YAML event parser disagreed".to_string(),
                    pos: None,
                });
            }
        };
        let pos = Some((span.start.line(), span.start.col() + 1));
        let kind = match event {
            Event::DocumentStart(_) => {
                documents += 1;
                (documents > 1).then_some(YamlFeature::MultipleDocuments)
            }
            Event::Alias(_) => Some(YamlFeature::Alias),
            Event::Scalar(_, _, anchor, tag) => feature_of(anchor, tag.is_some()),
            Event::SequenceStart(anchor, tag) | Event::MappingStart(anchor, tag) => {
                feature_of(anchor, tag.is_some())
            }
            _ => None,
        };
        if let Some(kind) = kind {
            return Err(StrictError::Feature { kind, pos });
        }
    }
    Ok(())
}

/// ノードのアンカー番号（0 はアンカーなし）とタグの有無から、拒否する機能を決める。
fn feature_of(anchor_id: usize, has_tag: bool) -> Option<YamlFeature> {
    if anchor_id != 0 {
        Some(YamlFeature::Anchor)
    } else if has_tag {
        Some(YamlFeature::Tag)
    } else {
        None
    }
}

/// 符号を 1 つ外した後に `0x`・`0o`・`0b` で始まるか。`yaml_serde` が整数として読む形に合わせる。
fn is_radix_literal(s: &str) -> bool {
    let t = s.strip_prefix(['+', '-']).unwrap_or(s);
    ["0x", "0o", "0b"].iter().any(|p| t.starts_with(p))
}

/// 1 回目の走査の種。節ごとに 1 要素を積む。
struct MarkSeed<'a> {
    flags: &'a RefCell<Vec<bool>>,
}

impl<'de> DeserializeSeed<'de> for MarkSeed<'_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        let idx = {
            let mut f = self.flags.borrow_mut();
            f.push(false);
            f.len() - 1
        };
        d.deserialize_any(MarkWalk {
            flags: self.flags,
            idx,
        })
    }
}

struct MarkWalk<'a> {
    flags: &'a RefCell<Vec<bool>>,
    idx: usize,
}

impl MarkWalk<'_> {
    fn mark<E>(&self) -> Result<(), E> {
        self.flags.borrow_mut()[self.idx] = true;
        Ok(())
    }
}

impl<'de> Visitor<'de> for MarkWalk<'_> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any YAML value")
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<(), E> {
        self.mark()
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<(), E> {
        self.mark()
    }
    fn visit_i128<E: de::Error>(self, _: i128) -> Result<(), E> {
        self.mark()
    }
    fn visit_u128<E: de::Error>(self, _: u128) -> Result<(), E> {
        self.mark()
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
        MarkSeed { flags: self.flags }.deserialize(d)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq
            .next_element_seed(MarkSeed { flags: self.flags })?
            .is_some()
        {}
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while map.next_key_seed(SkipKey)?.is_some() {
            map.next_value_seed(MarkSeed { flags: self.flags })?;
        }
        Ok(())
    }
    fn visit_enum<A: de::EnumAccess<'de>>(self, data: A) -> Result<(), A::Error> {
        use de::VariantAccess;
        let (_, variant) = data.variant::<IgnoredAny>()?;
        variant.newtype_variant_seed(MarkSeed { flags: self.flags })
    }
}

/// 表のキーは文字列として読まれるので、字面を見ない・節に数えない。
struct SkipKey;

impl<'de> DeserializeSeed<'de> for SkipKey {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        IgnoredAny::deserialize(d).map(|_| ())
    }
}

/// 2 回目の走査の種。1 回目で整数だった節は文字列として受けて字面を確かめる。
#[derive(Clone, Copy)]
struct RadixSeed<'a> {
    flags: &'a [bool],
    next: &'a Cell<usize>,
    slot: &'a Cell<bool>,
}

impl<'de> DeserializeSeed<'de> for RadixSeed<'_> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<(), D::Error> {
        let idx = self.next.get();
        self.next.set(idx + 1);
        if self.flags.get(idx).copied().unwrap_or(false) {
            d.deserialize_str(self)
        } else {
            d.deserialize_any(RadixWalk(self))
        }
    }
}

/// 整数の節を文字列として受ける。
impl<'de> Visitor<'de> for RadixSeed<'_> {
    type Value = ();

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an integer scalar")
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<(), E> {
        if is_radix_literal(v) {
            self.slot.set(true);
            Err(E::custom("non-decimal integer"))
        } else {
            Ok(())
        }
    }
}

struct RadixWalk<'a>(RadixSeed<'a>);

impl<'de> Visitor<'de> for RadixWalk<'_> {
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
        self.0.deserialize(d)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq.next_element_seed(self.0)?.is_some() {}
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while map.next_key_seed(SkipKey)?.is_some() {
            map.next_value_seed(self.0)?;
        }
        Ok(())
    }
    fn visit_enum<A: de::EnumAccess<'de>>(self, data: A) -> Result<(), A::Error> {
        use de::VariantAccess;
        let (_, variant) = data.variant::<IgnoredAny>()?;
        variant.newtype_variant_seed(self.0)
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

    fn feature(text: &str) -> (YamlFeature, Option<(usize, usize)>) {
        match check_yaml_features(text) {
            Err(StrictError::Feature { kind, pos }) => (kind, pos),
            other => panic!("expected feature, got {other:?} for {text:?}"),
        }
    }

    #[test]
    fn anchors_aliases_tags_and_documents_are_detected() {
        use YamlFeature::*;
        let cases: &[(&str, YamlFeature)] = &[
            ("a: &x 1\n", Anchor),
            ("a: &x 1\nb: *x\n", Anchor),
            ("x: &a {k: 1}\ny: *a\n", Anchor),
            ("a: &x {k: 1}\nb: {<<: *x}\n", Anchor),
            ("a: !t 1\n", Tag),
            ("a: !!str 1\n", Tag),
            ("a: !!int 31\n", Tag),
            ("a: ! 1\n", Tag),
            ("a: !!map {k: 1}\n", Tag),
            ("a: !t [1]\n", Tag),
            ("- &x 1\n", Anchor),
            ("&x a: 1\n", Anchor),
            ("a: &x\n", Anchor),
            ("---\na: 1\n---\nb: 2\n", MultipleDocuments),
        ];
        for (text, want) in cases {
            assert_eq!(feature(text).0, *want, "{text:?}");
        }
    }

    #[test]
    fn plain_text_that_looks_like_features_is_not_detected() {
        for text in [
            "title: \"Q&A\"\n",
            "title: 'a*b'\n",
            "title: Hello!\n",
            "title: a & b\n",
            "# &x comment\na: 1\n",
            "a: |\n  * item\n  &x\n  !t\n",
            "a: >\n  !important\n",
            "a: [x, \"!y\"]\n",
            "---\na: 1\n",
            "a: 1\n...\n",
            "",
        ] {
            assert!(check_yaml_features(text).is_ok(), "{text:?}");
        }
    }

    #[test]
    fn feature_position_is_one_based_line_and_column() {
        let (_, pos) = feature("a: 1\nb: &x 2\n");
        let (line, col) = pos.expect("position");
        assert_eq!(line, 2);
        // 桁の基準は `yaml_serde` と同じ 1 始まり（重複キーが行頭にあれば 1）。位置はアンカーを付けたノード
        // （ここでは値 `2`）の先頭である。
        assert_eq!(dup("a: 1\nb: 2\nb: 3\n").1, Some((3, 1)));
        assert_eq!(col, 7);
    }

    #[test]
    fn undefined_alias_is_rejected_fail_closed() {
        assert!(matches!(
            check_yaml_features("a: *x\n"),
            Err(StrictError::Other { .. })
        ));
    }

    #[test]
    fn feature_error_does_not_carry_names() {
        let e = check_yaml_features("a: &secretanchor secretvalue\n").unwrap_err();
        let s = format!("{e:?}");
        assert!(!s.contains("secret"), "{s}");
        let e = check_yaml_features("a: !secrettag 1\n").unwrap_err();
        assert!(!format!("{e:?}").contains("secret"));
    }

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

    fn radix(text: &str) -> Option<Option<(usize, usize)>> {
        match check_radix_ints(text) {
            Ok(()) => None,
            Err(StrictError::NonDecimalInt { pos }) => Some(pos),
            Err(e) => panic!("unexpected {e:?}"),
        }
    }

    #[test]
    fn radix_ints_are_detected_in_every_position() {
        for t in [
            "a: 0x1f\n",
            "a: -0x1f\n",
            "a: +0x1f\n",
            "a: 0o17\n",
            "a: 0b11\n",
            "a: {kind: text, min: 1, max: 0x1f}\n",
            "a: [1, 2, {n: 0x1f}]\n",
            "a:\n  - n: 0x1f\n",
            "a: !t 0x1f\n",
            "a: !!int 0x1f\n",
            "a: 0xffffffffffffffffffff\n",
            "a: &x 0x1f\nb: *x\n",
        ] {
            assert!(radix(t).is_some(), "not detected: {t:?}");
        }
    }

    #[test]
    fn radix_ints_report_scalar_position() {
        let p = radix("a: 1\nb: {m: 0x1f}\n")
            .expect("detected")
            .expect("pos");
        assert_eq!(p.0, 2);
    }

    #[test]
    fn decimal_and_text_forms_are_not_radix_ints() {
        for t in [
            "a: 31\n",
            "a: '0x1f'\n",
            "a: \"0x1f\"\n",
            "a: |\n  0x1f\n",
            "0x1f: 1\n",
            "a: 0X1F\n",
            "a: 1e3\n",
            "a: 010\n",
            "a: [true, null, 1.5, x]\n",
            "",
        ] {
            assert!(radix(t).is_none(), "wrongly detected: {t:?}");
        }
    }

    #[test]
    fn radix_check_keeps_syntax_errors_as_other() {
        assert!(matches!(
            check_radix_ints("a: [1\n"),
            Err(StrictError::Other { .. })
        ));
    }
}
