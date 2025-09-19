use regex::Regex;

pub fn part_tag(n: usize) -> String {
    format!("<part>{}</part>", n)
}

pub fn remove_think_tags(text: &str) -> String {
    let rg = Regex::new(r"(?s)<think>.*?</think>\s*").unwrap();
    rg.replace_all(text, "").to_string()
}
