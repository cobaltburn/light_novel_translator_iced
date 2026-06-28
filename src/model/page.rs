use crate::{actions::contains_japanese, model::Activity, view::DisplayType};
use iced::{
    Color, Element,
    alignment::Horizontal,
    color,
    widget::{Button, Column, button, right, span, text},
};
use phf::phf_map;
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use rig_core::message::Message;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ffi::OsStr,
    iter,
    ops::Not,
    path::PathBuf,
};

const JACCARD_TOLERANCE: f64 = 0.25;
const FREQUENCY_TOLERANCE: f64 = 10.0;
const SECTION_CAPACITY: usize = 8 * 1024;
const MIN_PERCENT: f64 = 70.0;
const MAX_PERCENT: f64 = 100.0;

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
        let sections = sections
            .into_iter()
            .map(|japanese| Section::new(japanese))
            .collect();

        Page {
            path,
            sections,
            ..Default::default()
        }
    }

    pub fn clear(&mut self) {
        self.sections.iter_mut().for_each(|e| e.content.clear());
        self.errors.clear();
    }

    pub fn file_stem(&self) -> Option<&OsStr> {
        self.path.file_stem()
    }

    pub fn active(&self) -> bool {
        matches!(self.activity, Activity::Active)
    }

    pub fn check_incomplete(&self) -> bool {
        self.sections.iter().any(|e| e.content.is_empty())
    }

    pub fn check_japanese(&self) -> Vec<PageError> {
        self.sections
            .iter()
            .enumerate()
            .filter(|(_, e)| contains_japanese(&e.content))
            .map(|(i, _)| PageError::Japanese(i))
            .collect()
    }

    pub fn check_frequency(&self) -> Vec<PageError> {
        self.sections
            .par_iter()
            .enumerate()
            .filter(|(_, e)| check_char_frequency(&e.content))
            .map(|(i, _)| PageError::Repeat(i))
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
                let (min, max) = if i == last {
                    (MIN_PERCENT - 5.0, MAX_PERCENT + 5.0)
                } else {
                    (MIN_PERCENT, MAX_PERCENT)
                };
                let valid = p > min && p < max;
                valid.not().then_some(PageError::Size(i))
            })
            .collect()
    }

    fn check_jaccard(&self, last_section: &str) -> Vec<PageError> {
        let sections = self.sections.iter().map(|s| s.content.as_str());
        let full_sections: Vec<_> = iter::once(last_section).chain(sections).collect();
        self.sections
            .par_iter()
            .map(|s| s.content.as_str())
            .enumerate()
            .filter_map(|(i, a)| {
                if a.is_empty() {
                    return None;
                }
                full_sections
                    .par_iter()
                    .enumerate()
                    .filter(|&(j, b)| i + 1 != j && !b.is_empty())
                    .any(|(_, b)| jaccard(a, b) > JACCARD_TOLERANCE)
                    .then_some(PageError::Copy(i))
            })
            .collect()
    }

    pub fn check_page(&mut self, last_section: &str) {
        self.errors = [
            self.check_size(),
            self.check_japanese(),
            self.check_frequency(),
            self.check_jaccard(last_section),
        ]
        .concat();

        self.activity = if let Some(error) = self.errors.first() {
            Activity::Error(error.index() + 1)
        } else if self.check_incomplete() {
            Activity::Incomplete
        } else {
            Activity::Complete
        };
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

        let errors = empty_sections
            .chain(errors)
            .map(Into::into)
            .collect::<Column<_>>();

        right(errors.spacing(5).align_x(Horizontal::Right))
            .padding(20)
            .into()
    }

    pub fn spans<Link: 'static + Clone>(
        &self,
        display: DisplayType,
        on_press: impl Fn(usize) -> Option<Link> + 'static,
    ) -> Vec<text::Span<'_, Link>> {
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
                            let highlight = end != ',' && end.is_ascii_punctuation();
                            spans.push(
                                span(end)
                                    .color_maybe(highlight.then_some(Color::BLACK))
                                    .background_maybe(highlight.then_some(color!(0xffff00))),
                            );
                        }
                    }
                    DisplayType::Full | DisplayType::Japanese => spans.push(span(content)),
                }

                spans
            })
            .collect()
    }
}

fn jaccard(a: &str, b: &str) -> f64 {
    let set_a: HashSet<&str> = a.split_whitespace().collect();
    let set_b: HashSet<&str> = b.split_whitespace().collect();
    let intersection = set_a.intersection(&set_b).count() as f64;
    let union = set_a.union(&set_b).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
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
}

#[derive(Debug, Clone)]
pub enum PageError {
    Japanese(usize),
    Size(usize),
    Repeat(usize),
    Copy(usize),
}

impl PageError {
    pub fn index(&self) -> usize {
        match self {
            PageError::Japanese(i)
            | PageError::Size(i)
            | PageError::Repeat(i)
            | PageError::Copy(i) => *i,
        }
    }

    pub fn error_button<T: 'static + Clone>(
        &self,
        on_press: &impl Fn(usize) -> Option<T>,
    ) -> Button<'_, T> {
        let make_btn = |label: String, i: usize| {
            button(text(label))
                .padding(5)
                .style(button::primary)
                .on_press_maybe(on_press(i))
        };

        match self {
            PageError::Japanese(i) => make_btn(format!("Japanese error: {:2}", i + 1), *i),
            PageError::Size(i) => make_btn(format!("Size error: {:2}", i + 1), *i),
            PageError::Repeat(i) => make_btn(format!("Repeat error: {:2}", i + 1), *i),
            PageError::Copy(i) => make_btn(format!("Copy error: {:2}", i + 1), *i),
        }
    }
}
