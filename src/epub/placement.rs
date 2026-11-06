//! Works out where toc ids and images belong in the translated text. The source
//! and the translation differ in lines and sections, so positions are matched
//! by title or estimated.

use crate::{
    epub::{
        anchors::{Anchor, AnchorPosition},
        html::{TextRun, html_to_markdown, strip_syosetu_tags, strip_tags},
    },
    error::Result,
    text::{content_sections, markdown_sections},
};
use quick_xml::{
    Reader, XmlVersion,
    events::{BytesStart, Event},
};
use std::collections::{HashMap, HashSet};

const TITLE_MATCH: f64 = 0.6;

/// How far, in lines, a heading may be from the estimated position and still be used.
const HEADING_WINDOW: usize = 120;

struct TranslatedLine<'a> {
    section: usize,
    line: usize,
    text: &'a str,
}

/// Places the toc ids in the translated content, in document order. An id goes on the
/// line matching its translated toc title, else on the heading nearest its estimated
/// position, else on the estimate. Estimates use the share of non-blank lines before the
/// id in the whole source page, since translation merges and splits paragraphs and the
/// source may no longer split into the same sections it was translated in.
pub fn place_ids(
    html: &str,
    content: &str,
    toc_ids: &[String],
    titles: &HashMap<String, String>,
) -> Result<Vec<AnchorPosition>> {
    let positions = id_positions(html, toc_ids)?;
    let sections = content_sections(content).collect::<Vec<_>>();
    let lines: Vec<_> = sections
        .iter()
        .enumerate()
        .flat_map(|(section, text)| {
            text.lines()
                .enumerate()
                .map(move |(line, text)| TranslatedLine {
                    section,
                    line,
                    text,
                })
        })
        .collect();
    let filled: Vec<_> = (0..lines.len())
        .filter(|&i| !lines[i].text.trim().is_empty())
        .collect();
    if filled.is_empty() {
        return Ok(positions);
    }

    let source = section_markdown(html)?;
    let is_filled = |l: &&str| !l.trim().is_empty();
    let offsets: Vec<_> = source
        .iter()
        .scan(0, |total, s| {
            let offset = *total;
            *total += s.lines().filter(is_filled).count();
            Some(offset)
        })
        .collect();
    let source_total = source
        .iter()
        .map(|s| s.lines().filter(is_filled).count())
        .sum::<usize>();
    let section_lens: Vec<_> = sections.iter().map(|s| s.lines().count()).collect();

    let mut cursor = 0;
    let positions = positions
        .into_iter()
        .map(|position| {
            let Anchor::Id(id) = &position.anchor else {
                return position;
            };
            let before = source
                .get(position.section)
                .map_or_default(|s| s.lines().take(position.line).filter(is_filled).count());
            let share = (offsets.get(position.section).unwrap_or(&0) + before) as f64
                / source_total.max(1) as f64;
            let estimate = filled[((share * filled.len() as f64) as usize).min(filled.len() - 1)];

            let found = titles
                .get(id)
                .and_then(|title| find_title(&lines, title, cursor))
                .or_else(|| find_heading(&lines, cursor, estimate));
            if let Some(i) = found {
                cursor = i + 1;
            }
            let TranslatedLine { section, line, .. } = lines[found.unwrap_or(estimate)];
            AnchorPosition {
                position: line as f64 / section_lens[section] as f64,
                section,
                line,
                ..position
            }
        })
        .collect();

    Ok(positions)
}

pub fn id_positions(html: &str, toc_ids: &[String]) -> Result<Vec<AnchorPosition>> {
    let sections = section_markdown(html)?;
    let sections: Vec<Vec<_>> = sections.iter().map(|s| s.lines().collect()).collect();
    let html = strip_tags(&strip_syosetu_tags(html)?)?;

    let mut cursor = (0, 0);
    let positions = match_ids(&html, toc_ids)?
        .into_iter()
        .filter_map(|(id, text)| {
            let found = find_line(&sections, &text, cursor);
            if let Some((section, line, _)) = found {
                cursor = (section, line + 1);
            }
            let (section, line, position) =
                found.or_else(|| find_line(&sections, &text, (0, 0)))?;
            Some(AnchorPosition {
                anchor: Anchor::Id(id),
                section,
                line,
                position,
            })
        })
        .collect();

    Ok(positions)
}

pub fn image_positions(
    html: &str,
    images: Vec<(BytesStart<'static>, Option<String>)>,
) -> Result<Vec<AnchorPosition>> {
    let sections = section_markdown(html)?;
    let sections: Vec<Vec<_>> = sections.iter().map(|s| s.lines().collect()).collect();
    let last_section = sections.len().saturating_sub(1);

    let positions = images
        .into_iter()
        .map(|(tag, text)| {
            let found = text.and_then(|text| find_line(&sections, &text, (0, 0)));
            let (section, line, position) = found.unwrap_or((last_section, usize::MAX, 1.0));
            AnchorPosition {
                anchor: Anchor::Image(tag),
                section,
                line,
                position,
            }
        })
        .collect();

    Ok(positions)
}

pub fn match_ids(html: &str, toc_ids: &[String]) -> Result<Vec<(String, String)>> {
    let mut reader = Reader::from_str(html);
    let ids: HashSet<_> = toc_ids.iter().map(|e| e.as_str()).collect();

    let mut pairs = Vec::new();
    let mut pending = Vec::new();
    let mut run = TextRun::default();
    loop {
        let event = reader.read_event()?;
        if run.push(&event)? {
            continue;
        }
        if let Some(text) = run.finish()
            && !pending.is_empty()
        {
            pairs.extend(pending.drain(..).map(|id| (id, text.clone())));
        }

        match event {
            Event::Start(tag) => {
                if let Some(attr) = tag.try_get_attribute("id")?
                    && ids.contains(attr.value.as_ref())
                {
                    let id = attr.normalized_value(XmlVersion::Implicit1_0)?.to_string();
                    let text = reader.read_text(tag.name())?.into_inner();
                    match next_text(text.as_ref())? {
                        Some(text) => pairs.push((id, text)),
                        None => pending.push(id),
                    }
                }
            }
            Event::Empty(tag) => {
                if let Some(attr) = tag.try_get_attribute("id")?
                    && ids.contains(attr.value.as_ref())
                {
                    pending.push(attr.normalized_value(XmlVersion::Implicit1_0)?.to_string());
                }
            }
            Event::Eof => break,
            _ => (),
        }
    }

    Ok(pairs)
}

fn next_text(text: &str) -> Result<Option<String>> {
    let mut reader = Reader::from_str(text);
    let mut run = TextRun::default();

    loop {
        let event = reader.read_event()?;
        if run.push(&event)? {
            continue;
        }
        if let Some(text) = run.finish() {
            return Ok(Some(text));
        }
        if let Event::Eof = event {
            return Ok(None);
        }
    }
}

fn section_markdown(html: &str) -> Result<Vec<String>> {
    Ok(markdown_sections(&html_to_markdown(html)?))
}

fn find_line(
    sections: &[Vec<&str>],
    text: &str,
    (start_section, start_line): (usize, usize),
) -> Option<(usize, usize, f64)> {
    sections
        .iter()
        .enumerate()
        .skip(start_section)
        .find_map(|(section, lines)| {
            let skip = if section == start_section {
                start_line
            } else {
                0
            };
            let line = lines.iter().skip(skip).position(|l| l.contains(text))? + skip;
            Some((section, line, line as f64 / lines.len() as f64))
        })
}

/// Index of the line from `start` on that best matches `title`, earliest on ties.
fn find_title(lines: &[TranslatedLine<'_>], title: &str, start: usize) -> Option<usize> {
    let title = title_words(title);
    let mut best = None;
    let mut best_score = TITLE_MATCH;
    for (i, line) in lines.iter().enumerate().skip(start) {
        let score = title_similarity(&title, line.text);
        if score > best_score || (best.is_none() && score == best_score) {
            best = Some(i);
            best_score = score;
        }
    }
    best
}

/// Index of the heading from `start` on that is closest to `estimate`.
fn find_heading(lines: &[TranslatedLine<'_>], start: usize, estimate: usize) -> Option<usize> {
    let from = start.max(estimate.saturating_sub(HEADING_WINDOW));
    let to = lines.len().min(estimate + HEADING_WINDOW + 1);
    (from..to)
        .filter(|&i| looks_like_heading(lines[i].text))
        .min_by_key(|&i| i.abs_diff(estimate))
}

/// Lowercased words, with a trailing plural `s` dropped so "Flowers" matches "Flower's".
fn title_words(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| {
            let w = w.to_lowercase();
            match w.strip_suffix('s') {
                Some(stem) if stem.len() > 2 => stem.to_string(),
                _ => w,
            }
        })
        .filter(|w| w != "s")
        .collect()
}

/// Dice coefficient of the word sets, so long prose lines that mention the title score low.
fn title_similarity(title: &HashSet<String>, line: &str) -> f64 {
    let line = title_words(line);
    if title.is_empty() || line.is_empty() {
        return 0.0;
    }
    let shared = title.intersection(&line).count();
    2.0 * shared as f64 / (title.len() + line.len()) as f64
}

/// A short line without sentence punctuation, like a chapter title, or a markdown heading.
fn looks_like_heading(line: &str) -> bool {
    let line = line.trim();
    if line.starts_with('#') {
        return true;
    }
    let opens_like_prose = line.starts_with(['"', '“', '‘', '\'', '(', '「', '『', '<', '[']);
    let ends_like_prose = line.ends_with([
        '.', '!', '?', '"', '”', '’', '\'', '…', '—', '–', ')', ':', ',', ';', '」', '』',
    ]);
    line.chars().any(char::is_alphabetic)
        && line.split_whitespace().count() <= 20
        && !opens_like_prose
        && !ends_like_prose
}

#[cfg(test)]
mod test {
    use super::{Anchor, id_positions, looks_like_heading, match_ids, place_ids};
    use std::collections::HashMap;

    #[test]
    fn matches_ids_on_empty_anchors() {
        let html = r#"<body><p><a id="toc-003"/><span><span>プロローグ</span></span></p><p><a id="toc-002"/>　<span>剣と魔法</span></p><h1 id="toc-004">＃１</h1></body>"#;
        let ids = ["toc-002", "toc-003", "toc-004"].map(String::from);
        assert_eq!(
            match_ids(html, &ids).unwrap(),
            [
                ("toc-003", "プロローグ"),
                ("toc-002", "剣と魔法"),
                ("toc-004", "＃１")
            ]
            .map(|(id, text)| (id.to_string(), text.to_string()))
        );
    }

    #[test]
    fn places_repeated_titles_once_in_order() {
        let html = r#"<html><head></head><body><p id="toc-1">閑話</p><p>本文</p><p id="toc-2">見送り</p><p>本文</p><p id="toc-3">閑話</p><p>本文</p></body></html>"#;
        let ids = ["toc-1", "toc-2", "toc-3"].map(String::from);
        let positions: Vec<_> = id_positions(html, &ids)
            .unwrap()
            .into_iter()
            .map(|p| match p.anchor {
                Anchor::Id(id) => (id, p.section, p.line),
                Anchor::Image(_) => unreachable!(),
            })
            .collect();
        assert_eq!(
            positions,
            [("toc-1", 0, 0), ("toc-2", 0, 2), ("toc-3", 0, 4)].map(|(id, section, line)| (
                id.to_string(),
                section,
                line
            ))
        );
    }

    #[test]
    fn places_ids_on_translated_headings() {
        let html = r#"<html><head></head><body><p id="c1">第一章</p><p>あ。</p><p>い。</p><p>う。</p><p id="c2">第二章</p><p>え。</p></body></html>"#;
        // The translation split paragraphs, so the source position of c2 lands on prose
        let content = "<part>1</part>\n\nChapter One\n\nA.\n\nB.\n\nC.\n\nD.\n\nE.\n\nF.\n\nThe Second Chapter\n\nG.\n";
        let ids = ["c1", "c2"].map(String::from);
        let place = |titles: &HashMap<String, String>| -> Vec<_> {
            place_ids(html, content, &ids, titles)
                .unwrap()
                .into_iter()
                .map(|p| (p.section, p.line))
                .collect()
        };

        // Without titles, the nearest heading-like line is used
        assert_eq!(place(&HashMap::new()), [(0, 2), (0, 16)]);
        // A loosely matching translated toc title wins over the estimate
        let titles = HashMap::from([("c2".to_string(), "Second Chapters".to_string())]);
        assert_eq!(place(&titles), [(0, 2), (0, 16)]);
    }

    #[test]
    fn detects_headings() {
        assert!(looks_like_heading(
            "Side Story: The Witch of Ordine and the Black Fan"
        ));
        assert!(looks_like_heading("## Chapter 3"));
        assert!(!looks_like_heading("He bowed and said to her:"));
        assert!(!looks_like_heading("“Alright, I’m off! Fabiola!”"));
        assert!(!looks_like_heading("The time goes back slightly—"));
        assert!(!looks_like_heading("◆◆◆"));
    }

    #[test]
    fn matches_ids_with_entities() {
        let html =
            r#"<body><h2 id="c1">A &amp; B&#x3042; Saga</h2><p><a id="c2"/>Tom &lt;3</p></body>"#;
        let ids = ["c1", "c2"].map(String::from);
        assert_eq!(
            match_ids(html, &ids).unwrap(),
            [("c1", "A & Bあ Saga"), ("c2", "Tom <3")]
                .map(|(id, text)| (id.to_string(), text.to_string()))
        );
    }
}
