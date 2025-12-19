use crate::components::ghost_button::ghost_button;
use crate::controller::server::Connection;
use crate::message::Message;
use crate::state::server_state::ServerState;
use crate::state::translation_model::TranslationModel;
use crate::state::{
    server_state::ServerAction, translation_model::TransAction, translator::Translator,
};
use crate::view::{menu_button, text_scrollable};
use iced::alignment::Vertical;
use iced::widget::container::transparent;
use iced::widget::space::vertical;
use iced::widget::{
    Button, Column, Container, Row, button, checkbox, column, container, pick_list, row,
    scrollable, svg, text, text_input,
};
use iced::{Border, Color, Element, Length, Padding, Renderer, Theme, color};
use iced_aw::menu::Item;
use iced_aw::{Menu, MenuBar, typed_input};

pub fn traslation_view(
    Translator {
        translation_model, ..
    }: &Translator,
) -> Element<'_, Message> {
    let content = translation_model.current_content().unwrap_or_default();

    container(column![
        vertical(),
        column![
            translation_menu_bar(translation_model),
            row![
                translation_side_bar(translation_model),
                text_scrollable(content)
            ]
            .spacing(10)
        ]
        .height(Length::FillPortion(9))
        .padding(10),
        vertical(),
    ])
    .center_x(Length::Fill)
    .align_top(Length::Fill)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

pub fn translation_menu_bar(state: &TranslationModel) -> Row<'_, Message> {
    row![
        MenuBar::new(vec![
            file_menu(state),
            server_menu(state),
            settings_menu(&state.server_state)
        ])
        .spacing(5),
        translate_button(state),
        model_pick_list(state),
    ]
    .width(Length::Fill)
    .spacing(5)
    .padding(Padding::default().bottom(15))
}

pub fn model_pick_list(
    TranslationModel { server_state, .. }: &TranslationModel,
) -> Element<'_, Message> {
    pick_list(
        server_state.models.clone(),
        server_state.current_model.clone(),
        |model| ServerAction::SelectModel(model).into(),
    )
    .width(250)
    .into()
}

pub fn translate_button(model: &TranslationModel) -> Button<'_, Message> {
    let (button_text, message) = if !model.server_state.handles.is_empty() {
        ("cancel", Some(ServerAction::Abort.into()))
    } else if !model.server_state.connected() {
        ("translate", None)
    } else {
        let page = model.current_page;
        let message = page.map(|page| TransAction::Translate(page).into());
        ("translate", message)
    };

    button(text(button_text).center()).on_press_maybe(message)
}

pub fn translation_side_bar(state: &TranslationModel) -> Container<'_, Message> {
    container(
        scrollable(translation_path_buttons(state).width(250).spacing(10)).height(Length::Fill),
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

pub fn translation_path_buttons(model: &TranslationModel) -> Column<'_, Message> {
    model
        .pages
        .iter()
        .enumerate()
        .map(|(i, page)| {
            let name = page.path.file_stem().unwrap().to_string_lossy();
            let mut button_text = text(name).width(Length::Fill);
            if model.current_page.is_some_and(|p| p == i) {
                button_text = button_text.color(color!(0x2ac3de))
            }

            let button_content = row![text(format!("{}. ", i + 1)), button_text]
                .push(page.complete.then_some(check_mark()))
                .padding(Padding::default().right(10));

            ghost_button(button_content)
                .on_press(TransAction::SetPage(i).into())
                .into()
        })
        .collect()
}

pub fn check_mark<'a>() -> Container<'a, Message> {
    container(svg("./icons/check_mark.svg"))
        .width(20)
        .height(20)
        .align_y(Vertical::Center)
}

pub fn server_menu(model: &TranslationModel) -> Item<'_, Message, Theme, Renderer> {
    Item::with_menu(
        menu_button("server"),
        Menu::new(vec![
            Item::new(ollama_input()),
            Item::new(claude_input(model)),
        ])
        .width(300),
    )
}

pub fn settings_menu(state: &ServerState) -> Item<'_, Message, Theme, Renderer> {
    Item::with_menu(
        menu_button("settings"),
        Menu::new(vec![
            Item::new(pause_input(state)),
            Item::new(
                checkbox(state.settings.think)
                    .label("Think")
                    .on_toggle(|x| ServerAction::ThinkToggled(x).into()),
            ),
        ])
        .spacing(5)
        .width(250),
    )
}

pub fn pause_input(state: &ServerState) -> Container<'_, Message> {
    container(
        row![
            text("Pause").center(),
            typed_input(&state.settings.pause, |x| ServerAction::SetPause(x).into()),
        ]
        .align_y(Vertical::Center)
        .spacing(5),
    )
    .align_left(Length::Fill)
}

pub fn ollama_input() -> Container<'static, Message> {
    container(
        row![
            text("Ollama").center(),
            button("connect").on_press(ServerAction::Connect(Connection::Ollama).into()),
        ]
        .align_y(Vertical::Center)
        .spacing(5),
    )
    .align_left(Length::Fill)
    .padding(10)
}

pub fn claude_input(model: &TranslationModel) -> Container<'_, Message> {
    let key = model.server_state.api_key.clone();
    container(
        row![
            text("Claude:").center(),
            text_input("api-key", &model.server_state.api_key)
                .on_input(|key| ServerAction::EditApiKey(key).into()),
            button("connect").on_press(ServerAction::Connect(Connection::Claude(key)).into()),
        ]
        .align_y(Vertical::Center)
        .spacing(5),
    )
    .align_left(Length::Fill)
    .padding(10)
}

pub fn file_menu(model: &TranslationModel) -> Item<'_, Message, Theme, Renderer> {
    Item::with_menu(
        menu_button("file"),
        Menu::new(vec![
            Item::new(epub_select(model)),
            Item::new(file_menu_buttons(model)),
        ])
        .spacing(10)
        .width(300),
    )
}

fn file_menu_buttons(
    TranslationModel { file_name, .. }: &TranslationModel,
) -> Element<'_, Message> {
    let not_empty = !file_name.is_empty();
    let load_message = not_empty.then_some(TransAction::LoadTranslation.into());
    let save_message = not_empty.then_some(TransAction::SaveTranslation(file_name.clone()).into());

    row![
        button(text("save").center())
            .on_press_maybe(save_message)
            .padding(5),
        button(text("load").center())
            .on_press_maybe(load_message)
            .padding(5)
    ]
    .spacing(10)
    .into()
}

pub fn epub_select(model: &TranslationModel) -> Row<'_, Message> {
    row![
        button(text("epub").center()).on_press(TransAction::OpenEpub.into()),
        container(text(&model.file_name))
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
