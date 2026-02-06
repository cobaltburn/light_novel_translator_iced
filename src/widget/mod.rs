use crate::app::ICONS;
use iced::alignment::Vertical;
use iced::widget::button::Status;
use iced::widget::{Button, button, container, svg};
use iced::{Border, Element, Renderer, Theme, advanced};

pub mod side_bar;

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

pub fn check_mark<'a, T: 'a>() -> Element<'a, T> {
    let check_icon = ICONS.join("check_mark.svg");
    container(svg(check_icon))
        .width(20)
        .height(20)
        .align_y(Vertical::Center)
        .into()
}

pub fn cross_mark<'a, T: 'a>() -> Element<'a, T> {
    let check_icon = ICONS.join("cross_mark.svg");
    container(svg(check_icon))
        .width(20)
        .height(20)
        .align_y(Vertical::Center)
        .into()
}

pub fn active_mark<'a, T: 'a>() -> Element<'a, T> {
    let check_icon = ICONS.join("green_circle.svg");
    container(svg(check_icon))
        .width(20)
        .height(20)
        .align_y(Vertical::Center)
        .into()
}
