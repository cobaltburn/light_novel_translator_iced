use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
    path::Path,
    sync::LazyLock,
};

use crate::{
    controller::{insert_image_markers, part_tag, strip_syosetu_tags, strip_tags},
    error::Result,
};
use html2md::rewrite_html;
use pulldown_cmark::{Parser, Tag, TagEnd};
use quick_xml::{Writer, events::BytesText};
use rbook::epub::toc::EpubToc;
use regex::Regex;

pub fn image_marker(n: usize) -> String {
    format!("[[IMG:{n}]]")
}

pub static IMAGE_MARKER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\?\[\\?\[\s*IMG\s*:\s*(\d+)\s*\\?\]\\?\]").unwrap());
pub fn has_translatable_text(text: &str) -> bool {
    !markdown_sections(&IMAGE_MARKER_RE.replace_all(text, "")).is_empty()
}

pub fn image_marker_indices(content: &str) -> HashSet<usize> {
    IMAGE_MARKER_RE
        .captures_iter(content)
        .filter_map(|caps| caps[1].parse().ok())
        .collect()
}

/// Markdown without images, used to locate positions in the source text.
pub fn html_to_markdown(html: &str) -> Result<String> {
    let html = strip_syosetu_tags(html)?;
    markdown_from_html(&html)
}

/// Markdown with images replaced by markers, used as the source text for translation.
pub fn html_to_marked_markdown(html: &str) -> Result<String> {
    let html = strip_syosetu_tags(html)?;
    let html = insert_image_markers(&html)?;
    markdown_from_html(&html)
}

fn markdown_from_html(html: &str) -> Result<String> {
    let html = strip_tags(html)?;
    let markdown = rewrite_html(&html, false);
    let markdown: Vec<_> = markdown.lines().map(|s| s.trim()).collect();
    Ok(markdown.join("\n"))
}

pub fn toc_to_markdown(toc: EpubToc<'_>) -> Result<String> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);
    if let Some(toc) = toc.contents() {
        for entry in toc.flatten() {
            let href = entry.href().map(|e| e.as_str()).unwrap_or_default();
            writer
                .create_element("a")
                .with_attribute(("href", href))
                .write_text_content(BytesText::new(entry.label()))?;
        }
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
