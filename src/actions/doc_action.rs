use crate::{
    controller::parse::{convert_html, join_partition, partition_text},
    message::{Message, display_error, open_epub},
    model::doc::Doc,
};
use epub::doc::EpubDoc;
use iced::Task;
use std::io::Cursor;

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum DocAction {
    SetEpub(String, Vec<u8>),
    OpenEpub,
    SetPage(usize),
    Inc,
    Dec,
}

impl Doc {
    pub fn perform(&mut self, action: DocAction) -> Task<Message> {
        match action {
            DocAction::OpenEpub => Task::future(open_epub())
                .and_then(|(name, buf)| Task::done(DocAction::SetEpub(name, buf).into())),
            DocAction::SetEpub(file_name, buffer) => self.set_epub(file_name, buffer),
            DocAction::SetPage(page) => self.set_page(page).into(),
            DocAction::Inc => self.inc_page().into(),
            DocAction::Dec => self.dec_page().into(),
        }
    }

    pub fn inc_page(&mut self) {
        if let Some(page) = self.current_page {
            let page = page + 1;
            if page < self.total_pages {
                self.set_page(page);
            }
        }
    }

    pub fn dec_page(&mut self) {
        let Some(page) = self.current_page else {
            return;
        };
        if let Some(page) = page.checked_sub(1) {
            self.set_page(page);
        }
    }

    pub fn set_epub(&mut self, file_name: String, buffer: Vec<u8>) -> Task<Message> {
        let epub = match EpubDoc::from_reader(Cursor::new(buffer)) {
            Ok(epub) => epub,
            Err(error) => return Task::future(display_error(error)).discard(),
        };
        self.current_page = Some(0);
        self.total_pages = epub.get_num_chapters();
        self.file_name = Some(file_name);

        self.epub = Some(epub);
        self.set_page(0);

        Task::none()
    }

    pub fn get_page(&mut self, page: usize) -> Option<String> {
        let epub = self.epub.as_mut()?;
        epub.set_current_chapter(page);
        let html = epub.get_current_str()?.0;
        let markdown = convert_html(&html).unwrap();
        let parts = partition_text(&markdown);
        Some(join_partition(parts))
    }

    pub fn set_page(&mut self, page: usize) {
        if let Some(content) = self.get_page(page) {
            self.current_page = Some(page);
            self.content = content;
        }
    }
}
