use crate::{controller::xml::remove_think_tags, message::Message};
use iced::Task;
use std::path::PathBuf;
use tokio::fs;

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct TranslationModel {
    pub content: &'static str,
    pub current_page: Option<usize>,
    pub pages: Vec<Page>,
    pub file_drop_down: bool,
}

impl TranslationModel {
    pub fn update_content(&mut self, text: String, page: usize) {
        if let Some(page) = self.pages.get_mut(page) {
            page.content.push_str(&text);
        }
    }

    pub fn current_content(&self) -> Option<&str> {
        Some(&self.pages.get(self.current_page?)?.content)
    }

    pub fn set_current_page(&mut self, page: usize) {
        self.current_page = Some(page);
    }

    pub fn mark_complete(&mut self, page: usize) {
        if let Some(page) = self.pages.get_mut(page) {
            page.complete = true;
        }
    }

    pub fn save_pages(&mut self, path: PathBuf) -> Task<Message> {
        let tasks = self
            .pages
            .iter()
            .filter(|page| page.complete)
            .filter(|page| !page.content.is_empty())
            .map(|page| {
                let name = page.path.file_stem().unwrap().to_os_string();
                (name, page.content.clone())
            })
            .map(|(name, text)| {
                let text = remove_think_tags(&text);
                (name, text)
            })
            .map(|(name, text)| {
                let path = path.clone();
                Task::future(async move {
                    let file_path = path.join(name).with_extension("md");
                    _ = fs::write(file_path, text.as_bytes())
                        .await
                        .inspect_err(|err| log::error!("{}", err));
                })
            });
        Task::batch(tasks).discard()
    }

    pub fn perform(&mut self, action: TransAction) -> Task<Message> {
        match action {
            TransAction::SetPage(page) => self.set_current_page(page).into(),
            TransAction::UpdateContent(text, page) => self.update_content(text, page).into(),
            TransAction::PageComplete(page) => self.mark_complete(page).into(),
            TransAction::SavePages(path) => self.save_pages(path),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum TransAction {
    SetPage(usize),
    UpdateContent(String, usize),
    PageComplete(usize),
    SavePages(PathBuf),
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Page {
    pub path: PathBuf,
    pub content: String,
    pub complete: bool,
}

impl Page {
    pub fn new(path: PathBuf) -> Self {
        Page {
            path,
            content: String::new(),
            complete: false,
        }
    }
}
