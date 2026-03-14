use epub::doc::EpubDoc;
use iced::widget::image::Handle;
use std::{io::Cursor, path::PathBuf};

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct Format {
    pub pages: Vec<FormatPage>,
    pub source_folder: String,
    pub epub_path: PathBuf,
    pub epub: Option<EpubDoc<Cursor<Vec<u8>>>>,
    pub cover: Option<Handle>,
    pub metadata: EpubMetadata,
}

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct FormatPage {
    pub path: PathBuf,
    pub content: String,
}

impl From<(PathBuf, String)> for FormatPage {
    fn from((path, content): (PathBuf, String)) -> Self {
        FormatPage {
            path,
            content: content,
        }
    }
}

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct EpubMetadata {
    pub title: String,
    pub authors: String,
}
