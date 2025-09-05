use crate::{
    controller::{
        doc::get_ordered_path,
        markdown::{convert_html, join_partition, partition_text},
    },
    message::Message,
    state::{
        doc_model::DocModel,
        server_state::ServerState,
        translation_model::{Page, TranslationModel},
    },
    view::View,
};
use epub::doc::EpubDoc;
use iced::Task;
use std::io::Cursor;

#[non_exhaustive]
#[derive(Default)]
pub struct Translator {
    pub view: View,
    pub epub: Option<EpubDoc<Cursor<Vec<u8>>>>,
    pub doc_model: DocModel,
    pub translation_model: TranslationModel,
    pub server_state: ServerState,
}

impl Translator {
    pub fn get_page(&mut self, page: usize) -> Option<String> {
        let epub = self.epub.as_mut()?;
        epub.set_current_page(page);
        let html = epub.get_current_str()?.0;
        let markdown = convert_html(&html).unwrap();
        let parts = partition_text(&markdown);
        Some(join_partition(parts))
    }

    pub fn execute_translation(&mut self, page: usize) -> Task<Message> {
        let Some(model) = self.server_state.current_model.clone() else {
            return Task::none();
        };
        let Some(current_page) = self.translation_model.pages.get_mut(page) else {
            return Task::done(Message::Abort);
        };

        current_page.content.clear();
        let server = self.server_state.server.clone();

        let (task, handle) = Task::future(server.translate(model))
            .and_then(move |stream| {
                Task::run(stream, move |response| match response {
                    Ok(msg) => Message::UpdateTranslation(msg.message.content, page),
                    Err(_) => {
                        log::error!("Failed to read stream");
                        Message::None
                    }
                })
            })
            .abortable();

        let (next_task, next_handle) = Task::done(Message::Translate(page + 1)).abortable();
        self.translation_model.handles = Some((handle, next_handle));

        task.chain(next_task)
    }

    pub fn select_page(&mut self, page: usize) -> Task<Message> {
        if let Some(content) = self.get_page(page) {
            self.doc_model.current_page = Some(page);
            self.doc_model.content = content;
        }
        Task::none()
    }

    pub fn set_file(&mut self, doc: Option<(String, EpubDoc<Cursor<Vec<u8>>>)>) -> Task<Message> {
        match doc {
            Some((file_name, epub)) => {
                self.doc_model.current_page = Some(0);
                self.doc_model.total_pages = epub.get_num_pages();
                self.doc_model.path = Some(file_name);
                self.epub = Some(epub);
                self.translation_model.current_page = Some(0);
                let paths = get_ordered_path(self.epub.as_mut().unwrap());
                self.translation_model.pages =
                    paths.into_iter().map(|path| Page::new(path)).collect();
                Task::done(Message::SelectPage(0))
            }
            None => Task::none(),
        }
    }

    pub fn set_view(&mut self, view: View) -> Task<Message> {
        self.view = view;
        Task::none()
    }
}
