//! WebView 生成と設定(設計書 §8): `NetworkSession`・永続化・各種シグナル
//! (TLS/クラッシュ/ポップアップ/ダウンロード)。
//!
//! 本実装は todo 3-6(NetworkSession 最小結線・`enable_developer_extras`・
//! `load_uri`)。§8.2〜8.6 のシグナル結線と Cookie 永続化は M7。ここでは
//! `main.rs`(3-4)が `mod webview;` を結線できるよう、§4 のモジュール構成に沿った
//! プレースホルダとして用意する。
#![allow(dead_code)]
