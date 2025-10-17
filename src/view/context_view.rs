use crate::{
    message::Message,
    state::{translation_model::TransAction, translator::Translator},
};
use iced::{
    Border, Color, Length, Padding,
    widget::{Container, Row, Space, button, column, container, row, text, text_editor},
};

pub fn context_view(state: &Translator) -> Container<'_, Message> {
    container(column![
        Space::with_height(Length::FillPortion(1)),
        column![
            context_button_bar(state),
            text_editor(&state.translation_model.context)
                .wrapping(text::Wrapping::WordOrGlyph)
                .on_action(|action| TransAction::EditContext(action).into())
                .height(Length::Fill)
                .padding(10)
                .style(|theme, status| text_editor::Style {
                    border: Border {
                        color: Color::WHITE,
                        width: 1.0,
                        radius: 8.into(),
                    },
                    ..text_editor::default(theme, status)
                })
        ]
        .height(Length::FillPortion(9))
        .padding(10),
        Space::with_height(Length::FillPortion(1))
    ])
    .center_x(Length::Fill)
    .align_top(Length::Fill)
}

pub fn context_button_bar(_: &Translator) -> Row<'_, Message> {
    row![button(text("select").center()).on_press(Message::OpenContext)]
        .spacing(5)
        .padding(Padding::default().bottom(15))
}
