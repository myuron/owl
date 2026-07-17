//! Normal モードのキー解決とキーシーケンス(`gg` / `yy`)の状態遷移。
//!
//! GTK イベントに依存しない純粋な状態遷移として切り出す(設計書 §7.3)。
//! `resolve_key` は「現在の pending・モード・入力キー」から「取るべきアクション+
//! 伝播(Stop/Proceed)」と「次の pending」を返す。副作用(実際のスクロールや
//! クリップボード操作、ステータスバー更新)は呼び出し側が担う。実装は後続
//! マイルストーンで `main` から結線されるまで未使用のため dead_code を許可する。
#![allow(dead_code)]

/// モード(設計書 §6)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
    Hint,
}

/// 修飾なしの入力キー。テスト対象の状態遷移に必要な種類のみを扱う。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyInput {
    Char(char),
    Esc,
}

/// キー解決の結果として呼び出し側が実行すべきアクション(設計書 §7.4)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ScrollLeft,
    ScrollDown,
    ScrollUp,
    ScrollRight,
    ScrollTop,
    ScrollBottom,
    Back,
    Forward,
    Reload,
    CopyUrl,
    StopLoading,
    EnterMode(Mode),
}

/// イベントの伝播(設計書 §7.1)。`Stop` = owl が消費、`Proceed` = 素通し。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Propagation {
    Stop,
    Proceed,
}

/// キー解決の結果。取るべきアクション(なければ `None`)と伝播。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyOutcome {
    pub action: Option<Action>,
    pub propagation: Propagation,
}

impl KeyOutcome {
    fn new(action: Option<Action>, propagation: Propagation) -> Self {
        Self {
            action,
            propagation,
        }
    }

    /// アクションを伴わずにキーを消費する(pending 記録・Esc クリア等)。
    fn stop() -> Self {
        Self::new(None, Propagation::Stop)
    }

    /// アクションを実行しつつキーを消費する。
    fn act(action: Action) -> Self {
        Self::new(Some(action), Propagation::Stop)
    }

    /// キーを素通しする(Insert/Command でのページ入力)。
    fn proceed() -> Self {
        Self::new(None, Propagation::Proceed)
    }
}

/// 現在の pending・モード・入力キーから、取るべき結果と次の pending を返す。
///
/// 判定順(設計書 §7.2・§7.3):
/// - Insert / Command: `Esc` 以外は素通し(pending は使わない)。
/// - Normal + `Esc`: pending があればクリアのみ、なければ `StopLoading`。
/// - Normal + 文字キー: pending が成立すれば `gg`/`yy` を実行、
///   それ以外は破棄してそのキーを単独キーとして解釈し直す。
pub fn resolve_key(
    pending: Option<char>,
    mode: Mode,
    input: KeyInput,
) -> (KeyOutcome, Option<char>) {
    match mode {
        Mode::Normal => resolve_normal(pending, input),
        // Insert: Esc のみ Normal へ、他は素通し(§7.2)。
        Mode::Insert => match input {
            KeyInput::Esc => (KeyOutcome::act(Action::EnterMode(Mode::Normal)), None),
            KeyInput::Char(_) => (KeyOutcome::proceed(), None),
        },
        // Command: すべて素通し(Entry が処理。Esc/Enter は Entry 側で拾う。§7.2)。
        Mode::Command => (KeyOutcome::proceed(), None),
        // Hint: すべて Stop。Esc は Normal へ。ラベル文字の JS 転送は M5 で結線(§7.2・§9)。
        Mode::Hint => match input {
            KeyInput::Esc => (KeyOutcome::act(Action::EnterMode(Mode::Normal)), None),
            KeyInput::Char(_) => (KeyOutcome::stop(), None),
        },
    }
}

/// Normal モードのキー解決(pending の成立・破棄・Esc の排他処理)。
fn resolve_normal(pending: Option<char>, input: KeyInput) -> (KeyOutcome, Option<char>) {
    let ch = match input {
        // Esc は排他: pending があればクリアのみ、なければ読み込み中断(§7.3)。
        KeyInput::Esc => {
            if pending.is_some() {
                return (KeyOutcome::stop(), None);
            }
            return (KeyOutcome::act(Action::StopLoading), None);
        }
        KeyInput::Char(c) => c,
    };

    match pending {
        // シーケンス成立: `gg` → ページ先頭、`yy` → URL コピー。
        Some('g') if ch == 'g' => (KeyOutcome::act(Action::ScrollTop), None),
        Some('y') if ch == 'y' => (KeyOutcome::act(Action::CopyUrl), None),
        // pending なし、または不成立で破棄 → 単独キーとして解釈し直す(§7.3)。
        _ => resolve_normal_single(ch),
    }
}

/// 単独キーのバインド表(設計書 §7.4)。`g`/`y` はプレフィックスキーとして
/// pending に記録し、割り当てのないキーは何もせず Stop する(§7.2)。
fn resolve_normal_single(ch: char) -> (KeyOutcome, Option<char>) {
    match ch {
        'g' | 'y' => (KeyOutcome::stop(), Some(ch)),
        'h' => (KeyOutcome::act(Action::ScrollLeft), None),
        'j' => (KeyOutcome::act(Action::ScrollDown), None),
        'k' => (KeyOutcome::act(Action::ScrollUp), None),
        'l' => (KeyOutcome::act(Action::ScrollRight), None),
        'G' => (KeyOutcome::act(Action::ScrollBottom), None),
        'H' => (KeyOutcome::act(Action::Back), None),
        'L' => (KeyOutcome::act(Action::Forward), None),
        'r' => (KeyOutcome::act(Action::Reload), None),
        'i' => (KeyOutcome::act(Action::EnterMode(Mode::Insert)), None),
        ':' => (KeyOutcome::act(Action::EnterMode(Mode::Command)), None),
        'f' => (KeyOutcome::act(Action::EnterMode(Mode::Hint)), None),
        // 未割り当て: 何もせず Stop(ページに漏らさない。要求 3.3)。
        _ => (KeyOutcome::stop(), None),
    }
}

/// モード遷移(設計書 §6)。遷移をまたいでキーシーケンスは持ち越さないため、
/// 新しいモードと、必ずクリアされた `pending`(= `None`)を返す。
pub fn set_mode(new_mode: Mode) -> (Mode, Option<char>) {
    (new_mode, None)
}

#[cfg(test)]
mod tests {
    use super::{Action, KeyInput, KeyOutcome, Mode, Propagation, resolve_key, set_mode};

    /// Normal モード + 指定 pending で 1 キーを解決するヘルパー。
    fn normal(pending: Option<char>, input: KeyInput) -> (KeyOutcome, Option<char>) {
        resolve_key(pending, Mode::Normal, input)
    }

    // --- 2.1 シーケンス開始 ---

    #[test]
    fn k01_g_records_pending_and_stops() {
        let (outcome, pending) = normal(None, KeyInput::Char('g'));
        assert_eq!(outcome.action, None);
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, Some('g'));
    }

    #[test]
    fn k02_y_records_pending_and_stops() {
        let (outcome, pending) = normal(None, KeyInput::Char('y'));
        assert_eq!(outcome.action, None);
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, Some('y'));
    }

    // --- 2.2 シーケンス成立 ---

    #[test]
    fn k10_gg_scrolls_to_top_and_clears() {
        let (outcome, pending) = normal(Some('g'), KeyInput::Char('g'));
        assert_eq!(outcome.action, Some(Action::ScrollTop));
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, None);
    }

    #[test]
    fn k11_yy_copies_url_and_clears() {
        let (outcome, pending) = normal(Some('y'), KeyInput::Char('y'));
        assert_eq!(outcome.action, Some(Action::CopyUrl));
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, None);
    }

    // --- 2.3 シーケンス破棄と再解釈 ---

    #[test]
    fn k20_g_then_j_reinterprets_as_scroll_down() {
        let (outcome, pending) = normal(Some('g'), KeyInput::Char('j'));
        assert_eq!(outcome.action, Some(Action::ScrollDown));
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, None);
    }

    #[test]
    fn k21_y_then_capital_g_reinterprets_as_scroll_bottom() {
        let (outcome, pending) = normal(Some('y'), KeyInput::Char('G'));
        assert_eq!(outcome.action, Some(Action::ScrollBottom));
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, None);
    }

    #[test]
    fn k22_g_then_y_becomes_pending_y() {
        // 破棄 → `y` は単独ではプレフィックスキー → pending が `y` になる
        let (outcome, pending) = normal(Some('g'), KeyInput::Char('y'));
        assert_eq!(outcome.action, None);
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, Some('y'));
    }

    #[test]
    fn k23_y_then_g_becomes_pending_g() {
        let (outcome, pending) = normal(Some('y'), KeyInput::Char('g'));
        assert_eq!(outcome.action, None);
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, Some('g'));
    }

    #[test]
    fn k24_g_then_unassigned_does_nothing() {
        let (outcome, pending) = normal(Some('g'), KeyInput::Char('q'));
        assert_eq!(outcome.action, None);
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, None);
    }

    #[test]
    fn k25_g_then_i_enters_insert() {
        let (outcome, pending) = normal(Some('g'), KeyInput::Char('i'));
        assert_eq!(outcome.action, Some(Action::EnterMode(Mode::Insert)));
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, None);
    }

    #[test]
    fn k26_g_then_colon_enters_command() {
        let (outcome, pending) = normal(Some('g'), KeyInput::Char(':'));
        assert_eq!(outcome.action, Some(Action::EnterMode(Mode::Command)));
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, None);
    }

    // --- 2.4 Esc の排他処理 ---

    #[test]
    fn k30_esc_with_pending_only_clears() {
        // pending があれば クリアのみ(読み込み中断は呼ばれない)
        let (outcome, pending) = normal(Some('g'), KeyInput::Esc);
        assert_eq!(outcome.action, None);
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, None);
    }

    #[test]
    fn k31_esc_without_pending_stops_loading() {
        let (outcome, pending) = normal(None, KeyInput::Esc);
        assert_eq!(outcome.action, Some(Action::StopLoading));
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, None);
    }

    // --- 2.5 モード境界 ---

    #[test]
    fn k40_insert_mode_g_is_not_recorded_and_proceeds() {
        // Insert では素通し。pending に記録しない(§7.2)
        let (outcome, pending) = resolve_key(None, Mode::Insert, KeyInput::Char('g'));
        assert_eq!(outcome.action, None);
        assert_eq!(outcome.propagation, Propagation::Proceed);
        assert_eq!(pending, None);
    }

    #[test]
    fn k41_set_mode_clears_pending() {
        // §6: set_mode はモード遷移をまたいで pending を持ち越さない
        let (mode, pending) = set_mode(Mode::Normal);
        assert_eq!(mode, Mode::Normal);
        assert_eq!(pending, None);
        // クリア後、Normal 復帰後の `g` は新規シーケンス開始として扱われる
        let (outcome, pending) = resolve_key(pending, mode, KeyInput::Char('g'));
        assert_eq!(outcome.action, None);
        assert_eq!(pending, Some('g'));
    }

    // --- モード別アームの網羅(K-ID 外だが相異なる分岐を固定する)---

    #[test]
    fn insert_esc_enters_normal() {
        let (outcome, pending) = resolve_key(None, Mode::Insert, KeyInput::Esc);
        assert_eq!(outcome.action, Some(Action::EnterMode(Mode::Normal)));
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, None);
    }

    #[test]
    fn command_mode_proceeds() {
        // Esc/Enter も含め Entry 側が処理するため素通し(§7.2)
        for input in [KeyInput::Char('a'), KeyInput::Esc] {
            let (outcome, pending) = resolve_key(None, Mode::Command, input);
            assert_eq!(outcome.action, None);
            assert_eq!(outcome.propagation, Propagation::Proceed);
            assert_eq!(pending, None);
        }
    }

    #[test]
    fn hint_char_is_stopped() {
        // Hint はすべて Stop(ラベル文字はページに漏らさない。§7.2)
        let (outcome, pending) = resolve_key(None, Mode::Hint, KeyInput::Char('a'));
        assert_eq!(outcome.action, None);
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, None);
    }

    #[test]
    fn hint_esc_enters_normal() {
        let (outcome, pending) = resolve_key(None, Mode::Hint, KeyInput::Esc);
        assert_eq!(outcome.action, Some(Action::EnterMode(Mode::Normal)));
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, None);
    }

    // --- 単独キーのバインド表(§7.4)の網羅 ---

    #[test]
    fn single_key_bindings() {
        let cases = [
            ('h', Action::ScrollLeft),
            ('j', Action::ScrollDown),
            ('k', Action::ScrollUp),
            ('l', Action::ScrollRight),
            ('G', Action::ScrollBottom),
            ('H', Action::Back),
            ('L', Action::Forward),
            ('r', Action::Reload),
            ('i', Action::EnterMode(Mode::Insert)),
            (':', Action::EnterMode(Mode::Command)),
            ('f', Action::EnterMode(Mode::Hint)),
        ];
        for (ch, action) in cases {
            let (outcome, pending) = normal(None, KeyInput::Char(ch));
            assert_eq!(outcome.action, Some(action), "key {ch:?}");
            assert_eq!(outcome.propagation, Propagation::Stop, "key {ch:?}");
            assert_eq!(pending, None, "key {ch:?}");
        }
    }
}
