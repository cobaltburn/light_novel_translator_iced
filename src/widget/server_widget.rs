use std::ops::RangeBounds;

use crate::{
    actions::server,
    model::{Method, Server, Think},
};
use iced::{
    Element, Length, Padding,
    alignment::Vertical,
    widget::{button, container, radio, row, text},
};
use iced_aw::NumberInput;

pub fn ollama_input() -> Element<'static, server::Action> {
    container(
        row![
            text("Ollama: ").center(),
            button("connect").on_press(server::Action::Connect),
        ]
        .align_y(Vertical::Center)
        .spacing(5),
    )
    .align_left(Length::Fill)
    .padding(Padding::default().top(5))
    .into()
}

pub fn think_selector(state: &Server) -> Element<'_, server::Action> {
    let selection = [
        ("None", Think::None),
        ("Low", Think::Low),
        ("Medium", Think::Medium),
        ("High", Think::High),
    ];
    let radio_buttons = selection
        .into_iter()
        .map(|(l, t)| radio(l, t, Some(state.settings.think), server::Action::SetThink).into());
    container(row![text("Think:")].extend(radio_buttons).spacing(10))
        .align_left(Length::Fill)
        .into()
}

pub fn execution_selector(state: &Server) -> Element<'_, server::Action> {
    let selection = [
        ("Chain", Method::Chain),
        ("Batch", Method::Batch),
        ("History", Method::History),
    ];
    let radio_buttons = selection
        .into_iter()
        .map(|(l, t)| radio(l, t, Some(state.method), server::Action::SetMethod).into());

    container(row![text("Execution:")].extend(radio_buttons).spacing(10))
        .align_left(Length::Fill)
        .into()
}

pub fn context_window_input(state: &Server) -> Element<'_, server::Action> {
    container(
        row![
            text("Context window:"),
            NumberInput::new(
                &state.settings.context_window,
                2..=10,
                server::Action::SetWindow
            )
        ]
        .align_y(Vertical::Center)
        .spacing(10),
    )
    .align_left(Length::Fill)
    .padding(Padding::default().bottom(5))
    .into()
}

pub fn server_setting_input<'a>(
    setting: &'a str,
    value: f64,
    range: impl RangeBounds<f64>,
    on_change: impl 'a + Fn(f64) -> server::Action + Clone,
) -> Element<'a, server::Action> {
    container(
        row![
            text(setting),
            NumberInput::new(&value, range, on_change)
                .ignore_buttons(true)
                .ignore_scroll(true),
        ]
        .align_y(Vertical::Center)
        .spacing(10),
    )
    .align_left(Length::Fill)
    .padding(Padding::default().bottom(5))
    .into()
}
