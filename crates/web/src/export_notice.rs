//! 履歴の書き出しを案内する通知欄（`export-notice`）の文と、案内を出す時機の判定（SAVE-29・D-57・TASK-340）。
//!
//! 履歴はブラウザの保存領域にだけ置かれ、受講者が消す・ブラウザが自動で削除する・保存できない状態（シークレットモード等）で
//! 開く、のいずれでも失われ、再び開くと新しい状態で始まる。決定 D-57 は、失わない手段として「初回の保存時」と
//! 「章の終わり」に書き出しを案内すると決めた。案内の回数・文面・消える時機は未決で、ユーザーが決める。
//! 本モジュールの文と規則は**仮置き**であり、変えるときはこのモジュールだけを直す。
//!
//! 呼び出し元は `host`（案内の種類を `HostModel` に持つ）と `engine_parts` の `LogPart`（描画の文を引く）である。
//! 純関数だけを持ち、DOM・保存先・通信に触れない。文は固定の定数だけで作り、教材・受講者の値を含めない（A03）。
//! 書き出しの操作そのもの（リンク・ボタン）は TASK-531、通知欄の配置は TASK-534 が持つ。
//! `Event::ChapterCompleted` は core がまだ出さない（TASK-233 以降）ため、章の終わりは
//! 「章（`main` の根）の最後の Step（`is_final`）の達成、またはモジュールからその Step への復帰（`ModuleExited`）」でも判定する（SHELL-10）。TASK-339 が別に定めたら一本化する。

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::event::Event;

/// 案内の通知欄の束縛点の名前。
pub const EXPORT_NOTICE_FIELD: &str = "export_notice";

/// 初回の保存時の案内の文（仮置き）。
pub const FIRST_SAVE_TEXT: &str = "履歴はこのブラウザの中にだけ保存されます。保存領域が消えると戻せないため、履歴を書き出して手元に残してください。";

/// 章の終わりの案内の文（仮置き）。
pub const CHAPTER_END_TEXT: &str = "章の終わりです。ここまでの履歴を書き出して手元に残してください（このブラウザの保存領域が消えると戻せません）。";

/// 案内の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportHint {
    /// 保存済みの履歴が無い状態で始めたページの、最初の状態を変える操作（初回の保存を試みる時点）。
    FirstSave,
    /// 章の終わりに達した操作。
    ChapterEnd,
}

/// 通知欄の文。案内が無ければ空文字列。
pub fn export_notice_text(hint: Option<ExportHint>) -> &'static str {
    match hint {
        None => "",
        Some(ExportHint::FirstSave) => FIRST_SAVE_TEXT,
        Some(ExportHint::ChapterEnd) => CHAPTER_END_TEXT,
    }
}

/// 試験用の表示名（`stateJson` 用）。
pub fn export_hint_name(hint: Option<ExportHint>) -> Option<&'static str> {
    match hint {
        None => None,
        Some(ExportHint::FirstSave) => Some("first_save"),
        Some(ExportHint::ChapterEnd) => Some("chapter_end"),
    }
}

/// `uid` が `main` の根の最後の Step（`is_final`）か。IR に無い Step は偽（fail-closed）。
pub(crate) fn is_main_final(ir: &Ir, uid: &str) -> bool {
    uid.starts_with("main/")
        && ir
            .nodes
            .iter()
            .find(|n| n.uid == uid)
            .is_some_and(|n| n.step.is_final)
}

/// 1 件のイベントが章の終わりか。`ChapterCompleted`、`main` の根の最後の Step の `Achieved`、または
/// 学習モジュールから戻った先がその Step の `ModuleExited`。
/// モジュールからの復帰では core が呼び出し元の `Achieved` を出さず `ModuleExited` だけを出すため、
/// 最後の Step がモジュールを呼ぶ章でも章の終わりを検出できるようにする。
/// IR に無い Step は偽（fail-closed）。学習モジュールの最後の Step は呼び出し元へ戻るため数えない。
pub fn is_chapter_end(ir: &Ir, ev: &Event) -> bool {
    match ev {
        Event::ChapterCompleted { .. } => true,
        Event::Achieved { at, .. } | Event::ModuleExited { at, .. } => {
            is_main_final(ir, at.step.0.as_str())
        }
        _ => false,
    }
}

/// イベントの列のどれかが章の終わりか。
pub fn chapter_end_in(ir: &Ir, events: &[Event]) -> bool {
    events.iter().any(|e| is_chapter_end(ir, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 文は空でなく種類ごとに違い_なしは空() {
        assert_eq!(export_notice_text(None), "");
        let a = export_notice_text(Some(ExportHint::FirstSave));
        let b = export_notice_text(Some(ExportHint::ChapterEnd));
        assert!(!a.is_empty() && !b.is_empty() && a != b);
    }

    fn ir_with_final(uid: &str) -> Ir {
        let mut ir = fandhe_workshop_core::load_ir(
            &std::fs::read(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../testdata/engine-event-consistency/test-data/min-3step.json"),
            )
            .unwrap(),
        )
        .unwrap();
        for n in &mut ir.nodes {
            n.step.is_final = n.uid == uid;
        }
        ir
    }

    fn stamp(uid: &str) -> fandhe_workshop_core::addr::Stamp {
        fandhe_workshop_core::addr::Stamp {
            generation: fandhe_workshop_core::addr::Generation::INITIAL,
            step: fandhe_workshop_core::addr::StepUid(uid.to_owned()),
        }
    }

    #[test]
    fn モジュールから最後の_step_へ戻る復帰は章の終わり() {
        let ir = ir_with_final("main/st-3");
        let exited = |uid: &str| Event::ModuleExited {
            at: stamp(uid),
            instance: "m".to_owned(),
        };
        assert!(is_chapter_end(&ir, &exited("main/st-3")));
        assert!(!is_chapter_end(&ir, &exited("main/st-2")));
        assert!(!is_chapter_end(&ir, &exited("m/st-9")));
        assert!(chapter_end_in(
            &ir,
            &[exited("main/st-2"), exited("main/st-3")]
        ));
    }

    #[test]
    fn 試験用の名前() {
        assert_eq!(export_hint_name(None), None);
        assert_eq!(
            export_hint_name(Some(ExportHint::ChapterEnd)),
            Some("chapter_end")
        );
    }
}
