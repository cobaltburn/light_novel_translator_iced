use iced::widget::{button, container, rich_text, scrollable, span, text};
use iced::{
    Border, Color, Element, Length, Padding,
    alignment::{Horizontal, Vertical},
    color,
    widget::{
        Button, Container,
        button::{Status, primary},
        container::transparent,
        text::{Span, Wrapping},
    },
};
use std::fmt;

pub mod consensus_view;
pub mod doc_view;
pub mod format_view;
pub mod translation_view;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Translation,
    Doc,
    Consensus,
    Format,
}

impl fmt::Display for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let view = match self {
            View::Doc => "Document",
            View::Format => "Build",
            View::Translation => "Translation",
            View::Consensus => "Consensus",
        };
        write!(f, "{}", view)
    }
}

pub fn text_scrollable<'a, T: fmt::Display, E: 'a>(content: T) -> Container<'a, E> {
    let scroll = scrollable(
        text(content.to_string())
            .width(Length::Fill)
            .align_x(Horizontal::Left)
            .align_y(Vertical::Center)
            .wrapping(Wrapping::WordOrGlyph),
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

pub fn rich_text_scrollable<E: Clone + 'static>(content: Vec<Span<E>>) -> Element<E> {
    container(
        scrollable(
            rich_text(content)
                .on_link_click(|e| e)
                .wrapping(Wrapping::WordOrGlyph)
                .align_x(Horizontal::Left)
                .align_y(Vertical::Center)
                .width(Length::Fill),
        )
        .spacing(5)
        .width(Length::Fill)
        .anchor_top(),
    )
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

pub fn part_span(i: usize, content: String) -> [Span<'static>; 2] {
    let text = format!("\n\nPart: {}\nCount: {}\n\n", i + 1, content.len());
    [span(text).color(color!(0xff0000)), span(content)]
}

pub fn menu_button<'a, T: 'a>(button_text: &str) -> Button<'_, T> {
    button(text(button_text).center()).style(|theme, status| match status {
        Status::Disabled => primary(theme, Status::Active),
        _ => primary(theme, status),
    })
}

#[derive(Default, Debug, Clone, Copy)]
pub enum DisplayType {
    #[default]
    Full,
    End,
    Japanese,
}
