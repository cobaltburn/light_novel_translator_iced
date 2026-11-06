use crate::{
    page::{DisplayType, Page},
    server::Server,
};
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct Consensus {
    pub server: Server,
    pub file_path: Option<PathBuf>,
    pub current_page: usize,
    pub candidates: Vec<Candidate>,
    pub pages: Vec<Page>,
    pub display: DisplayType,
}

impl Consensus {
    pub fn current_page(&self) -> Option<&Page> {
        self.pages.get(self.current_page)
    }

    pub fn file_name(&self) -> Option<String> {
        Some(
            self.file_path
                .as_ref()?
                .file_name()?
                .to_string_lossy()
                .to_string(),
        )
    }
}

#[derive(Debug, Default)]
pub struct Candidate {
    pub name: String,
    pub pages: Vec<(PathBuf, Vec<String>)>,
}
