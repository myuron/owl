//! ウィンドウ構築(設計書 §5)。
//!
//! `gtk::ApplicationWindow` 直下に縦 `gtk::Box` を置き、WebView(§5-1)を `vexpand`
//! で表示領域全体に広げる(todo 3-5)。ステータスバー(§5-2)・コマンドライン(§5-3)は
//! M1 では載せない(本実装はステータスバー = M2、コマンドライン = M4)。WebView の
//! 生成・設定(NetworkSession・`load_uri` 等 §8)は webview.rs に委譲する(§4)。

use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, Orientation};

use crate::webview;

/// `activate` から呼ばれ、ウィンドウを構築して present する(設計書 §13-2)。
///
/// §5: `ApplicationWindow` 直下に縦 `gtk::Box` を置き、WebView を `vexpand = true`
/// で全面に広げる(§5-1)。ツールバー・メニューバーは持たない(要求 5 章)。M1 では
/// ステータスバー・コマンドラインを載せず WebView のみを配置する(§5-2・§5-3)。
/// 初期 URI(`uri`)の読み込みは webview.rs の `load_uri`(§8)が担う。
pub fn build(app: &Application, uri: &str) {
    // §4: WebView の生成・設定は webview.rs に委譲する。
    let web_view = webview::build(uri);
    // §5-1: WebView を縦方向に伸ばし、表示領域全体を占める。
    web_view.set_vexpand(true);

    // §5: ApplicationWindow 直下の縦 Box。M1 では WebView 1 要素のみ。
    // ステータスバー(§5-2)・コマンドライン(§5-3)は後続 M でこの Box に append する。
    let layout = gtk4::Box::new(Orientation::Vertical, 0);
    layout.append(&web_view);

    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(1024)
        .default_height(768)
        .child(&layout)
        .build();

    window.present();
}
