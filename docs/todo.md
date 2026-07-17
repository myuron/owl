# owl — TDD 実装 TODO リスト

[test.md](test.md) のテスト項目を TDD ワークフローで実装するための手順書。

## ワークフロー(各サイクル共通)

1. **Red** — テストを先に書き、`cargo test` が**失敗する**ことを確認する。テスト対象の関数・型はまだ存在しないため、まずコンパイルエラーになることを確認する(テストが正しく対象を参照している証拠)。
2. **Green** — テストを通す最小限の実装を書き、`cargo test` が全部通ることを確認する。
3. **Refactor** — テストを green に保ったまま、コードを整理する。1 変更ごとに `cargo test` を回す。

対象は GTK 不要の純粋ロジックのみ(test.md §3 の項目は対象外。手動確認は docs/checklist.md)。

---

## サイクル 1: `parse_open_input`(command.rs)

### 1-R. Red — テストを書き、コンパイルエラーを確認する

- [x] `src/command.rs` を新規作成し、`src/main.rs` に `mod command;` を追加する(実装はまだ書かない)
- [x] `src/command.rs` に `#[cfg(test)] mod tests` を作り、`use super::parse_open_input;` で未実装の関数を参照するテストを書く
  - [x] 規則 1(前処理): P-34(空文字列 → `None`)、P-36(空白のみ → `None`)、P-35(前後空白 trim → `https://example.com`)
  - [x] 規則 2(localhost): P-10、P-11、P-12、P-13(`localhost:abc` はスキームとして素通し)、P-14(`localhost.example.com` → https 補完)、P-15
  - [x] 規則 3(スキームあり): P-01〜P-05、P-06(大文字スキーム `HTTPS://` もそのまま)
  - [x] 規則 4(https 補完): P-20〜P-23、P-24(空白を含むと規則 5 へ)
  - [x] 規則 5(DuckDuckGo 検索): P-30、P-31(空白の URL エンコード)、P-32(非 ASCII の percent-encoding)、P-33(`&` `=` `?` のエンコード)
  - [x] 優先順位: P-40(規則 3 > 4)、P-41(規則 2 > 5)、P-42(規則 2 > 3)
- [x] `cargo test` を実行し、**コンパイルエラーで失敗する**ことを確認する(`parse_open_input` が未定義)

### 1-G. Green — テストを通す実装を書く

- [x] `parse_open_input(&str) -> Option<String>` を実装する(判定は test.md 冒頭の規則 1→5 を上から順に適用)
  - [x] 規則 1: trim して空なら `None`
  - [x] 規則 2: `^localhost(:数字ポート)?(/パス)?$` に `http://` を補完
  - [x] 規則 3: `^[a-zA-Z][a-zA-Z0-9+.-]*:` にマッチしたらそのまま(大文字小文字を区別しない)
  - [x] 規則 4: 空白を含まず `.` を含むなら `https://` を補完
  - [x] 規則 5: それ以外は DuckDuckGo 検索 URL(クエリを percent-encoding)
- [x] 必要な依存(正規表現・URL エンコード用クレート、または std のみでの実装)を Cargo.toml に追加する
- [x] `cargo test` が**全件 green** になることを確認する

### 1-F. Refactor — green を維持したまま整理する

- [x] 規則ごとの判定を小さなヘルパー関数に分割し、規則 1→5 の適用順が関数本体から読み取れる形にする
- [x] 正規表現をコンパイル 1 回に抑える(`OnceLock` 等)か、std のみで簡潔に書けるなら正規表現を外す
- [x] マジック文字列(`https://duckduckgo.com/?q=` 等)を定数化する
- [x] 各ステップ後に `cargo test` で green を確認する
- [x] `cargo clippy` / `cargo fmt` を通す

---

## サイクル 2: キーシーケンスの状態遷移(keys.rs)

### 2-R. Red — テストを書き、コンパイルエラーを確認する

- [x] `src/keys.rs` を新規作成し、`src/main.rs` に `mod keys;` を追加する(実装はまだ書かない)
- [x] GTK 非依存の純粋な状態遷移関数のインターフェースをテスト側から決める(例: `resolve_key(pending: Option<char>, mode: Mode, input: KeyInput) -> (KeyOutcome, Option<char>)` — アクション・伝播 Stop/Proceed・次の pending を返す)
- [x] `#[cfg(test)] mod tests` に未実装の型・関数を参照するテストを書く
  - [x] シーケンス開始: K-01(`g` → pending 記録・Stop)、K-02(`y` → pending 記録・Stop)
  - [x] シーケンス成立: K-10(`gg` → ページ先頭・pending クリア)、K-11(`yy` → URL コピー・pending クリア)
  - [x] 破棄と再解釈: K-20(`g` の後 `j` → 下スクロール)、K-21(`y` の後 `G` → 末尾)、K-22(`g` の後 `y` → pending が `y` に)、K-23(`y` の後 `g` → pending が `g` に)、K-24(未割り当てキー → 何もせず Stop)、K-25(`i` → Insert 遷移)、K-26(`:` → Command 遷移)
  - [x] Esc の排他処理: K-30(pending あり → クリアのみ、中断しない)、K-31(pending なし → `stop_loading` 相当)
  - [x] モード境界: K-40(Insert モードでは `g` を pending に記録せず Proceed)、K-41(`set_mode` 相当の遷移が pending をクリアする)
- [x] `cargo test` を実行し、**コンパイルエラーで失敗する**ことを確認する

### 2-G. Green — テストを通す実装を書く

- [x] テストで決めた型(`Mode`・アクション enum・伝播の enum 等)を定義する
- [x] 状態遷移関数を実装する(§7.3: pending 記録 → 成立 or 破棄して単独キーとして再解釈、Esc の排他処理、Insert では素通し)
- [x] pending クリアを担うモード遷移(K-41 対象)を実装する
- [x] `cargo test` が**全件 green**(サイクル 1 のテスト含む)になることを確認する

### 2-F. Refactor — green を維持したまま整理する

- [x] 「破棄 → 単独キーとして再解釈」を、単独キー解決関数の再利用(自己呼び出し)で表現し、バインド表の重複をなくす
- [x] 単独キーのバインド表(`h/j/k/l`、`G`、`i`、`:` 等)を match または表引きに一本化する
- [x] 各ステップ後に `cargo test` で green を確認する
- [x] `cargo clippy` / `cargo fmt` を通す

---

## サイクル 3: M1 スケルトン(GUI 結線)

design.md §16 のマイルストーン **M1** を実装タスクへ展開する。ゴールは
「ウィンドウ + WebView でハードコード URL が表示され、`nix build` が通る」
(§16)。同時に **`webkit6` crate の実用性(§17 の技術リスク)をコンパイル
レベルで検証する**最初の実ビルドを兼ねる。

**このサイクルは TDD ではない。** GTK/WebKit を含むため自動ユニットテストの
対象外(design §14)。検証は `nix build` の通過と手動確認(`docs/checklist.md`)
で行う。純粋ロジック(`command.rs`/`keys.rs`)は M3/M4 で結線されるまで
`#![allow(dead_code)]` のまま据え置く。

**スコープ境界(M1 で“やらない”ことを固定する):** モード/キー処理(§6・§7)は
M3、`:open`/コマンド(§11)は M4、hint(§9)は M5、TLS Fail・エラーページ・
クラッシュ復帰・ポップアップ抑制・DL キャンセル(§8.3〜8.6)は M7。M1 は
「表示されるだけ」の殻に徹する。

**引数 URL 起動は §16 では本来 M2** だが、`load_uri` の呼び出し先を切り替えるだけ
(数行)で M1 の表示確認にそのまま使えるため、**M1 へ意図的に前倒しする**。生 URL を
そのまま渡すに留め、`:open` の補完規則(§11)の適用は M4 のまま据え置く。

### 3-1. ビルド依存の追加(design §2・§15)

- [ ] `Cargo.toml` に `gtk4`(§2: `^0.11`)と `webkit6`(§2: `0.6.x`)を追加する
      (実際の最新整合バージョンは実装時に確認し、design §2 の指定に合わせる)
- [ ] `flake.nix` の devShell(`devShells.default`)に `pkgs.gtk4` /
      `pkgs.webkitgtk_6_0` / `pkgs.pkg-config` を追加する
- [ ] `nix/rust.nix` の `buildRustPackage` に以下を追加する(§15):
  - [ ] `nativeBuildInputs = [ pkg-config wrapGAppsHook4 ]`(引数に追加で受け取る)
  - [ ] `buildInputs = [ gtk4 webkitgtk_6_0 ]`
- [ ] `nix develop` に入って `cargo build` が通ることを確認する(§15 の基本ループ)
- [ ] **注意:** gtk4/webkit6 が `Cargo.toml` に入ると `just test`/`lint`/`coverage` も
      **コンパイルに GTK ネイティブ依存が必要**になる(テスト対象は純粋ロジックのままだが
      ビルド環境が変わる)。ローカルは `nix develop` 内、CI は devShell 経由で回す前提を保つ

### 3-2. カバレッジ/CI ゲートの GTK 除外(Justfile)

- [ ] `coverage` の `--ignore-filename-regex` を `main\.rs` から
      GTK 依存ファイル(`main\.rs`・`window\.rs`・`webview\.rs`)も除外する形へ広げる
      — GTK 依存コードは region/line 100% ゲートの対象外(純粋ロジックのみを 100% に保つ)
  - [ ] 部分一致で無関係ファイル(将来の `domain.rs` 等)を巻き込まないようアンカーする
        (例: `(^|/)src/(main|window|webview)\.rs$`)
- [ ] `just coverage` が緑のままであること(サイクル 1・2 のカバレッジが 100% を維持)を確認する

### 3-3. `webkit6` シグナル存在確認(§16・§17 のリスク前倒し)

- [ ] M7 で使う予定の §8 のシグナル/API が **コンパイルレベルで存在する**ことを確認する
      (実装はしない。存在しなければ §2 のフォールバック再検討 or 設計改訂の判断材料にする):
  - [ ] シグナル: `WebView::connect_create`(§8.4 ポップアップ抑制)
  - [ ] シグナル: `connect_load_failed` / `load-failed-with-tls-errors`(§8.3・§8.6)
  - [ ] シグナル: `connect_web_process_terminated`(§8.6 クラッシュ復帰)
  - [ ] シグナル: `NetworkSession::connect_download_started`(§8.5)
  - [ ] API: `NetworkSession::set_tls_errors_policy`(§8.3 TLS Fail)
  - [ ] API: `WebView::load_alternate_html`(§8.6 エラーページ戦略の要)
  - [ ] API: `CookieManager::set_persistent_storage(…, Sqlite)`(§8.2 Cookie 永続化)
  - [ ] API: `Download::cancel`(§8.5 DL キャンセル)
  - [ ] 欠落があれば `docs/design.md §17` の表へ追記し、対処方針を書く。
        **全て確認できるまで §17 のリスクを「解消」と見なさない**

### 3-4. `main.rs`: Application 生成と起動フロー(§4・§13)

- [x] `mod window;` / `mod webview;` を追加する(本体は最小スケルトン。`window` は
      空ウィンドウ提示、`webview` は §4 のプレースホルダ。肉付けは 3-5 / 3-6)
- [x] `gtk::Application::new(Some("dev.myuron.owl"), NON_UNIQUE)` を生成する(§13-1)
- [x] `activate` シグナルでウィンドウを構築する(§13-2)
- [x] `std::env::args` の第 1 引数があればその URL、なければ `about:blank` を初期 URL にする(§13-3)
      — 引数 URL は §16 では M2 だが本節冒頭の通り M1 へ前倒し。`parse_open_input` は
      未結線でよい(補完規則の適用は M4)。生 URL をそのまま渡す。判定は GTK 非依存の
      純粋関数 `command::initial_uri` に切り出し、TDD で単体テスト済み(`s13_*`)
- [x] `println!("Hello, world!")` を置き換える

### 3-5. `window.rs`: ウィンドウとレイアウト(§4・§5)

- [ ] `gtk::ApplicationWindow` を生成する
- [ ] 直下に縦 `gtk::Box` を置き、`webkit6::WebView`(`vexpand = true`)を追加する(§5-1)
- [ ] ステータスバー(§5-2)・コマンドライン(§5-3)は **M1 ではプレースホルダも省略可**
      (本実装はステータスバー = M2、コマンドライン = M4)。ここで足すなら空の枠に留める
- [ ] `window.present()` で表示する

### 3-6. `webview.rs`: WebView 生成と設定(§4・§8.7)

- [ ] `webkit6::NetworkSession` を生成する(§8.2: data=`user_data_dir()/owl`、
      cache=`user_cache_dir()/owl`。M1 は最小結線でよく、Cookie 永続化の作り込みは M7)
- [ ] NetworkSession を紐付けて `webkit6::WebView` を生成する
- [ ] `webkit6::Settings` で `enable_developer_extras = true` のみ明示する(§8.7)
- [ ] `web_view.load_uri(初期 URL)` でページを表示する
- [ ] 起動パスで同期 I/O は XDG ディレクトリ作成以外行わない(§13-4)

### 3-7. 手動確認(docs/checklist.md)

- [ ] `docs/checklist.md` を **新規作成**し、M1 の確認項目を書く(design §14:
      「手動確認用チェックリストを実装時に用意する」。このファイルはまだ存在しない)
  - [ ] `nix build` が成功する(§16 の M1 完了条件)
  - [ ] `./result/bin/owl https://example.com` で当該ページが表示される
  - [ ] 引数なし起動で `about:blank` になる
  - [ ] `webkit6` の必要シグナルがコンパイルレベルで揃っている(3-3 の結果を記録)

### 3-8. M1 完了条件

- [ ] `just ci`(`fmt-check → lint → coverage → build`)が緑になる
- [ ] `nix build` が通り、起動してハードコード/引数 URL のページが表示される
- [ ] design §2 の GTK4 構成が実ビルドで確定した(§16: 「ここで GTK4 構成の最終確定」)。
      致命的問題があれば §17・§2 のフォールバック判断を design.md に反映する

---

## 完了条件(サイクル 1・2: 純粋ロジック)

- [x] test.md の全 ID(P-01〜P-42、K-01〜K-41)に対応するテストが存在し、すべて green
- [x] `cargo test` が GTK なしで完結する(テスト対象が gtk/webkit クレートに依存していない)
- [x] `cargo clippy` 警告なし、`cargo fmt --check` 通過
