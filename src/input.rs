//! キー入力の GTK 結線(設計書 §7.1・§7.2・§6)。
//!
//! `gtk::EventControllerKey` を **ウィンドウに capture phase で** 取り付け、WebView より先に
//! キーを横取りする(§7.1)。GTK のキーイベントを純粋関数 `keys::classify_input` で `KeyInput`
//! へ落とし、`keys::resolve_key` で「アクション + 伝播 + 次の pending」を求め、アクションを
//! 実行(スクロールは注入 JS §8.1、ナビゲーションは WebView API 直叩き §7.4、URL コピーは
//! クリップボード、モード遷移は §6)する。純粋な判定は `keys.rs`、副作用はここ(§4 の分離)。
//!
//! 中心状態(`mode`・`pending_key`、§3.3)は `Rc<Cell<AppState>>` で共有する。§3.3 は一般に
//! `Rc<RefCell<..>>` を挙げるが、`AppState` は `Copy` なので `Cell` に倒す(借用を跨がず
//! 構造的にパニックしない)。GTK 依存のため `just coverage` の対象外(Justfile の除外に追加)。

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{ApplicationWindow, EventControllerKey, Label, PropagationPhase, gdk, gio, glib};
use webkit6::WebView;
use webkit6::prelude::*;

use crate::keys::{self, Action, Mode};

/// アプリの中心状態(設計書 §3.3)。`Copy` なので `Rc<Cell<..>>` で各ハンドラへ配れる。
#[derive(Debug, Clone, Copy)]
struct AppState {
    /// 現在のモード。
    mode: Mode,
    /// `g`・`y` 等のシーケンス途中の先行キー。
    pending_key: Option<char>,
}

impl AppState {
    fn initial() -> Self {
        Self {
            mode: Mode::Normal,
            pending_key: None,
        }
    }
}

/// ウィンドウにキーコントローラを取り付ける(設計書 §7.1)。
///
/// capture phase で登録することで、フォーカスが WebView にあってもウィンドウが先にキーを
/// 受け取れる。`mode_label` はモードインジケータ(§5-2・§12)で、モード遷移時に更新する。
pub fn install(window: &ApplicationWindow, web_view: &WebView, mode_label: &Label) {
    let state = Rc::new(Cell::new(AppState::initial()));

    let controller = EventControllerKey::new();
    // §7.1: capture(親→子)で WebView より先にキーを見る。
    controller.set_propagation_phase(PropagationPhase::Capture);

    let web_view = web_view.clone();
    let mode_label = mode_label.clone();
    controller.connect_key_pressed(move |_controller, keyval, _keycode, mods| {
        // §7.1・§7.2: GTK の keyval・修飾状態を純粋な入力種別へ分類する。SHIFT は keyval 側で
        // 文字へ畳み込み済みのため分類に使わない(`classify_input`)。
        let escape = keyval == gdk::Key::Escape;
        let ctrl = mods.contains(gdk::ModifierType::CONTROL_MASK);
        let other_mod = mods.intersects(
            gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SUPER_MASK
                | gdk::ModifierType::META_MASK,
        );
        let input = keys::classify_input(escape, ctrl, other_mod, keyval.to_unicode());

        let current = state.get();
        let (outcome, pending) = keys::resolve_key(current.pending_key, current.mode, input);

        let new_mode = match outcome.action {
            Some(action) => dispatch(action, &web_view, &mode_label, current.mode),
            None => current.mode,
        };
        state.set(AppState {
            mode: new_mode,
            pending_key: pending,
        });

        to_glib(outcome.propagation)
    });

    window.add_controller(controller);
}

/// 解決されたアクションを実行し、遷移後のモードを返す(設計書 §7.4・§8.1・§6)。
fn dispatch(action: Action, web_view: &WebView, mode_label: &Label, current: Mode) -> Mode {
    match action {
        // スクロールは注入 JS(§8.1)。文字列生成は純粋関数 `scroll_script`(§7.4)へ委譲。
        Action::ScrollLeft
        | Action::ScrollRight
        | Action::ScrollUp
        | Action::ScrollDown
        | Action::ScrollTop
        | Action::ScrollBottom
        | Action::ScrollHalfDown
        | Action::ScrollHalfUp => {
            if let Some(js) = keys::scroll_script(action) {
                eval_js(web_view, js);
            }
        }
        // ナビゲーションは WebView API 直叩き(§7.4)。
        Action::Back => web_view.go_back(),
        Action::Forward => web_view.go_forward(),
        Action::Reload => web_view.reload(),
        Action::StopLoading => web_view.stop_loading(),
        // §7.4: 現在ページの URL をクリップボードへ(widget 版で Display の Option を回避)。
        Action::CopyUrl => {
            if let Some(uri) = web_view.uri() {
                web_view.clipboard().set_text(uri.as_str());
            }
        }
        Action::EnterMode(target) => {
            return apply_enter_mode(target, web_view, mode_label, current);
        }
    }
    current
}

/// モード遷移の副作用を適用し、遷移後のモードを返す(設計書 §6)。
fn apply_enter_mode(target: Mode, web_view: &WebView, mode_label: &Label, current: Mode) -> Mode {
    match target {
        Mode::Insert => {
            mode_label.set_text(keys::mode_indicator(Mode::Insert));
            Mode::Insert
        }
        Mode::Normal => {
            // §6: Insert → Normal はページ側の focus を外し、GTK フォーカスを WebView 本体へ戻す。
            eval_js(
                web_view,
                "document.activeElement && document.activeElement.blur()",
            );
            web_view.grab_focus();
            mode_label.set_text(keys::mode_indicator(Mode::Normal));
            Mode::Normal
        }
        // §16.3: Command/Hint は M4/M5 で本結線。ここで遷移させると Command は Esc も Proceed
        // する(§7.2)ため Entry 未結線の M3 では復帰不能になる。M3 では inert に倒し、現在の
        // モード(Normal)に留まる(pending は resolve_key が既にクリア済み)。
        Mode::Command | Mode::Hint => current,
    }
}

/// 注入 JS を fire-and-forget で評価する(設計書 §8.1)。結果・エラーは扱わない(スクロールや
/// blur は失敗しても実害がない)。GTK メインスレッドのシグナルハンドラから呼ぶ前提。
fn eval_js(web_view: &WebView, script: &str) {
    web_view.evaluate_javascript(script, None, None, gio::Cancellable::NONE, |_result| {});
}

/// 純粋ロジックの伝播を GTK の `glib::Propagation` へ変換する(設計書 §7.1)。
fn to_glib(propagation: keys::Propagation) -> glib::Propagation {
    match propagation {
        keys::Propagation::Stop => glib::Propagation::Stop,
        keys::Propagation::Proceed => glib::Propagation::Proceed,
    }
}
