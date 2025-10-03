use crate::message::Message;
use iced::widget::button::Status;
use iced::widget::{Button, button};
use iced::{Border, Color, Element, Renderer, Theme, advanced};

pub fn ghost_button<'a>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Button<'a, Message, Theme, Renderer>
where
    Theme: button::Catalog + 'a,
    Renderer: advanced::Renderer,
{
    button(content).style(|theme, status| match status {
        Status::Hovered => button::Style {
            text_color: Color::WHITE,
            border: Border {
                color: theme.palette().primary,
                width: 1.0,
                radius: 0.into(),
            },
            ..Default::default()
        },
        _ => button::Style {
            text_color: Color::WHITE,
            background: None,
            ..Default::default()
        },
    })
}
