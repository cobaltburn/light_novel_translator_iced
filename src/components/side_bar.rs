use crate::app::ICONS;
use crate::{components::text_button, message::Message, state::translator::Translator, view::View};
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
        side_bar_toggle(state).width(Length::Fill),
        side_bar_button(View::Translation, &state.view),
        side_bar_button(View::Doc, &state.view),
        side_bar_button(View::Format, &state.view),
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
    container(side_bar_toggle(state).width(50))
}

pub fn side_bar_button(view: View, current_view: &View) -> Button<'static, Message> {
    let current = &view == current_view;
    let button_text = text(view.to_string()).center().style(move |theme| {
        if current {
            text::primary(theme)
        } else {
            text::default(theme)
        }
    });

    text_button(button_text)
        .on_press(Message::SetView(view))
        .padding(10)
        .width(Length::Fill)
}

pub fn side_bar_toggle(state: &Translator) -> Button<'_, Message> {
    let image_path = match state.side_bar_collapsed {
        true => &ICONS.join("chevron-right.svg"),
        false => &ICONS.join("chevron-left.svg"),
    };

    button(
        container(svg(image_path))
            .max_width(30)
            .max_height(30)
            .align_x(Horizontal::Left),
    )
    .on_press(Message::ToggleSideBar)
}
