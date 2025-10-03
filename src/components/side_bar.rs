use crate::{
    components::ghost_button::ghost_button, message::Message, state::translator::Translator,
    view::View,
};
use iced::alignment::Horizontal;
use iced::widget::{Button, Container, svg};
use iced::{
    Border, Color, Length,
    widget::{button, column, container, scrollable, text},
};

// TODO have the icons be moved into file on build
pub fn side_bar_container(state: &Translator) -> Container<'_, Message> {
    match state.side_bar_collapsed {
        false => side_bar(state),
        true => side_bar_collapsed(state),
    }
}
pub fn side_bar(state: &Translator) -> Container<'_, Message> {
    container(scrollable(column![
        side_bar_button(state).width(Length::Fill),
        ghost_button(text(View::Doc.to_string()).center())
            .on_press(Message::SetView(View::Doc))
            .padding(10)
            .width(Length::Fill),
        ghost_button(text(View::Translation.to_string()).center())
            .on_press(Message::SetView(View::Translation))
            .padding(10)
            .width(Length::Fill),
        ghost_button(text(View::Context.to_string()).center())
            .on_press(Message::SetView(View::Context))
            .padding(10)
            .width(Length::Fill),
        ghost_button(text(View::Format.to_string()).center())
            .on_press(Message::SetView(View::Context))
            .padding(10)
            .width(Length::Fill)
    ]))
    .width(200)
    .height(Length::Fill)
    .style(|_| container::Style {
        border: Border {
            color: Color::WHITE,
            width: 1.0,
            radius: 0.into(),
        },
        ..Default::default()
    })
}

pub fn side_bar_collapsed(state: &Translator) -> Container<'_, Message> {
    container(side_bar_button(state).width(50))
}

pub fn side_bar_button(state: &Translator) -> Button<'_, Message> {
    let image_path = if state.side_bar_collapsed {
        "./icons/chevron-right.svg"
    } else {
        "./icons/chevron-left.svg"
    };

    button(
        container(svg(image_path))
            .max_width(30)
            .max_height(30)
            .align_x(Horizontal::Left),
    )
    .on_press(Message::ToggleSideBar)
}
