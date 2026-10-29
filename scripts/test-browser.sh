#!/usr/bin/env bash
# ブラウザ試験（Playwright の共通ヘルパ）を流す（`make test-browser` から呼ばれる）。
#
# 配信設定の web（TEST_HOOKS なし）と試験の設定の web（TEST_HOOKS=1）を一時ディレクトリへビルドし、
# tools/browser/*.test.mjs を node:test で実行する。配信設定は FANDHE_WEB_DIR、試験の設定は FANDHE_WEB_TEST_DIR で渡す。
# 1 周目の終わりの履歴を読み込み欄から開き、次の章へ進むの実クリックで 2 周目を新しい根で始め、書き出しで世界状態の一致・閉じた根の保持・ops の追記を確かめる試験（round-handoff。MOD-19・MOD-5・MOD-8。TASK-254 2/2。入力は native の emit_round_handoff_for_browser が書き出す）を含む。
# 範囲外の章へ進まない試験（out-of-range-chapter。CH-3 を最後に使う絞った IR で CH-3 の完了後に範囲外の章へ入る操作が 0 件、範囲外の `?start=` は断られ、対照では出る。SHELL-12・D-67。TASK-527）を含む。
# 章の飛ばし・選択を出さない試験（chapter-select-absent。全時点で章を選ぶ部品が 0 件、章をまたぐ操作は「次の章へ進む」の 0 か 1 個だけ、開始点 10 件の教材でも一覧が出ない。検出器の対照つき。SHELL-11・D-73。TASK-528）を含む。
# 共通ヘルパの試験（harness・safety。安全性の計数の対照）と、実際の画面の試験（smoke-hello・keydown-action・esc-attr・esc-once（Esc が 10 場面（処理中か否か × フォーカスなし・root の外のボタン・入力欄・root の中のボタン・main の外の部品）でちょうど 1 回届く。FEND-6・FEND-8。TASK-353）・input-limit・paste・paste-notice（上限の案内文と貼り付けの観察。TERM-15・TERM-19）・learner-text・enter-submit・key-invariants（Enter 系・Tab 系のキーの不変条件の基準。FEND-21。TASK-351）・submit-wiring・tab-complete・shift-tab・tab-cycle・esc-key・json-depth・json-load-order・doc-links・doc-tree・tick-interval・three-views・log-lines-xss・pack-body-xss（Step の本文への注入 49 件。教材パックをコンパイルして配信する設定の画面の本文の区画で CSP あり・なしの計数 0・生の HTML が文字・外部の画像が代わりの文字。SAFE-1。TASK-144）・structure-xss（構造定義の欄と ID への注入。SAFE-2・TASK-145 1/2。`docs/testing.md` 5.366）・e2-ir（structure-xss が使う IR の組み立ての試験。TASK-145 1/2）・practice-data-xss（練習データへの注入 50 件。`put` のパスが左の成果物の行に文字として出て、指した受付がログに文字として出る。属性の値は画面に出ない経路の固定。SAFE-3。TASK-145 2/2。`docs/testing.md` 5.367）・e3-ir（practice-data-xss が使う IR の組み立ての試験。TASK-145 2/2）・input-field-xss（入力欄からの注入 49 件 × 3 通り。配信する設定・CSP あり・なし。SAFE-5。TASK-147 1/2。`docs/testing.md` 5.368）・url-xss（URL のクエリとハッシュからの注入 61 回。配信する設定・CSP あり・なし。SAFE-7。TASK-147 2/2。`docs/testing.md` 5.370）・reject-lines・reject-keeps-draft（拒否の 7 分類で、入力欄から送った本文が拒否の後も欄と状態の下書きで変わらない。JUDGE-5・TERM-17。TASK-271）・clarify-draft（問い返しの後に入力欄が空になり、全選択せずに続けて打った入力が wrong_order で拒否され unknown_command にならない。JUDGE-27・TERM-22。TASK-284 1/2）・reject-draft-select（拒否の後に下書きの全体が選択状態になり、全選択せずに打った文字で置き換わり連結されない。JUDGE-27・TERM-22。TASK-284 2/2）・esc-guide-st14（ST-1.4 の問い返しの後に `/ne` で候補が開いた状態で、Esc の 1 回目は閉じるだけ・2 回目で取り消しが受け付けられ、案内は IR の文面がそのまま出る。順序を教える案内文の変種ではその案内が出る。TERM-9・JUDGE-18・D-60。TASK-452 1/2）・modifier-enter-paste-st14（ST-1.4 の画面で、修飾キー付き Enter が送信にも改行にも補完にもならず、上限を超える途中への貼り付けが貼った分だけを切りキャレットが直後に残る。TERM-14・TERM-16・D-60・D-65。TASK-452 2/2）・draft-no-concat-screen（問い返し・拒否の後の下書きが連結されないことを画面の行と欄の値だけで確かめる。AI 代理の道具と同じ操作の経路つき。JUDGE-27・TERM-22。TASK-444）・choice-answer（選択肢の誤答・選び直し・達成。TERM-24）・choice-labels（`st-1-4`・`st-2-7` を含む 20 Step の全場面で選択肢のボタンに ID の文字が出る件数が 0 で、`label` を外した対照では検出する。TERM-25。TASK-446）・読むだけ・確認だけの Step が 1 択＋確認の回答で操作 2 回（余計な操作 1 回）になることを CSP あり・なし×マウス・キーボードで固定し、誤答を挟む対照で数え方を確かめる試験（read-only-step。TERM-30・PACK-35・D-69。TASK-449。`docs/testing.md` 5.348）・approval-buttons（承認のボタンと対象つきの直す。TERM-27）・approval-revise-labels（承認の「直す」の候補が宣言順に表示名で出て、誤った候補の押下は拒否・正しい対象は受付になる。承認の部品と ST-2.6 と宣言なしの対照。TERM-28・INTRO-29。TASK-448 1/2）・approval-reapprove（差し戻しの後に再承認の Step へ移り、はいで進み、直す・いいえは拒否になる。承認の部品と ST-2.6。TERM-29・TERM-28。TASK-448 2/2）・approval-flow（承認の操作のログ文言と CH-2 相当の完走。TERM-27）・input-ports（入力方式ごとに選択肢・承認のボタンが切り替わり、ui_action の Step は左の行のクリックで受付になる。TERM-23。TASK-450 1/3）・input-field-hidden（入力欄なしの教材で入力欄・送信・補完の候補・OS の選択が 0 件で、停止と選択肢・画面操作・承認・確認は出て最後まで進む。PACK-20・TERM-23。TASK-450 2/3）・browser-notice（Chrome 以外・旧版の Chrome で案内が 1 件出て閉じられ、再読み込みでも再表示されない。Chrome 153 では出ない。SHELL-30・D-74。TASK-544）・modifier-key-hint（User-Agent から推定した OS ごとに貼り付けの修飾キー Cmd／Ctrl の案内が合い、推定できなければ OS を選ぶよう促し Cmd・Ctrl を出さず、選ぶ欄で切り替えると案内が同じノードのまま変わり、入力欄なしの教材では案内も選ぶ欄も 0 件。JUDGE-28・D-48。TASK-285 1/2・2/2）・op-guide（操作プロファイルの説明 `op-guide` の修飾キーが推定した OS と選ぶ欄で Cmd／Ctrl に切り替わり、選ぶ欄が名前を持ち Tab で届く。入力欄なしでは 0 件。A11Y-25・D-79。TASK-392 1/2）・modifier-key-right-pane（案内と選ぶ欄が右のエリアの中で働き、幅 360px のタブの往復で選んだ値が保たれ、Enter・Shift+Enter・Tab・Esc の結果が OS によらず同じ。TERM-38・JUDGE-28・D-48。TASK-456）・ch0-3-run（CH-0〜CH-3 の形の 20 Step を `?fixture=ch0-3` で 1 つのページに開いて最後まで進め、CH-1 の 6 Step を含む区間（st-0-1 から st-2-1）をキーボードだけで完走する。TERM-23。TASK-450 3/3）・ch4-9-run（CH-4〜CH-9 の形の 17 Step を `?fixture=ch4-9` で CH-9 の ST-9.1 まで通す。試験の設定の通し・配信する設定を DOM だけの観察で通す試験・DOM の観察の較正・パスワード欄の検出器の対照。ページ内エラー 0 件・「次の章へ進む」が各章の後に 1 個（5 回）・パスワード欄 0 件。ROUND1-1・ART-24・SHELL-10。TASK-674 1/2）・next-chapter（「次の章へ進む」だけで CH-0 から CH-3 まで通し、CH-1 の途中までをキーボードだけで越え、越えた後の再読み込みで同じ Step に戻る。SHELL-10・INTRO-3。TASK-526 2/2）・tree-attrs・list-parent-attrs・list-fold（ログが 1,000 項目を超える入力で 1 リストが 1,000 項目以下に畳まれ、親が同じ要素のまま。FEND-22。TASK-360 1/2）・artifact-limit（左の成果物の 4,095・4,096・4,097 項目と、境目をまたぐ入力と、古い側の畳みで、状態の行と root の非置換を確かめる。ART-14・FEND-22。TASK-484 2/2）・log-limit（右のログの元の行が 4,096 件を超える入力でも 1,000 項目に畳まれ全再描画が起きない。TERM-35・FEND-22。TASK-454 2/2）・confirm-burst・stop-move-ignore・esc-outcomes（Esc の画面の結果。TERM-7。処理中は候補が開いていても停止し候補も閉じる・処理中でなく候補が開いていれば閉じるだけ・どちらでもなければ取り消しの入力で、受け付けない Step では拒否の行と次の行動が出る。TERM-8）・reload-ignore・reload-notice・restore-cancel・candidates・step-status（受付済み（確認待ち）の表示と確認を省く Step の受付と同時の達成。TERM-31。TASK-435 1/2）・confirm-status（指す・答える・承認するの 3 種の確認の後にだけ達成が出る。TERM-31。TASK-435 2/2）・history-export・history-import・history-ops（履歴の書き出しと読み込みの操作を枠に置く。書き出しのボタンで現れるリンクのダウンロードを Playwright の `Download` で受け、別のオリジンで `shell-import` の input へ `setInputFiles` で渡すと開く。ファイル画面を選ばずに閉じても読み込みの結果・通知が変わらず、書き換えた・空のファイルは開かず通知が出る。配信する設定でも画面の操作だけで書き出しの文が出る。SHELL-15。TASK-531 1/2）・受講の途中の履歴を別のオリジン（名前の違うサブパス・ルート配信）の別の context で読み込むと同じ Step・世界状態・復習・ログ・チェックポイント・画面になり、続きの操作の結果と書き出しのバイト列が一致する試験（history-continue。SHELL-15。TASK-531 2/2。`docs/testing.md` 5.244）・artifact-view-check（左のビュー共通の検査器の自己確認。TASK-471）・artifact-view-controls（検査器の負の対照 3 種。印を消す・部品を項目の外に置く・innerHTML。TASK-471）・diff-view（差分 (b) の本体。インデント・行番号と記号の列・2 層の印。TASK-475 1/2。(a) の対照とインデントが畳まれる観察。TASK-475 2/2）・doc-view（文書 (b) の本体。見出し・一覧・表・引用・行内のコードの構造が保たれる。TASK-476 1/2。`-`/`+` の組・(a) の対照と構造喪失の観察。TASK-476 2/2）・card-view（カード (a)・(b) の本体と 20 件の規模。(a) は card_basic と状態・ラベルの tag で印は内側の div、(b) は article と marker で印は li 自身。ART-1・ART-17。TASK-477）・card-row-mark（カードの変化の印が行単位で、状態が下書きからオープンに変わっても欄の要素に印が出ず、欄の属性が変化の有無で変わらない観察の固定。欄単位の印を注入する負の対照つき。ART-17・D-49。TASK-487）・check-view（チェックの一覧 (a)・(b) の本体と 20 件の規模。(a) は rich_text・記号の span・状態の tag で印は内側の div、(b) は li 自身が印の宿主で marker のバッジ。状態は記号つきの文字。ART-1・ART-17。TASK-478）・app-demo-view（アプリのデモのワイヤー (a)・(b) の本体。(a) は text・tag・rich_text と input・button・card_basic で描き印は内側の div、(b) は headless-ui の field（読み取り専用）と自作の span・button で描き印は li 自身で role・aria- を出さない。タブの li は app.css で横に並ぶ。ART-1・ART-17。TASK-479）・app-demo-login（模擬ログインのデータで、アプリのデモ (a)・(b) のパスワード欄と文言が 0 件。名前の行を指すだけで進む。データと DOM の負の対照つき。ART-24。TASK-492）・point-rows（7 種のビューの指せる行が 4 属性を組で持ち、宛先の無い行は持たない。指せる行はフォーカスを受け 24x24 以上。クリックの効果は確かめない。ART-11。TASK-480 1/2）・point-click（指せる行のクリックが指す操作として受け付けられ、表示だけの行は何も受け付けない。アプリのデモの名札の行 screen/top だけが指せる行になる。ART-11。TASK-480 2/2）・point-keys（指せる行にフォーカスして Enter・Space を押すと click と同じ入力になる。押しっぱなしは 1 回・修飾キー付きは何もしない。A11Y-18・FEND-13。TASK-358 1/2）・keyboard-confirm（Tab・文字・Enter・Space だけで、確認を含む Step を達成まで進める。製品の左の成果物の行が指せる行になる。CSP あり・なし。A11Y-18・FEND-13。TASK-358 2/2）・task-tree（タスクツリー (a)・(b) の本体。(b) は木の属性、(a) はチェックと深さのクラス、どちらも状態の記号と文字。TASK-474）・artifact-mark-toggle（前後の切り替えで行も項目の内側も置き換わらないこと・印の文字を消す対照・切り替えて戻す対照。TASK-469 2/2）・view-switch（ビューの切り替え・更新で根の要素とフォーカスが保たれる・選択の属性の 101 回・部品を項目の外に置く対照。ART-5・ART-12。TASK-470 2/2）・ime（CDP による IME の模擬のヘルパの自己確認）・ime-input（入力欄と IME の層 3。BUILD-10。誤送信 0・送信漏れ 0。CDP の模擬）・history-versions（保存・復元・書き出し・版の層 3。BUILD-10。往復・読み込み後の保存と復帰・版の判定・壊れた履歴の拒否）・view-only（表示だけで開いた履歴の理由が通知欄に出て、入力・保存・停止の操作が状態・ログを変えない。読み込みと起動時の復元。VER-8。TASK-176 1/2）・view-only-storage（表示だけで開いた履歴に入力・クリック・保存・停止を送っても保存先の値が変わらず、書き出したファイルが読み込んだ履歴とバイト単位で同一。localStorage・IndexedDB・表の view_only の全行・開ける履歴の対照つき。SAVE-25。TASK-335）・refused-history（開かない履歴の理由が枠の通知欄に出て、状態・ログ・入力欄・保存先が変わらず、起動時の復元では閉じられない通知が出る。読み込みは表の refused の 3 行。VER-8。TASK-176 2/2）・ops-limit（`ops` の件数の上限 20,000 件。上限ちょうどの実操作の履歴は開き、1 件多い・10 倍は too_many_ops が 1 秒以内に出て状態・保存先が変わらず、保存先の 19,990 件は再読み込みから 1 秒以内に開く。SAFE-19・SAFE-41。TASK-156。`docs/testing.md` 5.374）・version-notice（変換して開いた履歴で枠の通知欄に通知が 1 件出て、閉じると消える。変換なし・表示だけでは出ない。CSP あり・なし。SHELL-20。TASK-535 1/2。変換して開いた後の操作の保存が開いた側の新しい版で書かれる。TASK-535 2/2）・version-histories（版の違う履歴 15 件を開き、種類・理由・軸が期待の表と合い、開く 7 件は写しが一致し、開かない 8 件は実行環境の差し替えが 0 回で理由が空でない。VER-2。TASK-175 2/2）・reload-fresh（保存済みの履歴が無い場合と未操作の再読み込みで新しく始まり世代が上がらない。操作後は復元される対照つき。SAVE-3・SAVE-28）・submit-disabled（送信ボタンが無効なときの Enter は 0 件・有効なときは click と同じ。FEND-14。TASK-352 2/3）・reload-points（8 Step の教材を `?fixture=reload-points` で起動の前に選び、実際の `page.reload()` で 15 地点の一致と完走した世界状態の一致を確かめる。TASK-319 3/3。SAVE-3）。transition-limit（巡回する IR 3 本が遷移 1,000 回で止まり警告が 1 件だけ残る・止めた後の入力でタブが落ちない・巡回しない長い連鎖は 1,000 回ちょうどなら通り 1,001 回で止まる。SAFE-23。TASK-160 1/3。止めたときの専用の見出しの行・ログの通知欄の理由・着地を行に並べないまとめ表示。SAFE-34。TASK-160 2/3。止めた後は操作の前の Step に戻り次の入力が受け付けられる。SAFE-35。TASK-160 3/3）。element-identity（入力・進行・保存・確認の間 root の全置換が 0 回で、参照を保持した 18 要素が同一のまま入力欄のフォーカスと入力途中の値が保たれる。FEND-3。B-7 と、全再描画で保持した 6 要素が残らずフォーカスが失われる負の対照 B-7n。TASK-348 2/3・3/3）。SAFE-9・SAFE-12・SAFE-23・EVT-3・EVT-7・EVT-8・EVT-9・EVT-10・EVT-11・EVT-12・EVT-18・SAVE-6・FEND-15・TERM-1・TERM-2・TERM-21）、
# CSP の meta の配置と内容の検査（csp-placement。SAFE-13。TASK-151 1/2。ブラウザ不要）、インラインを使わない画面の検査
# （inline-free。SAFE-15。TASK-151 2/2）、画面を開いて操作したときの CSP あり・なしの通信・違反・警告・エラーの計数と要求の種類の一致
# （csp-runs。SAFE-13。TASK-151 2/2。CH-4〜CH-9 の形の 17 Step の通しで章ごとの配信元以外への通信 0 件を数える T5・D2 を含む。
# TASK-674 2/2。docs/testing.md 5.285）を含む。
# 受講者の入力・履歴を外部へ送らないことの通信の計数（egress-count と記録器の単体試験 egress。SAFE-16。TASK-153。docs/testing.md 5.371）を含む。
# 実際の画面への対照の自己確認（safety-control。SAFE-8。CSP あり・なし）と、信頼できない入力への安全性の観察の記録
# （safety-observe。BUILD-4。TASK-79 2/2。観察値は assert せず diagnostic に残し、完走して終了コード 0 になることを関門にする）を含む。
# サブパス配信の構成要素と相対パスの参照の検査（delivery-subpath。DLV-1・DLV-2。TASK-106 1/3。取得資源が構成要素だけで、壊した写しの対照で不合格になる）と、
# 章への直接リンク 10 件のサブパス配信の検査（delivery-direct-links。DLV-1。TASK-106 2/3。query の 1 形で取得資源・警告 0 件・要求の集合の一致）と、名前の違うサブパス 2 か所とルート配信の検査（delivery-bases。DLV-2。TASK-106 3/3）を含む。
# 画面の枠の試験（shell-columns・shell-tabs・shell-boundary（959px と 960px の境目・Tab での到達・キーだけの CH-1。TASK-369）・shell-widths・shell-mock・shell-position。SHELL-1・A11Y-5。TASK-517 1/2・2/2。幅 960px 以上の 2 列・960px 未満のタブの切り替え・4 つの幅での CH-1 の通し）と、枠の通しの試験の骨格（shell-journey。CH-0〜CH-3 の 20 Step を 1280px と 360px で最後まで通し、左右の取り付け点・Step の題・位置・章の移動を確かめる。SHELL-1・SHELL-7・SHELL-8・SHELL-10。TASK-548 1/2。途中にチェックポイントの保存→復帰・再読み込み・書き出し→別のオリジンでの読み込み・保存の失敗を挟む。TASK-548 2/2）を含む。
# 開始点を指す直接リンク `?start=` の試験（direct-link-start。試験の設定で、開始点 3 件の教材を指定なし・各開始点・一致しない値 10 形で開く。一致で節点が変わり開始点ごとに履歴の鍵が分かれ、不一致は値を出さず保存先を変えず断る。DLV-9・SAFE-33。TASK-109 1/2。記録は docs/testing.md 5.199）と、開始点を増やしても配信物の IR が写しで増えず、同じ IR のどの開始点から開いても履歴の内容のハッシュが同じことの試験（start-count-invariance。旧方式の写しと開始点ごとの別 IR の対照つき。DLV-9。TASK-109 2/2。記録は docs/testing.md 5.201）と、直接リンク 10 件が新しい文脈でその章の開始点で開き、再読み込みの後・1 操作して受付済みにした後の再読み込みでも同じ状態になる試験（direct-link-reload。開始点 10 件の教材 `?fixture=ch0-3-links`。DLV-7。TASK-110 1/2。記録は docs/testing.md 5.204）と、章 A を進めて保存した後に章 B のリンクを開いても A の履歴が消えず通知が出ず、A に戻ると続きが出る試験（direct-link-roundtrip。同じ保存領域での往復と、鍵が章で分かれない症状の写しによる対照つき。DLV-8。TASK-110 2/2。記録は docs/testing.md 5.206）を含む。
# 10MB に近い履歴で秘密情報の検査を含む保存が変更から 1 秒以内に収まることの測定（secret-scan-time。SAFE-31。TASK-166。値は diagnostic に残し、記録は docs/testing.md 5.186）を含む。
# compile した IR の手元の配信と章の完走の試験（author-preview。形式 B の試験用教材を compile → IR_FILE を指定したビルド → サブパス配信 → CH-0〜CH-3 の完走。
# 配信元以外への通信 0 件・ページ内エラー 0 件。AUTHOR-21。TASK-427 1/2。記録は docs/testing.md 5.292）を含む。
# 操作要素の `data-payload` の書き換えの試験（data-payload-tamper。実行 0・外部通信 0・形の合わない値は状態を変えない・タブが落ちない。値の読み込みは data-payload-cases。SAFE-6。TASK-148 1/2。記録は docs/testing.md 5.369）を含む。
# IR の大きさの上限の境界の試験（ir-size-limit。配信する設定の画面で、上限ちょうどは読め、+1 バイトと 10 倍は理由つきで断り、取得の終わりから 1 秒未満・タブが落ちない。SAFE-17・SAFE-45。TASK-154。docs/testing.md 5.373）を含む。
# 構造の合わない巨大な入力の拒否の試験（shape-mismatch。100 万・500 万要素の配列・巨大な文字列・連想配列を含む IR と履歴が malformed・missing_version で断られタブが落ちず、100,000 要素・文字の形の合うものは開く。SAFE-24。TASK-161。docs/testing.md 5.375）を含む。
# 断った後に保存済みの履歴と画面が書き換わらないことの試験（refusal-keeps-saved。履歴・起動時の IR・起動後の IR・保存先からの起動の 4 経路で、保存先の全体・状態・画面が変わらず入力の番兵が出ない。SAFE-40。TASK-169。docs/testing.md 5.378）。
# 前提: version-histories は core の試験（crates/core/tests/version_histories.rs）が書き出した履歴を FANDHE_VERSION_HISTORIES_DIR で受ける。
# 前提バイナリ: fandhe-pack の debug ビルドはこのスクリプトが行い（scripts/build-pack-bin.sh。位置は cargo の出力から解決するため
# CARGO_TARGET_DIR に追従する）、FANDHE_PACK_BIN で node:test へ渡す（pack-bin。BUILD-16。TASK-91）。
# 前提: `npm ci` と `npx playwright install chromium` の済み。導入はここでは試みない。
# Chrome の原本で流すときは FANDHE_BROWSER_CHANNEL=chrome を付ける（既定は同梱 Chromium）。Chrome の安定版は別に導入する
# （`npx playwright install chrome`）。扱いは README の「Chrome の原本と Chromium の扱い」、記録は docs/testing.md 5.128。
# 任意: FANDHE_TEST_TAP_OUT=<ファイル> を渡すと、画面の spec 出力に加えて TAP をそのファイルへも書く。件数の照合
# （scripts/check-frontend-bump.sh counts browser。FEND-17 (2)・TASK-101）が読む。未指定なら node --test の引数は従来のまま。
# CI では browser-test ジョブが流す（TASK-79。Playwright とブラウザの導入はジョブ側の step が行う）。
#
# 終了コード: 0 = 全試験が通過 / 1 = 失敗・前提の不足 / 2 = 誤用（引数あり・不正な channel）。
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=./lib.sh
source "${DIR}/lib.sh"

reject_args "$(basename "$0")" "$@"

case "${FANDHE_BROWSER_CHANNEL:-}" in
  '' | chromium | chrome) ;;
  *)
    echo "error: FANDHE_BROWSER_CHANNEL must be chromium or chrome" >&2
    exit 2
    ;;
esac

for cmd in node cargo wasm-bindgen jq mktemp rm; do
  require_cmd "${cmd}"
done

if [[ ! -d "${ROOT_DIR}/node_modules/playwright" ]]; then
  echo "error: playwright is not installed; run npm ci && npx playwright install chromium" >&2
  exit 1
fi

TMP="$(mktemp -d "${TMPDIR:-/tmp}/fandhe-test-browser.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

log "building fandhe-pack (debug) for browser tests"
FANDHE_PACK_BIN="$("${DIR}/build-pack-bin.sh")"
export FANDHE_PACK_BIN

log "building web into a temporary directory"
# 親環境の TEST_HOOKS を継承すると試験設定が生成されるため、空にして配信設定へ固定する。
TEST_HOOKS="" OUT_DIR="${TMP}/web" SKIP_WASM_OPT=1 "${ROOT_DIR}/tools/build-web.sh" >/dev/null

log "building web (test hooks) into a temporary directory"
TEST_HOOKS=1 OUT_DIR="${TMP}/web-test" SKIP_WASM_OPT=1 "${ROOT_DIR}/tools/build-web.sh" >/dev/null

# 作成者の手順のビルド（TASK-427 1/2。author-preview）。形式 B の試験用教材を compile し、その出力を IR_FILE に渡して配信する設定でビルドする。
# 教材の IR をコミットせず毎回 compile するのは、作成者の手順そのものを流すため。ビルドは試験の外で行う（計時の試験に負荷を与えない）。
log "compiling the CH-0..CH-3 authoring pack and building the delivery config with IR_FILE"
"${FANDHE_PACK_BIN}" compile "${ROOT_DIR}/crates/web/tests/data/ch0_3_pack" -o "${TMP}/author-ir.json" >/dev/null
TEST_HOOKS="" OUT_DIR="${TMP}/web-author" IR_FILE="${TMP}/author-ir.json" SKIP_WASM_OPT=1 "${ROOT_DIR}/tools/build-web.sh" >/dev/null

# 版の違う履歴 15 件と開く側の教材を、native の試験と同じ作り方で書き出す（version-histories。VER-2）。
# 書き出しの試験は #[ignore] で、ここだけが --ignored で呼ぶ。15 件がそろわなければ失敗にする（fail-closed）。
log "emitting version histories for browser tests"
mkdir -p "${TMP}/version-histories"
# cargo は cwd の Cargo.toml を探すため、呼び出し元の cwd に依存しないようサブシェルで ROOT_DIR へ移して実行する。
(cd "${ROOT_DIR}" && FANDHE_VERSION_HISTORIES_OUT="${TMP}/version-histories" cargo test --locked -p fandhe-workshop-core \
  --test version_histories -- --ignored --exact emit_histories_for_browser >/dev/null)
emitted="$(find "${TMP}/version-histories" -maxdepth 1 -name '*.history.json' | wc -l | tr -d ' ')"
if [[ "${emitted}" != "15" ]]; then
  echo "error: expected 15 emitted histories, got ${emitted}" >&2
  exit 1
fi
export FANDHE_VERSION_HISTORIES_DIR="${TMP}/version-histories"

log "emitting the round hand-off inputs (IR, histories, round 2 plan) for browser tests"
mkdir -p "${TMP}/round-handoff"
(cd "${ROOT_DIR}" && FANDHE_ROUND_HANDOFF_OUT="${TMP}/round-handoff" cargo test --locked -p fandhe-workshop-web \
  --test round_begin_host -- --ignored --exact emit_round_handoff_for_browser >/dev/null)
for f in ir.json round1-end.history.json round1-half.history.json round1-plan.json round2-plan.json expect.json; do
  if [[ ! -s "${TMP}/round-handoff/${f}" ]]; then
    echo "error: round hand-off input is missing: ${f}" >&2
    exit 1
  fi
done
export FANDHE_ROUND_HANDOFF_DIR="${TMP}/round-handoff"

# 空配列の展開は bash 3.2 の set -u で未定義扱いになるため、要素がある場合だけ渡す。
REPORTER_ARGS=()
if [[ -n "${FANDHE_TEST_TAP_OUT:-}" ]]; then
  REPORTER_ARGS=(--test-reporter=spec --test-reporter-destination=stdout \
    --test-reporter=tap "--test-reporter-destination=${FANDHE_TEST_TAP_OUT}")
fi

FANDHE_WEB_DIR="${TMP}/web" FANDHE_WEB_TEST_DIR="${TMP}/web-test" \
  FANDHE_WEB_AUTHOR_DIR="${TMP}/web-author" FANDHE_AUTHOR_IR="${TMP}/author-ir.json" node --test ${REPORTER_ARGS[@]+"${REPORTER_ARGS[@]}"} \
  "${ROOT_DIR}/tools/browser/harness.test.mjs" \
  "${ROOT_DIR}/tools/browser/pack-bin.test.mjs" \
  "${ROOT_DIR}/tools/browser/safety.test.mjs" \
  "${ROOT_DIR}/tools/browser/safety-control.test.mjs" \
  "${ROOT_DIR}/tools/browser/csp-placement.test.mjs" \
  "${ROOT_DIR}/tools/browser/inline-free.test.mjs" \
  "${ROOT_DIR}/tools/browser/csp-runs.test.mjs" \
  "${ROOT_DIR}/tools/browser/egress.test.mjs" \
  "${ROOT_DIR}/tools/browser/egress-count.test.mjs" \
  "${ROOT_DIR}/tools/browser/smoke-hello.test.mjs" \
  "${ROOT_DIR}/tools/browser/delivery-subpath.test.mjs" \
  "${ROOT_DIR}/tools/browser/delivery-direct-links.test.mjs" \
  "${ROOT_DIR}/tools/browser/delivery-bases.test.mjs" \
  "${ROOT_DIR}/tools/browser/wasm-mime.test.mjs" \
  "${ROOT_DIR}/tools/browser/shell-columns.test.mjs" \
  "${ROOT_DIR}/tools/browser/shell-tabs.test.mjs" \
  "${ROOT_DIR}/tools/browser/shell-boundary.test.mjs" \
  "${ROOT_DIR}/tools/browser/shell-widths.test.mjs" \
  "${ROOT_DIR}/tools/browser/shell-mock.test.mjs" \
  "${ROOT_DIR}/tools/browser/shell-position.test.mjs" \
  "${ROOT_DIR}/tools/browser/shell-journey.test.mjs" \
  "${ROOT_DIR}/tools/browser/step-body.test.mjs" \
  "${ROOT_DIR}/tools/browser/doc-body.test.mjs" \
  "${ROOT_DIR}/tools/browser/doc-body-limits.test.mjs" \
  "${ROOT_DIR}/tools/browser/left-pane.test.mjs" \
  "${ROOT_DIR}/tools/browser/theme-marks.test.mjs" \
  "${ROOT_DIR}/tools/browser/author-preview.test.mjs" \
  "${ROOT_DIR}/tools/browser/versioned-url.test.mjs" \
  "${ROOT_DIR}/tools/browser/keydown-action.test.mjs" \
  "${ROOT_DIR}/tools/browser/esc-attr.test.mjs" \
  "${ROOT_DIR}/tools/browser/esc-once.test.mjs" \
  "${ROOT_DIR}/tools/browser/input-limit.test.mjs" \
  "${ROOT_DIR}/tools/browser/paste.test.mjs" \
  "${ROOT_DIR}/tools/browser/paste-notice.test.mjs" \
  "${ROOT_DIR}/tools/browser/learner-text.test.mjs" \
  "${ROOT_DIR}/tools/browser/enter-submit.test.mjs" \
  "${ROOT_DIR}/tools/browser/key-invariants.test.mjs" \
  "${ROOT_DIR}/tools/browser/field-space.test.mjs" \
  "${ROOT_DIR}/tools/browser/submit-disabled.test.mjs" \
  "${ROOT_DIR}/tools/browser/submit-wiring.test.mjs" \
  "${ROOT_DIR}/tools/browser/tab-complete.test.mjs" \
  "${ROOT_DIR}/tools/browser/shift-tab.test.mjs" \
  "${ROOT_DIR}/tools/browser/tab-cycle.test.mjs" \
  "${ROOT_DIR}/tools/browser/esc-key.test.mjs" \
  "${ROOT_DIR}/tools/browser/json-depth.test.mjs" \
  "${ROOT_DIR}/tools/browser/json-load-order.test.mjs" \
  "${ROOT_DIR}/tools/browser/doc-links.test.mjs" \
  "${ROOT_DIR}/tools/browser/doc-tree.test.mjs" \
  "${ROOT_DIR}/tools/browser/tick-interval.test.mjs" \
  "${ROOT_DIR}/tools/browser/tick-dispatch.test.mjs" \
  "${ROOT_DIR}/tools/browser/three-views.test.mjs" \
  "${ROOT_DIR}/tools/browser/xss-cases.test.mjs" \
  "${ROOT_DIR}/tools/browser/log-lines-xss.test.mjs" \
  "${ROOT_DIR}/tools/browser/pack-body-xss.test.mjs" \
  "${ROOT_DIR}/tools/browser/e2-ir.test.mjs" \
  "${ROOT_DIR}/tools/browser/structure-xss.test.mjs" \
  "${ROOT_DIR}/tools/browser/e3-ir.test.mjs" \
  "${ROOT_DIR}/tools/browser/practice-data-xss.test.mjs" \
  "${ROOT_DIR}/tools/browser/data-payload-cases.test.mjs" \
  "${ROOT_DIR}/tools/browser/data-payload-tamper.test.mjs" \
  "${ROOT_DIR}/tools/browser/input-field-xss.test.mjs" \
  "${ROOT_DIR}/tools/browser/url-cases.test.mjs" \
  "${ROOT_DIR}/tools/browser/url-xss.test.mjs" \
  "${ROOT_DIR}/tools/browser/error-inputs.test.mjs" \
  "${ROOT_DIR}/tools/browser/reject-lines.test.mjs" \
  "${ROOT_DIR}/tools/browser/reject-keeps-draft.test.mjs" \
  "${ROOT_DIR}/tools/browser/clarify-draft.test.mjs" \
  "${ROOT_DIR}/tools/browser/reject-draft-select.test.mjs" \
  "${ROOT_DIR}/tools/browser/draft-no-concat-screen.test.mjs" \
  "${ROOT_DIR}/tools/browser/esc-guide-st14.test.mjs" \
  "${ROOT_DIR}/tools/browser/modifier-enter-paste-st14.test.mjs" \
  "${ROOT_DIR}/tools/browser/choice-answer.test.mjs" \
  "${ROOT_DIR}/tools/browser/choice-labels.test.mjs" \
  "${ROOT_DIR}/tools/browser/read-only-step.test.mjs" \
  "${ROOT_DIR}/tools/browser/approval-buttons.test.mjs" \
  "${ROOT_DIR}/tools/browser/approval-revise-labels.test.mjs" \
  "${ROOT_DIR}/tools/browser/approval-reapprove.test.mjs" \
  "${ROOT_DIR}/tools/browser/approval-flow.test.mjs" \
  "${ROOT_DIR}/tools/browser/input-ports.test.mjs" \
  "${ROOT_DIR}/tools/browser/input-field-hidden.test.mjs" \
  "${ROOT_DIR}/tools/browser/modifier-key-hint.test.mjs" \
  "${ROOT_DIR}/tools/browser/modifier-key-right-pane.test.mjs" \
  "${ROOT_DIR}/tools/browser/op-guide.test.mjs" \
  "${ROOT_DIR}/tools/browser/browser-notice.test.mjs" \
  "${ROOT_DIR}/tools/browser/ch0-3-run.test.mjs" \
  "${ROOT_DIR}/tools/browser/l2-journey-calibrate.test.mjs" \
  "${ROOT_DIR}/tools/browser/ir-recv-limit.test.mjs" \
  "${ROOT_DIR}/tools/browser/ir-size-limit.test.mjs" \
  "${ROOT_DIR}/tools/browser/shape-mismatch.test.mjs" \
  "${ROOT_DIR}/tools/browser/refusal-keeps-saved.test.mjs" \
  "${ROOT_DIR}/tools/browser/next-chapter.test.mjs" \
  "${ROOT_DIR}/tools/browser/ch4-9-run.test.mjs" \
  "${ROOT_DIR}/tools/browser/round-handoff.test.mjs" \
  "${ROOT_DIR}/tools/browser/out-of-range-chapter.test.mjs" \
  "${ROOT_DIR}/tools/browser/chapter-select-absent.test.mjs" \
  "${ROOT_DIR}/tools/browser/tree-attrs.test.mjs" \
  "${ROOT_DIR}/tools/browser/list-parent-attrs.test.mjs" \
  "${ROOT_DIR}/tools/browser/list-fold.test.mjs" \
  "${ROOT_DIR}/tools/browser/artifact-limit.test.mjs" \
  "${ROOT_DIR}/tools/browser/log-limit.test.mjs" \
  "${ROOT_DIR}/tools/browser/confirm-burst.test.mjs" \
  "${ROOT_DIR}/tools/browser/stop-move-ignore.test.mjs" \
  "${ROOT_DIR}/tools/browser/reload-ignore.test.mjs" \
  "${ROOT_DIR}/tools/browser/reload-notice.test.mjs" \
  "${ROOT_DIR}/tools/browser/restore-cancel.test.mjs" \
  "${ROOT_DIR}/tools/browser/checkpoint-restore.test.mjs" \
  "${ROOT_DIR}/tools/browser/checkpoint-list.test.mjs" \
  "${ROOT_DIR}/tools/browser/candidates.test.mjs" \
  "${ROOT_DIR}/tools/browser/storage-select.test.mjs" \
  "${ROOT_DIR}/tools/browser/artifact-view-check.test.mjs" \
  "${ROOT_DIR}/tools/browser/artifact-view-controls.test.mjs" \
  "${ROOT_DIR}/tools/browser/view-escape.test.mjs" \
  "${ROOT_DIR}/tools/browser/artifact-mark-toggle.test.mjs" \
  "${ROOT_DIR}/tools/browser/file-tree-view.test.mjs" \
  "${ROOT_DIR}/tools/browser/task-tree.test.mjs" \
  "${ROOT_DIR}/tools/browser/diff-view.test.mjs" \
  "${ROOT_DIR}/tools/browser/doc-view.test.mjs" \
  "${ROOT_DIR}/tools/browser/card-view.test.mjs" \
  "${ROOT_DIR}/tools/browser/card-row-mark.test.mjs" \
  "${ROOT_DIR}/tools/browser/check-view.test.mjs" \
  "${ROOT_DIR}/tools/browser/app-demo-view.test.mjs" \
  "${ROOT_DIR}/tools/browser/app-demo-login.test.mjs" \
  "${ROOT_DIR}/tools/browser/view-scale.test.mjs" \
  "${ROOT_DIR}/tools/browser/point-rows.test.mjs" \
  "${ROOT_DIR}/tools/browser/point-click.test.mjs" \
  "${ROOT_DIR}/tools/browser/point-keys.test.mjs" \
  "${ROOT_DIR}/tools/browser/keyboard-confirm.test.mjs" \
  "${ROOT_DIR}/tools/browser/view-switch.test.mjs" \
  "${ROOT_DIR}/tools/browser/product-redraw.test.mjs" \
  "${ROOT_DIR}/tools/browser/element-identity.test.mjs" \
  "${ROOT_DIR}/tools/browser/storage-leave.test.mjs" \
  "${ROOT_DIR}/tools/browser/save-latency.test.mjs" \
  "${ROOT_DIR}/tools/browser/secret-scan-time.test.mjs" \
  "${ROOT_DIR}/tools/browser/secret-warning.test.mjs" \
  "${ROOT_DIR}/tools/browser/secret-hold.test.mjs" \
  "${ROOT_DIR}/tools/browser/secret-choice.test.mjs" \
  "${ROOT_DIR}/tools/browser/save-quota.test.mjs" \
  "${ROOT_DIR}/tools/browser/stale-tab.test.mjs" \
  "${ROOT_DIR}/tools/browser/save-idb-failure.test.mjs" \
  "${ROOT_DIR}/tools/browser/storage-lost.test.mjs" \
  "${ROOT_DIR}/tools/browser/chapter-keys.test.mjs" \
  "${ROOT_DIR}/tools/browser/direct-link-start.test.mjs" \
  "${ROOT_DIR}/tools/browser/ir-location.test.mjs" \
  "${ROOT_DIR}/tools/browser/start-count-invariance.test.mjs" \
  "${ROOT_DIR}/tools/browser/direct-link-reload.test.mjs" \
  "${ROOT_DIR}/tools/browser/direct-link-roundtrip.test.mjs" \
  "${ROOT_DIR}/tools/browser/key-stability.test.mjs" \
  "${ROOT_DIR}/tools/browser/idb-versionchange.test.mjs" \
  "${ROOT_DIR}/tools/browser/history-export.test.mjs" \
  "${ROOT_DIR}/tools/browser/history-import.test.mjs" \
  "${ROOT_DIR}/tools/browser/history-ops.test.mjs" \
  "${ROOT_DIR}/tools/browser/history-continue.test.mjs" \
  "${ROOT_DIR}/tools/browser/run-status.test.mjs" \
  "${ROOT_DIR}/tools/browser/step-status.test.mjs" \
  "${ROOT_DIR}/tools/browser/confirm-status.test.mjs" \
  "${ROOT_DIR}/tools/browser/stop-redo.test.mjs" \
  "${ROOT_DIR}/tools/browser/esc-outcomes.test.mjs" \
  "${ROOT_DIR}/tools/browser/ime.test.mjs" \
  "${ROOT_DIR}/tools/browser/ime-input.test.mjs" \
  "${ROOT_DIR}/tools/browser/history-versions.test.mjs" \
  "${ROOT_DIR}/tools/browser/version-histories.test.mjs" \
  "${ROOT_DIR}/tools/browser/version-notice.test.mjs" \
  "${ROOT_DIR}/tools/browser/view-only.test.mjs" \
  "${ROOT_DIR}/tools/browser/view-only-storage.test.mjs" \
  "${ROOT_DIR}/tools/browser/refused-history.test.mjs" \
  "${ROOT_DIR}/tools/browser/ops-limit.test.mjs" \
  "${ROOT_DIR}/tools/browser/reload-fresh.test.mjs" \
  "${ROOT_DIR}/tools/browser/reload-points.test.mjs" \
  "${ROOT_DIR}/tools/browser/transition-limit.test.mjs" \
  "${ROOT_DIR}/tools/browser/safety-observe.test.mjs" \
  "${ROOT_DIR}/tools/browser/ir-refusal.test.mjs" \
  "${ROOT_DIR}/tools/browser/reduced-motion.test.mjs" \
  "${ROOT_DIR}/tools/browser/reduced-motion-st15.test.mjs" \
  "${ROOT_DIR}/tools/browser/reduced-motion-lo3.test.mjs"
