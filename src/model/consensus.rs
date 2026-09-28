use crate::{
    actions::consensus,
    model::{Page, Server},
    view::DisplayType,
    widget::{SidebarAction, SidebarDeps, SidebarRow},
};
use iced::widget::button::Status;
use iced::{
    Border, Color, Element, Length, Renderer, Theme,
    alignment::Vertical,
    widget::{button, container, row, text},
};
use iced_aw::menu::Item;
use std::{iter::once, path::PathBuf};

#[derive(Debug, Default)]
pub struct Consensus {
    pub server: Server,
    pub file_path: PathBuf,
    pub current_page: usize,
    pub candidates: Vec<Candidate>,
    pub pages: Vec<Page>,
    pub translations: Vec<Vec<String>>,
    pub display: DisplayType,
}

impl Consensus {
    pub fn current_page(&self) -> Option<&Page> {
        self.pages.get(self.current_page)
    }

    pub fn file_name(&self) -> String {
        self.file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }

    pub fn candidate_items(&self) -> Vec<Item<'_, consensus::Action, Theme, Renderer>> {
        self.candidates
            .iter()
            .enumerate()
            .map(|(i, Candidate { name, .. })| Item::new(candidate_select(Some(i), name)))
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
                activity: p.activity,
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

impl SidebarAction for consensus::Action {
    fn set_page(page: usize) -> Self {
        consensus::Action::SetPage(page)
    }
    fn save_page(name: String, page: usize) -> Self {
        consensus::Action::SavePage { name, page }
    }
    fn translate(page: usize) -> Self {
        consensus::Action::Consensus(page)
    }
    fn translate_page(page: usize) -> Self {
        consensus::Action::ConsensusPage(page)
    }
    fn translate_part(page: usize, part: usize) -> Self {
        consensus::Action::ConsensusPart { page, part }
    }
}

fn candidate_select(i: Option<usize>, folder: &str) -> Element<'_, consensus::Action> {
    let x_button = i.map(|i| {
        button(text("x").center())
            .style(|theme, status| match status {
                Status::Hovered => button::primary(theme, status),
                _ => button::text(theme, status),
            })
            .on_press(consensus::Action::DropCandidate(i))
    });

    row![
        button(text("candidate").center()).on_press(consensus::Action::SelectCandidate(i)),
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
