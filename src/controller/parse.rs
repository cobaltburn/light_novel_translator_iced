use crate::controller::part_tag;
use regex::Regex;

pub fn remove_think_tags(text: &str) -> String {
    let rg = Regex::new(r"(?s)<think>.*?</think>\s*").unwrap();
    rg.replace_all(text, "").to_string()
}

pub fn partition_text(text: &str) -> Vec<String> {
    let mut messages = Vec::new();
    let mut msg = String::new();
    let sentences = text.split_inclusive("。").map(str::to_owned);

    for sentence in sentences {
        if msg.len() < 2000 {
            msg.push_str(&sentence);
        } else {
            messages.push(msg);
            msg = String::new();
        }
    }
    if !msg.is_empty() {
        messages.push(msg);
    }

    messages
}

pub fn join_partition(parts: Vec<String>) -> String {
    parts
        .into_iter()
        .enumerate()
        .map(|(n, part)| {
            let tag = part_tag(n + 1);
            format!("{}\n\n{}", tag, part)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
