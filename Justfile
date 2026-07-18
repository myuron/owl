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
# GTK 結線ファイル(main.rs / window.rs / webview.rs / input.rs)はユニットテスト対象外
# (design §14)なので 100% ゲートから除外する。正規表現は末尾 `$` とパス区切り `(^|/)src/` で
# アンカーし、将来の純粋ロジック(例: domain.rs)を部分一致で巻き込まないようにする(todo 3-2)。
coverage:
  cargo llvm-cov \
    --ignore-filename-regex '(^|/)src/(main|window|webview|input)\.rs$' \
    --fail-under-regions 100 \
    --fail-under-lines 100

# ミューテーションテスト: 純粋モジュール限定で「分岐内の挙動」の未検証を機械検出する
# (CLAUDE.md 規約 4)。coverage は region の通過だけを見るため、丸めを truncate に
# 変えても 100% のままだが、cargo-mutants はその mutant が生き残る(= テスト不足)ことを
# 検出する。GTK 依存の結線ファイルは対象外(純粋ロジックのみを対象にする)。
mutants:
  cargo mutants --no-shuffle -f src/command.rs -f src/keys.rs

build:
  nix build

ci: fmt-check lint coverage mutants build