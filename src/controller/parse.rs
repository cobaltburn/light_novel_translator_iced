use crate::controller::part_tag;
use regex::Regex;

pub fn remove_think_tags(text: &str) -> String {
    let rg = Regex::new(r"(?s)<think>.*?</think>\s*").unwrap();
    rg.replace_all(text, "").to_string()
}

const PARTITION_SIZE: usize = 8000;

pub fn partition_text(text: &str) -> Vec<String> {
    let mut messages = Vec::with_capacity(10);
    let mut msg = String::with_capacity(PARTITION_SIZE + 500);

    for sentence in text.lines() {
        if msg.len() < PARTITION_SIZE {
            msg.push_str(&format!("{sentence}\n"));
        } else {
            messages.push(msg.trim().to_string());
            msg.clear();
        }
    }

    let msg = msg.trim();
    if !msg.is_empty() {
        messages.push(msg.to_string());
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
