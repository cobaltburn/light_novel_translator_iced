use crate::{
    actions::consensus_action::ConsensusAction,
    model::{
        Activity,
        page::{Page, Section},
        server::Server,
    },
    widget::{active_mark, check_mark, context_menu_button, cross_mark, text_button},
};
use iced::widget::button::Status;
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

    pub fn current_sections(&self) -> Option<&[Section]> {
        Some(&self.pages.get(self.current_page)?.sections)
    }

    pub fn candidate_items(&self) -> Vec<Item<'_, ConsensusAction, Theme, Renderer>> {
        self.candidates
            .iter()
            .enumerate()
            .map(|(i, Candidate { name, .. })| Item::new(candidate_select(Some(i), &name)))
            .chain(once(Item::new(candidate_select(None, ""))))
            .collect()
    }

    pub fn sidebar_deps(&self) -> SidebarDeps {
        let active = self.server.connected() && self.server.handles.is_empty();
        let rows = self
            .pages
            .iter()
            .map(|p| SidebarRow {
                name: p
                    .path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                activity: p.activity.clone(),
                section_count: p.sections.len(),
            })
            .collect();
        SidebarDeps {
            current_page: self.current_page,
            active,
            rows,
        }
    }
}

#[derive(Hash)]
pub struct SidebarDeps {
    pub current_page: usize,
    pub active: bool,
    pub rows: Vec<SidebarRow>,
}

#[derive(Hash)]
pub struct SidebarRow {
    pub name: String,
    pub activity: Activity,
    pub section_count: usize,
}

pub fn build_path_buttons(deps: &SidebarDeps) -> Column<'static, ConsensusAction> {
    let current = deps.current_page;
    let active = deps.active;

    deps.rows
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let name = entry.name.clone();
            let activity = entry.activity.clone();
            let section_count = entry.section_count;

            let button_text =
                text!("{}. {}", i + 1, &name)
                    .width(Length::Fill)
                    .style(move |theme| {
                        if current == i {
                            text::primary(theme)
                        } else {
                            text::default(theme)
                        }
                    });

            let button_content = row![button_text]
                .push(match activity {
                    Activity::Incomplete => None,
                    Activity::Complete => Some(check_mark()),
                    Activity::Error(e) => Some(row![text(e), cross_mark()].spacing(5).into()),
                    Activity::Active => Some(active_mark()),
                })
                .padding(Padding::default().right(10));

            ContextMenu::new(
                text_button(button_content).on_press(ConsensusAction::SetPage(i)),
                move || path_button_overlay(section_count, name.clone(), i, active),
            )
            .into()
        })
        .collect()
}

fn path_button_overlay<'a>(
    count: usize,
    name: String,
    page: usize,
    active: bool,
) -> Element<'a, ConsensusAction> {
    let overlay = column![
        context_menu_button(text("save").color(Color::WHITE))
            .on_press(ConsensusAction::SavePage { name, page }),
        context_menu_button(text("translate").color(Color::WHITE))
            .on_press_maybe(active.then_some(ConsensusAction::Consensus(page))),
        context_menu_button(text("translate page").color(Color::WHITE))
            .on_press_maybe(active.then_some(ConsensusAction::ConsensusPage(page)))
    ]
    .extend((0..count).map(|part| {
        context_menu_button(text!("translate part {}", part + 1).color(Color::WHITE))
            .on_press_maybe(active.then_some(ConsensusAction::ConsensusPart { page, part }))
            .into()
    }))
    .padding(5)
    .spacing(5);

    container(scrollable(overlay).width(Length::Fill))
        .style(container::rounded_box)
        .max_height(400)
        .width(300)
        .into()
}

fn candidate_select(i: Option<usize>, folder: &str) -> Element<'_, ConsensusAction> {
    let x_button = i.map(|i| {
        button(text("x").center())
            .style(|theme, status| match status {
                Status::Hovered => button::primary(theme, status),
                _ => button::text(theme, status),
            })
            .on_press(ConsensusAction::DropCandidate(i))
    });

    row![
        button(text("candidate").center()).on_press(ConsensusAction::SelectCandidate(i)),
        container(
            row![text(folder).width(Length::Fill)]
                .push(x_button)
                .align_y(Vertical::Center)
        )
        .height(35)
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
