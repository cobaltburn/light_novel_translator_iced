use crate::{
    epub::html::{
        images::insert_image_markers,
        strip::{strip_syosetu_tags, strip_tags},
    },
    error::Result,
};
use html2md::rewrite_html;
use pulldown_cmark::{Options, Parser, Tag, TagEnd, html::push_html};
use quick_xml::{Writer, events::BytesText};
use rbook::epub::toc::EpubToc;
use std::{collections::HashMap, io::Cursor, path::Path};

pub fn to_html(markdown: &str) -> String {
    let options =
        Options::ENABLE_SMART_PUNCTUATION | Options::ENABLE_TABLES | Options::ENABLE_FOOTNOTES;
    let parser = Parser::new_ext(markdown, options);
    let mut html = String::with_capacity(markdown.len());
    push_html(&mut html, parser);
    html
}

pub fn html_to_markdown(html: &str) -> Result<String> {
    let html = strip_syosetu_tags(html)?;
    markdown_from_html(&html)
}

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

#[cfg(test)]
mod test {
    use super::to_html;

    #[test]
    fn ignores_tildes() {
        let html = to_html("Heeey~ ~wait~ for me~~ Nooo~~");
        assert!(!html.contains("<sub>"));
        assert!(!html.contains("<del>"));
    }

    #[test]
    fn ignores_caret_superscript() {
        assert!(!to_html("It costs 2^10^ gold").contains("<sup>"));
    }

    #[test]
    fn ignores_dollar_math() {
        assert!(!to_html("It was $5 or $10").contains("math"));
    }

    #[test]
    fn keeps_smart_quotes() {
        assert!(to_html("\"Hello\"").contains("“Hello”"));
    }
}
