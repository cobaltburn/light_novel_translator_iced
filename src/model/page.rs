use crate::{actions::contains_japanese, model::Activity};
use std::path::PathBuf;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Page {
    pub path: PathBuf,
    pub sections: Vec<Section>,
    pub activity: Activity,
}

#[derive(Debug, Clone)]
pub struct Section {
    pub japanese: String,
    pub content: String,
}

impl Page {
    pub fn new(path: PathBuf, sections: Vec<String>) -> Self {
        let sections = sections
            .into_iter()
            .map(|japanese| Section {
                japanese,
                content: String::new(),
            })
            .collect();

        Page {
            activity: Activity::Incomplete,
            path,
            sections,
        }
    }

    pub fn clear_content(&mut self) {
        self.sections.iter_mut().for_each(|e| e.content.clear());
    }

    pub fn check_japanese(&self) -> Option<usize> {
        self.sections
            .iter()
            .position(|e| contains_japanese(&e.content))
    }

    pub fn check_size(&self) -> Option<usize> {
        let sections = &self.sections[..self.sections.len() - 1];

        let lengths: Vec<_> = sections
            .iter()
            .map(|e| e.content.len())
            .filter(|e| e != &0)
            .collect();

        if lengths.is_empty() || lengths.len() != sections.len() {
            return None;
        }

        let mid = lengths.len() / 2;
        let mut lens = lengths.clone();
        let (_, median, _) = lens.select_nth_unstable(mid);

        lengths
            .into_iter()
            .enumerate()
            .find(|&(_, count)| count > (*median + 1000) || count < median.saturating_sub(1000))
            .map(|(i, _)| i)
    }

    pub fn check_incomplete(&self) -> bool {
        self.sections.iter().any(|e| e.content.is_empty())
    }
}
