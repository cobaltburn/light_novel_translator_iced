use iced::widget::button::Status;
use iced::widget::{Button, button};
use iced::{Border, Color, Element, Renderer, Theme, advanced};

pub fn ghost_button<'a, T: 'a>(
    content: impl Into<Element<'a, T, Theme, Renderer>>,
) -> Button<'a, T, Theme, Renderer>
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
