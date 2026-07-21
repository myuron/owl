//! WebView 生成と設定(設計書 §8): `NetworkSession`・永続化・各種シグナル
//! (TLS/クラッシュ/ポップアップ/ダウンロード)。
//!
//! 本実装は永続 `NetworkSession`(§8.2)・`Settings` の `enable_developer_extras`(§8.7)・
//! 初期 URI の `load_uri` に加え、M7 の堅牢化 — Cookie 永続化(§8.2)・TLS Fail(§8.3)・
//! 新規ウィンドウ抑制(§8.4)・ダウンロードキャンセル(§8.5)・エラーページ/クラッシュ復帰
//! (§8.6)の各シグナル結線を担う。data/cache のパス算出・エラーページ HTML・ダウンロード
//! ブロック表示の組み立ては GTK 非依存の純粋関数(`command`)へ委譲する(§4)。
//!
//! M5(§9・§10): `UserContentManager` に page.js を document-start・全フレームで注入し、
//! JS → Rust の script message handler `"owl"` を登録する。受信 callback の結線と JS 呼び出し
//! (`owlHints.*`)の駆動は `input`(§4 の副作用側)が担う。

use gtk4::{Label, glib};
use webkit6::prelude::*;
use webkit6::{
    CookiePersistentStorage, NetworkError, NetworkSession, Settings, TLSErrorsPolicy,
    UserContentInjectedFrames, UserContentManager, UserScript, UserScriptInjectionTime, WebView,
};

use crate::command;

/// ページへ常駐注入する JS(設計書 §9・§10)。ビルド時に埋め込む。
const PAGE_JS: &str = include_str!("page.js");

/// JS → Rust の script message handler 名(設計書 §9.2)。
pub const HINT_MESSAGE_HANDLER: &str = "owl";

/// Cookie 永続化 SQLite の格納ファイル名(設計書 §8.2)。data ディレクトリ配下に置く。
const COOKIE_STORE_FILE: &str = "cookies.sqlite";

/// ブロックしたダウンロードのメッセージを表示し続ける秒数(設計書 §8.5「数秒表示」)。
const DOWNLOAD_MESSAGE_SECS: u32 = 4;

/// 初期 URI を読み込んだ `WebView` を生成して返す(設計書 §5-1・§8)。
///
/// §8.2: `$XDG_DATA_HOME/owl`・`$XDG_CACHE_HOME/owl` を data/cache とする永続
/// `NetworkSession` を作り、WebView に紐付ける。パス算出は `command::app_subdir`
/// (単体テスト済み)、XDG ベースの取得は `glib`。§13-4 に従い、起動パスの同期 I/O は
/// この XDG ディレクトリ作成のみに限る。§8.7: 明示する設定は `enable_developer_extras`
/// のみで、他は WebKit 既定に任せる。`uri` は呼び出し側(`main`)で決定済みの生 URI
/// (補完規則 §11 の適用は M4)。
pub fn build(uri: &str) -> WebView {
    // §8.2: data/cache ディレクトリ。パス算出は純粋関数へ、XDG ベースは glib から。
    let data_dir = command::app_subdir(&glib::user_data_dir());
    let cache_dir = command::app_subdir(&glib::user_cache_dir());

    // §13-4: 起動パスで許される同期 I/O は XDG ディレクトリ作成のみ。作成失敗は
    // WebKit 側でも再試行されるため M1 では致命視しない(エラー通知は M7 の範囲)。
    let _ = std::fs::create_dir_all(&data_dir);
    let _ = std::fs::create_dir_all(&cache_dir);

    let network_session = NetworkSession::new(
        Some(&data_dir.to_string_lossy()),
        Some(&cache_dir.to_string_lossy()),
    );

    // §8.2: Cookie を SQLite で明示的に永続化する。cookie_manager は取得できないことは通常
    // ないが、取れなければ Cookie 非永続のまま起動を続ける(他機能に影響しない)。
    if let Some(cookie_manager) = network_session.cookie_manager() {
        let cookie_path = data_dir.join(COOKIE_STORE_FILE);
        cookie_manager.set_persistent_storage(
            &cookie_path.to_string_lossy(),
            CookiePersistentStorage::Sqlite,
        );
    }

    // §8.3: 証明書無効サイトは読み込み失敗にする(例外許可 UI は作らない。要求 3.1)。失敗は
    // `load-failed-with-tls-errors` で検知しエラーページ(§8.6)を出す。
    network_session.set_tls_errors_policy(TLSErrorsPolicy::Fail);

    // §8.7: 既定を基本とし、インスペクタ用に developer extras のみ明示する。
    let settings = Settings::new();
    settings.set_enable_developer_extras(true);

    // §9・§10: page.js を document-start・全フレームで常駐注入し、JS → Rust の
    // メッセージハンドラ "owl" を登録する。Rust 側の受信結線(callback)は input が担う(§4)。
    let content_manager = UserContentManager::new();
    let user_script = UserScript::new(
        PAGE_JS,
        UserContentInjectedFrames::AllFrames,
        UserScriptInjectionTime::Start,
        &[],
        &[],
    );
    content_manager.add_script(&user_script);
    content_manager.register_script_message_handler(HINT_MESSAGE_HANDLER, None);

    // §8.2: NetworkSession を紐付けて WebView を生成する。
    let web_view = WebView::builder()
        .network_session(&network_session)
        .user_content_manager(&content_manager)
        .settings(&settings)
        .build();

    // §8.4・§8.6: 初期ロードの失敗も拾えるよう、load_uri より前にシグナルを結線する。
    install_popup_suppression(&web_view);
    install_error_pages(&web_view);

    web_view.load_uri(uri);

    web_view
}

/// 新規ウィンドウ生成要求を現在の WebView での遷移へ倒す(設計書 §8.4)。
///
/// `target="_blank"` や `window.open` で発火する `create` を捕まえ、要求 URI を現在の WebView で
/// `load_uri` してから `None` を返す(新規ウィンドウを作らせない)。URI が取れない要求(空の
/// `window.open()` 等)は何もせず握り潰す(新規ウィンドウを開かせないことが目的)。
fn install_popup_suppression(web_view: &WebView) {
    web_view.connect_create(|wv, navigation_action| {
        if let Some(uri) = navigation_action.request().and_then(|r| r.uri()) {
            wv.load_uri(&uri);
        }
        None
    });
}

/// 読み込み失敗・TLS 失敗・WebProcess クラッシュを最小エラーページへ倒す(設計書 §8.6)。
///
/// いずれも `command::error_page_html` の HTML を `load_alternate_html` で失敗 URI をオリジンに
/// 表示する。エラーページからは `r`(§7.4 の `reload`)でそのまま復帰できる(§8.6)。エラー種別・
/// URL の HTML エスケープは純粋関数側で済んでいる(§4・規約 6)。
fn install_error_pages(web_view: &WebView) {
    // 読み込み失敗(§8.6)。ただし Esc 中断・ダウンロード化・リダイレクト等の**キャンセル**は
    // 失敗ではないためエラーページを出さず既定に委ねる(`false` を返す)。TLS 失敗はこの
    // シグナルではなく `load-failed-with-tls-errors` 側へ回るため、ここでは扱わない。
    web_view.connect_load_failed(|wv, _event, failing_uri, error| {
        if error.matches(NetworkError::Cancelled) {
            return false;
        }
        let html = command::error_page_html(&error.to_string(), failing_uri);
        wv.load_alternate_html(&html, failing_uri, None);
        true
    });

    // TLS 証明書エラー(§8.3・§8.6)。戻り値は `bool`(`true` = ハンドル済み。M1-31)。
    web_view.connect_load_failed_with_tls_errors(|wv, failing_uri, _certificate, _errors| {
        let html = command::error_page_html("TLS certificate error", failing_uri);
        wv.load_alternate_html(&html, failing_uri, None);
        true
    });

    // WebProcess クラッシュ(§8.6)。現在の URI をオリジンにエラーページを出し、`r` の
    // `reload()` で WebProcess を再 spawn できるようにする。URI 不明時は空にフォールバック。
    web_view.connect_web_process_terminated(|wv, _reason| {
        let uri = wv.uri().unwrap_or_default();
        let html = command::error_page_html("renderer crashed", &uri);
        wv.load_alternate_html(&html, &uri, None);
    });
}

/// ダウンロード開始を即時キャンセルし、ブロックをステータスバーへ数秒表示する(設計書 §8.5)。
///
/// NetworkSession は `web_view` に紐付け済み(`build`)。ステータスバーのメッセージ欄
/// (`message_label`、§5-2)はウィンドウ構築後にしか存在しないため、`window` から本関数で
/// 後付け結線する(§4: 副作用側の結線)。表示は `DOWNLOAD_MESSAGE_SECS` 秒後に自動で消すが、
/// その間に別のメッセージ(コマンドエラー §11 等)が入っていたら上書きしない(現在のテキストが
/// 自分のものであるときだけ消す)。ファイル名の抽出・文言組み立ては純粋関数へ委譲する(§4)。
pub fn install_download_guard(web_view: &WebView, message_label: &Label) {
    // network_session は builder で紐付け済み。取得できなければ DL ガードなしで続行する。
    let Some(network_session) = web_view.network_session() else {
        return;
    };

    let message_label = message_label.clone();
    network_session.connect_download_started(move |_session, download| {
        // §8.5: 即時キャンセルする。
        download.cancel();

        let uri = download.request().and_then(|r| r.uri());
        let message = command::download_blocked_message(uri.as_deref().unwrap_or_default());
        message_label.set_text(&message);

        // §8.5: 数秒だけ表示する。表示中に別メッセージが入っていたら上書きしない。
        let label = message_label.clone();
        glib::timeout_add_seconds_local(DOWNLOAD_MESSAGE_SECS, move || {
            if label.text() == message {
                label.set_text("");
            }
            glib::ControlFlow::Break
        });
    });
}
