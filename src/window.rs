//! ウィンドウ構築(設計書 §5)。
//!
//! `gtk::ApplicationWindow` 直下に縦 `gtk::Box` を置き、WebView(§5-1)を `vexpand`
//! で表示領域に広げ、その下にステータスバー(§5-2・§12)を 1 行、さらに下にコマンドライン
//! (§5-3、通常は非表示)を配置する。ステータスバーは WebView のプロパティ通知
//! (`notify::uri`/`title`/`is-loading`/`estimated-load-progress`)にバインドし、
//! ポーリングはしない(§12)。表示文字列の組み立てのうち純粋ロジック(読み込み状態 →
//! `[NN%]`)は `command::format_load_progress` に委譲する(§4 の純粋ロジック分離)。
//! コマンドライン Entry の結線(表示・実行・キャンセル、§11)は `input`(M4)が担う。
//! WebView の生成・設定(NetworkSession・`load_uri` 等 §8)は webview.rs に委譲する(§4)。

use std::sync::Once;

use gtk4::pango::EllipsizeMode;
use gtk4::prelude::*;
use gtk4::{Align, Application, ApplicationWindow, CssProvider, Entry, Label, Orientation};
use webkit6::WebView;
use webkit6::prelude::*;

use crate::command;
use crate::input;
use crate::webview;

/// ステータスバー・コマンドラインの最小 CSS(設計書 §5: 配色・等幅フォントのみをハードコード)。
/// `.owl-message` はコマンドのエラー表示(§11)で、通常テキストと区別できる警告色にする。
const STATUS_BAR_CSS: &str = ".owl-statusbar, .owl-commandline {
    font-family: monospace;
    padding: 2px 6px;
    background-color: #1e1e1e;
    color: #d4d4d4;
}
.owl-message {
    color: #e5a03c;
}";

/// `activate` から呼ばれ、ウィンドウを構築して present する(設計書 §13-2)。
///
/// §5: `ApplicationWindow` 直下に縦 `gtk::Box` を置き、WebView を `vexpand = true` で
/// 広げ(§5-1)、直下にステータスバー(§5-2)、さらに下にコマンドライン(§5-3、通常非表示)を
/// append する。ツールバー・メニューバーは持たない(要求 5 章)。初期 URI(`uri`)の読み込みは
/// webview.rs の `load_uri`(§8)が担う。
pub fn build(app: &Application, uri: &str) {
    // §4: WebView の生成・設定は webview.rs に委譲する。
    let web_view = webview::build(uri);
    // §5-1: WebView を縦方向に伸ばし、表示領域を占める。
    web_view.set_vexpand(true);

    // §5-2・§12: ステータスバーを組み立て、WebView の notify に結線する。モードインジケータと
    // メッセージ(エラー)のラベルは M4 のキー結線(input)から更新するため受け取る。
    let (status_bar, mode_label, message_label) = build_status_bar(&web_view);

    // §5-3: コマンドライン。command モードでのみ表示する(初期は非表示)。表示・実行・
    // キャンセルの結線は input(§11)が担う。
    let command_entry = build_command_line();

    // §5: ApplicationWindow 直下の縦 Box。上に WebView、下にステータスバー、コマンドライン。
    let layout = gtk4::Box::new(Orientation::Vertical, 0);
    layout.append(&web_view);
    layout.append(&status_bar);
    layout.append(&command_entry);

    let window = ApplicationWindow::builder()
        .application(app)
        .default_width(1024)
        .default_height(768)
        .child(&layout)
        .build();

    // §7.1・§11: EventControllerKey をウィンドウに capture phase で取り付け、コマンドライン
    // Entry の実行/キャンセルも結線する(M4)。モード遷移・エラー表示のため各ラベル/Entry を渡す。
    input::install(
        &window,
        &web_view,
        &mode_label,
        &command_entry,
        &message_label,
    );

    window.present();
}

/// コマンドライン Entry を組み立てる(設計書 §5-3)。command モードでのみ表示するため
/// 初期は非表示。表示・初期値 `:`・フォーカス・実行(Enter)・キャンセル(Esc)の結線は
/// `input`(§11)が担う。
fn build_command_line() -> Entry {
    let entry = Entry::new();
    entry.add_css_class("owl-commandline");
    entry.set_visible(false);
    entry
}

/// ステータスバー(高さ 1 行の横 `gtk::Box`)を組み立て、バー・モードインジケータ・
/// メッセージ(エラー)のラベルを返す(設計書 §5-2・§12)。
///
/// 左から: モードインジケータ・メッセージ・URL・タイトル、右端に読み込み状態(§5-2)。URL/
/// タイトル/読み込み状態は WebView の notify シグナルで更新する(§12: プロパティ通知に
/// バインド、ポーリングしない)。モードインジケータとメッセージは WebView のプロパティでは
/// なくキー入力(モード遷移・コマンド実行)で変わるため、そのラベルを呼び出し側(M4 の
/// `input`)へ返して結線させる。メッセージ欄はコマンドのエラー表示(§11)に使う(将来
/// §8.5 の「download blocked」表示もここへ集約する余地を残す)。
fn build_status_bar(web_view: &WebView) -> (gtk4::Box, Label, Label) {
    install_css();

    let bar = gtk4::Box::new(Orientation::Horizontal, 8);
    bar.add_css_class("owl-statusbar");

    // モードインジケータ(§5-2: `-- INSERT --` 相当、normal 時は空)。初期は Normal で空。
    // 内容更新は M3/M4 のキー結線(`input::install` → `keys::mode_indicator`)が担う(§12)。
    let mode = Label::new(None);

    // メッセージ(§11: 未知コマンド/`:open` 空引数のエラー)。初期は空。内容更新は M4 の
    // キー結線(`input` の command 実行)が担う。警告色は `.owl-message` で付ける。
    let message = Label::new(None);
    message.add_css_class("owl-message");
    message.set_halign(Align::Start);
    // 長大な未知コマンド名でもラベルの自然幅がウィンドウ最小幅を押し広げないよう省略する
    // (URL/タイトルと同じ扱い)。
    message.set_ellipsize(EllipsizeMode::End);
    message.set_max_width_chars(40);

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
    // 初期値・以降の更新とも `update_progress` に一本化する。
    let progress = Label::new(None);
    progress.set_halign(Align::End);
    update_progress(&progress, web_view);

    bar.append(&mode);
    bar.append(&message);
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

    // 読み込み状態は is-loading・estimated-load-progress の両方で更新する(§12)。両ハンドラは
    // 同一処理のため `update_progress` に括り出す(重複排除)。片方だけだと完了時に `[100%]`
    // が残る/更新が飛ぶため、両方の notify に結線する必要がある。
    let progress_label = progress.clone();
    web_view.connect_estimated_load_progress_notify(move |wv| update_progress(&progress_label, wv));
    let progress_label = progress.clone();
    web_view.connect_is_loading_notify(move |wv| update_progress(&progress_label, wv));

    (bar, mode, message)
}

/// 読み込み状態ラベルを現在の WebView プロパティで更新する(設計書 §12)。
///
/// 初期化と is-loading / estimated-load-progress の両 notify から共用する。表示文字列の
/// 組み立ては純粋関数 `command::format_load_progress`(§12)へ委譲する。
fn update_progress(label: &Label, web_view: &WebView) {
    label.set_text(&command::format_load_progress(
        web_view.is_loading(),
        web_view.estimated_load_progress(),
    ));
}

/// ステータスバーの CSS をディスプレイへ適用する(設計書 §5)。配色・等幅フォントの
/// 最小限のみをハードコードする。
///
/// per-window の `build_status_bar` から呼ばれるが、Display への provider 追加は 1 回で
/// 足りる。「1 回」を呼び出し側の前提(`NON_UNIQUE` §13-1)に依存させず、`Once` で構造的に
/// 担保する(CLAUDE.md 規約 3: 時間的不変条件はコメントでなく型/構造で強制する)。
fn install_css() {
    static CSS_ONCE: Once = Once::new();
    CSS_ONCE.call_once(|| {
        let provider = CssProvider::new();
        provider.load_from_string(STATUS_BAR_CSS);
        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}
