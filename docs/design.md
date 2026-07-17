# owl — 設計書

[要求文書](requirements.md) の MVP スコープを対象とした設計を記す。要求の背景・理由は要求文書に譲り、本書は「どう作るか」に集中する。

## 1. 設計方針

- **薄い殻に徹する**: owl の実装対象は GTK ウィンドウ・モード管理・キーバインド・コマンドラインのみ。ページに関わる処理(レンダリング、JS 実行、スクロールの実体、ヒントの描画)は WebKitGTK と注入 JavaScript に委譲する。
- **モードを単一の真実とする**: すべてのキー入力の解釈は「現在のモード」だけで決まる。モード状態は Rust 側に一元管理し、ページ側(JS)には持たせない。
- **Rust ⇔ ページの通信は 1 本の細い経路に限定する**: Rust → ページは `evaluate_javascript`、ページ → Rust は `UserContentManager` の script message handler のみ。これ以外の経路(カスタム URI スキーム等)は作らない。

## 2. 技術スタック(確定)

要求文書 6 章の第一候補構成を採用する。事前調査によりバージョン整合を確認済み。

| 層 | 選定 | バージョン |
|---|---|---|
| 言語 | Rust (edition 2024) | — |
| GTK バインディング | `gtk4` crate | ^0.11(`webkit6` の依存に合わせる) |
| WebKit バインディング | `webkit6` crate | 0.6.x(調査時点の最新 0.6.1) |
| ネイティブライブラリ(Nix 供給) | `webkitgtk_6_0` / GTK4 | WebKitGTK 2.52.5 / GTK 4.22.4(flake の nixpkgs) |

- フォールバック(GTK3 + `webkit2gtk`)は**採用しない**ことをここで確定する。バージョン整合は確認済みであり、残るリスクは「`webkit6` crate の利用実績の少なさ」だが、これはマイルストーン 1(スケルトン)の実ビルドで早期検証する(§13)。マイルストーン 1 で致命的問題が出た場合のみフォールバックを再検討し、本書を改訂する。
- 上記以外の依存 crate は原則追加しない。CLI 引数は `std::env::args` で足りるため `clap` は使わない。ロギングが必要になったら `eprintln!` から始める。

## 3. アーキテクチャ

### 3.1 プロセスモデル

WebKitGTK のマルチプロセス構成をそのまま使う。owl 自身(UIProcess)は GTK メインループ上のシングルスレッドで動作し、レンダリングは WebKit が spawn する WebProcess、ネットワークは NetworkProcess が担う。owl はスレッドを作らない。非同期 API(`evaluate_javascript` 等)はすべて GLib メインループのコールバックで受ける。

### 3.2 コンポーネント図

```
┌─ owl (UIProcess) ─────────────────────────────────┐
│  App (gtk::Application)                           │
│   └─ Window (gtk::ApplicationWindow)              │
│       ├─ EventControllerKey (capture phase) ──┐   │
│       ├─ gtk::Box (vertical)                  │   │
│       │   ├─ webkit6::WebView (expand)        │   │
│       │   ├─ StatusBar (gtk::Box + Labels)    │   │
│       │   └─ CommandLine (gtk::Entry, 通常非表示) │
│       └─ ModeState (Rc<RefCell<Mode>>) ◄──────┘   │
│                                                   │
│  Rust → ページ: evaluate_javascript               │
│  ページ → Rust: script message handler ("owl")    │
└───────────────────────────────────────────────────┘
         │ WebKit IPC(owl は関与しない)
   WebProcess / NetworkProcess
```

### 3.3 状態の持ち方

GTK4 はシングルスレッド UI のため、共有状態は `Rc<RefCell<...>>` で持つ。中心となる状態は 1 つの `AppState` にまとめ、各シグナルハンドラへ clone して配る:

```rust
struct AppState {
    mode: Mode,                  // 現在のモード
    pending_key: Option<char>,   // 'g'・'y' 等、シーケンス途中の先行キー
}
```

WebView・ステータスバー等のウィジェット参照はシグナル接続時にクロージャへ個別に clone する(神オブジェクト化を避ける)。

## 4. モジュール構成

```
src/
├── main.rs        // エントリポイント: Application 生成、引数処理
├── window.rs      // ウィンドウ構築: レイアウト、WebView・ステータスバー・コマンドラインの組み立て
├── webview.rs     // WebView 生成と設定: NetworkSession、永続化、各種シグナル(TLS/クラッシュ/ポップアップ/ダウンロード)
├── mode.rs        // Mode enum とモード遷移(遷移時の副作用: ステータスバー更新、コマンドライン表示切替)
├── keys.rs        // EventControllerKey のハンドラ: モード別のキー解釈、キーシーケンス処理
├── command.rs     // command モード: :open のパース、コマンド実行
├── hints.rs       // hint モード: JS 側との連携、ラベル入力の転送
└── page.js        // ページへ注入する UserScript(ヒント描画、focus 検知)— include_str! で埋め込む
```

## 5. UI 構造

`gtk::ApplicationWindow` 直下に縦の `gtk::Box`:

1. **WebView** — `vexpand = true`。ページ表示領域。
2. **ステータスバー** — 高さ 1 行の `gtk::Box`。左から: モードインジケータ(`-- INSERT --` 相当。normal 時は空)、URL、ページタイトル。右端に読み込み状態。それぞれ `gtk::Label`(URL・タイトルは ellipsize)。
3. **コマンドライン** — `gtk::Entry`。command モード時のみ `set_visible(true)` にしてフォーカスを移す。

ツールバー・メニューバーは持たない(要求 5 章)。CSS は最小限(ステータスバーの配色・等幅フォント)を `gtk::CssProvider` でハードコードする。

## 6. モード管理

```rust
enum Mode {
    Normal,
    Insert,
    Command,
    Hint,
}
```

遷移は以下のみ。遷移関数 `set_mode(state, new_mode)` を 1 箇所に置き、ステータスバーのインジケータ更新・コマンドラインの表示/非表示・フォーカス移動・`pending_key` のクリアという副作用をそこへ集約する(モード遷移をまたいでキーシーケンスは持ち越さない)。

| 遷移 | トリガ |
|---|---|
| Normal → Insert | `i`、hint でのテキスト入力欄選択、ユーザー起因 focus 検知(§10) |
| Normal → Command | `:` |
| Normal → Hint | `f` |
| Insert → Normal | `Esc`(ページ側の focus を外す: `document.activeElement.blur()` を評価し、GTK フォーカスを WebView 本体へ戻す) |
| Command → Normal | `Enter`(実行後)/ `Esc`(キャンセル) |
| Hint → Normal | 選択確定(リンクの場合)/ `Esc` |
| Hint → Insert | 選択確定(テキスト入力欄の場合) |

## 7. キー入力処理

### 7.1 横取りの仕組み

`gtk::EventControllerKey` を **ウィンドウに capture phase で** 取り付ける。GTK4 のイベント伝播は capture(親→子)→ target → bubble(子→親)の順なので、capture でウィンドウが WebView より先にキーを見られる。ハンドラの返り値で挙動を決める:

- `glib::Propagation::Stop` — owl が消費。ページには届かない。
- `glib::Propagation::Proceed` — 素通し。通常どおり WebView(またはコマンドラインの Entry)へ届く。

### 7.2 モード別の解釈

| モード | 修飾なしキー | 修飾付きキー |
|---|---|---|
| Normal | バインド表にあれば実行して Stop。**なければ何もせず Stop**(ページに漏らさない。要求 3.3) | バインド表にあるもの(`Ctrl+d`/`Ctrl+u`)のみ Stop、他は Proceed |
| Insert | `Esc` のみ Stop(Normal へ)。他はすべて Proceed | すべて Proceed |
| Command | すべて Proceed(Entry が処理)。`Esc`/`Enter` は Entry 側のシグナルで拾う | 同左 |
| Hint | すべて Stop。ラベル文字は JS へ転送(§9)、`Esc` は Normal へ、他は無視 | Stop(無視) |

### 7.3 キーシーケンス(`gg` / `yy`)

`AppState.pending_key` で処理する。Normal モードで `g`・`y` を受けたら `pending_key` に記録して Stop。次のキーで:

- `pending_key == Some('g')` かつ `g` → ページ先頭へ。`Some('y')` かつ `y` → URL コピー。
- それ以外の組み合わせ → シーケンス破棄し、そのキーを単独キーとして解釈し直す。

タイムアウトは設けない(vim も既定では `timeout` 前提だが、2 打鍵のみなので破棄条件は「次のキー」で十分)。`Esc` は排他的に処理する: `pending_key` が `Some` ならクリアするのみ(読み込み中断はしない)、`None` なら読み込み中断(`stop_loading`)。モード遷移時のクリアは `set_mode` が担う(§6)。

### 7.4 Normal モードのバインド表

| キー | 動作 | 実装 |
|---|---|---|
| `h` `j` `k` `l` | スクロール(左/下/上/右、50px) | JS: `window.scrollBy()`(§8.1) |
| `gg` / `G` | ページ先頭 / 末尾 | JS: `window.scrollTo()` |
| `Ctrl+d` / `Ctrl+u` | 半ページ下 / 上 | JS: `window.scrollBy(0, ±innerHeight/2)` |
| `H` / `L` | 戻る / 進む | `WebView::go_back()` / `go_forward()` |
| `r` | リロード | `WebView::reload()` |
| `yy` | URL コピー | `gdk::Display::clipboard().set_text(uri)` |
| `Esc` | 読み込み中断 | `WebView::stop_loading()` |
| `f` / `i` / `:` | モード遷移 | §6 |

`d` `u` `o` `t` 等、将来のタブ管理・その他拡張と衝突しうるキーには何も割り当てない(無視して Stop)。

## 8. WebView 統合

### 8.1 スクロールの実装方式

スクロールはすべて注入 JS(`window.scrollBy` / `scrollTo`、`behavior: "instant"`)で行う。GTK 側でのスクロール座標操作は WebKitGTK の API 上できないため、JS 委譲が唯一の現実解であり、設計方針(殻に徹する)とも一致する。既知の制限: メインフレームがスクロールしないページ(スクロールコンテナが内部 div のページ)では効かないことがある。MVP では許容し、支障が大きければヒント同様の要素探索で対応を検討する。

### 8.2 データ永続化

`webkit6::NetworkSession::new(data_dir, cache_dir)` で永続セッションを作り、WebView 生成時に紐付ける:

- data: `$XDG_DATA_HOME/owl`(`glib::user_data_dir().join("owl")`)
- cache: `$XDG_CACHE_HOME/owl`

Cookie は `network_session.cookie_manager().set_persistent_storage(path, Sqlite)` で明示的に永続化する。ローカルストレージ等は NetworkSession のデータディレクトリ配下に WebKit が保存する。

### 8.3 TLS エラー

`NetworkSession::set_tls_errors_policy(TLSErrorsPolicy::Fail)` を設定。これにより証明書無効サイトは読み込み失敗となり、`load-failed-with-tls-errors` シグナルで検知してエラーページ(§8.6)を表示する。例外許可の UI は作らない(要求 3.1)。

### 8.4 新規ウィンドウ抑制

`WebView::connect_create`(`target="_blank"` / `window.open` で発火)で、`NavigationAction` から要求 URI を取り出して**現在の WebView で** `load_uri` し、`None` を返す(新規ウィンドウを作らせない)。

### 8.5 ダウンロードのキャンセル

`NetworkSession::connect_download_started` で `Download::cancel()` を即時に呼ぶ。黙って捨てず、ステータスバーに「download blocked: <ファイル名>」を数秒表示する。

### 8.6 エラーページとクラッシュ復帰

- 読み込み失敗(`load-failed`): `load_alternate_html` で最小のエラーページ(エラー種別、対象 URL、「r でリロード」)を表示する。`r` は通常どおり `reload()` に割り当たっているため、エラーページからそのまま復帰できる。
- WebProcess クラッシュ(`web-process-terminated`): 同じエラーページを「renderer crashed」として表示する。`reload()` で WebProcess は再 spawn される。

### 8.7 WebView 設定

`webkit6::Settings` は既定値を基本とし、明示するのは以下のみ:

- `enable_developer_extras = true`(自分用ブラウザとしてインスペクタは実用上ほしい。UI からの導線は作らず、将来 `:devtools` 等で開く余地だけ残す)
- ハードウェアアクセラレーション等は WebKit 既定に任せる。

## 9. hint モード

### 9.1 役割分担

- **JS 側(page.js)**: クリック可能要素の列挙、ラベルの採番と DOM オーバーレイ描画、絞り込み表示、確定時のクリック/フォーカス実行。
- **Rust 側**: キー入力の受付(§7.2 のとおり Hint モードのキーは全て Rust が握る)、入力文字の JS への転送、結果メッセージによるモード遷移。

キーイベントは GTK 層で止まるためページの keydown には届かない。したがって「入力→絞り込み」は Rust から `evaluate_javascript("owlHints.input('a')")` の形で駆動する。

### 9.2 プロトコル

Rust → JS(`evaluate_javascript`):

- `owlHints.start()` — 要素列挙とラベル表示
- `owlHints.input(ch)` — ラベル文字の追加入力(絞り込み・確定判定)
- `owlHints.cancel()` — オーバーレイ除去

JS → Rust(script message handler `"owl"`、JSON 文字列):

- `{"type":"hint_result","target":"link"}` — クリック実行済み。owl は Normal へ
- `{"type":"hint_result","target":"input"}` — フォーカス実行済み。owl は Insert へ
- `{"type":"hint_none"}` — 候補 0 件(絞り込みで全滅含む)。owl は Normal へ

### 9.3 対象要素とラベル

- 対象: `a[href]`, `button`, `input`(hidden 以外), `textarea`, `select`, `[onclick]`, `[role=button]`, `[role=link]`, `contenteditable`。ビューポート内かつ可視(`getBoundingClientRect` + `visibility`/`display` 判定)のもの。
- ラベル文字: ホームロー `sadfjklewcmpgh`(qutebrowser 既定)。要素数に応じて 1〜2 文字を採番。
- 確定動作: リンクは `.click()`(SPA のハンドラも動く)。テキスト入力欄は `.focus()`。

### 9.4 描画

`position: fixed` の `<div>` をページへ直接挿入する(高 z-index、独自 class 名 `owl-hint`)。CSS 汚染リスクはあるが MVP では許容。オーバーレイは `cancel`/確定時に全除去する。

## 10. insert モード自動移行

要求 3.3: ユーザー操作起因の focus でのみ Insert へ入り、`autofocus`・スクリプト起因では入らない。

- **hint 経由**: JS の `hint_result: input` メッセージで Rust 側が遷移させる(§9.2)。focus 検知には依存しない。
- **マウスクリック経由**: page.js が capture で `mousedown` を監視し、タイムスタンプを記録。`focusin` 受信時に「直近 200ms 以内に mousedown があった」場合のみ `{"type":"focus","editable":true}` を送る。owl は Normal モードでこれを受けたら Insert へ遷移する。
- **autofocus / スクリプト**: 上記条件を満たさない focusin は通知しない(送らない)。
- editable 判定: `input`(text 系 type)、`textarea`、`contenteditable`。

page.js は `UserContentManager::add_script`(`UserScript`、document-start、全フレーム)で常駐させ、hint 機能と focus 監視を同居させる。

## 11. command モードと `:open`

- `:` で Entry を表示し、初期値 `":"` を入れてフォーカス。Entry の `activate`(Enter)でパース・実行、`Esc`(Entry 上の EventControllerKey)でキャンセル。
- コマンドは `:open <input>` と `:quit` の 2 つ。前方一致補完はしない(MVP)。未知コマンドはステータスバーにエラー表示。

`:open` の入力解釈(要求 3.3 の規則を実装仕様として詳細化)。入力はまず前後の空白を trim し、以下を上から順に適用する:

```
1. trim 後が空                                          → 無効。ステータスバーにエラー表示、遷移しない
2. `^localhost(:数字ポート)?(/パス)?$`                  → http:// を補完
3. スキームあり(`^[a-zA-Z][a-zA-Z0-9+.-]*:` にマッチ)  → そのまま URL
4. 空白を含まず `.` を含むホスト名形式                   → https:// を補完
5. それ以外                                             → DuckDuckGo 検索
   https://duckduckgo.com/?q=<URL エンコード済みクエリ>
```

規則の詳細:

- **localhost をスキームより先に判定する**: `localhost:8080` はスキーム正規表現にもマッチしてしまう(`localhost:` がスキームに見える)ため、規則 2 を先に置く。`localhost:abc` のようにポートが数字でないものは規則 2 に落ちず、規則 3 でスキーム `localhost:` の URL としてそのまま扱われる(WebKit が読み込み失敗 → エラーページ)。一貫性を優先し、特別扱いしない。
- **スキームは大文字小文字を区別しない**(RFC 3986)。`HTTPS://example.com` はそのまま URL として扱う。

この判定関数 `parse_open_input(&str) -> Option<String>`(`None` = 空入力)は純粋関数として `command.rs` に置き、ユニットテストを書く(§14)。エラー表示等の副作用は呼び出し側が行う。

## 12. ステータスバー

WebView のプロパティ通知にバインドする。ポーリングはしない。

| 表示 | ソース |
|---|---|
| モードインジケータ | `set_mode`(§6)から更新 |
| URL | `notify::uri` |
| タイトル | `notify::title` |
| 読み込み状態 | `notify::estimated-load-progress` + `notify::is-loading`(読み込み中のみ `[42%]` 等を表示) |

## 13. 起動フロー

1. `gtk::Application::new(Some("dev.myuron.owl"), NON_UNIQUE)` — `NON_UNIQUE` により `owl <url>` の再実行が常に新プロセス・新ウィンドウになる(単一ウィンドウモデルを保つ最も単純な方法)。
2. `activate` でウィンドウ構築(§5)、NetworkSession・WebView 構築(§8)、UserScript 登録(§10)。
3. `std::env::args` の第 1 引数があれば `load_uri`(`:open` と同じ補完規則を適用する)。なければ `about:blank`。
4. 起動時間目標(1 秒以内)のため、起動パスで同期 I/O は XDG ディレクトリ作成以外行わない。

## 14. テスト方針

- **ユニットテスト**: 純粋ロジックのみ対象 — `parse_open_input`(URL/検索の判定)、キーシーケンスの状態遷移(`pending_key` の解決)。`cargo test` で完結し GTK 不要にする。
- **結合・E2E**: 自動化しない。WebKitGTK を含む E2E はコストに見合わないため、マイルストーンごとの手動確認と、MVP 完了条件である 1 週間の dogfooding(要求 3.4)を検証とする。
- 手動確認用チェックリストを `docs/checklist.md` として実装時に用意する(TLS エラー表示、クラッシュ復帰、autofocus ページで Insert に落ちないこと等、要求 3.1〜3.3 の項目を列挙)。

## 15. ビルド(Nix)

- 既存の flake(`flake.nix` / `nix/rust.nix`)に `webkitgtk_6_0`・`gtk4` と `pkg-config` をネイティブ依存として追加する。
- 実行時に GSettings スキーマと GLib 関連の環境が必要になるため、パッケージは `wrapGAppsHook4` でラップする。
- `nix develop` の devShell にも同じ依存を入れ、`cargo build` が devShell 内で通ることを開発の基本ループとする。

## 16. 実装マイルストーン

各マイルストーンは「動くものが増える」単位で切る。M1 が技術リスク(`webkit6` crate の実用性)の検証を兼ねる。

1. **M1 — スケルトン**: 依存追加(Cargo.toml / flake)、ウィンドウ + WebView でハードコード URL が表示される。`nix build` が通る。→ **ここで GTK4 構成の最終確定**(§2)
2. **M2 — ナビゲーションとステータスバー**: 引数 URL 起動、戻る/進む/リロード/中断(API 直叩き)、ステータスバー表示
3. **M3 — モードとキーバインド**: モード管理、Normal のバインド一式(スクロール含む)、Insert(手動 `i`/`Esc` のみ)
4. **M4 — command モード**: コマンドライン UI、`:open` パース + テスト、`:quit`
5. **M5 — hint モード**: page.js 注入、ヒント表示・選択・モード遷移
6. **M6 — insert 自動移行**: focus 検知(mousedown 相関)、autofocus 抑止の確認
7. **M7 — 堅牢化**: 永続化、TLS Fail、エラーページ、クラッシュ復帰、ポップアップ抑制、ダウンロードキャンセル
8. **M8 — dogfooding**: チェックリスト消化 → 1 週間常用(要求 3.4)

## 17. リスクと未確定事項

| 項目 | 内容 | 対処 |
|---|---|---|
| `webkit6` crate の成熟度 | 利用実績が少なく、シグナル/API の欠落がありうる | M1 で本書 §8 の全シグナルの存在をコンパイルレベルで確認する |
| メインフレーム非スクロールのページ | §8.1 の JS スクロールが効かない | MVP では許容。頻出なら要素探索方式へ改訂 |
| iframe 内のヒント・focus 検知 | UserScript は全フレーム注入するが、クロスオリジン iframe とのメッセージングは要検証 | M5 で検証。MVP ではメインフレームのみ動作保証とする |
| `about:blank` 上での JS 注入 | 空白ページでヒント等が動かなくても実害なし | 対処不要と確認のみ |
