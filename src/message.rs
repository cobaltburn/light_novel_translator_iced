use crate::actions::{
    doc_action::DocAction, format_action::FormatAction, trans_action::TransAction,
};
use crate::{error::Error, state::translator::Translator, view::View};
use iced::Task;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Message {
    DocAction(DocAction),
    TranslationAction(usize, TransAction),
    FormatAction(FormatAction),
    SetView(View),
    ToggleSideBar,
    SelectTab(usize),
    CloseTab(usize),
}

impl Translator {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DocAction(action) => self.doc_model.perform(action),
            Message::TranslationAction(tab, action) => self.translation_action(tab, action),
            Message::FormatAction(action) => self.format_model.perform(action),
            Message::SetView(view) => self.set_view(view).into(),
            Message::ToggleSideBar => self.toggle_side_bar_collapse().into(),
            Message::SelectTab(tab) => self.set_tab(tab).into(),
            Message::CloseTab(tab) => self.close_tab(tab).into(),
        }
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
