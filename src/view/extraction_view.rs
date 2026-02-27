use crate::{
    actions::{extraction_action::ExtractAction, server_action::ServerAction},
    model::{
        extraction::{Extraction, ImageView},
        server::{Method, Server, Think},
    },
    view::menu_button,
};
use iced::widget::{button, column, container, image::Viewer, radio, row, scrollable, text};
use iced::{
    Border, Color, Element, Length, Padding,
    alignment::{Horizontal, Vertical},
    widget::{
        container::transparent,
        space::{horizontal, vertical},
    },
};
use iced::{Renderer, Theme};
use iced_aw::menu::Item;
use iced_aw::{Menu, MenuBar};

pub fn extraction_view(model: &Extraction) -> Element<'_, ExtractAction> {
    container(column![
        vertical(),
        column![
            menu_bar(model),
            row![
                column![image_view_selector(model), side_bar(model)]
                    .align_x(Horizontal::Center)
                    .spacing(10),
                page_content(model)
            ]
            .spacing(10)
        ]
        .height(Length::FillPortion(9))
        .padding(10),
        vertical(),
    ])
    .center_x(Length::Fill)
    .align_top(Length::Fill)
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(10)
    .into()
}

fn side_bar(model: &Extraction) -> Element<'_, ExtractAction> {
    container(scrollable(model.image_buttons().width(250).spacing(10)).height(Length::Fill))
        .height(Length::Fill)
        .padding(Padding::new(10.0).left(0).right(5))
        .style(|theme| {
            transparent(theme).border(Border {
                color: Color::WHITE,
                width: 1.0,
                radius: 8.into(),
            })
        })
        .into()
}

fn image_view_selector(model: &Extraction) -> Element<'_, ExtractAction> {
    row![
        radio(
            "Image",
            ImageView::Image,
            Some(model.image_view),
            ExtractAction::SetImageView
        ),
        radio(
            "Text",
            ImageView::Text,
            Some(model.image_view),
            ExtractAction::SetImageView
        ),
        radio(
            "Split",
            ImageView::Split,
            Some(model.image_view),
            ExtractAction::SetImageView
        ),
    ]
    .spacing(10)
    .into()
}

fn page_content(model: &Extraction) -> Element<'_, ExtractAction> {
    match model.image_view {
        ImageView::Image => image_content(model),
        ImageView::Text => text_content(model),
        ImageView::Split => row![image_content(model), text_content(model)]
            .spacing(5)
            .into(),
    }
}

fn image_content(model: &Extraction) -> Element<'_, ExtractAction> {
    let content = model.get_current_page().map(|page| {
        Viewer::new(&page.handle)
            .width(Length::Fill)
            .height(Length::Fill)
    });

    container(content)
        .style(|theme| {
            transparent(theme).border(Border {
                color: Color::WHITE,
                width: 1.0,
                radius: 8.into(),
            })
        })
        .center(Length::Fill)
        .height(Length::Fill)
        .width(Length::Fill)
        .padding(Padding::new(10.0).right(5))
        .into()
}

fn text_content(model: &Extraction) -> Element<'_, ExtractAction> {
    let content = model.get_current_page().map(|page| text(&page.content));

    container(scrollable(content).width(Length::Fill))
        .style(|theme| {
            transparent(theme).border(Border {
                color: Color::WHITE,
                width: 1.0,
                radius: 8.into(),
            })
        })
        .height(Length::Fill)
        .width(Length::Fill)
        .padding(Padding::new(10.0).right(5))
        .into()
}

fn menu_bar(model @ Extraction { server_state, .. }: &Extraction) -> Element<'_, ExtractAction> {
    row![
        button("select").on_press(ExtractAction::SelectImages),
        button("save").on_press(ExtractAction::SaveText),
        MenuBar::new(vec![server_menu(server_state)]),
        extract_button(model),
        server_state.model_pick_list().map(Into::into)
    ]
    .width(Length::Fill)
    .spacing(10)
    .padding(Padding::default().bottom(15))
    .into()
}

fn server_menu(state: &Server) -> Item<'_, ExtractAction, Theme, Renderer> {
    Item::with_menu(
        menu_button("server"),
        Menu::new(vec![
            Item::new(ollama_connect(state).map(Into::into)),
            Item::new(think_selector(state).map(Into::into)),
        ])
        .width(400),
    )
}

fn extract_button(model: &Extraction) -> Element<'_, ExtractAction> {
    let (button_text, message) = if !model.server_state.handles.is_empty() {
        ("cancel", Some(ServerAction::Abort.into()))
    } else if !model.server_state.connected() {
        ("extract", None)
    } else {
        let msg = ExtractAction::ExtractText(model.current_page);
        ("extract", Some(msg))
    };

    button(text(button_text).center())
        .on_press_maybe(message)
        .into()
}

fn ollama_connect(state: &Server) -> Element<'_, ServerAction> {
    container(
        row![
            text("Ollama").center(),
            button("connect").on_press(ServerAction::Connect),
            horizontal(),
            execution_selector(state),
        ]
        .align_y(Vertical::Center)
        .spacing(5),
    )
    .align_left(Length::Fill)
    .padding(10)
    .into()
}

fn think_selector(state: &Server) -> Element<'_, ServerAction> {
    container(
        row![
            text("Think:"),
            radio(
                "None",
                Think::None,
                Some(state.settings.think),
                ServerAction::SetThink
            ),
            radio(
                "Low",
                Think::Low,
                Some(state.settings.think),
                ServerAction::SetThink
            ),
            radio(
                "Medium",
                Think::Medium,
                Some(state.settings.think),
                ServerAction::SetThink
            ),
            radio(
                "High",
                Think::High,
                Some(state.settings.think),
                ServerAction::SetThink
            ),
        ]
        .spacing(10),
    )
    .align_left(Length::Fill)
    .padding(10)
    .into()
}

fn execution_selector(model: &Server) -> Element<'_, ServerAction> {
    row![
        text("Excetion:"),
        radio(
            "Chain",
            Method::Chain,
            Some(model.method),
            ServerAction::SetMethod
        ),
        radio(
            "Batch",
            Method::Batch,
            Some(model.method),
            ServerAction::SetMethod
        )
    ]
    .spacing(10)
    .into()
}
