use crate::components::ghost_button::ghost_button;
use crate::controller::server::Connection;
use crate::message::Message;
use crate::state::{
    server_state::ServerAction, translation_model::TransAction, translator::Translator,
};
use crate::view::{menu_button, text_scrollable};
use iced::alignment::{Horizontal, Vertical};
use iced::widget::container::transparent;
use iced::widget::{
    Button, Container, Row, Space, button, column, container, pick_list, row, scrollable, svg,
    text, text_input,
};
use iced::{Border, Color, Element, Length, Padding, Renderer, Theme, color};
use iced_aw::menu::Item;
use iced_aw::{Menu, MenuBar};

pub fn traslation_view(state: &Translator) -> Container<'_, Message> {
    let content = state
        .translation_model
        .current_content()
        .unwrap_or_default();

    container(column![
        Space::with_height(Length::FillPortion(1)),
        column![
            translation_menu_bar(state),
            row![translation_side_bar(state), text_scrollable(content)].spacing(10)
        ]
        .height(Length::FillPortion(9))
        .padding(10),
        Space::with_height(Length::FillPortion(1))
    ])
    .center_x(Length::Fill)
    .align_top(Length::Fill)
}

pub fn translation_menu_bar(state: &Translator) -> Row<'_, Message> {
    row![
        MenuBar::new(vec![file_menu(state), server_menu(state)]).spacing(5),
        translate_button(state),
        model_pick_list(state),
    ]
    .width(Length::Fill)
    .spacing(5)
    .padding(Padding::default().bottom(15))
}

pub fn model_pick_list(state: &'_ Translator) -> Element<'_, Message> {
    pick_list(
        state.server_state.models.clone(),
        state.server_state.current_model.clone(),
        |model| ServerAction::SelectModel(model).into(),
    )
    .width(250)
    .into()
}

pub fn translate_button(state: &Translator) -> Button<'_, Message> {
    let (button_text, msg) = if state.server_state.handles.is_empty() {
        let page = state.translation_model.current_page;
        let connected = state.server_state.connected();
        let message = connected.then_some(page.map(|page| Message::Translate(page)));
        ("translate", message.flatten())
    } else {
        ("cancel", Some(ServerAction::Abort.into()))
    };

    button(text(button_text).center()).on_press_maybe(msg)
}

pub fn translation_side_bar(state: &Translator) -> Container<'_, Message> {
    container(
        scrollable(
            column(translation_path_buttons(state))
                .width(250)
                .spacing(10),
        )
        .height(Length::Fill),
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

pub fn translation_path_buttons(state: &Translator) -> Vec<Element<'_, Message>> {
    state
        .translation_model
        .pages
        .iter()
        .enumerate()
        .map(|(i, page)| {
            let name = page.path.file_stem().unwrap().to_str().unwrap();
            let mut button_text = text(name).width(Length::Fill);
            if state.translation_model.current_page.is_some_and(|p| p == i) {
                button_text = button_text.color(color!(0x2ac3de))
            }

            let button_content = row![button_text]
                .push_maybe(page.complete.then_some(check_mark()))
                .push(Space::with_width(10));

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

pub fn server_menu(state: &Translator) -> Item<'_, Message, Theme, Renderer> {
    let url = state.server_state.url.clone();
    let key = state.server_state.api_key.clone();
    Item::with_menu(
        menu_button("server"),
        Menu::new(vec![
            Item::new(ollama_input(state)),
            Item::new(
                container(
                    button(text("connect"))
                        .on_press(ServerAction::Connect(Connection::Ollama(url)).into()),
                )
                .align_x(Horizontal::Center)
                .padding(5),
            ),
            Item::new(claude_input(state)),
            Item::new(
                container(
                    button(text("connect"))
                        .on_press(ServerAction::Connect(Connection::Claude(key)).into()),
                )
                .align_x(Horizontal::Center)
                .padding(5),
            ),
        ])
        .width(400),
    )
}

pub fn ollama_input(state: &Translator) -> Container<'_, Message> {
    container(
        row![
            container(text("Ollama:").center()),
            text_input("http://localhost:11434", &state.server_state.url)
                .style(|theme, status| {
                    let mut style = text_input::default(theme, status);
                    if state.server_state.invalid_url {
                        style.value = Color::from_rgb(1.0, 0.0, 0.0);
                    }
                    style
                })
                .on_input(|url| ServerAction::EditUrl(url).into()),
        ]
        .align_y(Vertical::Center)
        .spacing(5),
    )
    .padding(5)
}

pub fn claude_input(state: &Translator) -> Container<'_, Message> {
    container(
        row![
            container(text("Claude:").center()),
            text_input("api-key", &state.server_state.api_key)
                .on_input(|key| ServerAction::EditApiKey(key).into()),
        ]
        .align_y(Vertical::Center)
        .spacing(5),
    )
    .padding(5)
}

pub fn file_menu(state: &Translator) -> Item<'_, Message, Theme, Renderer> {
    Item::with_menu(
        menu_button("file"),
        Menu::new(vec![
            Item::new(epub_select(state)),
            Item::new(
                button(text("save").center())
                    .on_press(Message::SaveTranslation)
                    .padding(5),
            ),
        ])
        .spacing(10)
        .width(300),
    )
}

pub fn epub_select(state: &Translator) -> Row<'_, Message> {
    let path = state.doc_model.path.clone().unwrap_or_default();
    row![
        button(text("epub").center()).on_press(Message::OpenEpub),
        container(text(path))
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
