use crate::{
    actions::trans_action::TransAction,
    message::Message,
    model::{server::Server, translation::Translation},
    view::{DisplayType, menu_button, rich_text_scrollable},
    widget::{
        context_menu_button,
        page_sidebar::build_path_buttons,
        server_widget::{context_window_input, execution_selector, ollama_input, think_selector},
    },
};
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{
    Border, Color, Element, Function, Length, Padding, Renderer, Theme,
    alignment::Vertical,
    border::Radius,
    widget::{Button, Container, Row, container::transparent, lazy, space::vertical, stack},
};
use iced_aw::{ContextMenu, Menu, MenuBar, TabBar, card::Status, menu::Item, style::tab_bar};
use std::collections::BTreeMap;

pub fn translation_view(
    models: &BTreeMap<usize, Translation>,
    tab_id: usize,
) -> Element<'_, Message> {
    let tabs = models.iter().map(|(&i, e)| (i, e.tab_label())).collect();

    let mut tabs = TabBar::with_tab_labels(tabs, Message::SelectTab)
        .set_active_tab(&tab_id)
        .padding(Padding::new(0.0).left(1))
        .height(Length::Fixed(40.0))
        .text_size(13.0)
        .style(|theme, status| match status {
            Status::Active | Status::Hovered => tab_bar::Style {
                text_color: Color::BLACK,
                icon_color: Color::from_rgb8(0, 255, 0),
                ..tab_bar::primary(theme, status)
            },
            _ => tab_bar::Style {
                icon_color: Color::from_rgb8(0, 255, 0),
                ..tab_bar::primary(theme, status)
            },
        });

    if models.len() > 1 {
        tabs = tabs.on_close(Message::CloseTab);
    }

    let model = models.get(&tab_id);
    let tab = model.map(|model| tab(model).map(Message::TransAction.with(tab_id)));
    let add_tab = model.map(|model| new_tab_button(model));

    column![row![tabs, add_tab].height(Length::Fixed(40.0)), tab].into()
}

fn new_tab_button(_model: &Translation) -> Element<'_, Message> {
    button(container(text("+").center()).center(Length::Fill))
        .on_press(Message::AddTab)
        .width(Length::Fixed(50.0))
        .height(Length::Fill)
        .style(|theme, status| button::Style {
            border: Border {
                color: Color::WHITE,
                width: 0.5,
                radius: Radius::from(0),
            },
            ..button::primary(theme, status)
        })
        .into()
}

fn tab(model: &Translation) -> Element<'_, TransAction> {
    let page = model.current_page();
    let current_page = model.current_page;
    let can_translate = model.server.handles.is_empty()
        && model.server.connected()
        && !model.file_name().is_empty();
    let on_press = move |part| {
        can_translate.then_some(TransAction::TranslatePart {
            page: current_page,
            part,
        })
    };

    let error_cards = page.map(|p| p.error_cards(on_press));
    let content = page
        .map(|p| p.spans(model.display, on_press))
        .unwrap_or_default();

    container(column![
        vertical(),
        column![
            menu_bar(model),
            row![
                side_bar(model),
                stack![
                    ContextMenu::new(rich_text_scrollable(content), || container(column![
                        context_menu_button(text("full").color(Color::WHITE))
                            .on_press(TransAction::SetDisplay(DisplayType::Full))
                            .width(Length::Fill),
                        context_menu_button(text("end").color(Color::WHITE))
                            .on_press(TransAction::SetDisplay(DisplayType::End))
                            .width(Length::Fill),
                        context_menu_button(text("japanese").color(Color::WHITE))
                            .on_press(TransAction::SetDisplay(DisplayType::Japanese))
                            .width(Length::Fill)
                    ])
                    .style(container::rounded_box)
                    .width(100)
                    .into()),
                    error_cards
                ]
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

fn side_bar(model: &Translation) -> Container<'_, TransAction> {
    let buttons = lazy(model.sidebar_deps(), |deps| {
        build_path_buttons(deps).width(250).spacing(10)
    });
    container(scrollable(buttons).spacing(5).height(Length::Fill))
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

fn menu_bar(
    model @ Translation {
        server: server_state,
        ..
    }: &Translation,
) -> Row<'_, TransAction> {
    row![
        MenuBar::new(vec![epub_menu(model), server_menu(server_state)]).spacing(5),
        translate_button(model),
        server_state.model_pick_list().map(Into::into),
    ]
    .width(Length::Fill)
    .spacing(5)
    .padding(Padding::default().bottom(15))
}

fn translate_button(model: &Translation) -> Button<'_, TransAction> {
    let (button_text, message) = if !model.server.handles.is_empty() {
        ("cancel", Some(TransAction::CancelTranslate))
    } else if !model.server.connected() || model.file_name().is_empty() {
        ("translate", None)
    } else {
        let msg = TransAction::Translate(model.current_page);
        ("translate", Some(msg))
    };

    button(text(button_text).center()).on_press_maybe(message)
}

fn server_menu(state: &Server) -> Item<'_, TransAction, Theme, Renderer> {
    Item::with_menu(
        menu_button("server"),
        Menu::new(vec![
            Item::new(ollama_input().map(Into::into)),
            Item::new(think_selector(state).map(Into::into)),
            Item::new(execution_selector(state).map(Into::into)),
            Item::new(context_window_input(state).map(Into::into)),
        ])
        .padding(10)
        .spacing(10)
        .width(400),
    )
}

fn epub_menu(model: &Translation) -> Item<'_, TransAction, Theme, Renderer> {
    Item::with_menu(
        menu_button("epub"),
        Menu::new(vec![
            Item::new(epub_select(model)),
            Item::new(file_menu_buttons(model)),
        ])
        .spacing(10)
        .width(400),
    )
}

fn file_menu_buttons(state: &Translation) -> Element<'_, TransAction> {
    let file_name = state.file_name();
    let not_empty = !file_name.is_empty();
    let save = not_empty.then_some(TransAction::SaveTranslation(file_name));
    let recovery = not_empty.then_some(TransAction::Recover);

    row![
        button(text("save").center())
            .on_press_maybe(save)
            .padding(5),
        button(text("recover").center())
            .on_press_maybe(recovery)
            .padding(5)
    ]
    .align_y(Vertical::Center)
    .spacing(10)
    .into()
}

fn epub_select(model: &Translation) -> Row<'_, TransAction> {
    row![
        button(text("epub").center()).on_press(TransAction::OpenEpub),
        container(text(model.file_name()))
            .width(Length::Fill)
            .padding(5)
            .style(|theme| transparent(theme).border(Border {
                color: Color::WHITE,
                width: 0.5,
                radius: 5.into(),
            }))
    ]
    .align_y(Vertical::Center)
    .padding(5)
    .spacing(10)
}
