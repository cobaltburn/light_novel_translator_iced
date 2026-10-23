use crate::{
    controller::{
        Anchor, AnchorPosition, IMAGE_MARKER_RE, PageLinks, extract_head, is_header,
        update_resource_tag,
    },
    error::Result,
};
use quick_xml::{
    Reader, Writer, XmlVersion,
    escape::{escape, partial_escape},
    events::{BytesStart, Event},
};
use regex::Regex;
use std::{
    borrow::Cow,
    collections::{HashMap, VecDeque},
    fmt::Write as _,
    io::{self, Cursor},
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

const ANCHOR_TAG: &str = "a";
const PARAGRAPH_TAG: &str = "p";
const BR: &str = "br";

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

/// The first id goes on the illustration itself, any others on empty anchors inside it.
fn write_image_tags(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    image_tags: Vec<BytesStart<'_>>,
    ids: Vec<String>,
) -> Result<()> {
    if image_tags.is_empty() {
        return Ok(());
    }

    let mut ids = ids.into_iter();
    let mut div = writer
        .create_element("div")
        .with_attribute(("class", "illustration"));
    if let Some(id) = ids.next() {
        div = div.with_attribute(("id", id.as_str()));
    }
    div.write_inner_content(|writer| {
        for id in ids {
            let anchor = BytesStart::new(ANCHOR_TAG).with_attributes([("id", id.as_str())]);
            writer.write_event(Event::Empty(anchor))?;
        }
        for tag in image_tags {
            writer.write_event(Event::Empty(tag))?;
        }
        Ok(())
    })?;

    Ok(())
}

static PART_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<part>.*?</part>").unwrap());

/// The translated sections of a page, indexed the same way as `AnchorPosition::section`.
pub fn content_sections(content: &str) -> impl Iterator<Item = &str> {
    PART_RE.split(content).filter(|e| !e.trim().is_empty())
}

pub fn insert_anchors(content: &str, anchors: Vec<AnchorPosition>) -> Result<String> {
    let mut section_anchors: HashMap<usize, Vec<AnchorPosition>> = HashMap::new();
    for p in anchors {
        section_anchors.entry(p.section).or_default().push(p);
    }

    let sections = content_sections(content).map(partial_escape);
    let mut output = String::with_capacity(content.len());
    // Ids waiting for a line with text, carried across blank lines and into the next section
    let mut ids = Vec::new();

    for (index, section) in sections.enumerate() {
        let mut anchors = section_anchors.remove(&index).unwrap_or_default();
        if anchors.is_empty() && ids.is_empty() {
            output.push_str(&section);
            continue;
        }
        anchors.sort_by(|a, b| a.position.total_cmp(&b.position));
        let mut anchors = VecDeque::from(anchors);

        let line_count = section.lines().count();
        for (i, line) in section.lines().enumerate() {
            let position = i as f64 / line_count as f64;
            let mut popped = Vec::new();
            while let Some(p) = anchors.pop_front_if(|p| position >= p.position) {
                popped.push(p);
            }
            write_anchors(&mut output, popped, &mut ids)?;
            write_line(&mut output, line, &mut ids);
        }

        write_anchors(&mut output, anchors, &mut ids)?;
    }

    let mut remaining: Vec<_> = section_anchors.into_iter().collect();
    remaining.sort_by_key(|(section, _)| *section);
    for (_, mut anchors) in remaining {
        anchors.sort_by(|a, b| a.position.total_cmp(&b.position));
        write_anchors(&mut output, anchors, &mut ids)?;
    }

    ids.iter().for_each(|id| write_id_div(&mut output, id));

    Ok(output)
}

/// Writes the images as a single centered block and adds the ids to `ids`, which belong on
/// the next line with text. Ids already waiting go on the images, so they don't end up
/// pointing past them.
fn write_anchors(
    output: &mut String,
    anchors: impl IntoIterator<Item = AnchorPosition>,
    ids: &mut Vec<String>,
) -> Result<()> {
    let mut images = Vec::new();
    let mut new_ids = Vec::new();
    for AnchorPosition { anchor, .. } in anchors {
        match anchor {
            Anchor::Image(tag) => images.push(tag),
            Anchor::Id(id) => new_ids.push(id),
        }
    }
    if !images.is_empty() {
        write_image_block(output, images, std::mem::take(ids))?;
    }
    ids.extend(new_ids);
    Ok(())
}

/// Writes the line with the id anchors at the start of its text, after any heading marker,
/// so they end up inside its paragraph or heading where `write_body` moves the id onto
/// the element itself. Readers handle that better than an empty anchor between blocks.
/// A blank line leaves the ids for the next line.
fn write_line(output: &mut String, line: &str, ids: &mut Vec<String>) {
    if line.trim().is_empty() {
        output.push_str(line);
        output.push('\n');
        return;
    }

    let text = line.trim_start();
    let hashes = text.len() - text.trim_start_matches('#').len();
    let marker = match text[hashes..].strip_prefix(' ') {
        Some(_) if (1..=6).contains(&hashes) => hashes + 1,
        _ => 0,
    };
    let split = line.len() - text.len() + marker;

    output.push_str(&line[..split]);
    for id in ids.drain(..) {
        let _ = write!(output, "<a id=\"{}\"/>", escape(&id));
    }
    output.push_str(&line[split..]);
    output.push('\n');
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
            write_image_block(&mut output, vec![tag], Vec::new())?;
        }
    }

    output.push_str(&content[last..]);
    Ok(output)
}

/// An empty block for an id with nothing after it to go on. The closing tag is written out,
/// since html parsers treat `<div/>` as an open tag that swallows what follows. The blank
/// line closes the markdown html block, so anything after it is parsed as markdown again.
fn write_id_div(output: &mut String, id: &str) {
    let _ = writeln!(output, "<div id=\"{}\"></div>\n", escape(id));
}

/// Writes the images as a markdown html block, followed by a blank line so the
/// block is closed before the next line of text.
fn write_image_block(
    output: &mut String,
    image_tags: Vec<BytesStart<'_>>,
    ids: Vec<String>,
) -> Result<()> {
    if image_tags.is_empty() {
        return Ok(());
    }

    let mut writer = Writer::new(Cursor::new(Vec::new()));
    write_image_tags(&mut writer, image_tags, ids)?;
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

        let html = body(&crate::controller::to_html(&markdown));
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

        let html = body(&crate::controller::to_html(&markdown));
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
        let html = body(&crate::controller::to_html(&markdown));
        assert!(html.contains(
            "<div class=\"illustration\" id=\"c1\"><a id=\"c2\"/><img src=\"../Images/1.jpg\"/></div>"
        ));
        assert!(html.contains("<p>Text.</p>"));
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
        write_image_block(&mut output, vec![img], Vec::new()).unwrap();
        assert!(
            output.starts_with("<div class=\"illustration\"><img src=\"../Images/1.jpg\"/></div>")
        );
        assert!(!output.contains("<p>"));
    }
}
