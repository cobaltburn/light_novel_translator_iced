use crate::controller::part_tag;
use regex::Regex;

pub fn remove_think_tags(text: &str) -> String {
    let rg = Regex::new(r"(?s)<think>.*?</think>\s*").unwrap();
    rg.replace_all(text, "").to_string()
}

pub fn partition_text(text: &str) -> Vec<String> {
    let mut messages = Vec::with_capacity(text.len() / 2000);
    let mut msg = String::with_capacity(2500);
    let sentences = text.split_inclusive("。");

    for sentence in sentences {
        if msg.len() < 2000 {
            msg.push_str(&sentence);
        } else {
            messages.push(msg.trim().to_string());
            msg = String::with_capacity(2500);
        }
    }

    if !msg.trim().is_empty() {
        messages.push(msg.trim().to_string());
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
