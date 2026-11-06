use crate::{
    page::{DisplayType, Page},
    server::Server,
};
use std::{borrow::Cow, path::PathBuf};

#[derive(Default, Debug)]
pub struct Translation {
    pub server: Server,
    pub file_path: Option<PathBuf>,
    pub glossary: Option<(PathBuf, String)>,
    pub current_page: usize,
    pub pages: Vec<Page>,
    pub display: DisplayType,
}

impl Translation {
    pub fn current_page(&self) -> Option<&Page> {
        self.pages.get(self.current_page)
    }

    pub fn epub_name(&self) -> Option<Cow<'_, str>> {
        Some(self.file_path.as_ref()?.file_name()?.to_string_lossy())
    }

    pub fn glossary_name(&self) -> Option<Cow<'_, str>> {
        Some(self.glossary.as_ref()?.0.file_name()?.to_string_lossy())
    }

    pub fn can_translate(&self) -> bool {
        self.server.handles.is_empty() && self.server.connected() && self.file_path.is_some()
    }
}

impl From<Server> for Translation {
    fn from(server: Server) -> Self {
        Self {
            server,
            ..Default::default()
        }
    }
}
