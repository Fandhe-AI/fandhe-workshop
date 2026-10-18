//! 外観（`pack.appearance`）の閉じた集合と読み取り（TASK-53 1/2）。
//!
//! 出所は語彙の文書の EP-11（`pack.appearance`）と spec の PACK-21 である。呼び出し元は、pack の
//! `appearance`（`pack.appearance` の検査と診断）と、web の `profile::Theme`（`data-theme` 属性の値）である。
//!
//! contract に置く理由: 画面と教材の検査は同じ閉じた集合・同じ読み取りの規則を使うが、web から pack への
//! normal 依存は禁じられている（`docs/architecture.md` 第 4 節）。2 つ以上の責務が参照する純粋な関数は
//! contract に置く（同第 6 節の判定規則 1。前例は `profile`）。
//!
//! 外観は見た目だけを変える。スタイル（CSS・色・フォント）を指定する欄は設けず、`theme` 以外の欄は拒否する。
//! I/O も状態も持たない純関数だけで構成し、wasm32 でも動く。

use serde_json::Value;

/// 外観の種類（EP-11）。閉じた集合で、教材の文字列は必ずここを通ってから画面の属性になる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    /// 標準（既定）。
    #[default]
    Standard,
    /// モノクロ。
    Mono,
    /// 高コントラスト。
    HighContrast,
    /// 読みやすさ重視。
    Reading,
}

impl Theme {
    /// 全種類。
    pub const ALL: [Theme; 4] = [
        Theme::Standard,
        Theme::Mono,
        Theme::HighContrast,
        Theme::Reading,
    ];

    /// 語彙の綴り。`data-theme` の値になる。
    pub const fn as_str(self) -> &'static str {
        match self {
            Theme::Standard => "standard",
            Theme::Mono => "mono",
            Theme::HighContrast => "high_contrast",
            Theme::Reading => "reading",
        }
    }

    /// 語彙の綴りから読む。大文字小文字を区別して完全一致だけを通す。
    pub fn from_vocab(s: &str) -> Option<Theme> {
        Theme::ALL.into_iter().find(|t| t.as_str() == s)
    }
}

/// `pack.appearance` が読めない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppearanceReadError {
    /// 表ではない。
    NotAMapping,
    /// `theme` 以外の欄がある。名前は未検査の値で、診断へ写す側が検査する。
    UnknownKey(String),
    /// `theme` が文字列ではない。
    ThemeNotString,
    /// `theme` が 4 値のどれでもない。
    ThemeUnknown,
}

/// `pack.appearance` の値を読む。`None` と空の表は既定値（`standard`）になる。
pub fn read_appearance(v: Option<&Value>) -> Result<Theme, AppearanceReadError> {
    let Some(v) = v else {
        return Ok(Theme::default());
    };
    let map = v.as_object().ok_or(AppearanceReadError::NotAMapping)?;
    let mut theme = Theme::default();
    for (k, val) in map {
        match k.as_str() {
            "theme" => {
                let s = val.as_str().ok_or(AppearanceReadError::ThemeNotString)?;
                theme = Theme::from_vocab(s).ok_or(AppearanceReadError::ThemeUnknown)?;
            }
            other => return Err(AppearanceReadError::UnknownKey(other.to_owned())),
        }
    }
    Ok(theme)
}
