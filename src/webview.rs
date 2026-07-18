//! WebView 生成と設定(設計書 §8): `NetworkSession`・永続化・各種シグナル
//! (TLS/クラッシュ/ポップアップ/ダウンロード)。
//!
//! 本実装は todo 3-6 の最小結線 — 永続 `NetworkSession`(§8.2)・`Settings` の
//! `enable_developer_extras`(§8.7)・初期 URI の `load_uri`。Cookie 永続化
//! (`set_persistent_storage`)と §8.3〜8.6 のシグナル結線は M7。data/cache の
//! パス算出は GTK 非依存の純粋関数 `command::app_subdir`(§8.2)に委譲する。
//!
//! M5(§9・§10): `UserContentManager` に page.js を document-start・全フレームで注入し、
//! JS → Rust の script message handler `"owl"` を登録する。受信 callback の結線と JS 呼び出し
//! (`owlHints.*`)の駆動は `input`(§4 の副作用側)が担う。

use gtk4::glib;
use webkit6::prelude::*;
use webkit6::{
    NetworkSession, Settings, UserContentInjectedFrames, UserContentManager, UserScript,
    UserScriptInjectionTime, WebView,
};

use crate::command;

/// ページへ常駐注入する JS(設計書 §9・§10)。ビルド時に埋め込む。
const PAGE_JS: &str = include_str!("page.js");

/// JS → Rust の script message handler 名(設計書 §9.2)。
pub const HINT_MESSAGE_HANDLER: &str = "owl";

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

    web_view.load_uri(uri);

    web_view
}
