//! エントリポイント: `gtk::Application` 生成と起動フロー(設計書 §4・§13)。
//!
//! GTK 結線コードはユニットテスト対象外(§14)。検証は `nix build` の通過と
//! 手動確認(`docs/checklist.md` M1)で行う。起動引数 → 初期 URL の決定は
//! GTK 非依存の純粋ロジック `command::initial_uri`(§13-3)へ切り出し、単体
//! テスト済み。純粋ロジックの `keys` は M3 で結線されるまで据え置く。

mod command;
mod keys;
mod webview;
mod window;

use gtk4::prelude::*;
use gtk4::{Application, gio, glib};

/// アプリケーション ID(設計書 §13-1)。
const APP_ID: &str = "dev.myuron.owl";

fn main() -> glib::ExitCode {
    // §13-3: 第 1 引数があればその URL、なければ about:blank を初期 URL にする。
    // 判定は純粋関数 `command::initial_uri` に委譲(補完規則 §11 の適用は M4)。
    // 引数 URL 起動は §16 では本来 M2 だが、todo サイクル 3 冒頭のとおり M1 へ前倒し。
    let arg = std::env::args().nth(1);
    let initial_uri = command::initial_uri(arg.as_deref()).to_string();

    // §13-1: NON_UNIQUE により `owl <url>` の再実行が常に新プロセス・新ウィンドウになる
    // (単一ウィンドウモデルを保つ最も単純な方法)。
    let app = Application::new(Some(APP_ID), gio::ApplicationFlags::NON_UNIQUE);

    // §13-2: activate でウィンドウを構築する(レイアウト・WebView は window/webview へ委譲)。
    app.connect_activate(move |app| window::build(app, &initial_uri));

    // 初期 URL は上で `std::env::args` から取得済み。GApplication に argv を渡すと
    // URL 引数を不明オプションとして扱い起動に失敗するため、空の引数で run する。
    app.run_with_args::<&str>(&[])
}
