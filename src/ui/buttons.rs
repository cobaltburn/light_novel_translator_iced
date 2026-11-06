use iced::widget::button::{Status, primary};
use iced::widget::{Button, button, text};
use iced::{Border, Element, Renderer, Theme, advanced};

pub fn text_button<'a, T: 'a>(content: impl Into<Element<'a, T>>) -> Button<'a, T>
where
    Theme: button::Catalog + 'a,
    Renderer: advanced::Renderer,
{
    button(content).style(|theme, status| match status {
        Status::Hovered => button::Style {
            border: Border {
                color: theme.palette().primary,
                width: 1.0,
                radius: 0.into(),
            },
            ..button::text(theme, status)
        },
        _ => button::Style {
            ..button::text(theme, status)
        },
    })
}

pub fn context_menu_button<'a, T: 'a>(content: impl Into<Element<'a, T>>) -> Button<'a, T>
where
    Theme: button::Catalog + 'a,
    Renderer: advanced::Renderer,
{
    button(content).style(|theme, status| match status {
        Status::Hovered => button::primary(theme, status),
        _ => button::text(theme, status),
    })
}

pub fn menu_button<'a, T: 'a>(button_text: &str) -> Button<'_, T> {
    button(text(button_text).center()).style(|theme, status| match status {
        Status::Disabled => primary(theme, Status::Active),
        _ => primary(theme, status),
    })
}
