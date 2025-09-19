use crate::{
    controller::{
        doc::get_ordered_path,
        markdown::{convert_html, join_partition, partition_text},
    },
    message::Message,
    state::{
        doc_model::DocModel,
        format_model::FormatModel,
        server_state::ServerState,
        translation_model::{Page, TransAction, TranslationModel},
    },
    view::View,
};
use epub::doc::EpubDoc;
use iced::{Task, futures::StreamExt};
use std::{io::Cursor, ops::Not};

#[non_exhaustive]
#[derive(Default, Debug)]
pub struct Translator {
    pub view: View,
    pub epub: Option<EpubDoc<Cursor<Vec<u8>>>>,
    pub side_bar_collapsed: bool,
    pub doc_model: DocModel,
    pub translation_model: TranslationModel,
    pub server_state: ServerState,
    pub format_model: FormatModel,
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
        let Some(model) = &self.server_state.current_model else {
            return Task::none();
        };

        let Some(current_page) = self.translation_model.pages.get_mut(page) else {
            return Task::done(TransAction::Abort.into());
        };

        let Some(epub) = self.epub.as_mut() else {
            return Task::none();
        };

        current_page.content.clear();
        epub.set_current_page(page);
        let html = epub.get_current_str().expect("max page excepted").0;

        let markdown = match convert_html(&html) {
            Ok(markdown) => markdown,
            Err(error) => {
                log::error!("{}", error);
                return Task::none();
            }
        };

        if markdown.is_empty() {
            let mark_task = Task::done(TransAction::PageComplete(page).into());
            let next_task = Task::done(Message::Translate(page + 1));
            return mark_task.chain(next_task);
        }

        let partitioned = partition_text(&markdown);
        let sections = partitioned.chunks(3).enumerate();

        let context = self.translation_model.context.text();
        let context = context.is_empty().not().then_some(context);

        let mut task = Task::none();
        for (i, section) in sections {
            let server = self.server_state.server.clone();
            let tag = format!("\n\n<part>{}</part>\n\n", i + 1);
            let (tag_task, tag_handle) =
                Task::done(TransAction::UpdateContent(tag, page).into()).abortable();

            let capacity: usize = 5;
            let (trans_task, handle) =
                Task::future(server.translate(model.clone(), section.to_owned(), context.clone()))
                    .and_then(move |stream| {
                        Task::run(stream.chunks(capacity), move |responses| {
                            let mut content: Vec<String> = Vec::with_capacity(capacity);
                            for response in responses {
                                match response {
                                    Ok(msg) => content.push(msg.message.content),
                                    Err(_) => {
                                        log::error!("Failed to read stream");
                                        return TransAction::Abort.into();
                                    }
                                }
                            }
                            TransAction::UpdateContent(content.join(""), page).into()
                        })
                    })
                    .abortable();

            task = task.chain(tag_task).chain(trans_task);
            self.translation_model.handles.push(handle);
            self.translation_model.handles.push(tag_handle);
        }

        let (mark_task, mark_handle) =
            Task::done(TransAction::PageComplete(page).into()).abortable();
        let (next_task, next_handle) = Task::done(Message::Translate(page + 1)).abortable();

        self.translation_model.handles.push(mark_handle);
        self.translation_model.handles.push(next_handle);

        task.chain(mark_task).chain(next_task)
    }

    pub fn select_page(&mut self, page: usize) -> Task<Message> {
        if let Some(content) = self.get_page(page) {
            self.doc_model.current_page = Some(page);
            self.doc_model.content = content;
        }
        Task::none()
    }

    pub fn set_file(
        &mut self,
        (file_name, mut epub): (String, EpubDoc<Cursor<Vec<u8>>>),
    ) -> Task<Message> {
        self.doc_model.current_page = Some(0);
        self.doc_model.total_pages = epub.get_num_pages();
        self.doc_model.path = Some(file_name);

        self.translation_model.current_page = Some(0);
        let paths = get_ordered_path(&mut epub);
        self.translation_model.pages = paths.into_iter().map(|path| Page::new(path)).collect();

        self.epub = Some(epub);

        Task::done(Message::SelectPage(0))
    }

    pub fn set_view(&mut self, view: View) -> Task<Message> {
        self.view = view;
        Task::none()
    }

    pub fn toggle_side_bar_collapse(&mut self) -> Task<Message> {
        self.side_bar_collapsed = !self.side_bar_collapsed;
        Task::none()
    }
}
