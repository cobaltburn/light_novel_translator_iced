use crate::{
    actions::{clean_invisible_chars, contains_japanese, is_japanese_char},
    controller::{IMAGE_MARKER_RE, TOC_PAGE_PREFIX, image_marker_indices},
    model::Activity,
    view::DisplayType,
};
use iced::{
    Color, Element,
    alignment::Horizontal,
    color,
    widget::{Button, Column, button, right, span, text},
};
use phf::phf_map;
use pulldown_cmark::{Event, Parser, Tag};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use rig::message::Message;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ffi::OsStr,
    hash::Hash,
    iter,
    ops::Range,
    path::{Path, PathBuf},
};

const JACCARD_TOLERANCE: f64 = 0.25;
const FREQUENCY_TOLERANCE: f64 = 15.0;
const SECTION_CAPACITY: usize = 8 * 1024;
const MIN_PERCENT: f64 = 65.0;
const MAX_PERCENT: f64 = 110.0;
const SIZE_TOLERANCE: f64 = 12.5;
/// In Japanese characters; full partitions are 8192 bytes (~2700 characters).
const SMALL_SECTION: usize = 1000;
const BOUNDARY_PARAGRAPHS: usize = 5;
const BOUNDARY_MIN_WORDS: usize = 5;
const BOUNDARY_MIN_BIGRAMS: usize = 5;
const BOUNDARY_TOLERANCE: f64 = 0.3;
const BOUNDARY_JAPANESE_TOLERANCE: f64 = 0.5;
const HEAD_MIN_LINES: usize = 2;

#[non_exhaustive]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Page {
    pub path: PathBuf,
    pub sections: Vec<Section>,
    #[serde(skip)]
    pub activity: Activity,
    #[serde(skip)]
    pub errors: Vec<PageError>,
}

impl Page {
    pub fn new(path: PathBuf, sections: Vec<String>) -> Self {
        let sections = sections.into_iter().map(Section::new).collect();
        Page {
            path: absolute_page_path(path),
            sections,
            ..Default::default()
        }
    }

    pub fn clear(&mut self) {
        self.sections.iter_mut().for_each(|e| e.content.clear());
        self.errors.clear();
    }

    pub fn file_prefix(&self) -> Option<&OsStr> {
        self.path.file_prefix()
    }

    pub fn name(&self) -> String {
        self.file_prefix()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }

    pub fn active(&self) -> bool {
        matches!(self.activity, Activity::Active(_))
    }

    pub fn check_incomplete(&self) -> bool {
        self.sections.iter().any(|e| e.content.is_empty())
    }

    fn check_page(&mut self, prior: &[(&Section, &LineSets)], heads: &[LineSets]) {
        self.errors = [
            self.check_size(),
            self.check_japanese(),
            self.check_frequency(),
            self.check_copy(prior, heads),
            self.check_image_tags(),
            self.check_header_tags(),
            self.check_link_tags(),
            self.check_end_character(),
        ]
        .concat();
        self.errors.sort_by_key(|e| e.index);

        self.activity = if let Some(error) = self.errors.first() {
            Activity::Error(error.index + 1)
        } else if self.check_incomplete() {
            Activity::Incomplete
        } else {
            Activity::Complete
        };
    }

    pub fn check_japanese(&self) -> Vec<PageError> {
        self.sections
            .iter()
            .enumerate()
            .filter_map(|(i, e)| contains_japanese(&e.content).then_some(PageError::japanese(i)))
            .collect()
    }

    pub fn check_frequency(&self) -> Vec<PageError> {
        self.sections
            .par_iter()
            .enumerate()
            .filter_map(|(i, e)| check_char_frequency(&e.content).then_some(PageError::repeat(i)))
            .collect()
    }

    pub fn check_size(&self) -> Vec<PageError> {
        let last = self.sections.len() - 1;
        self.sections
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.content.is_empty())
            .filter_map(|(i, Section { japanese, content })| {
                let p = (content.len() as f64 / japanese.len() as f64) * 100.0;
                let (min, max) = if i == last && japanese.chars().count() < SMALL_SECTION {
                    (MIN_PERCENT - SIZE_TOLERANCE, MAX_PERCENT + SIZE_TOLERANCE)
                } else {
                    (MIN_PERCENT, MAX_PERCENT)
                };
                let size_error = p <= min || p >= max;
                size_error.then_some(PageError::size(i, content.len()))
            })
            .collect()
    }

    fn check_copy(&self, prior: &[(&Section, &LineSets)], heads: &[LineSets]) -> Vec<PageError> {
        debug_assert_eq!(self.sections.len(), heads.len());
        let full: Vec<_> = prior
            .iter()
            .copied()
            .chain(self.sections.iter().zip(heads))
            .collect();
        let word_sets: Vec<HashSet<&str>> = full
            .par_iter()
            .map(|(s, _)| s.content.split_whitespace().collect())
            .collect();
        let prior_len = prior.len();
        self.sections
            .par_iter()
            .enumerate()
            .filter(|(_, s)| !s.content.is_empty())
            .filter_map(|(i, _)| {
                let index = prior_len + i;
                let words = &word_sets[index];
                let (_, head) = full[index];
                let duplicate = word_sets[..index]
                    .iter()
                    .any(|b| !b.is_empty() && set_jaccard(words, b) > JACCARD_TOLERANCE);
                let overlap = || {
                    index
                        .checked_sub(1)
                        .map(|previous| full[previous].0)
                        .is_some_and(|p| {
                            !p.content.is_empty() && sections_overlap(&LineSets::tail(p), head)
                        })
                };
                let repeated_head = || {
                    full[..index]
                        .iter()
                        .any(|(p, h)| !p.content.is_empty() && head_repeated(h, head))
                };
                (duplicate || overlap() || repeated_head()).then_some(PageError::copy(i))
            })
            .collect()
    }

    fn check_image_tags(&self) -> Vec<PageError> {
        self.sections
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.content.is_empty())
            .filter_map(|(i, Section { japanese, content })| {
                let japanese_tags = image_marker_indices(japanese);
                let content_tags = image_marker_indices(content);
                let dif = japanese_tags.symmetric_difference(&content_tags).count();
                (dif != 0).then_some(PageError::image(i))
            })
            .collect()
    }

    fn check_header_tags(&self) -> Vec<PageError> {
        self.sections
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.content.is_empty())
            .filter_map(|(i, Section { japanese, content })| {
                let unequal = header_tag_count(&japanese) != header_tag_count(&content);
                unequal.then_some(PageError::header(i))
            })
            .collect()
    }

    fn check_link_tags(&self) -> Vec<PageError> {
        self.sections
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.content.is_empty())
            .filter_map(|(i, Section { japanese, content })| {
                let unequal = link_tag_count(&japanese) != link_tag_count(&content);
                unequal.then_some(PageError::link(i))
            })
            .collect()
    }

    fn check_end_character(&self) -> Vec<PageError> {
        self.sections
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.content.is_empty())
            .filter_map(|(i, Section { japanese, content })| {
                let jp = japanese.trim_end().chars().last()?.is_alphanumeric();
                let c = content.trim_end().chars().last()?.is_alphanumeric();
                (jp != c).then_some(PageError::end(i))
            })
            .collect()
    }

    pub fn error_cards<T: 'static + Clone>(
        &self,
        on_press: impl Fn(usize) -> Option<T> + 'static,
    ) -> Element<'_, T> {
        let make_btn = |label: String, i: usize| {
            button(text(label))
                .padding(5)
                .style(button::primary)
                .on_press_maybe(on_press(i))
        };

        let empty_sections = self
            .sections
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.content.is_empty().then_some(i))
            .map(|i| make_btn(format!("Empty part: {:2}", i + 1), i));

        let errors = self.errors.iter().map(|e| e.error_button(&on_press));
        let errors: Column<'_, T> = empty_sections.chain(errors).map(Into::into).collect();

        right(errors.spacing(5).align_x(Horizontal::Right))
            .padding(20)
            .into()
    }

    pub fn spans<Link: 'static + Clone>(
        &self,
        display: DisplayType,
        on_press: impl Fn(usize) -> Option<Link> + 'static,
    ) -> Vec<text::Span<'_, Link>> {
        let has_japanese_error = |i| {
            self.errors
                .iter()
                .any(|e| e.index == i && matches!(e.error, ErrorType::Japanese))
        };

        self.sections
            .iter()
            .enumerate()
            .flat_map(|(i, section)| {
                let content = section.span_content(display);
                let mut spans = vec![
                    span(format!("\n\nPart: {}\nCount: {}\n\n", i + 1, content.len()))
                        .color(color!(0xff0000))
                        .link_maybe(on_press(i)),
                ];

                match display {
                    DisplayType::End => {
                        let mut content = content.into_owned();
                        let end = content.pop();
                        spans.push(span(content));
                        if let Some(end) = end {
                            let end_span = span(end);
                            spans.push(if end != ',' && end.is_ascii_punctuation() {
                                highlight(end_span)
                            } else {
                                end_span
                            });
                        }
                    }
                    DisplayType::Full if has_japanese_error(i) => {
                        spans.extend(japanese_runs(&section.content).map(|(japanese, run)| {
                            span(run).color_maybe(japanese.then_some(color!(0xff0000)))
                        }));
                    }
                    DisplayType::Full | DisplayType::Japanese => spans.push(span(content)),
                }

                spans
            })
            .collect()
    }
}

pub fn absolute_page_path(path: PathBuf) -> PathBuf {
    if path.is_relative() {
        Path::new("/").join(path)
    } else {
        path
    }
}

pub fn check_page_at(pages: &mut [Page], index: usize) {
    let next = index + 1;
    let check_next = pages
        .get(next)
        .is_some_and(|p| !p.active() && p.sections.iter().any(|s| !s.content.is_empty()));
    check_pages(pages, index..if check_next { next + 1 } else { next });
}

pub fn check_all_pages(pages: &mut [Page]) {
    check_pages(pages, 0..pages.len());
}

fn check_pages(pages: &mut [Page], range: Range<usize>) {
    // Always the heads of `pages[index - 1]`.
    let mut previous_heads = range
        .start
        .checked_sub(1)
        .and_then(|previous| pages.get(previous))
        .map(|p| LineSets::heads(&p.sections))
        .unwrap_or_default();

    for index in range {
        let Some((before, after)) = pages.split_at_mut_checked(index) else {
            return;
        };
        let Some(page) = after.first_mut() else {
            return;
        };
        let heads = LineSets::heads(&page.sections);
        let prior: Vec<_> =
            prior_page(before).map_or_default(|p| p.sections.iter().zip(&previous_heads).collect());
        page.check_page(&prior, &heads);
        previous_heads = heads;
    }
}

fn prior_page(before: &[Page]) -> Option<&Page> {
    before
        .last()
        .filter(|p| p.file_prefix().is_some_and(|f| f != TOC_PAGE_PREFIX))
}

fn japanese_runs(text: &str) -> impl Iterator<Item = (bool, &str)> {
    let mut rest = text;
    iter::from_fn(move || {
        let japanese = is_japanese_char(rest.chars().next()?);
        let end = rest
            .find(|c| is_japanese_char(c) != japanese)
            .unwrap_or(rest.len());
        let (run, tail) = rest.split_at(end);
        rest = tail;
        Some((japanese, run))
    })
}

fn highlight<Link>(span: text::Span<'_, Link>) -> text::Span<'_, Link> {
    span.color(Color::BLACK).background(color!(0xffff00))
}

fn set_jaccard<T: Eq + Hash>(a: &HashSet<T>, b: &HashSet<T>) -> f64 {
    let intersection = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

/// The word set of the English and bigram set of the Japanese for the lines at
/// one end of a section, each compared as a whole by the copy checks.
struct LineSets {
    english: HashSet<String>,
    japanese: HashSet<(char, char)>,
    /// English lines that went into `english`.
    lines: usize,
}

impl LineSets {
    fn heads(sections: &[Section]) -> Vec<Self> {
        sections.par_iter().map(Self::head).collect()
    }

    fn head(section: &Section) -> Self {
        Self::new(section.content.lines(), section.japanese.lines())
    }

    fn tail(section: &Section) -> Self {
        Self::new(
            section.content.lines().rev(),
            section.japanese.lines().rev(),
        )
    }

    fn new<'a>(
        english: impl Iterator<Item = &'a str>,
        japanese: impl Iterator<Item = &'a str>,
    ) -> Self {
        let (lines, english) = line_set(english, BOUNDARY_PARAGRAPHS, BOUNDARY_MIN_WORDS, word_set);
        let (_, japanese) = line_set(
            japanese,
            BOUNDARY_PARAGRAPHS * 2,
            BOUNDARY_MIN_BIGRAMS,
            bigram_set,
        );
        Self {
            english,
            japanese,
            lines,
        }
    }
}

fn sections_overlap(prev: &LineSets, next: &LineSets) -> bool {
    set_jaccard(&prev.english, &next.english) > BOUNDARY_TOLERANCE
        && set_jaccard(&prev.japanese, &next.japanese) <= BOUNDARY_JAPANESE_TOLERANCE
}

fn head_repeated(earlier: &LineSets, next: &LineSets) -> bool {
    earlier.lines >= HEAD_MIN_LINES && sections_overlap(earlier, next)
}

fn line_set<'a, T: Eq + Hash>(
    lines: impl Iterator<Item = &'a str>,
    n: usize,
    min_size: usize,
    to_set: impl Fn(&str) -> HashSet<T>,
) -> (usize, HashSet<T>) {
    lines
        .map(|line| to_set(IMAGE_MARKER_RE.replace_all(line, "").trim()))
        .filter(|set| set.len() >= min_size)
        .take(n)
        .fold((0, HashSet::new()), |(count, mut union), set| {
            union.extend(set);
            (count + 1, union)
        })
}

fn word_set(line: &str) -> HashSet<String> {
    line.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

fn bigram_set(line: &str) -> HashSet<(char, char)> {
    line.chars().zip(line.chars().skip(1)).collect()
}

static LETTER_FREQUENCY: phf::Map<char, f64> = phf_map! {
    'z'=> 0.074,
    'q'=> 0.12,
    'x'=> 0.15,
    'j'=> 0.16,
    'k'=> 0.77,
    'v'=> 0.98,
    'b'=> 1.5,
    'p'=> 1.9,
    'g'=> 2.0,
    'y'=> 2.0,
    'f'=> 2.2,
    'm'=> 2.4,
    'w'=> 2.4,
    'c'=> 2.8,
    'u'=> 2.8,
    'l'=> 4.0,
    'd'=> 4.3,
    'r'=> 6.0,
    'h'=> 6.1,
    's'=> 6.3,
    'n'=> 6.7,
    'i'=> 7.0,
    'o'=> 7.5,
    'a'=> 8.2,
    't'=> 9.1,
    'e'=> 12.7,
};

fn check_char_frequency(text: &str) -> bool {
    let char_count: HashMap<char, usize> = text
        .to_lowercase()
        .chars()
        .filter(|e| e.is_alphabetic())
        .fold(HashMap::with_capacity(32), |mut acc, e| {
            *acc.entry(e).or_insert(0) += 1;
            acc
        });

    let total: usize = char_count.values().sum();

    char_count
        .into_iter()
        .map(|(ch, count)| (ch, (count as f64 / total as f64) * 100.0))
        .any(|(ch, percent)| {
            LETTER_FREQUENCY
                .get(&ch)
                .is_some_and(|frequency| percent > frequency + FREQUENCY_TOLERANCE)
        })
}

fn header_tag_count(text: &str) -> usize {
    Parser::new(text)
        .filter(|e| matches!(e, Event::Start(Tag::Heading { .. })))
        .count()
}

fn link_tag_count(text: &str) -> usize {
    Parser::new(text)
        .filter(|e| matches!(e, Event::Start(Tag::Link { .. })))
        .count()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Section {
    pub japanese: String,
    pub content: String,
}

impl Section {
    pub fn new(japanese: String) -> Self {
        Self {
            japanese,
            content: String::with_capacity(SECTION_CAPACITY),
        }
    }

    pub fn history_message(&self) -> [Message; 2] {
        [
            Message::user(&self.japanese),
            Message::assistant(&self.content),
        ]
    }

    pub fn span_content(&self, display: DisplayType) -> Cow<'_, str> {
        match display {
            DisplayType::End => {
                let lines: Vec<_> = self.content.lines().collect();
                lines
                    .get(lines.len().saturating_sub(10)..)
                    .unwrap_or_default()
                    .join("\n")
                    .into()
            }
            DisplayType::Full => self.content.as_str().into(),
            DisplayType::Japanese => self.japanese.as_str().into(),
        }
    }

    pub fn clean(&mut self) {
        self.content = clean_invisible_chars(&self.content);
        self.content = self.content.replace(['“', '”'], "\"");
        self.split_lines();
    }

    fn split_lines(&mut self) {
        let lines: Vec<_> = self.content.lines().collect();
        let mut text = Vec::new();
        for [l1, l2] in lines.array_windows() {
            text.push(*l1);
            if !l1.is_empty() && !l2.is_empty() {
                text.push("");
            }
        }

        if let Some(last) = lines.last() {
            text.push(last);
        };

        self.content = text.join("\n")
    }
}

#[derive(Debug, Clone)]
pub struct PageError {
    pub index: usize,
    pub error: ErrorType,
}

impl PageError {
    pub fn japanese(index: usize) -> Self {
        PageError {
            index,
            error: ErrorType::Japanese,
        }
    }
    pub fn size(index: usize, size: usize) -> Self {
        PageError {
            index,
            error: ErrorType::Size(size),
        }
    }
    pub fn repeat(index: usize) -> Self {
        PageError {
            index,
            error: ErrorType::Repeat,
        }
    }
    pub fn copy(index: usize) -> Self {
        PageError {
            index,
            error: ErrorType::Copy,
        }
    }
    pub fn image(index: usize) -> Self {
        PageError {
            index,
            error: ErrorType::Image,
        }
    }
    pub fn header(index: usize) -> Self {
        PageError {
            index,
            error: ErrorType::Header,
        }
    }
    pub fn link(index: usize) -> Self {
        PageError {
            index,
            error: ErrorType::Link,
        }
    }
    pub fn end(index: usize) -> Self {
        PageError {
            index,
            error: ErrorType::End,
        }
    }

    pub fn error_button<T: 'static + Clone>(
        &self,
        on_press: &impl Fn(usize) -> Option<T>,
    ) -> Button<'_, T> {
        let make_btn = |label: String| {
            button(text(label))
                .padding(5)
                .style(button::primary)
                .on_press_maybe(on_press(self.index))
        };

        let position = self.index + 1;
        match self.error {
            ErrorType::Japanese => make_btn(format!("Japanese error: {:2}", position)),
            ErrorType::Size(c) => make_btn(format!("Size error: {:2}, count:{}", position, c)),
            ErrorType::Repeat => make_btn(format!("Repeat error: {:2}", position)),
            ErrorType::Copy => make_btn(format!("Copy error: {:2}", position)),
            ErrorType::Image => make_btn(format!("Image Tag error: {:2}", position)),
            ErrorType::Header => make_btn(format!("Header Tag error: {:2}", position)),
            ErrorType::Link => make_btn(format!("Link Tag error: {:2}", position)),
            ErrorType::End => make_btn(format!("End Punctuation error: {:2}", position)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ErrorType {
    Japanese,
    Size(usize),
    Repeat,
    Copy,
    Image,
    Header,
    Link,
    End,
}

#[cfg(test)]
mod test {
    use super::{
        ErrorType, LineSets, Page, Section, absolute_page_path, check_all_pages, check_page_at,
        head_repeated, sections_overlap,
    };
    use std::{path::PathBuf, slice};

    fn boundary_overlap(prev: &Section, next: &Section) -> bool {
        sections_overlap(&LineSets::tail(prev), &LineSets::head(next))
    }

    fn head_overlap(earlier: &Section, next: &Section) -> bool {
        head_repeated(&LineSets::head(earlier), &LineSets::head(next))
    }

    #[test]
    fn rechecking_page_updates_next_page() {
        let tail = "On his thick wrist gleamed a resplendent golden guild ring.";
        let mut pages = vec![
            Page::new(
                "p-001.xhtml".into(),
                vec!["その太い手首には、黄金に輝くギルドリングが光っていた。".into()],
            ),
            Page::new(
                "p-002.xhtml".into(),
                vec!["受付の奥から現れた男は、こちらを値踏みするように見下ろしてきた。".into()],
            ),
        ];
        pages[0].sections[0].content = tail.into();
        pages[1].sections[0].content = tail.into();
        let has_copy = |p: &Page| p.errors.iter().any(|e| matches!(e.error, ErrorType::Copy));

        check_page_at(&mut pages, 1);
        assert!(has_copy(&pages[1]));

        pages[0].sections[0].content =
            "The man who appeared from behind the counter looked down at me.".into();
        check_page_at(&mut pages, 0);
        assert!(!has_copy(&pages[1]));
    }

    #[test]
    fn page_paths_are_absolute() {
        let toc = Page::new(PathBuf::from("__toc__.xhtml"), Vec::new());
        assert_eq!(toc.path, PathBuf::from("/__toc__.xhtml"));
        assert_eq!(
            absolute_page_path("/OEBPS/text/p-001.xhtml".into()),
            PathBuf::from("/OEBPS/text/p-001.xhtml")
        );
    }

    fn section(japanese: &str, content: &str) -> Section {
        Section {
            japanese: japanese.into(),
            content: content.into(),
        }
    }

    fn page(sections: Vec<Section>) -> Page {
        Page {
            sections,
            ..Default::default()
        }
    }

    #[test]
    fn detects_retranslated_short_dialogue() {
        let prev = section(
            "ハーデン侯爵は口の端を大きく吊り上げて、意地の悪い笑みを浮かべた。\n「そういった狙いでございましたか。感服いたしました」",
            "Marquis Harden gave a malicious smile.\n\n\"So that was your aim. I am in awe.\"",
        );
        let next = section(
            "「……とはいえアイザス伯は、密偵など警戒もせんだろうがね」",
            "\"So that was your aim. I am in awe.\"\n\n\"...That said, Count Aizas likely isn't wary of spies at all.\"",
        );
        assert!(boundary_overlap(&prev, &next));
    }

    #[test]
    fn scene_break_does_not_hide_overlap() {
        let prev = section(
            "その太い手首には、黄金に輝くギルドリングが光っていた。\n◇◇◇◇◇◇◇◇◇◇",
            "On his thick wrist gleamed a resplendent golden guild ring.\n\n◇◇◇◇◇◇◇◇◇◇",
        );
        let next = section(
            "◇◇◇◇◇◇◇◇◇◇\n受付の奥から現れた男は、こちらを値踏みするように見下ろしてきた。",
            "◇◇◇◇◇◇◇◇◇◇\n\nOn his thick wrist gleamed a resplendent golden guild ring.",
        );
        assert!(boundary_overlap(&prev, &next));
    }

    #[test]
    fn image_markers_do_not_hide_overlap() {
        let prev = section(
            "その太い手首には、黄金に輝くギルドリングが光っていた。\n[[IMG: 3]]",
            "On his thick wrist gleamed a resplendent golden guild ring.\n\n\\[\\[IMG: 3\\]\\]",
        );
        let next = section(
            "[[IMG: 4]]\n受付の奥から現れた男は、こちらを値踏みするように見下ろしてきた。",
            "\\[\\[IMG: 4\\]\\]\n\nOn his thick wrist gleamed a resplendent golden guild ring.",
        );
        assert!(boundary_overlap(&prev, &next));
    }

    #[test]
    fn detects_retranslated_status_line() {
        let prev = section(
            "【スキル獲得：剣術熟練度レベル３】",
            "[Skill acquired: Sword Mastery Lv. 3]",
        );
        let next = section(
            "受付の奥から現れた男は、こちらを値踏みするように見下ろしてきた。",
            "[Skill acquired: Sword Mastery Lv. 3]\n\nThe man who appeared from behind the counter looked down at me.",
        );
        assert!(boundary_overlap(&prev, &next));
    }

    #[test]
    fn small_section_uses_wider_size_range() {
        // 900 characters / 2700 bytes of Japanese against 1650 bytes of English is 61%.
        let japanese = "あ".repeat(900);
        let content = "a".repeat(1650);
        let small = page(vec![section(&japanese, &content)]);
        assert!(small.check_size().is_empty());

        let japanese = "あ".repeat(1200);
        let content = "a".repeat(2200);
        let large = page(vec![section(&japanese, &content)]);
        assert_eq!(large.check_size().len(), 1);
    }

    #[test]
    fn duplicate_and_overlap_report_one_copy_error() {
        let text = "On his thick wrist gleamed a resplendent golden guild ring.";
        let mut page = page(vec![
            section(
                "その太い手首には、黄金に輝くギルドリングが光っていた。",
                text,
            ),
            section(
                "受付の奥から現れた男は、こちらを値踏みするように見下ろしてきた。",
                text,
            ),
        ]);
        check_all_pages(slice::from_mut(&mut page));
        let copies: Vec<_> = page
            .errors
            .iter()
            .filter(|e| matches!(e.error, ErrorType::Copy))
            .map(|e| e.index)
            .collect();
        assert_eq!(copies, [1]);
    }

    #[test]
    fn detects_retranslated_tail() {
        let prev = section(
            "ハーデン侯爵は口の端を大きく吊り上げて、意地の悪い笑みを浮かべた。\n「そういった狙いでございましたか。感服いたしました」",
            "Marquis Harden curled up the corners of his mouth broadly and gave a malicious smile.\n\n\"So that was your aim. I am in awe.\"",
        );
        let next = section(
            "「……とはいえアイザス伯は、密偵など警戒もせんだろうがね」\nハーデン侯爵は会談でのアイザス伯爵の様子を思い出し、深い溜息を吐いた。",
            "Marquis Harden curled up the corners of his mouth broadly and gave a malicious smile.\n\n\"...That said, Count Aizas likely isn't wary of spies at all.\"",
        );
        assert!(boundary_overlap(&prev, &next));
    }

    #[test]
    fn detects_reworded_retranslation() {
        let prev = section(
            "その太い手首には、黄金に輝くギルドリングが光っていた。",
            "On his thick wrist gleamed a resplendent golden guild ring. No explanation needed, it was the very proof of his rank.",
        );
        let next = section(
            "受付の奥から現れた男は、こちらを値踏みするように見下ろしてきた。",
            "On his thick wrist shone a brilliantly golden guild ring. No explanation needed, it was the proof of his rank.",
        );
        assert!(boundary_overlap(&prev, &next));
    }

    #[test]
    fn ignores_lines_repeated_in_japanese() {
        let prev = section(
            "俺がそう言うと先輩はゆっくりと頷いた、そして少し考え込んだ。",
            "When I say that, Senpai slowly nods, and then falls into thought for a little while.",
        );
        let next = section(
            "俺がそう言うと先輩はゆっくりと頷いた、そして少し考え込んだ。",
            "When I say that, Senpai slowly nods, and then falls into thought for a little while.",
        );
        assert!(!boundary_overlap(&prev, &next));
    }

    #[test]
    fn detects_short_section_repeated_later() {
        let short =
            "Marquis Harden gave a malicious smile.\n\n\"So that was your aim. I am in awe.\"";
        let repeated = format!(
            "{short}\n\n\
             The man who appeared from behind the counter looked down at me appraisingly.\n\n\
             On his thick wrist gleamed a resplendent golden guild ring, proof of his rank.\n\n\
             I want to observe the relationship between Count Aizas and Erma-kun carefully.\n\n\
             Snow had begun falling over the capital while nobody inside noticed anything strange.\n\n\
             Her voice trembled slightly, yet she refused to lower her gaze from his face."
        );
        let mut page = page(vec![
            section(
                "ハーデン侯爵は意地の悪い笑みを浮かべた。\n「そういった狙いでございましたか。感服いたしました」",
                short,
            ),
            section(
                "二次試験の結果が発表され、試験を受けた七四八二人中五五八人が合格した。",
                "The results of the second exam were announced, and of the 7,482 people who took the exam, 558 passed.",
            ),
            section(
                "受付の奥から現れた男は、こちらを値踏みするように見下ろしてきた。\nアイザス伯とエルマ君の関係をしっかりと見ておきたいのだ。",
                &repeated,
            ),
        ]);
        check_all_pages(slice::from_mut(&mut page));
        let copies: Vec<_> = page
            .errors
            .iter()
            .filter(|e| matches!(e.error, ErrorType::Copy))
            .map(|e| e.index)
            .collect();
        assert_eq!(copies, [2]);
    }

    #[test]
    fn ignores_heads_repeated_in_japanese() {
        let earlier = section(
            "俺がそう言うと先輩はゆっくりと頷いた、そして少し考え込んだ。",
            "When I say that, Senpai slowly nods, and then falls into thought for a little while.",
        );
        let next = section(
            "俺がそう言うと先輩はゆっくりと頷いた、そして少し考え込んだ。\n二次試験の結果が発表された。",
            "When I say that, Senpai slowly nods, and then falls into thought for a little while.\n\nThe results of the second exam were announced.",
        );
        assert!(!head_overlap(&earlier, &next));
    }

    #[test]
    fn ignores_head_with_one_compared_line() {
        let earlier = section(
            "ハーデン侯爵は口の端を大きく吊り上げて、意地の悪い笑みを浮かべた。\n「はい」",
            "Marquis Harden gave a malicious smile.\n\n\"Yes.\"",
        );
        let next = section(
            "ハーデン侯爵は意地悪く笑った。\n二次試験の結果が発表され、試験を受けた七四八二人中五五八人が合格した。",
            "Marquis Harden gave a malicious smile.\n\nThe results of the second exam were announced, and of the 7,482 people who took the exam, 558 passed.",
        );
        assert!(!head_overlap(&earlier, &next));
    }

    #[test]
    fn ignores_single_shared_opening_line() {
        let earlier = section(
            "ハーデン侯爵は口の端を大きく吊り上げて、意地の悪い笑みを浮かべた。\n「そういった狙いでございましたか。感服いたしました」",
            "Marquis Harden gave a malicious smile.\n\n\"So that was your aim. I am in awe.\"",
        );
        let next = section(
            "ハーデン侯爵は意地悪く笑った。\n二次試験の結果が発表され、試験を受けた七四八二人中五五八人が合格した。",
            "Marquis Harden gave a malicious smile.\n\nThe results of the second exam were announced, and of the 7,482 people who took the exam, 558 passed.",
        );
        assert!(!head_overlap(&earlier, &next));
    }

    #[test]
    fn ignores_unrelated_sections() {
        let prev = section(
            "アイザス伯とエルマ君の関係をしっかりと見ておきたいのだ。",
            "I want to observe the relationship between Count Aizas and Erma-kun carefully.",
        );
        let next = section(
            "二次試験の結果が発表され、試験を受けた七四八二人中五五八人が合格した。",
            "The results of the second exam were announced, and of the 7,482 people who took the exam, 558 passed.",
        );
        assert!(!boundary_overlap(&prev, &next));
    }
}
