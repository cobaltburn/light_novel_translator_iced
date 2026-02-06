use iced::Element;
use iced::widget::container::transparent;
use iced::widget::text::Span;
use iced::widget::{button, container, rich_text, scrollable, text};
use iced::{
    Border, Color, Font, Length, Padding,
    alignment::{Horizontal, Vertical},
    widget::{
        Button, Container,
        button::{Status, primary},
    },
};
use std::fmt;

pub mod doc_view;
pub mod extraction_view;
pub mod format_view;
pub mod translation_view;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Translation,
    Doc,
    Format,
    Extraction,
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let view = match self {
            View::Doc => "Document",
            View::Translation => "Translation",
            View::Format => "Format",
            View::Extraction => "Extraction",
        };
        write!(f, "{}", view)
    }
}

pub const NOTO_SANS: Font = Font::with_name("Noto Sans CJK JP");

pub fn text_scrollable<'a, T: fmt::Display, E: 'a>(content: T) -> Container<'a, E> {
    let scroll = scrollable(
        text(content.to_string())
            .width(Length::Fill)
            .align_x(Horizontal::Left)
            .align_y(Vertical::Center)
            .font(NOTO_SANS),
    )
    .anchor_top()
    .width(Length::Fill);
    container(scroll)
        .style(|theme| {
            transparent(theme).border(Border {
                color: Color::WHITE,
                width: 1.0,
                radius: 8.into(),
            })
        })
        .height(Length::Fill)
        .width(Length::Fill)
        .padding(Padding::new(10.0).right(5))
}

pub fn rich_text_scrollable<'a, E: 'a>(content: Vec<Span<'a>>) -> Element<'a, E> {
    let scroll = scrollable(
        rich_text(content)
            .width(Length::Fill)
            .align_x(Horizontal::Left)
            .align_y(Vertical::Center)
            .font(NOTO_SANS),
    )
    .anchor_top()
    .width(Length::Fill);
    container(scroll)
        .style(|theme| {
            transparent(theme).border(Border {
                color: Color::WHITE,
                width: 1.0,
                radius: 8.into(),
            })
        })
        .height(Length::Fill)
        .width(Length::Fill)
        .padding(Padding::new(10.0).right(5))
        .into()
}

pub fn menu_button<'a, T: 'a>(button_text: &'_ str) -> Button<'_, T> {
    button(text(button_text).center()).style(|theme, status| match status {
        Status::Disabled => primary(theme, Status::Active),
        _ => primary(theme, status),
    })
}
