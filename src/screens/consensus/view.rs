use crate::{
    llm::Settings,
    page::DisplayType,
    screens::consensus::{Action, Candidate, Consensus},
    server::{
        self, Method, Server, num_ctx_input, ollama_input, server_setting_input, think_selector,
    },
    ui::{SidebarAction, context_menu_button, menu_button, page_side_bar, rich_text_scrollable},
};
use iced::widget::{
    Container, button, button::Status, column, container, radio, row, space::vertical, stack, text,
};
use iced::{Border, Color, Element, Length, Padding, Renderer, Theme, alignment::Vertical};
use iced_aw::{ContextMenu, Menu, MenuBar, menu::Item};
use std::iter::once;

pub fn consensus_view(model: &Consensus) -> Element<'_, Action> {
    let page = model.current_page();
    let current_page = model.current_page;
    let can_consensus =
        model.server.handles.is_empty() && model.server.connected() && model.file_name().is_some();
    let on_press = move |part| {
        can_consensus.then_some(Action::ConsensusPart {
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
                            .on_press(Action::SetDisplay(DisplayType::Full))
                            .width(Length::Fill),
                        context_menu_button(text("end").color(Color::WHITE))
                            .on_press(Action::SetDisplay(DisplayType::End))
                            .width(Length::Fill),
                        context_menu_button(text("japanese").color(Color::WHITE))
                            .on_press(Action::SetDisplay(DisplayType::Japanese))
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

fn side_bar(model: &Consensus) -> Container<'_, Action> {
    let active = model.server.connected() && model.server.handles.is_empty();
    page_side_bar(&model.pages, model.current_page, active)
}

fn menu_bar(model @ Consensus { server, .. }: &Consensus) -> Element<'_, Action> {
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

fn consensus_button(model: &Consensus) -> Element<'_, Action> {
    let (button_text, message) = if !model.server.handles.is_empty() {
        ("cancel", Some(Action::Cancel))
    } else if !model.server.connected()
        || model.file_name().is_none()
        || model.candidates.is_empty()
    {
        ("translate", None)
    } else {
        let msg = Action::Consensus(model.current_page);
        ("translate", Some(msg))
    };

    button(text(button_text).center())
        .on_press_maybe(message)
        .into()
}

fn server_menu(state: &Server) -> Item<'_, Action, Theme, Renderer> {
    let Settings {
        top_p,
        repeat_penalty,
        ..
    } = state.settings;
    Item::with_menu(
        menu_button("server"),
        Menu::new(vec![
            Item::new(ollama_input().map(Into::into)),
            Item::new(think_selector(state).map(Into::into)),
            Item::new(execution_selector(state).map(Into::into)),
            Item::new(num_ctx_input(state).map(Into::into)),
            Item::new(
                server_setting_input("Top p:", top_p, 0.0..=1.0, server::Action::SetTopP)
                    .map(Into::into),
            ),
            Item::new(
                server_setting_input(
                    "Repeat Penalty:",
                    repeat_penalty,
                    0.0..=2.0,
                    server::Action::SetRepeatPenalty,
                )
                .map(Into::into),
            ),
        ])
        .spacing(10)
        .width(400),
    )
}

fn execution_selector(state: &Server) -> Element<'_, server::Action> {
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

fn epub_menu(model: &Consensus) -> Item<'_, Action, Theme, Renderer> {
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

fn save_button(model: &Consensus) -> Element<'_, Action> {
    button(text("save").center())
        .on_press_maybe(model.file_name().map(Action::SaveTranslation))
        .padding(5)
        .into()
}

fn epub_select(model: &Consensus) -> Element<'_, Action> {
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
    .spacing(10)
    .padding(5)
    .into()
}

fn candidate_menu(model: &Consensus) -> Item<'_, Action, Theme, Renderer> {
    Item::with_menu(
        menu_button("candidate"),
        Menu::new(model.candidate_items())
            .spacing(10)
            .width(400)
            .padding(10),
    )
}

impl Consensus {
    fn candidate_items(&self) -> Vec<Item<'_, Action, Theme, Renderer>> {
        self.candidates
            .iter()
            .enumerate()
            .map(|(i, Candidate { name, .. })| Item::new(candidate_select(Some(i), name)))
            .chain(once(Item::new(candidate_select(None, ""))))
            .collect()
    }
}

fn candidate_select(i: Option<usize>, folder: &str) -> Element<'_, Action> {
    let x_button = i.map(|i| {
        button(text("x").center())
            .style(|theme, status| match status {
                Status::Hovered => button::primary(theme, status),
                _ => button::text(theme, status),
            })
            .on_press(Action::DropCandidate(i))
    });

    row![
        button(text("candidate").center()).on_press(Action::SelectCandidate(i)),
        container(
            row![text(folder).width(Length::Fill)]
                .push(x_button)
                .align_y(Vertical::Center)
        )
        .height(35)
        .width(Length::Fill)
        .padding(5)
        .style(|theme| {
            container::transparent(theme).border(Border {
                color: Color::WHITE,
                width: 0.5,
                radius: 5.into(),
            })
        }),
    ]
    .align_y(Vertical::Center)
    .spacing(10)
    .into()
}

impl SidebarAction for Action {
    fn set_page(page: usize) -> Self {
        Action::SetPage(page)
    }
    fn save_page(name: String, page: usize) -> Self {
        Action::SavePage { name, page }
    }
    fn translate(page: usize) -> Self {
        Action::Consensus(page)
    }
    fn translate_page(page: usize) -> Self {
        Action::ConsensusPage(page)
    }
    fn translate_part(page: usize, part: usize) -> Self {
        Action::ConsensusPart { page, part }
    }
}
