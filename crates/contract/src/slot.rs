//! 文面の差し込みの枠 `{{名前|代替の文}}` の閉じた集合・構文の読み取り・差し込みの純関数（TASK-46、語彙 v1.1 の EP-7）。
//!
//! 呼び出し元は 2 つある。静的検査をする pack（`slot_check`。枠の名前・分類の制限・代替の文の有無を診断にする）と、
//! 実行時に拒否・問い返しの文面を組み立てる core（TASK-267。値の組を作って [`substitute`] を呼ぶ）である。
//! 両者が同じ表と構文を読むため、`docs/architecture.md` 第 6 節の判定規則 1 に従い contract に置く。
//! 実行時の値の組み立て（入力の先頭の語・近い候補・処理中の件数）は本モジュールの責務ではない。
//!
//! 枠は式でもスクリプトでもない。名前は [`SlotName`] の 6 語に閉じ、正規表現も再帰も使わず、1 回の走査で読む。
//! [`substitute`] は差し込んだ値を再び走査しない。実行時の値には受講者の入力の先頭の語が入るため、
//! 入力に `{{...}}` が混ざっても展開されない（A03）。結果は平文で、HTML として解釈しない。表示側（web）は
//! 既定のエスケープで描く前提である。教材の文面は信頼しない入力として扱い、構文の誤り・未知の名前は
//! パニックせずエラーで返す（fail-closed）。エラーの `Display` は入力値を含まない固定文にする。

use core::fmt;

use crate::judgment::Category;

/// この契約の版。枠の表・構文の規則を変えたら上げる。
pub const SLOT_CONTRACT_VERSION: u32 = 1;

/// 枠の名前。語彙の文書の `slot` の 6 語に閉じる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotName {
    /// 受講者の入力の先頭の語。
    InputHead,
    /// 近いコマンド（編集距離で選ぶ）。
    NearestCommand,
    /// 今の Step の題名。
    StepTitle,
    /// 今の Step の `do`。
    StepDo,
    /// 処理中の残りの回数。
    Remaining,
    /// 処理中の全体の回数。
    Total,
}

impl SlotName {
    /// 全 6 語。
    pub const ALL: [SlotName; 6] = [
        Self::InputHead,
        Self::NearestCommand,
        Self::StepTitle,
        Self::StepDo,
        Self::Remaining,
        Self::Total,
    ];

    /// 教材に書く名前。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InputHead => "input_head",
            Self::NearestCommand => "nearest_command",
            Self::StepTitle => "step_title",
            Self::StepDo => "step_do",
            Self::Remaining => "remaining",
            Self::Total => "total",
        }
    }

    /// 名前から読む。大文字・前後の空白・未知の語は `None`。
    pub fn from_str_opt(s: &str) -> Option<SlotName> {
        Self::ALL.into_iter().find(|n| n.as_str() == s)
    }

    /// 実行時に空（値なし）になりうるか。真の枠には代替の文を要求する（`E-SLOT-NO-FALLBACK`）。
    ///
    /// `step_do` は偽である。値を持たない Step で使うことを `E-SLOT-NEEDS-DO` が別に拒否するため。
    pub fn may_be_empty(self) -> bool {
        matches!(self, Self::InputHead | Self::NearestCommand)
    }

    /// この枠を使える分類を 1 つに限るとき、その分類。`None` はすべての分類で使える。
    /// 分類が限られた枠は、分類を持たない問い返しの文面では使えない。
    pub fn only_category(self) -> Option<Category> {
        match self {
            Self::NearestCommand => Some(Category::UnknownCommand),
            Self::Remaining | Self::Total => Some(Category::Busy),
            Self::InputHead | Self::StepTitle | Self::StepDo => None,
        }
    }
}

/// 文面の中の枠 1 つ。名前は前後の空白を除いた生の文字列で、既知かどうかは判定しない
/// （未知の名前を pack が診断にするため）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotRef<'a> {
    /// 名前（前後の空白を除いた生の文字列）。
    pub name: &'a str,
    /// 最初の `|` の後ろの代替の文。`|` が無ければ `None`。
    pub fallback: Option<&'a str>,
}

/// 構文の誤り。`Display` は入力値を含まない固定文にする。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotSyntaxError {
    /// `{{` が `}}` で閉じていない。
    Unclosed,
    /// 名前が空（`{{ }}`・`{{|x}}`）。
    EmptyName,
    /// 枠の中に `{{` がある（入れ子は受けない）。
    Nested,
}

impl fmt::Display for SlotSyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unclosed => "slot is not closed",
            Self::EmptyName => "slot name is empty",
            Self::Nested => "slots cannot be nested",
        })
    }
}

impl std::error::Error for SlotSyntaxError {}

/// 1 つ分の枠を読む。`text` は `{{` の直後から始まるとし、中身と `}}` の後ろの残りを返す。
fn read_one(after_open: &str) -> Result<(SlotRef<'_>, &str), SlotSyntaxError> {
    let Some(end) = after_open.find("}}") else {
        return Err(SlotSyntaxError::Unclosed);
    };
    let inner = &after_open[..end];
    if inner.contains("{{") {
        return Err(SlotSyntaxError::Nested);
    }
    let (name, fallback) = match inner.split_once('|') {
        Some((n, f)) => (n, Some(f)),
        None => (inner, None),
    };
    let name = name.trim();
    if name.is_empty() {
        return Err(SlotSyntaxError::EmptyName);
    }
    Ok((SlotRef { name, fallback }, &after_open[end + 2..]))
}

/// 文面の枠を先頭から順にすべて読む。`{{` から次の `}}` までを 1 つの枠とし、最初の `|` で名前と代替の文に分ける。
/// `{{` を伴わない `}}` は地の文として扱う。1 回の走査で、文面の長さに線形である。
pub fn parse_slots(text: &str) -> Result<Vec<SlotRef<'_>>, SlotSyntaxError> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("{{") {
        let (slot, tail) = read_one(&rest[i + 2..])?;
        out.push(slot);
        rest = tail;
    }
    Ok(out)
}

/// 差し込む値の組。実行時の組み立ては core（TASK-267）、検査では最悪の場合の値を pack が作る。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotValues {
    /// 入力の先頭の語。入力が空なら `None`。
    pub input_head: Option<String>,
    /// 近いコマンド。候補が無ければ `None`。
    pub nearest_command: Option<String>,
    /// 今の Step の題名。
    pub step_title: String,
    /// 今の Step の `do`。無ければ `None`。
    pub step_do: Option<String>,
    /// 処理中の残りの回数。処理中でなければ `None`。
    pub remaining: Option<u64>,
    /// 処理中の全体の回数。処理中でなければ `None`。
    pub total: Option<u64>,
}

/// 差し込みの失敗。`Display` は入力値を含まない固定文にする。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubstituteError {
    /// 構文の誤り。
    Syntax(SlotSyntaxError),
    /// 名前が閉じた集合にない。
    Unknown,
}

impl fmt::Display for SubstituteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(e) => e.fmt(f),
            Self::Unknown => f.write_str("unknown slot name"),
        }
    }
}

impl std::error::Error for SubstituteError {}

impl From<SlotSyntaxError> for SubstituteError {
    fn from(e: SlotSyntaxError) -> Self {
        Self::Syntax(e)
    }
}

impl SlotValues {
    /// 枠の値を文字列で返す。値が無い・空白だけのときは `None`。
    fn get(&self, name: SlotName) -> Option<String> {
        let v = match name {
            SlotName::InputHead => self.input_head.clone(),
            SlotName::NearestCommand => self.nearest_command.clone(),
            SlotName::StepTitle => Some(self.step_title.clone()),
            SlotName::StepDo => self.step_do.clone(),
            SlotName::Remaining => self.remaining.map(|n| n.to_string()),
            SlotName::Total => self.total.map(|n| n.to_string()),
        };
        v.filter(|s| !s.trim().is_empty())
    }
}

/// 文面の枠に値を差し込む。値が無い（または空白だけ）ときは代替の文を使い、代替の文も無ければ空にする。
///
/// 未知の名前・構文の誤りはエラーで返す（fail-closed）。差し込んだ値は再び走査しないので、値の中の
/// `{{...}}` は展開されない。
pub fn substitute(text: &str, v: &SlotValues) -> Result<String, SubstituteError> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("{{") {
        out.push_str(&rest[..i]);
        let (slot, tail) = read_one(&rest[i + 2..])?;
        let name = SlotName::from_str_opt(slot.name).ok_or(SubstituteError::Unknown)?;
        match v.get(name) {
            Some(value) => out.push_str(&value),
            None => out.push_str(slot.fallback.unwrap_or("")),
        }
        rest = tail;
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> SlotValues {
        SlotValues {
            input_head: Some("/hlep".to_string()),
            nearest_command: Some("/help".to_string()),
            step_title: "題名".to_string(),
            step_do: Some("やること".to_string()),
            remaining: Some(2),
            total: Some(5),
        }
    }

    fn empty_values() -> SlotValues {
        SlotValues {
            input_head: None,
            nearest_command: None,
            step_title: "題名".to_string(),
            step_do: None,
            remaining: None,
            total: None,
        }
    }

    #[test]
    fn parse_reads_plain_text_and_slots() {
        assert_eq!(parse_slots("枠なし"), Ok(vec![]));
        assert_eq!(
            parse_slots("{{x}}"),
            Ok(vec![SlotRef {
                name: "x",
                fallback: None
            }])
        );
        assert_eq!(
            parse_slots("a{{ x |代替}}b{{y}}"),
            Ok(vec![
                SlotRef {
                    name: "x",
                    fallback: Some("代替")
                },
                SlotRef {
                    name: "y",
                    fallback: None
                }
            ])
        );
    }

    #[test]
    fn parse_splits_at_the_first_bar_only() {
        let s = parse_slots("{{x|a|b}}").expect("ok");
        assert_eq!(s[0].fallback, Some("a|b"));
    }

    #[test]
    fn parse_rejects_syntax_errors() {
        assert_eq!(parse_slots("{{x"), Err(SlotSyntaxError::Unclosed));
        assert_eq!(parse_slots("{{x} }"), Err(SlotSyntaxError::Unclosed));
        assert_eq!(parse_slots("{{ }}"), Err(SlotSyntaxError::EmptyName));
        assert_eq!(parse_slots("{{|a}}"), Err(SlotSyntaxError::EmptyName));
        assert_eq!(parse_slots("{{a|{{b}}"), Err(SlotSyntaxError::Nested));
        assert_eq!(parse_slots("{{a{{b"), Err(SlotSyntaxError::Unclosed));
    }

    #[test]
    fn stray_close_is_plain_text() {
        assert_eq!(parse_slots("a}}b"), Ok(vec![]));
    }

    #[test]
    fn parse_handles_multibyte_text() {
        let s = parse_slots("「{{input_head|入力した語}}」は無い").expect("ok");
        assert_eq!(s[0].name, "input_head");
        assert_eq!(s[0].fallback, Some("入力した語"));
    }

    #[test]
    fn substitute_uses_values_then_fallback_then_empty() {
        let t = "「{{input_head|代替}}」と{{nearest_command}}";
        assert_eq!(substitute(t, &values()).as_deref(), Ok("「/hlep」と/help"));
        assert_eq!(substitute(t, &empty_values()).as_deref(), Ok("「代替」と"));
    }

    #[test]
    fn substitute_treats_blank_value_as_absent() {
        let mut v = values();
        v.input_head = Some("  ".to_string());
        assert_eq!(substitute("{{input_head|代替}}", &v).as_deref(), Ok("代替"));
    }

    #[test]
    fn substitute_renders_counts_and_step_fields() {
        assert_eq!(
            substitute(
                "{{remaining}}/{{total}} {{step_title}} {{step_do}}",
                &values()
            )
            .as_deref(),
            Ok("2/5 題名 やること")
        );
    }

    #[test]
    fn substitute_does_not_rescan_inserted_values() {
        let mut v = values();
        v.input_head = Some("{{step_title}}".to_string());
        assert_eq!(
            substitute("[{{input_head}}]", &v).as_deref(),
            Ok("[{{step_title}}]")
        );
    }

    #[test]
    fn substitute_fails_closed_on_unknown_and_syntax() {
        assert_eq!(
            substitute("{{nearest|x}}", &values()),
            Err(SubstituteError::Unknown)
        );
        assert_eq!(
            substitute("{{input_head", &values()),
            Err(SubstituteError::Syntax(SlotSyntaxError::Unclosed))
        );
    }

    #[test]
    fn errors_do_not_echo_input() {
        let secret = "SECRET-VALUE";
        let e = substitute(&format!("{{{{{secret}}}}}"), &values()).unwrap_err();
        assert!(!e.to_string().contains(secret));
        let e = parse_slots(&format!("{{{{{secret}")).unwrap_err();
        assert!(!e.to_string().contains(secret));
    }

    #[test]
    fn table_matches_the_vocabulary_rules() {
        assert_eq!(SlotName::ALL.len(), 6);
        for n in SlotName::ALL {
            assert_eq!(SlotName::from_str_opt(n.as_str()), Some(n));
        }
        assert_eq!(SlotName::from_str_opt("Input_Head"), None);
        let empty: Vec<_> = SlotName::ALL
            .into_iter()
            .filter(|n| n.may_be_empty())
            .map(SlotName::as_str)
            .collect();
        assert_eq!(empty, ["input_head", "nearest_command"]);
        assert_eq!(
            SlotName::NearestCommand.only_category(),
            Some(Category::UnknownCommand)
        );
        assert_eq!(SlotName::Remaining.only_category(), Some(Category::Busy));
        assert_eq!(SlotName::Total.only_category(), Some(Category::Busy));
        assert_eq!(SlotName::InputHead.only_category(), None);
        assert_eq!(SlotName::StepTitle.only_category(), None);
        assert_eq!(SlotName::StepDo.only_category(), None);
    }
}
