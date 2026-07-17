//! command モード: `:open` の入力解釈。
//!
//! `parse_open_input` は GTK 非依存の純粋関数(設計書 §11)。エラー表示等の
//! 副作用は呼び出し側が担う。実装は後続マイルストーン(M2 以降)で `main` から
//! 結線されるまで未使用のため、モジュール全体に dead_code を許可する。
#![allow(dead_code)]

const DUCKDUCKGO_SEARCH: &str = "https://duckduckgo.com/?q=";
const HTTP_SCHEME: &str = "http://";
const HTTPS_SCHEME: &str = "https://";

/// 起動引数が無いときの初期 URL(設計書 §13-3)。
const BLANK_URI: &str = "about:blank";

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
    use super::{initial_uri, parse_open_input};

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
}
