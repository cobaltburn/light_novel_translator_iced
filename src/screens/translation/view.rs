use crate::{
    llm::Settings,
    page::DisplayType,
    screens::translation::{Action, Translation},
    server::{
        self, Server, context_window_input, execution_selector, ollama_input, server_setting_input,
        think_selector,
    },
    ui::{SidebarAction, context_menu_button, menu_button, page_side_bar, rich_text_scrollable},
};
use iced::widget::{
    Button, Column, Container, Row, button, column, container, container::transparent, row, rule,
    scrollable, space::vertical, stack, text,
};
use iced::{Border, Color, Element, Length, Padding, Renderer, Theme, alignment::Vertical, border};
use iced_aw::{ContextMenu, Menu, MenuBar, menu::Item};
use std::iter;

pub fn translation_view(model: &Translation) -> Element<'_, Action> {
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
    let active = model.server.connected() && model.server.handles.is_empty();
    page_side_bar(&model.pages, model.current_page, active)
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
        ("cancel", Some(Action::Cancel))
    } else if !model.server.connected() || model.epub_name().is_none() {
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
            Item::new(glossary_select(model)),
            Item::new(file_menu_buttons(model)),
        ])
        .spacing(10)
        .width(400),
    )
}

fn file_menu_buttons(state: &Translation) -> Element<'_, Action> {
    let file_name = state.epub_name();
    let recover =
        (file_name.is_some() && state.server.handles.is_empty()).then_some(Action::Recover);
    row![
        button(text("save").center())
            .on_press_maybe(file_name.map(|e| Action::Save(e.into_owned())))
            .padding(5),
        button(text("recover").center())
            .on_press_maybe(recover)
            .padding(5)
    ]
    .align_y(Vertical::Center)
    .spacing(10)
    .into()
}

fn epub_select(model: &Translation) -> Element<'_, Action> {
    row![
        button(text("epub").center()).on_press(Action::OpenEpub),
        container(text(model.epub_name().unwrap_or_default()))
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
    .into()
}

fn glossary_select(model: &Translation) -> Element<'_, Action> {
    let can_select = model.file_path.is_some() && model.server.handles.is_empty();
    let action = can_select.then_some(Action::OpenGlossary);
    row![
        button(text("glossary").center()).on_press_maybe(action),
        container(text(model.glossary_name().unwrap_or_default()))
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
    .into()
}

impl SidebarAction for Action {
    fn set_page(page: usize) -> Self {
        Action::SetPage(page)
    }
    fn save_page(name: String, page: usize) -> Self {
        Action::SavePage { name, page }
    }
    fn translate(page: usize) -> Self {
        Action::Translate(page)
    }
    fn translate_page(page: usize) -> Self {
        Action::TranslatePage(page)
    }
    fn translate_part(page: usize, part: usize) -> Self {
        Action::TranslatePart { page, part }
    }
}
