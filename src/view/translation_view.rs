use crate::{
    actions::{server_action::ServerAction, trans_action::TransAction},
    controller::part_tag,
    message::Message,
    model::{
        server::{Method, Server, Think},
        translation::Translation,
    },
    view::{menu_button, rich_text_scrollable},
};
use iced::widget::{button, column, container, radio, row, scrollable, span, text};
use iced::{
    Border, Color, Element, Function, Length, Padding, Renderer, Theme,
    alignment::Vertical,
    widget::{
        Button, Container, Row,
        container::transparent,
        space::{horizontal, vertical},
    },
};
use iced_aw::{Menu, MenuBar, TabLabel, Tabs, card::Status, menu::Item, style::tab_bar};

pub fn traslation_view(models: &Vec<Translation>, tab_id: usize) -> Element<'_, Message> {
    let tabs = models
        .iter()
        .map(translation_labeled_tab)
        .enumerate()
        .map(|(i, (label, tab))| {
            let tab = tab.map(Message::TranslationAction.with(i));
            (i, label, tab)
        });

    let mut tabs = Tabs::new_with_tabs(tabs, Message::SelectTab)
        .set_active_tab(&tab_id)
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

    if models.len() > 1 {
        tabs = tabs.on_close(Message::CloseTab);
    }

    tabs.into()
}

fn translation_labeled_tab(model: &Translation) -> (TabLabel, Element<'_, TransAction>) {
    (model.tab_label(), tab(model))
}

fn tab(model: &Translation) -> Element<'_, TransAction> {
    let content = model.current_content().unwrap_or_default();
    let content = content
        .into_iter()
        .enumerate()
        .map(|(i, t)| {
            [
                span(part_tag(i + 1)).color(Color::from_rgb8(255, 0, 0)),
                span(t),
            ]
        })
        .flatten()
        .collect();

    container(column![
        vertical(),
        column![
            menu_bar(model),
            row![side_bar(model), rich_text_scrollable(content)].spacing(10)
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

fn menu_bar(model @ Translation { server_state, .. }: &Translation) -> Row<'_, TransAction> {
    row![
        MenuBar::new(vec![file_menu(model), server_menu(server_state),]).spacing(5),
        translate_button(model),
        server_state.model_pick_list().map(Into::into),
    ]
    .width(Length::Fill)
    .spacing(5)
    .padding(Padding::default().bottom(15))
}

fn translate_button(model: &Translation) -> Button<'_, TransAction> {
    let (button_text, message) = if !model.server_state.handles.is_empty() {
        ("cancel", Some(TransAction::CancelTranslate))
    } else if !model.server_state.connected() {
        ("translate", None)
    } else {
        let msg = TransAction::Translate(model.current_page);
        ("translate", Some(msg))
    };

    button(text(button_text).center()).on_press_maybe(message)
}

fn side_bar(model: &Translation) -> Container<'_, TransAction> {
    container(scrollable(model.path_buttons().width(250).spacing(10)).height(Length::Fill))
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

fn server_menu(state: &Server) -> Item<'_, TransAction, Theme, Renderer> {
    Item::with_menu(
        menu_button("server"),
        Menu::new(vec![
            Item::new(ollama_input(state).map(Into::into)),
            Item::new(think_selector(state).map(Into::into)),
        ])
        .width(400),
    )
}

fn ollama_input(state: &Server) -> Element<'_, ServerAction> {
    container(
        row![
            text("Ollama").center(),
            button("connect").on_press(ServerAction::Connect),
            horizontal(),
            execution_selector(state)
        ]
        .align_y(Vertical::Center)
        .spacing(5),
    )
    .align_left(Length::Fill)
    .padding(10)
    .into()
}

fn think_selector(state: &Server) -> Element<'_, ServerAction> {
    container(
        row![
            text("Think:"),
            radio(
                "None",
                Think::None,
                Some(state.settings.think),
                ServerAction::SetThink
            ),
            radio(
                "Low",
                Think::Low,
                Some(state.settings.think),
                ServerAction::SetThink
            ),
            radio(
                "Medium",
                Think::Medium,
                Some(state.settings.think),
                ServerAction::SetThink
            ),
            radio(
                "High",
                Think::High,
                Some(state.settings.think),
                ServerAction::SetThink
            ),
        ]
        .spacing(10),
    )
    .align_left(Length::Fill)
    .padding(10)
    .into()
}

fn execution_selector(state: &Server) -> Element<'_, ServerAction> {
    row![
        text("Excetion:"),
        radio(
            "Chain",
            Method::Chain,
            Some(state.method),
            ServerAction::SetMethod
        ),
        radio(
            "Batch",
            Method::Batch,
            Some(state.method),
            ServerAction::SetMethod
        )
    ]
    .spacing(10)
    .into()
}

fn file_menu(model: &Translation) -> Item<'_, TransAction, Theme, Renderer> {
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

fn file_menu_buttons(Translation { file_name, .. }: &Translation) -> Element<'_, TransAction> {
    let not_empty = !file_name.is_empty();
    let save_message = not_empty.then_some(TransAction::SaveTranslation(file_name.clone()));

    button(text("save").center())
        .on_press_maybe(save_message)
        .padding(5)
        .into()
}

fn epub_select(model: &Translation) -> Row<'_, TransAction> {
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
