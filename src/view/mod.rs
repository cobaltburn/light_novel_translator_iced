use crate::{message::Message, state::translator::Translator};
use iced::{
    Border, Color, Element, Font, Length, Padding,
    alignment::{Horizontal, Vertical},
    widget::{button, container, scrollable, text},
};

pub mod doc_screen;
pub mod translation_screen;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Doc,
    Translation,
}

pub const NOTO_SANS: Font = Font::with_name("Noto Sans CJK JP");

impl Translator {
    pub fn file_select_button(&'_ self) -> Element<'_, Message> {
        container(
            button(text("select").center())
                .on_press(Message::OpenFile)
                .padding(5),
        )
        .align_x(Horizontal::Center)
        .padding(Padding::new(0.0).bottom(10))
        .into()
    }

    pub fn text_scrollable(content: &'_ str) -> Element<'_, Message> {
        container(
            scrollable(
                text(content)
                    .width(Length::Fill)
                    .align_x(Horizontal::Left)
                    .align_y(Vertical::Center)
                    .font(NOTO_SANS),
            )
            .anchor_top()
            .width(Length::Fill),
        )
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
        .into()
    }
}
