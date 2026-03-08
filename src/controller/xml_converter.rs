use std::io::Cursor;

use crate::error::Result;
use quick_xml::Writer;
use quick_xml::events::BytesStart;
use quick_xml::{Reader, events::Event};

pub const H1: &[u8] = b"h1";
pub const H2: &[u8] = b"h2";
pub const H3: &[u8] = b"h3";
pub const H4: &[u8] = b"h4";
pub const H5: &[u8] = b"h5";
pub const H6: &[u8] = b"h6";
pub const P: &[u8] = b"p";
pub const TAGS: &[&[u8]] = &[H1, H2, H3, H4, H5, H6, P];
pub const HEAD: &[u8] = b"head";
const STYLE: &[u8] = b"style";
const SCRIPT: &[u8] = b"script";
const INVALID_TAG: [&[u8]; 2] = [STYLE, SCRIPT];

pub struct XmlConverter<'a> {
    pub skip_tags: Vec<&'a [u8]>,
    pub skip_class: Vec<String>,
}

impl XmlConverter<'_> {
    pub fn new(skip_tags: Vec<&'static [u8]>, skip_class: Vec<String>) -> Self {
        Self {
            skip_tags,
            skip_class,
        }
    }

    pub fn convert(&self, html: &str) -> Result<String> {
        let mut reader = Reader::from_str(html);
        reader.config_mut().trim_text(true);

        let buffer = self.remove_tags(&mut reader)?;
        reader = Reader::from_reader(&buffer);
        reader.config_mut().trim_text(true);

        let mut content = Vec::with_capacity(20);

        loop {
            match reader.read_event()? {
                Event::Start(tag) if TAGS.contains(&tag.name().as_ref()) => {
                    let text = reader.read_text(tag.name())?;
                    let text = extract_text(&text)?;
                    let count = match tag.name().as_ref() {
                        _ if text.is_empty() => None,
                        H1 => Some(1),
                        H2 => Some(2),
                        H3 => Some(3),
                        H4 => Some(4),
                        H5 => Some(5),
                        H6 => Some(6),
                        _ => None,
                    };

                    let lead = count.map_or(String::new(), |i| format!("{} ", "#".repeat(i)));
                    content.push(format!("{}{}", lead, text));
                }
                Event::Text(text) => {
                    content.push(text.decode()?.to_string());
                }
                Event::Eof => break,
                _ => (),
            }
        }

        Ok(String::from(content.join("\n").trim()))
    }

    fn remove_tags<'a>(&self, reader: &mut Reader<&'a [u8]>) -> Result<Vec<u8>> {
        let mut writer = Writer::new(Cursor::new(Vec::<u8>::new()));
        loop {
            match reader.read_event()? {
                Event::Start(tag) if self.should_skip(&tag)? => {
                    reader.read_to_end(tag.name())?;
                }
                e @ (Event::Start(_) | Event::End(_) | Event::Empty(_) | Event::Text(_)) => {
                    writer.write_event(e)?
                }
                Event::Eof => break,
                _ => (),
            }
        }

        let buffer = writer.into_inner().into_inner();
        Ok(buffer)
    }

    fn should_skip(&self, tag: &BytesStart<'_>) -> Result<bool> {
        let class_attr = "class";
        let name = tag.name();
        let skip = INVALID_TAG.contains(&name.as_ref())
            || self.skip_tags.contains(&name.as_ref())
            || tag.try_get_attribute(class_attr)?.is_some_and(|a| {
                let val = a.unescape_value().unwrap_or_default();
                self.skip_class.iter().any(|e| val.contains(e))
            });
        Ok(skip)
    }
}

fn extract_text(text: &str) -> Result<String> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut content = String::with_capacity(text.len());
    loop {
        match reader.read_event()? {
            Event::Text(text) => content.push_str(&text.decode()?),
            Event::Eof => break,
            _ => (),
        }
    }
    Ok(content)
}
