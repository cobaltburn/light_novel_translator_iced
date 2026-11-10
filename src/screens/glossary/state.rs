use crate::{chapter::Chapter, server::Server};
use std::{collections::VecDeque, path::PathBuf};

#[derive(Default, Debug)]
pub struct Glossary {
    pub server: Server,
    pub file_path: Option<PathBuf>,
    pub current_page: usize,
    pub chapters: Vec<Chapter>,
    /// Chapters the current run has not sent yet.
    pub pending: VecDeque<usize>,
}

impl Glossary {
    pub fn file_name(&self) -> Option<String> {
        Some(
            self.file_path
                .as_ref()?
                .file_name()?
                .to_string_lossy()
                .to_string(),
        )
    }
    pub fn file_stem(&self) -> Option<String> {
        Some(
            self.file_path
                .as_ref()?
                .file_stem()?
                .to_string_lossy()
                .to_string(),
        )
    }

    pub fn current_content(&self) -> Option<&str> {
        self.chapters
            .get(self.current_page)
            .map(|c| c.content.as_str())
    }
}
