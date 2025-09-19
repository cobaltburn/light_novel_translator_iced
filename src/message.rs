use crate::state::{
    doc_model::DocAction, server_state::ServerAction, translation_model::TransAction,
    translator::Translator,
};
use crate::view::View;
use epub::doc::EpubDoc;
use iced::Task;
use std::io::Cursor;
use std::path::PathBuf;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Message {
    DocAction(DocAction),
    ServerAction(ServerAction),
    TranslationAction(TransAction),
    OpenEpub,
    SetEpub((String, EpubDoc<Cursor<Vec<u8>>>)),
    SaveTranslation,
    OpenContext,
    SelectPage(usize),
    SetView(View),
    Translate(usize),
    ToggleSideBar,
    None,
}

impl Translator {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DocAction(action) => self.doc_model.perform(action),
            Message::ServerAction(action) => self.server_state.perform(action),
            Message::TranslationAction(action) => self.translation_model.perform(action),
            Message::OpenEpub => {
                Task::future(open_epub()).and_then(|doc| Task::done(Message::SetEpub(doc)))
            }
            Message::SaveTranslation => Task::future(pick_save_folder())
                .and_then(|path| Task::done(TransAction::SavePages(path).into())),
            Message::OpenContext => Task::future(open_context())
                .and_then(|context| Task::done(TransAction::SetContext(context).into())),
            Message::SetEpub(doc) => self.set_file(doc),
            Message::SelectPage(page) => self.select_page(page),
            Message::SetView(view) => self.set_view(view),
            Message::Translate(page) => self.execute_translation(page),
            Message::ToggleSideBar => self.toggle_side_bar_collapse(),
            Message::None => Task::none(),
        }
    }
}

impl From<TransAction> for Message {
    fn from(action: TransAction) -> Self {
        Message::TranslationAction(action)
    }
}

impl From<ServerAction> for Message {
    fn from(action: ServerAction) -> Self {
        Message::ServerAction(action)
    }
}

impl From<DocAction> for Message {
    fn from(action: DocAction) -> Self {
        Message::DocAction(action)
    }
}

async fn open_epub() -> Option<(String, EpubDoc<Cursor<Vec<u8>>>)> {
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

async fn pick_save_folder() -> Option<PathBuf> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("save translation")
        .pick_folder()
        .await?;
    Some(handle.path().to_path_buf())
}

async fn open_context() -> Option<String> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("select context")
        .add_filter("files", &["txt", "md", "xml"])
        .pick_file()
        .await?;
    let buf = handle.read().await;
    let context = String::from_utf8(buf).ok()?;
    Some(context)
}
