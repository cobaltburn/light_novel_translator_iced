use crate::error::Result;
use quick_xml::{
    Reader,
    escape::{escape, resolve_html5_entity},
    events::{BytesRef, Event},
};
use std::fmt::Write as _;

/// html2md only decodes the predefined XML entities, so numeric and HTML5 references
/// are resolved to their characters here. The result is escaped text; unknown
/// entities are kept as-is.
pub fn resolve_entity(entity: &BytesRef<'_>) -> Result<String> {
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

#[cfg(test)]
mod test {
    use super::text_content;

    #[test]
    fn joins_text_split_at_entities() {
        let html = "<span>Tom</span> &amp; Jerry&hellip;&#33;&bogus;";
        assert_eq!(text_content(html).unwrap(), "Tom & Jerry…!&bogus;");
    }
}
