//! The tab bar above the translation screen.

use crate::{
    app::Message,
    screens::translation::{Translation, translation_view},
};
use iced::widget::{button, column, container, row, text};
use iced::{Border, Color, Element, Function, Length, Padding, border::Radius};
use iced_aw::{TabBar, TabLabel, card::Status, style::tab_bar, tab_bar::Position};
use std::collections::BTreeMap;

pub fn translation_tabs(
    models: &BTreeMap<usize, Translation>,
    tab_id: usize,
) -> Element<'_, Message> {
    let tabs = models.iter().map(|(&i, e)| (i, tab_label(e))).collect();

    let mut tabs = TabBar::with_tab_labels(tabs, Message::SelectTab)
        .set_active_tab(&tab_id)
        .padding(Padding::new(0.0))
        .set_position(Position::Left)
        .height(Length::Fixed(40.0))
        .text_size(13.0)
        .icon_size(20.0)
        .style(|theme, status| match status {
            Status::Active | Status::Hovered => tab_bar::Style {
                text_color: Color::BLACK,
                icon_color: Color::from_rgb8(0, 255, 0),
                ..tab_bar::primary(theme, status)
            },
            _ => tab_bar::Style {
                icon_color: Color::from_rgb8(0, 255, 0),
                ..tab_bar::primary(theme, status)
            },
        });

    if models.len() > 1 {
        tabs = tabs.on_close(Message::CloseTab);
    }

    let model = models.get(&tab_id);
    let tab = model.map(|model| translation_view(model).map(Message::TransAction.with(tab_id)));
    let add_tab = model.map(|_| new_tab_button());

    column![row![tabs, add_tab].height(Length::Fixed(40.0)), tab].into()
}

fn new_tab_button() -> Element<'static, Message> {
    button(container(text("+").center()).center(Length::Fill))
        .on_press(Message::AddTab)
        .width(Length::Fixed(50.0))
        .height(Length::Fill)
        .style(|theme, status| button::Style {
            border: Border {
                color: Color::WHITE,
                width: 0.5,
                radius: Radius::from(0),
            },
            ..button::primary(theme, status)
        })
        .into()
}

fn tab_label(model: &Translation) -> TabLabel {
    let epub_name = model.epub_name().unwrap_or_default().into_owned();
    match model.server.handles.is_empty() {
        true => TabLabel::Text(epub_name),
        false => TabLabel::IconText('\u{25CF}', epub_name),
    }
}
