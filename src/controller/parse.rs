use crate::{
    controller::{part_tag, strip_syosetu_tags, strip_tags},
    error::Result,
};
use html2md::rewrite_html;
use regex::Regex;

pub fn html_to_markdown(html: &str) -> Result<String> {
    let html = strip_syosetu_tags(html)?;
    let html = strip_tags(&html)?;
    let markdown = rewrite_html(&html, false);
    let markdown: Vec<_> = markdown.lines().map(|s| s.trim()).collect();
    Ok(markdown.join("\n"))
}

pub fn is_empty_section(section: &str) -> bool {
    section.trim_matches('#').is_empty()
}

pub fn remove_think_tags(text: &str) -> String {
    let rg = Regex::new(r"(?s)<think>.*?</think>\s*").unwrap();
    rg.replace_all(text, "").to_string()
}

const PARTITION_SIZE: usize = 8000;

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
