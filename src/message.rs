use crate::error::Error;
use crate::state::format_model::FormatAction;
use crate::state::{
    doc_model::DocAction, server_state::ServerAction, translation_model::TransAction,
    translator::Translator,
};
use crate::view::View;
use iced::Task;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Message {
    DocAction(DocAction),
    TranslationAction(TransAction),
    FormatAction(FormatAction),
    SetView(View),
    ToggleSideBar,
}

impl Translator {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DocAction(action) => self.doc_model.perform(action),
            Message::TranslationAction(action) => self.translation_model.perform(action),
            Message::FormatAction(action) => self.format_model.perform(action),
            Message::SetView(view) => self.set_view(view).into(),
            Message::ToggleSideBar => self.toggle_side_bar_collapse().into(),
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
        Message::TranslationAction(TransAction::ServerAction(action))
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

pub async fn open_epub() -> Option<(String, Vec<u8>)> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("select epub")
        .add_filter("epub", &["epub"])
        .pick_file()
        .await?;
    let buf = handle.read().await;
    Some((handle.file_name(), buf))
}

pub async fn display_error<T: Into<Error>>(error: T) {
    let error: Error = error.into();
    log::error!("{:#?}", error);
    _ = rfd::AsyncMessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_description(error.to_string())
        .set_buttons(rfd::MessageButtons::Ok)
        .set_title("error message")
        .show()
        .await;
}
