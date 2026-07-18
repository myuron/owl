# 修正計画: issue #14 — HTTPS/検索が TLS エラーになる

- Issue: #14 `bug: open <検索文字列>を行なうとエラーが表示される`
- 種別: Nix パッケージングの依存漏れ(コードのバグではない)
- 対象ブランチ: `main` から `fix/glib-networking-tls` を新規に切る

## 1. 背景 / 症状

`:open <検索語>` は `https://duckduckgo.com/?q=...`(`src/command.rs` の `DUCKDUCKGO_SEARCH`)を
生成する。この HTTPS 接続が「TLS support is not available (install glib-networking)」相当の
エラーになり、検索結果が表示されない。検索に限らず **すべての `https://` URL で失敗**する
(検索で顕在化しただけ)。

WebKitGTK(libsoup 3)は GIO の TLS バックエンドを `glib-networking` の GIO モジュールとして
`GIO_EXTRA_MODULES` 経由で動的ロードする。これが実行環境に存在しないと TLS が使えない。

## 2. 実環境での確認結果(調査済み)

issue の診断を実機で裏取りした:

1. **`./result` のランタイムクロージャに `glib-networking` が無い。**
   `nix path-info -r ./result | grep glib-networking` → ヒット無し。
2. **ラッパーが `GIO_EXTRA_MODULES` に TLS バックエンドを入れていない。**
   `makeBinaryWrapper` 生成ラッパーのソースを見ると、prefix しているのは dconf のモジュールだけ:
   `--prefix 'GIO_EXTRA_MODULES' ':' '.../dconf-.../lib/gio/modules'`。
   glib-networking は `buildInputs` に無いため `wrapGAppsHook4` が拾えていない。
3. **「開発ホストでは一見動く」理由も判明。**
   このホスト(NixOS + gnome-session)では `GIO_EXTRA_MODULES` に glib-networking が
   **ホスト環境から継承**されている。flake の devShell には `shellHook` が無く TLS を
   自前保証していないため、報告者環境(**Hyprland / GNOME セッション無し**)では継承されず、
   `./result` 実行でも `nix develop` 内 `cargo run` でも失敗する。

→ root cause は issue の指摘どおり。`command.rs` / `webview.rs` は変更しない。

## 3. 修正内容

### 3.1 `nix/rust.nix` — ビルド成果物に TLS バックエンドを同梱

- 関数の引数リストに `glib-networking` を追加。
- `buildInputs` に `glib-networking` を追加。
  → `wrapGAppsHook4` が `lib/gio/modules` を検出し、ラッパーの `GIO_EXTRA_MODULES` に
     glib-networking を prefix するようになる。`./result` のクロージャにも入る。

### 3.2 `flake.nix`(devShell)— ホスト環境に依存せず TLS を効かせる

- `devShells.default` の `packages` に `pkgs.glib-networking` を追加。
- devShell に `shellHook` を新規追加し、**既存値へ追記**する形で GIO モジュールを通す
  (dconf/gvfs 等の既存 `GIO_EXTRA_MODULES` を壊さない):

  ```nix
  shellHook = ''
    export GIO_EXTRA_MODULES="${pkgs.glib-networking}/lib/gio/modules''${GIO_EXTRA_MODULES:+:$GIO_EXTRA_MODULES}"
  '';
  ```

  (現状 devShell に `shellHook` は無いため新規追加。既存 `packages` はタブ混在の
   インデントなので周囲に合わせる。)

### 3.3 `docs/design.md §15` — 権威ドキュメントへ依存を反映

- CLAUDE.md ワークフロー「実装と design.md の食い違いは同一 PR で更新」に従う。
- §15(ビルド(Nix))の依存記述に、`glib-networking`
  (GIO TLS バックエンド、HTTPS 接続に必須)を `webkitgtk_6_0`・`gtk4`・`pkg-config` と
  並べて追記する。

## 4. 対象外 / 注意

- `src/command.rs` / `src/webview.rs` などコードは変更しない(純粋ロジックのバグではない)。
- `just test` / `just coverage` / `just mutants` の対象(純粋ロジック)には影響しない。

## 5. 検証(end-to-end)

1. `nix develop` に再入場し、`echo $GIO_EXTRA_MODULES` に glib-networking のパスが
   含まれることを確認(shellHook が効いているか)。
2. `nix develop --command just ci`(fmt-check → lint → coverage → mutants → build)が緑。
3. `nix build` 後、`nix path-info -r ./result | grep glib-networking` がヒットすること、
   および `./result/bin/owl` のラッパーが `GIO_EXTRA_MODULES` に glib-networking を
   prefix していることを確認。
4. 実機起動して `:open rust lang` で DuckDuckGo 検索結果が **エラー無く表示**されること、
   任意の `https://` URL(例 `:open https://example.com`)が開けることを確認。

## 6. ブランチ / PR フロー

- `main` から `fix/glib-networking-tls` を切って作業(CLAUDE.md: 最初の編集前にブランチ)。
- コミット/push/PR 作成はユーザー指示で実施。
- PR 作成後は `/review` をサブエージェント(`model: fable` = Claude Fable 5)で回し、
  結果を要約提示する(マージはしない)。
