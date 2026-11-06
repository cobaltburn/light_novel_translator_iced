use regex::{Captures, Regex};
use std::sync::LazyLock;

pub fn contains_japanese(text: &str) -> bool {
    text.chars().any(is_japanese_char)
}

pub fn is_japanese_char(ch: char) -> bool {
    ch.is_alphabetic()
        && matches!(ch,
            '\u{3040}'..='\u{309F}' |  // Hiragana
            '\u{30A0}'..='\u{30FF}' |  // Katakana
            '\u{4E00}'..='\u{9FFF}' |  // CJK Unified Ideographs (common kanji)
            '\u{3400}'..='\u{4DBF}' |  // CJK Unified Ideographs Extension A
            '\u{FF65}'..='\u{FF9F}' |  // Half-width Katakana
            '\u{31F0}'..='\u{31FF}'    // Katakana Phonetic Extensions
        )
}

/// Full-width spaces at the start of a line are Japanese paragraph indents.
static INDENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\u{3000}+").unwrap());
static DASH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[―—─━]+").unwrap());
static ELLIPSIS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"…+|・{2,}").unwrap());
static COMMA_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"、[ \u{3000}]?").unwrap());
/// Longer dash runs are scene separators rather than em dashes.
const MAX_DASH_RUN: usize = 3;

pub fn replace_jp_symbols(text: &str) -> String {
    let text = INDENT_RE.replace_all(text, "");
    let text = DASH_RE.replace_all(&text, |caps: &Captures| match caps[0].chars().count() {
        n if n <= MAX_DASH_RUN => "—".to_string(),
        _ => caps[0].to_string(),
    });
    let text = ELLIPSIS_RE.replace_all(&text, "…");
    let text = COMMA_RE.replace_all(&text, ", ");

    text.chars()
        .map(|c| match c {
            '「' | '」' => '"',
            '『' | '』' => '\'',
            '\u{3000}' => ' ',
            '。' => '.',
            '〜' => '~',
            '！'
            | '？'
            | '（'
            | '）'
            | '，'
            | '．'
            | '：'
            | '；'
            | '～'
            | '０'..='９'
            | 'Ａ'..='Ｚ'
            | 'ａ'..='ｚ' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod test {
    use super::replace_jp_symbols;

    #[test]
    fn converts_quotes() {
        assert_eq!(
            replace_jp_symbols("「She said 『no』」"),
            "\"She said 'no'\""
        );
    }

    #[test]
    fn converts_dashes_and_ellipses() {
        assert_eq!(replace_jp_symbols("Wait――no……"), "Wait—no…");
        assert_eq!(replace_jp_symbols("Well・・・"), "Well…");
        // Long runs are separators and stay as they are
        assert_eq!(replace_jp_symbols("――――――"), "――――――");
    }

    #[test]
    fn converts_full_width_punctuation() {
        assert_eq!(
            replace_jp_symbols("\u{3000}What！？ Level ５０、ok。"),
            "What!? Level 50, ok."
        );
        assert_eq!(replace_jp_symbols("Hey～ 〜"), "Hey~ ~");
    }

    #[test]
    fn keeps_markdown_and_html_characters() {
        assert_eq!(replace_jp_symbols("＊＃＜＞＆"), "＊＃＜＞＆");
    }
}
