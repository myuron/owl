fmt:
  cargo fmt

fmt-check:
  cargo fmt --check

lint:
  cargo clippy -- -D warnings

test:
  cargo test

# カバレッジゲート: 純粋ロジック(GTK 非依存)の region/line を 100% に保つ。
# 未テストの match アーム等を機械的に検出する(CLAUDE.md「相異なるアームは残らずテスト」)。
# GTK 結線ファイル(main.rs / window.rs / webview.rs)はユニットテスト対象外(design §14)
# なので 100% ゲートから除外する。正規表現は末尾 `$` とパス区切り `(^|/)src/` でアンカーし、
# 将来の純粋ロジック(例: domain.rs)を部分一致で巻き込まないようにする(todo 3-2)。
coverage:
  cargo llvm-cov \
    --ignore-filename-regex '(^|/)src/(main|window|webview)\.rs$' \
    --fail-under-regions 100 \
    --fail-under-lines 100

build:
  nix build

ci: fmt-check lint coverage build