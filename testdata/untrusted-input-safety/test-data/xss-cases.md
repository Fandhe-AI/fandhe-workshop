# XSS の試験用の入力（PoC-7 手順 1）

実施の前に固定する試験データです。固定の日時と SHA-256 は `../env.md` に記録します。固定の後は書き換えません。足りないと分かったものは、このファイルを残したまま `test-data-addendum/` に別に足し、理由を記録します。

## このファイルの読み方

- 各ケースの**ペイロード**（試験用の入力）は、すべて `~~~~~~text` で囲んだブロックの中に書いてあります。ブロックの中は文字として表示されるだけで、このファイルを Markdown として描画しても実行されません。
- ペイロードは、実行されたら `window.__poc7_hit('<ケースの ID>')` を呼ぶか、実在しない予約済みのドメイン `example.invalid`（RFC 2606 / RFC 6761）の `https://example.invalid/<ケースの ID>` へ通信しようとする形にしています。実在の宛先は使いません。
- ペイロードの中の `\u{HEX}` は、試験の道具が Unicode の 1 文字（コードポイント HEX）に展開します（制御文字や見えない文字を、レビューで読める形で書くため）。文字としての `\u{` を試したいケースは作っていません。
- ペイロードの前後の改行は、ブロックの境界の改行で、含みません。複数行のペイロードは行を `\n` でつなぎます。
- 「入り口」の欄は、`02-poc-plan.md` の PoC-7 手順 3 の 6 つの入り口の記号です。そのケースを、どの入り口で使うかを示します（入り口ごとにペイロードを入れる位置は、手順 3 の担当が決める）。

| 記号 | 入り口 | ペイロードの入れ方 |
|------|-------|----------------|
| E1 | 教材パックの Markdown | Step の本文（`body`）の Markdown に、そのまま入れる |
| E2 | 構造定義の文字列 | 構造定義（TOML・YAML）の文字列の欄（タイトル、ヒント、拒否の文面、ID、コマンド名など）の値に入れる |
| E3 | 練習データ | 練習データ（fixtures）のファイルの本文・名前に入れる |
| E4 | 履歴の読み込み | 履歴ファイル（JSON）の文字列の欄の値に入れる |
| E5 | 入力欄 | 受講者の入力として送る（拒否の文面への差し込み `{{input_head}}` の経路を含む） |
| E6 | URL のクエリ・ハッシュ | 直接リンクのクエリ・ハッシュの値に入れる |

- 「発火」の欄: `hit` は記録用の関数の呼び出し、`net` は `example.invalid` への通信、`hit+net` は両方、`none` は見えるだけの崩れ（実行の経路は無いが、表示が壊れていないかを見る）。
- 「期待」は全ケース共通で、**実行 0 件・外部への通信の試み 0 件**です（表示が文字として出るか、無害な部品に変わるかは問いません）。

## 分類の一覧

| 番号 | 分類 | ケース数 |
|------|------|--------|
| 1 | 生の HTML のタグ | 6 |
| 2 | イベント属性 | 6 |
| 3 | `javascript:`・`data:`・`vbscript:` の URL | 7 |
| 4 | Markdown のリンク・画像の構文を使った注入 | 7 |
| 5 | 属性値の閉じ忘れによる注入 | 5 |
| 6 | SVG・MathML | 6 |
| 7 | エスケープの二重化 | 6 |
| 8 | 紛らわしい Unicode の文字 | 6 |
| 9 | 構造定義の文字列の欄への注入 | 6 |
| 10 | 履歴ファイルの文字列の欄への注入 | 5 |
| 11 | URL のクエリ・ハッシュ（直接リンク） | 4 |
| | 合計 | 64 |

## 1. 生の HTML のタグ

### XSS-01
- 分類: 1
- 入り口: E1, E2, E3, E4, E5
- 発火: hit

~~~~~~text
<script>__poc7_hit('XSS-01')</script>
~~~~~~

### XSS-02
- 分類: 1
- 入り口: E1, E2, E3, E4, E5
- 発火: net

~~~~~~text
<img src="https://example.invalid/XSS-02">
~~~~~~

### XSS-03
- 分類: 1
- 入り口: E1, E2, E3, E5
- 発火: net

~~~~~~text
<iframe src="https://example.invalid/XSS-03"></iframe>
~~~~~~

### XSS-04
- 分類: 1
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
<iframe srcdoc="<script>parent.__poc7_hit('XSS-04')</script>"></iframe>
~~~~~~

### XSS-05
- 分類: 1
- 入り口: E1, E2, E3, E5
- 発火: net

~~~~~~text
<object data="https://example.invalid/XSS-05"></object>
<embed src="https://example.invalid/XSS-05b">
~~~~~~

### XSS-06
- 分類: 1
- 入り口: E1, E2, E3, E5
- 発火: net

~~~~~~text
<link rel="stylesheet" href="https://example.invalid/XSS-06.css">
<style>@import url(https://example.invalid/XSS-06b.css);</style>
<meta http-equiv="refresh" content="0;url=https://example.invalid/XSS-06c">
~~~~~~

## 2. イベント属性

### XSS-07
- 分類: 2
- 入り口: E1, E2, E3, E4, E5
- 発火: hit

~~~~~~text
<img src=x onerror="__poc7_hit('XSS-07')">
~~~~~~

### XSS-08
- 分類: 2
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
<details open ontoggle="__poc7_hit('XSS-08')">x</details>
~~~~~~

### XSS-09
- 分類: 2
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
<input autofocus onfocus="__poc7_hit('XSS-09')">
~~~~~~

### XSS-10
- 分類: 2
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
<a href="#" onclick="__poc7_hit('XSS-10')">クリック</a>
~~~~~~

### XSS-11
- 分類: 2
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
<video><source src="x" onerror="__poc7_hit('XSS-11')"></video>
~~~~~~

### XSS-12
- 分類: 2
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
<body onload="__poc7_hit('XSS-12')">
<div onmouseover="__poc7_hit('XSS-12b')" style="display:block;width:100vw;height:100vh">x</div>
~~~~~~

## 3. `javascript:`・`data:`・`vbscript:` の URL

### XSS-13
- 分類: 3
- 入り口: E1, E2, E3, E4, E5, E6
- 発火: hit

~~~~~~text
[クリック](javascript:__poc7_hit('XSS-13'))
~~~~~~

### XSS-14
- 分類: 3
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
[クリック](JaVaScRiPt:__poc7_hit('XSS-14'))
~~~~~~

### XSS-15
- 分類: 3
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
[クリック](data:text/html,<script>parent.__poc7_hit('XSS-15')</script>)
~~~~~~

### XSS-16
- 分類: 3
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
[クリック](vbscript:msgbox(1))
<a href="vbscript:__poc7_hit('XSS-16')">x</a>
~~~~~~

### XSS-17
- 分類: 3
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
[クリック](java\u{09}script:__poc7_hit('XSS-17'))
<a href="java\u{0A}script:__poc7_hit('XSS-17b')">x</a>
~~~~~~

### XSS-18
- 分類: 3
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
[クリック](\u{01}javascript:__poc7_hit('XSS-18'))
<a href="\u{20}\u{20}javascript:__poc7_hit('XSS-18b')">x</a>
~~~~~~

### XSS-19
- 分類: 3
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
<a href="data:text/html;base64,PHNjcmlwdD5wYXJlbnQuX19wb2M3X2hpdCgnWFNTLTE5Jyk8L3NjcmlwdD4=">x</a>
~~~~~~

## 4. Markdown のリンク・画像の構文を使った注入

### XSS-20
- 分類: 4
- 入り口: E1, E2, E3, E5
- 発火: net

~~~~~~text
![画像](https://example.invalid/XSS-20)
~~~~~~

### XSS-21
- 分類: 4
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
![画像](x" onerror="__poc7_hit('XSS-21'))
~~~~~~

### XSS-22
- 分類: 4
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
[リンク](/ok "title" onmouseover="__poc7_hit('XSS-22')")
~~~~~~

### XSS-23
- 分類: 4
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
![画像](javascript:__poc7_hit('XSS-23'))
~~~~~~

### XSS-24
- 分類: 4
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
[参照][r]

[r]: javascript:__poc7_hit('XSS-24')
~~~~~~

### XSS-25
- 分類: 4
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
<javascript:__poc7_hit('XSS-25')>
~~~~~~

### XSS-26
- 分類: 4
- 入り口: E1, E2, E3, E5
- 発火: net

~~~~~~text
![a](//example.invalid/XSS-26.png)
[b](//example.invalid/XSS-26b)
~~~~~~

## 5. 属性値の閉じ忘れによる注入

### XSS-27
- 分類: 5
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
"><img src=x onerror="__poc7_hit('XSS-27')">
~~~~~~

### XSS-28
- 分類: 5
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
' onmouseover='__poc7_hit("XSS-28")' x='
~~~~~~

### XSS-29
- 分類: 5
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
[x](/a "\"><img src=x onerror=__poc7_hit('XSS-29')>")
~~~~~~

### XSS-30
- 分類: 5
- 入り口: E1, E2, E3, E5
- 発火: net

~~~~~~text
<img src="https://example.invalid/XSS-30
text after the unclosed attribute">
~~~~~~

### XSS-31
- 分類: 5
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
</p><script>__poc7_hit('XSS-31')</script><p class="
~~~~~~

## 6. SVG・MathML

### XSS-32
- 分類: 6
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
<svg onload="__poc7_hit('XSS-32')"></svg>
~~~~~~

### XSS-33
- 分類: 6
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
<svg><script>__poc7_hit('XSS-33')</script></svg>
~~~~~~

### XSS-34
- 分類: 6
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
<svg><a xlink:href="javascript:__poc7_hit('XSS-34')"><text x="10" y="20">クリック</text></a></svg>
~~~~~~

### XSS-35
- 分類: 6
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
<svg><animate onbegin="__poc7_hit('XSS-35')" attributeName="x" dur="1s"></animate></svg>
~~~~~~

### XSS-36
- 分類: 6
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
<math><mtext><table><mglyph><style><img src=x onerror="__poc7_hit('XSS-36')">
~~~~~~

### XSS-37
- 分類: 6
- 入り口: E1, E2, E3, E5
- 発火: hit+net

~~~~~~text
<svg><use href="https://example.invalid/XSS-37#a"></use></svg>
<math href="javascript:__poc7_hit('XSS-37b')">x</math>
~~~~~~

## 7. エスケープの二重化

### XSS-38
- 分類: 7
- 入り口: E1, E2, E3, E4, E5
- 発火: hit

~~~~~~text
&lt;script&gt;__poc7_hit('XSS-38')&lt;/script&gt;
~~~~~~

### XSS-39
- 分類: 7
- 入り口: E1, E2, E3, E4, E5
- 発火: hit

~~~~~~text
&amp;lt;img src=x onerror=__poc7_hit('XSS-39')&amp;gt;
~~~~~~

### XSS-40
- 分類: 7
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
&#60;img src=x onerror=__poc7_hit('XSS-40')&#62;
&#x3c;img src=x onerror=__poc7_hit('XSS-40b')&#x3e;
~~~~~~

### XSS-41
- 分類: 7
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
[クリック](&#106;avascript:__poc7_hit('XSS-41'))
[クリック](&#x6A;avascript&colon;__poc7_hit('XSS-41b'))
~~~~~~

### XSS-42
- 分類: 7
- 入り口: E1, E2, E3, E4, E5
- 発火: hit

~~~~~~text
%3Cimg src=x onerror=__poc7_hit('XSS-42')%3E
~~~~~~

### XSS-43
- 分類: 7
- 入り口: E1, E2, E3, E4, E5
- 発火: hit

~~~~~~text
\u003cimg src=x onerror=__poc7_hit('XSS-43')\u003e
~~~~~~

## 8. 紛らわしい Unicode の文字

### XSS-44
- 分類: 8
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
\u{FF1C}script\u{FF1E}__poc7_hit('XSS-44')\u{FF1C}/script\u{FF1E}
~~~~~~

### XSS-45
- 分類: 8
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
[クリック](java\u{200B}script:__poc7_hit('XSS-45'))
[クリック](\u{FF4A}\u{FF41}\u{FF56}\u{FF41}script\u{FF1A}__poc7_hit('XSS-45b'))
~~~~~~

### XSS-46
- 分類: 8
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
<img src=x onerror="__poc7_hit('XSS-46')\u{202E}">\u{202E}txet desrever
~~~~~~

### XSS-47
- 分類: 8
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
<\u{00}script>__poc7_hit('XSS-47')</script>
<scr\u{AD}ipt>__poc7_hit('XSS-47b')</scr\u{AD}ipt>
~~~~~~

### XSS-48
- 分類: 8
- 入り口: E1, E2, E3, E5
- 発火: hit

~~~~~~text
<img src=x onerror\u{3D}"__poc7_hit('XSS-48')">
<img src=x \u{FF4F}nerror="__poc7_hit('XSS-48b')">
~~~~~~

### XSS-49
- 分類: 8
- 入り口: E1, E2, E3, E4, E5
- 発火: none

~~~~~~text
\u{202E}\u{2066}\u{2069}\u{200F}\u{FEFF}\u{2028}\u{2029}紛らわしい制御文字だけ
~~~~~~

## 9. 構造定義の文字列の欄への注入

### XSS-50
- 分類: 9
- 入り口: E2
- 発火: hit

~~~~~~text
<img src=x onerror="__poc7_hit('XSS-50')">題名
~~~~~~

### XSS-51
- 分類: 9
- 入り口: E2
- 発火: hit

~~~~~~text
st-1"><img src=x onerror="__poc7_hit('XSS-51')">
~~~~~~

### XSS-52
- 分類: 9
- 入り口: E2
- 発火: hit

~~~~~~text
"><svg onload=__poc7_hit('XSS-52')>
~~~~~~

### XSS-53
- 分類: 9
- 入り口: E2
- 発火: hit

~~~~~~text
{{input_head}}<img src=x onerror="__poc7_hit('XSS-53')">{{input_head}}
~~~~~~

### XSS-54
- 分類: 9
- 入り口: E2, E3
- 発火: net

~~~~~~text
../../../XSS-54/etc/passwd
https://example.invalid/XSS-54/fixture.md
~~~~~~

### XSS-55
- 分類: 9
- 入り口: E2
- 発火: none

~~~~~~text
a: &a [x, x, x, x, x, x, x, x, x]
b: &b [*a, *a, *a, *a, *a, *a, *a, *a, *a]
c: &c [*b, *b, *b, *b, *b, *b, *b, *b, *b]
d: !!binary "PHNjcmlwdD5fX3BvYzdfaGl0KCdYU1MtNTUnKTwvc2NyaXB0Pg=="
~~~~~~

## 10. 履歴ファイルの文字列の欄への注入

### XSS-56
- 分類: 10
- 入り口: E4
- 発火: hit

~~~~~~text
</script><script>__poc7_hit('XSS-56')</script>
~~~~~~

### XSS-57
- 分類: 10
- 入り口: E4
- 発火: hit

~~~~~~text
<img src=x onerror="__poc7_hit('XSS-57')">私の回答です
~~~~~~

### XSS-58
- 分類: 10
- 入り口: E4
- 発火: none

~~~~~~text
{"__proto__":{"polluted":"XSS-58"},"constructor":{"prototype":{"polluted":"XSS-58"}}}
~~~~~~

### XSS-59
- 分類: 10
- 入り口: E4
- 発火: net

~~~~~~text
st-1/../../XSS-59","next":"https://example.invalid/XSS-59
~~~~~~

### XSS-60
- 分類: 10
- 入り口: E4
- 発火: hit

~~~~~~text
[履歴のリンク](javascript:__poc7_hit('XSS-60'))\u{2028}**強調**
~~~~~~

## 11. URL のクエリ・ハッシュ（直接リンク）

### XSS-61
- 分類: 11
- 入り口: E6
- 発火: hit

~~~~~~text
"><img src=x onerror=__poc7_hit('XSS-61')>
~~~~~~

### XSS-62
- 分類: 11
- 入り口: E6
- 発火: hit

~~~~~~text
javascript:__poc7_hit('XSS-62')
~~~~~~

### XSS-63
- 分類: 11
- 入り口: E6
- 発火: net

~~~~~~text
https://example.invalid/XSS-63
~~~~~~

### XSS-64
- 分類: 11
- 入り口: E6
- 発火: hit

~~~~~~text
/chapter/ch-1#<svg onload=__poc7_hit('XSS-64')>
~~~~~~
