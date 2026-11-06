//! Points images, stylesheets and scripts at their new paths in the output epub.

use crate::{
    epub::{
        html::tags::{IMAGE_TAG, IMG_TAG, LINK_TAG, SCRIPT_TAG, SRC, XLINK},
        resources::PageLinks,
    },
    error::Result,
};
use quick_xml::{
    Reader, Writer, XmlVersion,
    events::{BytesStart, Event},
};
use std::io::Cursor;

pub const IMAGES: &str = "../Images";
pub const STYLES: &str = "../Styles";
pub const SCRIPTS: &str = "../Script";

pub fn update_image_paths(html: &str, links: PageLinks<'_>) -> Result<String> {
    let mut reader = Reader::from_str(html);
    reader.config_mut().trim_text(true);
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    loop {
        match reader.read_event()? {
            Event::Empty(tag) if tag.name().as_ref() == IMG_TAG => {
                let tag = update_tag_path(tag, links, IMAGES, SRC)?;
                writer.write_event(Event::Empty(tag))?;
            }
            Event::Empty(tag) if tag.name().as_ref() == IMAGE_TAG => {
                let tag = update_tag_path(tag, links, IMAGES, XLINK)?;
                writer.write_event(Event::Empty(tag))?;
            }
            Event::Eof => break,
            event => writer.write_event(event)?,
        }
    }

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

pub fn update_resource_tag<'a>(event: Event<'a>, links: PageLinks<'_>) -> Result<Event<'a>> {
    Ok(match event {
        Event::Empty(tag) if tag.name().as_ref() == LINK_TAG => {
            Event::Empty(update_tag_path(tag, links, STYLES, "href")?)
        }
        Event::Empty(tag) if tag.name().as_ref() == SCRIPT_TAG => {
            Event::Empty(update_tag_path(tag, links, SCRIPTS, SRC)?)
        }
        Event::Start(tag) if tag.name().as_ref() == SCRIPT_TAG => {
            Event::Start(update_tag_path(tag, links, SCRIPTS, SRC)?)
        }
        event => event,
    })
}

pub fn update_resource_paths(html: &str, links: PageLinks<'_>) -> Result<String> {
    let mut reader = Reader::from_str(html);
    reader.config_mut().trim_text(true);
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    loop {
        match reader.read_event()? {
            Event::Eof => break,
            event => writer.write_event(update_resource_tag(event, links)?)?,
        }
    }

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

pub fn update_tag_path(
    tag: BytesStart<'_>,
    links: PageLinks<'_>,
    folder: &str,
    attr: &str,
) -> Result<BytesStart<'static>> {
    let Some(link) = tag.try_get_attribute(attr)? else {
        return Ok(tag.into_owned());
    };

    let link = link.normalized_value(XmlVersion::Implicit1_0)?;
    let path = links.resolve(&link, folder);

    let attributes: Vec<_> = tag
        .attributes()
        .flatten()
        .filter(|a| a.key.as_ref() != attr)
        .collect();

    let tag = BytesStart::new(tag.name().as_ref())
        .with_attributes(attributes)
        .with_attributes([(attr, path.as_str())])
        .into_owned();
    Ok(tag)
}
