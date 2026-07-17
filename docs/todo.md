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

## 完了条件

- [x] test.md の全 ID(P-01〜P-42、K-01〜K-41)に対応するテストが存在し、すべて green
- [x] `cargo test` が GTK なしで完結する(テスト対象が gtk/webkit クレートに依存していない)
- [x] `cargo clippy` 警告なし、`cargo fmt --check` 通過
