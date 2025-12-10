use crate::state::format_model::FormatAction;
use crate::state::{
    doc_model::DocAction, server_state::ServerAction, translation_model::TransAction,
    translator::Translator,
};
use crate::view::View;
use iced::Task;
use std::path::{Path, PathBuf};
use tokio::fs;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Message {
    DocAction(DocAction),
    ServerAction(ServerAction),
    TranslationAction(TransAction),
    FormatAction(FormatAction),
    OpenEpub,
    SetEpub((String, Vec<u8>)),
    SaveTranslation(String),
    LoadTranslation,
    SelectPage(usize),
    SetView(View),
    Translate(usize),
    ToggleSideBar,
    Error(String),
}

impl Translator {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DocAction(action) => self.doc_model.perform(action),
            Message::ServerAction(action) => self.server_state.perform(action),
            Message::TranslationAction(action) => self.translation_model.perform(action),
            Message::FormatAction(action) => self.format_model.perform(action),
            Message::OpenEpub => {
                Task::future(open_epub()).and_then(|doc| Task::done(Message::SetEpub(doc)))
            }
            Message::SaveTranslation(file_name) => Task::future(pick_save_folder(file_name))
                .and_then(|path| Task::future(async { fs::create_dir(&path).await.map(|_| path) }))
                .then(|path| match path {
                    Ok(path) => Task::done(TransAction::SavePages(path).into()),
                    Err(err) => Task::future(display_error(err.to_string())).discard(),
                }),
            Message::LoadTranslation => Task::future(load_folder_markdown())
                .and_then(|pages| Task::done(TransAction::LoadPages(pages).into())),
            Message::SetEpub(doc) => self.set_epub(doc),
            Message::SelectPage(page) => self.select_page(page).into(),
            Message::SetView(view) => self.set_view(view).into(),
            Message::Translate(page) => self.execute_translation(page),
            Message::ToggleSideBar => self.toggle_side_bar_collapse().into(),
            Message::Error(error) => Task::future(display_error(error)).discard(),
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

impl From<FormatAction> for Message {
    fn from(action: FormatAction) -> Self {
        Message::FormatAction(action)
    }
}

pub async fn load_folder_markdown() -> Option<Vec<(PathBuf, String)>> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("load folder")
        .pick_folder()
        .await?;
    let mut dirs = fs::read_dir(handle.path()).await.ok()?;
    let mut pages = Vec::new();
    while let Ok(Some(entry)) = dirs.next_entry().await {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|x| x == "md") {
            let content = fs::read_to_string(&path).await.ok()?;
            pages.push((path, content));
        }
    }

    Some(pages)
}

pub async fn open_epub() -> Option<(String, Vec<u8>)> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("select epub")
        .add_filter("epub", &["epub"])
        .pick_file()
        .await?;
    let buf = handle.read().await;
    Some((handle.file_name(), buf))
}

pub async fn pick_save_folder(file_name: String) -> Option<PathBuf> {
    let file_name = Path::new(&file_name).file_stem()?.to_str()?;
    let handle = rfd::AsyncFileDialog::new()
        .set_title("save translation")
        .set_file_name(file_name)
        .save_file()
        .await?;
    Some(handle.path().to_path_buf())
}

pub async fn display_error<T: Into<String>>(error: T) {
    _ = rfd::AsyncMessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_description(error)
        .set_buttons(rfd::MessageButtons::Ok)
        .set_title("error message")
        .show()
        .await;
}
