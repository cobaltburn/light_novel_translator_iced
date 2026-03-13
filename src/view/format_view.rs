use crate::{actions::format_action::FormatAction, model::format::Format};
use iced::alignment::Horizontal;
use iced::widget::{Column, image, text_input};
use iced::widget::{button, column, container, row, scrollable, space::vertical, text};
use iced::{
    Border, Color, Element, Length, Padding,
    alignment::Vertical,
    widget::{Row, container::transparent},
};

pub fn format_view(model: &Format) -> Element<'_, FormatAction> {
    container(column![
        vertical(),
        column![
            format_menu_bar(model),
            row![epub_image(model), epub_metadata(model)].spacing(10),
            container(button(text("build").center()).on_press(FormatAction::Build))
                .align_right(Length::Fill)
                .padding(20)
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

fn format_menu_bar(model: &Format) -> Row<'_, FormatAction> {
    row![epub_button(model), content_button(model),]
        .width(Length::Fill)
        .spacing(10)
        .padding(Padding::default().bottom(15))
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
    let name = model
        .epub_path
        .file_name()
        .map(|e| e.to_string_lossy())
        .unwrap_or_default();
    row![
        button(text("epub").center()).on_press(FormatAction::SelectEpub),
        container(text(name).center())
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

fn epub_image(model: &Format) -> Element<'_, FormatAction> {
    let cover_image = model.cover.as_ref().map(|handle| image(handle));
    container(cover_image)
        .padding(10)
        .height(Length::Fill)
        .width(Length::Fill)
        .center(Length::Fill)
        .into()
}

fn epub_metadata(model @ Format { metadata, .. }: &Format) -> Element<'_, FormatAction> {
    let label_width = 80;
    let content = column![
        row![
            container(text("Title: "))
                .align_right(Length::Fill)
                .width(label_width),
            container(text_input("Title", &metadata.title))
        ]
        .align_y(Vertical::Center),
        row![
            container(text("Author(s): "))
                .align_right(Length::Fill)
                .width(label_width),
            container(text_input("Author(s)", &metadata.authors))
        ]
        .align_y(Vertical::Center),
        row![
            container(text("Series: "))
                .align_right(Length::Fill)
                .width(label_width),
            container(text_input("Series", &metadata.series))
        ]
        .align_y(Vertical::Center),
        content_files(model)
    ]
    .spacing(5);
    container(content)
        .padding(10)
        .height(Length::Fill)
        .width(Length::Fill)
        .align_top(Length::Fill)
        .align_x(Horizontal::Center)
        .into()
}

fn content_files(Format { pages, .. }: &Format) -> Element<'_, FormatAction> {
    let pages: Column<_> = pages
        .iter()
        .filter_map(|p| p.path.file_stem())
        .map(|p| text(p.to_string_lossy()).into())
        .collect();
    container(
        scrollable(
            pages
                .padding(Padding::new(5.0).horizontal(10))
                .width(Length::Fill)
                .spacing(10),
        )
        .height(Length::Fill),
    )
    .height(Length::Fill)
    .width(Length::Fill)
    .padding(Padding::new(10.0).left(0).right(5))
    .style(|theme| {
        transparent(theme).border(Border {
            color: Color::WHITE,
            width: 1.0,
            radius: 8.into(),
        })
    })
    .into()
}

/* fn format_text(model: &Format) -> Container<'_, FormatAction> {
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
} */

/* fn format_side_bar(model: &Format) -> Container<'_, FormatAction> {
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
} */

/* fn format_path_buttons(model: &Format) -> Vec<Element<'_, FormatAction>> {
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
} */
