use pulldown_cmark::{Options, Parser, html::push_html};
use quick_xml::{
    Reader, Writer,
    events::{BytesStart, Event},
};
use regex::Regex;
use std::{io::Cursor, os::unix::ffi::OsStrExt, path::PathBuf};

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

pub fn update_image_paths(xml: &str) -> anyhow::Result<String> {
    let image_folder = PathBuf::from("../Images");
    let mut reader = Reader::from_str(xml);
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    loop {
        match reader.read_event()? {
            Event::Eof => break,
            Event::Empty(tag) if tag.name().as_ref() == IMG.as_bytes() => {
                let tag = update_image(tag, &image_folder, SRC)?;
                writer.write_event(Event::Empty(tag))?;
            }
            Event::Empty(tag) if tag.name().as_ref() == IMAGE.as_bytes() => {
                let tag = update_image(tag, &image_folder, XLINK)?;
                writer.write_event(Event::Empty(tag))?;
            }
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
) -> anyhow::Result<BytesStart<'static>> {
    let link = tag.try_get_attribute(attr)?;
    let tag = if let Some(link) = link {
        let path = PathBuf::from(str::from_utf8(&link.value)?);
        let file_name = path.file_name().unwrap();
        let path = image_folder.join(file_name);
        let path = path.as_os_str();

        let attributes = tag
            .attributes()
            .flatten()
            .filter(|a| a.key.as_ref() != attr.as_bytes())
            .collect::<Vec<_>>();

        BytesStart::new(str::from_utf8(tag.name().into_inner())?)
            .with_attributes(attributes)
            .with_attributes([(attr.as_bytes(), path.as_bytes())])
            .into_owned()
    } else {
        tag.into_owned()
    };
    Ok(tag)
}
