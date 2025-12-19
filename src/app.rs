use crate::components::side_bar::side_bar_container;
use crate::message::Message;
use crate::state::translator::Translator;
use crate::view::{
    View, doc_view::doc_view, format_view::format_view, translation_view::traslation_view,
};
use iced::alignment::Horizontal;
use iced::widget::{Space, column, container, row};
use iced::{Element, Font, Length, Theme};

pub fn app() -> iced::Result {
    iced::application(Translator::default, Translator::update, Translator::view)
        .title("light novel translator")
        .default_font(Font::DEFAULT)
        .theme(Theme::TokyoNightStorm)
        .run()
}

impl Translator {
    pub fn view(&self) -> Element<'_, Message> {
        container(row![
            side_bar_container(self),
            column![
                Space::new().height(Length::Fixed(10.0)),
                Self::view_select(&self)
            ]
            .padding(10)
            .width(Length::Fill)
            .align_x(Horizontal::Center),
        ])
        .into()
    }

    pub fn view_select(&self) -> Element<'_, Message> {
        match self.view {
            View::Doc => doc_view(self).map(Into::into),
            View::Translation => traslation_view(self).map(Into::into),
            View::Format => format_view(self).map(Into::into),
        }
    }
}
