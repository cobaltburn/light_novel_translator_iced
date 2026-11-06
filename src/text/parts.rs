//! A page's translation is stored as text split up by `<part>` tags.

use regex::Regex;
use std::sync::LazyLock;

const PARTITION_SIZE: usize = 8000;

static PART_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<part>.*?</part>").unwrap());
static NUMBERED_PART_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<part>\d+</part>").unwrap());

pub fn part_tag(n: usize) -> String {
    format!("\n\n<part>{}</part>\n\n", n)
}

pub fn partition_text(text: &str) -> Vec<String> {
    text.lines()
        .fold(Vec::new(), |mut msgs: Vec<String>, line| {
            for sentence in line.split_inclusive('。') {
                if let Some(msg) = msgs.last_mut()
                    && msg.len() < PARTITION_SIZE
                {
                    msg.push_str(sentence);
                } else {
                    msgs.push(sentence.to_string());
                }
            }

            if let Some(msg) = msgs.last_mut() {
                msg.push('\n');
            }

            msgs
        })
}

pub fn markdown_sections(markdown: &str) -> Vec<String> {
    partition_text(markdown)
        .into_iter()
        .filter(|e| !is_empty_section(e))
        .collect()
}

pub fn is_empty_section(section: &str) -> bool {
    section.trim().trim_matches('#').is_empty()
}

pub fn join_partition(parts: &[String]) -> String {
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

/// The translated sections of a page, indexed the same way as `AnchorPosition::section`.
pub fn content_sections(content: &str) -> impl Iterator<Item = &str> {
    PART_RE.split(content).filter(|e| !e.trim().is_empty())
}

/// The translated parts of a saved page, in order.
pub fn split_parts(text: &str) -> Vec<String> {
    NUMBERED_PART_RE
        .split(text)
        .filter(|e| !e.is_empty())
        .map(str::to_owned)
        .collect()
}
