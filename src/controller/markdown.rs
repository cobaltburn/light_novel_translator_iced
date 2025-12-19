use crate::{controller::xml::part_tag, error::Result};
use htmd::HtmlToMarkdown;
use pulldown_cmark::{Event, Options, Parser, Tag};
use std::{collections::HashMap, path::PathBuf};

pub fn convert_html(html: &str) -> Result<String> {
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

// TODO think about this
pub fn parse_anchors(markdown: &str) -> HashMap<String, String> {
    let mut parser = Parser::new_ext(markdown, Options::all());
    let mut links = HashMap::new();

    while let Some(event) = parser.next() {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => {
                if let Some(Event::Text(title)) = parser.next() {
                    let path = to_name(&PathBuf::from(&*dest_url)).unwrap();
                    let path = path.split("#").collect::<Vec<_>>();
                    let path = path.first().unwrap().to_string();
                    let title = title.to_string();
                    links.insert(path, title);
                }
            }
            _ => (),
        }
    }
    links
}

fn to_name(path: &PathBuf) -> Option<String> {
    Some(path.file_name()?.to_string_lossy().to_string())
}
