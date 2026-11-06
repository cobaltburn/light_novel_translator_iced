use crate::{
    epub::{
        html::{
            resource_links::{IMAGES, update_tag_path},
            strip::strip_syosetu_tags,
            tags::{HEAD_TAG, IMG_TAG, SRC, XLINK, is_image},
            text::TextRun,
        },
        resources::PageLinks,
    },
    error::Result,
    text::image_marker,
};
use quick_xml::{
    Reader, Writer,
    events::{BytesStart, BytesText, Event},
};
use std::io::Cursor;

pub fn insert_image_markers(html: &str) -> Result<String> {
    let mut reader = Reader::from_str(html);
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let mut count = 0;

    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == HEAD_TAG => {
                reader.read_to_end(tag.name())?;
            }
            Event::Empty(tag) if is_image(&tag) => {
                writer
                    .create_element("p")
                    .write_text_content(BytesText::new(&image_marker(count)))?;
                count += 1;
            }
            Event::Eof => break,
            e => writer.write_event(e)?,
        }
    }

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

pub fn image_anchors(
    html: &str,
    links: PageLinks<'_>,
) -> Result<Vec<(BytesStart<'static>, Option<String>)>> {
    let html = strip_syosetu_tags(html)?;
    let mut reader = Reader::from_str(&html);
    let mut anchors = vec![];
    let mut pending = vec![];
    let mut run = TextRun::default();

    loop {
        let event = reader.read_event()?;
        if run.push(&event)? {
            continue;
        }
        if let Some(text) = run.finish()
            && !pending.is_empty()
        {
            anchors.extend(pending.drain(..).map(|tag| (tag, Some(text.clone()))));
        }

        match event {
            Event::Start(tag) if tag.name().as_ref() == HEAD_TAG => {
                reader.read_to_end(tag.name())?;
            }
            Event::Empty(tag) if is_image(&tag) => {
                let attr = if tag.name().as_ref() == IMG_TAG {
                    SRC
                } else {
                    XLINK
                };
                pending.push(update_tag_path(tag, links, IMAGES, attr)?);
            }
            Event::Eof => break,
            _ => (),
        }
    }

    anchors.extend(pending.into_iter().map(|tag| (tag, None)));
    Ok(anchors)
}
