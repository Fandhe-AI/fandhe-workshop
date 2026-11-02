# 入力: IR。引数 $ops: ["choice:opt-start", "ui:v-doc review/to", ...]。出力: {ok, reason, steps:[{changes}], final, node}
def apply_effects($attrs):
  reduce .[] as $e ($attrs;
    if ($e | keys) == ["set_attr"] and (.[$e.set_attr.path] != null) then
      .[$e.set_attr.path][$e.set_attr.attr] = $e.set_attr.value
    elif ($e | keys) == ["put"] and ($e.put.fixture != null) then
      .[$e.put.path] = $e.put.attrs
    else error("unsupported effect") end);
def diff($a; $b):
  reduce ($b | keys[]) as $p ({};
    reduce ($b[$p] | keys[]) as $k (.;
      if ($a[$p][$k]) != $b[$p][$k] then .[$p][$k] = $b[$p][$k] else . end));
. as $ir
| ($ir.nodes | map({key: .uid, value: .step}) | from_entries) as $steps
| ($ir.starts[0]) as $s
| ({} | ($s.effects // []) | apply_effects({})) as $init
| try (
    reduce $ops[] as $op ({node: $s.node, phase: "input", attrs: $init, log: []};
      ($steps[.node]) as $st
      | .attrs as $before
      | if .phase == "input" then
          ( [ $st.rules[] | select(
                (.when == null) and
                (if ($op | startswith("choice:")) then (.on.option? == ($op | ltrimstr("choice:")))
                  else (.on.ui? != null and ((.on.ui.view + " " + .on.ui.path) == ($op | ltrimstr("ui:")))) end)
              ) ] | first // error("no rule") ) as $r
          | if $r.outcome.kind != "accept" then error("not accepted") else . end
          | .attrs = ($r.outcome.effects // [] | apply_effects($before))
          | .phase = "confirm"
        else
          ( if $st.confirm.kind == "answer" then
              ($op | startswith("choice:")) and ([$st.confirm.options[] | select(.id == ($op | ltrimstr("choice:")) and .correct == true)] | length == 1)
            elif $st.confirm.kind == "point_at" then
              ($op | startswith("ui:")) and ([$st.confirm.targets[] | select((.view + " " + .path) == ($op | ltrimstr("ui:")))] | length >= 1)
            else false end ) as $good
          | if $good | not then error("not achieved") else . end
          | .attrs = ($st.on_achieved // [] | apply_effects($before))
          | (if $st.final == true then . else
                (if ($st.next | length) != 1 or ($st.next[0].when != null) then error("next") else . end)
                | .node = $st.next[0].to end)
          | .phase = "input"
        end
      | .log += [{op: $op, changes: diff($before; .attrs), node: .node, phase: .phase}]
    ) | {ok: true, steps: .log, final: .attrs, node: .node, phase: .phase}
  ) catch {ok: false, reason: .}
