use crate::{message::Message, state::translator::Translator, view::text_scrollable};
use iced::{
    Length,
    widget::{Container, Space, column, container},
};

pub fn format_view(state: &Translator) -> Container<'_, Message> {
    let content = state.format_model.current_content().unwrap_or("");

    container(column![
        Space::with_height(Length::FillPortion(1)),
        column![text_scrollable(content)].height(Length::FillPortion(9)),
        Space::with_height(Length::FillPortion(1)),
    ])
}
