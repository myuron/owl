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

- [x] `Cargo.toml` に `gtk4`(§2: `^0.11`)と `webkit6`(§2: `0.6.x`)を追加する
      (実際の最新整合バージョンは実装時に確認し、design §2 の指定に合わせる)
- [x] `flake.nix` の devShell(`devShells.default`)に `pkgs.gtk4` /
      `pkgs.webkitgtk_6_0` / `pkgs.pkg-config` を追加する
- [x] `nix/rust.nix` の `buildRustPackage` に以下を追加する(§15):
  - [x] `nativeBuildInputs = [ pkg-config wrapGAppsHook4 ]`(引数に追加で受け取る)
  - [x] `buildInputs = [ gtk4 webkitgtk_6_0 ]`
- [x] `nix develop` に入って `cargo build` が通ることを確認する(§15 の基本ループ)
- [x] **注意:** gtk4/webkit6 が `Cargo.toml` に入ると `just test`/`lint`/`coverage` も
      **コンパイルに GTK ネイティブ依存が必要**になる(テスト対象は純粋ロジックのままだが
      ビルド環境が変わる)。ローカルは `nix develop` 内、CI は devShell 経由で回す前提を保つ

### 3-2. カバレッジ/CI ゲートの GTK 除外(Justfile)

- [x] `coverage` の `--ignore-filename-regex` を `main\.rs` から
      GTK 依存ファイル(`main\.rs`・`window\.rs`・`webview\.rs`)も除外する形へ広げる
      — GTK 依存コードは region/line 100% ゲートの対象外(純粋ロジックのみを 100% に保つ)
  - [x] 部分一致で無関係ファイル(将来の `domain.rs` 等)を巻き込まないようアンカーする
        (例: `(^|/)src/(main|window|webview)\.rs$`)
- [x] `just coverage` が緑のままであること(サイクル 1・2 のカバレッジが 100% を維持)を確認する

### 3-3. `webkit6` シグナル存在確認(§16・§17 のリスク前倒し)

- [x] M7 で使う予定の §8 のシグナル/API が **コンパイルレベルで存在する**ことを確認する
      (実装はしない。存在しなければ §2 のフォールバック再検討 or 設計改訂の判断材料にする)
      — `examples/webkit_api_probe.rs` プローブを `cargo build --example` で解決、全 8 個成功後に削除:
  - [x] シグナル: `WebView::connect_create`(§8.4 ポップアップ抑制)
  - [x] シグナル: `connect_load_failed` / `load-failed-with-tls-errors`(§8.3・§8.6)
        — 戻り値は `Propagation` でなく `bool`(checklist M1-31・design §17 に記録)
  - [x] シグナル: `connect_web_process_terminated`(§8.6 クラッシュ復帰)
  - [x] シグナル: `NetworkSession::connect_download_started`(§8.5)
  - [x] API: `NetworkSession::set_tls_errors_policy`(§8.3 TLS Fail)
  - [x] API: `WebView::load_alternate_html`(§8.6 エラーページ戦略の要)
  - [x] API: `CookieManager::set_persistent_storage(…, Sqlite)`(§8.2 Cookie 永続化)
  - [x] API: `Download::cancel`(§8.5 DL キャンセル)
  - [x] 欠落があれば `docs/design.md §17` の表へ追記し、対処方針を書く。
        **全て確認できるまで §17 のリスクを「解消」と見なさない**
        — 欠落なし。§17 の webkit6 成熟度リスク行を「解消済み」へ更新

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

- [x] `gtk::ApplicationWindow` を生成する
- [x] 直下に縦 `gtk::Box` を置き、`webkit6::WebView`(`vexpand = true`)を追加する(§5-1)
- [x] ステータスバー(§5-2)・コマンドライン(§5-3)は **M1 ではプレースホルダも省略可**
      (本実装はステータスバー = M2、コマンドライン = M4)。M1 では省略(WebView のみ配置)
- [x] `window.present()` で表示する

### 3-6. `webview.rs`: WebView 生成と設定(§4・§8.7)

- [x] `webkit6::NetworkSession` を生成する(§8.2: data=`user_data_dir()/owl`、
      cache=`user_cache_dir()/owl`。M1 は最小結線でよく、Cookie 永続化の作り込みは M7)
      — パス算出は純粋関数 `command::app_subdir` に切り出し、TDD で単体テスト済み(`s82_*`)
- [x] NetworkSession を紐付けて `webkit6::WebView` を生成する
- [x] `webkit6::Settings` で `enable_developer_extras = true` のみ明示する(§8.7)
- [x] `web_view.load_uri(初期 URL)` でページを表示する
- [x] 起動パスで同期 I/O は XDG ディレクトリ作成以外行わない(§13-4)

### 3-7. 手動確認(docs/checklist.md)

- [x] `docs/checklist.md` を **新規作成**し、M1 の確認項目を書く(design §14:
      「手動確認用チェックリストを実装時に用意する」)
  - [x] `nix build` が成功する(§16 の M1 完了条件)
  - [x] `./result/bin/owl https://example.com` で当該ページが表示される(目視 + TLS 接続で裏取り)
  - [x] 引数なし起動で `about:blank` になる
  - [x] `webkit6` の必要シグナルがコンパイルレベルで揃っている(3-3 の結果を記録)
        — 3-3 完了。§8 の全 8 シグナル/API が存在(checklist M1.4)

### 3-8. M1 完了条件

- [x] `just ci`(`fmt-check → lint → coverage → build`)が緑になる(checklist M1-03)
- [x] `nix build` が通り、起動してハードコード/引数 URL のページが表示される(checklist M1-02・M1-10・M1-11)
- [x] design §2 の GTK4 構成が実ビルドで確定した(§16: 「ここで GTK4 構成の最終確定」)。
      致命的問題があれば §17・§2 のフォールバック判断を design.md に反映する
      — 致命的問題なし。§17 の webkit6 成熟度リスクを「解消済み」へ更新(フォールバック不要)

---

## サイクル 4: M2 ステータスバー

design.md §16 のマイルストーン **M2**「ナビゲーションとステータスバー」を実装タスクへ
展開する。ゴールは「`owl <url>` 起動でステータスバーに URL・ページタイトル・読み込み状態が
表示され、リンク遷移で追従更新される」(§5-2・§12)。

**スコープ境界:** M2 のうち引数 URL 起動は M1 へ前倒し済み。**ナビゲーション(戻る/進む/
リロード/中断)は M2 では扱わない** — 要求 §3.2 のとおりナビゲーションはキーバインド駆動で、
そのトリガ(`EventControllerKey`)は **M3** の範囲(M2 に他のトリガ手段は無い: ツールバー
無し §5、command モードは M4)。`keys.rs` の `Action::{Back,Forward,Reload,StopLoading}` は
M3 で結線する。モードインジケータの内容更新(`set_mode` → ラベル)も M3(M2 はラベル枠のみ・
常に空)。ステータスバーは WebView の property notify にバインドする自己完結機能で、マウスの
リンククリック(要求上マウスは無効化しない)で更新を手動検証できる。

### 4-R. Red — テストを書き、コンパイルエラーを確認する

- [x] `command.rs` の `#[cfg(test)] mod tests` に、未実装の `format_load_progress` を参照する
      `s12_*` テストを書く(§12。起動ヘルパー `s13_*`/`s82_*` と同じく design セクション引用の
      テスト命名にする)。相異なる分岐を全て固定する(CLAUDE.md 規則 2・4):
  - [x] `s12_not_loading_is_empty`(`is_loading=false` は progress 値に依らず空)
  - [x] `s12_loading_formats_percent`(`(true, 0.42)` → `[42%]`、四捨五入)
  - [x] `s12_loading_zero_percent`(`(true, 0.0)` → `[0%]`)
  - [x] `s12_loading_full`(`(true, 1.0)` → `[100%]`)
- [x] `cargo test` が**コンパイルエラーで失敗する**ことを確認する(`format_load_progress` 未定義)

### 4-G. Green — テストを通す実装を書く

- [x] `command.rs` に `format_load_progress(is_loading: bool, progress: f64) -> String` を実装する
      (読み込み中は `[NN%]`、非読み込み時は空。§12)。モジュール doc をステータスバー用途へ拡張
- [x] `cargo test` が**全件 green**(サイクル 1・2 含む)になることを確認する

### 4-F. Refactor — green を維持したまま整理する

- [x] 実装は最小(分岐 1 つ + フォーマット)のため追加のリファクタは不要
- [x] `just coverage` で `command.rs`/`keys.rs` の region/line 100% 維持を確認する
- [x] `cargo clippy`(-D warnings)/ `cargo fmt --check` を通す

### 4-結線. GTK 結線(TDD 対象外、手動確認)

GTK/WebKit を含むためユニットテスト対象外(design §14)。`window.rs` はカバレッジ除外済み
(Justfile 変更不要)。検証は手動確認(docs/checklist.md M2)で行う。

- [x] `window.rs` にステータスバー(高さ 1 行の横 `gtk::Box`)を組み立て、縦 `Box` の
      WebView 直下に append する(§5-2)
  - [x] ラベル 4 つ: モードインジケータ(空)・URL(ellipsize・hexpand)・タイトル
        (ellipsize)・読み込み状態(右端)
  - [x] 初期値を WebView の現在プロパティ(`uri`/`title`/`is_loading`/
        `estimated_load_progress`)から設定
- [x] notify 結線(§12、ポーリングしない): `notify::uri`/`title`/`is-loading`/
      `estimated-load-progress`。読み込み状態は `command::format_load_progress` で組む。
      各クロージャへは更新対象ラベルのみ clone(§3.3)
- [x] `gtk::CssProvider` で最小 CSS(配色・等幅フォント)を適用(§5)
- [x] `cargo build` が通ることを確認する

### 4-手動確認(docs/checklist.md M2)

- [x] `docs/checklist.md` に M2 セクション(M2-01〜M2-13)を追記する
- [ ] `nix build` した `./result/bin/owl` で ステータスバー表示・リンククリック追従・
      引数なし起動を**目視確認**する(checklist M2-10〜M2-13、ユーザー環境で実施)

### 4-完了条件

- [x] `just ci`(fmt-check → lint → coverage → mutants → build)が緑になる
      (mutants ゲート追加後も緑。coverage は command.rs/keys.rs 100% 維持)
- [ ] `nix build` した起動でステータスバーに URL・タイトル・読み込み状態が表示される
      (checklist M2-10〜M2-13 の目視確認・上記 4-手動確認と同一)

---

## サイクル 5: M3 モードとキーバインド

design.md §16 のマイルストーン **M3**「モードとキーバインド」を実装タスクへ展開する。ゴールは
「モード管理・Normal のバインド一式(スクロール含む)・ナビゲーション(戻る/進む/リロード/中断)・
モードインジケータ更新・Insert(手動 `i`/`Esc`)」(§6・§7・§8.1・§12)。

**スコープ境界:** `:`(Command)/`f`(Hint) は M3 では **inert**(pending クリアのみで Normal 維持)—
Command/Hint モードは M4/M5 で本結線(§7.2: Command は Esc も Proceed するため未結線で遷移すると
復帰不能)。`:open` 補完(§11)は M4、insert 自動移行(§10)は M6、永続化・エラーページ等(§8)は M7。

**方針:** 純粋ロジック(`keys.rs`)と GTK 結線(`input.rs`)を分離(§4・§14)。純粋部は TDD +
100% coverage + mutants ゲート、GTK 結線部は手動確認(checklist M3)。

### 5-A. 純粋 `keys.rs` の拡張(TDD、§7.2 の完全実装)

- [x] `docs/test.md §2` に新 ID を追記(§2.6 修飾キー K-50〜K-57、§2.7 モード別非文字キー K-60〜K-66、
      §2.8 `classify_input` C-01〜C-08、§2.9 `scroll_script` S-01〜S-09、§2.10 `mode_indicator` M-01〜M-04)
- [x] **Red**: 新 `Action`/`KeyInput` 変種・純粋関数を参照するテストを書き、コンパイルエラーを確認
- [x] **Green**: `Action` に `ScrollHalfDown`/`ScrollHalfUp`、`KeyInput` に `Ctrl(char)`/`SpecialBare`/
      `OtherModified` を追加。`resolve_normal` を全変種 match に書換(§7.2: Ctrl+d/u のみ Stop・他 Ctrl は
      Proceed、SpecialBare は Stop、OtherModified は Proceed、修飾系は pending 破棄)。Insert/Hint アームを
      `Esc` 以外の全変種へ一般化
- [x] **Green**: 純粋関数 `scroll_script`(§7.4 の量・§8.1 の `behavior:'instant'` の厳密 JS)、
      `mode_indicator`(§5-2)、`classify_input`(GTK keyval+修飾 → `KeyInput`、§7.1/§7.2)を実装
- [x] **Refactor**: `just coverage`(command.rs/keys.rs region/line 100% 維持)・`just mutants`
      (survivor なし)・`cargo clippy`(-D warnings)/`cargo fmt --check` を通す

### 5-B. GTK 結線(TDD 対象外、design §14。手動確認 = checklist)

- [x] `Justfile` の `coverage` 除外正規表現に `input` を追加(GTK 結線は 100% ゲート対象外)
- [x] `src/input.rs` を新規作成: `Rc<Cell<AppState{mode,pending_key}>>` を持ち、`EventControllerKey` を
      ウィンドウに **capture phase**(§7.1)で取り付ける。keyval+修飾 → `keys::classify_input` →
      `keys::resolve_key` → Action ディスパッチ(スクロール JS `evaluate_javascript`、ナビゲーション
      `go_back`/`go_forward`/`reload`/`stop_loading`、`CopyUrl` は `clipboard().set_text`、`EnterMode` は
      `apply_enter_mode`)→ 状態書戻し → `glib::Propagation` を返す
- [x] `apply_enter_mode`: Insert はインジケータ更新、Normal は `document.activeElement.blur()` 評価 +
      `grab_focus` + インジケータ空(§6)、Command/Hint は inert(現モード維持)
- [x] `src/window.rs`: `build_status_bar` を `(Box, Label)` へ変更しモードインジケータのラベルを返す。
      `build` で `input::install(&window, &web_view, &mode_label)` を呼ぶ
- [x] `src/main.rs`: `mod input;` を追加
- [x] `nix develop` 内で `cargo build`/`cargo clippy` が通ることを確認

### 5-手動確認(docs/checklist.md M3)

- [x] `docs/checklist.md` に M3 セクションを追記する
- [ ] `nix build` した `./result/bin/owl` でモード遷移・スクロール・ナビゲーション・yy コピー・
      Insert 入力・`:`/`f` の inert を**目視確認**する(checklist M3、ユーザー環境で実施)

### 5-完了条件

- [x] `just ci`(fmt-check → lint → coverage → mutants → build)が緑になる
- [ ] `nix build` した起動で M3 のキーバインドが期待どおり動く(checklist M3 の目視確認)

---

## サイクル 6: M4 command モード

design.md §16 のマイルストーン **M4**「command モード」を実装タスクへ展開する。ゴールは
「`:` でコマンドライン(Entry)を開き、`:open <input>` で補完済み URL を開ける・`:quit` で
終了できる・未知コマンドはステータスバーにエラー表示」(§11・§5-3)。

**スコープ境界:** `:open` の補完規則(`parse_open_input`)は M1 で実装・テスト済み。M4 は
その**コマンドディスパッチ**(`parse_command`)と**コマンドライン UI の GTK 結線**を担う。
M3 で inert だった `:`(Command 遷移)を本結線する。`f`(Hint)は M5 まで inert のまま据え置く。
insert 自動移行(§10)は M6、エラーページ等(§8)は M7。

**方針:** 純粋ロジック(`command::parse_command`)と GTK 結線(`input`)を分離(§4・§14)。
純粋部は TDD + 100% coverage + mutants ゲート、GTK 結線部は手動確認(checklist M4)。

### 6-A. 純粋 `parse_command` の実装(TDD、§11)

- [x] `docs/test.md §1.7` に新 ID CMD-01〜CMD-11 を追記(コマンドディスパッチ)
- [x] **Red**: 未実装の `Command` enum・`parse_command` を参照する `cmd01_*`〜`cmd11_*` テストを書き、
      コンパイルエラーを確認。相異なる分岐(`Open`/`Quit`/`Noop`/`Error`)とエラー文字列を厳密に固定
      (CLAUDE.md 規約 2・4)
- [x] **Green**: `command.rs` に `Command`(`Open`/`Quit`/`Noop`/`Error`)と `parse_command` を実装
      (先頭 `:` を 1 個剥がし trim → 最初の空白で分割 → `open`/`quit`/未知。`:open` 引数は
      `parse_open_input` で補完)。`parse_open_input` の結線で dead_code 解消 → モジュールの
      `#![allow(dead_code)]` を撤去し doc を実態へ更新
- [x] **Refactor**: `just coverage`(command.rs/keys.rs region/line 100% 維持)・`just mutants`
      (survivor なし)・`cargo clippy`(-D warnings)/`cargo fmt --check` を通す

### 6-B. GTK 結線(TDD 対象外、design §14。手動確認 = checklist)

- [x] `src/window.rs`: コマンドライン Entry(`build_command_line`、初期非表示 §5-3)を縦 Box の
      ステータスバー直下に append。ステータスバーにメッセージ欄(エラー表示 §11、`.owl-message`
      警告色)を追加し `build_status_bar` を `(Box, Label, Label)` へ変更(mode・message を返す)
- [x] `src/input.rs`: `install` に `command_entry`/`message_label` を追加し、中心状態 `Rc<Cell<AppState>>`
      を window key コントローラと Entry の activate/Esc の 3 ハンドラで共有。Command 遷移で Entry を
      `:` 初期値・非選択フォーカス(`grab_focus_without_selecting` + `set_position(-1)`)・メッセージ
      クリアで開く。`activate` で `parse_command` を実行(`Open`→`load_uri`・`Quit`→`window.close()`・
      `Noop`→無視・`Error`→メッセージ表示)、Entry 上の Esc コントローラでキャンセル。いずれも
      `leave_command`(Entry 非表示・フォーカス復帰・Normal 復帰)で締める
- [x] `nix develop` 内で `cargo build`/`cargo clippy` が通ることを確認

### 6-手動確認(docs/checklist.md M4)

- [x] `docs/checklist.md` に M4 セクションを追記する
- [ ] `nix build` した `./result/bin/owl` で `:` 起動・`:open`(補完各種)・`:quit`・未知コマンドの
      エラー表示・Esc キャンセルを**目視確認**する(checklist M4、ユーザー環境で実施)

### 6-完了条件

- [x] `just ci`(fmt-check → lint → coverage → mutants → build)が緑になる
- [ ] `nix build` した起動で M4 の command モードが期待どおり動く(checklist M4 の目視確認)

---

## 完了条件(サイクル 1・2: 純粋ロジック)

- [x] test.md の全 ID(P-01〜P-42、K-01〜K-41)に対応するテストが存在し、すべて green
- [x] `cargo test` が GTK なしで完結する(テスト対象が gtk/webkit クレートに依存していない)
- [x] `cargo clippy` 警告なし、`cargo fmt --check` 通過
