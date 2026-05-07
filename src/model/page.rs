use crate::{actions::contains_japanese, model::Activity};
use iced::{
    Element,
    alignment::Horizontal,
    widget::{Column, container, right, text},
};
use rig::message::Message;
use serde::{Deserialize, Serialize};
use std::{ffi::OsStr, path::PathBuf};

const SIZE_TOLERANCE: usize = 1000;
const SIZE_FLOOR: usize = 5000;
const SECTION_CAPACITY: usize = 8 * 1024;

#[non_exhaustive]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Page {
    pub path: PathBuf,
    pub sections: Vec<Section>,
    #[serde(skip)]
    pub activity: Activity,
    #[serde(skip)]
    pub size_error: Vec<usize>,
    #[serde(skip)]
    pub jap_error: Vec<usize>,
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
            size_error: Vec::new(),
            jap_error: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.sections.iter_mut().for_each(|e| e.content.clear());
        self.jap_error.clear();
        self.size_error.clear();
    }

    pub fn file_stem(&self) -> Option<&OsStr> {
        self.path.file_stem()
    }

    pub fn check_incomplete(&self) -> bool {
        self.sections.iter().any(|e| e.content.is_empty())
    }

    pub fn check_japanese(&self) -> Vec<usize> {
        self.sections
            .iter()
            .enumerate()
            .filter(|(_, e)| contains_japanese(&e.content))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn check_size(&self) -> Vec<usize> {
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
                    return count > max;
                }
                count < SIZE_FLOOR || count > max || count < min
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn check_page(&mut self) {
        self.size_error = self.check_size();
        self.jap_error = self.check_japanese();

        self.activity = if self.check_incomplete() {
            Activity::Incomplete
        } else if let Some(i) = self.size_error.first() {
            Activity::Error(i + 1)
        } else if let Some(i) = self.jap_error.first() {
            Activity::Error(i + 1)
        } else {
            Activity::Complete
        };
    }

    pub fn error_cards<T: 'static>(&self) -> Element<'_, T> {
        let current_sections = self
            .sections
            .iter()
            .enumerate()
            .filter(|(_, s)| s.content.is_empty())
            .map(|(i, _)| text!("Empty part: {:2}", i + 1));
        let errors = self
            .jap_error
            .iter()
            .map(|i| text!("Japanese error: {:2}", i + 1));

        let errors = self
            .size_error
            .iter()
            .map(|i| text!("Size error: {:2}", i + 1))
            .chain(errors)
            .chain(current_sections)
            .map(|e| container(e).padding(5).style(container::primary).into())
            .collect::<Column<_>>();

        right(errors.spacing(5).align_x(Horizontal::Right))
            .padding(20)
            .into()
    }
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
