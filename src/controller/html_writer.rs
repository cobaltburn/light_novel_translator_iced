use crate::{
    controller::{IdPosition, count_lines, extract_head, update_tag_path},
    error::Result,
};
use quick_xml::{
    Reader, Writer,
    escape::escape,
    events::{BytesEnd, BytesStart, Event},
};
use regex::Regex;
use std::{
    borrow::Cow,
    collections::{HashMap, VecDeque},
    fmt::Write as _,
    io::{self, Cursor},
    path::PathBuf,
    sync::LazyLock,
};

pub fn write_header(writer: &mut Writer<Cursor<Vec<u8>>>, html: &str) -> Result<()> {
    let head = extract_head(html)?;

    writer
        .create_element("head")
        .write_inner_content(|writer| write_head(writer, head).map_err(io::Error::other))?;

    Ok(())
}

pub fn write_head(writer: &mut Writer<Cursor<Vec<u8>>>, head: Cow<'_, str>) -> Result<()> {
    let folder = PathBuf::from("../Styles");
    let mut reader = Reader::from_str(&head);
    reader.config_mut().trim_text(true);

    loop {
        match reader.read_event()? {
            Event::Empty(tag) if tag.name().as_ref() == b"link" => {
                let tag = update_tag_path(tag, &folder, "href")?;
                writer.write_event(Event::Empty(tag))?;
            }
            Event::Eof => break,
            e => writer.write_event(e)?,
        }
    }

    writer
        .create_element("link")
        .with_attribute(("rel", "stylesheet"))
        .with_attribute(("type", "text/css"))
        .with_attribute(("href", "../stylesheet.css"))
        .write_empty()?;
    Ok(())
}

const ANCHOR_TAG: &[u8] = b"a";
const DIV: &str = "div";

pub fn write_body(writer: &mut Writer<Cursor<Vec<u8>>>, content: &str) -> Result<()> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    writer
        .create_element("body")
        .with_attribute(("class", "p-text"))
        .write_inner_content(|writer| {
            loop {
                match reader.read_event().map_err(io::Error::other)? {
                    Event::Start(tag) if tag.name().as_ref() == ANCHOR_TAG => {
                        writer.write_event(Event::Start(BytesStart::new(DIV)))?;
                        writer.write_event(Event::Start(tag))?;
                    }
                    Event::End(tag) if tag.name().as_ref() == ANCHOR_TAG => {
                        writer.write_event(Event::End(tag))?;
                        writer.write_event(Event::End(BytesEnd::new(DIV)))?;
                    }
                    Event::Eof => break,
                    e => writer.write_event(e)?,
                }
            }
            Ok(())
        })?;
    Ok(())
}

pub fn add_image_tags(content: &str, mut images: Vec<(BytesStart<'_>, f64)>) -> Result<String> {
    let lines = count_lines(content)?;
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    let mut line_count = 0;
    images.sort_by(|(_, a), (_, b)| a.total_cmp(b));
    let mut images = VecDeque::from(images);

    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"p" => {
                line_count += 1;
                let position = line_count as f64 / lines as f64;

                let mut image_tags = Vec::new();
                while let Some((tag, _)) = images.pop_front_if(|&mut (_, i)| position >= i) {
                    image_tags.push(tag);
                }

                write_image_tags(&mut writer, image_tags)?;
                writer.write_event(Event::Start(tag))?;
            }
            Event::Eof => break,
            e => writer.write_event(e)?,
        }
    }

    let remaining: Vec<_> = images.into_iter().map(|(tag, _)| tag).collect();
    write_image_tags(&mut writer, remaining)?;

    Ok(String::from_utf8(writer.into_inner().into_inner())?)
}

fn write_image_tags(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    image_tags: Vec<BytesStart<'_>>,
) -> Result<()> {
    if image_tags.is_empty() {
        return Ok(());
    }

    writer
        .create_element("div")
        .with_attribute(("style", "text-align: center;"))
        .write_inner_content(|writer| {
            writer.create_element("p").write_inner_content(|writer| {
                for tag in image_tags {
                    writer.write_event(Event::Empty(tag))?;
                }
                Ok(())
            })?;
            Ok(())
        })?;

    Ok(())
}

static PART_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<part>.*?</part>").unwrap());

pub fn insert_toc_ids(content: &str, id_positions: Vec<IdPosition>) -> String {
    let mut section_ids: HashMap<usize, Vec<IdPosition>> = HashMap::new();
    for p in id_positions {
        section_ids.entry(p.section).or_default().push(p);
    }

    let sections = PART_RE
        .split(content)
        .filter(|e| !e.trim().is_empty())
        .map(escape);
    let mut output = String::with_capacity(content.len());

    for (index, section) in sections.enumerate() {
        let Some(mut ids) = section_ids.remove(&index) else {
            output.push_str(&section);
            continue;
        };
        ids.sort_by(|a, b| a.position.total_cmp(&b.position));
        let mut ids = VecDeque::from(ids);

        let line_count = section.lines().count();
        for (i, line) in section.lines().enumerate() {
            let position = i as f64 / line_count as f64;
            while let Some(p) = ids.pop_front_if(|p| position >= p.position) {
                write_id_div(&mut output, &p.id);
            }
            output.push_str(line);
            output.push('\n');
        }

        for p in ids {
            write_id_div(&mut output, &p.id);
        }
    }

    output
}

fn write_id_div(output: &mut String, id: &str) {
    let _ = writeln!(output, "<div id=\"{}\"></div>", escape(id));
}
