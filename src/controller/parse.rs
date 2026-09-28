use std::{collections::HashMap, io::Cursor, path::Path};

use crate::{
    controller::{Nav, part_tag, strip_syosetu_tags, strip_tags},
    error::Result,
};
use html2md::rewrite_html;
use pulldown_cmark::{Parser, Tag, TagEnd};
use quick_xml::{Writer, events::BytesText};
use regex::Regex;

pub fn html_to_markdown(html: &str) -> Result<String> {
    let html = strip_syosetu_tags(html)?;
    let html = strip_tags(&html)?;
    let markdown = rewrite_html(&html, false);
    let markdown: Vec<_> = markdown.lines().map(|s| s.trim()).collect();
    Ok(markdown.join("\n"))
}

pub fn nav_to_markdown(navs: &[Nav]) -> Result<String> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    for Nav { label, path } in navs {
        writer
            .create_element("a")
            .with_attribute(("href", path.to_string_lossy()))
            .write_text_content(BytesText::new(label))?;
    }

    let nav_html = String::from_utf8(writer.into_inner().into_inner())?;
    Ok(rewrite_html(&nav_html, false))
}

pub fn parse_links(content: &str) -> HashMap<String, String> {
    use pulldown_cmark::Event;

    let mut links = HashMap::new();
    let mut link: Option<(String, String)> = None;
    for event in Parser::new(content) {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) => {
                let file_name = Path::new(dest_url.as_ref()).file_name().unwrap_or_default();
                link = Some((file_name.to_string_lossy().into_owned(), String::new()));
            }
            Event::Text(text) => {
                if let Some((_, title)) = &mut link {
                    title.push_str(&text);
                }
            }
            Event::End(TagEnd::Link) => {
                if let Some((url, title)) = link.take()
                    && !title.is_empty()
                {
                    links.insert(url, title);
                }
            }
            _ => (),
        }
    }

    links
}

pub fn is_empty_section(section: &str) -> bool {
    section.trim().trim_matches('#').is_empty()
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

/// Partitions markdown into the non-empty sections that are sent for translation.
/// Anything that maps positions onto translated sections must use this so indices line up.
pub fn markdown_sections(markdown: &str) -> Vec<String> {
    partition_text(markdown)
        .into_iter()
        .filter(|e| !is_empty_section(e))
        .collect()
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
