//! ウィンドウ構築(設計書 §5)。
//!
//! `gtk::ApplicationWindow` 直下に縦 `gtk::Box` を置き、WebView(§5-1)を `vexpand`
//! で表示領域に広げ、その下にステータスバー(§5-2・§12)を 1 行で配置する。
//! ステータスバーは WebView のプロパティ通知(`notify::uri`/`title`/`is-loading`/
//! `estimated-load-progress`)にバインドし、ポーリングはしない(§12)。表示文字列の
//! 組み立てのうち純粋ロジック(読み込み状態 → `[NN%]`)は `command::format_load_progress`
//! に委譲する(§4 の純粋ロジック分離)。コマンドライン(§5-3)は M4。WebView の生成・
//! 設定(NetworkSession・`load_uri` 等 §8)は webview.rs に委譲する(§4)。

use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{Align, Application, ApplicationWindow, CssProvider, Label, Orientation};
use webkit6::WebView;
use webkit6::prelude::*;

use crate::command;
use crate::webview;

/// ステータスバーの最小 CSS(設計書 §5: 配色・等幅フォントのみをハードコード)。
const STATUS_BAR_CSS: &str = ".owl-statusbar {
    font-family: monospace;
    padding: 2px 6px;
    background-color: #1e1e1e;
    color: #d4d4d4;
}";

/// `activate` から呼ばれ、ウィンドウを構築して present する(設計書 §13-2)。
///
/// §5: `ApplicationWindow` 直下に縦 `gtk::Box` を置き、WebView を `vexpand = true` で
/// 広げ(§5-1)、直下にステータスバー(§5-2)を append する。ツールバー・メニューバーは
/// 持たない(要求 5 章)。コマンドライン(§5-3)は M4。初期 URI(`uri`)の読み込みは
/// webview.rs の `load_uri`(§8)が担う。
pub fn build(app: &Application, uri: &str) {
    // §4: WebView の生成・設定は webview.rs に委譲する。
    let web_view = webview::build(uri);
    // §5-1: WebView を縦方向に伸ばし、表示領域を占める。
    web_view.set_vexpand(true);

    // §5-2・§12: ステータスバーを組み立て、WebView の notify に結線する。
    let status_bar = build_status_bar(&web_view);

    // §5: ApplicationWindow 直下の縦 Box。上に WebView、下にステータスバー 1 行。
    let layout = gtk4::Box::new(Orientation::Vertical, 0);
    layout.append(&web_view);
    layout.append(&status_bar);

    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(1024)
        .default_height(768)
        .child(&layout)
        .build();

    window.present();
}

/// ステータスバー(高さ 1 行の横 `gtk::Box`)を組み立てて返す(設計書 §5-2・§12)。
///
/// 左から: モードインジケータ・URL・タイトル、右端に読み込み状態(§5-2)。各ラベルの
/// 初期値は WebView の現在のプロパティから設定し、以後は notify シグナルで更新する
/// (§12: プロパティ通知にバインド、ポーリングしない)。
fn build_status_bar(web_view: &WebView) -> gtk4::Box {
    install_css();

    let bar = gtk4::Box::new(Orientation::Horizontal, 8);
    bar.add_css_class("owl-statusbar");

    // モードインジケータ(§5-2: `-- INSERT --` 相当、normal 時は空)。M2 ではモード遷移が
    // 無いため常に空。内容更新(set_mode → ラベル)は M3 で結線する。ラベル枠のみ用意する。
    let mode = Label::new(None);

    // URL(§12: notify::uri)。余白を占め、長い URL は末尾省略する。
    let url = Label::new(web_view.uri().as_deref());
    url.set_ellipsize(EllipsizeMode::End);
    url.set_halign(Align::Start);
    url.set_xalign(0.0);
    url.set_hexpand(true);

    // タイトル(§12: notify::title)。長いタイトルは末尾省略する。
    let title = Label::new(web_view.title().as_deref());
    title.set_ellipsize(EllipsizeMode::End);
    title.set_max_width_chars(40);

    // 読み込み状態(§12: notify::is-loading + estimated-load-progress)。右端に置く。
    let progress = Label::new(Some(&command::format_load_progress(
        web_view.is_loading(),
        web_view.estimated_load_progress(),
    )));
    progress.set_halign(Align::End);

    bar.append(&mode);
    bar.append(&url);
    bar.append(&title);
    bar.append(&progress);

    // §12・§3.3: notify に結線する。各クロージャへは更新対象のラベルのみ clone し(神
    // オブジェクト化を避ける)、プロパティ値はシグナル引数の WebView から読む。
    let url_label = url.clone();
    web_view.connect_uri_notify(move |wv| {
        url_label.set_text(wv.uri().as_deref().unwrap_or_default());
    });

    let title_label = title.clone();
    web_view.connect_title_notify(move |wv| {
        title_label.set_text(wv.title().as_deref().unwrap_or_default());
    });

    // 読み込み状態は is-loading・estimated-load-progress の両方で更新する。表示文字列の
    // 組み立ては純粋関数 `command::format_load_progress`(§12)へ委譲する。
    let progress_label = progress.clone();
    web_view.connect_estimated_load_progress_notify(move |wv| {
        progress_label.set_text(&command::format_load_progress(
            wv.is_loading(),
            wv.estimated_load_progress(),
        ));
    });
    let progress_label = progress.clone();
    web_view.connect_is_loading_notify(move |wv| {
        progress_label.set_text(&command::format_load_progress(
            wv.is_loading(),
            wv.estimated_load_progress(),
        ));
    });

    bar
}

/// ステータスバーの CSS をディスプレイへ適用する(設計書 §5)。
///
/// `NON_UNIQUE`(§13-1)によりプロセス毎に 1 ウィンドウのため、本関数は起動毎に 1 度だけ
/// 呼ばれる。配色・等幅フォントの最小限のみをハードコードする(§5)。
fn install_css() {
    let provider = CssProvider::new();
    provider.load_from_string(STATUS_BAR_CSS);
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
