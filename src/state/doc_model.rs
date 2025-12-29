use epub::doc::EpubDoc;
use std::io::Cursor;

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct DocModel {
    pub epub: Option<EpubDoc<Cursor<Vec<u8>>>>,
    pub file_name: Option<String>,
    pub current_page: Option<usize>,
    pub total_pages: usize,
    pub content: String,
}
