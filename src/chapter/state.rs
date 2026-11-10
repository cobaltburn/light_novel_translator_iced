use std::{ffi::OsStr, path::PathBuf};

#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq)]
pub enum Activity {
    #[default]
    Incomplete,
    Complete,
    Active,
}

#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct Chapter {
    pub path: PathBuf,
    pub text: String,
    pub content: String,
    pub activity: Activity,
}

impl Chapter {
    pub fn new(path: PathBuf, text: String) -> Self {
        Chapter {
            path,
            text,
            ..Default::default()
        }
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
}
