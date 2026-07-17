//! ウィンドウ構築(設計書 §5)。
//!
//! レイアウト本実装 — 縦 `gtk::Box` への WebView(§5-1)・ステータスバー(§5-2)・
//! コマンドライン(§5-3)の組み立て — は todo 3-5、WebView 生成は 3-6。ここでは
//! `main.rs`(3-4)の起動フローを結線するための最小の殻に留める(M1 は「表示される
//! だけの殻」に徹する。todo サイクル 3 冒頭)。
#![allow(dead_code)]

use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow};

/// `activate` から呼ばれ、ウィンドウを構築して present する(設計書 §13-2)。
///
/// 3-4 時点では空の `ApplicationWindow` を表示する骨組み。初期 URI(`uri`)は
/// 3-6 で生成する WebView へ `load_uri` する(§5-1・§8)。ツールバー・メニューバーは
/// 持たない(要求 5 章)。
pub fn build(app: &Application, uri: &str) {
    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(1024)
        .default_height(768)
        .build();

    // TODO(3-5): 縦 `gtk::Box` に WebView(`vexpand`, §5-1)・ステータスバー・
    //            コマンドラインを組む。
    // TODO(3-6): `webview::build(uri)` を生成して縦 Box に append し `load_uri` する。
    let _ = uri;

    window.present();
}
