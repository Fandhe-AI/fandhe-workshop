//! 上部の区画 `shell-checkpoint` に置く、チェックポイントの保存と復帰の操作（SHELL-13。TASK-529 1/2）の導出・復号・描画。
//!
//! 呼び出し元は `host::EngineHost` だけで、`view` が [`checkpoint_section`] を上部の区画の `rest` に置き、`update` が
//! 前後の [`offer`] を比べて [`CHECKPOINT_OPS_FIELD`] を dirty に積み、押下を [`parse_restore_payload`] と今の提示で照合して
//! `Control::Save`・`Control::Restore` を core へ送る。保存・復帰の規則そのもの（EVT-12・MOD-9）は core の `checkpoint` が持つ。
//!
//! 枠の契約（`shell`。版 2）は変えない。`shell-checkpoint` の入れ物の識別子は契約済みで、中のボタンの名前は
//! `offers::ACTION_CHOOSE` と同じく `shell-` の名前空間の外で本モジュールが定める。ボタンの文言は定数と u32 の 10 進だけで、
//! 教材由来の値は入れない（SAFE-11）。
//!
//! 出す条件は、session があり、表示だけのモードでなく、現在の Step があること（欠けたら 0 件。fail-closed）。
//! 押下の payload は DOM 上で書き換えられうるので、復帰先は payload から作らず、今の提示（`state.checkpoints` の鍵）と
//! 完全一致したときだけ使う（SAFE-33 と同じ方針）。
//!
//! 境界: 1/2 は最小の文言のボタンの結線までで、一覧としての表示と再読み込み・書き出しをまたぐ一致は 2/2（TASK-529 2/2）、
//! 章の終わりの自動保存と数の上限は TASK-339 が扱う。

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_core::SessionState;

use crate::artifact_view::{ACTION_ATTR, PAYLOAD_ATTR};
use crate::list_policy::keyed_view_or_notice;
use crate::parts::{FieldKind, PartField};
use crate::shell::TopSection;

/// 保存の action の名前。payload は空だけを受ける。
pub const ACTION_SAVE_CHECKPOINT: &str = "save-checkpoint";

/// 復帰の action の名前。payload はチェックポイントの ID の 10 進（正規形）だけを受ける。
pub const ACTION_RESTORE_CHECKPOINT: &str = "restore-checkpoint";

/// 保存のボタンの `data-testid`（0 か 1 個）。
pub const SAVE_TESTID: &str = "checkpoint-save";

/// 復帰のボタンの `data-testid`（チェックポイントごとに 0 個以上）。
pub const RESTORE_TESTID: &str = "checkpoint-restore";

/// 復帰のボタンに載せる ID の属性名。試験が読むための値で、payload と同じ。
pub const CHECKPOINT_ATTR: &str = "data-checkpoint";

/// 保存のボタンの文言。
pub const SAVE_LABEL: &str = "チェックポイントを保存";

/// 入れ物の CSS クラス。`static/app.css` の同名のクラスと一致させる。
pub const CHECKPOINT_CLASS: &str = "sh-checkpoint";

/// 保存・復帰のボタンの keyed list の field。提示が変わると `host::EngineHost` がこの名前を dirty に積む。
pub const CHECKPOINT_OPS_FIELD: PartField = PartField {
    name: "checkpoint_ops",
    kind: FieldKind::KeyedList,
};

/// 1 ページに出す復帰のボタンの数。保存・ページ送りのボタンと合わせて 1 リストの目安（`LIST_ITEMS_TARGET`）に収める（FEND-22・D-46）。
/// 新しい側から数えて分け、チェックポイントの数に関わらず、どれにもページ送りで届く（上限で黙って切らない）。
pub const PAGE_SIZE: usize = 500;

/// ページ送りの action の名前。payload はページ番号（新しい側が 0）の 10 進（正規形）だけを受ける。
pub const ACTION_CHECKPOINT_PAGE: &str = "checkpoint-page";

/// 「古い側のページへ」のボタンの `data-testid`（0 か 1 個）。
pub const OLDER_TESTID: &str = "checkpoint-older";

/// 「新しい側のページへ」のボタンの `data-testid`（0 か 1 個）。
pub const NEWER_TESTID: &str = "checkpoint-newer";

/// 復帰のボタンの文言。
pub fn restore_label(id: u32) -> String {
    format!("チェックポイント {id} に戻る")
}

/// 今提示する操作。前後で比べて dirty を積むための値で、出さないときは空の値。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckpointOffer {
    /// 保存のボタンを出すか。
    pub can_save: bool,
    /// 今のページで復帰のボタンを出すチェックポイントの ID（昇順）。
    pub ids: Vec<u32>,
    /// 今のページ（新しい側が 0）。ページ数に収まるよう丸めた値。
    pub page: usize,
    /// ページ数（チェックポイントが無いときは 0）。
    pub page_count: usize,
}

/// 状態から提示を導く。現在の Step が無いときは空（core の保存・復帰が no-op になるため）。処理中は止めない（EVT-12）。
/// `page` は希望のページで、ページ数を超えるときは最も古いページへ丸める。
pub fn offer(state: &SessionState, page: usize) -> CheckpointOffer {
    if state.run.current.is_none() {
        return CheckpointOffer::default();
    }
    let all: Vec<u32> = state.checkpoints.keys().map(|k| k.0).collect();
    offer_for(&all, page)
}

/// 昇順の ID の列とページから提示を導く（`offer` の本体。native の試験で多数の ID を直接与えるため分ける）。
pub fn offer_for(all: &[u32], page: usize) -> CheckpointOffer {
    let page_count = all.len().div_ceil(PAGE_SIZE);
    let page = page.min(page_count.saturating_sub(1));
    // 新しい側から page 番目の窓。窓の中は昇順のまま。
    let end = all.len().saturating_sub(page * PAGE_SIZE);
    let start = end.saturating_sub(PAGE_SIZE);
    CheckpointOffer {
        can_save: true,
        ids: all[start..end].to_vec(),
        page,
        page_count,
    }
}

/// ページ送りの payload をページ番号に直す。正規形の 10 進（`"0"` は許す。先頭 `0` は断る）だけを受ける。
pub fn parse_page_payload(payload: &str) -> Option<usize> {
    let b = payload.as_bytes();
    if b.is_empty()
        || b.len() > 10
        || (b[0] == b'0' && b.len() > 1)
        || !b.iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    payload.parse::<u32>().ok().map(|n| n as usize)
}

/// ページ送りの押下が今の提示のページ数に収まるときだけ、そのページを返す。
pub fn page_target(offer: &CheckpointOffer, page: usize) -> Option<usize> {
    (page < offer.page_count).then_some(page)
}

/// 復帰の payload を ID に直す。正規形の 10 進（空でない・数字だけ・10 桁以下・先頭が `0` でない・`u32` に収まる）だけを受ける。
/// ID は 1 から振られるので `"0"` も断る。
pub fn parse_restore_payload(payload: &str) -> Option<u32> {
    let b = payload.as_bytes();
    if b.is_empty() || b.len() > 10 || b[0] == b'0' || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    payload.parse::<u32>().ok()
}

/// 押下が今の提示と一致するときだけ復帰先の ID を返す（SAFE-33）。
pub fn restore_target(offer: &CheckpointOffer, id: u32) -> Option<u32> {
    offer.ids.contains(&id).then_some(id)
}

/// ページ送りのボタン。文言は定数だけで、押下の payload は移動先のページ番号。
fn page_button(testid: &str, label: &str, target: usize) -> Node {
    let payload = target.to_string();
    el(
        "button",
        vec![
            ("type", "button"),
            ("data-testid", testid),
            (ACTION_ATTR, ACTION_CHECKPOINT_PAGE),
            (PAYLOAD_ATTR, payload.as_str()),
        ],
        vec![text(label)],
    )
}

/// `shell-checkpoint` の入れ物。入れ物は常に描き、中の keyed list の項目の出し入れだけで表す（FEND-3）。
/// 項目の要素はボタンそのもので、フォーカスを保つため子にしない。並びは保存、続いて復帰の ID の昇順。
pub fn checkpoint_section(offer: &CheckpointOffer) -> Node {
    let mut items: Vec<(String, Node)> = Vec::new();
    if offer.can_save {
        items.push((
            "save".to_owned(),
            el(
                "button",
                vec![
                    ("type", "button"),
                    ("data-testid", SAVE_TESTID),
                    (ACTION_ATTR, ACTION_SAVE_CHECKPOINT),
                    (PAYLOAD_ATTR, ""),
                ],
                vec![text(SAVE_LABEL)],
            ),
        ));
        if offer.page + 1 < offer.page_count {
            items.push((
                "older".to_owned(),
                page_button(OLDER_TESTID, "古いチェックポイントを表示", offer.page + 1),
            ));
        }
        if offer.page > 0 {
            items.push((
                "newer".to_owned(),
                page_button(NEWER_TESTID, "新しいチェックポイントを表示", offer.page - 1),
            ));
        }
        for id in &offer.ids {
            let id_s = id.to_string();
            items.push((
                format!("restore:{id}"),
                el(
                    "button",
                    vec![
                        ("type", "button"),
                        ("data-testid", RESTORE_TESTID),
                        (ACTION_ATTR, ACTION_RESTORE_CHECKPOINT),
                        (PAYLOAD_ATTR, id_s.as_str()),
                        (CHECKPOINT_ATTR, id_s.as_str()),
                    ],
                    vec![text(restore_label(*id))],
                ),
            ));
        }
    }
    el(
        "div",
        vec![
            ("data-testid", TopSection::Checkpoint.testid()),
            ("class", CHECKPOINT_CLASS),
        ],
        vec![keyed_view_or_notice(
            "div",
            Vec::new(),
            &CHECKPOINT_OPS_FIELD,
            items,
        )],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payloadは正規形の10進だけ() {
        assert_eq!(parse_restore_payload("1"), Some(1));
        assert_eq!(parse_restore_payload("4294967295"), Some(u32::MAX));
        for bad in [
            "",
            "0",
            "01",
            "+1",
            " 1",
            "1 ",
            "12345678901",
            "4294967296",
            "a",
            "１",
        ] {
            assert_eq!(parse_restore_payload(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn 提示なしは入れ物だけで項目を持たない() {
        let s = format!("{:?}", checkpoint_section(&CheckpointOffer::default()));
        assert!(s.contains("shell-checkpoint"), "{s}");
        assert!(!s.contains(SAVE_TESTID), "{s}");
    }

    #[test]
    fn 提示ありは保存1と復帰の個数で検査器を通る() {
        let o = CheckpointOffer {
            can_save: true,
            ids: vec![1, 2],
            page: 0,
            page_count: 1,
        };
        let n = checkpoint_section(&o);
        let s = format!("{n:?}");
        assert_eq!(s.matches(SAVE_TESTID).count(), 1, "{s}");
        assert_eq!(s.matches(RESTORE_TESTID).count(), 2, "{s}");
        assert!(crate::shell::check_no_undefined(&n).is_ok());
        assert!(crate::a11y::check_no_undefined(&n).is_ok());
        assert_eq!(restore_target(&o, 2), Some(2));
        assert_eq!(restore_target(&o, 999), None);
    }

    #[test]
    fn 数千件でも最新の復帰にページ送りで届く() {
        // 4,096 件超（keyed list の上限を超える数）。
        let all: Vec<u32> = (1..=5_000).collect();
        let mut seen = std::collections::BTreeSet::new();
        let first = offer_for(&all, 0);
        assert_eq!(first.page_count, 10);
        assert_eq!(first.ids.last(), Some(&5_000));
        for p in 0..first.page_count {
            let o = offer_for(&all, p);
            let n = checkpoint_section(&o);
            let s = format!("{n:?}");
            // 1 リストは目安以内で、上限超過の通知（打ち切り）は出ない。
            assert!(o.ids.len() + 3 <= crate::list_policy::LIST_ITEMS_TARGET);
            assert!(!s.contains("limit"), "{s}");
            assert_eq!(s.matches(RESTORE_TESTID).count(), o.ids.len(), "{s}");
            assert_eq!(s.matches(OLDER_TESTID).count(), usize::from(p + 1 < 10));
            assert_eq!(s.matches(NEWER_TESTID).count(), usize::from(p > 0));
            seen.extend(o.ids.iter().copied());
        }
        assert_eq!(seen.len(), 5_000);
        // 範囲外のページは最も古いページへ丸める。
        assert_eq!(offer_for(&all, 99).page, 9);
        assert_eq!(page_target(&first, 10), None);
        assert_eq!(page_target(&first, 9), Some(9));
        assert_eq!(parse_page_payload("0"), Some(0));
        assert_eq!(parse_page_payload("01"), None);
        assert_eq!(parse_page_payload("-1"), None);
        assert_eq!(parse_page_payload(""), None);
    }
}
