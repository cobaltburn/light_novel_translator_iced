use crate::{actions::contains_japanese, model::Activity};
use std::{ffi::OsStr, iter, path::PathBuf};

#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct Page {
    pub path: PathBuf,
    pub sections: Vec<Section>,
    pub activity: Activity,
    pub size_error: Vec<usize>,
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
        let sections = &self.sections[..self.sections.len() - 1];

        let lengths: Vec<_> = sections.iter().map(|e| e.content.len()).collect();

        if lengths.is_empty() {
            return lengths;
        }

        let mid = lengths.len() / 2;
        let mut lens = lengths.clone();
        let (_, median, _) = lens.select_nth_unstable(mid);

        let max = *median + 1000;
        let min = median.saturating_sub(1000);
        let base = 5000;

        lengths
            .into_iter()
            .enumerate()
            .filter(|&(_, e)| e != 0)
            .filter_map(|(i, count)| (count < base || count > max || count < min).then_some(i))
            .chain(iter::from_fn(|| {
                if self.sections.last()?.content.len() > max {
                    Some(self.sections.len() - 1)
                } else {
                    None
                }
            }))
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Section {
    pub japanese: String,
    pub content: String,
}

impl Section {
    pub fn new(japanese: String) -> Self {
        Self {
            japanese,
            content: String::new(),
        }
    }
}
