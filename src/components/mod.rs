use iced::widget::button::Status;
use iced::widget::{Button, button};
use iced::{Border, Element, Renderer, Theme, advanced};

pub mod side_bar;

pub fn text_button<'a, T: 'a>(
    content: impl Into<Element<'a, T, Theme, Renderer>>,
) -> Button<'a, T, Theme, Renderer>
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

pub fn context_menu_button<'a, T: 'a>(
    content: impl Into<Element<'a, T, Theme, Renderer>>,
) -> Button<'a, T, Theme, Renderer>
where
    Theme: button::Catalog + 'a,
    Renderer: advanced::Renderer,
{
    button(content).style(|theme, status| match status {
        Status::Hovered => button::primary(theme, status),
        _ => button::text(theme, status),
    })
}
