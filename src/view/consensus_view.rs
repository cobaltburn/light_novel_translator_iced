use crate::{
    actions::{consensus, server},
    model::{Consensus, Method, Server},
    view::{DisplayType, menu_button, rich_text_scrollable},
    widget::{build_path_buttons, context_menu_button, ollama_input, think_selector},
};
use iced::{
    Border, Color, Element, Length, Padding, Renderer, Theme,
    alignment::Vertical,
    widget::{
        Container, button, column, container, lazy, radio, row, scrollable, space::vertical, stack,
        text,
    },
};
use iced_aw::{ContextMenu, Menu, MenuBar, menu::Item};
use std::ops::Not;

pub fn consensus_view(model: &Consensus) -> Element<'_, consensus::Action> {
    let page = model.current_page();
    let current_page = model.current_page;
    let can_consensus = model.server.handles.is_empty()
        && model.server.connected()
        && !model.file_name().is_empty();
    let on_press = move |part| {
        can_consensus.then_some(consensus::Action::ConsensusPart {
            page: current_page,
            part,
        })
    };

    let error_cards = page.map(|p| p.error_cards(on_press));
    let content = page.map_or_default(|p| p.spans(model.display, on_press));

    container(column![
        vertical(),
        column![
            menu_bar(model),
            row![
                side_bar(model),
                stack![
                    ContextMenu::new(rich_text_scrollable(content), || container(column![
                        context_menu_button(text("full").color(Color::WHITE))
                            .on_press(consensus::Action::SetDisplay(DisplayType::Full))
                            .width(Length::Fill),
                        context_menu_button(text("end").color(Color::WHITE))
                            .on_press(consensus::Action::SetDisplay(DisplayType::End))
                            .width(Length::Fill),
                        context_menu_button(text("japanese").color(Color::WHITE))
                            .on_press(consensus::Action::SetDisplay(DisplayType::Japanese))
                            .width(Length::Fill)
                    ])
                    .style(container::rounded_box)
                    .width(100)
                    .into()),
                    error_cards
                ]
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
    .padding(10)
    .into()
}

fn side_bar(model: &Consensus) -> Container<'_, consensus::Action> {
    let buttons = lazy(model.sidebar_deps(), |deps| {
        build_path_buttons(deps).width(250).spacing(10)
    });
    container(scrollable(buttons).spacing(5).height(Length::Fill))
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

fn menu_bar(model @ Consensus { server, .. }: &Consensus) -> Element<'_, consensus::Action> {
    row![
        MenuBar::new(vec![
            epub_menu(model),
            candidate_menu(model),
            server_menu(server)
        ])
        .spacing(5),
        consensus_button(model),
        server.model_pick_list().map(Into::into),
    ]
    .width(Length::Fill)
    .spacing(5)
    .padding(Padding::default().bottom(15))
    .into()
}

fn consensus_button(model: &Consensus) -> Element<'_, consensus::Action> {
    let (button_text, message) = if !model.server.handles.is_empty() {
        ("cancel", Some(consensus::Action::CancelConsensus))
    } else if !model.server.connected()
        || model.file_name().is_empty()
        || model.candidates.is_empty()
    {
        ("translate", None)
    } else {
        let msg = consensus::Action::Consensus(model.current_page);
        ("translate", Some(msg))
    };

    button(text(button_text).center())
        .on_press_maybe(message)
        .into()
}

fn server_menu(state: &Server) -> Item<'_, consensus::Action, Theme, Renderer> {
    Item::with_menu(
        menu_button("server"),
        Menu::new(vec![
            Item::new(ollama_input().map(Into::into)),
            Item::new(think_selector(state).map(Into::into)),
            Item::new(execution_selector(state).map(Into::into)),
        ])
        .spacing(10)
        .width(400),
    )
}

pub fn execution_selector(state: &Server) -> Element<'_, server::Action> {
    container(
        row![
            text("Execution:"),
            radio(
                "Chain",
                Method::Chain,
                Some(state.method),
                server::Action::SetMethod
            ),
            radio(
                "Batch",
                Method::Batch,
                Some(state.method),
                server::Action::SetMethod
            ),
        ]
        .spacing(10),
    )
    .align_left(Length::Fill)
    .into()
}

fn epub_menu(model: &Consensus) -> Item<'_, consensus::Action, Theme, Renderer> {
    Item::with_menu(
        menu_button("epub"),
        Menu::new(vec![
            Item::new(epub_select(model)),
            Item::new(save_button(model)),
        ])
        .spacing(10)
        .width(400),
    )
}

fn save_button(model: &Consensus) -> Element<'_, consensus::Action> {
    let file_name = model.file_name();
    let not_empty = file_name.is_empty().not();
    let save_message = not_empty.then_some(consensus::Action::SaveTranslation(file_name));

    button(text("save").center())
        .on_press_maybe(save_message)
        .padding(5)
        .into()
}

fn epub_select(model: &Consensus) -> Element<'_, consensus::Action> {
    row![
        button(text("epub").center()).on_press(consensus::Action::OpenEpub),
        container(text(model.file_name()))
            .width(Length::Fill)
            .padding(5)
            .style(|theme| container::transparent(theme).border(Border {
                color: Color::WHITE,
                width: 0.5,
                radius: 5.into(),
            }))
    ]
    .align_y(Vertical::Center)
    .spacing(10)
    .padding(5)
    .into()
}

fn candidate_menu(model: &Consensus) -> Item<'_, consensus::Action, Theme, Renderer> {
    Item::with_menu(
        menu_button("candidate"),
        Menu::new(model.candidate_items())
            .spacing(10)
            .width(400)
            .padding(10),
    )
}
