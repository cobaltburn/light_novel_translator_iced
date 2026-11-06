use crate::{
    app::{Message, Translator, side_bar::side_bar_container, tabs::translation_tabs},
    screens::{
        consensus::consensus_view, doc::doc_view, format::format_view, glossary::glossary_view,
    },
};
use iced::{
    Element, Length,
    alignment::Horizontal,
    widget::{column, container, row},
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Translation,
    Glossary,
    Consensus,
    Format,
    Doc,
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let view = match self {
            View::Doc => "Document",
            View::Format => "Build",
            View::Translation => "Translation",
            View::Consensus => "Consensus",
            View::Glossary => "Glossary",
        };
        write!(f, "{}", view)
    }
}

impl Translator {
    pub fn view(&self) -> Element<'_, Message> {
        container(row![
            side_bar_container(self),
            column![self.view_select()]
                .width(Length::Fill)
                .align_x(Horizontal::Center),
        ])
        .into()
    }

    pub fn view_select(&self) -> Element<'_, Message> {
        match self.view {
            View::Doc => doc_view(&self.doc).map(Into::into),
            View::Translation => translation_tabs(&self.translations, self.active_tab),
            View::Format => format_view(&self.format).map(Into::into),
            View::Consensus => consensus_view(&self.consensus).map(Into::into),
            View::Glossary => glossary_view(&self.glossary).map(Into::into),
        }
    }
}
