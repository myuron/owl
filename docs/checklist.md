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
