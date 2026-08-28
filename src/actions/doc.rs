use crate::{
    controller::{join_partition, partition_text, strip_syosetu_tags, strip_tags},
    error::{Result, ResultTaskExt},
    message::select_epub,
    model::Doc,
};
use epub::doc::EpubDoc;
use html2md::rewrite_html;
use iced::Task;
use std::{io::Cursor, path::PathBuf};

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    SetEpub(PathBuf, Vec<u8>),
    OpenEpub,
    SetPage(usize),
    Inc,
    Dec,
}

impl Doc {
    pub fn perform(&mut self, action: Action) -> Task<Action> {
        match action {
            Action::OpenEpub => Task::future(select_epub())
                .and_then(|(name, buf)| Task::done(Action::SetEpub(name, buf))),
            Action::SetEpub(file_name, buffer) => self.set_epub(file_name, buffer).ok_or_display(),
            Action::SetPage(page) => self.set_page(page).into(),
            Action::Inc => self.inc_page().into(),
            Action::Dec => self.dec_page().into(),
        }
    }

    pub fn inc_page(&mut self) {
        if let Some(page) = self.current_page.map(|p| p + 1)
            && page < self.total_pages
        {
            self.set_page(page);
        }
    }

    pub fn dec_page(&mut self) {
        if let Some(page) = self.current_page.and_then(|p| p.checked_sub(1)) {
            self.set_page(page);
        };
    }

    pub fn set_epub(&mut self, file_name: PathBuf, buffer: Vec<u8>) -> Result<()> {
        let epub = EpubDoc::from_reader(Cursor::new(buffer))?;
        self.current_page = Some(0);
        self.total_pages = epub.get_num_chapters();

        self.file_name = file_name
            .file_name()
            .map(|e| e.to_string_lossy().to_string());

        self.epub = Some(epub);
        self.set_page(0);
        Ok(())
    }

    pub fn get_page(&mut self, page: usize) -> Option<String> {
        let epub = self.epub.as_mut()?;
        epub.set_current_chapter(page);
        let html = epub.get_current_str()?.0;
        let html = strip_syosetu_tags(&html).ok()?;
        let html = strip_tags(&html).ok()?;
        let markdown = rewrite_html(&html, false);
        let lines = markdown.lines();
        let markdown = lines.map(|s| format!("{}\n", s.trim())).collect::<String>();
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
