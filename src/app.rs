use crate::message::Message;
use crate::state::translator::Translator;
use crate::view::View;
use iced::alignment::{Horizontal, Vertical};
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
            Self::side_bar(&self),
            column![
                Space::with_height(Length::Fixed(10.0)),
                Self::view_select(&self)
            ]
            .padding(10)
            .align_x(Horizontal::Center),
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .into()
    }

    pub fn view_select(&'_ self) -> Element<'_, Message> {
        match self.view {
            View::Doc => Self::doc_screen(&self),
            View::Translation => Self::traslation_screen(&self),
        }
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}
