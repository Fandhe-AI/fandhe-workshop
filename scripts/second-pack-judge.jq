# 第 2 教材の誤入力の判定を IR 上で静的に解く（scripts/check-second-pack.sh の項目 text-checks が使う。TASK-746 2/2）。
# 入力: IR。引数 $ops: ["choice:opt-start", "ui:v-doc review/to", "cancel", ...]（開始点から順に送る入力）。
# 出力: 入力ごとに 1 要素の配列 [{op, kind: "accepted"|"achieved"|"rejected", rejection?, category?}]。
#
# 位置づけ: エンジン（crates/core の select_judge.rs・judge.rs）の文面の ID の選び方の写しであり、エンジンそのものではない。
#   受付の段: when の無い規則のうち on が一致する最初のものを使う。when のある規則は読まない（エンジンの入力の判定が
#   NoConditions を渡して when の規則を飛ばすことの写し）。一致なし・宣言外は分類 not_in_this_step の既定の文面に落ちる。
#   確認の段: answer は選択肢の on_wrong、無ければ確認の on_wrong。point_at の外れは on_miss。形に合わない入力は既定に落ちる。
# 限界: エンジンの判定順が変わると食い違う。食い違いは test が観測した分類との照合で一部だけ検出される。
# scripts/second-pack-fold.jq は正しい経路だけを畳み込む別の道具で、辿り方の重複は互いの参照で受け入れる。
. as $ir
| ($ir.nodes | map({key: .uid, value: .step}) | from_entries) as $steps
| ($ir.starts[0].node) as $start
| def rj($id): {rejection: $id, category: $ir.rejections[$id].category};
  def dflt: rj($ir.defaults.not_in_this_step);
  def opid($op): if ($op | startswith("choice:")) then ($op | ltrimstr("choice:")) else null end;
  def uiid($op): if ($op | startswith("ui:")) then ($op | ltrimstr("ui:")) else null end;
  reduce $ops[] as $op ({node: $start, phase: "input", out: []};
    ($steps[.node]) as $st
    | (if .phase == "input" then
        ( [ $st.rules[] | select((.when == null) and
              ((opid($op) != null and .on.option? == opid($op))
              or (uiid($op) != null and .on.ui? != null and ((.on.ui.view + " " + .on.ui.path) == uiid($op))))) ] | first ) as $r
        | if $r == null then {kind: "rejected"} + dflt
          elif $r.outcome.kind == "accept" then {kind: "accepted"}
          else {kind: "rejected"} + rj($r.outcome.rejection) end
      else
        ($st.confirm) as $c
        | if $c.kind == "answer" and opid($op) != null then
            ([$c.options[] | select(.id == opid($op))] | first) as $o
            | if $o == null then {kind: "rejected"} + dflt
              elif $o.correct == true then {kind: "achieved"}
              else {kind: "rejected"} + rj($o.on_wrong // $c.on_wrong // $st.on_wrong // $ir.defaults.not_in_this_step) end
          elif $c.kind == "point_at" and uiid($op) != null then
            if ([$c.targets[] | select((.view + " " + .path) == uiid($op))] | length) >= 1 then {kind: "achieved"}
            else {kind: "rejected"} + rj($c.on_miss // $st.on_miss // $ir.defaults.not_in_this_step) end
          else {kind: "rejected"} + dflt end
      end) as $res
    | .out += [{op: $op} + $res]
    | if $res.kind == "accepted" then .phase = "confirm"
      elif $res.kind == "achieved" then
        (if $st.final == true then . else .node = $st.next[0].to end) | .phase = "input"
      else . end
  ) | .out
