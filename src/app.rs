use crate::components::side_bar::side_bar_container;
use crate::message::Message;
use crate::state::translator::Translator;
use crate::view::{
    View, doc_view::doc_view, format_view::format_view, translation_view::traslation_view,
};
use iced::alignment::Horizontal;
use iced::widget::{column, container, row};
use iced::{Element, Length, Theme};
use iced_aw::ICED_AW_FONT_BYTES;

pub fn app() -> iced::Result {
    iced::application(Translator::default, Translator::update, Translator::view)
        .title("light novel translator")
        .theme(Theme::TokyoNightStorm)
        .font(ICED_AW_FONT_BYTES)
        .run()
}

impl Translator {
    pub fn view(&self) -> Element<'_, Message> {
        container(row![
            side_bar_container(self),
            column![Self::view_select(&self)]
                .width(Length::Fill)
                .align_x(Horizontal::Center),
        ])
        .into()
    }

    pub fn view_select(&self) -> Element<'_, Message> {
        match self.view {
            View::Doc => doc_view(self).map(Into::into),
            View::Translation => traslation_view(self),
            View::Format => format_view(self).map(Into::into),
        }
    }
}
