use crate::{
    actions::server_action::ServerAction,
    model::server::{Method, Server, Think},
};
use iced::{
    Element, Length, Padding,
    alignment::Vertical,
    widget::{button, container, radio, row, text},
};
use iced_aw::NumberInput;

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
    let selection = [
        ("None", Think::None),
        ("Low", Think::Low),
        ("Medium", Think::Medium),
        ("High", Think::High),
    ];
    let radio_buttons = selection
        .into_iter()
        .map(|(l, t)| radio(l, t, Some(state.settings.think), ServerAction::SetThink).into());
    container(row![text("Think:")].extend(radio_buttons).spacing(10))
        .align_left(Length::Fill)
        .into()
}

pub fn execution_selector(state: &Server) -> Element<'_, ServerAction> {
    let selection = [
        ("Chain", Method::Chain),
        ("Batch", Method::Batch),
        ("History", Method::History),
    ];
    let radio_buttons = selection
        .into_iter()
        .map(|(l, t)| radio(l, t, Some(state.method), ServerAction::SetMethod).into());

    container(row![text("Execution:")].extend(radio_buttons).spacing(10))
        .align_left(Length::Fill)
        .into()
}

pub fn context_window_input(state: &Server) -> Element<'_, ServerAction> {
    container(
        row![
            text("Context window:"),
            NumberInput::new(
                &state.settings.context_window,
                2..=10,
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
