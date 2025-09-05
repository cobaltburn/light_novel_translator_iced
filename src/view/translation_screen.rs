use crate::components::ghost_button::ghost_button;
use crate::message::Message;
use crate::state::translator::Translator;
use iced::alignment::Vertical;
use iced::widget::{
    Container, Space, button, column, container, pick_list, row, scrollable, svg, text,
};
use iced::{Border, Color, Element, Length, Padding, color};

impl Translator {
    pub fn traslation_screen(&'_ self) -> Container<'_, Message> {
        let content = self.translation_model.current_content().unwrap_or("");

        container(
            column![
                Space::with_height(Length::FillPortion(1)),
                row![
                    Self::file_select_button(self),
                    button(text("connect").center()).on_press(Message::Connect(None)),
                    Translator::translate_button(self),
                    pick_list(
                        self.server_state.models.clone(),
                        self.server_state.current_model.clone(),
                        Message::SelectModel
                    )
                    .width(250)
                ]
                .spacing(5)
                .padding(Padding::new(0.0).bottom(5.0)),
                row![
                    Self::translation_side_bar(self),
                    Space::with_width(10),
                    Self::text_scrollable(content),
                ]
                .height(Length::FillPortion(9))
            ]
            .height(Length::Fill),
        )
        .center_x(Length::Fill)
        .align_top(Length::Fill)
        .padding(10)
    }

    pub fn translate_button(&'_ self) -> Element<'_, Message> {
        let (button_text, msg) = if self.translation_model.handles.is_none() {
            let page = self.translation_model.current_page;
            ("translate", page.map(|page| Message::Translate(page)))
        } else {
            ("cancel", Some(Message::Abort))
        };

        button(text(button_text).center())
            .on_press_maybe(msg)
            .into()
    }

    pub fn translation_side_bar(&'_ self) -> Element<'_, Message> {
        container(
            scrollable(column(Self::path_buttons(self)).width(250).spacing(10))
                .height(Length::Fill),
        )
        .height(Length::Fill)
        .padding(Padding::new(10.0).left(0).right(5))
        .style(|_| container::Style {
            border: Border {
                color: Color::WHITE,
                width: 1.0,
                radius: 8.into(),
            },
            ..Default::default()
        })
        .into()
    }

    pub fn path_buttons(&'_ self) -> Vec<Element<'_, Message>> {
        self.translation_model
            .pages
            .iter()
            .enumerate()
            .map(|(i, page)| {
                let name = page.path.file_stem().unwrap().to_str().unwrap();
                let mut button_text = text(name).width(Length::Fill);
                if self.translation_model.current_page.is_some_and(|p| p == i) {
                    button_text = button_text.color(color!(0x2ac3de))
                }

                ghost_button(row![
                    button_text,
                    container(svg("./icons/check_mark.svg"))
                        .width(24)
                        .height(24)
                        .align_y(Vertical::Center),
                    Space::with_width(10)
                ])
                .on_press(Message::SetTranslationPage(i))
                .into()
            })
            .collect()
    }
}
