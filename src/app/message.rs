use crate::{
    app::{Translator, View},
    screens::{consensus, doc, format, glossary, translation},
};
use iced::Task;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Message {
    DocAction(doc::Action),
    TransAction(usize, translation::Action),
    FormatAction(format::Action),
    ConsensusAction(consensus::Action),
    GlossaryAction(glossary::Action),
    SetView(View),
    ToggleSideBar,
    SelectTab(usize),
    CloseTab(usize),
    AddTab,
}

impl Translator {
    #[allow(clippy::unit_arg)]
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DocAction(action) => self.doc.perform(action).map(Into::into),
            Message::FormatAction(action) => self.format.perform(action).map(Into::into),
            Message::GlossaryAction(action) => self.glossary.perform(action).map(Into::into),
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

impl From<glossary::Action> for Message {
    fn from(action: glossary::Action) -> Self {
        Message::GlossaryAction(action)
    }
}
