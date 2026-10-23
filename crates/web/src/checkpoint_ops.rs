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

/// 復帰のボタンの文言。
pub fn restore_label(id: u32) -> String {
    format!("チェックポイント {id} に戻る")
}

/// 今提示する操作。前後で比べて dirty を積むための値で、出さないときは空の値。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckpointOffer {
    /// 保存のボタンを出すか。
    pub can_save: bool,
    /// 復帰のボタンを出すチェックポイントの ID（昇順）。
    pub ids: Vec<u32>,
}

/// 状態から提示を導く。現在の Step が無いときは空（core の保存・復帰が no-op になるため）。処理中は止めない（EVT-12）。
pub fn offer(state: &SessionState) -> CheckpointOffer {
    if state.run.current.is_none() {
        return CheckpointOffer::default();
    }
    CheckpointOffer {
        can_save: true,
        ids: state.checkpoints.keys().map(|k| k.0).collect(),
    }
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
}
