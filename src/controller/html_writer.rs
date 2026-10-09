use crate::{
    controller::{
        Anchor, AnchorPosition, IMAGE_MARKER_RE, PageLinks, extract_head, update_resource_tag,
    },
    error::Result,
};
use quick_xml::{
    Reader, Writer,
    escape::{escape, partial_escape},
    events::{BytesStart, Event},
};
use regex::Regex;
use std::{
    borrow::Cow,
    collections::{HashMap, VecDeque},
    fmt::Write as _,
    io::{self, Cursor},
    mem,
    sync::LazyLock,
};

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

const ANCHOR_TAG: &[u8] = b"a";
const BR: &str = "br";

pub fn write_body(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    content: &str,
    page: usize,
) -> io::Result<()> {
    let mut reader = Reader::from_str(content);
    let h_tags: &[&[u8]] = &[b"h1", b"h2", b"h3", b"h4"];

    writer
        .create_element("body")
        .write_inner_content(|writer| {
            let mut count = 0;
            let mut after_link = false;
            loop {
                match reader.read_event().map_err(io::Error::other)? {
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
                    Event::Text(text) if text.iter().all(u8::is_ascii_whitespace) => {
                        writer.write_event(Event::Text(text))?;
                        continue;
                    }
                    Event::Start(tag) if h_tags.contains(&tag.name().as_ref()) => {
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

fn write_image_tags(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    image_tags: Vec<BytesStart<'_>>,
) -> Result<()> {
    if image_tags.is_empty() {
        return Ok(());
    }

    writer
        .create_element("div")
        .with_attribute(("class", "illustration"))
        .write_inner_content(|writer| {
            for tag in image_tags {
                writer.write_event(Event::Empty(tag))?;
            }
            Ok(())
        })?;

    Ok(())
}

static PART_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<part>.*?</part>").unwrap());

pub fn insert_anchors(content: &str, anchors: Vec<AnchorPosition>) -> Result<String> {
    let mut section_anchors: HashMap<usize, Vec<AnchorPosition>> = HashMap::new();
    for p in anchors {
        section_anchors.entry(p.section).or_default().push(p);
    }

    let sections = PART_RE
        .split(content)
        .filter(|e| !e.trim().is_empty())
        .map(partial_escape);
    let mut output = String::with_capacity(content.len());

    for (index, section) in sections.enumerate() {
        let Some(mut anchors) = section_anchors.remove(&index) else {
            output.push_str(&section);
            continue;
        };
        anchors.sort_by(|a, b| a.position.total_cmp(&b.position));
        let mut anchors = VecDeque::from(anchors);

        let line_count = section.lines().count();
        for (i, line) in section.lines().enumerate() {
            let position = i as f64 / line_count as f64;
            let mut popped = Vec::new();
            while let Some(p) = anchors.pop_front_if(|p| position >= p.position) {
                popped.push(p);
            }
            write_anchors(&mut output, popped)?;
            output.push_str(line);
            output.push('\n');
        }

        write_anchors(&mut output, anchors)?;
    }

    let mut remaining: Vec<_> = section_anchors.into_iter().collect();
    remaining.sort_by_key(|(section, _)| *section);
    for (_, mut anchors) in remaining {
        anchors.sort_by(|a, b| a.position.total_cmp(&b.position));
        write_anchors(&mut output, anchors)?;
    }

    Ok(output)
}

/// Writes anchors in order, grouping consecutive images into a single centered block.
fn write_anchors(
    output: &mut String,
    anchors: impl IntoIterator<Item = AnchorPosition>,
) -> Result<()> {
    let mut images = Vec::new();
    for AnchorPosition { anchor, .. } in anchors {
        match anchor {
            Anchor::Image(tag) => images.push(tag),
            Anchor::Id(id) => {
                write_image_block(output, mem::take(&mut images))?;
                write_id_div(output, &id);
            }
        }
    }
    write_image_block(output, images)
}

/// Replaces each marker with its image block. Markers without a matching image, and repeats
/// of a marker that was already placed, are removed.
pub fn replace_image_markers(
    content: &str,
    mut images: HashMap<usize, BytesStart<'_>>,
) -> Result<String> {
    let mut output = String::with_capacity(content.len());
    let mut last = 0;

    for caps in IMAGE_MARKER_RE.captures_iter(content) {
        let Some(marker) = caps.get(0) else {
            continue;
        };
        output.push_str(&content[last..marker.start()]);
        last = marker.end();

        let tag = caps[1].parse().ok().and_then(|n: usize| images.remove(&n));
        if let Some(tag) = tag {
            output.push_str("\n\n");
            write_image_block(&mut output, vec![tag])?;
        }
    }

    output.push_str(&content[last..]);
    Ok(output)
}

fn write_id_div(output: &mut String, id: &str) {
    let _ = writeln!(output, "<a id=\"{}\"/>", escape(id));
}

/// Writes the images as a markdown html block, followed by a blank line so the
/// block is closed before the next line of text.
fn write_image_block(output: &mut String, image_tags: Vec<BytesStart<'_>>) -> Result<()> {
    if image_tags.is_empty() {
        return Ok(());
    }

    let mut writer = Writer::new(Cursor::new(Vec::new()));
    write_image_tags(&mut writer, image_tags)?;
    output.push_str(str::from_utf8(&writer.into_inner().into_inner())?);
    output.push_str("\n\n");
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

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
    fn keeps_quotes_for_smart_punctuation() {
        let content = insert_anchors("\"Don't,\" she said <3 & left", Vec::new()).unwrap();
        assert_eq!(content, "\"Don't,\" she said &lt;3 &amp; left");
    }

    #[test]
    fn writes_illustration_block() {
        let mut output = String::new();
        let img = BytesStart::new("img").with_attributes([("src", "../Images/1.jpg")]);
        write_image_block(&mut output, vec![img]).unwrap();
        assert!(
            output.starts_with("<div class=\"illustration\"><img src=\"../Images/1.jpg\"/></div>")
        );
        assert!(!output.contains("<p>"));
    }
}
