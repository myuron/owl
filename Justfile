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
# GTK 結線用の main.rs はユニットテスト対象外なので除外する。
coverage:
  cargo llvm-cov \
    --ignore-filename-regex 'main\.rs' \
    --fail-under-regions 100 \
    --fail-under-lines 100

build:
  nix build

ci: fmt-check lint coverage build