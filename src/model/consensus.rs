use crate::{
    actions::consensus_action::ConsensusAction,
    model::{Activity, page::Page, server::Server},
    widget::{active_mark, check_mark, context_menu_button, cross_mark, text_button},
};
use iced::{
    Border, Color, Element, Length, Padding, Renderer, Theme,
    alignment::Vertical,
    widget::{Column, button, column, container, row, scrollable, text},
};
use iced_aw::{ContextMenu, menu::Item};
use std::{iter::once, path::PathBuf};

#[derive(Debug, Default)]
pub struct Consensus {
    pub server: Server,
    pub file_path: PathBuf,
    pub current_page: usize,
    pub candidates: Vec<Candidate>,
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
            .map(|e| &e.content);

        Some(text)
    }

    pub fn file_name(&self) -> String {
        self.file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }

    pub fn current_jap_errors(&self) -> Option<&[usize]> {
        Some(&self.pages.get(self.current_page)?.jap_error)
    }

    pub fn current_size_errors(&self) -> Option<&[usize]> {
        Some(&self.pages.get(self.current_page)?.size_error)
    }

    pub fn canidate_items(&self) -> Vec<Item<'_, ConsensusAction, Theme, Renderer>> {
        self.candidates
            .iter()
            .enumerate()
            .map(|(i, Candidate { name, .. })| Item::new(canidate_select(Some(i), &name)))
            .chain(once(Item::new(canidate_select(None, ""))))
            .collect()
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
            .on_press_maybe(active.then_some(ConsensusAction::ConsensusPart { page, part }))
            .into()
    });

    let overlay = column![
        context_menu_button(text("save").color(Color::WHITE))
            .on_press(ConsensusAction::SavePage { name, page }),
        context_menu_button(text("translate").color(Color::WHITE))
            .on_press_maybe(active.then_some(ConsensusAction::Consensus(page))),
        context_menu_button(text("translate page").color(Color::WHITE))
            .on_press_maybe(active.then_some(ConsensusAction::ConsensusPage(page)))
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

fn canidate_select(i: Option<usize>, folder: &str) -> Element<'_, ConsensusAction> {
    let x_button = i.map(|i| {
        button(text("x").center())
            .style(button::text)
            .on_press(ConsensusAction::DropCandidate(i))
    });

    row![
        button(text("canidate").center()).on_press(ConsensusAction::SelectCandidate(i)),
        container(
            row![text(folder).width(Length::Fill)]
                .push(x_button)
                .align_y(Vertical::Center)
        )
        .width(Length::Fill)
        .padding(5)
        .style(|theme| {
            container::transparent(theme).border(Border {
                color: Color::WHITE,
                width: 0.5,
                radius: 5.into(),
            })
        }),
    ]
    .align_y(Vertical::Center)
    .spacing(10)
    .into()
}

#[derive(Debug, Default)]
pub struct Candidate {
    pub name: String,
    pub pages: Vec<(PathBuf, Vec<String>)>,
}
