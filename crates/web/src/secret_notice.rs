//! 秘密情報らしい文字列の警告（`secret-notice`）の文と検査の口（SAFE-27・SAFE-29・TASK-164 1/3）。
//!
//! 呼び出し元は 3 つの保存の入口（`EngineHost::save_after_change`・`persist::save_deferred`・`persist::install_save_on_leave`）で、
//! どれも書く直前の値を [`secret_hold`] で調べ、見つかれば保存先へ積まない（D-44 の失敗側。元に戻せない書き込みを避ける）。
//! 表示は `engine_parts` の `LogPart` が [`secret_notice_text`] を引く。
//! 文は固定の定数と形の名前（`SecretShape::label`）だけで作り、値・指紋・教材の文字列・受講者の値を含めない。
//! 文言と配置は仮置きで、確定は TASK-534 が持つ。「このまま保存する」などの選択は TASK-164 3/3 が持つ。

use fandhe_workshop_core::secret_scan::{scan_json_text, shapes_of, SecretShape};

/// 通知欄の束縛点の名前（`engine_parts` の `F_LOG` の末尾と同じ。ホストが dirty に積むときに使う）。
pub const SECRET_NOTICE_FIELD: &str = "secret_notice";

/// 遅延保存の検出をホストへ届ける action の名前（`decode_action` が受ける私的な名前。`RuntimeAction` の契約には入れない）。
pub const ACTION_SECRET_HOLD: &str = "secret-hold";

/// 保存する値を調べ、見つかった形を固定の順序で返す。無ければ空。
///
/// 3 つの保存の入口が同じ判定を使うための唯一の入口。値は返さず、保存もしない。
pub fn secret_hold(value: &str) -> Vec<SecretShape> {
    shapes_of(&scan_json_text(value))
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

    #[test]
    fn plain_text_is_not_held() {
        assert!(secret_hold("{\"draft\":\"hello\"}").is_empty());
        assert!(secret_hold("not json at all").is_empty());
    }
}
