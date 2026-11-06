//! How a page is drawn. The states live in their own files so they stay free of widgets.

use crate::{
    page::{ErrorType, Page, PageError, Section},
    text::is_japanese_char,
};
use iced::{
    Color, Element,
    alignment::Horizontal,
    color,
    widget::{Button, Column, button, right, span, text},
};
use std::{borrow::Cow, iter};

#[derive(Default, Debug, Clone, Copy)]
pub enum DisplayType {
    #[default]
    Full,
    End,
    Japanese,
}

impl Page {
    pub fn error_cards<T: 'static + Clone>(
        &self,
        on_press: impl Fn(usize) -> Option<T> + 'static,
    ) -> Element<'_, T> {
        let make_btn = |label: String, i: usize| {
            button(text(label))
                .padding(5)
                .style(button::primary)
                .on_press_maybe(on_press(i))
        };

        let empty_sections = self
            .sections
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.content.is_empty().then_some(i))
            .map(|i| make_btn(format!("Empty part: {:2}", i + 1), i));

        let errors = self.errors.iter().map(|e| e.error_button(&on_press));
        let errors: Column<'_, T> = empty_sections.chain(errors).map(Into::into).collect();

        right(errors.spacing(5).align_x(Horizontal::Right))
            .padding(20)
            .into()
    }

    pub fn spans<Link: 'static + Clone>(
        &self,
        display: DisplayType,
        on_press: impl Fn(usize) -> Option<Link> + 'static,
    ) -> Vec<Vec<text::Span<'_, Link>>> {
        let has_japanese_error = |i| {
            self.errors
                .iter()
                .any(|e| e.index == i && matches!(e.error, ErrorType::Japanese))
        };

        self.sections
            .iter()
            .enumerate()
            .map(|(i, section)| {
                let content = section.span_content(display);
                let mut spans = vec![
                    span(format!("\n\nPart: {}\nCount: {}\n\n", i + 1, content.len()))
                        .color(color!(0xff0000))
                        .link_maybe(on_press(i)),
                ];

                match display {
                    DisplayType::End => {
                        let mut content = content.into_owned();
                        let end = content.pop();
                        spans.push(span(content));
                        if let Some(end) = end {
                            let end_span = span(end);
                            spans.push(if end != ',' && end.is_ascii_punctuation() {
                                highlight(end_span)
                            } else {
                                end_span
                            });
                        }
                    }
                    DisplayType::Full if has_japanese_error(i) => {
                        spans.extend(japanese_runs(&section.content).map(|(japanese, run)| {
                            span(run).color_maybe(japanese.then_some(color!(0xff0000)))
                        }));
                    }
                    DisplayType::Full | DisplayType::Japanese => spans.push(span(content)),
                }

                spans
            })
            .collect()
    }
}

impl Section {
    pub fn span_content(&self, display: DisplayType) -> Cow<'_, str> {
        match display {
            DisplayType::End => {
                let lines: Vec<_> = self.content.lines().collect();
                lines
                    .get(lines.len().saturating_sub(10)..)
                    .unwrap_or_default()
                    .join("\n")
                    .into()
            }
            DisplayType::Full => self.content.as_str().into(),
            DisplayType::Japanese => self.japanese.as_str().into(),
        }
    }
}

impl PageError {
    pub fn error_button<T: 'static + Clone>(
        &self,
        on_press: &impl Fn(usize) -> Option<T>,
    ) -> Button<'_, T> {
        let make_btn = |label: String| {
            button(text(label))
                .padding(5)
                .style(button::primary)
                .on_press_maybe(on_press(self.index))
        };

        let position = self.index + 1;
        match self.error {
            ErrorType::Japanese => make_btn(format!("Japanese error: {:2}", position)),
            ErrorType::Size(c) => make_btn(format!("Size error: {:2}, count:{}", position, c)),
            ErrorType::Repeat => make_btn(format!("Repeat error: {:2}", position)),
            ErrorType::Copy => make_btn(format!("Copy error: {:2}", position)),
            ErrorType::Image => make_btn(format!("Image Tag error: {:2}", position)),
            ErrorType::Header => make_btn(format!("Header Tag error: {:2}", position)),
            ErrorType::Link => make_btn(format!("Link Tag error: {:2}", position)),
            ErrorType::End => make_btn(format!("End Punctuation error: {:2}", position)),
        }
    }
}

fn japanese_runs(text: &str) -> impl Iterator<Item = (bool, &str)> {
    let mut rest = text;
    iter::from_fn(move || {
        let japanese = is_japanese_char(rest.chars().next()?);
        let end = rest
            .find(|c| is_japanese_char(c) != japanese)
            .unwrap_or(rest.len());
        let (run, tail) = rest.split_at(end);
        rest = tail;
        Some((japanese, run))
    })
}

fn highlight<Link>(span: text::Span<'_, Link>) -> text::Span<'_, Link> {
    span.color(Color::BLACK).background(color!(0xffff00))
}
