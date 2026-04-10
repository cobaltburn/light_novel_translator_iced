use std::path::PathBuf;

use crate::{
    actions::consensus_action::ConsensusAction,
    model::{Activity, page::Page, server::Server},
    widget::{active_mark, check_mark, context_menu_button, cross_mark, text_button},
};
use iced::{
    Color, Element, Length, Padding,
    widget::{Column, column, container, row, scrollable, text},
};
use iced_aw::ContextMenu;

#[derive(Debug, Default)]
pub struct Consensus {
    pub server: Server,
    pub file_name: String,
    pub current_page: usize,
    pub pages: Vec<Page>,
    pub translations: Vec<Vec<String>>,
}

impl Consensus {
    pub fn current_content(&self) -> Option<impl Iterator<Item = &String>> {
        let text = self
            .pages
            .get(self.current_page)?
            .sections
            .iter()
            .map(|e| &e.text);

        Some(text)
    }

    pub fn path_buttons(&self) -> Column<'_, ConsensusAction> {
        self.pages
            .iter()
            .enumerate()
            .map(|(i, page)| {
                let name = page.path.file_stem().unwrap().to_string_lossy();
                let button_text =
                    text!("{}. {}", i + 1, &name)
                        .width(Length::Fill)
                        .style(move |theme| {
                            if self.current_page == i {
                                text::primary(theme)
                            } else {
                                text::default(theme)
                            }
                        });

                let mark = match page.activity {
                    Activity::Incomplete => None,
                    Activity::Complete => Some(check_mark()),
                    Activity::Error(i) => Some(row![text(i), cross_mark()].spacing(5).into()),
                    Activity::Active => Some(active_mark()),
                };

                let button_content = row![button_text]
                    .push(mark)
                    .padding(Padding::default().right(10));

                let count = page.sections.len();
                let active = self.server.connected() && self.server.handles.is_empty();
                let underlay = text_button(button_content).on_press(ConsensusAction::SetPage(i));

                ContextMenu::new(underlay, move || {
                    path_button_overlay(count, name.to_string(), i, active)
                })
                .into()
            })
            .collect()
    }
}

fn path_button_overlay<'a>(
    count: usize,
    name: String,
    page: usize,
    active: bool,
) -> Element<'a, ConsensusAction> {
    let part_buttons = (0..count).map(|part| {
        context_menu_button(text!("translate part {}", part + 1).color(Color::WHITE))
            .on_press_maybe(active.then_some(ConsensusAction::TranslatePart { page, part }))
            .into()
    });

    let overlay = column![
        context_menu_button(text("save").color(Color::WHITE))
            .on_press(ConsensusAction::SavePage { name, page }),
        context_menu_button(text("translate").color(Color::WHITE))
            .on_press_maybe(active.then_some(ConsensusAction::Translate(page))),
        context_menu_button(text("translate page").color(Color::WHITE))
            .on_press_maybe(active.then_some(ConsensusAction::TranslatePage(page)))
    ]
    .extend(part_buttons)
    .padding(5)
    .spacing(5);

    container(scrollable(overlay).width(Length::Fill))
        .style(container::rounded_box)
        .max_height(400)
        .width(300)
        .into()
}

pub struct Folder {
    pub path: PathBuf,
    pub text: Vec<(PathBuf, String)>,
}
