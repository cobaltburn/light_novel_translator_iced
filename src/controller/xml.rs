use crate::error::{Error, Result};
use pulldown_cmark::{Options, Parser, html::push_html};
use quick_xml::{
    Reader, Writer,
    events::{BytesStart, Event},
};
use regex::Regex;
use std::{borrow::Cow, io::Cursor, os::unix::ffi::OsStrExt, path::PathBuf};

pub fn to_xml(markdown: &str) -> String {
    let mut html = String::with_capacity(markdown.len());
    let parser = Parser::new_ext(markdown, Options::all());
    push_html(&mut html, parser);
    html
}

pub fn remove_think_tags(input: &str) -> String {
    let rg = Regex::new(r"(?s)<think>.*?</think>\s*").unwrap();
    rg.replace_all(input, "").to_string()
}

pub fn remove_part_tags(input: &str) -> String {
    let rg = Regex::new(r"(?s)<part>.*?</part>\s*").unwrap();
    rg.replace_all(input, "").to_string()
}

pub fn strip_data_tags(html: &str) -> Result<String> {
    let mut reader = Reader::from_str(html);
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    loop {
        match reader.read_event()? {
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
pub const IMG: &str = "img";
pub const IMAGE: &str = "image";

pub fn update_image_paths(html: &str) -> Result<String> {
    let folder = PathBuf::from("../Images");
    let mut reader = Reader::from_str(html);
    reader.config_mut().trim_text(true);
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    loop {
        match reader.read_event()? {
            Event::Empty(tag) if tag.name().as_ref() == IMG.as_bytes() => {
                let tag = update_tag_path(tag, &folder, SRC)?;
                writer.write_event(Event::Empty(tag))?;
            }
            Event::Empty(tag) if tag.name().as_ref() == IMAGE.as_bytes() => {
                let tag = update_tag_path(tag, &folder, XLINK)?;
                writer.write_event(Event::Empty(tag))?;
            }
            Event::Eof => break,
            event => writer.write_event(event)?,
        }
    }

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

pub fn update_style_path(html: &str) -> Result<String> {
    let folder = PathBuf::from("../Styles");
    let mut reader = Reader::from_str(html);
    reader.config_mut().trim_text(true);
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    loop {
        match reader.read_event()? {
            Event::Empty(tag) if tag.name().as_ref() == b"link" => {
                let tag = update_tag_path(tag, &folder, "href")?;
                writer.write_event(Event::Empty(tag))?;
            }
            Event::Eof => break,
            event => writer.write_event(event)?,
        }
    }

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

pub fn update_tag_path(
    tag: BytesStart<'_>,
    folder: &PathBuf,
    attr: &str,
) -> Result<BytesStart<'static>> {
    let Some(link) = tag.try_get_attribute(attr)? else {
        return Ok(tag.into_owned());
    };

    let path = PathBuf::from(link.unescape_value()?.as_ref());
    let file_name = path.file_name().unwrap();
    let path = folder.join(file_name);
    let path = path.as_os_str();

    let attributes: Vec<_> = tag
        .attributes()
        .flatten()
        .filter(|a| a.key.as_ref() != attr.as_bytes())
        .collect();

    let tag = BytesStart::new(str::from_utf8(tag.name().as_ref())?)
        .with_attributes(attributes)
        .with_attributes([(attr.as_bytes(), path.as_bytes())])
        .into_owned();
    Ok(tag)
}

pub fn extract_body(html: &str) -> Result<Cow<'_, str>> {
    let mut reader = Reader::from_str(html);
    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"body" => {
                return Ok(reader.read_text(tag.name())?);
            }
            Event::Eof => return Err(Error::BuildError("No body tag found")),
            _ => (),
        }
    }
}

pub fn extract_head(html: &str) -> Result<Cow<'_, str>> {
    let mut reader = Reader::from_str(html);
    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"head" => {
                return Ok(reader.read_text(tag.name())?);
            }
            Event::Eof => return Err(Error::BuildError("No head tag found")),
            _ => (),
        }
    }
}

pub fn count_lines(html: &str) -> Result<i32> {
    let mut reader = Reader::from_str(html);
    let mut count = 0;
    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"p" => count += 1,
            Event::Eof => break,
            _ => (),
        }
    }

    Ok(count)
}

pub fn image_position(html: &str) -> Result<Vec<(BytesStart<'_>, f64)>> {
    let folder = PathBuf::from("../Images");
    let mut reader = Reader::from_str(html);
    let mut count = 0;
    let mut images = vec![];

    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"p" => count += 1,
            Event::Empty(tag) if tag.name().as_ref() == IMG.as_bytes() => {
                let tag = update_tag_path(tag, &folder, SRC)?;
                images.push((tag, count))
            }
            Event::Empty(tag) if tag.name().as_ref() == IMAGE.as_bytes() => {
                let tag = update_tag_path(tag, &folder, XLINK)?;
                images.push((tag, count))
            }
            Event::Eof => break,
            _ => (),
        }
    }

    images.sort_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap());
    let images = images
        .into_iter()
        .map(|(b, i)| (b, i as f64 / count as f64))
        .collect();
    Ok(images)
}
