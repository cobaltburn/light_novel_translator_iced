use crate::{
    components::ghost_button::ghost_button,
    message::Message,
    state::{format_model::FormatAction, translator::Translator},
    view::{menu_button, translation_view::check_mark},
};
use iced::{
    Border, Color, Element, Length, Padding, Renderer, Theme, color,
    widget::{
        Container, Row, Space, button, column, container, row, scrollable, text, text_editor,
    },
};
use iced::{alignment::Vertical, widget::container::transparent};
use iced_aw::{Menu, MenuBar, menu::Item};
use std::ops::Not;

pub fn format_view(state: &Translator) -> Container<'_, Message> {
    container(column![
        Space::with_height(Length::FillPortion(1)),
        column![
            format_menu_bar(state),
            row![format_side_bar(state), format_text(state)].spacing(10)
        ]
        .height(Length::FillPortion(9))
        .padding(10),
        Space::with_height(Length::FillPortion(1)),
    ])
}

fn format_menu_bar(state: &Translator) -> Row<'_, Message> {
    row![
        MenuBar::new(vec![build_menu(state)]).spacing(5),
        button(text("build").center()).on_press(FormatAction::Build.into()),
        button(text("save")).on_press(FormatAction::SaveFiles.into()),
    ]
    .width(Length::Fill)
    .spacing(10)
    .padding(Padding::default().bottom(15))
}

fn build_menu(state: &Translator) -> Item<'_, Message, Theme, Renderer> {
    Item::with_menu(
        menu_button("content"),
        Menu::new(vec![
            Item::new(epub_button(state)),
            Item::new(toc_button(state)),
            Item::new(content_button(state)),
        ])
        .spacing(10)
        .width(400),
    )
}

fn content_button(state: &Translator) -> Row<'_, Message> {
    row![
        button(text("content")).on_press(FormatAction::SelectFolder.into()),
        container(text(&state.format_model.source_folder).center())
            .width(Length::Fill)
            .padding(5)
            .style(|theme| transparent(theme).border(Border {
                color: Color::WHITE,
                width: 0.5,
                radius: 5.into(),
            }))
    ]
    .align_y(Vertical::Center)
    .padding(5)
    .spacing(10)
}

fn toc_button(state: &Translator) -> Row<'_, Message> {
    let toc = if let Some(path) = &state.format_model.toc_path {
        path.to_string_lossy()
    } else {
        "".into()
    };
    row![
        button(text("toc").center()).on_press(FormatAction::SelectToc.into()),
        container(text(toc).center())
            .width(Length::Fill)
            .padding(5)
            .style(|theme| transparent(theme).border(Border {
                color: Color::WHITE,
                width: 0.5,
                radius: 5.into(),
            }))
    ]
    .align_y(Vertical::Center)
    .padding(5)
    .spacing(10)
}

fn epub_button(state: &Translator) -> Row<'_, Message> {
    row![
        button(text("epub").center()).on_press(FormatAction::SelectEpub.into()),
        container(text(&state.format_model.epub_name).center())
            .width(Length::Fill)
            .padding(5)
            .style(|theme| transparent(theme).border(Border {
                color: Color::WHITE,
                width: 0.5,
                radius: 5.into(),
            }))
    ]
    .align_y(Vertical::Center)
    .padding(5)
    .spacing(10)
}

fn format_text(state: &Translator) -> Container<'_, Message> {
    let current_text: Element<Message> = match state.format_model.current_content() {
        Some(content) => text_editor(content)
            .wrapping(text::Wrapping::WordOrGlyph)
            .on_action(|action| FormatAction::EditContent(action).into())
            .height(Length::Fill)
            .style(|theme, status| text_editor::Style {
                border: Border::default(),
                ..text_editor::default(theme, status)
            })
            .into(),
        None => text("").into(),
    };
    container(current_text)
        .padding(10)
        .height(Length::Fill)
        .width(Length::Fill)
        .style(|theme| {
            transparent(theme).border(Border {
                color: Color::WHITE,
                width: 1.0,
                radius: 8.into(),
            })
        })
}

pub fn format_side_bar(state: &Translator) -> Container<'_, Message> {
    container(
        scrollable(column(format_path_buttons(state)).width(250).spacing(10)).height(Length::Fill),
    )
    .height(Length::Fill)
    .padding(Padding::new(10.0).left(0).right(5))
    .style(|theme| {
        transparent(theme).border(Border {
            color: Color::WHITE,
            width: 1.0,
            radius: 8.into(),
        })
    })
}

fn format_path_buttons(state: &Translator) -> Vec<Element<'_, Message>> {
    state
        .format_model
        .pages
        .iter()
        .enumerate()
        .map(|(i, page)| {
            let name = page.path.file_stem().unwrap().to_str().unwrap();
            let mut button_text = text(name).width(Length::Fill);
            if state.format_model.current_page.is_some_and(|p| p == i) {
                button_text = button_text.color(color!(0x2ac3de))
            }

            let button_content = row![button_text]
                .push_maybe(page.changed.not().then_some(check_mark()))
                .push(Space::with_width(10));

            ghost_button(button_content)
                .on_press(FormatAction::SetPage(i).into())
                .into()
        })
        .collect()
}
