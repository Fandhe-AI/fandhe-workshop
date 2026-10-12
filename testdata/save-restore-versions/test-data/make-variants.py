#!/usr/bin/env python3
"""PoC-6 の版違い教材パックを、PoC-4 の教材パック（module-reuse-rounds/test-data/pack）から作る。
実行は 1 回だけ（固定の前）。固定の後は書き換えない。format-b/ には触れない。"""
import re, shutil, sys
from pathlib import Path

here = Path(__file__).resolve().parent
src = here.parent.parent / "module-reuse-rounds" / "test-data" / "pack"
out = here / "packs"
if out.exists():
    sys.exit("packs/ が既にあります（固定の後に作り直さない）")

def make(name, version, vocab="1.2", pack_id=None):
    d = out / name
    shutil.copytree(src, d)
    w = d / "workshop.yaml"
    t = w.read_text(encoding="utf-8")
    t = t.replace("  version: 0.0.1\n", f"  version: {version}\n  vocab: \"{vocab}\"\n", 1)
    if pack_id:
        t = t.replace("  id: bbs-tutorial\n", f"  id: {pack_id}\n", 1)
    w.write_text(t, encoding="utf-8")
    return d

def sub(path, old, new, count=1):
    t = path.read_text(encoding="utf-8")
    assert old in t, (path, old)
    path.write_text(t.replace(old, new, count), encoding="utf-8")

# 基準版 1.0.0
make("v1.0.0", "1.0.0")

# (i) 1.1.0: 文言だけ違う（Step・遷移・ルール・ID は同じ）
d = make("v1.1.0", "1.1.0")
sub(d / "workshop.yaml", "h-2-1-1: { level: 1, text: 企画の repo で、新しい企画を作る工程です。 }",
    "h-2-1-1: { level: 1, text: 企画を入れる repo で、新しい企画のアイデアを作る工程です。 }")
sub(d / "workshop.yaml", "message: 名前は、小文字・数字・ハイフンだけで書く形（kebab-case）にします。",
    "message: 名前には、小文字・数字・ハイフンだけを使います（kebab-case）。")
(d / "modules/idea-cycle/st-2-4.md").write_text(
    "# 検証計画の工程\n\n検証項目の候補から 1 つを選びます。ここは選ぶことが目的なので、左の変化を指す確認は置きません。\n",
    encoding="utf-8")

# (ii) 1.2.0: Step を 1 つ足す（st-2-3 の次に st-2-3b を挟む）
d = make("v1.2.0", "1.2.0")
m = d / "modules/idea-cycle/module.yaml"
sub(m, "    next:\n      - { to: st-2-2, when: { flag: hold } }\n      - { to: st-2-4 }\n",
    "    next:\n      - { to: st-2-2, when: { flag: hold } }\n      - { to: st-2-3b }\n")
sub(m, "  # ST-2.4 PoC 計画の工程",
    """  # ST-2.3b 検証計画の前の確認（1.2.0 で足した Step）
  - id: st-2-3b
    title: 検証計画の前の確認
    do: 確認した、を選びます
    body: st-2-3b.md
    views: [v-doc]
    next: [{ to: st-2-4 }]
    input:
      kind: choice
      options:
        - { id: ack, label: 確認した }
    rules:
      - id: r-ack
        on: { option: ack }
        outcome: { kind: accept }
    confirm:
      kind: none
      reason: 確認を選ぶこと自体が目的で、左の変化を指す学習目標がない

  # ST-2.4 PoC 計画の工程""")
(d / "modules/idea-cycle/st-2-3b.md").write_text(
    "# 検証計画の前の確認\n\n追求判定の結果を確かめてから、検証計画の工程に進みます。\n", encoding="utf-8")
shutil.rmtree(d / "tests")

# (iii) 2.0.0: Step を 1 つ消す（st-2-6 を消し、st-2-5 の次を st-2-7 にする）
d = make("v2.0.0", "2.0.0")
m = d / "modules/idea-cycle/module.yaml"
t = m.read_text(encoding="utf-8")
t = t.replace("  - id: s-2-6\n    node: st-2-6\n    effects:\n      - put: { path: \"ideas/${idea}/README.md\", fixture: fx-kakuu-bbs-readme, attrs: { status: ビヘイビア定義 } }\n      - put: { path: \"ideas/${idea}/02-poc-plan.md\", fixture: fx-kakuu-poc-plan }\n", "", 1)
t = t.replace("      - { to: st-2-6 }\n", "      - { to: st-2-7 }\n", 1)
i = t.index("  # ST-2.6 ビヘイビア定義の工程")
j = t.index("  # ST-2.7 タスク分解の工程")
t = t[:i] + t[j:]
m.write_text(t, encoding="utf-8")
(d / "modules/idea-cycle/st-2-6.md").unlink()
shutil.rmtree(d / "tests")

# (iv-a) 語彙の宣言だけ違う履歴の元: 内容の版は 1.0.0 のまま、語彙を 1.0 と宣言した教材
make("v1.0.0-vocab-1.0", "1.0.0", vocab="1.0")

# 別の教材（壊れた履歴 6 の元）
make("other-pack", "1.0.0", pack_id="other-tutorial")
print("done")
