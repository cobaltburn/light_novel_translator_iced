use crate::{
    controller::{
        doc::get_ordered_path,
        markdown::{convert_html, join_partition, partition_text},
    },
    message::Message,
    state::{
        doc_model::DocModel,
        format_model::FormatModel,
        server_state::{ServerAction, ServerState},
        translation_model::{Page, TransAction, TranslationModel},
    },
    view::View,
};
use epub::doc::EpubDoc;
use iced::Task;
use ollama_rs::generation::chat::ChatMessage;
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
        let Some(model) = self.server_state.current_model.clone() else {
            return Task::none();
        };

        let Some(current_page) = self.translation_model.pages.get_mut(page) else {
            return Task::done(ServerAction::Abort.into());
        };

        let Some(epub) = self.epub.as_mut() else {
            return Task::none();
        };

        current_page.content.clear();
        epub.set_current_page(page);
        let html = epub.get_current_str().expect("max page exceeded").0;

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

        self.server_state.init_history(context);
        let history = &self.server_state.history;

        let mut task = Task::none();
        for (i, section) in sections {
            let server = self.server_state.server.clone();
            let tag = format!("\n\n<part>{}</part>\n\n", i + 1);
            let (tag_task, tag_handle) =
                Task::done(TransAction::UpdateContent(tag, page).into()).abortable();

            let (trans_task, handle) =
                Task::future(server.translate(model.clone(), section.to_owned(), history.clone()))
                    .and_then(move |stream| {
                        Task::run(stream, move |response| match response {
                            Ok(msg) => TransAction::UpdateContent(msg.message.content, page).into(),
                            Err(_) => {
                                log::error!("Failed to read stream");
                                return ServerAction::Abort.into();
                            }
                        })
                    })
                    .abortable();

            task = task.chain(tag_task).chain(trans_task);
            self.server_state.handles.push(handle);
            self.server_state.handles.push(tag_handle);
        }

        let (mark_task, mark_handle) =
            Task::done(TransAction::PageComplete(page).into()).abortable();
        let (next_task, next_handle) = Task::done(Message::Translate(page + 1)).abortable();

        self.server_state.handles.push(mark_handle);
        self.server_state.handles.push(next_handle);

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

pub fn generate_system_message(context: &Option<String>) -> ChatMessage {
    let mut prompt = String::from(
        r#"You are a highly skilled professional translator. You will be translating text from a Japanese book. You are a native speaker of English and Japanese. Translate the given text accurately, taking into account the context and specific instructions provided. If no additional instructions or context are provided, use your expertise to consider what the most appropriate context is and provide a natural translation that aligns with that context. When translating, strive to faithfully reflect the meaning and tone of the original text, pay attention to cultural nuances and differences in language usage, and ensure that the translation is grammatically correct and easy to read. For technical terms and proper nouns, either leave them in the original language or use appropriate translations as necessary. The translation should be complete. All the provide text should be translated. The translation should only contain the translated text. This should be a full translation of the given text. Do not summarize the text. Format the translated text into paragraphs with proper syntax. Match the paragraph structure of the original text. The text passed is in a markdown format the response should be in markdown. "#,
    );

    if let Some(context) = context {
        let context = format!(
            "Here is context about the story of the book, {}.\n",
            context
        );
        prompt.push_str(&context);
    }
    prompt.push_str("Take a deep breath, calm down, and start translating.");

    ChatMessage::system(prompt)
}
