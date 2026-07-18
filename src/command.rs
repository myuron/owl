//! command モード: コマンドのディスパッチ・`:open` の入力解釈、および起動時の GTK 非依存な
//! 純粋ヘルパー。
//!
//! `parse_command` はコマンドライン入力(`:open`/`:quit`/未知)の分類(設計書 §11)、
//! `parse_open_input` は `:open` の URL 補完(§11)。加えて、GTK 側(起動フロー・
//! ステータスバー)が必要とする純粋ロジック — 初期 URL 決定(`initial_uri`, §13-3)、
//! NetworkSession の data/cache ディレクトリ算出(`app_subdir`, §8.2)、ステータスバーの
//! 読み込み状態表示(`format_load_progress`, §12)— をここに集約する。いずれも gtk/webkit
//! に依存せず単体テストできる形にし、実際の `load_uri`・ディレクトリ作成・ラベル更新・
//! 終了・エラー表示等の副作用は呼び出し側(`input`/`window`/`webview`)が担う(設計書 §4 の
//! 純粋ロジック分離)。

use std::path::{Path, PathBuf};

const DUCKDUCKGO_SEARCH: &str = "https://duckduckgo.com/?q=";
const HTTP_SCHEME: &str = "http://";
const HTTPS_SCHEME: &str = "https://";

/// `:open` の引数が空(`parse_open_input` が `None`)のときの使用法エラー(設計書 §11)。
const OPEN_USAGE: &str = "usage: open <url or query>";

/// 起動引数が無いときの初期 URL(設計書 §13-3)。
const BLANK_URI: &str = "about:blank";

/// XDG ベースディレクトリ配下に置く owl 専用サブディレクトリ名(設計書 §8.2)。
const APP_SUBDIR: &str = "owl";

/// XDG ベースディレクトリ(`base`)配下の owl 用ディレクトリを返す(設計書 §8.2)。
///
/// NetworkSession の data=`$XDG_DATA_HOME/owl`・cache=`$XDG_CACHE_HOME/owl` は
/// いずれも各 XDG ベース + `owl`。ベースを引数で受け取る純粋関数にし、GTK 依存の
/// `glib::user_data_dir()` / `user_cache_dir()` 取得と実際のディレクトリ作成
/// (§13-4 の同期 I/O)は呼び出し側(`webview`)が担う。
pub fn app_subdir(base: &Path) -> PathBuf {
    base.join(APP_SUBDIR)
}

/// 起動引数から初期 URL を決める(設計書 §13-3)。GTK 非依存の純粋関数。
///
/// 第 1 引数(`arg`)があればそれを、無ければ `about:blank` を返す。
/// M1 では生 URL を素通しする(`:open` の補完規則 §11 = `parse_open_input`
/// の適用は M4。§13-3 は最終的に補完規則を通す想定だが、todo サイクル 3 冒頭の
/// とおり M1 では前倒し表示のため生 URL のまま渡す)。実際の `load_uri` は
/// 呼び出し側(`main`/`webview`)が担う。
pub fn initial_uri(arg: Option<&str>) -> &str {
    arg.unwrap_or(BLANK_URI)
}

/// ステータスバーの読み込み状態表示を組む(設計書 §12)。GTK 非依存の純粋関数。
///
/// `notify::is-loading`(`is_loading`)と `notify::estimated-load-progress`
/// (`progress`、0.0〜1.0)を文字列化する。§12「読み込み中のみ `[42%]` 等を表示」に
/// 従い、読み込み中は `[NN%]`(四捨五入)を、非読み込み時は空文字を返す。実際の
/// ラベル更新は呼び出し側(`window`)が notify シグナルで駆動する。
pub fn format_load_progress(is_loading: bool, progress: f64) -> String {
    if !is_loading {
        return String::new();
    }
    format!("[{}%]", (progress * 100.0).round() as u32)
}

/// command モードのコマンドライン入力を解釈した結果(設計書 §11)。
///
/// 副作用(`load_uri`・終了・エラー表示)は呼び出し側(`input`)が担う(設計書 §4 の
/// 純粋ロジック分離)。`Open` の URL は `parse_open_input` で補完済み(呼び出し側は
/// そのまま `load_uri` するだけ)。`Error` の文字列はステータスバーへそのまま表示する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// `:open <input>` → 補完済み URL を読み込む。
    Open(String),
    /// `:quit` → 終了する。
    Quit,
    /// 空(`:` のみ・空白のみ・空文字列)→ 何もせず Normal へ戻すだけ。
    Noop,
    /// 未知コマンド、または `:open` の空引数 → ステータスバーにエラー表示。
    Error(String),
}

/// command モードのコマンドライン入力を `Command` へ分類する(設計書 §11)。
///
/// 入力は Entry の初期値 `:`(§5-3)から始まる。先頭 `:` を 1 個だけ剥がし、trim してから
/// 最初の空白でコマンド名と引数に分割する。コマンドは `:open <input>` と `:quit` の 2 つ
/// (前方一致補完はしない。MVP)。コマンド名は大文字小文字を区別する。`:open` の引数は
/// `parse_open_input`(§1)で補完済み URL に解決する。空入力は `Noop`、未知コマンドと
/// `:open` の空引数は `Error`。
pub fn parse_command(input: &str) -> Command {
    // 先頭 `:`(コマンドプロンプト)を 1 個だけ剥がして trim する。先頭が `:` でなければ
    // そのまま(Entry が空・`:` を消された場合の堅牢性)。
    let body = input.strip_prefix(':').unwrap_or(input).trim();
    if body.is_empty() {
        return Command::Noop;
    }
    // 最初の空白でコマンド名と引数へ分割する(空白がなければ引数なし)。引数側の trim は
    // `parse_open_input` が担うためここではしない。
    let (name, rest) = match body.split_once(char::is_whitespace) {
        Some((name, rest)) => (name, rest),
        None => (body, ""),
    };
    match name {
        "open" => match parse_open_input(rest) {
            Some(url) => Command::Open(url),
            None => Command::Error(OPEN_USAGE.to_string()),
        },
        "quit" => Command::Quit,
        _ => Command::Error(format!("unknown command: {name}")),
    }
}

/// `:open <input>` の入力を解釈して遷移先 URL を返す(`None` = 空入力)。
///
/// 入力を trim 後、規則 1→5 を上から順に適用する(設計書 §11):
/// 1. 空 → `None`
/// 2. `localhost`(任意で `:数字` / `/パス`)→ `http://` を補完
/// 3. スキームあり → そのまま
/// 4. 空白を含まず `.` を含む → `https://` を補完
/// 5. それ以外 → DuckDuckGo 検索
pub fn parse_open_input(input: &str) -> Option<String> {
    let input = input.trim();

    if input.is_empty() {
        return None; // 規則 1
    }
    if is_localhost(input) {
        return Some(format!("{HTTP_SCHEME}{input}")); // 規則 2
    }
    if has_scheme(input) {
        return Some(input.to_string()); // 規則 3
    }
    if looks_like_host(input) {
        return Some(format!("{HTTPS_SCHEME}{input}")); // 規則 4
    }
    Some(format!("{DUCKDUCKGO_SEARCH}{}", percent_encode(input))) // 規則 5
}

/// 規則 2: `^localhost(:数字ポート)?(/パス)?$`。
///
/// ポートが数字でない(`localhost:abc`)場合は false を返し、規則 3 の
/// スキーム判定へ委ねる(設計書 §11: 一貫性を優先し特別扱いしない)。
fn is_localhost(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("localhost") else {
        return false;
    };
    // `:数字ポート` があれば消費する(数字が 1 桁もなければ localhost ではない)。
    let rest = match rest.strip_prefix(':') {
        Some(after_colon) => {
            let digits = after_colon.len()
                - after_colon
                    .trim_start_matches(|c: char| c.is_ascii_digit())
                    .len();
            if digits == 0 {
                return false;
            }
            &after_colon[digits..]
        }
        None => rest,
    };
    // 残りは空、またはパス(`/...`)のみ許す。
    rest.is_empty() || rest.starts_with('/')
}

/// 規則 3: `^[a-zA-Z][a-zA-Z0-9+-]*:` にマッチするか(スキームあり)。
///
/// 設計書 §11 の正規表現はスキーム部にドット `.` を許すが、それだと
/// `example.com:8443`(ポート付きホスト、P-23)までスキーム扱いになってしまう。
/// 実運用ではホスト名として `https://` 補完したいので、ここではスキーム部から
/// ドットを除外する。`localhost:abc`(ドットなし、P-13)はスキーム扱いのまま。
fn has_scheme(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    for c in chars {
        if c == ':' {
            return true;
        }
        if !(c.is_ascii_alphanumeric() || c == '+' || c == '-') {
            return false;
        }
    }
    false
}

/// 規則 4: 空白を含まず `.` を含むホスト名形式か。
fn looks_like_host(s: &str) -> bool {
    !s.chars().any(char::is_whitespace) && s.contains('.')
}

/// 予約されていない文字(`A-Za-z0-9-._~`)以外を UTF-8 バイト単位で
/// `%XX`(大文字 hex)にエンコードする。空白は `%20` になる。
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        Command, app_subdir, format_load_progress, initial_uri, parse_command, parse_open_input,
    };
    use std::path::{Path, PathBuf};

    // --- 1.7 parse_command(command モードのコマンドディスパッチ、§11)---

    #[test]
    fn cmd01_quit() {
        assert_eq!(parse_command(":quit"), Command::Quit);
    }

    #[test]
    fn cmd02_open_host_is_completed() {
        // 引数を parse_open_input で補完済み URL に解決する(生 `example.com` ではない)。
        assert_eq!(
            parse_command(":open example.com"),
            Command::Open("https://example.com".to_string())
        );
    }

    #[test]
    fn cmd03_open_without_arg_is_usage_error() {
        // parse_open_input が None(空引数)→ 使用法エラー(文字列を厳密に固定)。
        assert_eq!(
            parse_command(":open"),
            Command::Error("usage: open <url or query>".to_string())
        );
    }

    #[test]
    fn cmd04_unknown_command_is_error() {
        assert_eq!(
            parse_command(":foo"),
            Command::Error("unknown command: foo".to_string())
        );
    }

    #[test]
    fn cmd05_colon_only_is_noop() {
        assert_eq!(parse_command(":"), Command::Noop);
    }

    #[test]
    fn cmd06_empty_is_noop() {
        // Entry が空にクリアされた場合の堅牢性。
        assert_eq!(parse_command(""), Command::Noop);
    }

    #[test]
    fn cmd07_without_leading_colon() {
        // strip_prefix(':') の非該当分岐(先頭が `:` でない)。
        assert_eq!(parse_command("quit"), Command::Quit);
    }

    #[test]
    fn cmd08_open_search_query_with_space() {
        // 引数が空白を含んでも rest として parse_open_input へ渡る(検索へ)。
        assert_eq!(
            parse_command(":open hello world"),
            Command::Open("https://duckduckgo.com/?q=hello%20world".to_string())
        );
    }

    #[test]
    fn cmd09_command_name_is_case_sensitive() {
        // コマンド名は大文字小文字を区別する(`:OPEN` は未知コマンド)。
        assert_eq!(
            parse_command(":OPEN example.com"),
            Command::Error("unknown command: OPEN".to_string())
        );
    }

    #[test]
    fn cmd10_quit_ignores_trailing_args() {
        assert_eq!(parse_command(":quit now"), Command::Quit);
    }

    #[test]
    fn cmd11_surrounding_whitespace_is_trimmed() {
        // 先頭 `:` 除去後に trim してから分割する(trim を外すと name が "" になり落ちる)。
        assert_eq!(parse_command(":  quit  "), Command::Quit);
    }

    // --- 規則 1: 前処理(trim・空入力)---

    #[test]
    fn p34_empty_string_is_none() {
        assert_eq!(parse_open_input(""), None);
    }

    #[test]
    fn p36_whitespace_only_is_none() {
        assert_eq!(parse_open_input("   "), None);
    }

    #[test]
    fn p35_surrounding_whitespace_is_trimmed() {
        assert_eq!(
            parse_open_input("  example.com  "),
            Some("https://example.com".to_string())
        );
    }

    // --- 規則 2: localhost ---

    #[test]
    fn p10_localhost() {
        assert_eq!(
            parse_open_input("localhost"),
            Some("http://localhost".to_string())
        );
    }

    #[test]
    fn p11_localhost_with_port() {
        assert_eq!(
            parse_open_input("localhost:8080"),
            Some("http://localhost:8080".to_string())
        );
    }

    #[test]
    fn p12_localhost_port_and_path() {
        assert_eq!(
            parse_open_input("localhost:8080/path"),
            Some("http://localhost:8080/path".to_string())
        );
    }

    #[test]
    fn p13_localhost_non_numeric_port_is_scheme() {
        // ポートが数字でない → 規則 2 の対象外 → 規則 3(スキーム)としてそのまま
        assert_eq!(
            parse_open_input("localhost:abc"),
            Some("localhost:abc".to_string())
        );
    }

    #[test]
    fn p14_localhost_subdomain_is_https() {
        // localhost.example.com は規則 2 の対象外 → 規則 4
        assert_eq!(
            parse_open_input("localhost.example.com"),
            Some("https://localhost.example.com".to_string())
        );
    }

    #[test]
    fn p15_localhost_path_no_port() {
        assert_eq!(
            parse_open_input("localhost/path"),
            Some("http://localhost/path".to_string())
        );
    }

    // --- 規則 3: スキームあり ---

    #[test]
    fn p01_https_scheme() {
        assert_eq!(
            parse_open_input("https://example.com"),
            Some("https://example.com".to_string())
        );
    }

    #[test]
    fn p02_http_scheme() {
        assert_eq!(
            parse_open_input("http://example.com"),
            Some("http://example.com".to_string())
        );
    }

    #[test]
    fn p03_file_scheme() {
        assert_eq!(
            parse_open_input("file:///home/user/index.html"),
            Some("file:///home/user/index.html".to_string())
        );
    }

    #[test]
    fn p04_about_scheme() {
        assert_eq!(
            parse_open_input("about:blank"),
            Some("about:blank".to_string())
        );
    }

    #[test]
    fn p05_scheme_with_plus() {
        // 正規表現 [a-zA-Z0-9+.-]* の + の確認
        assert_eq!(
            parse_open_input("git+ssh://host/repo"),
            Some("git+ssh://host/repo".to_string())
        );
    }

    #[test]
    fn p06_uppercase_scheme() {
        // スキーム判定は大文字小文字を区別しない(RFC 3986)
        assert_eq!(
            parse_open_input("HTTPS://example.com"),
            Some("HTTPS://example.com".to_string())
        );
    }

    #[test]
    fn p07_leading_digit_before_colon_is_not_scheme() {
        // 規則 3 のスキームは英字始まり(§11: `^[a-zA-Z]...`)。先頭が数字なら ':' を
        // 含んでもスキーム扱いしない(has_scheme の「英字始まり」判定を固定する)。`.` を
        // 含まないため規則 5(検索)へ落ちる。P-25 は数字始まりでも `:` 無しのため、この
        // ':' 前の英字始まり判定は別途固定する必要がある(cargo-mutants で顕在化)。
        assert_eq!(
            parse_open_input("1:2"),
            Some("https://duckduckgo.com/?q=1%3A2".to_string())
        );
    }

    // --- 規則 4: ホスト名形式(https:// 補完)---

    #[test]
    fn p20_bare_host() {
        assert_eq!(
            parse_open_input("example.com"),
            Some("https://example.com".to_string())
        );
    }

    #[test]
    fn p21_multi_label_host() {
        assert_eq!(
            parse_open_input("sub.domain.co.jp"),
            Some("https://sub.domain.co.jp".to_string())
        );
    }

    #[test]
    fn p22_host_with_path_and_query() {
        assert_eq!(
            parse_open_input("example.com/path?q=1"),
            Some("https://example.com/path?q=1".to_string())
        );
    }

    #[test]
    fn p23_host_with_port() {
        // ポート付きホスト。example.com: はスキーム正規表現にも見えるが、
        // スキーム部にドットを含むためスキーム扱いしない → https:// 補完
        assert_eq!(
            parse_open_input("example.com:8443"),
            Some("https://example.com:8443".to_string())
        );
    }

    #[test]
    fn p24_whitespace_falls_through_to_search() {
        // 空白を含む → 規則 4 の対象外 → 規則 5(検索)
        assert_eq!(
            parse_open_input("foo bar.com"),
            Some("https://duckduckgo.com/?q=foo%20bar.com".to_string())
        );
    }

    #[test]
    fn p25_numeric_leading_host_is_not_scheme() {
        // 先頭が英字でない(IP 等)→ スキーム判定は false → 規則 4 で https 補完
        assert_eq!(
            parse_open_input("8.8.8.8"),
            Some("https://8.8.8.8".to_string())
        );
    }

    // --- 規則 5: DuckDuckGo 検索 ---

    #[test]
    fn p30_single_word_search() {
        assert_eq!(
            parse_open_input("hello"),
            Some("https://duckduckgo.com/?q=hello".to_string())
        );
    }

    #[test]
    fn p31_space_is_percent_encoded() {
        let result = parse_open_input("hello world").unwrap();
        assert!(result.starts_with("https://duckduckgo.com/?q="));
        assert!(result.contains("hello%20world"), "got: {result}");
    }

    #[test]
    fn p32_non_ascii_is_percent_encoded() {
        let result = parse_open_input("rust 所有権").unwrap();
        assert!(result.starts_with("https://duckduckgo.com/?q="));
        // 非 ASCII が percent-encoding される(生のマルチバイト文字が残らない)
        assert!(!result.contains('所'), "got: {result}");
        assert!(result.contains('%'), "got: {result}");
    }

    #[test]
    fn p33_url_special_chars_are_encoded() {
        let result = parse_open_input("a&b=c?d").unwrap();
        assert!(result.starts_with("https://duckduckgo.com/?q="));
        // & = ? がエンコードされ、クエリが壊れない
        let query = result.strip_prefix("https://duckduckgo.com/?q=").unwrap();
        assert!(!query.contains('&'), "got: {result}");
        assert!(!query.contains('='), "got: {result}");
        assert!(!query.contains('?'), "got: {result}");
    }

    // --- 規則の優先順位 ---

    #[test]
    fn p40_scheme_beats_host_completion() {
        // 規則 3 と 4 の両方に該当 → 規則 3 が勝つ(二重補完しない)
        assert_eq!(
            parse_open_input("http://example.com"),
            Some("http://example.com".to_string())
        );
    }

    #[test]
    fn p41_localhost_beats_search() {
        assert_eq!(
            parse_open_input("localhost"),
            Some("http://localhost".to_string())
        );
    }

    #[test]
    fn p42_localhost_beats_scheme() {
        // localhost:8080 はスキームにも見えるが規則 2 が勝つ(素通ししない)
        assert_eq!(
            parse_open_input("localhost:8080"),
            Some("http://localhost:8080".to_string())
        );
    }

    // --- §13-3: 起動引数からの初期 URL 決定 ---

    #[test]
    fn s13_first_arg_is_initial_uri() {
        // 第 1 引数があればそれが初期 URL。M1 は生 URL を素通し
        // (補完規則 §11 の適用は M4。todo サイクル 3 冒頭)。
        assert_eq!(
            initial_uri(Some("https://example.com")),
            "https://example.com"
        );
    }

    #[test]
    fn s13_no_arg_is_about_blank() {
        // 引数が無ければ about:blank。
        assert_eq!(initial_uri(None), "about:blank");
    }

    // --- §8.2: NetworkSession の data/cache ディレクトリ算出 ---

    // --- §12: ステータスバーの読み込み状態表示 ---

    #[test]
    fn s12_not_loading_is_empty() {
        // §12: 読み込み中のみ表示。非読み込み時は progress 値に依らず空。
        assert_eq!(format_load_progress(false, 0.5), "");
    }

    #[test]
    fn s12_loading_formats_percent() {
        // 0.42 → [42%](四捨五入)。
        assert_eq!(format_load_progress(true, 0.42), "[42%]");
    }

    #[test]
    fn s12_loading_zero_percent() {
        assert_eq!(format_load_progress(true, 0.0), "[0%]");
    }

    #[test]
    fn s12_loading_full() {
        assert_eq!(format_load_progress(true, 1.0), "[100%]");
    }

    #[test]
    fn s12_loading_rounds_half_up() {
        // §12 doc の「四捨五入」を固定する。truncate 実装なら 99% になり落ちる値を選ぶ
        // (CLAUDE.md 規約 4: coverage は分岐網羅であって挙動網羅ではない)。
        // 0.425 は f64 で 42.4999… のため四捨五入でも 42% になり probe に使えない点に注意。
        assert_eq!(format_load_progress(true, 0.999), "[100%]");
    }

    #[test]
    fn s82_app_subdir_appends_owl_to_xdg_base() {
        // §8.2: data=$XDG_DATA_HOME/owl・cache=$XDG_CACHE_HOME/owl。いずれも
        // XDG ベースディレクトリ直下に "owl" を付与する(data/cache とも同じ規則)。
        assert_eq!(
            app_subdir(Path::new("/home/u/.local/share")),
            PathBuf::from("/home/u/.local/share/owl")
        );
        assert_eq!(
            app_subdir(Path::new("/home/u/.cache")),
            PathBuf::from("/home/u/.cache/owl")
        );
    }
}
