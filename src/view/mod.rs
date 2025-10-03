use std::fmt;

use crate::{message::Message, state::translator::Translator};
use iced::{
    Border, Color, Font, Length, Padding,
    alignment::{Horizontal, Vertical},
    widget::{Container, button, container, lazy, scrollable, text},
};

pub mod context_view;
pub mod doc_view;
pub mod format_view;
pub mod translation_view;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Doc,
    Translation,
    Context,
    Format,
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let view = match self {
            View::Doc => "Document",
            View::Translation => "Translation",
            View::Context => "Context",
            View::Format => "Format",
        };
        write!(f, "{}", view)
    }
}

pub const NOTO_SANS: Font = Font::with_name("Noto Sans CJK JP");

pub fn epub_select_button(_: &Translator) -> Container<'_, Message> {
    container(button(text("epub").center()).on_press(Message::OpenEpub)).align_x(Horizontal::Center)
}

pub fn text_scrollable(content: &'_ str) -> Container<'_, Message> {
    let scroll = lazy(content, |content| {
        scrollable(
            text(String::from(*content))
                .width(Length::Fill)
                .align_x(Horizontal::Left)
                .align_y(Vertical::Center)
                .font(NOTO_SANS),
        )
        .anchor_top()
        .width(Length::Fill)
    });
    container(scroll)
        .style(|_| container::Style {
            border: Border {
                color: Color::WHITE,
                width: 1.0,
                radius: 8.0.into(),
            },
            ..Default::default()
        })
        .height(Length::FillPortion(10))
        .width(Length::Fill)
        .padding(Padding::new(10.0).right(5))
}
