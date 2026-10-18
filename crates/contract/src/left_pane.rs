//! 左エリアの設定（`pack.left_pane`）の閉じた集合と読み取り（TASK-53 2/2）。
//!
//! 出所は語彙の文書の EP-12（`pack.left_pane`）と spec の PACK-22 である。呼び出し元は、pack の
//! `left_pane`（`pack.left_pane` の検査と `E-LEFT-*` の診断）と、web の `profile`（`LeftPane` の再公開）である。
//!
//! contract に置く理由: 画面と教材の検査は同じ閉じた集合・同じ読み取りの規則を使うが、web から pack への
//! normal 依存は禁じられている（`docs/architecture.md` 第 4 節）。2 つ以上の責務が参照する純粋な関数は
//! contract に置く（同第 6 節の判定規則 1。前例は `profile`・`appearance`）。
//!
//! 既定値は語彙 v1.3 の 5 節に従い、`world_list` は `shown`、`view_tabs` は `all` である。
//! `world_list`・`view_tabs` 以外の欄は拒否する。I/O も状態も持たない純関数だけで構成し、wasm32 でも動く。

use serde_json::Value;

/// 左エリアの世界の一覧の表示（EP-12）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorldList {
    /// 出す（既定）。
    #[default]
    Shown,
    /// 出さない。
    Hidden,
}

impl WorldList {
    /// 全種類。
    pub const ALL: [WorldList; 2] = [WorldList::Shown, WorldList::Hidden];

    /// 語彙の綴り。
    pub const fn as_str(self) -> &'static str {
        match self {
            WorldList::Shown => "shown",
            WorldList::Hidden => "hidden",
        }
    }

    /// 語彙の綴りから読む。大文字小文字を区別して完全一致だけを通す。
    pub fn from_vocab(s: &str) -> Option<WorldList> {
        WorldList::ALL.into_iter().find(|v| v.as_str() == s)
    }

    /// 出すか。
    pub const fn is_shown(self) -> bool {
        matches!(self, WorldList::Shown)
    }
}

/// 左エリアのビューの切り替えの表示（EP-12）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewTabs {
    /// すべてのビューのボタンを出す（既定）。
    #[default]
    All,
    /// 教材が宣言したビューだけに絞る。
    Declared,
    /// ボタンを出さない。
    Hidden,
}

impl ViewTabs {
    /// 全種類。
    pub const ALL: [ViewTabs; 3] = [ViewTabs::All, ViewTabs::Declared, ViewTabs::Hidden];

    /// 語彙の綴り。
    pub const fn as_str(self) -> &'static str {
        match self {
            ViewTabs::All => "all",
            ViewTabs::Declared => "declared",
            ViewTabs::Hidden => "hidden",
        }
    }

    /// 語彙の綴りから読む。大文字小文字を区別して完全一致だけを通す。
    pub fn from_vocab(s: &str) -> Option<ViewTabs> {
        ViewTabs::ALL.into_iter().find(|v| v.as_str() == s)
    }

    /// 切り替えのボタンを出すか。
    pub const fn shows_buttons(self) -> bool {
        !matches!(self, ViewTabs::Hidden)
    }

    /// 宣言したビューだけに絞るか。ビューの種類そのものは TASK-467 が持つため、種類の型に依らない述語にする。
    pub const fn limits_to_declared(self) -> bool {
        matches!(self, ViewTabs::Declared)
    }
}

/// 左エリアの設定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LeftPane {
    /// 世界の一覧。
    pub world_list: WorldList,
    /// ビューの切り替え。
    pub view_tabs: ViewTabs,
}

impl LeftPane {
    /// 既定（`shown`・`all`）か。`E-LEFT-*` は既定から変えたときだけ検査する。
    pub fn is_default(self) -> bool {
        self == LeftPane::default()
    }
}

/// `pack.left_pane` が読めない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeftPaneReadError {
    /// 表ではない。
    NotAMapping,
    /// `world_list`・`view_tabs` 以外の欄がある。名前は未検査の値で、診断へ写す側が検査する。
    UnknownKey(String),
    /// `world_list` が文字列ではない。
    WorldListNotString,
    /// `world_list` が 2 値のどれでもない。
    WorldListUnknown,
    /// `view_tabs` が文字列ではない。
    ViewTabsNotString,
    /// `view_tabs` が 3 値のどれでもない。
    ViewTabsUnknown,
}

/// `pack.left_pane` の値を読む。`None` と空の表は既定値になる。
pub fn read_left_pane(v: Option<&Value>) -> Result<LeftPane, LeftPaneReadError> {
    let Some(v) = v else {
        return Ok(LeftPane::default());
    };
    let map = v.as_object().ok_or(LeftPaneReadError::NotAMapping)?;
    let mut lp = LeftPane::default();
    for (k, val) in map {
        match k.as_str() {
            "world_list" => {
                let s = val.as_str().ok_or(LeftPaneReadError::WorldListNotString)?;
                lp.world_list =
                    WorldList::from_vocab(s).ok_or(LeftPaneReadError::WorldListUnknown)?;
            }
            "view_tabs" => {
                let s = val.as_str().ok_or(LeftPaneReadError::ViewTabsNotString)?;
                lp.view_tabs = ViewTabs::from_vocab(s).ok_or(LeftPaneReadError::ViewTabsUnknown)?;
            }
            other => return Err(LeftPaneReadError::UnknownKey(other.to_owned())),
        }
    }
    Ok(lp)
}
