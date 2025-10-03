use crate::message::Message;
use crate::state::doc_model::DocAction;
use crate::state::translator::Translator;
use crate::view::{epub_select_button, text_scrollable};
use iced::alignment::Horizontal;
use iced::widget::{Container, Space, button, column, container, pick_list, row, text};
use iced::{Length, Padding};

pub fn doc_view(state: &Translator) -> Container<'_, Message> {
    container(column![
        Space::with_height(Length::FillPortion(1)),
        column![
            epub_select_button(&state).padding(Padding::new(0.0).bottom(15)),
            text_scrollable(&state.doc_model.content),
        ]
        .height(Length::FillPortion(9))
        .padding(10),
        page_selector(state).height(Length::FillPortion(1))
    ])
    .center_x(Length::Fill)
    .align_top(Length::Fill)
}

pub fn page_selector(state: &Translator) -> Container<'_, Message> {
    container(row![
        button(text("◀")).on_press(DocAction::Dec.into()),
        pick_list(
            (0..state.doc_model.total_pages).collect::<Vec<usize>>(),
            state.doc_model.current_page,
            Message::SelectPage
        ),
        button(text("▶")).on_press(DocAction::Inc.into())
    ])
    .width(Length::Fill)
    .align_x(Horizontal::Center)
    .padding(10)
}
