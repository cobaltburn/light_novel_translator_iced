//! The scrolling text panes, and jumping to a section inside one.

use iced::advanced::widget::{
    Id, Operation, operate,
    operation::{self, Outcome, scrollable::AbsoluteOffset},
};
use iced::widget::{
    Column, Container, container,
    container::transparent,
    rich_text, scrollable, text,
    text::{Span, Wrapping},
};
use iced::{
    Border, Color, Element, Length, Padding, Rectangle, Task, Vector,
    alignment::{Horizontal, Vertical},
};
use std::{iter, mem};

const TEXT_CHUNK: usize = 8 * 1024;

/// Id of the scrollable built by [`rich_text_scrollable`].
pub const CONTENT_SCROLL: &str = "content-scroll";

/// Cuts `content` at the last newline before each [`TEXT_CHUNK`] bytes, or at
/// the limit if there is none. The newline is dropped, since each piece starts
/// a new line anyway. Appending to `content` only changes the last piece.
fn text_chunks(mut content: &str) -> impl Iterator<Item = &str> {
    iter::from_fn(move || {
        if content.is_empty() {
            return None;
        }
        if content.len() <= TEXT_CHUNK {
            return Some(mem::take(&mut content));
        }
        let limit = content.floor_char_boundary(TEXT_CHUNK);
        let (chunk, rest) = match content[..limit].rfind('\n') {
            Some(newline) => (&content[..newline], &content[newline + 1..]),
            None => content.split_at(limit),
        };
        content = rest;
        Some(chunk)
    })
}

pub fn text_scrollable<'a, E: 'a>(content: &'a str) -> Container<'a, E> {
    let chunks = text_chunks(content).map(|chunk| {
        text(chunk)
            .width(Length::Fill)
            .align_x(Horizontal::Left)
            .align_y(Vertical::Center)
            .wrapping(Wrapping::WordOrGlyph)
            .into()
    });
    let scroll = scrollable(Column::with_children(chunks).width(Length::Fill))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn chunks(content: &str) -> Vec<&str> {
        text_chunks(content).collect()
    }

    #[test]
    fn text_chunks_keeps_short_content_whole() {
        assert_eq!(chunks(""), Vec::<&str>::new());
        assert_eq!(chunks("a\nb\n"), ["a\nb\n"]);
    }

    #[test]
    fn text_chunks_cuts_at_the_last_newline_and_drops_it() {
        let first = "a".repeat(TEXT_CHUNK - 10);
        let content = format!("{first}\nb\n\n{}", "c".repeat(20));

        assert_eq!(chunks(&content), [format!("{first}\nb\n"), "c".repeat(20)]);
    }

    #[test]
    fn text_chunks_cuts_a_line_with_no_newline_on_a_char_boundary() {
        let content = "語".repeat(TEXT_CHUNK);

        let pieces = chunks(&content);

        assert!(pieces.iter().all(|piece| piece.len() <= TEXT_CHUNK));
        assert_eq!(pieces.concat(), content);
    }

    #[test]
    fn text_chunks_only_changes_the_last_piece_when_content_grows() {
        let content: String = (0..4000).map(|i| format!("line {i}\n")).collect();

        for end in 1..content.len() {
            let before = chunks(&content[..end]);
            let after = chunks(&content[..=end]);
            let settled = before.len() - 1;

            assert_eq!(before[..settled], after[..settled]);
        }
        assert!(chunks(&content).len() > 1);
    }
}
