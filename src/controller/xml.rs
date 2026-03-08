use crate::error::{Error, Result};
use pulldown_cmark::{Options, Parser, html::push_html};
use quick_xml::{
    Reader, Writer,
    events::{BytesStart, Event},
};
use regex::Regex;
use std::{borrow::Cow, io::Cursor, os::unix::ffi::OsStrExt, path::PathBuf};

pub fn to_xml(markdown: &str) -> String {
    let mut html = String::new();
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

pub fn wrap_tag(xml: &str, tag: &str) -> String {
    format!("<{0}>\n{1}\n</{0}>", tag, xml)
}

const SRC: &str = "src";
const XLINK: &str = "xlink:href";
const IMG: &str = "img";
const IMAGE: &str = "image";

pub fn update_image_paths(xml: &str) -> Result<String> {
    let image_folder = PathBuf::from("../Images");
    let mut reader = Reader::from_str(xml);
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    loop {
        match reader.read_event()? {
            Event::Empty(tag) if tag.name().as_ref() == IMG.as_bytes() => {
                let tag = update_image(tag, &image_folder, SRC)?;
                writer.write_event(Event::Empty(tag))?;
            }
            Event::Empty(tag) if tag.name().as_ref() == IMAGE.as_bytes() => {
                let tag = update_image(tag, &image_folder, XLINK)?;
                writer.write_event(Event::Empty(tag))?;
            }
            Event::Eof => break,
            event => writer.write_event(event)?,
        }
    }

    let buf = writer.into_inner().into_inner();
    Ok(String::from_utf8(buf)?)
}

fn update_image(
    tag: BytesStart<'_>,
    image_folder: &PathBuf,
    attr: &str,
) -> Result<BytesStart<'static>> {
    let Some(link) = tag.try_get_attribute(attr)? else {
        return Ok(tag.into_owned());
    };

    let path = PathBuf::from(link.unescape_value()?.as_ref());
    let file_name = path.file_name().unwrap();
    let path = image_folder.join(file_name);
    let path = path.as_os_str();

    let attributes = tag
        .attributes()
        .flatten()
        .filter(|a| a.key.as_ref() != attr.as_bytes())
        .collect::<Vec<_>>();

    let tag = BytesStart::new(str::from_utf8(tag.name().as_ref())?)
        .with_attributes(attributes)
        .with_attributes([(attr.as_bytes(), path.as_bytes())])
        .into_owned();
    Ok(tag)
}

pub fn extract_html_tag(html: &str) -> Result<BytesStart<'_>> {
    let mut reader = Reader::from_str(html.as_ref());
    let tag = loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"html" => {
                break tag;
            }
            Event::Eof => return Err(Error::BuildError("No head tag found")),
            _ => (),
        }
    };
    let lang = "xml:lang";

    let attributes = tag
        .attributes()
        .flatten()
        .filter(|a| a.key.as_ref() != lang.as_bytes())
        .collect::<Vec<_>>();

    let tag = BytesStart::new(str::from_utf8(tag.name().as_ref())?)
        .with_attributes(attributes)
        .with_attributes([(lang, "en")])
        .into_owned();

    Ok(tag)
}

pub fn extract_head(html: &str) -> Result<Cow<'_, str>> {
    let mut reader = Reader::from_str(html.as_ref());
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

pub fn starting_image_tag(html: &str) -> Result<Option<BytesStart<'_>>> {
    let image_folder = PathBuf::from("../Images");
    let body = extract_body(html)?;
    let mut reader = Reader::from_str(body.as_ref());
    reader.config_mut().trim_text(true);

    loop {
        match reader.read_event()? {
            Event::Empty(tag) if tag.name().as_ref() == IMG.as_bytes() => {
                let tag = update_image(tag, &image_folder, SRC)?;
                return Ok(Some(tag));
            }
            Event::Empty(tag) if tag.name().as_ref() == IMAGE.as_bytes() => {
                let tag = update_image(tag, &image_folder, XLINK)?;
                return Ok(Some(tag));
            }
            Event::Text(_) | Event::Eof => return Ok(None),
            _ => (),
        }
    }
}

pub fn body_tag(html: &str) -> Result<BytesStart<'_>> {
    let mut reader = Reader::from_str(html);
    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"body" => return Ok(tag.into_owned()),
            Event::Eof => return Err(Error::BuildError("No body tag found")),
            _ => (),
        }
    }
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
