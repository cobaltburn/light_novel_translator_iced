use crate::{
    epub::html::{
        tags::{HEAD_TAG, IMAGE_TAG, IMG_TAG, is_ruby},
        text::resolve_entity,
    },
    error::Result,
};
use quick_xml::{
    Reader, Writer,
    events::{BytesStart, BytesText, Event},
};
use std::io::Cursor;

const SYOSETU_AFTERWORD: &str = "--afterword";
const SYOSETU_PREFACE: &str = "--preface";
const SYOSETU_ATTRIBUTES: &[&str] = &[SYOSETU_PREFACE, SYOSETU_AFTERWORD];

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

fn contains_author_notes(tag: &BytesStart<'_>) -> bool {
    tag.try_get_attribute("class")
        .ok()
        .flatten()
        .is_some_and(|a| SYOSETU_ATTRIBUTES.iter().any(|e| a.value.contains(e)))
}

pub fn strip_tags(html: &str) -> Result<String> {
    let mut reader = Reader::from_str(html);
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let tag_match = |e: &str| matches!(e, HEAD_TAG | IMG_TAG | IMAGE_TAG | "rt" | "rp");
    let mut ruby_depth = 0usize;
    let mut ruby_text = String::new();

    loop {
        let event = reader.read_event()?;
        match event {
            Event::Text(ref text) if ruby_depth > 0 => {
                ruby_text.push_str(&text.html_content());
                continue;
            }
            Event::GeneralRef(ref entity) if ruby_depth > 0 => {
                ruby_text.push_str(&resolve_entity(entity)?);
                continue;
            }
            Event::Start(_) | Event::End(_) | Event::Empty(_) | Event::Eof => {
                let text = ruby_text.trim();
                if !text.is_empty() {
                    writer.write_event(Event::Text(BytesText::from_escaped(text)))?;
                }
                ruby_text.clear();
            }
            _ => (),
        }

        match event {
            Event::Empty(tag) if tag_match(tag.name().as_ref()) => (),
            Event::Start(tag) if tag_match(tag.name().as_ref()) => {
                reader.read_to_end(tag.name())?;
            }
            Event::Start(tag) if is_ruby(tag.name()) => {
                ruby_depth += 1;
                writer.write_event(Event::Start(tag))?;
            }
            Event::End(tag) if is_ruby(tag.name()) => {
                ruby_depth = ruby_depth.saturating_sub(1);
                writer.write_event(Event::End(tag))?;
            }
            Event::GeneralRef(entity) => {
                let text = resolve_entity(&entity)?;
                writer.write_event(Event::Text(BytesText::from_escaped(text)))?;
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

#[cfg(test)]
mod test {
    use super::strip_tags;

    #[test]
    fn strips_ruby_readings() {
        let html = "<p><ruby>漢字<rp>(</rp><rt>かんじ</rt><rp>)</rp></ruby>を読む</p>";
        assert_eq!(strip_tags(html).unwrap(), "<p><ruby>漢字</ruby>を読む</p>");
    }

    #[test]
    fn trims_ruby_whitespace() {
        let html = "<p><ruby>\n  漢字\n  <rt>かんじ</rt>\n</ruby> を読む</p>";
        assert_eq!(strip_tags(html).unwrap(), "<p><ruby>漢字</ruby> を読む</p>");
    }

    #[test]
    fn keeps_entity_spacing_in_ruby() {
        let html = "<p><ruby> A &amp; B <rt>x</rt></ruby> &lt;</p>";
        assert_eq!(
            strip_tags(html).unwrap(),
            "<p><ruby>A &amp; B</ruby> &lt;</p>"
        );
    }

    #[test]
    fn trims_nested_ruby() {
        let html = "<ruby><ruby>漢<rt>かん</rt></ruby>\n  字\n<rt>じ</rt></ruby>";
        assert_eq!(strip_tags(html).unwrap(), "<ruby><ruby>漢</ruby>字</ruby>");
    }

    #[test]
    fn resolves_entities() {
        let html = "<p>A&nbsp;&amp;&#x3042;&#12354;&hellip;&bogus;</p>";
        assert_eq!(strip_tags(html).unwrap(), "<p>A\u{a0}&amp;ああ…&bogus;</p>");
        let markdown = html2md::rewrite_html(&strip_tags(html).unwrap(), false);
        assert_eq!(markdown, "A &ああ…&bogus;");
    }
}
