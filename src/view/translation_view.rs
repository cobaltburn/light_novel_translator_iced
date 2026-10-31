use crate::{
    actions::{
        server,
        translation::{self, Action},
    },
    message::Message,
    model::{Server, Settings, Translation},
    view::{DisplayType, menu_button, rich_text_scrollable},
    widget::{
        build_path_buttons, context_menu_button, context_window_input, execution_selector,
        num_ctx_input, ollama_input, server_setting_input, think_selector,
    },
};
use iced::{
    Border, Color, Element, Function, Length, Padding, Renderer, Theme,
    alignment::Vertical,
    border::Radius,
    widget::{
        Button, Column, Container, Row, container::transparent, lazy, rule, space::vertical, stack,
    },
};
use iced::{
    border,
    widget::{button, column, container, row, scrollable, text},
};
use iced_aw::{ContextMenu, Menu, MenuBar, TabBar, card::Status, menu::Item, style::tab_bar};
use std::{collections::BTreeMap, iter};

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
    let add_tab = model.map(|_| new_tab_button());

    column![row![tabs, add_tab].height(Length::Fixed(40.0)), tab].into()
}

fn new_tab_button() -> Element<'static, Message> {
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

fn tab(model: &Translation) -> Element<'_, Action> {
    let page = model.current_page();
    let current_page = model.current_page;
    let can_translate = model.can_translate();
    let on_press = move |part| {
        can_translate.then_some(Action::TranslatePart {
            page: current_page,
            part,
        })
    };

    let error_cards = page.map(|p| p.error_cards(on_press));
    let content = page.map_or_default(|p| p.spans(model.display, on_press));
    let sections = page.map(|p| p.sections.len());

    container(column![
        vertical(),
        column![
            menu_bar(model),
            row![
                side_bar(model),
                stack![
                    ContextMenu::new(rich_text_scrollable(content), move || view_context_menu(
                        sections
                    )),
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

fn view_context_menu<'a>(sections: Option<usize>) -> Element<'a, Action> {
    let display_buttons = [
        ("full", Action::SetDisplay(DisplayType::Full)),
        ("end", Action::SetDisplay(DisplayType::End)),
        ("japanese", Action::SetDisplay(DisplayType::Japanese)),
    ];

    let sections = sections.map(section_select);
    let menu: Column<_> = display_buttons
        .into_iter()
        .map(|(t, action)| view_context_menu_button(t, action).into())
        .chain(sections.into_iter().flatten())
        .collect();

    container(
        scrollable(menu.spacing(5).padding(Padding::default().vertical(5))).style(
            |theme, status| {
                let style = scrollable::default(theme, status);
                scrollable::Style {
                    vertical_rail: scrollable::Rail {
                        border: border::rounded(14),
                        ..style.vertical_rail
                    },
                    ..style
                }
            },
        ),
    )
    .style(|theme| container::rounded_box(theme).border(border::rounded(7)))
    .max_height(400)
    .width(300)
    .into()
}

fn section_select<'a>(sections: usize) -> impl Iterator<Item = Element<'a, Action>> {
    iter::once(rule::horizontal(1).into()).chain((0..sections).into_iter().map(|i| {
        view_context_menu_button(format!("section {}", i + 1), Action::ScrollToSection(i)).into()
    }))
}

fn view_context_menu_button<'a>(
    button_text: impl text::IntoFragment<'a>,
    action: Action,
) -> Button<'a, Action> {
    context_menu_button(text(button_text).color(Color::WHITE))
        .on_press(action)
        .padding(Padding::new(5.0).left(30))
        .width(Length::Fill)
}

fn side_bar(model: &Translation) -> Container<'_, Action> {
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
) -> Row<'_, Action> {
    row![
        MenuBar::new(vec![epub_menu(model), server_menu(server_state)]).spacing(5),
        translate_button(model),
        server_state.model_pick_list().map(Into::into),
    ]
    .width(Length::Fill)
    .spacing(5)
    .padding(Padding::default().bottom(15))
}

fn translate_button(model: &Translation) -> Button<'_, Action> {
    let (button_text, message) = if !model.server.handles.is_empty() {
        ("cancel", Some(Action::CancelTranslate))
    } else if !model.server.connected() || model.file_name().is_none() {
        ("translate", None)
    } else {
        let msg = Action::Translate(model.current_page);
        ("translate", Some(msg))
    };

    button(text(button_text).center()).on_press_maybe(message)
}

fn server_menu(state: &Server) -> Item<'_, Action, Theme, Renderer> {
    let Settings {
        temperature,
        top_p,
        repeat_penalty,
        ..
    } = state.settings;
    Item::with_menu(
        menu_button("server"),
        Menu::new(vec![
            Item::new(ollama_input().map(Into::into)),
            Item::new(think_selector(state).map(Into::into)),
            Item::new(execution_selector(state).map(Into::into)),
            Item::new(context_window_input(state).map(Into::into)),
            Item::new(num_ctx_input(state).map(Into::into)),
            Item::new(
                server_setting_input(
                    "Tempature:",
                    temperature,
                    0.0..=2.0,
                    server::Action::SetTemp,
                )
                .map(Into::into),
            ),
            Item::new(
                server_setting_input("Top p:", top_p, 0.0..=1.0, server::Action::SetTopP)
                    .map(Into::into),
            ),
            Item::new(
                server_setting_input(
                    "Repeat Penalty:",
                    repeat_penalty,
                    0.0..=2.0,
                    server::Action::SetRepeatPenalty,
                )
                .map(Into::into),
            ),
        ])
        .padding(10)
        .spacing(10)
        .width(400),
    )
}

fn epub_menu(model: &Translation) -> Item<'_, Action, Theme, Renderer> {
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

fn file_menu_buttons(state: &Translation) -> Element<'_, Action> {
    let file_name = state.file_name();
    let recover =
        (file_name.is_some() && state.server.handles.is_empty()).then_some(Action::Recover);
    row![
        button(text("save").center())
            .on_press_maybe(file_name.map(Action::SaveTranslation))
            .padding(5),
        button(text("recover").center())
            .on_press_maybe(recover)
            .padding(5)
    ]
    .align_y(Vertical::Center)
    .spacing(10)
    .into()
}

fn epub_select(model: &Translation) -> Row<'_, Action> {
    row![
        button(text("epub").center()).on_press(translation::Action::OpenEpub),
        container(text(model.file_name().unwrap_or_default()))
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
