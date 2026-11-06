use regex::Regex;

pub fn clean_invisible_chars(text: &str) -> String {
    text.chars()
        .filter(|&c| {
            // Keep normal whitespace
            if c == ' ' || c == '\n' || c == '\r' || c == '\t' {
                return true;
            }

            // Remove zero-width and invisible characters
            if matches!(c,
                '\u{200B}'..='\u{200F}' |
                '\u{2060}'..='\u{2064}' |
                '\u{FEFF}' |
                '\u{00AD}' |
                '\u{FFA0}'
            ) {
                return false;
            }

            // Remove other control characters
            !c.is_control()
        })
        .collect()
}

pub fn remove_think_tags(text: &str) -> String {
    let rg = Regex::new(r"(?s)<think>.*?</think>\s*").unwrap();
    rg.replace_all(text, "").to_string()
}
