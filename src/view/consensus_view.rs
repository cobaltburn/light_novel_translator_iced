use crate::{
    actions::consensus_action::ConsensusAction,
    controller::part_tag,
    model::{consensus::Consensus, server::Server},
    view::{menu_button, rich_text_scrollable},
    widget::server_widget::{
        context_window_input, execution_selector, ollama_input, think_selector,
    },
};
use iced::{
    Border, Color, Element, Length, Padding, Renderer, Theme,
    alignment::Vertical,
    color,
    widget::{Container, button, column, container, row, scrollable, space::vertical, span, text},
};
use iced_aw::{Menu, MenuBar, menu::Item};
use std::ops::Not;

pub fn consensus_view(model: &Consensus) -> Element<'_, ConsensusAction> {
    let content = model
        .current_content()
        .map(|e| {
            e.into_iter()
                .enumerate()
                .map(|(i, t)| [span(part_tag(i + 1)).color(color!(0xff0000)), span(t)])
                .flatten()
                .collect()
        })
        .unwrap_or_default();

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

fn side_bar(model: &Consensus) -> Container<'_, ConsensusAction> {
    container(
        scrollable(model.path_buttons().width(250).spacing(10))
            .spacing(5)
            .height(Length::Fill),
    )
    .height(Length::Fill)
    .padding(Padding::new(10.0).left(0).right(5))
    .style(|theme| {
        container::transparent(theme).border(Border {
            color: Color::WHITE,
            width: 1.0,
            radius: 8.into(),
        })
    })
}

fn menu_bar(model @ Consensus { server, .. }: &Consensus) -> Element<'_, ConsensusAction> {
    row![
        MenuBar::new(vec![epub_menu(model), server_menu(server),]).spacing(5),
        translate_button(model),
        server.model_pick_list().map(Into::into),
    ]
    .width(Length::Fill)
    .spacing(5)
    .padding(Padding::default().bottom(15))
    .into()
}

fn translate_button(model: &Consensus) -> Element<'_, ConsensusAction> {
    let (button_text, message) = if !model.server.handles.is_empty() {
        ("cancel", Some(ConsensusAction::CancelTranslate))
    } else if !model.server.connected() || model.file_name.is_empty() {
        ("translate", None)
    } else {
        let msg = ConsensusAction::Translate(model.current_page);
        ("translate", Some(msg))
    };

    button(text(button_text).center())
        .on_press_maybe(message)
        .into()
}

fn server_menu(state: &Server) -> Item<'_, ConsensusAction, Theme, Renderer> {
    Item::with_menu(
        menu_button("server"),
        Menu::new(vec![
            Item::new(ollama_input().map(Into::into)),
            Item::new(think_selector(state).map(Into::into)),
            Item::new(execution_selector(state).map(Into::into)),
            Item::new(context_window_input(state).map(Into::into)),
        ])
        .spacing(10)
        .width(400),
    )
}

fn epub_menu(model: &Consensus) -> Item<'_, ConsensusAction, Theme, Renderer> {
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
fn file_menu_buttons(Consensus { file_name, .. }: &Consensus) -> Element<'_, ConsensusAction> {
    let not_empty = file_name.is_empty().not();
    let save_message = not_empty.then_some(ConsensusAction::SaveTranslation(file_name.clone()));

    button(text("save").center())
        .on_press_maybe(save_message)
        .padding(5)
        .into()
}

fn epub_select(model: &Consensus) -> Element<'_, ConsensusAction> {
    row![
        button(text("epub").center()).on_press(ConsensusAction::OpenEpub),
        container(text(&model.file_name))
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
    .into()
}

fn folder_select(model: &Consensus) -> Element<'_, ConsensusAction> {
    row![
        button(text("folder").center()).on_press(ConsensusAction::OpenEpub),
        container(text(&model.file_name))
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
    .into()
}
