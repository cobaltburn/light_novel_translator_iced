use crate::{
    actions::trans_action::TransAction,
    model::{
        Activity,
        page::{Page, Section},
        server::Server,
    },
    widget::{active_mark, check_mark, context_menu_button, cross_mark, text_button},
};
use iced::{
    Color, Element, Length, Padding,
    widget::{Column, column, container, row, scrollable, text},
};
use iced_aw::{ContextMenu, TabLabel};
use std::path::PathBuf;

#[derive(Default, Debug)]
pub struct Translation {
    pub server: Server,
    pub file_path: PathBuf,
    pub current_page: usize,
    pub pages: Vec<Page>,
}

impl Translation {
    pub fn tab_label(&self) -> TabLabel {
        match self.server.handles.is_empty() {
            true => TabLabel::Text(self.file_name()),
            false => TabLabel::IconText('\u{25CF}', self.file_name()),
        }
    }

    pub fn file_name(&self) -> String {
        self.file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }

    pub fn current_content(&self) -> Option<Vec<&String>> {
        Some(
            self.pages
                .get(self.current_page)?
                .sections
                .iter()
                .map(|e| &e.content)
                .collect(),
        )
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

    pub fn sidebar_deps(&self) -> SidebarDeps {
        let inactive = self.server.connected() && self.server.handles.is_empty();
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
            inactive,
            rows,
        }
    }
}

#[derive(Hash)]
pub struct SidebarDeps {
    pub current_page: usize,
    pub inactive: bool,
    pub rows: Vec<SidebarRow>,
}

#[derive(Hash)]
pub struct SidebarRow {
    pub name: String,
    pub activity: Activity,
    pub section_count: usize,
}

pub fn build_path_buttons(deps: &SidebarDeps) -> Column<'static, TransAction> {
    let current = deps.current_page;
    let inactive = deps.inactive;

    deps.rows
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let name = entry.name.clone();
            let activity = entry.activity.clone();
            let section_count = entry.section_count;

            let button_text = text!("{}. {}", i + 1, &name)
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
                text_button(button_content).on_press(TransAction::SetPage(i)),
                move || path_button_overlay(section_count, name.clone(), i, inactive),
            )
            .into()
        })
        .collect()
}

impl From<Server> for Translation {
    fn from(server: Server) -> Self {
        Self {
            server,
            ..Default::default()
        }
    }
}

fn path_button_overlay<'a>(
    count: usize,
    name: String,
    page: usize,
    inactive: bool,
) -> Element<'a, TransAction> {
    let overlay = column![
        context_menu_button(text("save").color(Color::WHITE))
            .on_press(TransAction::SavePage { name, page }),
        context_menu_button(text("translate").color(Color::WHITE))
            .on_press_maybe(inactive.then_some(TransAction::Translate(page))),
        context_menu_button(text("translate page").color(Color::WHITE))
            .on_press_maybe(inactive.then_some(TransAction::TranslatePage(page)))
    ]
    .extend((0..count).map(|part| {
        context_menu_button(text!("translate part {}", part + 1).color(Color::WHITE))
            .on_press_maybe(inactive.then_some(TransAction::TranslatePart { page, part }))
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
