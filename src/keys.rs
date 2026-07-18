//! Normal モードのキー解決とキーシーケンス(`gg` / `yy`)の状態遷移。
//!
//! GTK イベントに依存しない純粋な状態遷移として切り出す(設計書 §7.3)。
//! `resolve_key` は「現在の pending・モード・入力キー」から「取るべきアクション+
//! 伝播(Stop/Proceed)」と「次の pending」を返す。副作用(実際のスクロールや
//! クリップボード操作、ステータスバー更新)は呼び出し側(`input`)が担う(M3 で結線)。
//! `set_mode`(§6)は仕様上の遷移関数だがテストでのみ参照し、GTK 側は `input::apply_enter_mode`
//! で遷移するため非テストコードから未使用。この種の未使用要素のため dead_code を許可する。
#![allow(dead_code)]

/// モード(設計書 §6)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
    Hint,
}

/// 入力キーの種別(設計書 §7.2)。GTK の keyval・修飾状態を `classify_input` で
/// この純粋な種別へ落としてから解決する。SHIFT は keyval 側で文字へ畳み込み済みのため
/// ここには現れない(`G` は `Char('G')`)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyInput {
    /// 修飾なしの文字キー。
    Char(char),
    /// Ctrl + 文字(`Ctrl+d`/`Ctrl+u` のみバインドあり。§7.4)。
    Ctrl(char),
    /// Escape。
    Esc,
    /// 修飾なしだが文字を持たない特殊キー(矢印・Fn 等)。§7.2 で Normal では Stop。
    SpecialBare,
    /// owl が束ねない修飾付きキー(Alt/Super/Meta、Ctrl+非文字)。§7.2 で Normal では Proceed。
    OtherModified,
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
    ScrollHalfDown,
    ScrollHalfUp,
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
        // Insert: Esc のみ Normal へ、他は(修飾付き含め)すべて素通し(§7.2)。
        Mode::Insert => match input {
            KeyInput::Esc => (KeyOutcome::act(Action::EnterMode(Mode::Normal)), None),
            _ => (KeyOutcome::proceed(), None),
        },
        // Command: すべて素通し(Entry が処理。Esc/Enter は Entry 側で拾う。§7.2)。
        Mode::Command => (KeyOutcome::proceed(), None),
        // Hint: すべて Stop。Esc は Normal へ。ラベル文字の JS 転送は M5 で結線(§7.2・§9)。
        Mode::Hint => match input {
            KeyInput::Esc => (KeyOutcome::act(Action::EnterMode(Mode::Normal)), None),
            _ => (KeyOutcome::stop(), None),
        },
    }
}

/// Normal モードのキー解決(pending の成立・破棄・Esc の排他処理、修飾キー §7.2)。
fn resolve_normal(pending: Option<char>, input: KeyInput) -> (KeyOutcome, Option<char>) {
    match input {
        // Esc は排他: pending があればクリアのみ、なければ読み込み中断(§7.3)。
        KeyInput::Esc => {
            if pending.is_some() {
                (KeyOutcome::stop(), None)
            } else {
                (KeyOutcome::act(Action::StopLoading), None)
            }
        }
        KeyInput::Char(ch) => resolve_normal_char(pending, ch),
        // §7.4: バインド表にある `Ctrl+d`/`Ctrl+u` のみ Stop、他の Ctrl 付きは Proceed(§7.2)。
        // 修飾キーはシーケンスを破棄する(pending → None。§7.3)。
        KeyInput::Ctrl('d') => (KeyOutcome::act(Action::ScrollHalfDown), None),
        KeyInput::Ctrl('u') => (KeyOutcome::act(Action::ScrollHalfUp), None),
        KeyInput::Ctrl(_) => (KeyOutcome::proceed(), None),
        // §7.2・要求 3.3: 修飾なしの未割り当て特殊キーは消費して Stop(ページに漏らさない)。
        KeyInput::SpecialBare => (KeyOutcome::stop(), None),
        // §7.2: owl が束ねない修飾付きキーはページへ素通し。
        KeyInput::OtherModified => (KeyOutcome::proceed(), None),
    }
}

/// Normal モードの文字キー解決(pending の成立・破棄と単独キー再解釈。§7.3)。
fn resolve_normal_char(pending: Option<char>, ch: char) -> (KeyOutcome, Option<char>) {
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

/// スクロール系 `Action` を注入 JS 文字列へ変換する(設計書 §7.4・§8.1)。
///
/// スクロール量は §7.4(`h/j/k/l` は 50px、`Ctrl+d`/`Ctrl+u` は `innerHeight/2`)、
/// `gg`/`G` は `scrollTo` で先頭/末尾。すべて §8.1 の `behavior:'instant'` を指定する。
/// 非スクロール `Action`(ナビゲーション・コピー・モード遷移)は `None`(呼び出し側が別途処理)。
pub fn scroll_script(action: Action) -> Option<&'static str> {
    let js = match action {
        Action::ScrollLeft => "window.scrollBy({left:-50,top:0,behavior:'instant'})",
        Action::ScrollRight => "window.scrollBy({left:50,top:0,behavior:'instant'})",
        Action::ScrollUp => "window.scrollBy({left:0,top:-50,behavior:'instant'})",
        Action::ScrollDown => "window.scrollBy({left:0,top:50,behavior:'instant'})",
        Action::ScrollTop => "window.scrollTo({left:0,top:0,behavior:'instant'})",
        Action::ScrollBottom => {
            "window.scrollTo({left:0,top:document.body.scrollHeight,behavior:'instant'})"
        }
        Action::ScrollHalfDown => {
            "window.scrollBy({left:0,top:window.innerHeight/2,behavior:'instant'})"
        }
        Action::ScrollHalfUp => {
            "window.scrollBy({left:0,top:-window.innerHeight/2,behavior:'instant'})"
        }
        _ => return None,
    };
    Some(js)
}

/// モードインジケータの表示文字列(設計書 §5-2・§12)。Normal は空、他は `-- MODE --` 相当。
pub fn mode_indicator(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => "",
        Mode::Insert => "-- INSERT --",
        Mode::Command => "-- COMMAND --",
        Mode::Hint => "-- HINT --",
    }
}

/// GTK のキーイベント(keyval・修飾状態)を純粋な `KeyInput` へ分類する(設計書 §7.1・§7.2)。
///
/// GTK 境界の判定を単体テスト可能な純粋関数に閉じる。`escape` を最優先し、次に owl が
/// 束ねない修飾(Alt/Super/Meta = `other_mod`)、次に Ctrl(英字のみ `Ctrl`、他は `OtherModified`)、
/// 最後に修飾なし(文字は `Char`、非文字は `SpecialBare`)の順で判定する。SHIFT は keyval 側で
/// 文字へ畳み込み済みのため分類には用いない(`Ctrl+Shift+D` は `Ctrl('d')` に正規化する)。
///
/// 留意点(いずれも現状の Normal バインドでは無害):
/// - `escape` を修飾より優先するため `Ctrl+Esc`/`Alt+Esc` も `Esc` として扱う(§7.2 の
///   「修飾付きは Proceed」より Esc の中断を優先。design §7.2 に明記、テスト C-01 で固定)。
/// - 修飾キー単独押下(Shift/Ctrl のみ等)は `unicode=None`・修飾なしのため `SpecialBare` と
///   なり Normal では Stop → **pending を破棄する**。`gg`/`yy` は Shift 不要なので問題ないが、
///   将来 Shift を含む 2 打鍵シーケンスを足す場合はこの破棄に注意する。
/// - GDK は Enter/Tab/BackSpace 等を制御文字(`'\r'`/`'\t'`/`'\u{8}'` 等)へ写すため `Char` に
///   なる(`SpecialBare` ではない)。Normal では未割り当て文字として Stop、Insert では Proceed
///   となり挙動は正しいが、`Char` が印字可能とは限らない点に注意する。
pub fn classify_input(
    escape: bool,
    ctrl: bool,
    other_mod: bool,
    unicode: Option<char>,
) -> KeyInput {
    if escape {
        return KeyInput::Esc;
    }
    if other_mod {
        return KeyInput::OtherModified;
    }
    if ctrl {
        return match unicode {
            Some(c) if c.is_ascii_alphabetic() => KeyInput::Ctrl(c.to_ascii_lowercase()),
            _ => KeyInput::OtherModified,
        };
    }
    match unicode {
        Some(c) => KeyInput::Char(c),
        None => KeyInput::SpecialBare,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Action, KeyInput, KeyOutcome, Mode, Propagation, classify_input, mode_indicator,
        resolve_key, scroll_script, set_mode,
    };

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

    // --- 2.6 修飾キー(Normal、§7.2・§7.4)---

    #[test]
    fn k50_ctrl_d_scrolls_half_down() {
        let (outcome, pending) = normal(None, KeyInput::Ctrl('d'));
        assert_eq!(outcome.action, Some(Action::ScrollHalfDown));
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, None);
    }

    #[test]
    fn k51_ctrl_u_scrolls_half_up() {
        let (outcome, pending) = normal(None, KeyInput::Ctrl('u'));
        assert_eq!(outcome.action, Some(Action::ScrollHalfUp));
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, None);
    }

    #[test]
    fn k52_unbound_ctrl_proceeds() {
        // §7.2: バインド表にない修飾付きキーはページへ素通し(Ctrl+C コピー等)。
        let (outcome, pending) = normal(None, KeyInput::Ctrl('a'));
        assert_eq!(outcome.action, None);
        assert_eq!(outcome.propagation, Propagation::Proceed);
        assert_eq!(pending, None);
    }

    #[test]
    fn k53_special_bare_is_stopped() {
        // §7.2・要求 3.3: 修飾なしの未割り当てキー(矢印等)は消費して Stop(漏らさない)。
        let (outcome, pending) = normal(None, KeyInput::SpecialBare);
        assert_eq!(outcome.action, None);
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, None);
    }

    #[test]
    fn k54_other_modified_proceeds() {
        // §7.2: Ctrl+d/u 以外の修飾付き(Alt/Super 等)は Proceed。
        let (outcome, pending) = normal(None, KeyInput::OtherModified);
        assert_eq!(outcome.action, None);
        assert_eq!(outcome.propagation, Propagation::Proceed);
        assert_eq!(pending, None);
    }

    #[test]
    fn k55_pending_g_then_ctrl_d_discards_and_scrolls() {
        // §7.3: 修飾キーはシーケンスを破棄。pending をクリアして Ctrl+d を実行。
        let (outcome, pending) = normal(Some('g'), KeyInput::Ctrl('d'));
        assert_eq!(outcome.action, Some(Action::ScrollHalfDown));
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, None);
    }

    #[test]
    fn k56_pending_g_then_special_bare_discards_and_stops() {
        let (outcome, pending) = normal(Some('g'), KeyInput::SpecialBare);
        assert_eq!(outcome.action, None);
        assert_eq!(outcome.propagation, Propagation::Stop);
        assert_eq!(pending, None);
    }

    #[test]
    fn k57_pending_g_then_other_modified_discards_and_proceeds() {
        let (outcome, pending) = normal(Some('g'), KeyInput::OtherModified);
        assert_eq!(outcome.action, None);
        assert_eq!(outcome.propagation, Propagation::Proceed);
        assert_eq!(pending, None);
    }

    // --- 2.7 モード別の非文字キー(§7.2)---

    #[test]
    fn k60_62_insert_non_char_proceeds() {
        for input in [
            KeyInput::Ctrl('a'),
            KeyInput::SpecialBare,
            KeyInput::OtherModified,
        ] {
            let (outcome, pending) = resolve_key(None, Mode::Insert, input);
            assert_eq!(outcome.action, None, "input {input:?}");
            assert_eq!(outcome.propagation, Propagation::Proceed, "input {input:?}");
            assert_eq!(pending, None, "input {input:?}");
        }
    }

    #[test]
    fn k63_65_hint_non_char_stops() {
        for input in [
            KeyInput::Ctrl('a'),
            KeyInput::SpecialBare,
            KeyInput::OtherModified,
        ] {
            let (outcome, pending) = resolve_key(None, Mode::Hint, input);
            assert_eq!(outcome.action, None, "input {input:?}");
            assert_eq!(outcome.propagation, Propagation::Stop, "input {input:?}");
            assert_eq!(pending, None, "input {input:?}");
        }
    }

    #[test]
    fn k66_command_non_char_proceeds() {
        for input in [
            KeyInput::Ctrl('a'),
            KeyInput::SpecialBare,
            KeyInput::OtherModified,
        ] {
            let (outcome, pending) = resolve_key(None, Mode::Command, input);
            assert_eq!(outcome.action, None, "input {input:?}");
            assert_eq!(outcome.propagation, Propagation::Proceed, "input {input:?}");
            assert_eq!(pending, None, "input {input:?}");
        }
    }

    // --- 2.8 classify_input(GTK キーイベント → KeyInput)---

    #[test]
    fn c01_escape_takes_priority() {
        // Escape は ctrl 併用でも Esc(短絡)。
        assert_eq!(
            classify_input(true, true, false, Some('\u{1b}')),
            KeyInput::Esc
        );
    }

    #[test]
    fn c02_other_mod_is_other_modified() {
        assert_eq!(
            classify_input(false, false, true, Some('x')),
            KeyInput::OtherModified
        );
    }

    #[test]
    fn c03_ctrl_letter_is_ctrl() {
        assert_eq!(
            classify_input(false, true, false, Some('d')),
            KeyInput::Ctrl('d')
        );
    }

    #[test]
    fn c04_ctrl_shift_letter_lowercases() {
        // Shift 併用の大文字は小文字化して Ctrl に畳む。
        assert_eq!(
            classify_input(false, true, false, Some('D')),
            KeyInput::Ctrl('d')
        );
    }

    #[test]
    fn c05_ctrl_non_char_is_other_modified() {
        assert_eq!(
            classify_input(false, true, false, None),
            KeyInput::OtherModified
        );
    }

    #[test]
    fn c06_ctrl_non_letter_is_other_modified() {
        assert_eq!(
            classify_input(false, true, false, Some('4')),
            KeyInput::OtherModified
        );
    }

    #[test]
    fn c07_bare_char_is_char() {
        assert_eq!(
            classify_input(false, false, false, Some('j')),
            KeyInput::Char('j')
        );
    }

    #[test]
    fn c08_bare_non_char_is_special_bare() {
        assert_eq!(
            classify_input(false, false, false, None),
            KeyInput::SpecialBare
        );
    }

    // --- 2.9 scroll_script(Action → 注入 JS)---

    #[test]
    fn s01_08_scroll_scripts_are_exact() {
        let cases = [
            (
                Action::ScrollLeft,
                "window.scrollBy({left:-50,top:0,behavior:'instant'})",
            ),
            (
                Action::ScrollRight,
                "window.scrollBy({left:50,top:0,behavior:'instant'})",
            ),
            (
                Action::ScrollUp,
                "window.scrollBy({left:0,top:-50,behavior:'instant'})",
            ),
            (
                Action::ScrollDown,
                "window.scrollBy({left:0,top:50,behavior:'instant'})",
            ),
            (
                Action::ScrollTop,
                "window.scrollTo({left:0,top:0,behavior:'instant'})",
            ),
            (
                Action::ScrollBottom,
                "window.scrollTo({left:0,top:document.body.scrollHeight,behavior:'instant'})",
            ),
            (
                Action::ScrollHalfDown,
                "window.scrollBy({left:0,top:window.innerHeight/2,behavior:'instant'})",
            ),
            (
                Action::ScrollHalfUp,
                "window.scrollBy({left:0,top:-window.innerHeight/2,behavior:'instant'})",
            ),
        ];
        for (action, js) in cases {
            assert_eq!(scroll_script(action), Some(js), "action {action:?}");
        }
    }

    #[test]
    fn s09_non_scroll_actions_have_no_script() {
        assert_eq!(scroll_script(Action::Back), None);
        assert_eq!(scroll_script(Action::CopyUrl), None);
    }

    // --- 2.10 mode_indicator(Mode → ステータスバー表示)---

    #[test]
    fn m01_04_mode_indicator_strings() {
        assert_eq!(mode_indicator(Mode::Normal), "");
        assert_eq!(mode_indicator(Mode::Insert), "-- INSERT --");
        assert_eq!(mode_indicator(Mode::Command), "-- COMMAND --");
        assert_eq!(mode_indicator(Mode::Hint), "-- HINT --");
    }
}
