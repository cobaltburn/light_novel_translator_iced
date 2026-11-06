use crate::{
    actions::{glossary::Action, server},
    model::{Glossary, Server},
    view::{menu_button, text_scrollable},
    widget::{Spinner, num_ctx_input, ollama_input, server_setting_input, think_selector},
};
use iced::widget::{
    Button, Row, button, center, column, container, row, space::vertical, stack, text,
};
use iced::{Border, Color, Element, Length, Padding, Renderer, Theme, alignment::Vertical};
use iced_aw::{Menu, MenuBar, menu::Item};
use std::time::Duration;

pub fn glossary_view(model: &Glossary) -> Element<'_, Action> {
    let active = !model.server.handles.is_empty();
    container(column![
        vertical(),
        column![
            menu_bar(model),
            stack![text_scrollable(&model.content)].push(active.then_some(center(
                Spinner::new().cycle_duration(Duration::from_secs(2))
            ))),
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

fn menu_bar(model @ Glossary { server, .. }: &Glossary) -> Row<'_, Action> {
    row![
        MenuBar::new(vec![epub_menu(model), server_menu(server)]).spacing(5),
        translate_button(model),
        server.model_pick_list().map(Into::into),
    ]
    .width(Length::Fill)
    .spacing(5)
    .padding(Padding::default().bottom(15))
}

fn translate_button(model: &Glossary) -> Button<'_, Action> {
    let (button_text, message) = if !model.server.handles.is_empty() {
        ("cancel", Some(Action::Cancel))
    } else if !model.server.connected() || model.file_path.is_none() {
        ("glossary", None)
    } else {
        let msg = Action::Glossary;
        ("glossary", Some(msg))
    };

    button(text(button_text).center()).on_press_maybe(message)
}

fn server_menu(state @ Server { settings, .. }: &Server) -> Item<'_, Action, Theme, Renderer> {
    use server::Action;
    let elements = [
        ollama_input(),
        think_selector(state),
        num_ctx_input(state),
        server_setting_input(
            "Tempature:",
            settings.temperature,
            0.0..=2.0,
            Action::SetTemp,
        ),
        server_setting_input("Top p:", settings.top_p, 0.0..=1.0, Action::SetTopP),
        server_setting_input(
            "Repeat Penalty:",
            settings.repeat_penalty,
            0.0..=2.0,
            Action::SetRepeatPenalty,
        ),
    ];

    let items = elements
        .into_iter()
        .map(|e| Item::new(e.map(Into::into)))
        .collect();

    Item::with_menu(
        menu_button("server"),
        Menu::new(items).padding(10).spacing(10).width(400),
    )
}

fn epub_menu(model: &Glossary) -> Item<'_, Action, Theme, Renderer> {
    Item::with_menu(
        menu_button("epub"),
        Menu::new(vec![
            Item::new(epub_select(model)),
            Item::new(file_menu_buttons(model)),
        ])
        .spacing(10)
        .width(400),
    )
}

fn file_menu_buttons(model: &Glossary) -> Element<'_, Action> {
    let can_save =
        model.file_path.is_some() && model.server.handles.is_empty() && !model.content.is_empty();
    row![
        button(text("save").center())
            .on_press_maybe(can_save.then_some(Action::Save))
            .padding(5),
    ]
    .align_y(Vertical::Center)
    .into()
}

fn epub_select(model: &Glossary) -> Row<'_, Action> {
    row![
        button(text("epub").center()).on_press(Action::OpenEpub),
        container(text(model.file_name().unwrap_or_default()))
            .width(Length::Fill)
            .padding(5)
            .style(|theme| container::transparent(theme).border(Border {
                color: Color::WHITE,
                width: 0.5,
                radius: 5.into(),
            }))
    ]
    .align_y(Vertical::Center)
    .padding(5)
    .spacing(10)
}
