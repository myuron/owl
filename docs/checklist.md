# owl — 手動確認チェックリスト

[設計書](design.md) §14 の方針に基づく手動確認項目。ユニットテスト([test.md](test.md))で
扱わない領域 — GTK/WebKit を含む結合・E2E、モード遷移の副作用、WebView 統合 — を
マイルストーンごとにここへ列挙する。自動化しない(§14: 「WebKitGTK を含む E2E はコストに
見合わない」)。各項目は `nix build` した `./result/bin/owl` に対し手動で確認する。

---

## M1: スケルトン(GUI 結線)

ゴール(design §16): 「ウィンドウ + WebView でハードコード/引数 URL が表示され、
`nix build` が通る」。同時に §17 の `webkit6` crate 成熟度リスクを実ビルドで検証する。

### M1.1 ビルド(design §16 の M1 完了条件)

| ID | 手順 | 期待結果 | 結果(2026-07-18) |
|---|---|---|---|
| M1-01 | `nix develop` 内で `cargo build` | エラーなくビルドが通る(§15 の基本ループ) | ✅ 通過(警告ゼロ) |
| M1-02 | `nix build` | 成功し `./result/bin/owl` が生成される(§16 の M1 完了条件) | ✅ `./result/bin/owl` 生成 |
| M1-03 | `just ci`(fmt-check → lint → coverage → build) | 全ステップ緑。coverage は純粋ロジックの region/line 100% を維持(GTK 依存の `main`/`window`/`webview` は除外) | ✅ fmt-check / clippy(-D warnings)/ coverage(command.rs・keys.rs 100%)/ nix build すべて緑 |

### M1.2 起動と URL 表示(design §13・§16)

| ID | 手順 | 期待結果 | 結果(2026-07-18) |
|---|---|---|---|
| M1-10 | `./result/bin/owl https://example.com` | ウィンドウが開き、当該ページが表示される(§13-3: 引数 URL を `load_uri`) | ✅ ページ描画を目視確認。`WebKitNetworkProcess` が example.com(`2606:4700:10::6814:179a`:443)へ TLS 接続を確立し、実フェッチも裏取り済み |
| M1-11 | `./result/bin/owl`(引数なし) | ウィンドウが開き、`about:blank`(空白ページ)になる(§13-3) | ✅ ウィンドウ起動・`WebKitWebProcess` 生成を確認(空白ページ) |
| M1-12 | `./result/bin/owl https://example.com` を再実行(既に 1 つ起動中) | 既存ウィンドウに集約されず、新プロセス・新ウィンドウが開く(§13-1: `NON_UNIQUE`) | ✅ 独立した `.owl-wrapped` プロセスが 2 つ(各自の Web/Network プロセス付き)。2 ウィンドウを目視確認 |
| M1-13 | 表示されたウィンドウ | ツールバー・メニューバーが無い。WebView が縦 `Box` 内で全体に広がる(§5-1: `vexpand`)。ステータスバー/コマンドラインは M1 では未実装(省略可、§5-2/§5-3) | ✅ ツールバー・メニューバー無し、WebView が全面。目視確認 |
| M1-14 | 起動から初期ページ表示までの体感 | 1 秒以内(§13-4 の起動時間目標。起動パスの同期 I/O は XDG ディレクトリ作成のみ) | ✅ 1 秒以内の体感(目視) |

### M1.3 データディレクトリ(design §8.2)

| ID | 手順 | 期待結果 | 結果(2026-07-18) |
|---|---|---|---|
| M1-20 | 起動後に `$XDG_DATA_HOME/owl`(既定 `~/.local/share/owl`)を確認 | ディレクトリが作成されている(NetworkSession の data ディレクトリ) | ✅ 空の `XDG_DATA_HOME` を与えて起動 → `owl/` が作成された(`command::app_subdir` + `create_dir_all`) |
| M1-21 | 起動後に `$XDG_CACHE_HOME/owl`(既定 `~/.cache/owl`)を確認 | ディレクトリが作成されている(NetworkSession の cache ディレクトリ) | ✅ 空の `XDG_CACHE_HOME` を与えて起動 → `owl/` が作成された |

> Cookie の明示的な永続化(`set_persistent_storage`, §8.2)・ローカルストレージの
> 具体的な確認は M7 の範囲。M1 では NetworkSession の最小結線とディレクトリ作成のみ確認する。

### M1.4 `webkit6` シグナル/API の存在確認(design §16・§17、todo 3-3)

M7 で使う §8 のシグナル/API が **コンパイルレベルで存在する**ことを確認し、結果を記録する
(実装はしない)。欠落があれば design §17 の表へ追記し、対処方針(§2 のフォールバック再検討
 or 設計改訂)を書く。**全て確認できるまで §17 のリスクを「解消」と見なさない。**

確認方法: 各 API を正しいクロージャ形状・引数型で参照する一時プローブ
`examples/webkit_api_probe.rs` を書き、`cargo build --example webkit_api_probe` で解決を
強制した(2026-07-18、webkit6 0.6.1 + `gtk_v4_18`)。全 8 個がコンパイル成功。確認後プローブは削除。

| ID | 対象 | 用途(design) | 確認結果(2026-07-18) |
|---|---|---|---|
| M1-30 | `WebView::connect_create` | §8.4 ポップアップ抑制 | ✅ 存在。`Fn(&WebView, &NavigationAction) -> Option<Widget>` |
| M1-31 | `connect_load_failed` / `load-failed-with-tls-errors` | §8.3・§8.6 | ✅ 両方存在。ただし戻り値は `glib::Propagation` ではなく **`bool`**(`true` = ハンドル済み)。M7 実装時に注意 |
| M1-32 | `connect_web_process_terminated` | §8.6 クラッシュ復帰 | ✅ 存在。`Fn(&WebView, WebProcessTerminationReason)` |
| M1-33 | `NetworkSession::connect_download_started` | §8.5 DL キャンセル | ✅ 存在。`Fn(&NetworkSession, &Download)` |
| M1-34 | `NetworkSession::set_tls_errors_policy` | §8.3 TLS Fail | ✅ 存在。`TLSErrorsPolicy::Fail` を受理 |
| M1-35 | `WebView::load_alternate_html` | §8.6 エラーページ | ✅ 存在。`(content, content_uri, base_uri: Option<&str>)` |
| M1-36 | `CookieManager::set_persistent_storage(…, Sqlite)` | §8.2 Cookie 永続化 | ✅ 存在。`CookiePersistentStorage::Sqlite` を受理 |
| M1-37 | `Download::cancel` | §8.5 DL キャンセル | ✅ 存在。`Download::cancel(&self)` |

### M1.5 M1 完了判定(design §16・§17)

| ID | 判定 | 結果(2026-07-18) |
|---|---|---|
| M1-40 | M1-01〜M1-14 が全て期待どおり(ビルド通過・URL 表示) | ✅ 達成(M1-01〜14 すべて緑・目視確認済み) |
| M1-41 | M1-30〜M1-37 が全て「存在」で、design §2 の GTK4 構成が実ビルドで確定した(§16) | ✅ 達成。M1-30〜M1-37 全 8 個がコンパイルレベルで存在。GTK4 構成も実ビルドで確定済み。§17 の webkit6 成熟度リスクを「解消」と判定 |
| M1-42 | 致命的問題があれば §17・§2 のフォールバック判断を design.md に反映済み | ✅ 致命的欠落なし。§8 の全シグナル/API が存在するためフォールバック(§2)は不要。§17 の該当行を「解消済み」へ更新し、M1-31 の戻り値差異(`bool`)を追記 |

---

## M2: ステータスバー

ゴール(design §16・§5-2・§12): 「`owl <url>` 起動でステータスバーに URL・ページ
タイトル・読み込み状態が表示され、リンク遷移で追従更新される」。

**スコープ境界:** ナビゲーション(戻る/進む/リロード/中断)は M2 では扱わない(要求 §3.2
のとおりキーバインド駆動 → トリガは M3)。モードインジケータの内容更新も M3(M2 は空枠のみ)。
M2 はステータスバー結線に集中する。

### M2.1 ビルド

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M2-01 | `just ci`(fmt-check → lint → coverage → build) | 全ステップ緑。coverage は純粋ロジック(`command.rs`・`keys.rs`)の region/line 100% を維持(新 `s12_*` テスト含む) | ✅ 全ステップ緑。coverage は command.rs 335/335・keys.rs 299/299 region 100%。`nix build` → `./result/bin/owl` 生成。加えて `about:blank` 起動のスモークテストで panic/GTK CRITICAL/CSS パースエラーが出ないことを確認(ステータスバー結線の実行時初期化 OK) |

### M2.2 ステータスバー表示(design §5-2・§12)

各項目は `nix build` した `./result/bin/owl` に対し手動で確認する。

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M2-10 | `./result/bin/owl https://example.com` | ウィンドウ下部に 1 行のステータスバー。URL 欄に `https://example.com`、読み込み中は右端に `[NN%]`(完了で空)、タイトル欄にページタイトルが表示される(§12: notify にバインド) | ✅ 目視確認(2026-07-22)。ステータスバーに URL 欄 `https://example.com`・タイトル欄 `Example Domain`(右端 `[NN%]` の左)・完了後は進捗欄が空 |
| M2-11 | 表示ページ内のリンクをマウスでクリック | URL 欄・タイトル欄が遷移先へ追従更新される(§12: `notify::uri`/`title`) | ✅ 目視確認(2026-07-22)。リンククリックで URL 欄・タイトル欄が遷移先へ追従更新された |
| M2-12 | `./result/bin/owl`(引数なし) | URL 欄が `about:blank`、タイトル欄は空、モードインジケータは空(normal・M2 では常に空) | ✅ 目視確認(2026-07-22)。URL 欄 `about:blank`・タイトル欄空・モードインジケータ空 |
| M2-13 | 表示されたウィンドウ | ツールバー・メニューバー無し。WebView が上部を占有し(`vexpand`)、下部にステータスバーが 1 行。配色・等幅フォントの CSS が効いている(§5) | ✅ 目視確認(2026-07-22)。ツールバー・メニューバー無し、WebView が上部占有・下部にステータスバー 1 行、等幅フォント/配色の CSS が有効 |

> ナビゲーション操作(H/L/r/Esc)とモードインジケータの内容更新は M3(キーバインド)で
> 確認する。M2 ではステータスバーの表示・追従更新のみを対象とする。

---

## M3: モードとキーバインド

ゴール(design §16.3・§6・§7): 「モード管理・Normal のバインド一式(スクロール含む)・
ナビゲーション・モードインジケータ更新・Insert(手動 `i`/`Esc`)」。純粋ロジック(`keys.rs`)は
ユニットテスト済み(test.md §2)。ここでは GTK 結線(`input.rs` の EventControllerKey)の実挙動を
`nix build` した `./result/bin/owl` で手動確認する。

**スコープ境界:** `:`(Command)/`f`(Hint) は M3 では inert(M4/M5 で本結線)。`:open` 補完は M4、
insert 自動移行は M6。

### M3.1 ビルド

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M3-01 | `just ci`(fmt-check → lint → coverage → mutants → build) | 全ステップ緑。coverage は command.rs/keys.rs region/line 100% 維持(新 `classify_input`/`scroll_script`/`mode_indicator` と拡張した `resolve_key` を含む)。mutants survivor ゼロ。`input.rs` は GTK 結線のため coverage 除外 | ✅ fmt-check / clippy(-D warnings)/ coverage(command.rs・keys.rs 100%)/ mutants / nix build 緑 |

### M3.2 横取りの前提(design §7.1)

**全バインドの土台**なので最初に確認する。

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M3-10 | ページ表示後、WebView 本体をクリックしてフォーカスを移し、`j` を押す | WebView にフォーカスがあってもウィンドウの capture phase コントローラが先にキーを受け取り、下スクロールする(§7.1: capture(親→子)) | ✅ 目視確認(2026-07-22)。WebView フォーカス時も `j` を capture 側が先取りし下スクロール |

### M3.3 スクロール(design §7.4・§8.1)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M3-20 | 縦長ページで `j`/`k`/`h`/`l` | 下/上/左/右へ 50px ずつスクロール(`behavior:'instant'`) | ✅ 目視確認(2026-07-22)。`j`/`k`/`h`/`l` で下/上/左/右へ 50px 即時スクロール |
| M3-21 | `gg` / `G` | ページ先頭 / 末尾へ | ✅ 目視確認(2026-07-22)。`gg` で先頭・`G` で末尾へジャンプ |
| M3-22 | `Ctrl+d` / `Ctrl+u` | 半ページ(ビューポート高の 1/2)下 / 上へ | ✅ 目視確認(2026-07-22)。`Ctrl+d` で半ページ下・`Ctrl+u` で半ページ上へスクロール |

### M3.4 ナビゲーション・コピー・中断(design §7.4)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M3-30 | リンクを辿った後 `H` → `L` | 戻る → 進む(`WebView::go_back`/`go_forward`)。ステータスバーの URL が追従 | ✅ 目視確認(2026-07-22)。`H` で戻る・`L` で進む、URL 欄が追従更新 |
| M3-31 | `r` | リロード(`WebView::reload`) | ✅ 目視確認(2026-07-22)。`r` で現在ページがリロード、進捗欄 `[NN%]` 表示後に消える |
| M3-32 | 読み込み中に `Esc`(pending なし) | 読み込み中断(`WebView::stop_loading`)。読み込み状態表示が止まる | ✅ 目視確認(2026-07-22)。読み込み中の `Esc` で中断、進捗欄の更新が停止 |
| M3-33 | `yy` | 現在ページの URL がクリップボードへコピーされる(他アプリに貼り付けて確認) | ✅ 目視確認(2026-07-22)。`yy` で現在 URL がクリップボードにコピーされ、別アプリへ貼り付け可能 |

### M3.5 モード遷移・インジケータ(design §6・§5-2・§12)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M3-40 | `i` を押す | ステータスバー左端に `-- INSERT --`。以後キー入力がページ(フォーム等)へ届く | ✅ 目視確認(2026-07-22)。`i` で左端に `-- INSERT --`、以後キー入力がページへ届く |
| M3-41 | フォーム入力欄にフォーカスした状態で `Esc` | Normal に戻り、入力欄の focus が外れる(`document.activeElement.blur()` + WebView へ grab_focus)。インジケータが空になる | ✅ 目視確認(2026-07-22)。`Esc` で Normal 復帰、入力欄の focus が外れモードインジケータが空 |
| M3-42 | `g` を押した直後に `Esc` | pending クリアのみ(読み込み中断は起きない。§7.3)。続く `g` は新規シーケンス開始 | ✅ 目視確認(2026-07-22)。`g` 後の `Esc` は pending クリアのみ(中断なし)、続く `g` は新規シーケンス扱い |

### M3.6 スコープ外の安全確認・既知の制限

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M3-50 | `f` を押す | 現状 **inert**: 何も起きず、モードは Normal のまま(トラップしない)。M5 で本結線。※`:`(Command)は M4 で本結線済み — 挙動は M4-10〜M4-24 で確認する | ✅ 目視確認(2026-07-22)。**M8 全部入りビルドでは `f` は Hint 本結線済み**(M5)。押すとクリック可能要素に黄色ラベルが重畳表示され `-- HINT --` へ遷移、`Esc` で消えて Normal 復帰。M3-50 の「inert」は M3 開発時点の期待であり M8 では実挙動どおり hint が動作(hint 本体の確認は M5-10〜M5-24) |
| M3-51 | Normal モードで矢印キー / PageUp / PageDown | 消費されて無反応(§7.2: 未割当の修飾なしキーはページに漏らさない)。スクロールは `h/j/k/l`・`gg/G`・`Ctrl+d/u` を使う想定。**バグではなく仕様**(既知の制限) | ✅ 目視確認(2026-07-22)。矢印/PageUp/PageDown は消費され無反応(仕様どおり) |
| M3-52 | 内側 `div` がスクロールコンテナのページで `j` 等 | メインフレームが動かないため効かないことがある(§8.1 の既知の制限、MVP 許容)。**バグではない** | ⚪ 未遭遇(2026-07-22)。該当ページに当たらず未確認。既知の制限のため MVP 許容 |
| M3-53 | Normal で選択テキストに対し `Ctrl+C` | ページへ素通しし、コピーできる(§7.2: バインド外の修飾付きは Proceed) | ✅ 目視確認(2026-07-22)。選択テキストに `Ctrl+C` がページへ素通しし、別アプリへ貼り付け可能 |

---

## M4: command モード

ゴール(design §16.4・§11・§5-3): 「`:` でコマンドライン(Entry)を開き、`:open <input>` で
補完済み URL を開ける・`:quit` で終了できる・未知コマンドはステータスバーにエラー表示」。
純粋ロジック(`command::parse_command`)はユニットテスト済み(test.md §1.7 CMD-01〜CMD-11)。
ここでは GTK 結線(コマンドライン Entry の表示・実行・キャンセル、`input`)の実挙動を
`nix build` した `./result/bin/owl` で手動確認する。

**スコープ境界:** `f`(Hint)は M4 でも inert(M5 で本結線)。`:open` の補完規則そのもの
(`parse_open_input`)は M1 で実装済み・テスト済みで、ここでは代表ケースの通し確認に留める。

### M4.1 ビルド

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M4-01 | `just ci`(fmt-check → lint → coverage → mutants → build) | 全ステップ緑。coverage は command.rs/keys.rs region/line 100% 維持(新 `parse_command` を含む)。mutants survivor ゼロ(84 mutants: 69 caught・15 unviable)。`input.rs`/`window.rs` は GTK 結線のため coverage 除外 | ✅ fmt-check / clippy(-D warnings)/ coverage(command.rs 432/432・keys.rs 541/541 region 100%)/ mutants(survivor 0)/ nix build 緑 |

### M4.2 コマンドライン UI(design §5-3・§11)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M4-10 | Normal モードで `:` を押す | ウィンドウ最下部にコマンドライン(Entry)が現れ、初期値 `:` が入ってフォーカスされる。カーソルは `:` の後ろ(全選択されていない)。モードインジケータが `-- COMMAND --` | ✅ 目視確認(2026-07-22)。`:` で最下部にコマンドライン Entry 出現、初期値 `:`・カーソルは末尾・`-- COMMAND --` 表示 |
| M4-11 | `:` を開いた状態で `open example.com` と入力 → Enter | `https://example.com` へ遷移(§11 規則 4 の https 補完)。コマンドラインが閉じ、Normal に戻る(インジケータが空)。ステータスバーの URL が追従 | ✅ 目視確認(2026-07-22)。`open example.com` → `https://example.com` へ遷移、コマンドライン閉じ Normal 復帰、URL 欄追従 |
| M4-12 | `:open localhost:8080` → Enter | `http://localhost:8080` へ遷移(§11 規則 2)。※接続先が無ければ読み込み失敗でよい(補完先の確認が目的) | ✅ 目視確認(2026-07-22)。`localhost:8080` に `http://` 補完され URL 欄 `http://localhost:8080`(接続先不在で読み込み失敗は許容、補完先を確認) |
| M4-13 | `:open rust 所有権` → Enter | DuckDuckGo 検索(`https://duckduckgo.com/?q=...` にエンコード)へ遷移(§11 規則 5) | ✅ 目視確認(2026-07-22)。`rust 所有権` が DuckDuckGo 検索(`https://duckduckgo.com/?q=...` エンコード)へ遷移 |
| M4-14 | `:quit` → Enter | ウィンドウが閉じ、プロセスが終了する(§11・§13-1: `NON_UNIQUE` の単一ウィンドウを閉じる) | ✅ 目視確認(2026-07-22)。`:quit` でウィンドウが閉じプロセス終了 |

### M4.3 エラー・キャンセル(design §11)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M4-20 | `:foo` → Enter | ステータスバーのメッセージ欄に `unknown command: foo`(警告色)。コマンドラインは閉じ Normal に戻る。遷移しない | ✅ 目視確認(2026-07-22)。`:foo` でメッセージ欄に警告色 `unknown command: foo`、コマンドライン閉じ Normal 復帰、遷移なし |
| M4-21 | `:open`(引数なし)→ Enter | メッセージ欄に `usage: open <url or query>`。遷移しない | ✅ 目視確認(2026-07-22)。`:open` 単体でメッセージ欄に `usage: open <url or query>`、遷移なし |
| M4-22 | `:` を開いた状態で `Esc` | 入力を破棄してコマンドラインが閉じ、Normal に戻る(§11: Entry 上の EventControllerKey)。インジケータが空。読み込み中断は起きない | ✅ 目視確認(2026-07-22)。`Esc` で入力破棄・コマンドライン閉じ Normal 復帰、インジケータ空、読み込み中断なし |
| M4-23 | `:`(コロンのみ)→ Enter | 何も起きずコマンドラインが閉じ Normal に戻る(`Noop`) | ✅ 目視確認(2026-07-22)。コロンのみ Enter は何も起きずコマンドライン閉じ Normal 復帰(Noop) |
| M4-24 | エラー表示後に再度 `:` を開く | 前回のエラーメッセージがクリアされる(新しい入力の開始) | ✅ 目視確認(2026-07-22)。エラー表示後に `:` を開くと前回のエラーメッセージがクリアされる |

### M4.4 スコープ外の安全確認

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M4-30 | `f` を押す | 現状 **inert**: 何も起きず Normal のまま(M5 で本結線) | ✅ 目視確認(2026-07-22)。**M8 全部入りビルドでは `f` は Hint 本結線済み**(M5)。押すと黄色ラベルが重畳表示され `-- HINT --` へ、`Esc` で Normal 復帰。「inert」は M4 開発時点の期待で M8 では実挙動どおり hint が動作(hint 本体は M5-10〜) |
| M4-31 | command モードで入力中に `j`/`k` 等 | ページへスクロールせず Entry へ文字入力される(§7.2: Command は全キー Proceed で Entry が処理) | ✅ 目視確認(2026-07-22)。command モード中の `j`/`k` はページへ漏れず Entry へ文字入力される |
| M4-32 | command モード中に WebView 本体をマウスでクリック | Entry のフォーカスは外れるがモードは Command のまま(インジケータは `-- COMMAND --` 継続、キーはページへ Proceed)。**復帰路は下部に見えている Entry をクリックし直すこと**。この状態の `Esc`/`Enter` は復帰**しない** — Command モードは全キー Proceed(`keys.rs:116`)で、Esc/Enter を拾うのは Entry 自身のハンドラ(`input.rs:155`/`180`)だけだが、フォーカスが WebView にあるため両キーはページへ流れ Entry ハンドラが発火しない。**バグではなく既知の制限**(M3-51/52 と同種、MVP 許容)。将来直すなら Entry の focus-leave で `leave_command` | ✅ 目視確認(2026-07-22)。WebView クリックで Entry フォーカスは外れモードは Command 継続。**Esc/Enter では復帰せず**(ページへ流れる)、下部の Entry をクリックし直すと再フォーカスし `Esc` で Normal 復帰。期待結果の「Esc/Enter で復帰」は M4 開発時の理想で実挙動と食い違うため、実挙動(Entry 再クリックのみ)へ修正(規約 9) |

---

## M5: hint モード

ゴール(design §16.5・§9・§10): 「`f` でクリック可能要素にラベルを重畳表示し、ラベルを
タイプして対象を選択(クリック相当)する。リンク選択時は遷移して Normal へ、テキスト入力欄
選択時は focus して Insert へ、`Esc` でキャンセルして Normal へ戻る」(要求 §3.3)。純粋ロジック
(`hints.rs` の JS 文字列生成・メッセージ解釈、`keys.rs` の `HintInput`)はユニットテスト済み
(test.md §2.11 H-01〜H-16・K-67)。ここでは GTK/JS 結線(page.js の描画・確定、`owlHints.*` 駆動、
script message handler 受信、モード遷移)の実挙動を `nix build` した `./result/bin/owl` で手動確認する。

**スコープ境界:** §10 の insert 自動移行(mousedown 相関の focus 検知)は M6。クロスオリジン
iframe 内のヒントは MVP 非保証(§17、メインフレームのみ動作保証)。エラーページ等(§8)は M7。

### M5.1 ビルド

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M5-01 | `just ci`(fmt-check → lint → coverage → mutants → build) | 全ステップ緑。coverage は command.rs/keys.rs/hints.rs region/line 100% 維持(新 `hints.rs`・`Action::HintInput` を含む)。mutants survivor ゼロ(107 mutants: 91 caught・16 unviable、`-f src/hints.rs` 追加)。`input.rs`/`webview.rs` は GTK 結線のため coverage 除外、`page.js` は JS のため対象外 | ✅ fmt-check / clippy(-D warnings)/ coverage(command.rs 432/432・keys.rs 550/550・hints.rs 185/185 region 100%)/ mutants(survivor 0)/ nix build 緑 |

### M5.2 ヒント表示・選択(design §9.2・§9.3・§9.4)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M5-10 | リンクの多いページ(例: `https://example.com` から辿った一覧)で `f` を押す | ビューポート内のクリック可能要素(`a[href]`・`button`・入力欄等)にホームロー(`sadfjklewcmpgh`)のラベルが重畳表示される。モードインジケータが `-- HINT --`。要素数に応じ 1〜2 文字ラベル(§9.3) | ✅ 目視確認(2026-07-22)。`f` でクリック可能要素にホームローのラベルが重畳表示、`-- HINT --` 表示、要素数に応じ 1〜2 文字ラベル |
| M5-11 | ラベルを 1 文字タイプ(2 文字ラベルの場合は 1 文字目) | 前方一致で候補が絞り込まれ、一致しないラベルは消える(§9.2) | ✅ 目視確認(2026-07-22)。1 文字タイプで前方一致絞り込み、非一致ラベルが消える |
| M5-12 | リンクのラベルを最後までタイプして確定 | 対象リンクがクリックされ(`.click()`)遷移。オーバーレイが消え、Normal に戻る(インジケータ空)。ステータスバーの URL が追従(§9.2 `hint_result:link`) | ✅ 目視確認(2026-07-22)。リンクのラベル確定で対象がクリックされ遷移、オーバーレイ消え Normal 復帰、URL 欄追従 |
| M5-13 | テキスト入力欄のラベルをタイプして確定 | 入力欄が focus され(`.focus()`)、Insert へ遷移(インジケータ `-- INSERT --`)。以後キー入力が入力欄へ届く(§9.2 `hint_result:input`) | ✅ 目視確認(2026-07-22)。入力欄のラベル確定で focus され `-- INSERT --` へ遷移、以後キー入力が入力欄へ届く |
| M5-14 | Hint 表示中に候補に無い文字をタイプ | 候補全滅として Normal に戻る(インジケータ空、§9.2 `hint_none`)。オーバーレイが消える | ✅ 目視確認(2026-07-22)。候補に無い文字で全滅し Normal 復帰(インジケータ空)、オーバーレイ消去 |

### M5.3 キャンセル・スコープ外・既知の制限(design §9・§17)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M5-20 | Hint 表示中に `Esc` | オーバーレイが消え(`owlHints.cancel()`)、Normal に戻る(インジケータ空)。読み込み中断は起きない | ✅ 目視確認(2026-07-22)。Hint 表示中の `Esc` でオーバーレイ消え Normal 復帰、読み込み中断なし |
| M5-21 | クリック可能要素が無い/ビューポート外だけのページで `f` | 候補 0 件で即 Normal に戻る(§9.2 `hint_none`)。オーバーレイは出ない | ⚪ 未遭遇(2026-07-22)。クリック可能要素が皆無のページに当たらず未確認(近い確認は M5-22 の `about:blank` で実施) |
| M5-22 | `about:blank`(引数なし起動)で `f` | page.js が注入されていれば候補 0 件で即 Normal へ戻る(`hint_none`)。UserScript が注入されない特殊ページ等では一瞬 `-- HINT --` になり `hint_none` が来ず Hint に留まる → `Esc` で復帰(§9・§17 の既知の制限。**バグではない**) | ✅ 目視確認(2026-07-22)。`about:blank` で `f` → 候補 0 件で即 Normal へ戻った(page.js 注入済み・`hint_none` 経路。Hint スタックは発生せず) |
| M5-23 | クロスオリジン iframe を含むページで `f` | メインフレームの要素のみラベル表示される(iframe 内は MVP 非保証。§17)。**バグではなく既知の制限** | ⚪ 未遭遇(2026-07-22)。クロスオリジン iframe を含むページで未検証。既知の制限のため MVP 許容(§17) |
| M5-24 | Insert 遷移後(M5-13)に `Esc` | Normal に戻り、入力欄の focus が外れる(M3 の Insert→Normal と同じ挙動) | ✅ 目視確認(2026-07-22)。hint→Insert 後の `Esc` で Normal 復帰、入力欄の focus が外れる |

---

## M6: insert モード自動移行

ゴール(design §16.6・§10): 「ユーザー操作起因の focus でのみ Insert へ入り、`autofocus`・
スクリプト起因では入らない」(要求 3.3)。M5 で hint 経由のテキスト入力欄選択(`hint_result:input`
→ Insert)は実装済みなので、M6 は **マウスクリック経由の focus 検知**(§10: page.js が capture で
mousedown を監視 → 直近 200ms 以内の focusin かつ editable なら `{"type":"focus"}` を送信 → owl は
Normal のときのみ Insert へ)を追加する。純粋ロジック(`hints.rs` の `HintMessage::Focus` パース)は
ユニットテスト済み(test.md §2.11 H-17)。ここでは page.js の focus 監視・script message handler 受信・
モード遷移の実挙動を `nix build` した `./result/bin/owl` で手動確認する。

**スコープ境界:** hint 経由の Insert 遷移(§9.2)は M5 で実装済み(M5-13 で確認)。クロスオリジン
iframe 内のクリック focus は MVP 非保証(§17、メインフレームのみ)。

### M6.1 ビルド

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M6-01 | `just ci`(fmt-check → lint → coverage → mutants → build) | 全ステップ緑。coverage は command.rs/keys.rs/hints.rs region/line 100% 維持(新 `HintMessage::Focus` パースを含む)。mutants survivor ゼロ。`input.rs`/`page.js` は GTK/JS 結線のため coverage 対象外 | ✅ fmt-check / clippy(-D warnings)/ coverage(command.rs 432/432・keys.rs 550/550・hints.rs 192/192 region 100%)/ mutants(108 mutants: 92 caught・16 unviable、survivor 0)/ nix build 緑 |

### M6.2 クリック focus → Insert(design §10)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M6-10 | テキスト入力欄(`input`/`textarea`/contenteditable)のあるページで、その入力欄をマウスでクリック | 自動で Insert へ遷移し、モードインジケータが `-- INSERT --`。以後キー入力が入力欄へ届く(§10 mousedown 相関の focusin)。**未 focus の欄・既に focus 済みの欄(`autofocus` 済みを初めてクリックする場合含む)の両方で遷移する**(後者は focusin が発火しないため mousedown 側の即時通知で拾う) | ✅ 目視確認(2026-07-22)。入力欄クリックで自動 Insert 遷移(`-- INSERT --`)、以後キー入力が入力欄へ届く |
| M6-11 | `<input autofocus>` を含むページを開く | Normal のまま(インジケータ空)。`autofocus` の focusin は mousedown を伴わないため通知されない(§10・要求 3.3) | ✅ 目視確認(2026-07-22)。`https://duckduckgo.com` を新規起動で開くと検索欄が自動フォーカスされるが owl は Normal のまま(要求 3.3 が守ろうとする実シナリオで検証)。※`data:` トップレベルは WebKit がブロックし about:blank、`file://` はこの WebKitGTK 構成で表示されなかったため実サイトで確認 |
| M6-12 | JS が読み込み時に `element.focus()` を呼ぶページを開く | Normal のまま。スクリプト起因 focus は通知されない(§10・要求 3.3) | ✅ 目視確認(2026-07-22)。`https://duckduckgo.com` 読み込み時のスクリプト起因の検索欄 focus でも Normal のまま(クリックせず確認)。M6-11 と同一検証 |
| M6-13 | M6-10 の Insert 遷移後に `Esc` | Normal に戻り、入力欄の focus が外れる(§6 Insert→Normal と同じ挙動) | ✅ 目視確認(2026-07-22)。クリック Insert 後の `Esc` で Normal 復帰、入力欄の focus が外れる |
| M6-14 | リンクやボタン等 editable でない要素をマウスでクリック | Normal のまま。`isEditable` が false のため focus 通知を送らない(§10) | ✅ 目視確認(2026-07-22)。editable でない要素(リンク/ボタン)のクリックでは Insert に入らず Normal のまま |

### M6.3 スコープ外・既知の制限(design §10・§17)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M6-20 | (既知の制限)悪意あるページが `window.webkit.messageHandlers.owl.postMessage('{"type":"focus"}')` を Normal 中に偽装送信 | owl は Insert へ入りうる。mousedown 相関は信頼できないページ側(page.js)が判定するため Normal 中の偽装は防げない。`Esc` で必ず Normal へ復帰でき実害は限定的(§17 MVP 許容、hint 偽装と同種)。**バグではなく既知の制限** | ⚪ 未遭遇(2026-07-22)。攻撃的ページ未用意のため実地未再現。受理条件(Normal 中のみ `focus` を受理)はコードで実装済み(`input.rs`)。既知リスクとして MVP 許容(§17) |
| M6-21 | Command/Hint/Insert モード中に focus メッセージ相当が届く状況(例: hint 表示中に入力欄をクリック) | Normal 以外では focus メッセージを受理しない(規約 6)。hint はモードを維持、他モードも遷移しない | ✅ 目視確認(2026-07-22)。Hint 表示中に入力欄をクリックしても Insert へ落ちず(focus メッセージ非受理)、モードが勝手に Insert へ遷移しない |
| M6-22 | Insert 遷移後、ページの余白等 **editable でない領域**をマウスでクリック | 入力欄の DOM focus は外れるが、モードは Insert のまま(インジケータ `-- INSERT --`、キーはページへ素通し)。§10 は「入る」方向のみ定義。`Esc` で Normal へ復帰できる。**バグではなく既知の制限**(Low L-2、MVP 許容) | ✅ 目視確認(2026-07-23)。Insert 中に余白をクリックしても Insert のまま(`-- INSERT --` 継続)、`Esc` で Normal 復帰。既知の制限どおり |
| M6-23 | `<label for=…>` 経由の focus や、ボタンを 200ms 超**長押し**してから離すクリック | focusin が mousedown から `FOCUS_WINDOW_MS`(200ms)を超えて発火するため Insert に入らないことがある。通常のクリックは窓内に収まる。**バグではなく既知の制限**(Low L-1、裁量値。MVP 許容) | ✅ 目視確認(2026-07-23)。入力欄を 200ms 超長押ししてから離すと Insert に入らないことを確認(通常クリックは M6-10 どおり入る)。既知の制限どおり(Low L-1、MVP 許容) |

---

## M7: 堅牢化

ゴール(design §16.7・§8.2〜8.6): 「Cookie 永続化・TLS Fail・新規ウィンドウ抑制・ダウンロード
キャンセル・エラーページ/クラッシュ復帰」。純粋ロジック(`command::{error_page_html,
download_blocked_message}`)はユニットテスト済み(test.md §1.8 DL-01〜04・ERR-01〜03)。ここでは
GTK/WebKit シグナル結線(`webview.rs`・`window.rs`)の実挙動を `nix build` した `./result/bin/owl`
で手動確認する。

**スコープ境界:** data/cache ディレクトリ作成は M1 で確認済み(M1-20/21)。M7 は Cookie の SQLite
永続化・TLS/エラー/クラッシュ/ポップアップ/DL の各シグナルを対象とする。次は M8 dogfooding。

### M7.1 ビルド

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M7-01 | `just ci`(fmt-check → lint → coverage → mutants → build) | 全ステップ緑。coverage は command.rs/keys.rs/hints.rs region/line 100% 維持(新 `error_page_html`/`download_blocked_message`/`uri_basename`/`html_escape`/`popup_navigation_uri` を含む)。mutants survivor ゼロ(129 mutants: 113 caught・16 unviable)。`webview.rs`/`window.rs` は GTK 結線のため coverage 除外 | ✅ fmt-check / clippy(-D warnings)/ coverage(command.rs 100%・keys.rs 100%・hints.rs 100%)/ mutants(survivor 0)/ nix build 緑。加えて空の `XDG_DATA_HOME` で `about:blank` 起動 → panic/CRITICAL なし・`owl/cookies.sqlite` 生成を確認(§8.2 の永続化が実挙動で有効) |

### M7.2 Cookie・データ永続化(design §8.2)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M7-10 | 起動後に `$XDG_DATA_HOME/owl`(既定 `~/.local/share/owl`)を確認 | `cookies.sqlite` が作成されている(`set_persistent_storage(…, Sqlite)`) | ✅ 空の `XDG_DATA_HOME` で起動 → `owl/cookies.sqlite` 生成を確認(スモークテスト) |
| M7-11 | Cookie を設定するサイト(例: ログインのある任意サイト)にアクセスし、owl を終了 → 再起動して同サイトへ | セッションが保持され、再ログインを求められない(Cookie が SQLite に永続化されている) | ✅ 目視確認(2026-07-23)。ログインのあるサイトでログイン → `:quit` → 再起動して同サイトへ、再ログインを求められずセッション保持(`cookies.sqlite` に SQLite 永続化) |

### M7.3 TLS Fail とエラーページ(design §8.3・§8.6)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M7-20 | `./result/bin/owl https://expired.badssl.com/`(証明書無効サイト) | 読み込み失敗 → 最小エラーページ(`TLS certificate error`・対象 URL・「press r to reload」)。例外許可 UI は出ない(要求 3.1) | ✅ 目視確認(2026-07-23)。`expired.badssl.com` で読み込み失敗 → `TLS certificate error` のエラーページを表示。例外許可 UI は出ず(要求 3.1) |
| M7-21 | `./result/bin/owl https://this-domain-does-not-exist.invalid/`(名前解決失敗) | 最小エラーページ(エラー種別・対象 URL・リロード案内)。`load-failed` 経由 | ✅ 目視確認(2026-07-23)。存在しないドメインで `load-failed` 経由の最小エラーページを表示(`Error resolving "this-domain-does-not-exist.invalid": Name or service not known`) |
| M7-22 | エラーページ表示中に `r` を押す | 対象 URL を再読み込みする(`WebView::reload`)。ネットワーク復旧後は正常表示へ復帰 | ✅ 目視確認(2026-07-23)。エラーページで `r` を押すと対象 URL を再読み込み(`.invalid` は再度同じエラーページ = リロード実行を確認)。正常ドメインでは正常表示へ復帰 |
| M7-23 | エラーページに攻撃的な URL(`https://x/<script>alert(1)</script>` 等)でアクセス | スクリプトが実行されず、URL がエスケープされてテキスト表示される(§8.6・規約 6 の HTML エスケープ) | ✅ 目視確認(2026-07-23)。`https://x/<script>alert(1)</script>` でエラーページ表示。`alert(1)` は**出ず**(スクリプト未実行)、`<script>alert(1)</script>` が文字としてそのまま表示(HTML エスケープ有効、規約 6) |

### M7.4 クラッシュ復帰(design §8.6)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M7-24 | 任意ページ表示中に WebProcess を kill(`pkill -f WebKitWebProcess` 等)してクラッシュを誘発 | エラーページ(`renderer crashed`・現在 URL・リロード案内)が表示される(`web-process-terminated`) | ✅ 目視確認(2026-07-23)。`pkill -f WebKitWebProcess` で WebProcess を kill → 白画面放置されず `renderer crashed` のエラーページを表示(`web-process-terminated`) |
| M7-25 | クラッシュ後のエラーページで `r` | WebProcess が再 spawn され、ページが再読み込みされる | ✅ 目視確認(2026-07-23)。クラッシュ後のエラーページで `r` を押すと WebProcess が再 spawn され元のページが再読み込み・復帰 |

### M7.5 新規ウィンドウ抑制(design §8.4)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M7-30 | `target="_blank"` のリンク、または `window.open('https://example.com')` を実行するページ | 新規ウィンドウ/タブを作らず、現在の WebView で当該 URL へ遷移する(`connect_create` → `popup_navigation_uri` → `load_uri` → `None`) | ✅ 目視確認(2026-07-23)。`target="_blank"` リンク/`window.open` で新規ウィンドウを作らず現在の WebView で当該 URL へ遷移 |
| M7-31 | URI を伴わない `window.open()`(空)、または `window.open('javascript:alert(1)')`/`window.open('data:text/html,...')` | 何も起きない(新規ウィンドウを開かせないことが目的。URI が無い/`javascript:`/`data:` の要求は握り潰す。§8.4・規約 6)。**バグではなく仕様** | ⚪ 未遭遇(2026-07-23)。攻撃的ページ未用意のため実地未再現。握り潰しロジック(`popup_navigation_uri` の `javascript:`/`data:`/空 URI 拒否)は実装・ユニットテスト済み(test.md POP-01〜05)。仕様として MVP 許容 |

### M7.6 ダウンロードキャンセル(design §8.5)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M7-32 | ダウンロードが始まるリンク(例: 任意のファイル直リンク)をクリック | ダウンロードは即時キャンセルされ(`Download::cancel`)、**現在ページは表示されたまま**、ステータスバーのメッセージ欄に `download blocked: <ファイル名>` が数秒(`DOWNLOAD_MESSAGE_SECS`=4 秒)表示される。ファイルは保存されない。トップレベル遷移がダウンロード化した場合も `load-failed`(`PolicyError::FrameLoadInterruptedByPolicyChange`)を除外するためエラーページに置換されない | ✅ 目視確認(2026-07-23)。ファイル直リンクのクリックでダウンロードは即時キャンセル、現在ページは表示されたまま、メッセージ欄に `download blocked: <ファイル名>` が数秒表示。ファイルは保存されず、エラーページにも置換されない |
| M7-33 | (既知の制限)ファイル名は URI の最後のパスセグメント由来 | `Content-Disposition` の suggested filename ではなく URI basename を表示する(download-started 時点で応答が未着のため)。末尾スラッシュ等でセグメントが空なら URI 全体を表示。`download.request()` が無い/空 URI では `download blocked: `(ファイル名空)になりうる。**MVP 許容**(メッセージ欄は ellipsize 済み)。**同一ファイル名**のブロックを 4 秒以内に 2 回行うと 1 回目のタイマーが 2 回目の表示を早期に消すことがある(Low L-2、MVP 許容) | ✅ 目視確認(2026-07-23)。M7-32 のメッセージ `download blocked: <ファイル名>` のファイル名が URL 末尾のパスセグメントと一致。空セグメント/同名 2 連続などの端ケースは未遭遇(既知の制限、MVP 許容) |

---

## M8: dogfooding

ゴール(design §16-8・§14・要求 §3.4): 「チェックリスト消化 → 1 週間常用」。M8 は**コード実装を持たない
最終検収マイルストーン**で、M1〜M7 の手動確認(本ファイルの各節)を実機 `./result/bin/owl` で消化し、
作者が 1 週間 owl を日常使いして **MVP 完了**(要求 §3.4)を判定する。

> 設計方針(design §14):「結合・E2E は自動化しない。マイルストーンごとの手動確認と、MVP 完了条件で
> ある 1 週間の dogfooding(要求 3.4)を検証とする」。M8 はこの検証そのものを回すフェーズであり、
> 新しい確認項目を発明するのではなく、**既存 M1〜M7 の項目を消化する**ことが本体である。

### M8.1 チェックリスト消化(要求 §3.1〜3.3 = design §16 の M1〜M7)

M2〜M7 の未消化項目(各節で「(実施予定)」のもの)を実機で消化する。各行の期待結果は**参照先の節に
詳述済み**なのでここでは再掲しない(消化時は参照先セルの「結果」列へ実挙動を記入する。記入は
「設計の理想」ではなく「実装の実挙動」で書く — CLAUDE.md 規約 9)。M1 は 2026-07-18 に消化済みのため対象外。

| 消化 | 対象 | 項目 ID | 参照節 |
|---|---|---|---|
| [x] | M2 ステータスバー | M2-10〜M2-13 | §M2.2 |
| [x] | M3 モードとキーバインド | M3-10〜M3-53 | §M3.2〜§M3.6 |
| [x] | M4 command モード | M4-10〜M4-32 | §M4.2〜§M4.4 |
| [x] | M5 hint モード | M5-10〜M5-24 | §M5.2〜§M5.3 |
| [x] | M6 insert 自動移行 | M6-10〜M6-23 | §M6.2〜§M6.3 |
| [x] | M7 堅牢化 | M7-11・M7-20〜M7-33 | §M7.2〜§M7.6 |

| ID | 判定 | 結果 |
|---|---|---|
| M8-01 | 上記 M2〜M7 の全項目が消化され、各参照節の「結果」列が「(実施予定)」でなく実挙動で埋まっている(要求 3.1〜3.3 の全機能が実機で動作) | ✅ 達成(2026-07-23)。M2〜M7 の全項目を消化。大半 ✅、既知の制限で実地再現しにくい 5 項目(M3-52・M5-21・M5-23・M6-20・M7-31)は ⚪ 未遭遇として仕様/テスト確認済みで記録。要求 3.1〜3.3 の全機能が実機で動作 |

### M8.2 1 週間常用ログ(要求 §3.4)

owl を日常閲覧(検索・記事閲覧・ログインが必要なサービスの利用)の中心に据えて 1 週間運用し、
各日 1 行を記録する。「致命的支障」の定義は §M8.3 を参照(タブ不在起因の不便は**含めない**)。

| ID | 日付 | 主な用途(検索/記事/ログイン等) | 遭遇した問題 | 致命的支障 (y/n) | メモ |
|---|---|---|---|---|---|
| M8-10 | | | | | |
| M8-11 | | | | | |
| M8-12 | | | | | |
| M8-13 | | | | | |
| M8-14 | | | | | |
| M8-15 | | | | | |
| M8-16 | | | | | |

### M8.3 致命的支障ログ(要求 §3.4)

常用中に発生した**クラッシュ・操作不能**などを記録する。要求 §3.4 の除外規定に注意:
**「タブ不在に起因する不便は『致命的な支障』に数えない」**(タブは MVP 完了後の最初の拡張のため)。
致命的支障が 1 件でも残れば MVP 未完了(要求 §3.4)。

| ID | 発生日 | 事象 | 再現手順 | 分類(致命的/非致命的) | 備考(対処・issue) |
|---|---|---|---|---|---|
| M8-20 | | | | | |

### M8.4 MVP 完了判定(要求 §3.4)

| ID | 判定 | 結果 |
|---|---|---|
| M8-30 | M8.1 の全項目が消化済み(要求 3.1〜3.3 の全項目が動作する) | ✅ 達成(2026-07-23)。M8-01 のとおり M2〜M7 の全項目を消化済み(要求 3.1〜3.3 の全機能が実機で動作) |
| M8-31 | M8-10〜M8-16 の 1 週間を owl 中心で運用し、致命的支障(M8.3、タブ不在起因を除く)がゼロ | (実施予定) |
| M8-32 | M8-30・M8-31 を満たし **MVP 完了**と判定。将来拡張(タブ管理, 要求 §4)へ進む | (実施予定) |
