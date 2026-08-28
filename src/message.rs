use crate::{
    actions::{consensus, doc, format, translation},
    error::Error,
    model::Translator,
    view::View,
};
use iced::Task;
use std::path::PathBuf;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Message {
    DocAction(doc::Action),
    TransAction(usize, translation::Action),
    FormatAction(format::Action),
    ConsensusAction(consensus::Action),
    SetView(View),
    ToggleSideBar,
    SelectTab(usize),
    CloseTab(usize),
    AddTab,
}

impl Translator {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DocAction(action) => self.doc.perform(action).map(Into::into),
            Message::FormatAction(action) => self.format.perform(action).map(Into::into),
            Message::ConsensusAction(action) => self.consensus.perform(action).map(Into::into),
            Message::TransAction(tab, action) => self.translation_action(tab, action),
            Message::SetView(view) => self.set_view(view).into(),
            Message::ToggleSideBar => self.toggle_side_bar_collapse().into(),
            Message::SelectTab(tab) => self.set_tab(tab).into(),
            Message::CloseTab(tab) => self.close_tab(tab).into(),
            Message::AddTab => self.add_tab().into(),
        }
    }
}

impl From<doc::Action> for Message {
    fn from(action: doc::Action) -> Self {
        Message::DocAction(action)
    }
}

impl From<format::Action> for Message {
    fn from(action: format::Action) -> Self {
        Message::FormatAction(action)
    }
}

impl From<consensus::Action> for Message {
    fn from(action: consensus::Action) -> Self {
        Message::ConsensusAction(action)
    }
}

pub async fn select_epub() -> Option<(PathBuf, Vec<u8>)> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("select epub")
        .add_filter("epub", &["epub"])
        .pick_file()
        .await?;
    let buf = handle.read().await;
    Some((handle.path().to_path_buf(), buf))
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
