use crate::controller::{error::Error, xml::part_tag};
use htmd::HtmlToMarkdown;

pub fn convert_html(html: &str) -> Result<String, Error> {
    let converter = HtmlToMarkdown::builder()
        .skip_tags(vec!["head", "img", "image"])
        .build();
    Ok(converter.convert(html)?)
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
    messages.push(msg);

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
