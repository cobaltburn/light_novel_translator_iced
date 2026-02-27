use std::io::Cursor;

use crate::error::Result;
use quick_xml::Writer;
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

pub struct XmlConverter {
    pub skip: Vec<&'static [u8]>,
}

impl XmlConverter {
    pub fn new(skip: Vec<&'static [u8]>) -> Self {
        Self { skip }
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
                    let text = match tag.name().as_ref() {
                        H1 => format!("# {}", text),
                        H2 => format!("## {}", text),
                        H3 => format!("### {}", text),
                        H4 => format!("#### {}", text),
                        H5 => format!("##### {}", text),
                        H6 => format!("###### {}", text),
                        _ => text,
                    };
                    content.push(text);
                }
                Event::Text(text) => {
                    content.push(text.decode()?.to_string());
                }
                Event::Eof => break,
                _ => (),
            }
        }

        Ok(content.join("\n"))
    }

    fn remove_tags<'a>(&self, reader: &mut Reader<&'a [u8]>) -> Result<Vec<u8>> {
        let mut writer = Writer::new(Cursor::new(Vec::<u8>::new()));
        loop {
            match reader.read_event()? {
                Event::Start(tag)
                    if INVALID_TAG.contains(&tag.name().as_ref())
                        || self.skip.contains(&tag.name().as_ref()) =>
                {
                    reader.read_to_end(tag.name())?;
                }
                e @ Event::Start(_)
                | e @ Event::End(_)
                | e @ Event::Empty(_)
                | e @ Event::Text(_) => writer.write_event(e)?,
                e @ Event::Eof => {
                    writer.write_event(e)?;
                    break;
                }
                _ => (),
            }
        }

        let buffer = writer.into_inner().into_inner();
        Ok(buffer)
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
