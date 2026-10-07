use std::{
    borrow::Cow,
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
use quick_xml::{Reader, Writer, XmlVersion, events::BytesText};
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
            let href = entry.href().map_or_default(|e| e.as_str());
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

pub fn parse_headers(html: &str) -> Result<HashMap<String, Vec<(String, String)>>> {
    use quick_xml::events::Event;
    let mut reader = Reader::from_str(html);
    let mut titles: Vec<_> = vec![(String::new(), Vec::new())];
    let h_tags: &[&[u8]] = &[b"h1", b"h2", b"h3", b"h4"];

    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"div" => {
                if let Some(id) = tag.try_get_attribute("id")? {
                    let id = id.normalized_value(XmlVersion::Implicit1_0)?;
                    titles.push((id.into_owned(), Vec::new()));
                }
            }
            Event::Start(tag) if h_tags.contains(&tag.name().as_ref()) => {
                let title = extract_text(reader.read_text(tag.name())?.into_inner())?;
                let id = tag.try_get_attribute("id")?;
                if let Some((_, titles)) = titles.last_mut()
                    && let Some(id) = id
                {
                    let id = id.normalized_value(XmlVersion::Implicit1_0)?.to_string();
                    titles.push((id, title));
                }
            }
            Event::Eof => break,
            _ => (),
        }
    }
    Ok(titles.into_iter().collect())
}

fn extract_text(html: Cow<'_, [u8]>) -> Result<String> {
    use quick_xml::events::Event;
    let mut reader = Reader::from_reader(html.as_ref());
    let mut title = String::new();
    loop {
        match reader.read_event()? {
            Event::Text(text) => {
                let text = text.html_content()?;
                title.push_str(text.as_ref());
            }
            Event::Eof => break,
            _ => (),
        }
    }
    Ok(title)
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
