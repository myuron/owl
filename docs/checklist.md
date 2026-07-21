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
| M2-10 | `./result/bin/owl https://example.com` | ウィンドウ下部に 1 行のステータスバー。URL 欄に `https://example.com`、読み込み中は右端に `[NN%]`(完了で空)、タイトル欄にページタイトルが表示される(§12: notify にバインド) | (実施予定) |
| M2-11 | 表示ページ内のリンクをマウスでクリック | URL 欄・タイトル欄が遷移先へ追従更新される(§12: `notify::uri`/`title`) | (実施予定) |
| M2-12 | `./result/bin/owl`(引数なし) | URL 欄が `about:blank`、タイトル欄は空、モードインジケータは空(normal・M2 では常に空) | (実施予定) |
| M2-13 | 表示されたウィンドウ | ツールバー・メニューバー無し。WebView が上部を占有し(`vexpand`)、下部にステータスバーが 1 行。配色・等幅フォントの CSS が効いている(§5) | (実施予定) |

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
| M3-10 | ページ表示後、WebView 本体をクリックしてフォーカスを移し、`j` を押す | WebView にフォーカスがあってもウィンドウの capture phase コントローラが先にキーを受け取り、下スクロールする(§7.1: capture(親→子)) | (実施予定) |

### M3.3 スクロール(design §7.4・§8.1)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M3-20 | 縦長ページで `j`/`k`/`h`/`l` | 下/上/左/右へ 50px ずつスクロール(`behavior:'instant'`) | (実施予定) |
| M3-21 | `gg` / `G` | ページ先頭 / 末尾へ | (実施予定) |
| M3-22 | `Ctrl+d` / `Ctrl+u` | 半ページ(ビューポート高の 1/2)下 / 上へ | (実施予定) |

### M3.4 ナビゲーション・コピー・中断(design §7.4)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M3-30 | リンクを辿った後 `H` → `L` | 戻る → 進む(`WebView::go_back`/`go_forward`)。ステータスバーの URL が追従 | (実施予定) |
| M3-31 | `r` | リロード(`WebView::reload`) | (実施予定) |
| M3-32 | 読み込み中に `Esc`(pending なし) | 読み込み中断(`WebView::stop_loading`)。読み込み状態表示が止まる | (実施予定) |
| M3-33 | `yy` | 現在ページの URL がクリップボードへコピーされる(他アプリに貼り付けて確認) | (実施予定) |

### M3.5 モード遷移・インジケータ(design §6・§5-2・§12)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M3-40 | `i` を押す | ステータスバー左端に `-- INSERT --`。以後キー入力がページ(フォーム等)へ届く | (実施予定) |
| M3-41 | フォーム入力欄にフォーカスした状態で `Esc` | Normal に戻り、入力欄の focus が外れる(`document.activeElement.blur()` + WebView へ grab_focus)。インジケータが空になる | (実施予定) |
| M3-42 | `g` を押した直後に `Esc` | pending クリアのみ(読み込み中断は起きない。§7.3)。続く `g` は新規シーケンス開始 | (実施予定) |

### M3.6 スコープ外の安全確認・既知の制限

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M3-50 | `f` を押す | 現状 **inert**: 何も起きず、モードは Normal のまま(トラップしない)。M5 で本結線。※`:`(Command)は M4 で本結線済み — 挙動は M4-10〜M4-24 で確認する | (実施予定) |
| M3-51 | Normal モードで矢印キー / PageUp / PageDown | 消費されて無反応(§7.2: 未割当の修飾なしキーはページに漏らさない)。スクロールは `h/j/k/l`・`gg/G`・`Ctrl+d/u` を使う想定。**バグではなく仕様**(既知の制限) | (実施予定) |
| M3-52 | 内側 `div` がスクロールコンテナのページで `j` 等 | メインフレームが動かないため効かないことがある(§8.1 の既知の制限、MVP 許容)。**バグではない** | (実施予定) |
| M3-53 | Normal で選択テキストに対し `Ctrl+C` | ページへ素通しし、コピーできる(§7.2: バインド外の修飾付きは Proceed) | (実施予定) |

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
| M4-10 | Normal モードで `:` を押す | ウィンドウ最下部にコマンドライン(Entry)が現れ、初期値 `:` が入ってフォーカスされる。カーソルは `:` の後ろ(全選択されていない)。モードインジケータが `-- COMMAND --` | (実施予定) |
| M4-11 | `:` を開いた状態で `open example.com` と入力 → Enter | `https://example.com` へ遷移(§11 規則 4 の https 補完)。コマンドラインが閉じ、Normal に戻る(インジケータが空)。ステータスバーの URL が追従 | (実施予定) |
| M4-12 | `:open localhost:8080` → Enter | `http://localhost:8080` へ遷移(§11 規則 2)。※接続先が無ければ読み込み失敗でよい(補完先の確認が目的) | (実施予定) |
| M4-13 | `:open rust 所有権` → Enter | DuckDuckGo 検索(`https://duckduckgo.com/?q=...` にエンコード)へ遷移(§11 規則 5) | (実施予定) |
| M4-14 | `:quit` → Enter | ウィンドウが閉じ、プロセスが終了する(§11・§13-1: `NON_UNIQUE` の単一ウィンドウを閉じる) | (実施予定) |

### M4.3 エラー・キャンセル(design §11)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M4-20 | `:foo` → Enter | ステータスバーのメッセージ欄に `unknown command: foo`(警告色)。コマンドラインは閉じ Normal に戻る。遷移しない | (実施予定) |
| M4-21 | `:open`(引数なし)→ Enter | メッセージ欄に `usage: open <url or query>`。遷移しない | (実施予定) |
| M4-22 | `:` を開いた状態で `Esc` | 入力を破棄してコマンドラインが閉じ、Normal に戻る(§11: Entry 上の EventControllerKey)。インジケータが空。読み込み中断は起きない | (実施予定) |
| M4-23 | `:`(コロンのみ)→ Enter | 何も起きずコマンドラインが閉じ Normal に戻る(`Noop`) | (実施予定) |
| M4-24 | エラー表示後に再度 `:` を開く | 前回のエラーメッセージがクリアされる(新しい入力の開始) | (実施予定) |

### M4.4 スコープ外の安全確認

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M4-30 | `f` を押す | 現状 **inert**: 何も起きず Normal のまま(M5 で本結線) | (実施予定) |
| M4-31 | command モードで入力中に `j`/`k` 等 | ページへスクロールせず Entry へ文字入力される(§7.2: Command は全キー Proceed で Entry が処理) | (実施予定) |
| M4-32 | command モード中に WebView 本体をマウスでクリック | Entry のフォーカスは外れるがモードは Command のまま(インジケータは `-- COMMAND --` 継続、キーはページへ Proceed)。Entry をクリックし直すか、`Esc`/`Enter` で復帰できる。**バグではなく既知の制限**(M3-51/52 と同種、MVP 許容)。将来直すなら Entry の focus-leave で `leave_command` | (実施予定) |

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
| M5-10 | リンクの多いページ(例: `https://example.com` から辿った一覧)で `f` を押す | ビューポート内のクリック可能要素(`a[href]`・`button`・入力欄等)にホームロー(`sadfjklewcmpgh`)のラベルが重畳表示される。モードインジケータが `-- HINT --`。要素数に応じ 1〜2 文字ラベル(§9.3) | (実施予定) |
| M5-11 | ラベルを 1 文字タイプ(2 文字ラベルの場合は 1 文字目) | 前方一致で候補が絞り込まれ、一致しないラベルは消える(§9.2) | (実施予定) |
| M5-12 | リンクのラベルを最後までタイプして確定 | 対象リンクがクリックされ(`.click()`)遷移。オーバーレイが消え、Normal に戻る(インジケータ空)。ステータスバーの URL が追従(§9.2 `hint_result:link`) | (実施予定) |
| M5-13 | テキスト入力欄のラベルをタイプして確定 | 入力欄が focus され(`.focus()`)、Insert へ遷移(インジケータ `-- INSERT --`)。以後キー入力が入力欄へ届く(§9.2 `hint_result:input`) | (実施予定) |
| M5-14 | Hint 表示中に候補に無い文字をタイプ | 候補全滅として Normal に戻る(インジケータ空、§9.2 `hint_none`)。オーバーレイが消える | (実施予定) |

### M5.3 キャンセル・スコープ外・既知の制限(design §9・§17)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M5-20 | Hint 表示中に `Esc` | オーバーレイが消え(`owlHints.cancel()`)、Normal に戻る(インジケータ空)。読み込み中断は起きない | (実施予定) |
| M5-21 | クリック可能要素が無い/ビューポート外だけのページで `f` | 候補 0 件で即 Normal に戻る(§9.2 `hint_none`)。オーバーレイは出ない | (実施予定) |
| M5-22 | `about:blank`(引数なし起動)で `f` | page.js が注入されていれば候補 0 件で即 Normal へ戻る(`hint_none`)。UserScript が注入されない特殊ページ等では一瞬 `-- HINT --` になり `hint_none` が来ず Hint に留まる → `Esc` で復帰(§9・§17 の既知の制限。**バグではない**) | (実施予定) |
| M5-23 | クロスオリジン iframe を含むページで `f` | メインフレームの要素のみラベル表示される(iframe 内は MVP 非保証。§17)。**バグではなく既知の制限** | (実施予定) |
| M5-24 | Insert 遷移後(M5-13)に `Esc` | Normal に戻り、入力欄の focus が外れる(M3 の Insert→Normal と同じ挙動) | (実施予定) |

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
| M6-10 | テキスト入力欄(`input`/`textarea`/contenteditable)のあるページで、その入力欄をマウスでクリック | 自動で Insert へ遷移し、モードインジケータが `-- INSERT --`。以後キー入力が入力欄へ届く(§10 mousedown 相関の focusin)。**未 focus の欄・既に focus 済みの欄(`autofocus` 済みを初めてクリックする場合含む)の両方で遷移する**(後者は focusin が発火しないため mousedown 側の即時通知で拾う) | (実施予定) |
| M6-11 | `<input autofocus>` を含むページを開く | Normal のまま(インジケータ空)。`autofocus` の focusin は mousedown を伴わないため通知されない(§10・要求 3.3) | (実施予定) |
| M6-12 | JS が読み込み時に `element.focus()` を呼ぶページを開く | Normal のまま。スクリプト起因 focus は通知されない(§10・要求 3.3) | (実施予定) |
| M6-13 | M6-10 の Insert 遷移後に `Esc` | Normal に戻り、入力欄の focus が外れる(§6 Insert→Normal と同じ挙動) | (実施予定) |
| M6-14 | リンクやボタン等 editable でない要素をマウスでクリック | Normal のまま。`isEditable` が false のため focus 通知を送らない(§10) | (実施予定) |

### M6.3 スコープ外・既知の制限(design §10・§17)

| ID | 手順 | 期待結果 | 結果 |
|---|---|---|---|
| M6-20 | (既知の制限)悪意あるページが `window.webkit.messageHandlers.owl.postMessage('{"type":"focus"}')` を Normal 中に偽装送信 | owl は Insert へ入りうる。mousedown 相関は信頼できないページ側(page.js)が判定するため Normal 中の偽装は防げない。`Esc` で必ず Normal へ復帰でき実害は限定的(§17 MVP 許容、hint 偽装と同種)。**バグではなく既知の制限** | (実施予定) |
| M6-21 | Command/Hint/Insert モード中に focus メッセージ相当が届く状況(例: hint 表示中に入力欄をクリック) | Normal 以外では focus メッセージを受理しない(規約 6)。hint はモードを維持、他モードも遷移しない | (実施予定) |
| M6-22 | Insert 遷移後、ページの余白等 **editable でない領域**をマウスでクリック | 入力欄の DOM focus は外れるが、モードは Insert のまま(インジケータ `-- INSERT --`、キーはページへ素通し)。§10 は「入る」方向のみ定義。`Esc` で Normal へ復帰できる。**バグではなく既知の制限**(Low L-2、MVP 許容) | (実施予定) |
| M6-23 | `<label for=…>` 経由の focus や、ボタンを 200ms 超**長押し**してから離すクリック | focusin が mousedown から `FOCUS_WINDOW_MS`(200ms)を超えて発火するため Insert に入らないことがある。通常のクリックは窓内に収まる。**バグではなく既知の制限**(Low L-1、裁量値。MVP 許容) | (実施予定) |
