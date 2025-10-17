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
    iced::application(
        "light novel translator",
        Translator::update,
        Translator::view,
    )
    .default_font(Font::DEFAULT)
    .theme(|_| Theme::TokyoNightStorm)
    .run()
}

impl Translator {
    pub fn view(&'_ self) -> Element<'_, Message> {
        container(row![
            side_bar_container(self),
            column![
                Space::with_height(Length::Fixed(10.0)),
                Self::view_select(&self)
            ]
            .padding(10)
            .width(Length::Fill)
            .align_x(Horizontal::Center),
        ])
        .into()
    }

    pub fn view_select(&'_ self) -> Element<'_, Message> {
        match self.view {
            View::Doc => doc_view(self),
            View::Translation => traslation_view(self),
            View::Format => format_view(self),
        }
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}
