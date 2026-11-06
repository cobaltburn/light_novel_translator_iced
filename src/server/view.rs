use crate::{
    llm::{MAX_NUM_CTX, Think},
    server::{self, Action, Method, Server},
};
use iced::{
    Element, Length, Padding,
    alignment::Vertical,
    widget::{button, container, pick_list, radio, row, text},
};
use iced_aw::NumberInput;
use std::ops::RangeBounds;

impl Server {
    pub fn model_pick_list(&self) -> Element<'_, Action> {
        pick_list(
            self.models.as_slice(),
            self.current_model.as_ref(),
            Action::SelectModel,
        )
        .width(250)
        .into()
    }
}

pub fn ollama_input() -> Element<'static, Action> {
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

pub fn think_selector(state: &Server) -> Element<'_, Action> {
    let selection = [
        ("None", Think::None),
        ("Low", Think::Low),
        ("Medium", Think::Medium),
        ("High", Think::High),
    ];
    let radio_buttons = selection
        .into_iter()
        .map(|(l, t)| radio(l, t, Some(state.settings.think), Action::SetThink).into());
    container(row![text("Think:")].extend(radio_buttons).spacing(10))
        .align_left(Length::Fill)
        .into()
}

pub fn execution_selector(state: &Server) -> Element<'_, Action> {
    let selection = [
        ("Chain", Method::Chain),
        ("Batch", Method::Batch),
        ("History", Method::History),
    ];
    let radio_buttons = selection
        .into_iter()
        .map(|(l, t)| radio(l, t, Some(state.method), Action::SetMethod).into());

    container(row![text("Execution:")].extend(radio_buttons).spacing(10))
        .align_left(Length::Fill)
        .into()
}

pub fn context_window_input(state: &Server) -> Element<'_, Action> {
    container(
        row![
            text("Context window:"),
            NumberInput::new(
                &state.settings.context_window,
                2..=20,
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

pub fn num_ctx_input(state: &Server) -> Element<'_, Action> {
    container(
        row![
            text("Context size (tokens):"),
            NumberInput::new(
                &state.settings.num_ctx,
                2048..=MAX_NUM_CTX,
                Action::SetNumCtx
            )
            .step(4096)
            .ignore_scroll(true),
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
    on_change: impl 'a + Fn(f64) -> Action + Clone,
) -> Element<'a, Action> {
    container(
        row![
            text(setting),
            NumberInput::new(&value, range, on_change)
                .step(0.05)
                .ignore_scroll(true),
        ]
        .align_y(Vertical::Center)
        .spacing(10),
    )
    .align_left(Length::Fill)
    .padding(Padding::default().bottom(5))
    .into()
}
