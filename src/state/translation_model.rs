use crate::message::Message;
use iced::{Task, task::Handle};
use std::path::PathBuf;

#[non_exhaustive]
#[derive(Default)]
pub struct TranslationModel {
    pub content: String,
    pub handles: Option<(Handle, Handle)>,
    pub current_page: Option<usize>,
    pub pages: Vec<Page>,
}

impl TranslationModel {
    pub fn update_content(&mut self, text: String, page: usize) -> Task<Message> {
        if let Some(page) = self.pages.get_mut(page) {
            page.content.push_str(&text);
        }
        Task::none()
    }

    pub fn current_content(&self) -> Option<&str> {
        Some(&self.pages.get(self.current_page?)?.content)
    }

    pub fn begin_translation(&mut self) -> Task<Message> {
        let Some(page) = self.current_page else {
            return Task::none();
        };

        return Task::done(Message::Translate(page));
    }

    pub fn set_current_page(&mut self, page: usize) -> Task<Message> {
        self.current_page = Some(page);
        Task::none()
    }

    pub fn abort_tranlation(&mut self) -> Task<Message> {
        if let Some((handle, next_handle)) = &self.handles {
            next_handle.abort();
            handle.abort();
            self.handles = None;
        }
        Task::none()
    }
}

#[non_exhaustive]
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
