use crate::{actions::contains_japanese, model::Activity};
use iced::{
    Element,
    alignment::Horizontal,
    widget::{Button, Column, button, right, text},
};
use phf::phf_map;
use rig_core::message::Message;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ffi::OsStr, path::PathBuf};

const SIZE_TOLERANCE: usize = 1000;
const SIZE_FLOOR: usize = 5000;
const SIZE_MAX: usize = 9000;
const SECTION_CAPACITY: usize = 8 * 1024;

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
            activity: Activity::Incomplete,
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
            .iter()
            .enumerate()
            .filter(|(_, e)| check_char_frequency(&e.content))
            .map(|(i, _)| PageError::Frequency(i))
            .collect()
    }

    pub fn check_size(&self) -> Vec<PageError> {
        let Some((last, sections)) = self.sections.split_last() else {
            return Vec::new();
        };
        if sections.is_empty() {
            return Vec::new();
        }

        let mut lengths: Vec<_> = sections.iter().map(|e| e.content.len()).collect();
        let mid = lengths.len() / 2;
        let mut sorted = lengths.clone();
        let (_, median, _) = sorted.select_nth_unstable(mid);
        let median = *median;

        let max = median + SIZE_TOLERANCE;
        let min = median.saturating_sub(SIZE_TOLERANCE);

        lengths.push(last.content.len());
        let last_index = lengths.len() - 1;

        lengths
            .into_iter()
            .enumerate()
            .filter(|&(i, count)| {
                if count == 0 {
                    return false;
                }
                if i == last_index {
                    return count > max || count > SIZE_MAX;
                }
                count < SIZE_FLOOR || count > SIZE_MAX || count > max || count < min
            })
            .map(|(i, _)| PageError::Size(i))
            .collect()
    }

    pub fn check_page(&mut self) {
        let check_size = self.check_size();
        let check_japanese = self.check_japanese();
        let check_frequency = self.check_frequency();

        self.activity = if self.check_incomplete() {
            Activity::Incomplete
        } else if let Some(PageError::Size(i)) = check_size.first() {
            Activity::Error(i + 1)
        } else if let Some(PageError::Japanese(i)) = check_japanese.first() {
            Activity::Error(i + 1)
        } else if let Some(PageError::Frequency(i)) = check_frequency.first() {
            Activity::Error(i + 1)
        } else {
            Activity::Complete
        };

        self.errors = [check_size, check_japanese, check_frequency].concat();
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

        let errors = errors
            .chain(empty_sections)
            .map(Into::into)
            .collect::<Column<_>>();

        right(errors.spacing(5).align_x(Horizontal::Right))
            .padding(20)
            .into()
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

const FREQUENCY_TOLERANCE: f64 = 10.0;

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
            Message::user(self.japanese.clone()),
            Message::assistant(self.content.clone()),
        ]
    }
}

#[derive(Debug, Clone)]
pub enum PageError {
    Japanese(usize),
    Size(usize),
    Frequency(usize),
}

impl PageError {
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
            PageError::Frequency(i) => make_btn(format!("Frequency error: {:2}", i + 1), *i),
        }
    }
}
