use iced::{
    Element, Length, Padding,
    alignment::Vertical,
    widget::{button, container, radio, row, text},
};
use iced_aw::NumberInput;

use crate::{
    actions::server_action::ServerAction,
    model::server::{Method, Server, Think},
};

pub fn ollama_input() -> Element<'static, ServerAction> {
    container(
        row![
            text("Ollama: ").center(),
            button("connect").on_press(ServerAction::Connect),
        ]
        .align_y(Vertical::Center)
        .spacing(5),
    )
    .align_left(Length::Fill)
    .padding(Padding::default().top(5))
    .into()
}

pub fn think_selector(state: &Server) -> Element<'_, ServerAction> {
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
    .into()
}

pub fn execution_selector(state: &Server) -> Element<'_, ServerAction> {
    container(
        row![
            text("Execution:"),
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
            ),
            radio(
                "History",
                Method::History,
                Some(state.method),
                ServerAction::SetMethod
            ),
        ]
        .spacing(10),
    )
    .align_left(Length::Fill)
    .into()
}

pub fn context_window_input(state: &Server) -> Element<'_, ServerAction> {
    container(
        row![
            text("Context window:"),
            NumberInput::new(
                &state.settings.context_window,
                0..=50,
                ServerAction::SetWindow
            )
        ]
        .align_y(Vertical::Center)
        .spacing(10),
    )
    .align_left(Length::Fill)
    .padding(Padding::default().bottom(5))
    .into()
}
