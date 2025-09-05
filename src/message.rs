use crate::{state::translator::Translator, view::View};
use epub::doc::EpubDoc;
use iced::Task;
use std::io::Cursor;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Message {
    OpenFile,
    SetFile(Option<(String, EpubDoc<Cursor<Vec<u8>>>)>),
    IncPage,
    DecPage,
    SelectPage(usize),
    SetView(View),
    SelectModel(String),
    SetModels(Vec<String>),
    Connect(Option<String>),
    SetTranslationPage(usize),
    UpdateTranslation(String, usize),
    BeginTranslation,
    Translate(usize),
    Abort,
    None,
}

impl Translator {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenFile => Task::perform(open_file(), Message::SetFile),
            Message::SetFile(doc) => self.set_file(doc),
            Message::SelectPage(page) => self.select_page(page),
            Message::IncPage => self.doc_model.inc_page(),
            Message::DecPage => self.doc_model.dec_page(),
            Message::SetView(view) => self.set_view(view),
            Message::SelectModel(model) => self.server_state.set_current_model(model),
            Message::SetModels(models) => self.server_state.set_models(models),
            Message::Connect(url) => self.server_state.connect_server(url),
            Message::SetTranslationPage(page) => self.translation_model.set_current_page(page),
            Message::BeginTranslation => self.translation_model.begin_translation(),
            Message::Translate(page) => self.execute_translation(page),
            Message::UpdateTranslation(text, page) => {
                self.translation_model.update_content(text, page)
            }
            Message::Abort => self.translation_model.abort_tranlation(),
            Message::None => Task::none(),
        }
    }
}

async fn open_file() -> Option<(String, EpubDoc<Cursor<Vec<u8>>>)> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("select epub")
        .add_filter("epub", &["epub"])
        .pick_file()
        .await?;
    let buf = handle.read().await;
    let epub = EpubDoc::from_reader(Cursor::new(buf)).ok()?;
    let file_name = handle.file_name();
    Some((file_name, epub))
}
