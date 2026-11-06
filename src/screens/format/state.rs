use crate::epub::{EpubMetadata, FormatPage};
use iced::widget::image::Handle;
use rbook::Epub;
use std::path::PathBuf;

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct Format {
    pub pages: Vec<FormatPage>,
    pub source_folder: String,
    pub epub_path: PathBuf,
    pub epub: Option<Epub>,
    pub cover: Option<Handle>,
    pub metadata: EpubMetadata,
}
