//! Writes toc ids and images into the translated markdown, before it becomes html.

use crate::{
    epub::html::ANCHOR_TAG,
    error::Result,
    text::{IMAGE_MARKER_RE, content_sections},
};
use quick_xml::{
    Writer,
    escape::{escape, partial_escape},
    events::{BytesStart, Event},
};
use std::{
    collections::{HashMap, VecDeque},
    fmt::Write as _,
    io::Cursor,
};

#[derive(Debug)]
pub enum Anchor {
    Id(String),
    Image(BytesStart<'static>),
}

#[derive(Debug)]
pub struct AnchorPosition {
    pub anchor: Anchor,
    pub section: usize,
    pub line: usize,
    pub position: f64,
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

/// An empty block for an id with nothing after it to go on. The closing tag is written out,
/// since html parsers treat `<div/>` as an open tag that swallows what follows. The blank
/// line closes the markdown html block, so anything after it is parsed as markdown again.
fn write_id_div(output: &mut String, id: &str) {
    let _ = writeln!(output, "<div id=\"{}\"></div>\n", escape(id));
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

#[cfg(test)]
mod test {
    use super::*;

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
