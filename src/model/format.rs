use epub::doc::EpubDoc;
use iced::widget::text_editor::{self, Content};
use std::{io::Cursor, path::PathBuf};

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct Format {
    pub pages: Vec<FormatPage>,
    pub current_page: Option<usize>,
    pub source_folder: String,
    pub epub_name: String,
    pub epub: Option<EpubDoc<Cursor<Vec<u8>>>>,
}

impl Format {
    pub fn current_content(&self) -> Option<&Content> {
        let page = self.current_page?;
        self.pages.get(page).map(|e| &e.content)
    }
}

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct FormatPage {
    pub path: PathBuf,
    pub content: text_editor::Content,
}

impl<S: AsRef<str>> From<(PathBuf, S)> for FormatPage {
    fn from((path, content): (PathBuf, S)) -> Self {
        FormatPage {
            path,
            content: Content::with_text(content.as_ref()),
        }
    }
}
