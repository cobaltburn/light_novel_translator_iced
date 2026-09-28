use crate::Result;
use epub::doc::{EpubDoc, EpubVersion};
use quick_xml::{
    Reader, XmlVersion,
    events::{BytesStart, Event},
};
use std::{io::Cursor, path::PathBuf};

pub const NCX_MIME: &str = "application/x-dtbncx+xml";

/// Reserved stem for the translated toc page, chosen so it cannot collide
/// with a spine file (e.g. `nav.xhtml` or `toc.xhtml`) when saved as `.md`.
pub const TOC_PAGE_STEM: &str = "__toc__";

#[derive(Debug, Default)]
pub struct Nav {
    pub label: String,
    pub path: PathBuf,
}

pub fn get_toc_path(epub: &EpubDoc<Cursor<Vec<u8>>>) -> Option<PathBuf> {
    let resource = match epub.version {
        EpubVersion::Version3_0 => epub.resources.get("toc"),
        EpubVersion::Version2_0 | EpubVersion::Unknown(_) => {
            epub.resources.values().find(|r| r.mime == NCX_MIME)
        }
    }?;
    Some(resource.path.clone())
}

pub fn read_toc(epub: &mut EpubDoc<Cursor<Vec<u8>>>) -> Option<(PathBuf, Vec<Nav>)> {
    let path = get_toc_path(epub)?;
    let doc = epub.get_resource_str_by_path(&path)?;
    let navs = match epub.version {
        EpubVersion::Version3_0 => parse_nav_doc(&doc),
        EpubVersion::Version2_0 | EpubVersion::Unknown(_) => parse_ncx(&doc),
    }
    .ok()?;
    Some((path, navs))
}

pub fn parse_nav_doc(html: &str) -> Result<Vec<Nav>> {
    let mut reader = Reader::from_str(html);
    reader.config_mut().trim_text(true);
    let nav_html = loop {
        match reader.read_event()? {
            Event::Start(tag) if is_nav_toc(&tag) => {
                break reader.read_text(tag.name())?;
            }
            Event::Eof => return Ok(Vec::new()),
            _ => (),
        }
    };

    let nav_html = nav_html.into_inner();
    let mut reader = Reader::from_reader(nav_html.as_ref());
    let mut navs: Vec<Nav> = Vec::new();
    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"a" => {
                if let Some(href) = tag.try_get_attribute("href")? {
                    let path = href.normalized_value(XmlVersion::Implicit1_0)?;
                    let path = PathBuf::from(path.as_ref());
                    let label = reader.read_text(tag.name())?.xml10_content()?.to_string();
                    navs.push(Nav { label, path });
                }
            }
            Event::Eof => break,
            _ => (),
        }
    }
    Ok(navs)
}

pub fn is_nav_toc(tag: &BytesStart<'_>) -> bool {
    tag.try_get_attribute("epub:type")
        .ok()
        .flatten()
        .and_then(|a| a.normalized_value(XmlVersion::Implicit1_0).ok())
        .is_some_and(|a| a == "toc")
}

pub fn parse_ncx(ncx: &str) -> Result<Vec<Nav>> {
    let mut reader = Reader::from_str(ncx);
    reader.config_mut().trim_text(true);

    let mut navs: Vec<Nav> = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    let mut in_label = false;

    loop {
        match reader.read_event()? {
            Event::Start(tag) => match tag.name().as_ref() {
                b"navPoint" => {
                    open.push(navs.len());
                    navs.push(Nav::default());
                }
                b"navLabel" => in_label = true,
                _ => (),
            },
            Event::End(tag) => match tag.name().as_ref() {
                b"navPoint" => {
                    open.pop();
                }
                b"navLabel" => in_label = false,
                _ => (),
            },
            Event::Text(text) if in_label => {
                if let Some(&i) = open.last() {
                    navs[i].label.push_str(&text.xml10_content()?);
                }
            }
            Event::Empty(tag) if tag.name().as_ref() == b"content" => {
                if let Some(&i) = open.last()
                    && let Some(src) = tag.try_get_attribute("src")?
                {
                    let value = src.normalized_value(XmlVersion::Implicit1_0)?;
                    navs[i].path = PathBuf::from(value.as_ref());
                }
            }
            Event::Eof => break,
            _ => (),
        }
    }

    Ok(navs)
}
