use crate::{
    actions::format_action::FormatAction, model::format::Format, view::menu_button,
    widget::text_button,
};
use iced::widget::{
    button, column, container, row, scrollable, space::vertical, text, text_editor,
};
use iced::{
    Border, Color, Element, Length, Padding, Renderer, Theme,
    alignment::Vertical,
    color,
    widget::{Container, Row, Space, container::transparent},
};
use iced_aw::{Menu, MenuBar, menu::Item};

pub fn format_view(model: &Format) -> Element<'_, FormatAction> {
    container(column![
        vertical(),
        column![
            format_menu_bar(model),
            row![format_side_bar(model), format_text(model)].spacing(10)
        ]
        .height(Length::FillPortion(9))
        .padding(10),
        vertical(),
    ])
    .center_x(Length::Fill)
    .align_top(Length::Fill)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(10)
    .into()
}

fn format_menu_bar(state: &Format) -> Row<'_, FormatAction> {
    row![
        MenuBar::new(vec![build_menu(state)]).spacing(5),
        button(text("build").center()).on_press(FormatAction::Build),
    ]
    .width(Length::Fill)
    .spacing(10)
    .padding(Padding::default().bottom(15))
}

fn build_menu(state: &Format) -> Item<'_, FormatAction, Theme, Renderer> {
    Item::with_menu(
        menu_button("content"),
        Menu::new(vec![
            Item::new(epub_button(state)),
            Item::new(content_button(state)),
        ])
        .spacing(10)
        .width(400),
    )
}

fn content_button(model: &Format) -> Row<'_, FormatAction> {
    row![
        button(text("content")).on_press(FormatAction::SelectFolder),
        container(text(&model.source_folder).center())
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

fn epub_button(model: &Format) -> Row<'_, FormatAction> {
    row![
        button(text("epub").center()).on_press(FormatAction::SelectEpub),
        container(text(&model.epub_name).center())
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

fn format_text(model: &Format) -> Container<'_, FormatAction> {
    let current_text = model.current_content().map(|content| {
        text_editor(content)
            .on_action(FormatAction::EditContent)
            .height(Length::Fill)
            .style(|theme, status| text_editor::Style {
                border: Border::default(),
                ..text_editor::default(theme, status)
            })
    });

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

pub fn format_side_bar(model: &Format) -> Container<'_, FormatAction> {
    container(
        scrollable(column(format_path_buttons(model)).width(250).spacing(10)).height(Length::Fill),
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

fn format_path_buttons(model: &Format) -> Vec<Element<'_, FormatAction>> {
    model
        .pages
        .iter()
        .enumerate()
        .map(|(i, page)| {
            let name = page.path.file_stem().unwrap().to_str().unwrap();
            let mut button_text = text(name).width(Length::Fill);
            if model.current_page.is_some_and(|p| p == i) {
                button_text = button_text.color(color!(0x2ac3de))
            }

            text_button(row![button_text, Space::new().width(10)])
                .on_press(FormatAction::SetPage(i))
                .into()
        })
        .collect()
}
