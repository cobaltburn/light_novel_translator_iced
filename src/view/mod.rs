use iced::advanced::widget::{
    Id, Operation, operate,
    operation::{self, Outcome, scrollable::AbsoluteOffset},
};
use iced::widget::{button, container, rich_text, scrollable, span, text};
use iced::{
    Border, Color, Element, Length, Padding, Rectangle, Task, Vector,
    alignment::{Horizontal, Vertical},
    color,
    widget::{
        Button, Column, Container,
        button::{Status, primary},
        container::transparent,
        text::{Span, Wrapping},
    },
};
use std::fmt;

mod consensus_view;
mod doc_view;
mod format_view;
mod translation_view;

pub use consensus_view::*;
pub use doc_view::*;
pub use format_view::*;
pub use translation_view::*;

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

/// Id of the scrollable built by [`rich_text_scrollable`].
pub const CONTENT_SCROLL: &str = "content-scroll";

fn section_id(i: usize) -> Id {
    Id::from(format!("content-section-{i}"))
}

/// Each inner `Vec` is one section, wrapped in its own container so
/// [`scroll_to_section`] can locate it.
pub fn rich_text_scrollable<E: Clone + 'static>(sections: Vec<Vec<Span<E>>>) -> Element<E> {
    let sections = sections.into_iter().enumerate().map(|(i, spans)| {
        container(
            rich_text(spans)
                .on_link_click(|e| e)
                .wrapping(Wrapping::WordOrGlyph)
                .align_x(Horizontal::Left)
                .align_y(Vertical::Center)
                .width(Length::Fill),
        )
        .id(section_id(i))
        .into()
    });

    container(
        scrollable(Column::with_children(sections).width(Length::Fill))
            .id(CONTENT_SCROLL)
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

/// Scrolls [`CONTENT_SCROLL`] so the top of `section` is at the top of the viewport.
pub fn scroll_to_section<T: Send + 'static>(section: usize) -> Task<T> {
    operate(FindSection {
        target: section_id(section),
        content_y: None,
        section_y: None,
    })
}

/// Measures the section's offset within the scrollable content, then chains
/// into a `scroll_to`. Layout bounds are unscrolled, so the difference is the
/// absolute offset regardless of the current scroll position.
struct FindSection {
    target: Id,
    content_y: Option<f32>,
    section_y: Option<f32>,
}

impl<T: Send + 'static> Operation<T> for FindSection {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>)) {
        operate(self);
    }

    fn scrollable(
        &mut self,
        id: Option<&Id>,
        _bounds: Rectangle,
        content_bounds: Rectangle,
        _translation: Vector,
        _state: &mut dyn operation::Scrollable,
    ) {
        if id.is_some_and(|id| *id == Id::new(CONTENT_SCROLL)) {
            self.content_y = Some(content_bounds.y);
        }
    }

    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        if id == Some(&self.target) {
            self.section_y = Some(bounds.y);
        }
    }

    fn finish(&self) -> Outcome<T> {
        match (self.content_y, self.section_y) {
            (Some(content_y), Some(section_y)) => {
                let offset = AbsoluteOffset {
                    x: None,
                    y: Some((section_y - content_y).max(0.0)),
                };
                Outcome::Chain(Box::new(operation::scrollable::scroll_to(
                    Id::new(CONTENT_SCROLL),
                    offset,
                )))
            }
            _ => Outcome::None,
        }
    }
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
