use crate::{message::Message, state::translator::Translator, view::text_scrollable};
use iced::{
    Border, Color, Element, Length, Padding,
    widget::{Container, Space, column, container, container::transparent, scrollable},
};

pub fn format_view(state: &Translator) -> Container<'_, Message> {
    let content = state.format_model.current_content().unwrap_or("");

    container(column![
        Space::with_height(Length::FillPortion(1)),
        column![text_scrollable(content)].height(Length::FillPortion(9)),
        Space::with_height(Length::FillPortion(1)),
    ])
}

pub fn format_side_bat(state: &Translator) -> Container<'_, Message> {
    container(
        scrollable(column(format_path_buttons(state)).width(250).spacing(10)).height(Length::Fill),
    )
    .height(Length::Fill)
    .padding(Padding::new(10.0).left(0).right(5))
    .style(|theme| {
        transparent(theme).border(Border {
            color: Color::WHITE,
            width: 1.0,
            radius: 8.into(),
        })
    })
}

pub fn format_path_buttons(state: &Translator) -> Vec<Element<'_, Message>> {
    todo!()
}
