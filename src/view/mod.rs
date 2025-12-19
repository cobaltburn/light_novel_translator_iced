use iced::widget::button::{Status, primary};
use iced::{
    Border, Color, Font, Length, Padding,
    alignment::{Horizontal, Vertical},
    widget::{Button, Container, button, container, scrollable, text},
};
use std::fmt;

pub mod doc_view;
pub mod format_view;
pub mod translation_view;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Doc,
    Translation,
    Format,
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let view = match self {
            View::Doc => "Document",
            View::Translation => "Translation",
            View::Format => "Format",
        };
        write!(f, "{}", view)
    }
}

pub const NOTO_SANS: Font = Font::with_name("Noto Sans CJK JP");

pub fn text_scrollable<'a, T: 'a>(content: &'a str) -> Container<'a, T> {
    let scroll = scrollable(
        text(content)
            .width(Length::Fill)
            .align_x(Horizontal::Left)
            .align_y(Vertical::Center)
            .font(NOTO_SANS),
    )
    .anchor_top()
    .width(Length::Fill);
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

pub fn menu_button<'a, T: 'a>(button_text: &'_ str) -> Button<'_, T> {
    button(text(button_text).center()).style(|theme, status| match status {
        Status::Disabled => primary(theme, Status::Active),
        _ => primary(theme, status),
    })
}
