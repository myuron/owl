//! hint モードの純粋ロジック(設計書 §9)。
//!
//! Rust ⇔ page.js の通信文字列の組み立て/解釈だけを担い、GTK/WebKit には依存しない
//! (§4 の純粋ロジック分離)。実際の `evaluate_javascript` 送信・モード遷移などの副作用は
//! 呼び出し側(`input`)が担う。
//!
//! - Rust → JS(§9.2): `owlHints.start()` / `owlHints.input(ch)` / `owlHints.cancel()` を
//!   組み立てる。`input_script` は Hint モードが**全ての文字キー**を転送する(§7.2)ため、
//!   `'`・`\`・改行等が来ても JS 構文を壊さないようエスケープする。
//! - JS → Rust(§9.2): script message handler `"owl"` が受け取る JSON 文字列を
//!   `parse_hint_message` で `HintMessage` へ解釈する。

/// JS → Rust の hint 結果メッセージ(設計書 §9.2)。呼び出し側がこれを見てモード遷移する
/// (`Link`/`None` → Normal、`Input` → Insert)。`Ignore` は未知メッセージ(M6 の `focus` 等、
/// あるいは壊れた JSON)を安全に読み飛ばすための前方互換アーム。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintMessage {
    /// `{"type":"hint_result","target":"link"}` — リンクをクリック実行済み。Normal へ。
    Link,
    /// `{"type":"hint_result","target":"input"}` — テキスト入力欄を focus 実行済み。Insert へ。
    Input,
    /// `{"type":"hint_none"}` — 候補 0 件(絞り込みで全滅含む)。Normal へ。
    None,
    /// 上記いずれにも当てはまらないメッセージ。無視する。
    Ignore,
}

/// `owlHints.start()`(要素列挙とラベル表示。§9.2)。
pub fn start_script() -> &'static str {
    "owlHints.start()"
}

/// `owlHints.cancel()`(オーバーレイ除去。§9.2)。
pub fn cancel_script() -> &'static str {
    "owlHints.cancel()"
}

/// `owlHints.input(ch)` を組み立てる(ラベル文字の追加入力。§9.2)。
///
/// Hint モードは修飾なしの**全文字キー**を転送する(§7.2)ため、`ch` には任意の文字が来うる。
/// シングルクォート文字列(§9.2 の表記に合わせる)を壊さないよう、`'`・`\`・制御文字・
/// JS の行区切り(U+2028/U+2029)をエスケープする。
pub fn input_script(ch: char) -> String {
    let mut script = String::from("owlHints.input('");
    push_js_escaped(&mut script, ch);
    script.push_str("')");
    script
}

/// 単一文字を JS のシングルクォート文字列リテラル向けにエスケープして追記する。
fn push_js_escaped(out: &mut String, ch: char) {
    match ch {
        '\'' => out.push_str("\\'"),
        '\\' => out.push_str("\\\\"),
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        // U+2028/U+2029 は JSON では有効だが JS の文字列リテラルでは行終端子となり構文を壊す。
        '\u{2028}' => out.push_str("\\u2028"),
        '\u{2029}' => out.push_str("\\u2029"),
        // その他の制御文字(U+0000〜U+001F)は `\uXXXX` へ。4 桁ゼロ詰めを守る。
        c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
        c => out.push(c),
    }
}

/// script message handler `"owl"` が受け取った JSON 文字列を `HintMessage` へ解釈する(§9.2)。
///
/// owl は送信側(page.js)も握るため、想定する固定の少数メッセージのみを認識すればよい。
/// 依存を増やさず(既存方針。§11 も std のみ)`type`/`target` フィールドの文字列値を取り出して
/// 判定する。未知の `type`・未知の `target`・壊れた入力はすべて `Ignore` に倒す。
pub fn parse_hint_message(input: &str) -> HintMessage {
    match json_string_field(input, "type") {
        Some("hint_none") => HintMessage::None,
        Some("hint_result") => match json_string_field(input, "target") {
            Some("link") => HintMessage::Link,
            Some("input") => HintMessage::Input,
            _ => HintMessage::Ignore,
        },
        _ => HintMessage::Ignore,
    }
}

/// `input` から `"key"` に対応する JSON 文字列値を取り出す(該当なしは `None`)。
///
/// `"key"` を探し、続く `:` と空白を読み飛ばし、`"` で囲まれた値を返す最小実装。owl が送る
/// メッセージの値にはエスケープ(`\"` 等)が現れないため、エスケープ解釈は行わない。
fn json_string_field<'a>(input: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\"");
    let after_key = input.find(&needle)? + needle.len();
    let rest = input[after_key..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(&rest[..end])
}

#[cfg(test)]
mod tests {
    use super::{HintMessage, cancel_script, input_script, parse_hint_message, start_script};

    // --- Rust → JS(§9.2)---

    #[test]
    fn h01_start_and_cancel_scripts_are_exact() {
        assert_eq!(start_script(), "owlHints.start()");
        assert_eq!(cancel_script(), "owlHints.cancel()");
    }

    #[test]
    fn h02_input_script_plain_char() {
        // ラベル文字(ホームロー)はそのまま埋め込む。
        assert_eq!(input_script('a'), "owlHints.input('a')");
        assert_eq!(input_script('s'), "owlHints.input('s')");
    }

    #[test]
    fn h03_input_script_escapes_quote_and_backslash() {
        // §7.2: 全文字を転送するため、クォート/バックスラッシュで JS 構文を壊さない。
        assert_eq!(input_script('\''), "owlHints.input('\\'')");
        assert_eq!(input_script('\\'), "owlHints.input('\\\\')");
    }

    #[test]
    fn h04_input_script_escapes_control_and_line_separators() {
        assert_eq!(input_script('\n'), "owlHints.input('\\n')");
        assert_eq!(input_script('\r'), "owlHints.input('\\r')");
        assert_eq!(input_script('\t'), "owlHints.input('\\t')");
        assert_eq!(input_script('\u{2028}'), "owlHints.input('\\u2028')");
        assert_eq!(input_script('\u{2029}'), "owlHints.input('\\u2029')");
        // 一般制御文字は 4 桁ゼロ詰めの `\uXXXX`(ゼロ詰めを外す mutant を落とす)。
        assert_eq!(input_script('\u{1}'), "owlHints.input('\\u0001')");
        assert_eq!(input_script('\u{1f}'), "owlHints.input('\\u001f')");
    }

    #[test]
    fn h05_input_script_boundary_space_is_literal() {
        // U+0020(空白)は制御文字ではないためエスケープしない(境界 `< 0x20` を固定する)。
        assert_eq!(input_script(' '), "owlHints.input(' ')");
    }

    // --- JS → Rust(§9.2)---

    #[test]
    fn h10_parse_hint_result_link() {
        assert_eq!(
            parse_hint_message("{\"type\":\"hint_result\",\"target\":\"link\"}"),
            HintMessage::Link
        );
    }

    #[test]
    fn h11_parse_hint_result_input() {
        assert_eq!(
            parse_hint_message("{\"type\":\"hint_result\",\"target\":\"input\"}"),
            HintMessage::Input
        );
    }

    #[test]
    fn h12_parse_hint_none() {
        assert_eq!(
            parse_hint_message("{\"type\":\"hint_none\"}"),
            HintMessage::None
        );
    }

    #[test]
    fn h13_parse_unknown_type_is_ignore() {
        // M6 の focus メッセージ等・未知 type は無視する(前方互換)。
        assert_eq!(
            parse_hint_message("{\"type\":\"focus\",\"editable\":true}"),
            HintMessage::Ignore
        );
    }

    #[test]
    fn h14_parse_hint_result_unknown_target_is_ignore() {
        assert_eq!(
            parse_hint_message("{\"type\":\"hint_result\",\"target\":\"bogus\"}"),
            HintMessage::Ignore
        );
        // target 欠落も Ignore。
        assert_eq!(
            parse_hint_message("{\"type\":\"hint_result\"}"),
            HintMessage::Ignore
        );
    }

    #[test]
    fn h15_parse_malformed_is_ignore() {
        // type 欠落・非 JSON・空文字はすべて Ignore。
        assert_eq!(parse_hint_message("{}"), HintMessage::Ignore);
        assert_eq!(parse_hint_message("not json"), HintMessage::Ignore);
        assert_eq!(parse_hint_message(""), HintMessage::Ignore);
        // フィールド値の抽出が壊れた形でも安全に Ignore へ倒れることを固定する
        // (`json_string_field` の各短絡パス: コロン欠落・非文字列値・閉じ引用符欠落)。
        assert_eq!(
            parse_hint_message("{\"type\" \"hint_none\"}"),
            HintMessage::Ignore
        );
        assert_eq!(parse_hint_message("{\"type\":5}"), HintMessage::Ignore);
        assert_eq!(
            parse_hint_message("{\"type\":\"hint_none"),
            HintMessage::Ignore
        );
    }

    #[test]
    fn h16_parse_tolerates_whitespace_around_colon() {
        // `:` 前後の空白を許容する(値の抽出が固定表記に過度依存しないことを固定)。
        assert_eq!(
            parse_hint_message("{ \"type\" : \"hint_none\" }"),
            HintMessage::None
        );
    }
}
