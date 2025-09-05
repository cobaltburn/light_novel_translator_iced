use crate::{
    components::ghost_button::ghost_button, message::Message, state::translator::Translator,
    view::View,
};
use iced::widget::svg;
use iced::{
    Border, Color, Element, Length,
    widget::{button, column, container, scrollable, text},
};

// TODO have the icons be moved into file on build
impl Translator {
    pub fn side_bar(&'_ self) -> Element<'_, Message> {
        container(scrollable(column![
            container(button(
                container(svg("./icons/chevrons-left-svgrepo-com.svg"))
                    .max_width(30)
                    .max_height(30)
                    .align_left(Length::Fill)
            ))
            .width(Length::Fill),
            ghost_button(text("Text").center())
                .on_press(Message::SetView(View::Doc))
                .padding(10)
                .width(Length::Fill),
            ghost_button(text("Translate").center())
                .on_press(Message::SetView(View::Translation))
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
        .into()
    }
}
