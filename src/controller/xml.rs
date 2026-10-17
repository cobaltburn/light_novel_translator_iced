use crate::{
    controller::{PageLinks, image_marker},
    error::{Error, Result},
};
use pulldown_cmark::{Options, Parser, html::push_html};
use quick_xml::{
    Reader, Writer, XmlVersion,
    escape::{escape, resolve_html5_entity},
    events::{BytesRef, BytesStart, BytesText, Event},
    name::QName,
};
use regex::Regex;
use std::{borrow::Cow, fmt::Write as _, io::Cursor};

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

const SYOSETU_AFTERWORD: &str = "--afterword";
const SYOSETU_PREFACE: &str = "--preface";
const SYOSETU_ATTRIBUTES: &[&str] = &[SYOSETU_PREFACE, SYOSETU_AFTERWORD];

fn contains_author_notes(tag: &BytesStart<'_>) -> bool {
    tag.try_get_attribute("class")
        .ok()
        .flatten()
        .is_some_and(|a| SYOSETU_ATTRIBUTES.iter().any(|e| a.value.contains(e)))
}

const RUBY_TAG: &str = "ruby";
const HEAD_TAG: &str = "head";

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

/// html2md only decodes the predefined XML entities, so numeric and HTML5 references
/// are resolved to their characters here. The result is escaped text; unknown
/// entities are kept as-is.
fn resolve_entity(entity: &BytesRef<'_>) -> Result<String> {
    Ok(match entity_text(entity)? {
        Some(text) => escape(text).into_owned(),
        None => format!("&{};", entity.html_content()),
    })
}

/// Resolves a numeric or HTML5 entity reference to its unescaped text.
fn entity_text(entity: &BytesRef<'_>) -> Result<Option<String>> {
    if let Some(ch) = entity.resolve_char_ref()? {
        return Ok(Some(ch.to_string()));
    }
    Ok(resolve_html5_entity(&entity.html_content()).map(str::to_string))
}

/// Joins the text pieces that the reader splits at every entity reference.
#[derive(Default)]
pub struct TextRun(String);

impl TextRun {
    /// Appends a text or entity event to the run. Returns false for any other event.
    pub fn push(&mut self, event: &Event<'_>) -> Result<bool> {
        match event {
            Event::Text(text) => self.0.push_str(&text.html_content()),
            Event::GeneralRef(entity) => match entity_text(entity)? {
                Some(text) => self.0.push_str(&text),
                None => {
                    let _ = write!(self.0, "&{};", entity.html_content());
                }
            },
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Ends the run, returning its trimmed text unless it is blank.
    pub fn finish(&mut self) -> Option<String> {
        let text = self.0.trim().to_string();
        self.0.clear();
        (!text.is_empty()).then_some(text)
    }
}

/// Returns the text of the html, with entities resolved and inline tags removed.
pub fn text_content(html: &str) -> Result<String> {
    let mut reader = Reader::from_str(html);
    let mut text = TextRun::default();
    loop {
        match reader.read_event()? {
            Event::Eof => break,
            event => {
                text.push(&event)?;
            }
        }
    }
    Ok(text.finish().unwrap_or_default())
}

pub const HEADER_TAGS: &[&str] = &["h1", "h2", "h3", "h4"];

pub fn is_header(name: QName<'_>) -> bool {
    HEADER_TAGS.contains(&name.as_ref())
}

pub fn is_ruby(name: QName<'_>) -> bool {
    name.as_ref() == RUBY_TAG
}

pub const SRC: &str = "src";
pub const XLINK: &str = "xlink:href";
pub const IMG_TAG: &str = "img";
pub const IMAGE_TAG: &str = "image";

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

pub const LINK_TAG: &str = "link";
pub const SCRIPT_TAG: &str = "script";

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

pub fn extract_head(html: &str) -> Result<Cow<'_, str>> {
    let mut reader = Reader::from_str(html);
    loop {
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == HEAD_TAG => {
                return Ok(reader.read_text(tag.name())?.html_content());
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
            Event::Start(tag) if tag.name().as_ref() == HEAD_TAG => {
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
    name.as_ref() == IMG_TAG || name.as_ref() == IMAGE_TAG
}

pub fn image_anchors(
    html: &str,
    links: PageLinks<'_>,
) -> Result<Vec<(BytesStart<'static>, Option<String>)>> {
    let html = strip_syosetu_tags(html)?;
    let mut reader = Reader::from_str(&html);
    let mut anchors = vec![];
    let mut pending = vec![];
    let mut run = TextRun::default();

    loop {
        let event = reader.read_event()?;
        if run.push(&event)? {
            continue;
        }
        if let Some(text) = run.finish()
            && !pending.is_empty()
        {
            anchors.extend(pending.drain(..).map(|tag| (tag, Some(text.clone()))));
        }

        match event {
            Event::Start(tag) if tag.name().as_ref() == HEAD_TAG => {
                reader.read_to_end(tag.name())?;
            }
            Event::Empty(tag) if is_image(&tag) => {
                let attr = if tag.name().as_ref() == IMG_TAG {
                    SRC
                } else {
                    XLINK
                };
                pending.push(update_tag_path(tag, links, IMAGES, attr)?);
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
    use super::{strip_tags, text_content, to_html};

    #[test]
    fn joins_text_split_at_entities() {
        let html = "<span>Tom</span> &amp; Jerry&hellip;&#33;&bogus;";
        assert_eq!(text_content(html).unwrap(), "Tom & Jerry…!&bogus;");
    }

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
