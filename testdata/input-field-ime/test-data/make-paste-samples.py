#!/usr/bin/env python3
"""貼り付けの試料を決定的に生成する（PoC-2 手順 1）。文字数は Unicode のコードポイントで数える。"""
import os
d = os.path.join(os.path.dirname(os.path.abspath(__file__)), "paste-samples")
os.makedirs(d, exist_ok=True)

def w(name, s):
    with open(os.path.join(d, name), "w", encoding="utf-8", newline="") as f:
        f.write(s)

w("paste-3lines.txt", "一行目：貼り付けの試験です\n二行目：改行が保たれるかを見ます\n三行目：自動では送信されません\n")
w("paste-100lines.txt", "".join(f"行 {i:03d}：貼り付けの試験の 1 行です\n" for i in range(1, 101)))

unit = "あいうえおかきくけこ0123456789abcdefghij"  # 30 文字（BMP）
def make(n):
    s = (unit * (n // len(unit) + 1))[:n]
    # 100 文字ごとに改行を入れる（改行 1 文字も数に含める）
    out = []
    for i, ch in enumerate(s):
        out.append("\n" if (i + 1) % 100 == 0 else ch)
    return "".join(out)

s20000 = make(20000)
assert len(s20000) == 20000
w("paste-20000.txt", s20000)
w("paste-20001.txt", make(20001))
astral = make(19999) + "\U0001F600"
assert len(astral) == 20000 and len(astral.encode("utf-16-le")) // 2 == 20001
w("paste-20000-astral.txt", astral)
w("paste-crlf-tabs.txt", "列A\t列B\t列C\r\n1\t2\t3\r\n\t先頭が Tab の行\r\n")
w("paste-command-lines.txt", "/help\n/new-idea sample-name\n")
