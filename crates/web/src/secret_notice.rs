//! 秘密情報らしい文字列の警告（`secret-notice`）の文と検査の口（SAFE-27・SAFE-29・TASK-164 1/3）。
//!
//! 呼び出し元は 3 つの保存の入口（`EngineHost::save_after_change`・`persist::save_deferred`・`persist::install_save_on_leave`）で、
//! どれも書く直前の値を [`secret_hold`] で調べ、見つかれば保存先へ積まない（D-44 の失敗側。元に戻せない書き込みを避ける）。
//! 表示は `engine_parts` の `LogPart` が [`secret_notice_text`] を引く。
//! 文は固定の定数と形の名前（`SecretShape::label`）だけで作り、値・指紋・教材の文字列・受講者の値を含めない。
//! 文言と配置は仮置きで、確定は TASK-534 が持つ。
//!
//! 警告には 2 つの選択を付ける（SAFE-28・D-44。TASK-164 3/3）。「このまま保存する」（[`ACTION_SECRET_KEEP`]）は、見つかった値を
//! 確認済みとして扱う。確認済みは形と指紋（`Finding`）だけを持ち、判定の入口 [`secret_hold_except`] で除かれる。
//! 「この入力を残さない」（[`ACTION_SECRET_DISCARD`]）は、保存せずにホストの中で起動の分岐を読み直す。どちらの処理も `EngineHost::update` が持つ。

use fandhe_workshop_core::secret_scan::{scan_json_text, shapes_of, Finding, SecretShape};

/// 通知欄の束縛点の名前（`engine_parts` の `F_LOG` の末尾と同じ。ホストが dirty に積むときに使う）。
pub const SECRET_NOTICE_FIELD: &str = "secret_notice";

/// 遅延保存の検出をホストへ届ける action の名前（`decode_action` が受ける私的な名前。`RuntimeAction` の契約には入れない）。
pub const ACTION_SECRET_HOLD: &str = "secret-hold";

/// 「このまま保存する」の action の名前（payload は読まない。`decode_action` が完全一致で受ける。SAFE-28）。
pub const ACTION_SECRET_KEEP: &str = "secret-keep";

/// 「この入力を残さない」の action の名前（payload は読まない。`decode_action` が完全一致で受ける。SAFE-28）。
pub const ACTION_SECRET_DISCARD: &str = "secret-discard";

/// 「この入力を残さない」の読み直しが済んだことをホストへ届ける action の名前（私的な名前。payload は `ok` か `fail`。SAFE-28）。
///
/// IndexedDB の取り直しは非同期で、完了のコールバックからホストへ触れないため、配送のタイマーに載せる。`fail` は取り直せなかったことで、
/// ホストは新しく始めて保存を止める（fail-closed）。
pub const ACTION_SECRET_DISCARD_READ: &str = "secret-discard-read";

/// 選択のボタンの一覧が束縛される field 名（`engine_parts` の `F_LOG` の末尾と同じ。警告の空・非空が変わるとき dirty に積む）。
pub const SECRET_CHOICE_FIELD: &str = "secret_choice";

/// 選択のボタンの一覧の `data-testid`。
pub const SECRET_CHOICE_TESTID: &str = "secret-choice";

/// 「このまま保存する」のボタンの `data-testid`。
pub const SECRET_KEEP_TESTID: &str = "secret-keep";

/// 「この入力を残さない」のボタンの `data-testid`。
pub const SECRET_DISCARD_TESTID: &str = "secret-discard";

/// 「このまま保存する」のボタンの文言（仮置き。確定は TASK-534）。
pub const SECRET_KEEP_LABEL: &str = "このまま保存する";

/// 「この入力を残さない」のボタンの文言（仮置き。確定は TASK-534）。
pub const SECRET_DISCARD_LABEL: &str = "この入力を残さない";

/// 保存する値を調べ、見つかった形を固定の順序で返す。無ければ空。
///
/// 確認済みを除かない（厳しい側）。ホストを借りられない場面の後退先と、確認済みが 0 件の判定に使う。値は返さず、保存もしない。
pub fn secret_hold(value: &str) -> Vec<SecretShape> {
    secret_hold_except(value, &[])
}

/// 保存する値を調べ、確認済みの検出を除いた形を固定の順序で返す（SAFE-28）。
///
/// 4 つの入口（即時の保存・遅延保存・離れる直前・書き出し）が `EngineHost::secret_gate` を通して使う判定の本体。
/// 確認済みは形と指紋の組で照合するため、同じ形でも別の値は除かれない。
pub fn secret_hold_except(value: &str, acknowledged: &[Finding]) -> Vec<SecretShape> {
    shapes_of(&unacknowledged(value, acknowledged))
}

/// 値の検出から、確認済みでないものを返す。
pub fn unacknowledged(value: &str, acknowledged: &[Finding]) -> Vec<Finding> {
    scan_json_text(value)
        .into_iter()
        .filter(|f| !acknowledged.contains(f))
        .collect()
}

/// 検出の payload。`<保存世代> <形の ID をカンマでつないだ列>` の固定の ASCII（値は含めない）。形が無ければ列は空。
///
/// 世代は、検出した時点のホストの保存世代。配送が遅れる間に履歴の差し替えや即時の保存が走れば、古い検出は捨てられる。
pub fn encode_secret_hold(epoch: u64, shapes: &[SecretShape]) -> String {
    let ids = shapes.iter().map(|s| s.id()).collect::<Vec<_>>().join(",");
    format!("{epoch} {ids}")
}

/// [`encode_secret_hold`] の逆。完全一致だけを受け、正規形でない世代・未知の ID・空の要素・重複・固定の順序と違う並びは
/// `None`（信頼しない入力）。形の列が空なら「警告なし」を表す。
pub fn decode_secret_hold(payload: &str) -> Option<(u64, Vec<SecretShape>)> {
    let (epoch, ids) = payload.split_once(' ')?;
    if epoch.is_empty()
        || (epoch.len() > 1 && epoch.starts_with('0'))
        || !epoch.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let epoch = epoch.parse::<u64>().ok()?;
    decode_shapes(ids).map(|shapes| (epoch, shapes))
}

/// 形の ID の列の復号。空文字列は空の列。
fn decode_shapes(payload: &str) -> Option<Vec<SecretShape>> {
    if payload.is_empty() {
        return Some(Vec::new());
    }
    let shapes = payload
        .split(',')
        .map(SecretShape::from_id)
        .collect::<Option<Vec<_>>>()?;
    // 固定の順序で重複なしのものだけを正規形とする。
    let pos = |s: &SecretShape| SecretShape::ALL.iter().position(|a| a == s);
    shapes
        .windows(2)
        .all(|w| pos(&w[0]) < pos(&w[1]))
        .then_some(shapes)
}

/// 警告の文。形が無ければ空文字列（表示が消える）。値は含めない。
pub fn secret_notice_text(shapes: &[SecretShape]) -> String {
    if shapes.is_empty() {
        return String::new();
    }
    let names = shapes
        .iter()
        .map(|s| s.label())
        .collect::<Vec<_>>()
        .join("・");
    format!("秘密情報らしい文字列があります（{names}）。値は表示しません。")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_empty_text_and_empty_payload() {
        assert_eq!(secret_notice_text(&[]), "");
        assert_eq!(encode_secret_hold(3, &[]), "3 ");
        assert_eq!(decode_secret_hold("3 "), Some((3, Vec::new())));
    }

    #[test]
    fn payload_round_trips_for_every_subset_prefix() {
        for n in 1..=SecretShape::ALL.len() {
            let shapes = SecretShape::ALL[..n].to_vec();
            assert_eq!(
                decode_secret_hold(&encode_secret_hold(7, &shapes)),
                Some((7, shapes))
            );
        }
    }

    #[test]
    fn decode_rejects_malformed_payloads() {
        for bad in [
            "",
            "jwt",
            "1",
            "1 unknown",
            "1 jwt,",
            "1 ,jwt",
            "1 jwt,,jwt",
            "1 jwt,jwt",
            "1  jwt",
            "1 jwt ",
            "1 JWT",
            "1 private_key,jwt",
            " jwt",
            "01 jwt",
            "-1 jwt",
            "+1 jwt",
            "18446744073709551616 jwt",
        ] {
            assert_eq!(decode_secret_hold(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn text_names_each_shape_and_nothing_else() {
        let t = secret_notice_text(&[SecretShape::Jwt, SecretShape::PrivateKey]);
        assert!(t.contains(SecretShape::Jwt.label()));
        assert!(t.contains(SecretShape::PrivateKey.label()));
        assert!(!t.contains(SecretShape::GoogleApiKey.label()));
    }

    /// 単体試験用の JWT の形の文字列（実行時に組み立てる）。
    fn jwt_like() -> String {
        let seg = |n: usize| "a".repeat(n);
        format!("eyJ{}.eyJ{}.{}", seg(20), seg(20), seg(20))
    }

    #[test]
    fn acknowledged_findings_are_excluded_by_shape_and_fingerprint() {
        let value = serde_json::json!({ "draft": jwt_like() }).to_string();
        let all = scan_json_text(&value);
        assert!(!all.is_empty());
        assert_eq!(secret_hold_except(&value, &[]), secret_hold(&value));
        assert!(secret_hold_except(&value, &all).is_empty());
        // 指紋が違えば除かれない。
        let other = Finding {
            shape: all[0].shape,
            fingerprint: all[0].fingerprint ^ 1,
        };
        assert!(!secret_hold_except(&value, &[other]).is_empty());
    }

    #[test]
    fn plain_text_is_not_held() {
        assert!(secret_hold("{\"draft\":\"hello\"}").is_empty());
        assert!(secret_hold("not json at all").is_empty());
    }
}
