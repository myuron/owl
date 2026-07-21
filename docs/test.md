# owl — ユニットテスト項目

[設計書](design.md) §14 のテスト方針に基づくユニットテストの項目一覧。対象は GTK 不要で `cargo test` 完結する純粋ロジックのみ:

1. `parse_open_input`(command.rs)— §11 の入力解釈規則
2. キーシーケンスの状態遷移(keys.rs)— §7.3 の `pending_key` の解決

結合・E2E は自動化しない(手動確認は docs/checklist.md に別途まとめる)。

## 1. `parse_open_input`(command.rs)

判定規則(§11)。入力は前後の空白を trim してから上から順に適用する:

```
1. trim 後が空                                          → 無効(None。エラー表示は呼び出し側)
2. ^localhost(:数字ポート)?(/パス)?$                    → http:// を補完
3. スキームあり(^[a-zA-Z][a-zA-Z0-9+.-]*: にマッチ)    → そのまま URL
4. 空白を含まず . を含むホスト名形式                     → https:// を補完
5. それ以外                                             → DuckDuckGo 検索
```

### 1.1 規則 1: 前処理(trim・空入力)

| ID | 入力 | 期待結果 |
|---|---|---|
| P-34 | `""`(空文字列) | `None`(ナビゲーションしない) |
| P-36 | `"   "`(空白のみ) | trim 後が空 → `None` |
| P-35 | `  example.com  `(前後空白) | trim → 規則 4 → `https://example.com` |

### 1.2 規則 2: localhost

| ID | 入力 | 期待結果 |
|---|---|---|
| P-10 | `localhost` | `http://localhost` |
| P-11 | `localhost:8080` | `http://localhost:8080` |
| P-12 | `localhost:8080/path`(ポート以降にパス) | `http://localhost:8080/path` |
| P-13 | `localhost:abc`(ポートが数字でない) | 規則 2 の対象外 → 規則 3 のスキーム正規表現にマッチ → そのまま `localhost:abc`(WebKit 側で読み込み失敗 → エラーページ。特別扱いしない) |
| P-14 | `localhost.example.com` | 規則 2 の対象外 → 規則 4 → `https://localhost.example.com` |
| P-15 | `localhost/path`(ポートなしパス付き) | `http://localhost/path` |

### 1.3 規則 3: スキームあり

| ID | 入力 | 期待結果 |
|---|---|---|
| P-01 | `https://example.com` | そのまま `https://example.com` |
| P-02 | `http://example.com` | そのまま |
| P-03 | `file:///home/user/index.html` | そのまま |
| P-04 | `about:blank` | そのまま |
| P-05 | `git+ssh://host/repo`(スキームに `+` を含む) | そのまま(正規表現 `[a-zA-Z0-9+.-]*` の確認) |
| P-06 | `HTTPS://example.com`(大文字スキーム) | そのまま(スキーム判定は大文字小文字を区別しない。RFC 3986) |

### 1.4 規則 4: ホスト名形式(https:// 補完)

| ID | 入力 | 期待結果 |
|---|---|---|
| P-20 | `example.com` | `https://example.com` |
| P-21 | `sub.domain.co.jp` | `https://sub.domain.co.jp` |
| P-22 | `example.com/path?q=1`(パス・クエリ付き) | `https://example.com/path?q=1` |
| P-23 | `example.com:8443`(ポート付きホスト) | `https://example.com:8443` |
| P-24 | `foo bar.com`(空白を含む) | 規則 4 の対象外 → 規則 5(検索) |

### 1.5 規則 5: DuckDuckGo 検索

| ID | 入力 | 期待結果 |
|---|---|---|
| P-30 | `hello`(`.` なし単語) | `https://duckduckgo.com/?q=hello` |
| P-31 | `hello world`(空白を含む) | `q=hello%20world` 等、空白が URL エンコードされる |
| P-32 | `rust 所有権`(非 ASCII) | クエリが percent-encoding される |
| P-33 | `a&b=c?d`(URL 特殊文字) | `&` `=` `?` がエンコードされ、クエリが壊れない |

### 1.6 規則の優先順位

| ID | 入力 | 期待結果 |
|---|---|---|
| P-40 | `http://example.com`(規則 3 と 4 の両方に該当) | 規則 3 が勝つ(そのまま。二重補完しない) |
| P-41 | `localhost`(規則 2 と 5 の両方に該当しうる) | 規則 2 が勝つ(`http://localhost`) |
| P-42 | `localhost:8080`(規則 2 と 3 の両方に該当。`localhost:` はスキーム正規表現にもマッチ) | 規則 2 が勝つ(`http://localhost:8080`。スキームとして素通ししない) |

### 1.7 `parse_command`(command.rs)— §11 のコマンドディスパッチ

command モード(§11)のコマンドライン入力(先頭 `:`)を `Command` へ分類する純粋関数
`parse_command(&str) -> Command`。`:open <input>` は引数を `parse_open_input`(§1)で補完済み
URL に解決する。エラーメッセージは呼び出し側(ステータスバー)がそのまま表示するため、
文字列を厳密にアサートする(CLAUDE.md 規約 4)。先頭 `:` は 1 個だけ剥がし、trim してから
最初の空白でコマンド名と引数に分割する。コマンド名は大文字小文字を区別する(`:OPEN` は未知)。

| ID | 入力 | 期待結果 |
|---|---|---|
| CMD-01 | `:quit` | `Command::Quit` |
| CMD-02 | `:open example.com` | `Command::Open("https://example.com")`(引数を `parse_open_input` で補完) |
| CMD-03 | `:open`(引数なし) | `Command::Error("usage: open <url or query>")`(`parse_open_input` が `None`) |
| CMD-04 | `:foo`(未知コマンド) | `Command::Error("unknown command: foo")` |
| CMD-05 | `:`(コロンのみ) | `Command::Noop`(何もしない・遷移だけ戻す) |
| CMD-06 | `""`(空文字列) | `Command::Noop`(Entry が空でクリアされた場合の堅牢性) |
| CMD-07 | `quit`(先頭 `:` なし) | `Command::Quit`(`strip_prefix(':')` の非該当分岐) |
| CMD-08 | `:open hello world`(空白入りクエリ) | `Command::Open("https://duckduckgo.com/?q=hello%20world")`(引数が空白を含んでも `parse_open_input` へ渡る) |
| CMD-09 | `:OPEN example.com`(大文字コマンド) | `Command::Error("unknown command: OPEN")`(コマンド名は大文字小文字を区別) |
| CMD-10 | `:quit now`(quit に余分な引数) | `Command::Quit`(quit は引数を無視) |
| CMD-11 | `:  quit  `(前後空白) | `Command::Quit`(先頭 `:` 除去後に trim してから分割) |

### 1.8 M7 堅牢化の純粋ロジック(command.rs)— §8.5・§8.6

WebView 統合(§8)の堅牢化のうち GTK 非依存な文字列組み立てを純粋関数に切り出す。実際の
シグナル結線(`load_alternate_html`・`Download::cancel`・ステータスバー更新)は呼び出し側
(`webview`/`window`)が担い手動確認する(§14・checklist M7)。エスケープ・整形は「分岐は
通るが挙動が誤る」型なので、誤実装で落ちる値を選んでアサートする(CLAUDE.md 規約 4)。

`download_blocked_message(uri) -> String`(§8.5): ブロックしたダウンロードのステータス表示
`download blocked: <ファイル名>`。ファイル名は URI の最後のパスセグメント(クエリ `?`・
フラグメント `#` を除く)。セグメントが空なら URI 全体にフォールバックする。

| ID | 入力 | 期待結果 |
|---|---|---|
| DL-01 | `https://example.com/files/report.pdf` | `download blocked: report.pdf`(最後のセグメント) |
| DL-02 | `https://example.com/a/report.pdf?x=1&y=2` | `download blocked: report.pdf`(クエリを除く) |
| DL-03 | `https://example.com/a/archive.zip#frag` | `download blocked: archive.zip`(フラグメントを除く) |
| DL-04 | `https://example.com/`(末尾スラッシュ・ファイル名なし) | `download blocked: https://example.com/`(空セグメント → URI 全体へフォールバック) |

`error_page_html(kind, url) -> String`(§8.6): 読み込み失敗・クラッシュ時の最小エラーページ。
エラー種別(`kind`)・対象 URL(`url`)・`r` でリロードの案内を含む。`kind`/`url` は HTML
エスケープする(`load_alternate_html` は失敗 URI をオリジンとして描画するため、攻撃者制御の
URL/種別を素の HTML に埋めると XSS になりうる。CLAUDE.md 規約 6 の信頼境界)。

| ID | 入力 | 期待結果 |
|---|---|---|
| ERR-01 | kind=`Could not connect`, url=`https://x.test/` | 両文字列とリロード案内(`reload`)を含む |
| ERR-02 | kind=`<script>&"'`(HTML 特殊文字) | `&lt;script&gt;&amp;&quot;&#39;` を含み、素の `<script>` を**含まない**(全エスケープ分岐を固定) |
| ERR-03 | url=`https://a/?q=1&r=2`(`&` を含む実 URL) | `q=1&amp;r=2` を含む(`&` がエスケープされ属性/実体参照が壊れない) |

`popup_navigation_uri(uri) -> Option<&str>`(§8.4): `window.open`/`target="_blank"` の要求 URI の
うちトップフレームへ遷移してよいものだけを返す。要求 URI はページ(信頼境界の外)が握るため、
`javascript:`/`data:` を拒否する(規約 6)。スキーム判定は大文字小文字を区別しない(RFC 3986)。

| ID | 入力 | 期待結果 |
|---|---|---|
| POP-01 | `https://example.com/x`(通常の遷移先) | `Some("https://example.com/x")`(そのまま許可) |
| POP-02 | `javascript:alert(1)` | `None`(トップフレームへ遷移させない。UXSS 防止) |
| POP-03 | `data:text/html,<script>alert(1)</script>` | `None`(拒否) |
| POP-04 | `JavaScript:…`/`DATA:…`(大文字混在) | `None`(スキームは大文字小文字を区別しない。`eq` に変えると落ちる) |
| POP-05 | `https://data.example.com`・`js`(短い入力) | `Some(...)`(接頭辞ではなくスキームで判定、境界で panic しない) |

## 2. キーシーケンスの状態遷移(keys.rs)

§7.3: Normal モードで `g`・`y` は `pending_key` に記録。次のキーで解決する。GTK イベントに依存しない純粋な状態遷移関数として切り出してテストする。

### 2.1 シーケンス開始

| ID | 事前状態 | 入力 | 期待結果 |
|---|---|---|---|
| K-01 | pending なし | `g` | `pending_key = Some('g')`、アクションなし、キー消費(Stop) |
| K-02 | pending なし | `y` | `pending_key = Some('y')`、アクションなし、Stop |

### 2.2 シーケンス成立

| ID | 事前状態 | 入力 | 期待結果 |
|---|---|---|---|
| K-10 | `Some('g')` | `g` | ページ先頭へスクロール、pending クリア |
| K-11 | `Some('y')` | `y` | URL コピー、pending クリア |

### 2.3 シーケンス破棄と再解釈

破棄後は「そのキーを単独キーとして解釈し直す」(§7.3)。

| ID | 事前状態 | 入力 | 期待結果 |
|---|---|---|---|
| K-20 | `Some('g')` | `j` | 破棄 → `j` を単独実行(下スクロール)、pending クリア |
| K-21 | `Some('y')` | `G` | 破棄 → `G` を単独実行(ページ末尾)、pending クリア |
| K-22 | `Some('g')` | `y` | 破棄 → `y` は単独ではプレフィックスキー → `pending_key = Some('y')` になる |
| K-23 | `Some('y')` | `g` | 破棄 → `pending_key = Some('g')` になる |
| K-24 | `Some('g')` | `q`(未割り当てキー) | 破棄 → 何もしない(Stop)、pending クリア |
| K-25 | `Some('g')` | `i` | 破棄 → Insert モードへ遷移、pending クリア |
| K-26 | `Some('g')` | `:` | 破棄 → Command モードへ遷移、pending クリア |

### 2.4 Esc の排他処理

§7.3: `Esc` は pending があればクリアのみ、なければ読み込み中断。

| ID | 事前状態 | 入力 | 期待結果 |
|---|---|---|---|
| K-30 | `Some('g')` | `Esc` | pending クリアのみ(読み込み中断は呼ばれない) |
| K-31 | pending なし | `Esc` | 読み込み中断(`stop_loading` 相当のアクション) |

### 2.5 モード境界

| ID | 事前状態 | 入力 | 期待結果 |
|---|---|---|---|
| K-40 | Insert モード | `g` | pending に記録されない(Insert では素通し。§7.2) |
| K-41 | `Some('g')` の状態でモード遷移 | (`set_mode` 呼び出し) | `set_mode` が pending をクリアする(§6)。Normal 復帰後の `g` は新規シーケンス開始として扱われる |

### 2.6 修飾キー(Normal、§7.2・§7.4)

`KeyInput::{Ctrl(char), SpecialBare, OtherModified}` を Normal モードで解決する。§7.2:
バインド表にある修飾付きキー(`Ctrl+d`/`Ctrl+u`)のみ Stop、他の修飾付きは Proceed。
修飾なしの未割り当て特殊キー(矢印等 = `SpecialBare`)は消費して Stop(ページに漏らさない。要求 3.3)。
修飾系はシーケンスを破棄する(pending を `None` に。§7.3)。

| ID | 事前状態 | 入力 | 期待結果 |
|---|---|---|---|
| K-50 | pending なし | `Ctrl+d` | 半ページ下スクロール(`ScrollHalfDown`)、Stop、pending なし |
| K-51 | pending なし | `Ctrl+u` | 半ページ上スクロール(`ScrollHalfUp`)、Stop、pending なし |
| K-52 | pending なし | `Ctrl+a`(未割り当ての修飾付き) | アクションなし、**Proceed**(ページへ素通し)、pending なし |
| K-53 | pending なし | `SpecialBare`(矢印等) | アクションなし、**Stop**(消費)、pending なし |
| K-54 | pending なし | `OtherModified`(Alt/Super 等) | アクションなし、**Proceed**、pending なし |
| K-55 | `Some('g')` | `Ctrl+d` | 破棄 → `ScrollHalfDown`、Stop、pending クリア |
| K-56 | `Some('g')` | `SpecialBare` | 破棄 → Stop、pending クリア |
| K-57 | `Some('g')` | `OtherModified` | 破棄 → Proceed、pending クリア |

### 2.7 モード別の非文字キー(§7.2)

`Char`/`Esc` 以外の `KeyInput` 変種も各モードで規定どおり伝播する(CLAUDE.md 規約 2: 相異なるアームを全網羅)。

| ID | 事前状態 | 入力 | 期待結果 |
|---|---|---|---|
| K-60 | Insert モード | `Ctrl+a` | Proceed(修飾付きも素通し。§7.2) |
| K-61 | Insert モード | `SpecialBare` | Proceed |
| K-62 | Insert モード | `OtherModified` | Proceed |
| K-63 | Hint モード | `Ctrl+a` | Stop(すべて Stop。§7.2) |
| K-64 | Hint モード | `SpecialBare` | Stop |
| K-65 | Hint モード | `OtherModified` | Stop |
| K-66 | Command モード | `Ctrl+a`/`SpecialBare`/`OtherModified` | Proceed(Entry が処理。§7.2) |
| K-67 | Hint モード | `Char(c)`(ラベル文字) | `HintInput(c)` を返し **Stop**(ページに漏らさず JS へ転送。§9.1・§9.2)、pending なし |

### 2.8 `classify_input`(GTK キーイベント → `KeyInput`)

GTK の keyval・修飾状態を純粋な入力種別へ分類する(§7.1・§7.2)。GTK 境界の判定を単体テスト可能な
純粋関数に閉じる。SHIFT は keyval 側で文字へ畳み込み済みのため分類に使わない。

| ID | escape / ctrl / other_mod / unicode | 期待結果 |
|---|---|---|
| C-01 | escape=true(ctrl 併用でも) | `Esc`(Escape を最優先) |
| C-02 | other_mod=true | `OtherModified`(Alt/Super/Meta) |
| C-03 | ctrl=true, `Some('d')` | `Ctrl('d')` |
| C-04 | ctrl=true, `Some('D')`(Shift 併用) | `Ctrl('d')`(小文字化する) |
| C-05 | ctrl=true, `None` | `OtherModified`(Ctrl+非文字キー) |
| C-06 | ctrl=true, `Some('4')`(非英字) | `OtherModified` |
| C-07 | 修飾なし, `Some('j')` | `Char('j')` |
| C-08 | 修飾なし, `None` | `SpecialBare`(矢印・Fn 等) |

### 2.9 `scroll_script`(Action → 注入 JS)

スクロール系 Action を注入 JS 文字列へ変換する(§7.4 の量・§8.1 の `behavior:'instant'`)。厳密文字列を
アサートし、丸め/座標/px の誤りを固定する(CLAUDE.md 規約 4)。非スクロール Action は `None`。

| ID | Action | 期待結果(JS 文字列) |
|---|---|---|
| S-01 | `ScrollLeft` | `window.scrollBy({left:-50,top:0,behavior:'instant'})` |
| S-02 | `ScrollRight` | `window.scrollBy({left:50,top:0,behavior:'instant'})` |
| S-03 | `ScrollUp` | `window.scrollBy({left:0,top:-50,behavior:'instant'})` |
| S-04 | `ScrollDown` | `window.scrollBy({left:0,top:50,behavior:'instant'})` |
| S-05 | `ScrollTop` | `window.scrollTo({left:0,top:0,behavior:'instant'})` |
| S-06 | `ScrollBottom` | `window.scrollTo({left:0,top:document.body.scrollHeight,behavior:'instant'})` |
| S-07 | `ScrollHalfDown` | `window.scrollBy({left:0,top:window.innerHeight/2,behavior:'instant'})` |
| S-08 | `ScrollHalfUp` | `window.scrollBy({left:0,top:-window.innerHeight/2,behavior:'instant'})` |
| S-09 | `Back` / `CopyUrl`(非スクロール) | `None` |

### 2.10 `mode_indicator`(Mode → ステータスバー表示)

モードインジケータの表示文字列(§5-2・§12)。Normal は空、他は `-- MODE --` 相当。空文字列も固定する
(非空へ変える mutant を落とす。CLAUDE.md 規約 4)。

| ID | Mode | 期待結果 |
|---|---|---|
| M-01 | `Normal` | `""`(空) |
| M-02 | `Insert` | `-- INSERT --` |
| M-03 | `Command` | `-- COMMAND --` |
| M-04 | `Hint` | `-- HINT --` |

### 2.11 hint モードの純粋ロジック(§9)

Rust ⇔ page.js の通信文字列の組み立て/解釈(`hints.rs`)。ラベル採番・要素列挙・描画は
page.js(JS 側)が担い**ユニットテスト対象外**(§3)。Rust 側の JS 文字列生成とメッセージ解釈のみ
純粋関数として固定する。厳密文字列・全アームをアサートし、誤実装で落ちる値を選ぶ(CLAUDE.md 規約 2・4)。

| ID | 対象 | 入力 | 期待結果 |
|---|---|---|---|
| H-01 | `start_script`/`cancel_script` | — | `owlHints.start()` / `owlHints.cancel()`(厳密一致) |
| H-02 | `input_script` | `'a'` / `'s'` | `owlHints.input('a')` / `owlHints.input('s')` |
| H-03 | `input_script` | `'\''` / `'\\'` | `\'` / `\\` へエスケープ(JS 構文を壊さない。§7.2) |
| H-04 | `input_script` | `\n`/`\r`/`\t`/U+2028/U+2029/`\u{1}`/`\u{1f}` | 各制御文字を `\n`…や 4 桁ゼロ詰め `\uXXXX` へ |
| H-05 | `input_script` | `' '`(U+0020) | エスケープせずそのまま(境界 `< 0x20` を固定) |
| H-10 | `parse_hint_message` | `{"type":"hint_result","target":"link"}` | `Link` |
| H-11 | `parse_hint_message` | `{"type":"hint_result","target":"input"}` | `Input` |
| H-12 | `parse_hint_message` | `{"type":"hint_none"}` | `None` |
| H-13 | `parse_hint_message` | `{"type":"bogus"}`(未知 type) | `Ignore`(未知 type は前方互換で無視) |
| H-14 | `parse_hint_message` | `hint_result` + 未知/欠落 target | `Ignore` |
| H-15 | `parse_hint_message` | `{}`/非 JSON/空/コロン欠落/非文字列値/閉じ引用符欠落 | `Ignore`(全短絡パス) |
| H-16 | `parse_hint_message` | `{ "type" : "hint_none" }` | `None`(`:` 前後の空白を許容) |
| H-17 | `parse_hint_message` | `{"type":"focus","editable":true}` | `Focus`(§10 クリック focus → Insert。focus アームが Ignore に落ちないことを固定) |

## 3. テスト対象外(参考)

以下は §14 によりユニットテストの対象外。手動確認チェックリスト(docs/checklist.md)で扱う:

- モード遷移の副作用(ステータスバー更新、コマンドライン表示、フォーカス移動)
- hint モードの **page.js 連携**(要素列挙・ラベル採番・描画・絞り込み・click/focus 確定)と GTK 結線
  (`owlHints.*` 駆動・script message handler 受信・モード遷移)。Rust 側の純粋ロジック
  (`hints.rs` の JS 文字列生成・メッセージ解釈)は §2.11(H-01〜H-16)で単体テストする
- insert 自動移行(mousedown 相関、autofocus 抑止)
- WebView 統合(TLS Fail、エラーページ、クラッシュ復帰、ポップアップ抑制、ダウンロードキャンセル)
