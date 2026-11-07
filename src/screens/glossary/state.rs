use crate::server::Server;
use std::{collections::VecDeque, path::PathBuf};

#[derive(Default, Debug)]
pub struct Glossary {
    pub server: Server,
    pub file_path: Option<PathBuf>,
    pub chapters: Vec<String>,
    /// Chapters the current run has not sent yet.
    pub pending: VecDeque<String>,
    /// The glossary from the last pass that finished.
    pub completed: String,
    pub content: String,
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
}
