use crate::{
    controller::{PageLinks, image_marker},
    error::{Error, Result},
};
use bstr::ByteSlice;
use pulldown_cmark::{Options, Parser, html::push_html};
use quick_xml::{
    Reader, Writer, XmlVersion,
    events::{BytesStart, BytesText, Event},
};
use regex::Regex;
use std::{borrow::Cow, io::Cursor};

pub fn to_html(markdown: &str) -> String {
    let options =
        Options::ENABLE_SMART_PUNCTUATION | Options::ENABLE_TABLES | Options::ENABLE_FOOTNOTES;
    let parser = Parser::new_ext(markdown, options);
    let mut html = String::with_capacity(markdown.len());
    push_html(&mut html, parser);
    html
}

pub fn remove_part_tags(content: &str) -> Cow<'_, str> {
    let rg = Regex::new(r"(?s)<part>.*?</part>\s*").unwrap();
    rg.replace_all(content, "")
}

pub fn strip_syosetu_tags(html: &str) -> Result<String> {
    let mut reader = Reader::from_str(html);
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    loop {
        match reader.read_event()? {
            Event::Start(tag) if contains_author_notes(&tag) => {
                reader.read_to_end(tag.name())?;
            }
            Event::Eof => break,
            e => writer.write_event(e)?,
        }
    }

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

const SYOSETU_AFTERWORD: &[u8] = b"--afterword";
const SYOSETU_PREFACE: &[u8] = b"--preface";
const SYOSETU_ATTRIBUTES: &[&[u8]] = &[SYOSETU_PREFACE, SYOSETU_AFTERWORD];

fn contains_author_notes(tag: &BytesStart<'_>) -> bool {
    tag.try_get_attribute("class")
        .ok()
        .flatten()
        .is_some_and(|a| SYOSETU_ATTRIBUTES.iter().any(|e| a.value.contains_str(e)))
}

pub fn strip_tags(html: &str) -> Result<String> {
    let mut reader = Reader::from_str(html);
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let tag_match = |e: &[u8]| matches!(e, b"head" | b"img" | b"image" | b"rt" | b"rp");

    loop {
        match reader.read_event()? {
            Event::Empty(tag) if tag_match(tag.name().as_ref()) => (),
            Event::Start(tag) if tag_match(tag.name().as_ref()) => {
                reader.read_to_end(tag.name())?;
            }
            e @ (Event::Start(_) | Event::End(_) | Event::Empty(_) | Event::Text(_)) => {
                writer.write_event(e)?;
            }
            Event::Eof => break,
            _ => (),
        }
    }

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

pub const SRC: &str = "src";
pub const XLINK: &str = "xlink:href";
pub const IMG_BYTES: &[u8] = b"img";
pub const IMAGE_BYTES: &[u8] = b"image";

pub const IMAGES: &str = "../Images";
pub const STYLES: &str = "../Styles";
pub const SCRIPTS: &str = "../Script";

pub fn update_image_paths(html: &str, links: PageLinks<'_>) -> Result<String> {
    let mut reader = Reader::from_str(html);
    reader.config_mut().trim_text(true);
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    loop {
        match reader.read_event()? {
            Event::Empty(tag) if tag.name().as_ref() == IMG_BYTES => {
                let tag = update_tag_path(tag, links, IMAGES, SRC)?;
                writer.write_event(Event::Empty(tag))?;
            }
            Event::Empty(tag) if tag.name().as_ref() == IMAGE_BYTES => {
                let tag = update_tag_path(tag, links, IMAGES, XLINK)?;
                writer.write_event(Event::Empty(tag))?;
            }
            Event::Eof => break,
            event => writer.write_event(event)?,
        }
    }

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

pub const LINK_BYTES: &[u8] = b"link";
pub const SCRIPT_BYTES: &[u8] = b"script";

pub fn update_resource_tag<'a>(event: Event<'a>, links: PageLinks<'_>) -> Result<Event<'a>> {
    Ok(match event {
        Event::Empty(tag) if tag.name().as_ref() == LINK_BYTES => {
            Event::Empty(update_tag_path(tag, links, STYLES, "href")?)
        }
        Event::Empty(tag) if tag.name().as_ref() == SCRIPT_BYTES => {
            Event::Empty(update_tag_path(tag, links, SCRIPTS, SRC)?)
        }
        Event::Start(tag) if tag.name().as_ref() == SCRIPT_BYTES => {
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
        .filter(|a| a.key.as_ref() != attr.as_bytes())
        .collect();

    let tag = BytesStart::new(str::from_utf8(tag.name().as_ref())?)
        .with_attributes(attributes)
        .with_attributes([(attr, path.as_str())])
        .into_owned();
    Ok(tag)
}

pub fn extract_head(html: &str) -> Result<Cow<'_, str>> {
    let mut reader = Reader::from_str(html);
    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"head" => {
                return Ok(reader.read_text(tag.name())?.decode()?);
            }
            Event::Eof => return Err(Error::BuildError("No head tag found")),
            _ => (),
        }
    }
}

pub fn insert_image_markers(html: &str) -> Result<String> {
    let mut reader = Reader::from_str(html);
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let mut count = 0;

    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"head" => {
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

fn is_image(tag: &BytesStart<'_>) -> bool {
    let name = tag.name();
    name.as_ref() == IMG_BYTES || name.as_ref() == IMAGE_BYTES
}

pub fn image_anchors(
    html: &str,
    links: PageLinks<'_>,
) -> Result<Vec<(BytesStart<'static>, Option<String>)>> {
    let html = strip_syosetu_tags(html)?;
    let mut reader = Reader::from_str(&html);
    let mut anchors = vec![];
    let mut pending = vec![];

    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"head" => {
                reader.read_to_end(tag.name())?;
            }
            Event::Empty(tag) if is_image(&tag) => {
                let attr = if tag.name().as_ref() == IMG_BYTES {
                    SRC
                } else {
                    XLINK
                };
                pending.push(update_tag_path(tag, links, IMAGES, attr)?);
            }
            Event::Text(text) if !pending.is_empty() => {
                let text = text.xml10_content()?;
                let text = text.trim();
                if !text.is_empty() {
                    anchors.extend(pending.drain(..).map(|tag| (tag, Some(text.to_string()))));
                }
            }
            Event::Eof => break,
            _ => (),
        }
    }

    anchors.extend(pending.into_iter().map(|tag| (tag, None)));
    Ok(anchors)
}

#[cfg(test)]
mod test {
    use super::{strip_tags, to_html};

    #[test]
    fn strips_ruby_readings() {
        let html = "<p><ruby>漢字<rp>(</rp><rt>かんじ</rt><rp>)</rp></ruby>を読む</p>";
        assert_eq!(strip_tags(html).unwrap(), "<p><ruby>漢字</ruby>を読む</p>");
    }

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
