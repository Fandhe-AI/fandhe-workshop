//! 教材テスト（`playthroughs`・`error_inputs`）を型つきで読む（TASK-404 1/3、AUTHOR-4）。
//!
//! 呼び出し元は `fandhe-pack test` の実行系 [`crate::play_run`] で、`source::load` が読んだ
//! `TestsSrc`（中身は未解釈の JSON 値）を、実行できる形（入力の列と期待）にする。読めない欄は `E-PARSE`、
//! 教材テストが無い・模範解答が 0 件のときは `E-STRUCTURE` にして、試験を流す前の関門（fail-closed）で止める。
//!
//! 入力の記法と期待の語は、PoC の `playthrough-model.md`・`error-inputs.md` が固定している。語の解釈は
//! [`crate::vocab::result_kind`]・[`crate::vocab::rejection_category`] を通す（[`crate::play_expect`] と同じ入口）。
//!
//! 診断の文面は英語の固定文と位置の番号だけで、入力・期待・本文の値は写さない。項目に写す ID は `safe_ident` を
//! 通ったものだけで、通らなければ 1 始まりの位置（`#n`）にする。
//!
//! 扱わないこと
//! - 流して判定すること（[`crate::play_run`]）
//! - `cmd:`・`text:` を画面と同じ関数で分けること（TASK-406）
//! - `text_checks`・`guidance_checks`・`asserts` の判定（後続。ここでは定義があることだけを [`Unsupported`] に積む）

use serde_json::{Map, Value};

use crate::diag::{DiagCode, Diagnostic};
use crate::play_expect::{parse_expect, read_error_input_expects, ResultExpect};
use crate::source::{safe_ident, LoadedPack};

/// 1 操作で受講者がする入力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayOp {
    /// `cmd:`。
    Command(String),
    /// `text:`。
    Text(String),
    /// `choice:`。
    Choice(String),
    /// `ui: "<view> <path>"`。
    Ui {
        /// ビューの ID。
        view: String,
        /// ビュー内の指す先。
        path: String,
    },
    /// `approval:`。
    Approval {
        /// `approve`・`reject`・`revise`。
        decision: ApprovalWord,
        /// 「直す」の対象。
        target: Option<String>,
    },
    /// `cancel: true`。
    Cancel,
    /// `tick: true`（進行 1 つ）。
    Tick,
}

/// 承認の 3 語。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalWord {
    /// 承認。
    Approve,
    /// 却下。
    Reject,
    /// 直す。
    Revise,
}

/// 通し試験の 1 操作の期待。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// 受付・拒否・問い返し（誤入力の期待と同じ 3 形）。
    Result(ResultExpect),
    /// `achieved`（確認の操作の達成）。
    Achieved,
    /// `progress`（進行が 1 つ進んだ）。
    Progress,
}

/// 入力と期待の組。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptStep {
    /// 入力。
    pub op: PlayOp,
    /// 期待。
    pub expect: Expect,
}

/// 模範解答の終わりの条件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// 章を完了している。
    Complete,
    /// 現在の Step がこの実体 ID である。
    AtStep(String),
}

/// 開始点 1 つ分の入力列。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// 開始点の ID。
    pub start: String,
    /// 入力列。
    pub steps: Vec<ScriptStep>,
}

/// 模範解答 1 本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playthrough {
    /// 項目の名前（`playthrough:<id>`）から接頭辞を除いた ID。安全でなければ `#n`。
    pub id: String,
    /// 終わりの条件。
    pub end: End,
    /// 開始点ごとの入力列。
    pub segments: Vec<Segment>,
}

/// 誤入力 1 件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorInput {
    /// ID。安全でなければ `#n`。
    pub id: String,
    /// 開始点の ID。
    pub start: String,
    /// 本番の入力の前に流す入力列。
    pub pre: Vec<ScriptStep>,
    /// 本番の入力。
    pub input: PlayOp,
    /// 期待。
    pub expect: ResultExpect,
}

/// 本版では判定しない定義（定義があるのに通ったと見せないため、実行系が不合格に倒す）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsupported {
    /// `text_checks` が定義されている。
    TextChecks,
    /// `guidance_checks` が定義されている。
    GuidanceChecks,
    /// 模範解答が `asserts` を持つ。
    Asserts,
}

/// 型つきで読んだ教材テスト。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Script {
    /// 模範解答。
    pub playthroughs: Vec<Playthrough>,
    /// 誤入力。
    pub error_inputs: Vec<ErrorInput>,
    /// 判定しない定義。
    pub unsupported: Vec<Unsupported>,
}

fn diag(file: &str, item: &str, code: DiagCode, message: String) -> Diagnostic {
    let file = if file.trim().is_empty() { "." } else { file };
    // file と item は空でない値にしたので、Diagnostic::new は失敗しない。
    Diagnostic::new(file, item, code, message).expect("file and item are non-empty")
}

/// 一覧の要素の ID。`safe_ident` を通るときだけ写し、そうでなければ 1 始まりの位置にする。
fn id_of(index: usize, v: &Value) -> String {
    match v.get("id").and_then(Value::as_str) {
        Some(id) if safe_ident(id) => id.to_string(),
        _ => format!("#{}", index + 1),
    }
}

/// 入力の鍵の組を読む。`expect` は呼び出し側が扱うので、ここでは鍵として許す。
fn read_op(obj: &Map<String, Value>) -> Result<PlayOp, &'static str> {
    const INPUT_KEYS: [&str; 7] = ["cmd", "text", "choice", "ui", "approval", "cancel", "tick"];
    for k in obj.keys() {
        if !INPUT_KEYS.contains(&k.as_str()) && k != "expect" && k != "target" {
            return Err("input has an unknown key");
        }
    }
    let present: Vec<&str> = INPUT_KEYS
        .iter()
        .copied()
        .filter(|k| obj.contains_key(*k))
        .collect();
    let [key] = present.as_slice() else {
        return Err("input must have exactly one input key");
    };
    let string = |k: &str| obj.get(k).and_then(Value::as_str).map(str::to_string);
    if obj.contains_key("target") && *key != "approval" {
        return Err("target is only for approval");
    }
    match *key {
        "cmd" => string("cmd")
            .map(PlayOp::Command)
            .ok_or("cmd must be a string"),
        "text" => string("text")
            .map(PlayOp::Text)
            .ok_or("text must be a string"),
        "choice" => string("choice")
            .map(PlayOp::Choice)
            .ok_or("choice must be a string"),
        "ui" => {
            let s = string("ui").ok_or("ui must be a string")?;
            match s.split_once(' ') {
                Some((view, path)) if !view.is_empty() && !path.is_empty() => Ok(PlayOp::Ui {
                    view: view.to_string(),
                    path: path.to_string(),
                }),
                _ => Err("ui must be `<view> <path>`"),
            }
        }
        "approval" => {
            let decision = match string("approval").as_deref() {
                Some("approve") => ApprovalWord::Approve,
                Some("reject") => ApprovalWord::Reject,
                Some("revise") => ApprovalWord::Revise,
                _ => return Err("approval must be approve, reject or revise"),
            };
            let target = match obj.get("target") {
                None => None,
                Some(Value::String(t)) => Some(t.clone()),
                Some(_) => return Err("target must be a string"),
            };
            Ok(PlayOp::Approval { decision, target })
        }
        "cancel" if obj.get("cancel") == Some(&Value::Bool(true)) => Ok(PlayOp::Cancel),
        "tick" if obj.get("tick") == Some(&Value::Bool(true)) => Ok(PlayOp::Tick),
        _ => Err("cancel and tick must be true"),
    }
}

fn read_step_expect(obj: &Map<String, Value>) -> Result<Expect, &'static str> {
    match obj.get("expect") {
        Some(Value::String(s)) => match s.as_str() {
            "achieved" => Ok(Expect::Achieved),
            "progress" => Ok(Expect::Progress),
            other => parse_expect(other)
                .map(Expect::Result)
                .map_err(|_| "expect is not a known result"),
        },
        Some(_) => Err("expect must be a string"),
        None => Err("expect is missing"),
    }
}

fn read_step(v: &Value) -> Result<ScriptStep, &'static str> {
    let obj = v.as_object().ok_or("step must be a table")?;
    Ok(ScriptStep {
        op: read_op(obj)?,
        expect: read_step_expect(obj)?,
    })
}

fn read_steps(v: Option<&Value>, what: &'static str) -> Result<Vec<ScriptStep>, String> {
    let arr = match v {
        None => return Ok(Vec::new()),
        Some(Value::Array(a)) => a,
        Some(_) => return Err(format!("{what} must be a list")),
    };
    arr.iter()
        .enumerate()
        .map(|(n, s)| read_step(s).map_err(|m| format!("{what} {}: {m}", n + 1)))
        .collect()
}

fn read_playthrough(index: usize, v: &Value) -> Result<Playthrough, String> {
    let obj = v.as_object().ok_or("playthrough must be a table")?;
    let end = match obj.get("end").and_then(Value::as_str) {
        Some("complete") => End::Complete,
        Some(s) if !s.is_empty() => End::AtStep(s.to_string()),
        _ => return Err("playthrough: end must be a non-empty string".to_string()),
    };
    let Some(Value::Array(segs)) = obj.get("segments") else {
        return Err("playthrough: segments must be a list".to_string());
    };
    if segs.is_empty() {
        return Err("playthrough: segments must not be empty".to_string());
    }
    let mut segments = Vec::new();
    for (n, s) in segs.iter().enumerate() {
        let start = s
            .get("start")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .ok_or_else(|| format!("playthrough: segment {}: start must be a string", n + 1))?;
        let steps = match s.get("steps") {
            Some(st @ Value::Array(_)) => read_steps(Some(st), "step")
                .map_err(|m| format!("playthrough: segment {}: {m}", n + 1))?,
            _ => {
                return Err(format!(
                    "playthrough: segment {}: steps must be a list",
                    n + 1
                ))
            }
        };
        segments.push(Segment {
            start: start.to_string(),
            steps,
        });
    }
    Ok(Playthrough {
        id: id_of(index, v),
        end,
        segments,
    })
}

/// 誤入力の `input` と `pre` と `start` を読む。`expect` は呼び出し側が [`read_error_input_expects`] で読む。
fn read_error_input(
    index: usize,
    v: &Value,
    expect: Option<ResultExpect>,
) -> Result<Option<ErrorInput>, String> {
    let start = v
        .get("start")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .ok_or("error_input: start must be a string")?;
    let pre = read_steps(v.get("pre"), "pre").map_err(|m| format!("error_input: {m}"))?;
    let obj = v
        .get("input")
        .and_then(Value::as_object)
        .ok_or("error_input: input must be a table")?;
    let op = read_op(obj).map_err(|m| format!("error_input: {m}"))?;
    Ok(expect.map(|expect| ErrorInput {
        id: id_of(index, v),
        start: start.to_string(),
        pre,
        input: op,
        expect,
    }))
}

/// 教材テストを読む。誤りがあれば、一覧の順の診断（`E-PARSE`、無い・空は `E-STRUCTURE`）を返す。
///
/// 診断が 1 件でもあれば `Err` で、試験は流さない。誤入力の `expect` の診断は [`read_error_input_expects`] のものを
/// 1 回だけ出す。
pub fn read(pack: &LoadedPack) -> Result<Script, Vec<Diagnostic>> {
    let (Some(file), Some(tests)) = (pack.tests_file.as_deref(), pack.tests.as_ref()) else {
        return Err(vec![diag(
            "tests",
            "tests",
            DiagCode::Structure,
            "test requires at least one playthrough".to_string(),
        )]);
    };
    let mut diags = Vec::new();
    if tests.playthroughs.is_empty() {
        diags.push(diag(
            file,
            "tests",
            DiagCode::Structure,
            "test requires at least one playthrough".to_string(),
        ));
    }
    let mut script = Script::default();
    for (n, v) in tests.playthroughs.iter().enumerate() {
        match read_playthrough(n, v) {
            Ok(p) => script.playthroughs.push(p),
            Err(m) => diags.push(diag(
                file,
                &format!("playthrough:{}", id_of(n, v)),
                DiagCode::Parse,
                m,
            )),
        }
    }
    let expects = match read_error_input_expects(pack) {
        Ok(e) => Some(e),
        Err(d) => {
            diags.extend(d);
            None
        }
    };
    for (n, v) in tests.error_inputs.iter().enumerate() {
        let expect = expects
            .as_ref()
            .and_then(|e| e.get(n))
            .map(|(_, expect)| *expect);
        match read_error_input(n, v, expect) {
            Ok(Some(e)) => script.error_inputs.push(e),
            Ok(None) => {}
            Err(m) => diags.push(diag(
                file,
                &format!("error_input:{}", id_of(n, v)),
                DiagCode::Parse,
                m,
            )),
        }
    }
    if !tests.text_checks.is_empty() {
        script.unsupported.push(Unsupported::TextChecks);
    }
    if !tests.guidance_checks.is_empty() {
        script.unsupported.push(Unsupported::GuidanceChecks);
    }
    if tests
        .playthroughs
        .iter()
        .any(|p| p.get("asserts").is_some())
    {
        script.unsupported.push(Unsupported::Asserts);
    }
    if diags.is_empty() {
        Ok(script)
    } else {
        Err(diags)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fandhe_workshop_contract::judgment::RejectCategory;
    use serde_json::json;

    fn op(v: Value) -> Result<PlayOp, &'static str> {
        read_op(v.as_object().expect("object"))
    }

    #[test]
    fn reads_every_input_shape() {
        assert_eq!(
            op(json!({"cmd": "/x a"})),
            Ok(PlayOp::Command("/x a".into()))
        );
        assert_eq!(op(json!({"text": "hi"})), Ok(PlayOp::Text("hi".into())));
        assert_eq!(op(json!({"choice": "c"})), Ok(PlayOp::Choice("c".into())));
        assert_eq!(
            op(json!({"ui": "v-files a/b c"})),
            Ok(PlayOp::Ui {
                view: "v-files".into(),
                path: "a/b c".into()
            })
        );
        assert_eq!(
            op(json!({"approval": "revise", "target": "t"})),
            Ok(PlayOp::Approval {
                decision: ApprovalWord::Revise,
                target: Some("t".into())
            })
        );
        assert_eq!(op(json!({"cancel": true})), Ok(PlayOp::Cancel));
        assert_eq!(op(json!({"tick": true})), Ok(PlayOp::Tick));
    }

    #[test]
    fn rejects_malformed_inputs() {
        for v in [
            json!({}),
            json!({"expect": "accepted"}),
            json!({"cmd": "a", "text": "b"}),
            json!({"cmd": 1}),
            json!({"foo": "x"}),
            json!({"ui": "only-view"}),
            json!({"approval": "maybe"}),
            json!({"cancel": false}),
            json!({"tick": "yes"}),
            json!({"cmd": "a", "target": "t"}),
        ] {
            assert!(op(v.clone()).is_err(), "{v}");
        }
    }

    #[test]
    fn reads_expect_words() {
        let e = |s: &str| read_step_expect(json!({"expect": s}).as_object().expect("object"));
        assert_eq!(e("achieved"), Ok(Expect::Achieved));
        assert_eq!(e("progress"), Ok(Expect::Progress));
        assert_eq!(
            e("accepted"),
            Ok(Expect::Result(ResultExpect::Accepted { free_text: false }))
        );
        assert_eq!(
            e("rejected:busy"),
            Ok(Expect::Result(ResultExpect::Rejected(RejectCategory::Busy)))
        );
        assert!(e("bogus").is_err());
        assert!(e("rejected").is_err());
    }

    #[test]
    fn unsafe_ids_become_positions() {
        assert_eq!(id_of(1, &json!({"id": "a b"})), "#2");
        assert_eq!(id_of(0, &json!({"id": "ok-1"})), "ok-1");
        assert_eq!(id_of(0, &json!({})), "#1");
    }

    #[test]
    fn playthrough_needs_segments_and_end() {
        let ok = json!({"id": "p", "end": "complete", "segments": [
            {"start": "s-1", "steps": [{"cmd": "/x", "expect": "accepted"}]}]});
        let p = read_playthrough(0, &ok).expect("reads");
        assert_eq!(p.end, End::Complete);
        assert_eq!(p.segments.len(), 1);
        for bad in [
            json!({"id": "p", "end": "", "segments": []}),
            json!({"id": "p", "end": "complete", "segments": []}),
            json!({"id": "p", "end": "complete"}),
            json!({"id": "p", "end": "complete", "segments": [{"steps": []}]}),
            json!({"id": "p", "end": "complete", "segments": [{"start": "s", "steps": [{"cmd": "x"}]}]}),
        ] {
            assert!(read_playthrough(0, &bad).is_err(), "{bad}");
        }
    }
}
