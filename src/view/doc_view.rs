use crate::actions::doc_action::DocAction;
use crate::state::doc_model::DocModel;
use crate::state::translator::Translator;
use crate::view::text_scrollable;
use iced::alignment::Horizontal;
use iced::widget::space::vertical;
use iced::widget::{Container, button, column, container, pick_list, row, text};
use iced::{Element, Length, Padding};

pub fn doc_view(Translator { doc_model, .. }: &Translator) -> Element<'_, DocAction> {
    container(column![
        vertical(),
        column![epub_select_button(), text_scrollable(&doc_model.content),]
            .height(Length::FillPortion(9))
            .padding(10),
        page_selector(doc_model).height(Length::FillPortion(1))
    ])
    .center_x(Length::Fill)
    .align_top(Length::Fill)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(10)
    .into()
}

pub fn page_selector(model: &DocModel) -> Container<'_, DocAction> {
    container(row![
        button(text("◀")).on_press(DocAction::Dec),
        pick_list(
            (0..model.total_pages).collect::<Vec<usize>>(),
            model.current_page,
            DocAction::SetPage
        ),
        button(text("▶")).on_press(DocAction::Inc)
    ])
    .width(Length::Fill)
    .align_x(Horizontal::Center)
    .padding(10)
}

pub fn epub_select_button() -> Container<'static, DocAction> {
    container(button(text("epub").center()).on_press(DocAction::OpenEpub))
        .align_x(Horizontal::Center)
        .padding(Padding::new(0.0).bottom(15))
}
