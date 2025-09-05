use crate::message::Message;
use crate::state::translator::Translator;
use iced::alignment::Horizontal;
use iced::widget::{Container, Space, button, column, container, pick_list, row, text};
use iced::{Element, Length};

impl Translator {
    pub fn doc_screen(&'_ self) -> Container<'_, Message> {
        container(column![
            Space::with_height(Length::FillPortion(1)),
            Self::file_select_button(&self),
            Self::text_scrollable(&self.doc_model.content),
            Self::page_selector(&self)
        ])
        .center_x(Length::Fill)
        .align_top(Length::Fill)
        .padding(10)
        .into()
    }

    pub fn page_selector(&'_ self) -> Element<'_, Message> {
        container(row![
            button(text("◀")).on_press(Message::DecPage),
            pick_list(
                (0..self.doc_model.total_pages).collect::<Vec<usize>>(),
                self.doc_model.current_page,
                Message::SelectPage
            ),
            button(text("▶")).on_press(Message::IncPage)
        ])
        .width(Length::Fill)
        .align_x(Horizontal::Center)
        .padding(10)
        .into()
    }
}
