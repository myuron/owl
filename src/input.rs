//! キー入力の GTK 結線(設計書 §7.1・§7.2・§6・§11)。
//!
//! `gtk::EventControllerKey` を **ウィンドウに capture phase で** 取り付け、WebView より先に
//! キーを横取りする(§7.1)。GTK のキーイベントを純粋関数 `keys::classify_input` で `KeyInput`
//! へ落とし、`keys::resolve_key` で「アクション + 伝播 + 次の pending」を求め、アクションを
//! 実行(スクロールは注入 JS §8.1、ナビゲーションは WebView API 直叩き §7.4、URL コピーは
//! クリップボード、モード遷移は §6)する。純粋な判定は `keys.rs`、副作用はここ(§4 の分離)。
//!
//! command モード(§11)もここで結線する: `:` で Entry を表示・`:` を初期値にしてフォーカスし、
//! Entry の `activate`(Enter)で `command::parse_command` を実行(`:open`/`:quit`/未知)、Entry 上の
//! `EventControllerKey`(Esc)でキャンセルする。コマンドの解釈は純粋関数 `command::parse_command`、
//! 実際の `load_uri`・終了・エラー表示はここが担う(§4 の分離)。
//!
//! 中心状態(`mode`・`pending_key`、§3.3)は `Rc<Cell<AppState>>` で共有する。§3.3 は一般に
//! `Rc<RefCell<..>>` を挙げるが、`AppState` は `Copy` なので `Cell` に倒す(借用を跨がず
//! 構造的にパニックしない)。GTK 依存のため `just coverage` の対象外(Justfile の除外に追加)。

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{ApplicationWindow, Entry, EventControllerKey, Label, PropagationPhase, gdk, gio, glib};
use webkit6::WebView;
use webkit6::prelude::*;

use crate::command::{self, Command};
use crate::hints::{self, HintMessage};
use crate::keys::{self, Action, Mode};
use crate::webview::HINT_MESSAGE_HANDLER;

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

/// ウィンドウにキーコントローラを取り付ける(設計書 §7.1・§11)。
///
/// capture phase で登録することで、フォーカスが WebView にあってもウィンドウが先にキーを
/// 受け取れる。`mode_label` はモードインジケータ(§5-2・§12)、`command_entry` はコマンドライン
/// (§5-3)、`message_label` はコマンドのエラー表示欄(§5-2・§11)。中心状態(`AppState`)を
/// ウィンドウの key コントローラ・Entry の `activate`/`Esc` の 3 ハンドラで共有する。
pub fn install(
    window: &ApplicationWindow,
    web_view: &WebView,
    mode_label: &Label,
    command_entry: &Entry,
    message_label: &Label,
) {
    let state = Rc::new(Cell::new(AppState::initial()));

    install_window_controller(
        window,
        web_view,
        mode_label,
        command_entry,
        message_label,
        &state,
    );
    install_command_entry(
        window,
        web_view,
        mode_label,
        command_entry,
        message_label,
        &state,
    );
    install_hint_message_handler(web_view, mode_label, &state);
}

/// ウィンドウの capture-phase キーコントローラ(§7.1)を取り付ける。
fn install_window_controller(
    window: &ApplicationWindow,
    web_view: &WebView,
    mode_label: &Label,
    command_entry: &Entry,
    message_label: &Label,
    state: &Rc<Cell<AppState>>,
) {
    let controller = EventControllerKey::new();
    // §7.1: capture(親→子)で WebView より先にキーを見る。
    controller.set_propagation_phase(PropagationPhase::Capture);

    let state = state.clone();
    let web_view = web_view.clone();
    let mode_label = mode_label.clone();
    let command_entry = command_entry.clone();
    let message_label = message_label.clone();
    controller.connect_key_pressed(move |_controller, keyval, _keycode, mods| {
        // §7.1・§7.2: GTK の keyval・修飾状態を純粋な入力種別へ分類する。SHIFT は keyval 側で
        // 文字へ畳み込み済みのため分類に使わない(`classify_input`)。
        let escape = keyval == gdk::Key::Escape;
        let ctrl = mods.contains(gdk::ModifierType::CONTROL_MASK);
        let other_mod = mods.intersects(
            gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SUPER_MASK
                | gdk::ModifierType::META_MASK
                | gdk::ModifierType::HYPER_MASK,
        );
        let input = keys::classify_input(escape, ctrl, other_mod, keyval.to_unicode());

        let current = state.get();
        let (outcome, pending) = keys::resolve_key(current.pending_key, current.mode, input);

        let new_mode = match outcome.action {
            Some(action) => dispatch(
                action,
                &web_view,
                &mode_label,
                &command_entry,
                &message_label,
                current.mode,
            ),
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

/// コマンドライン Entry の `activate`(Enter)と `Esc` を結線する(設計書 §11)。
fn install_command_entry(
    window: &ApplicationWindow,
    web_view: &WebView,
    mode_label: &Label,
    command_entry: &Entry,
    message_label: &Label,
    state: &Rc<Cell<AppState>>,
) {
    // Enter: コマンドを解釈・実行してから Normal へ戻す(§11)。
    {
        let state = state.clone();
        let window = window.clone();
        let web_view = web_view.clone();
        let mode_label = mode_label.clone();
        let message_label = message_label.clone();
        command_entry.connect_activate(move |entry| {
            match command::parse_command(entry.text().as_str()) {
                // §11: 補完済み URL をそのまま読み込む。
                Command::Open(url) => web_view.load_uri(&url),
                // §11: 終了(NON_UNIQUE の単一ウィンドウを閉じるとアプリが終わる。§13-1)。
                Command::Quit => {
                    window.close();
                    return;
                }
                // 空入力: 何もしない。
                Command::Noop => {}
                // 未知コマンド/空引数: ステータスバーにエラー表示(§11)。
                Command::Error(msg) => message_label.set_text(&msg),
            }
            leave_command(&state, entry, &web_view, &mode_label);
        });
    }

    // Esc: キャンセルして Normal へ戻す(§11: Entry 上の EventControllerKey で拾う)。
    // window の capture コントローラは Command モードでは Esc も Proceed する(§7.2)ため、
    // Esc はここまで届く。
    {
        let state = state.clone();
        let web_view = web_view.clone();
        let mode_label = mode_label.clone();
        let entry = command_entry.clone();
        let esc = EventControllerKey::new();
        esc.connect_key_pressed(move |_controller, keyval, _keycode, _mods| {
            if keyval != gdk::Key::Escape {
                return glib::Propagation::Proceed;
            }
            leave_command(&state, &entry, &web_view, &mode_label);
            glib::Propagation::Stop
        });
        command_entry.add_controller(esc);
    }
}

/// 解決されたアクションを実行し、遷移後のモードを返す(設計書 §7.4・§8.1・§6)。
fn dispatch(
    action: Action,
    web_view: &WebView,
    mode_label: &Label,
    command_entry: &Entry,
    message_label: &Label,
    current: Mode,
) -> Mode {
    // スクロールは注入 JS(§8.1)。対象 Action と JS 文字列の唯一の定義元は純粋関数
    // `scroll_script`(§7.4)。ここで先に引き当て、スクロール系はこの分岐で完結させる。
    if let Some(js) = keys::scroll_script(action) {
        eval_js(web_view, js);
        return current;
    }
    match action {
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
            return apply_enter_mode(
                target,
                web_view,
                mode_label,
                command_entry,
                message_label,
                current,
            );
        }
        // §9.2: Hint モードのラベル文字を `owlHints.input(ch)` へ転送する。絞り込み・確定と
        // その後のモード遷移は page.js → メッセージハンドラ(`install_hint_message_handler`)側で
        // 起きるため、ここではモードを変えず Hint に留まる。
        Action::HintInput(ch) => eval_js(web_view, &hints::input_script(ch)),
        // スクロール系は上の `scroll_script` で処理済み(到達しない)。網羅性のため明示する。
        Action::ScrollLeft
        | Action::ScrollRight
        | Action::ScrollUp
        | Action::ScrollDown
        | Action::ScrollTop
        | Action::ScrollBottom
        | Action::ScrollHalfDown
        | Action::ScrollHalfUp => {}
    }
    current
}

/// モード遷移の副作用を適用し、遷移後のモードを返す(設計書 §6・§11)。
fn apply_enter_mode(
    target: Mode,
    web_view: &WebView,
    mode_label: &Label,
    command_entry: &Entry,
    message_label: &Label,
    current: Mode,
) -> Mode {
    match target {
        Mode::Insert => {
            mode_label.set_text(keys::mode_indicator(Mode::Insert));
            Mode::Insert
        }
        Mode::Normal => {
            // §9.2: Hint → Normal(Esc キャンセル)はオーバーレイを除去する。確定経由の
            // Hint → Normal は page.js が除去済みでこの経路を通らない(メッセージハンドラ側)。
            if current == Mode::Hint {
                eval_js(web_view, hints::cancel_script());
            }
            // §6: Insert → Normal はページ側の focus を外し、GTK フォーカスを WebView 本体へ戻す。
            eval_js(
                web_view,
                "document.activeElement && document.activeElement.blur()",
            );
            web_view.grab_focus();
            mode_label.set_text(keys::mode_indicator(Mode::Normal));
            Mode::Normal
        }
        // §11: `:` で command モードへ。Entry を初期値 `:` で表示しフォーカスする。前回の
        // エラーメッセージはここでクリアする(新しい入力の開始)。実行/キャンセルは
        // `install_command_entry` の activate/Esc ハンドラが担う。
        Mode::Command => {
            message_label.set_text("");
            command_entry.set_text(":");
            command_entry.set_visible(true);
            // grab_focus は Entry のテキストを全選択してしまうため、選択しない版でフォーカスし、
            // カーソルを末尾(`:` の後ろ)に置く。
            command_entry.grab_focus_without_selecting();
            command_entry.set_position(-1);
            mode_label.set_text(keys::mode_indicator(Mode::Command));
            Mode::Command
        }
        // §9: `f` で Hint モードへ。`owlHints.start()` で要素列挙・ラベル表示を駆動する。
        // 以降のラベル文字は `Action::HintInput` として `owlHints.input(ch)` へ転送し、
        // 確定/全滅時のモード遷移はメッセージハンドラが行う。GTK フォーカスは WebView 本体の
        // ままにする(ラベル描画・click/focus は page.js が担うため移さない)。
        Mode::Hint => {
            eval_js(web_view, hints::start_script());
            mode_label.set_text(keys::mode_indicator(Mode::Hint));
            Mode::Hint
        }
    }
}

/// command モードを終了して Normal へ戻す(設計書 §11・§6)。
///
/// Entry を空にして隠し、モードインジケータを空へ、フォーカスを WebView 本体へ戻し、
/// 中心状態を Normal(pending クリア)へ書き戻す。activate(実行後)/Esc(キャンセル)の
/// 双方から共用する。
fn leave_command(
    state: &Rc<Cell<AppState>>,
    entry: &Entry,
    web_view: &WebView,
    mode_label: &Label,
) {
    entry.set_text("");
    entry.set_visible(false);
    mode_label.set_text(keys::mode_indicator(Mode::Normal));
    web_view.grab_focus();
    state.set(AppState {
        mode: Mode::Normal,
        pending_key: None,
    });
}

/// JS → Rust の hint 結果メッセージ(script message handler `"owl"`)を結線する(設計書 §9.2)。
///
/// page.js が確定/全滅時に送るメッセージを `hints::parse_hint_message` で解釈し、モードを
/// 遷移させる(`Link`/`None` → Normal、`Input` → Insert)。JS 側は既に click/focus と
/// オーバーレイ除去を済ませている(§9)ため、ここでは中心状態とモードインジケータの更新に
/// 留める(§6・§5-2)。ハンドラ名の登録・page.js 注入は `webview::build`(§4)が担う。
fn install_hint_message_handler(
    web_view: &WebView,
    mode_label: &Label,
    state: &Rc<Cell<AppState>>,
) {
    // UCM は `webview::build` が WebView に紐付け済み。取得できなければ hint は使えないが
    // 起動は継続する(他機能は影響を受けない)。
    let Some(content_manager) = web_view.user_content_manager() else {
        return;
    };

    let state = state.clone();
    let web_view = web_view.clone();
    let mode_label = mode_label.clone();
    content_manager.connect_script_message_received(
        Some(HINT_MESSAGE_HANDLER),
        move |_manager, value| {
            let next = match hints::parse_hint_message(value.to_str().as_str()) {
                // §9.2: リンククリック済み/候補 0 件は Normal へ。
                HintMessage::Link | HintMessage::None => Mode::Normal,
                // §9.2: テキスト入力欄 focus 済みは Insert へ。
                HintMessage::Input => Mode::Insert,
                // 未知メッセージ(M6 の focus 等・壊れた JSON)は無視する。
                HintMessage::Ignore => return,
            };
            mode_label.set_text(keys::mode_indicator(next));
            web_view.grab_focus();
            state.set(AppState {
                mode: next,
                pending_key: None,
            });
        },
    );
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
