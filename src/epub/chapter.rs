//! Assembles the xhtml of one translated chapter.

use crate::{
    epub::{
        anchors::{insert_anchors, replace_image_markers},
        html::{
            ANCHOR_TAG, BR, HEAD_TAG, PARAGRAPH_TAG, image_anchors, is_header, text_content,
            to_html, update_resource_tag,
        },
        placement::{image_positions, place_ids},
        resources::PageLinks,
    },
    error::{Error, Result},
    text::{image_marker_indices, replace_jp_symbols},
};
use quick_xml::{
    Reader, Writer, XmlVersion,
    events::{BytesDecl, BytesStart, Event},
};
use std::{
    borrow::Cow,
    collections::HashMap,
    io::{self, Cursor},
};

pub fn build_html(
    html: &str,
    content: &str,
    toc_ids: &[String],
    titles: &HashMap<String, String>,
    links: PageLinks<'_>,
    page: usize,
) -> Result<String> {
    let found = image_marker_indices(content);
    let (marked, estimated): (Vec<_>, Vec<_>) = image_anchors(html, links)?
        .into_iter()
        .enumerate()
        .partition(|(i, _)| found.contains(i));

    let mut anchors = place_ids(html, content, toc_ids, titles)?;
    let estimated = estimated.into_iter().map(|(_, image)| image).collect();
    anchors.extend(image_positions(html, estimated)?);
    let content = insert_anchors(content, anchors)?;

    let marked = marked.into_iter().map(|(i, (tag, _))| (i, tag)).collect();
    let content = replace_image_markers(&content, marked)?;
    let content = replace_jp_symbols(&content);
    let content = to_html(&content);

    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;

    writer
        .create_element("html")
        .with_attribute(("xmlns", "http://www.w3.org/1999/xhtml"))
        .with_attribute(("xmlns:epub", "http://www.idpf.org/2007/ops"))
        .with_attribute(("xml:lang", "en"))
        .write_inner_content(|writer| {
            write_header(writer, html, links)?;
            write_body(writer, &content, page)
        })?;

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

pub fn write_header(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    html: &str,
    links: PageLinks<'_>,
) -> io::Result<()> {
    let head = extract_head(html).map_err(io::Error::other)?;

    writer
        .create_element("head")
        .write_inner_content(|writer| write_head(writer, head, links).map_err(io::Error::other))?;

    Ok(())
}

pub fn extract_head(html: &str) -> Result<Cow<'_, str>> {
    let mut reader = Reader::from_str(html);
    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == HEAD_TAG => {
                return Ok(reader.read_text(tag.name())?.html_content());
            }
            Event::Eof => return Err(Error::BuildError("No head tag found")),
            _ => (),
        }
    }
}

pub fn write_head(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    head: Cow<'_, str>,
    links: PageLinks<'_>,
) -> Result<()> {
    let mut reader = Reader::from_str(&head);
    reader.config_mut().trim_text(true);

    loop {
        match reader.read_event()? {
            Event::Eof => break,
            e => writer.write_event(update_resource_tag(e, links)?)?,
        }
    }

    writer
        .create_element("link")
        .with_attribute(("rel", "stylesheet"))
        .with_attribute(("type", "text/css"))
        .with_attribute(("href", "../stylesheet.css"))
        .write_empty()?;
    Ok(())
}

pub fn write_body(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    content: &str,
    page: usize,
) -> io::Result<()> {
    let mut reader = Reader::from_str(content);

    writer
        .create_element("body")
        .write_inner_content(|writer| {
            let mut count = 0;
            let mut after_link = false;
            // A paragraph start is held back until the next event, so a leading id anchor
            // can be moved onto the paragraph
            let mut paragraph: Option<BytesStart<'_>> = None;
            loop {
                let event = reader.read_event().map_err(io::Error::other)?;
                if let Some(p) = paragraph.take() {
                    if let Event::Empty(tag) = &event
                        && let Some(id) = anchor_id(tag)
                    {
                        writer
                            .write_event(Event::Start(p.with_attributes([("id", id.as_str())])))?;
                        continue;
                    }
                    writer.write_event(Event::Start(p))?;
                }

                match event {
                    Event::Start(tag) if tag.name().as_ref() == PARAGRAPH_TAG => {
                        paragraph = Some(tag);
                    }
                    Event::Start(tag) if tag.name().as_ref() == ANCHOR_TAG => {
                        if after_link {
                            writer.write_event(Event::Empty(BytesStart::new(BR)))?;
                        }
                        writer.write_event(Event::Start(tag))?;
                    }
                    Event::End(tag) if tag.name().as_ref() == ANCHOR_TAG => {
                        writer.write_event(Event::End(tag))?;
                        after_link = true;
                        continue;
                    }
                    Event::Text(text) if text.chars().all(|c| c.is_ascii_whitespace()) => {
                        writer.write_event(Event::Text(text))?;
                        continue;
                    }
                    Event::Start(tag) if is_header(tag.name()) => {
                        count += 1;
                        let id = format!("h{:03}-{:03}", page, count);
                        let tag = tag.with_attributes([("id", id.as_str())]);
                        writer.write_event(Event::Start(tag))?;
                    }
                    Event::Eof => break,
                    e => writer.write_event(e)?,
                }
                after_link = false;
            }
            Ok(())
        })?;
    Ok(())
}

/// The id of an `<a>` that has nothing but an id, as written by `insert_anchors`.
fn anchor_id(tag: &BytesStart<'_>) -> Option<String> {
    if tag.name().as_ref() != ANCHOR_TAG {
        return None;
    }
    let mut attributes = tag.attributes().flatten();
    let id = attributes.next().filter(|a| a.key.as_ref() == "id")?;
    if attributes.next().is_some() {
        return None;
    }
    Some(
        id.normalized_value(XmlVersion::Implicit1_0)
            .ok()?
            .into_owned(),
    )
}

pub fn parse_headers(html: &str) -> Result<HashMap<String, Vec<(String, String)>>> {
    use quick_xml::events::Event;
    let mut reader = Reader::from_str(html);
    let mut titles: Vec<_> = vec![(String::new(), Vec::new())];

    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == "div" => {
                if let Some(id) = tag.try_get_attribute("id")? {
                    let id = id.normalized_value(XmlVersion::Implicit1_0)?;
                    titles.push((id.into_owned(), Vec::new()));
                }
            }
            Event::Start(tag) if is_header(tag.name()) => {
                let title = text_content(&reader.read_text(tag.name())?.into_inner())?;
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
    titles
        .iter_mut()
        .for_each(|(_, e)| e.retain(|e| !e.1.trim().is_empty()));

    Ok(titles.into_iter().collect())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::epub::anchors::{Anchor, AnchorPosition};

    fn body(content: &str) -> String {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        write_body(&mut writer, content, 0).unwrap();
        String::from_utf8(writer.into_inner().into_inner()).unwrap()
    }

    #[test]
    fn keeps_spaces_around_inline_tags() {
        let html = body("<p>He said <em>no</em> and left</p>");
        assert!(html.starts_with("<body><p>"));
        assert!(html.contains("<p>He said <em>no</em> and left</p>"));
    }

    #[test]
    fn keeps_inline_links_inline() {
        let html = body("<p>See <a href=\"a.xhtml\">this</a> page</p>");
        assert!(html.contains("<p>See <a href=\"a.xhtml\">this</a> page</p>"));
        assert!(!html.contains("<div"));
    }

    #[test]
    fn splits_consecutive_links() {
        let html = body("<p><a href=\"1.xhtml\">One</a>\n<a href=\"2.xhtml\">Two</a></p>");
        assert!(html.contains("</a>\n<br/><a href=\"2.xhtml\">"));
        assert!(!html.contains("<div"));
    }

    #[test]
    fn keeps_links_separated_by_text_inline() {
        let html = body("<p><a href=\"1.xhtml\">One</a> and <a href=\"2.xhtml\">Two</a></p>");
        assert!(html.contains("</a> and <a href=\"2.xhtml\">"));
        assert!(!html.contains("<br/>"));
    }

    #[test]
    fn puts_ids_on_their_paragraph() {
        let position = |id: &str, line: usize| AnchorPosition {
            anchor: Anchor::Id(id.to_string()),
            section: 0,
            line,
            position: line as f64 / 7.0,
        };
        let content = "<part>1</part>\n\nText.\n\nSide Story: Title\n\n## Heading\n";
        let anchors = vec![position("c1", 4), position("c2", 6)];
        let markdown = insert_anchors(content, anchors).unwrap();
        assert!(markdown.contains("\n<a id=\"c1\"/>Side Story: Title\n"));
        assert!(markdown.contains("\n## <a id=\"c2\"/>Heading\n"));

        let html = body(&to_html(&markdown));
        assert!(html.contains("<p id=\"c1\">Side Story: Title</p>"));
        assert!(html.contains("<h2 id=\"h000-001\"><a id=\"c2\"/>Heading</h2>"));
    }

    #[test]
    fn carries_ids_to_the_next_text() {
        let position = |id: &str, section: usize, position: f64| AnchorPosition {
            anchor: Anchor::Id(id.to_string()),
            section,
            line: 0,
            position,
        };
        let content = "<part>1</part>\n\nText.\n\nMore.\n<part>2</part>\n\nNext.\n";
        let anchors = vec![
            position("blank", 0, 3.0 / 5.0),
            position("end", 0, 1.0),
            position("missing", 5, 0.0),
        ];
        let markdown = insert_anchors(content, anchors).unwrap();
        assert!(markdown.contains("\n<a id=\"blank\"/>More.\n"));
        assert!(markdown.contains("\n<a id=\"end\"/>Next.\n"));
        assert!(markdown.ends_with("Next.\n<div id=\"missing\"></div>\n\n"));

        let html = body(&to_html(&markdown));
        assert!(html.contains("<p id=\"end\">Next.</p>"));
        assert!(html.contains("<div id=\"missing\"></div>"));
    }

    #[test]
    fn puts_waiting_ids_on_images() {
        let id = |id: &str| AnchorPosition {
            anchor: Anchor::Id(id.to_string()),
            section: 0,
            line: 0,
            position: 0.0,
        };
        let img = BytesStart::new("img").with_attributes([("src", "../Images/1.jpg")]);
        let image = AnchorPosition {
            anchor: Anchor::Image(img),
            section: 0,
            line: 1,
            position: 0.5,
        };
        let markdown = insert_anchors("\nText.\n", vec![id("c1"), id("c2"), image]).unwrap();
        let html = body(&to_html(&markdown));
        assert!(html.contains(
            "<div class=\"illustration\" id=\"c1\"><a id=\"c2\"/><img src=\"../Images/1.jpg\"/></div>"
        ));
        assert!(html.contains("<p>Text.</p>"));
    }
}
