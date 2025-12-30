use crate::actions::{server_action::ServerAction, trans_action::TransAction};
use crate::components::{context_menu_button, text_button};
use crate::controller::server::Connection;
use crate::message::Message;
use crate::state::translation_model::Method;
use crate::state::{
    server_state::ServerState, translation_model::TranslationModel, translator::Translator,
};
use crate::view::{menu_button, text_scrollable};
use iced::alignment::Vertical;
use iced::widget::container::transparent;
use iced::widget::space::{horizontal, vertical};
use iced::widget::{
    Button, Column, Container, Row, button, checkbox, column, container, pick_list, radio, row,
    scrollable, svg, text, text_input,
};
use iced::{Border, Color, Element, Function, Length, Padding, Renderer, Theme};
use iced_aw::card::Status;
use iced_aw::menu::Item;
use iced_aw::style::tab_bar;
use iced_aw::{ContextMenu, Menu, MenuBar, TabLabel, Tabs, typed_input};

pub fn traslation_view(
    Translator {
        current_tab,
        translation_models,
        ..
    }: &Translator,
) -> Element<'_, Message> {
    let tabs = translation_models
        .iter()
        .map(translation_labeled_tab)
        .enumerate()
        .map(|(i, (label, tab))| {
            let tab = tab.map(Message::TranslationAction.with(i));
            (i, label, tab)
        });

    let mut tabs = Tabs::new_with_tabs(tabs, Message::SelectTab)
        .set_active_tab(current_tab)
        .tab_label_padding(Padding::new(0.0))
        .tab_bar_height(Length::Fixed(40.0))
        .text_size(12.0)
        .tab_bar_style(|theme, status| match status {
            Status::Active | Status::Hovered => tab_bar::Style {
                text_color: Color::BLACK,
                ..tab_bar::primary(theme, status)
            },
            _ => tab_bar::primary(theme, status),
        });

    if translation_models.len() > 1 {
        tabs = tabs.on_close(Message::CloseTab);
    }

    tabs.into()
}

pub fn translation_labeled_tab(model: &TranslationModel) -> (TabLabel, Element<'_, TransAction>) {
    (model.tab_label(), translation_tab(model))
}

pub fn translation_tab(model: &TranslationModel) -> Element<'_, TransAction> {
    let content = model.current_content().unwrap_or_default();

    container(column![
        vertical(),
        column![
            translation_menu_bar(model),
            row![translation_side_bar(model), text_scrollable(content)].spacing(10)
        ]
        .height(Length::FillPortion(10))
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

pub fn translation_menu_bar(model: &TranslationModel) -> Row<'_, TransAction> {
    row![
        MenuBar::new(vec![file_menu(model), server_menu(model),]).spacing(5),
        translate_button(model),
        model_pick_list(model),
    ]
    .width(Length::Fill)
    .spacing(5)
    .padding(Padding::default().bottom(15))
}

pub fn model_pick_list(
    TranslationModel { server_state, .. }: &TranslationModel,
) -> Element<'_, TransAction> {
    pick_list(
        server_state.models.clone(),
        server_state.current_model.clone(),
        |model| ServerAction::SelectModel(model).into(),
    )
    .width(250)
    .into()
}

pub fn translate_button(model: &TranslationModel) -> Button<'_, TransAction> {
    let (button_text, message) = if !model.server_state.handles.is_empty() {
        ("cancel", Some(ServerAction::Abort.into()))
    } else if !model.server_state.connected() {
        ("translate", None)
    } else {
        let msg = model.current_page.map(TransAction::Translate);
        ("translate", msg)
    };

    button(text(button_text).center()).on_press_maybe(message)
}

pub fn translation_side_bar(state: &TranslationModel) -> Container<'_, TransAction> {
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

pub fn translation_path_buttons(model: &TranslationModel) -> Column<'_, TransAction> {
    model
        .pages
        .iter()
        .enumerate()
        .map(|(i, page)| {
            let name = page.path.file_stem().unwrap().to_string_lossy();
            let button_text = text(format!("{}. {}", i + 1, &name))
                .color(Color::WHITE)
                .width(Length::Fill)
                .style(move |theme| {
                    if model.current_page.is_some_and(|p| p == i) {
                        text::primary(theme)
                    } else {
                        text::default(theme)
                    }
                });

            let button_content = row![button_text]
                .push(page.complete.then_some(check_mark()))
                .padding(Padding::default().right(10));

            let count = page.sections.len();
            let connected = model.server_state.connected();
            let underlay = text_button(button_content).on_press(TransAction::SetPage(i));

            ContextMenu::new(underlay, move || {
                path_button_overlay(count, name.to_string(), i, connected)
            })
            .into()
        })
        .collect()
}

fn path_button_overlay<'a>(
    count: usize,
    name: String,
    page: usize,
    connected: bool,
) -> Element<'a, TransAction> {
    let part_buttons = (0..count).map(|i| {
        context_menu_button(text(format!("translate part {}", i + 1)).color(Color::WHITE))
            .on_press_maybe(connected.then_some(TransAction::TranslatePart(page, i)))
            .into()
    });

    let overlay = column![
        context_menu_button(text("save").color(Color::WHITE))
            .on_press(TransAction::SavePage(name, page)),
        context_menu_button(text("translate").color(Color::WHITE))
            .on_press_maybe(connected.then_some(TransAction::Translate(page))),
        context_menu_button(text("translate page").color(Color::WHITE))
            .on_press_maybe(connected.then_some(TransAction::TranslatePage(page)))
    ]
    .extend(part_buttons)
    .padding(5)
    .spacing(5);

    container(scrollable(overlay).width(Length::Fill))
        .style(container::rounded_box)
        .max_height(400)
        .width(300)
        .into()
}

pub fn check_mark<'a, T: 'a>() -> Container<'a, T> {
    container(svg("./icons/check_mark.svg"))
        .width(20)
        .height(20)
        .align_y(Vertical::Center)
}

pub fn server_menu(model: &TranslationModel) -> Item<'_, TransAction, Theme, Renderer> {
    Item::with_menu(
        menu_button("server"),
        Menu::new(vec![
            Item::new(ollama_input()),
            // Item::new(claude_input(model)),
            Item::new(setting_input(&model.server_state)),
            Item::new(execution_input(model)),
        ])
        .width(300),
    )
}

pub fn ollama_input() -> Container<'static, TransAction> {
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

pub fn claude_input(model: &TranslationModel) -> Container<'_, TransAction> {
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

pub fn setting_input(state: &ServerState) -> Element<'_, TransAction> {
    row![
        text("Pause:").center(),
        typed_input(&state.settings.pause, |x| ServerAction::SetPause(x).into()),
        horizontal(),
        checkbox(state.settings.think)
            .label("Think")
            .on_toggle(|x| ServerAction::ThinkToggled(x).into()),
    ]
    .align_y(Vertical::Center)
    .spacing(5)
    .padding(10)
    .into()
}

pub fn execution_input(model: &TranslationModel) -> Element<'_, TransAction> {
    row![
        radio(
            "Chain",
            Method::Chain,
            Some(model.method),
            TransAction::SetMethod
        ),
        radio(
            "Batch",
            Method::Batch,
            Some(model.method),
            TransAction::SetMethod
        )
    ]
    .spacing(10)
    .into()
}

pub fn file_menu(model: &TranslationModel) -> Item<'_, TransAction, Theme, Renderer> {
    Item::with_menu(
        menu_button("file"),
        Menu::new(vec![
            Item::new(epub_select(model)),
            Item::new(file_menu_buttons(model)),
        ])
        .spacing(10)
        .width(400),
    )
}

fn file_menu_buttons(
    TranslationModel { file_name, .. }: &TranslationModel,
) -> Element<'_, TransAction> {
    let not_empty = !file_name.is_empty();
    let save_message = not_empty.then_some(TransAction::SaveTranslation(file_name.clone()));

    row![
        button(text("save").center())
            .on_press_maybe(save_message)
            .padding(5),
    ]
    .spacing(10)
    .into()
}

pub fn epub_select(model: &TranslationModel) -> Row<'_, TransAction> {
    row![
        button(text("epub").center()).on_press(TransAction::OpenEpub),
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
